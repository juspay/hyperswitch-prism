# IncomingWebhook Flow Pattern Reference

Webhooks are the asynchronous half of every connector: the PSP calls UCS when a payment,
refund or dispute changes state. Unlike Authorize/PSync/Capture/Refund/RSync/Void, **webhooks
are not a `ConnectorIntegrationV2` flow**. There is no `connector_flow` marker, no
`RouterDataV2`, no `create_all_prerequisites!` entry, and no `macro_connector_implementation!`
invocation. It is a plain trait impl:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for {ConnectorName}<T>
{
    // ... method overrides ...
}
```

The trait is `IncomingWebhook` in `crates/types-traits/interfaces/src/connector_types.rs`.
It is a **required supertrait of `ConnectorServiceTrait`**, so every connector already has an
impl block; a connector without webhooks carries the empty one and inherits the defaults.

> For macro syntax details, see `macro-reference.md`. For the exhaustive long-form version of
> this pattern (per-connector variations, HMAC recipes, more worked examples), read
> `grace/rulesbook/codegen/guides/patterns/pattern_IncomingWebhook_flow.md`.

---

## The Two-Step Contract: ParseEvent → HandleEvent

This is the single most important thing to get right. The gRPC surface is
`service EventService` (`crates/types-traits/grpc-api-types/proto/services.proto`). Two of its
three RPCs carry the webhook contract (the third, `NotifyConnector`, is unrelated), and the
trait methods split cleanly between them:

| Phase | RPC (HTTP route) | Trait methods it calls | Has secrets? |
|---|---|---|---|
| **1. Parse** | `EventService.ParseEvent` (`POST /events/parse`) | `get_event_type`, then `get_webhook_event_reference` | **No** |
| **2. Handle** | `EventService.HandleEvent` (`POST /events/handle`) | `verify_webhook_source` (or the external-verification path), `get_webhook_integrity_checks`, `get_event_type` again, then one of `process_payment_webhook` / `process_refund_webhook` / `process_dispute_webhook`, then `get_webhook_api_response` | **Yes** |

Why it is split: the caller receives a raw HTTP body with no idea which merchant it belongs to.
ParseEvent is **stateless** — it extracts the event type and the resource IDs from the payload
alone, so the caller can look up the right merchant's webhook secret (or early-exit on an event
it does not care about). Only then does it call HandleEvent with those secrets.

`CompositeEventService.HandleEvent` (`POST /composite/events/handle`) is the single-call
shortcut for callers that already have the secrets resolved; the proto comment says so
outright. It does not change what the connector must implement.

The dispatchers are worth reading once — they are short:
- `parse_webhook_event` in `crates/integrations/connector-integration/src/webhook_utils.rs`
- `process_webhook_event` in the same file (the payment/refund/dispute fan-out)
- `EventService::parse_event` / `EventService::handle_event` in
  `crates/grpc-server/grpc-server/src/server/events.rs`

### Two consequences that bite

1. **`get_event_type` is called in BOTH phases and must not depend on secrets or context.**
   It takes only `RequestDetails`.
2. **`get_webhook_event_reference` returning `Ok(None)` is legal and means "no actionable
   reference".** The default impl on the trait returns exactly that, so a connector that
   forgets to override it silently parses every webhook to a null reference. 19 connectors
   override it today; 35 override `get_event_type`. That gap is the most common webhook bug
   in this repo.

---

## Method Roster and Defaults

Every method on `IncomingWebhook` has a default, so **omitting one compiles**. Know what each
default does before you rely on it:

| Method | Default behaviour | Overridden by |
|---|---|---|
| `verify_webhook_source` | `Ok(false)` — "not verified", not an error | 32 connectors |
| `get_webhook_integrity_checks` | `vec![]` | 3 |
| `get_webhook_source_verification_signature` | `Ok(Vec::new())` | 19 |
| `get_webhook_source_verification_message` | `Ok(Vec::new())` | 19 |
| `get_event_type` | `Err(WebhooksNotImplemented { operation: "get_event_type" })` | 35 |
| `get_webhook_event_reference` | **`Ok(None)`** | 19 |
| `process_payment_webhook` | `Err(WebhooksNotImplemented { .. })` | 33 |
| `process_refund_webhook` | `Err(WebhooksNotImplemented { .. })` | 23 |
| `process_dispute_webhook` | `Err(WebhooksNotImplemented { .. })` | 9 |
| `get_webhook_resource_object` | `Err(WebhooksNotImplemented { .. })` | 21 |
| `sample_webhook_body` | `b"{}"` | 30 |
| `get_webhook_api_response` | `EventAckResponse { status_code: 200, headers: vec![], body: None }` | 3 |

`WebhooksNotImplemented { operation }` takes a `&'static str`, so the "no webhooks" stub is
simply an empty impl block — do not hand-write the error returns.

