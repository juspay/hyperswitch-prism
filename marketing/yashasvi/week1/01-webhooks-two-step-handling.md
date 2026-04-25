# Why we split webhook handling into two RPCs — and why it solves more than payments

> If you've ever written a webhook receiver, this one is for you.
> *Yashasvi · Hyperswitch Prism · Week 1 / Post 1*

---

## The webhook problem nobody admits is hard

On paper, a webhook is "just" an HTTP POST you ack with `200 OK`.

In production, it's:

- A signed payload from a vendor whose signing scheme is different from every other vendor's.
- A reference to *some* resource — payment, refund, dispute, mandate, payout — that you have to figure out before you know which secret to verify it with.
- An event type that has to be mapped to *your* internal state machine, not the vendor's.
- An outbound HTTP call to the vendor in some cases (yes, PayPal, looking at you — `POST /v1/notifications/verify-webhook-signature` with OAuth2, just to check a signature).
- A single connector that often doesn't tell you whether the event is `AUTHORIZED` or `CAPTURED` (Noon, Fiuu — you need the original capture intent to disambiguate).
- And on top of all that — your gateway needs to decide *who* to route this to, *which* tenant, *which* secret, *which* idempotency key, before you even verify it.

Most webhook libraries collapse all of this into one function: `verify_and_parse(request, secret) -> Event`. That collapse is the bug. It forces you to know the secret *before* you know what the webhook is about.

Prism splits it into two phases, and exposes both — granular (`EventService`) and composite (`CompositeEventService`) — so you can pick the shape that matches your architecture.

---

## The two phases

```proto
service EventService {
  // Phase 1: Parse a raw webhook payload. No credentials required.
  // Returns resource reference and event type — sufficient to resolve
  // secrets or early-exit.
  rpc ParseEvent(EventServiceParseRequest) returns (EventServiceParseResponse);

  // Phase 2: Verify webhook source and return a unified typed response.
  // Response mirrors PaymentService.Get / RefundService.Get / DisputeService.Get.
  rpc HandleEvent(EventServiceHandleRequest) returns (EventServiceHandleResponse);
}
```

That's it. Two RPCs. Read them slowly:

**`ParseEvent`** takes only the raw HTTP request — headers, body, method, URI, query params. **No secret. No credentials. No DB.** It returns an `EventReference` (a oneof of `payment | refund | dispute | mandate | payout` IDs — both connector-side and merchant-side) plus a `WebhookEventType`.

**`HandleEvent`** takes the request *plus* the secret(s), an optional access token, and an optional `EventContext`. It does source verification, returns the verified, unified typed event content, and an `EventAckResponse` you should send back to the connector.

```proto
message EventReference {
  oneof resource {
    PaymentEventReference payment = 1;  // connector_transaction_id, merchant_transaction_id
    RefundEventReference  refund  = 2;  // connector_refund_id, merchant_refund_id, parent payment id
    DisputeEventReference dispute = 3;  // connector_dispute_id, parent payment id
    MandateEventReference mandate = 4;
    PayoutEventReference  payout  = 5;
  }
}
```

## Why splitting matters

Once you have `ParseEvent` separately, a whole class of architectures that used to be painful become trivial.

**1. Resolve the right secret before verifying.**
Multi-tenant gateways don't have *one* webhook secret per connector — they have one per `(tenant, connector_account)`. With one-shot APIs you either pre-load every secret in memory (bad) or do a lookup *and* a verify in the same critical path (worse). With `ParseEvent`, you extract the reference, look up the tenant from your DB / cache, then call `HandleEvent` with exactly the right secret. Cleaner code. Fewer bugs. Lower memory footprint.

**2. Idempotency and dedup before you do crypto.**
Verifying signatures is the most expensive thing you do per webhook (HMAC, timing-safe compare, sometimes outbound HTTP for PayPal-style verifications). `ParseEvent` is dirt cheap — just a payload parse. Use it to compute a dedup key from `(connector_event_id, reference_id)` and short-circuit replays *before* you touch crypto. At any non-trivial volume this is the difference between "200ms p99" and "30ms p99".

**3. Early-exit on irrelevant events.**
Connectors fire events you don't care about — `account.updated`, `capability.changed`, endpoint verification probes. `ParseEvent` returns `WebhookEventType` and an *optional* reference (absent for non-resource events). One cheap call, you ack and move on. No wasted verification.

**4. Routing.**
Got a multi-region setup? `ParseEvent` is enough to route the webhook to the region that owns the resource. The expensive `HandleEvent` runs once, in the right place.

## When you don't need any of that — the composite shape

If you're a single-tenant integrator, none of the above matters. You already know the secret. You just want a `webhook_in -> typed_event_out` function.

That's `CompositeEventService.HandleEvent`. One RPC. Internally orchestrates `ParseEvent` then `HandleEvent`. From the implementation:

