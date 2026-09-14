# How We Stopped Mapping Errors and Started Classifying Them.

Every payment processor has its own way of telling you something went wrong. And none of them agree on how.

One returns a `402` with a clean JSON body. Another returns a `200 OK` — technically a success — but buries a `"status": "failed"` three levels deep inside an XML response. A third one gives you an error code with no message, and you're left guessing whether to retry or just give up.

When you're building against one processor, this is annoying. When you're building a payment library that talks to dozens of them, it becomes the entire problem.

That's what we ran into with Prism.

## What Prism actually is

Prism is a connector library. It knows how to talk to Stripe, Adyen, Razorpay, Noon, Cybersource, and a lot of others — and more importantly, it turns their wildly different APIs into one consistent interface for anyone building on top of it.

The core is in Rust. The SDKs — Node, Python, Java — sit on top of it. The idea is that you write your payment logic once, against one interface, and Prism handles the translation underneath.

That translation is mostly about request and response shapes. But errors? That's where it gets interesting.

## The problem with just "passing errors through"

The naive approach is to just surface whatever the connector gives you. Error comes in, error goes out. Simple.

It falls apart fast.

If your error handling logic is written against Stripe's error codes and you switch to Adyen, you're rewriting. If a processor returns a network timeout that *looks* like a payment failure, your retry logic fires when it shouldn't — or doesn't fire when it should. And if something goes wrong at 2am, the on-call engineer is staring at a raw connector response with no context about what actually happened or what to do next.

Passing errors through isn't simple. It's just deferring the problem to whoever's downstream.

## The first thing we got wrong

We started by trying to map everything into one big enum. Every possible error state across every connector, flattened into a single list.

It grew fast and became hard to reason about. Is `PaymentDeclined` the same as `InsufficientFunds`? Technically no, but do they require different handling from an SDK user's perspective? Usually not. We were adding granularity that nobody upstream was actually using, and the mapping tables for each connector were getting unwieldy.

The real insight came when we stopped thinking about errors as *values* and started thinking about them as *layers*.

## Three layers, not one list

We settled on grouping errors by where they originate, because that's what actually determines what should happen next.

**Infrastructure errors** — the server returned a 5xx, the network timed out, the connector is down. These have nothing to do with the payment itself. They're handled at the transport layer and treated as retryable by default. There's no point passing them down to application logic as if they were business failures.

```rust
fn get_5xx_error_response(&self, res: Response) -> ErrorResponse {
    let error_message = match res.status_code {
        500 => "internal_server_error",
        502 => "bad_gateway",
        503 => "service_unavailable",
        504 => "gateway_timeout",
        _   => "unknown_error",
    };
    // Returns a retryable infrastructure error — never reaches business logic
}
```

This runs at the base trait level. Every connector gets it for free. No connector-specific code needed, because a 503 from Stripe means the same thing as a 503 from Adyen — the connector is temporarily unavailable, try later.

**Authentication and validation errors** — wrong API key, malformed request, a `401` from the connector's auth layer. Cybersource, for example, returns a very explicit `401` when credentials are invalid. These fail early and fast. The fix is always on the caller's side, not something to retry.

**Business logic errors** — the card was declined, the account has insufficient funds, the transaction limit was exceeded. These are connector-specific by nature, but they map to a predictable set of outcomes. Each connector adapter owns the translation from its error codes to these normalized outcomes.

Separating these three layers made a surprising amount of complexity disappear. A lot of the conditional logic we had scattered across the codebase was really just conflating infrastructure failures with business failures. Once you pull them apart, each one becomes much easier to reason about.

## What the unified error actually looks like

The thing a developer using the SDK sees is a normalized error — one shape, regardless of which connector is underneath.

Under the hood, that shape has three distinct parts. The protobuf definition captures it cleanly:

```protobuf
message ErrorInfo {
  optional UnifiedErrorDetails   unified_details   = 1;
  optional IssuerErrorDetails    issuer_details    = 2;
  optional ConnectorErrorDetails connector_details = 3;
}
```

There's the **unified layer** — the normalized code, message, and category that every caller can rely on regardless of connector. There's the **issuer layer** — information that came from the card network itself, like a bank decline reason. And there's the **connector layer** — the raw, untouched response from the processor.

At runtime, the `ErrorResponse` that flows through the system carries all of this in one place:

```rust
ErrorResponse {
    code: "DECLINED",
    message: "Insufficient funds",
    reason: Some("Card declined by issuer"),
    status_code: 402,
    attempt_status: Some(AttemptStatus::Failure),
    connector_transaction_id: Some("txn_123"),
    network_advice_code: Some("01"),           // Hint from the card network
    network_decline_code: Some("05"),          // Bank's own decline code
    network_error_message: Some("Do not honor"),
}
```