---

## `get_webhook_event_reference` — the ParseEvent payload

Return type: `Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>>`.

```rust
// domain_types/src/connector_types.rs, `pub enum WebhookResourceReference`
pub enum WebhookResourceReference {
    Payment(PaymentWebhookReference),
    Refund(RefundWebhookReference),
    Dispute(DisputeWebhookReference),
    Mandate(MandateWebhookReference),
    Payout(PayoutWebhookReference),
}

pub struct PaymentWebhookReference {
    pub connector_transaction_id: Option<String>,   // PSP-assigned
    pub merchant_transaction_id: Option<String>,    // caller-assigned, echoed back
}

pub struct RefundWebhookReference {
    pub connector_refund_id: Option<String>,
    pub merchant_refund_id: Option<String>,
    pub connector_transaction_id: Option<String>,   // the PARENT payment
    pub merchant_transaction_id: Option<String>,
}

pub struct DisputeWebhookReference {
    pub connector_dispute_id: Option<String>,
    pub connector_transaction_id: Option<String>,   // the PARENT payment
}

pub struct MandateWebhookReference {
    pub connector_mandate_id: Option<String>,
    pub merchant_transaction_id: Option<String>,
}

pub struct PayoutWebhookReference {
    pub connector_payout_id: Option<String>,
    pub merchant_payout_id: Option<String>,
}
```

Every field is `Option<String>`. Fill in what the payload actually carries and leave the rest
`None` — do **not** duplicate one ID into several fields to "be safe". The caller matches on
which field is populated.

### `connector_*_id` vs `merchant_*_id`

- `connector_*_id` — the PSP's own identifier for the resource.
- `merchant_*_id` — the identifier **you** sent (order ID / invoice ID / reference) and the
  connector echoed back.