```rust
// crates/internal/composite-service/src/events.rs
async fn handle_event(...) -> ... {
    // Phase 1: ParseEvent — extract reference and event type from the raw payload.
    let parse_resp = self.event_service.parse_event(parse_req).await?;

    // Phase 2: HandleEvent — source verification + unified event content.
    let handle_resp = self.event_service.handle_event(handle_req).await?;

    Ok(CompositeEventHandleResponse {
        reference: parse_resp.reference,
        event_type: handle_resp.event_type,
        event_content: handle_resp.event_content,
        source_verified: handle_resp.source_verified,
        merchant_event_id: handle_resp.merchant_event_id,
        event_ack_response: handle_resp.event_ack_response,
    })
}
```

Same building blocks. Different ergonomic. **Granular for orchestrators, composite for integrators.** You pick.

---

## The detail that makes it actually unified

The killer feature isn't the split. It's what `HandleEvent` returns:

```proto
message EventContent {
  oneof content {
    PaymentServiceGetResponse payments_response = 1;  // same shape as PaymentService.Get
    RefundResponse            refunds_response  = 2;  // same shape as RefundService.Get
    DisputeResponse           disputes_response = 3;  // same shape as DisputeService.Get
  }
}
```

A webhook from Stripe and a poll from Stripe collapse into the **same response type**. A webhook from Adyen and a poll from Adyen — same. A webhook from Stripe and a webhook from Adyen — same.

That means your downstream code — your state machine, your audit log, your reconciliation pipeline — has *one* code path. Not "polled-payment-handler" and "webhook-payment-handler" and "stripe-webhook-handler" and "adyen-webhook-handler". One handler, one type, done.

This is the part that makes Prism's webhook handling not just *a* webhook library but a **webhook unification layer**.

## Stateless, but not naive — the EventContext escape hatch

Here's a sharp edge most webhook libraries hit: some connectors send you events whose *meaning depends on something you sent earlier*. The classic offender: an event from Noon or Fiuu that says `transaction.success` — but doesn't tell you whether the original intent was authorize-only or authorize-and-capture. The same payload means `AUTHORIZED` for one merchant and `CAPTURED` for another.

Stateful gateways solve this by pulling the original capture intent from their DB. Prism is **stateless** — it can't. So Prism inverts the contract:

```proto
message EventServiceHandleRequest {
  optional string         merchant_event_id = 1;
  RequestDetails          request_details   = 2;
  optional WebhookSecrets webhook_secrets   = 3;
  optional AccessToken    access_token      = 4;  // for PayPal-style outbound verification
  optional EventContext   event_context     = 5;  // your business context, passed back in
}

message PaymentEventContext {
  optional CaptureMethod capture_method = 1;  // pass back what you sent in Authorize
}
```

You pass back the bits Prism needs. If a connector requires `event_context.payment.capture_method` and you don't supply it, you get `INVALID_ARGUMENT` with an actionable message — *which* field, *why*, *for which connector*. No silent wrong status.

The reason this matters for marketing: most "stateless" libraries are stateless until they aren't, and then they ask you to install Redis. Prism's statelessness is honest.

## And one more — the ack response is part of the contract

Connectors care what you reply with. Some want `200 OK` with an empty body. Some want a specific JSON shape. Some want `204`. Get it wrong and they retry, often for days, and your "webhook" turns into "denial of service from your own vendor".

```proto
message EventServiceHandleResponse {
  WebhookEventType event_type             = 1;
  EventContent     event_content          = 2;
  bool             source_verified        = 3;
  optional string  merchant_event_id      = 4;
  optional EventAckResponse event_ack_response = 5;  // <— what to send back
}

message EventAckResponse {
  uint32 status_code           = 1;
  map<string, string> headers  = 2;
  bytes body                   = 3;
}
```

Prism tells you the *exact* status, headers and body to return for that connector. You don't memorize 50 vendor quirks. You return what Prism told you to return.

---

## Why this isn't just for payments

The two-phase shape — `(parse without secret) → (verify with secret)` — is generic. Anything that emits signed events with multiple resource types over HTTP fits the same pattern: GitHub, Slack, Twilio, Shopify, AWS SNS, observability vendors, calendar webhooks. Today Prism's `WebhookEventType` enum is payments-shaped (PaymentIntentSuccess, RefundFailure, DisputeOpened, MandateActive, etc. — 30+ of them). Tomorrow it doesn't have to be.

If you've ever found yourself writing a "webhook router" service that does verification + event normalization + dedup + downstream fan-out, you've already built half of `EventService` by hand. The other half is the part where every vendor has a slightly different signing scheme, and you're maintaining 40 forks of the same `verify()` function. That's the part Prism takes off your plate.

---

## TL;DR

- **Two RPCs**: `ParseEvent` (no secret) and `HandleEvent` (with secret). Plus `CompositeEventService` for one-shot.
- **Reference before verify** lets you do tenant resolution, dedup, routing, early-exit cheaply.
- **Webhook output = poll output** — same proto types. One handler downstream.
- **EventContext** is the honest stateless escape hatch.
- **EventAckResponse** tells you exactly what to reply with.

It's a webhook library that takes the shape of webhooks seriously, instead of pretending they're "just HTTP POST + signature".

Code: [github.com/juspay/hyperswitch-prism](https://github.com/juspay/hyperswitch-prism) · `proto/services.proto` and `proto/payment.proto` are where this design lives.