The `connector_details` field preserves the original processor response, untouched. Developers hate black boxes — if a payment fails and they need to file a support ticket or cross-reference the connector's own documentation, they need to see exactly what came back. Hiding it behind a generic message just creates a support burden that falls on you.

## The retry question

One of the messier problems in payment error handling is knowing whether to try again.

A card decline is not retryable — trying the same card twice won't change the outcome. A gateway timeout probably is retryable. A duplicate transaction error is a special case that depends entirely on context. If you get this wrong, you either annoy users with redundant failures or accidentally charge them twice.

The way we handled it was by surfacing what the network actually tells you. Processors like Cybersource include advice codes in their responses — structured hints from the card network about what the merchant should do next. We expose those directly in the error response rather than collapsing them into a boolean. It preserves the actual signal instead of flattening it into a guess.

```rust
let network_advice_code = processor_information.as_ref().and_then(|info| {
    info.merchant_advice
        .as_ref()
        .and_then(|merchant_advice| merchant_advice.code_raw.clone())
});
```

The card network is already telling you what to do. You just have to stop throwing that information away.

## One place to change

The part we're most happy with is how localized the connector-specific knowledge is.

Each connector implements its own error mapping. When Cybersource returns a particular processor code, the translation to a normalized error category happens inside Cybersource's adapter — nowhere else. When Checkout.com categorizes an error differently from Adyen, each adapter handles that independently. The SDK doesn't change. The application code calling the SDK doesn't change.

The shape of that mapping varies by connector. Some are simple; some go deep. Razorpay, for instance, works at the top level — it maps its broad error category codes directly to attempt statuses:

```rust
let attempt_status = match error.code.as_str() {
    "BAD_REQUEST_ERROR"   => AttemptStatus::Failure,
    "AUTHENTICATION_ERROR" => AttemptStatus::AuthenticationFailed,
    "SERVER_ERROR"        => AttemptStatus::Pending, // Retryable
    _                     => AttemptStatus::Pending,
};
```

Checkout.com goes deeper — rather than mapping to a status directly, it first classifies the error *type*, which then drives what the library does next (retry, surface to user, escalate):

```rust
fn get_connector_error_type(&self, error_code: String) -> ConnectorErrorType {
    match error_code.as_str() {
        "card_expired"              => ConnectorErrorType::UserError,
        "amount_exceeds_balance"    => ConnectorErrorType::BusinessError,
        "api_calls_quota_exceeded"  => ConnectorErrorType::TechnicalError,
        // ... and many more
        _                           => ConnectorErrorType::UnknownError,
    }
}
```

The distinction matters: a `UserError` means the end user did something wrong (expired card, bad details). A `TechnicalError` means the integration or infrastructure has a problem. A `BusinessError` means the payment legitimately can't go through — different causes, different responses, different retry behaviour. Both Razorpay and Checkout.com converge on the same normalized output upstream despite taking completely different paths to get there.

When a new connector returns some unusual error code, the fix is in one file. That's it.

It's a property that's easy to undervalue until you're maintaining integrations with 30+ processors. Consistency here is what keeps the whole thing from becoming a maintenance nightmare.

## Real-world error transformations

The clearest way to see why this matters is to look at what two different processors return for the exact same failure — a card decline.

Stripe sends:

```json
{
  "error": {
    "code": "card_declined",
    "message": "Your card was declined.",
    "decline_code": "insufficient_funds"
  }
}
```

Adyen sends:

```json
{
  "errorCode": "DECLINED",
  "message": "Payment declined",
  "errorType": "REFUSED"
}
```

Different field names, different nesting, different vocabulary. Both get transformed by their respective adapters into the same `ErrorResponse` inside Prism — same `code`, same `attempt_status`, same structure. And the raw response from each connector is preserved in `connector_details`, so nothing is lost if you need to dig into it later.

From the outside, a developer using the SDK sees exactly one error shape regardless of which of these processors fired. The chaos happens inside the adapters, where it belongs. By the time it reaches application code, it's just a predictable object with fields you already know how to handle.

## What it looks like from the outside

From an SDK user's perspective, the surface is small. You call `authorize`, you either get a result back or you get an error. The error tells you what happened, why it happened, and what the network is actually advising. The raw connector response is there if you need to dig deeper.

You don't need to know whether the error came from a 5xx at the transport layer or a decline code from the issuing bank. You don't need to know that Stripe and Adyen encode the same failure differently. You don't need to update your error handling when a new connector gets added underneath.

That's the goal — not making errors disappear, but making them predictable.

Payment failures are inevitable. The question is just whether they're chaos or signal. We decided to make them signal.

---

*The full implementation — connector adapters, error mappings, and SDK clients — lives at [juspay/hyperswitch-prism](https://github.com/juspay/hyperswitch-prism).*