Getting these backwards makes the caller's lookup miss. When a payload has both an ID for the
event and an ID for the parent (Adyen's `psp_reference` vs `original_reference`), the **parent**
is the payment lookup key:

```rust
// connectors/adyen.rs, get_webhook_event_reference
WebhookEventCode::Capture
| WebhookEventCode::CaptureFailed
| WebhookEventCode::Cancellation
| WebhookEventCode::AuthorisationAdjustment => {
    WebhookResourceReference::Payment(PaymentWebhookReference {
        connector_transaction_id: notif.original_reference,        // parent auth
        merchant_transaction_id: Some(notif.merchant_reference),
    })
}
WebhookEventCode::Refund | WebhookEventCode::CancelOrRefund | ... => {
    WebhookResourceReference::Refund(RefundWebhookReference {
        connector_refund_id: Some(notif.psp_reference),            // this refund
        merchant_refund_id: Some(notif.merchant_reference),
        connector_transaction_id: notif.original_reference,        // parent payment
        merchant_transaction_id: None,
    })
}
WebhookEventCode::Unknown => return Ok(None),
```

### Canonical simple shape

```rust
fn get_webhook_event_reference(
    &self,
    request: RequestDetails,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
    let webhook_body: {ConnectorName}IncomingWebhookData = request
        .body
        .parse_struct("{ConnectorName}IncomingWebhookData")
        .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

    let webhook_resource_reference = match webhook_body.data {
        {ConnectorName}WebhookData::Payment(response_data) => {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(response_data.id),
                merchant_transaction_id: response_data.external_reference,
            })
        }
        {ConnectorName}WebhookData::Refund(response_data) => {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(response_data.id),
                merchant_refund_id: response_data.external_reference,
                connector_transaction_id: None,
                merchant_transaction_id: None,
            })
        }
    };

    Ok(Some(webhook_resource_reference))
}
```

(Shape from `connectors/givepayments.rs`. Reference implementations to copy from:
`adyen.rs` — richest event-code mapping; `givepayments.rs` — clean two-variant payload;
`fiuu.rs` — content-type-dependent body decoding.)

---

## `get_event_type` and the payment/refund/dispute fan-out

`process_webhook_event` picks which `process_*_webhook` to call from the `EventType` your
`get_event_type` returned, using `EventType::is_payment_event()` / `is_refund_event()` /
`is_dispute_event()` (`domain_types/src/connector_types.rs`). **Anything that matches none of
the three — mandate, payout, recovery, misc — falls through to
`get_payments_webhook_content`, not to an error.** So a refund webhook that you map to a
payment `EventType` will be routed to `process_payment_webhook` and produce a wrong-shaped
response, not a failure. Map the event type correctly and the fan-out takes care of itself.

`EventType` has no catch-all: use `EventType::IncomingWebhookEventUnspecified` for events the
connector sends that UCS does not model. `connectors/adyen.rs` also returns it as an early
guard when `request.body.is_empty()`, before attempting to decode. The legacy broad `Payment` /
`Refund` / `Dispute` variants exist for backward compatibility — prefer a specific variant.

---

## `process_*_webhook` — the HandleEvent payload

These build struct literals with **no `..Default::default()`** available, so every field must
be listed or you get E0063:

- `WebhookDetailsResponse` — **17** fields
- `RefundWebhookDetailsResponse` — **9** fields
- `DisputeWebhookDetailsResponse` — **11** fields

Read the real field list from `domain_types/src/connector_types.rs` before writing the literal.
The 17-field payment one, verbatim from `connectors/givepayments.rs`:

```rust
Ok(WebhookDetailsResponse {
    connector_returned_payment_method_details: None,
    resource_id,                       // Option<ResponseId>
    status,                            // AttemptStatus — never hardcode
    connector_response_reference_id: None,
    connector_request_reference_id: None,
    mandate_reference,                 // Option<Box<MandateReference>> — Box is not optional
    error_code: None,
    error_message: None,
    error_reason: None,
    raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
    status_code: 200,
    response_headers: None,
    amount_captured: None,
    minor_amount_captured: None,
    network_txn_id: None,
    payment_method_update: None,
    sender_payment_instrument_id: None,
})
```

The same status-mapping rules as PSync apply: derive `AttemptStatus` / `RefundStatus` from the
wire status via a `From` impl, with `#[serde(other)] Unknown` on the wire enum and an
exhaustive, catch-all-free match in the mapping. Never hardcode a status.

`EventContext` (`process_payment_webhook`'s fourth argument) currently carries only
`capture_method`. When a connector genuinely needs it, the missing-field error is the
purpose-built `WebhookError::WebhookMissingRequiredContext { field, origin }`, whose message
tells the caller to pass that field from the original request.

---

## Source verification

`verify_webhook_source` returns `Result<bool, _>` — **the bool is data, not a gate**.
`handle_event` treats an `Err` as `false` (it logs a warning and continues) and threads the
result into the response as `source_verified`. Processing happens either way. Do not assume a
failed verification stops the webhook.

The signature/message split exists so HMAC verification is uniform:

```rust
fn verify_webhook_source(
    &self,
    request: RequestDetails,
    connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorSpecificConfig>,
) -> Result<bool, error_stack::Report<errors::WebhookError>> {
    let algorithm = crypto::HmacSha256;

    let connector_webhook_secrets = connector_webhook_secret
        .ok_or_else(|| error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound))?;

    let signature =
        self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
    let message =
        self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

    algorithm
        .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
        .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
}
```

`ConnectorWebhookSecrets` has two fields: `secret: Vec<u8>` and
`additional_secret: Option<Secret<String>>`.

A connector listed in `config.webhook_source_verification_call.connectors_with_webhook_source_verification_call`
takes a different path entirely — `requires_external_webhook_verification` (a default method on
`ValidationTrait` in `interfaces/src/connector_types.rs`) makes the server call the connector's
verification API instead of the local `verify_webhook_source`.

Use the purpose-built `WebhookError` variants rather than a generic one:
`WebhookSignatureNotFound`, `WebhookVerificationSecretNotFound`,
`WebhookVerificationSecretInvalid`, `WebhookSourceVerificationFailed`,
`WebhookBodyDecodingFailed`, `WebhookResourceObjectNotFound`, `WebhookEventTypeNotFound`,
`WebhookReferenceIdNotFound`, `WebhookProcessingFailed`,
`WebhookAmountConversionFailed { reason }`, `WebhookResponseEncodingFailed`,
`WebhookMissingRequiredField { field }`, `WebhookMissingRequiredContext { field, origin }`.

---

## `RequestDetails` — what you get to parse from

```rust
pub struct RequestDetails {
    pub method: HttpMethod,
    pub uri: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub query_params: Option<String>,
}
```

Five fields, no more. Header keys are looked up as they arrive — connectors in this repo match
lowercase (`request.headers.get("content-type")`, `"gp-webhook-signature"`).

Body decoding is per-connector: `request.body.parse_struct("Type")` for JSON, or branch on the
content type when the PSP sends form-encoded bodies:

```rust
// connectors/fiuu.rs
let header = request.headers.get("content-type")
    .ok_or_else(|| report!(WebhookError::WebhookBodyDecodingFailed))?;

let payload: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
    serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
        .change_context(WebhookError::WebhookResourceObjectNotFound)?
} else {
    request.body.parse_struct("fiuu::FiuuWebhooksResponse")
        .change_context(WebhookError::WebhookResourceObjectNotFound)?
};
```

---

## `sample_webhook_body` and certification

```rust
fn sample_webhook_body(&self) -> &'static [u8] { b"{}" }
```

A minimal, structurally valid webhook body for this connector. The field-probe calls it via
`get_webhook_sample_body` (`crates/ffi/ffi/src/services/payments.rs`) to verify webhook handling
is actually implemented, so the connector owns its probe payload next to its webhook code. The
default `b"{}"` will not deserialise into any real payload type — override it with a realistic
body (see `connectors/givepayments.rs` for a full one) whenever you implement webhooks.

Separately, `check_connector_specs.rs` Phase 2B is deterministic and merge-relevant: if a
connector's `crates/internal/integration-tests/src/connector_specs/<name>/specs.json` lists
`"EventService/HandleEvent"` in `supported_suites`, it **must** also have
`connector_specs/<name>/webhook_payload.json`, or the check fails. Declaring the suite and
skipping the fixture is a hard error; not declaring the suite imposes no requirement.

---

## Implementation Checklist

- [ ] `impl connector_types::IncomingWebhook for {ConnectorName}<T>` exists (it is required by
      `ConnectorServiceTrait` — an empty block is the correct "no webhooks" stub)
- [ ] `get_event_type` overridden and stateless (no secrets, no context)
- [ ] `get_webhook_event_reference` overridden — the default `Ok(None)` is a silent failure
- [ ] `connector_*_id` vs `merchant_*_id` assigned in the right slots; parent payment ID used
      as the payment lookup key for capture/refund/dispute events
- [ ] Unrecognised events return `Ok(None)` from the reference method and
      `EventType::IncomingWebhookEventUnspecified` from `get_event_type`
- [ ] `process_payment_webhook` / `process_refund_webhook` / `process_dispute_webhook`
      implemented for the event classes the connector actually sends
- [ ] Every `*DetailsResponse` literal lists all fields (17 / 9 / 11)
- [ ] Statuses mapped through a `From` impl, exhaustive, no `_ =>` in the mapping and
      `#[serde(other)] Unknown` on the wire enum
- [ ] `verify_webhook_source` implemented if the PSP signs its webhooks; remember the bool is
      reported, not enforced
- [ ] `sample_webhook_body` overridden with a realistic payload
- [ ] If `specs.json` declares `EventService/HandleEvent`, `webhook_payload.json` exists
