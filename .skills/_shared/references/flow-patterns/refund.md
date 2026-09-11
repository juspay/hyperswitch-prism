# Refund Flow Pattern Reference

Refund flows process refund requests for previously successful payments. They are typically
simpler than payment flows but have distinct characteristics: different response schemas,
unique status semantics, and separate URL patterns.

Key components: **Refund** (process refund), **RSync** (check refund status).

## Request Patterns

Three common patterns for refund request bodies. See `macro-reference.md` for macro syntax.

### Pattern 1: Empty Body (Worldpay, PayPal)

Full refunds with no request body -- the connector resolves amount internally.

```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {}

impl TryFrom<...> for {ConnectorName}RefundRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
```

### Pattern 2: Amount-Required (Adyen, Stripe, Square)

Always send amount and currency, even for full refunds.

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundRequest {
    pub merchant_account: Secret<String>,
    pub amount: Amount,
    pub merchant_refund_reason: Option<String>,
    pub reference: String,
}

impl TryFrom<...> for {ConnectorName}RefundRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let auth_type = AuthType::try_from(&item.router_data.connector_config)?;
        let router_data = &item.router_data;

        Ok(Self {
            merchant_account: auth_type.merchant_account,
            amount: Amount {
                currency: router_data.request.currency.to_string(),
                value: router_data.request.minor_refund_amount,
            },
            merchant_refund_reason: router_data.request.reason.clone(),
            reference: router_data.request.refund_id.clone(),
        })
    }
}
```

### Pattern 3: Metadata-Rich (Checkout.com, Authorize.Net)

Supports extensive metadata, idempotency keys, and explicit refund type.

```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {
    pub amount: Option<MinorUnit>,
    pub currency: Option<String>,
    pub reason: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub idempotency_key: Option<String>,
}
```

## Response Patterns

### Simple Response (ID + status + links)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundResponse {
    pub outcome: String,
    #[serde(rename = "_links")]
    pub links: {ConnectorName}Links,
}
```

### Detailed Response (full confirmation)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}RefundResponse {
    pub id: String,
    pub status: {ConnectorName}RefundStatus,
    pub amount: MinorUnit,
    pub currency: String,
    pub created: Option<i64>,
    pub reason: Option<String>,
    /// Present only on in-band failures; drives the `Err(ErrorResponse)` branch below.
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
```

### Critical Rule: Verify Actual API Responses

Refund responses often differ from payment responses. Do not assume field parity.
A field present in payment responses (e.g. `transaction_reference`) may be absent
from refund responses.

## URL Construction

### Refund URL Patterns

| Style | URL | Notes |
|---|---|---|
| RESTful subresource | `{base}/payments/{payment_id}/refunds` | Most common |
| API-versioned | `{base}/{version}/payments/{payment_id}/refunds` | Adyen style |
| Dedicated endpoint | `{base}/refunds` | Payment ID in body (Stripe) |
| Transaction-based | `{base}/transactions/{txn_id}/refund` | Alternative |

### RSync URL Patterns

| Style | URL | Notes |
|---|---|---|
| Direct refund ID | `{base}/refunds/{refund_id}` | Worldpay, Stripe |
| Payment + refund | `{base}/payments/{payment_id}/refunds/{refund_id}` | Adyen |
| Actions-based | `{base}/payments/{payment_id}/actions` | Checkout (returns all actions) |
| Empty impl | N/A | Rely on webhooks; no RSync support |

## Async vs Sync Refund Processing

**Critical principle: A `200 OK` response often means "refund accepted", NOT "refund completed".**

Many connectors process refunds asynchronously. The initial response acknowledges receipt;
actual completion is confirmed later via RSync or webhook.

### When to Use `RefundStatus::Pending`

The connector returns a minimal response (just an ID or status like "accepted") and
processes the refund in the background. Requires RSync to verify completion.

Typical status strings: `"sentForRefund"`, `"pending"`, `"processing"`, `"accepted"`,
`"initiated"`, `"[refund-received]"`.

### When to Use `RefundStatus::Success`

The connector returns detailed confirmation with a clear "completed"/"succeeded" status,
full refund details, and processes synchronously.

Typical status strings: `"succeeded"`, `"completed"`, `"refunded"`.

### Decision Flow

```
Response received (200 OK)
  |
  +-> Minimal data (only ID/status)?
  |   -> RefundStatus::Pending, implement RSync
  |
  +-> Detailed confirmation?
      -> Check status field:
         "succeeded"/"completed"/"refunded"       -> Success
         "pending"/"processing"/"initiated"       -> Pending
         "failed"/"declined"/"refused"            -> Failure
```

### Real-World Examples

- **Worldpay**: Returns `"sentForRefund"` -> map to `Pending`. RSync later returns `"refunded"`.
- **Adyen**: Returns `"[refund-received]"` -> map to `Pending`. Completion via webhook/RSync.
- **Stripe**: May return `"succeeded"` (sync) or `"pending"` (async) -- handle both.

## Status Mapping

### Standard Statuses

The target is `common_enums::RefundStatus` -- do not declare a local enum with that name.
Its variants include `Pending`, `Success`, `Failure`, `TransactionFailure`, `ManualReview`
and `Unknown`; `Pending` is the `Default`.

### Example Mappings

Deserialize into a connector-specific enum with a `#[serde(other)]` catch-all, then map it
**exhaustively**. Matching on a raw `&str` forces a catch-all `_ =>` at the status-mapping
layer, which is exactly the pattern reviewers reject: it hides new vendor statuses from the
compiler. Both halves are required.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]   // adjust per connector API
pub enum {ConnectorName}RefundStatus {
    Pending,
    Processing,
    Initiated,
    Completed,
    Succeeded,
    Failed,
    Declined,
    Refused,
    /// Deserialization-layer catch-all: an unrecognised status string lands here instead of
    /// failing the response parse.
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        // Exhaustive -- no `_ =>` arm.
        match status {
            {ConnectorName}RefundStatus::Pending
            | {ConnectorName}RefundStatus::Processing
            | {ConnectorName}RefundStatus::Initiated => Self::Pending,

            {ConnectorName}RefundStatus::Completed
            | {ConnectorName}RefundStatus::Succeeded => Self::Success,

            {ConnectorName}RefundStatus::Failed
            | {ConnectorName}RefundStatus::Declined
            | {ConnectorName}RefundStatus::Refused => Self::Failure,

            // Non-terminal: an unrecognised status is not proof the refund failed.
            {ConnectorName}RefundStatus::Unknown => Self::Pending,
        }
    }
}
```

### Response TryFrom Implementation

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}RefundResponse, RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<...>,
    ) -> Result<Self, Self::Error> {
        let refund_status =
            common_enums::RefundStatus::from(item.response.status.clone());

        // In-band failure: a 2xx body carrying a declined/failed refund status is still a
        // failure and must come back as `Err(ErrorResponse { .. })`, not an `Ok` with a
        // Failure status. Branch on the status the connector reported.
        if matches!(refund_status, common_enums::RefundStatus::Failure) {
            return Ok(Self {
                response: Err(ErrorResponse {
                    // Never `.unwrap_or_default()`: an empty code reaches the merchant blank.
                    code: item
                        .response
                        .error_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: item
                        .response
                        .error_message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.error_message.clone(),
                    status_code: item.http_code,
                    // Refund flow -> `FlowStatus::Refund(..)`, carrying the status this
                    // response actually proves. Never `FlowStatus::Payment(..)` here, never a
                    // hardcoded terminal Failure on a shared path, and never a blanket `None`
                    // (a hard-declined refund left Pending keeps retrying forever).
                    // Exemplars: `connectors/flywire.rs`, `connectors/travelhub.rs`.
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    connector_transaction_id: Some(
                        item.router_data.request.connector_transaction_id.clone(),
                    ),
                    // `ErrorResponse` has 13 fields and implements `Default`.
                    ..Default::default()
                }),
                ..item.router_data
            });
        }

        let connector_refund_id = extract_refund_id(&item.response);

        let mut router_data = item.router_data;
        // `RefundsResponseData` has FOUR fields; it is a plain struct, so every one must be
        // listed unless you spread an existing value.
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        });
        Ok(router_data)
    }
}
```

Imports for the transformers file:

```rust
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::{
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    router_data::{ErrorResponse, FlowStatus},
};
```

### Extracting Refund ID

Refund IDs may come from different fields than payment IDs. Check multiple sources:

```rust
fn extract_refund_id(response: &{ConnectorName}RefundResponse) -> String {
    response.id.clone()
        .or_else(|| extract_id_from_href(&response.links.self_link.href))
        .or_else(|| response.reference.clone())
        .unwrap_or_else(|| "unknown".to_string())
}
```

## Partial Refund Handling

Not all connectors support partial refunds. When unsupported, reject early in `TryFrom`:

```rust
// `RefundsData` carries both amounts twice, in two unit types. Compare like with like:
// `minor_refund_amount: MinorUnit` against `minor_payment_amount: MinorUnit`
// (or `refund_amount: i64` against `payment_amount: i64`). Mixing them is a type error.
fn is_partial_refund(request: &RefundsData) -> bool {
    request.minor_refund_amount < request.minor_payment_amount
}
```

For connectors that support partial refunds, pass `minor_refund_amount` directly in the
request body. The connector tracks cumulative refund totals internally.

## Error Handling

### Common Refund Error Cases

| Error | Cause |
|---|---|
| `already_refunded` / `charge_already_refunded` | Payment already fully refunded |
| `insufficient_funds` / `refund_amount_exceeds_charge` | Refund exceeds remaining refundable amount |
| `invalid_charge` / `charge_not_found` | Original payment not found |
| Deserialization failure (`missing field`) | Refund response schema differs from payment response |
| `400 Bad Request` with empty body | Connector requires fields even for full refunds |
| `404 Not Found` | Wrong URL pattern for refund endpoint |

### NotSupported Error Pattern

Use `IntegrationError::NotSupported` to reject unsupported refund scenarios early,
before making API calls. Always be specific about what is not supported.

The variant has **three** fields (`domain_types::errors`), and `connector` is a
`&'static str` -- not a `String`:

```rust
NotSupported {
    message: String,
    connector: &'static str,
    context: IntegrationErrorContext,
}
```

```rust
// Partial refunds not supported
if is_partial_refund(&router_data.request) {
    return Err(IntegrationError::NotSupported {
        message: "Partial refunds".to_string(),
        connector: "{connector_name}",
        context: IntegrationErrorContext::default(),
    }
    .into());
}

// Payment method not supported for refunds
match &router_data.request.payment_method_data {
    Some(PaymentMethodData::BankTransfer(_)) => {
        return Err(IntegrationError::NotSupported {
            message: "Refunds for bank transfers".to_string(),
            connector: "{connector_name}",
            context: IntegrationErrorContext::default(),
        }
        .into());
    }
    _ => {}
}

// Currency restriction -- prefer the purpose-built variant, which has the same three-field
// shape.
// Use real `common_enums::Currency` variants for whatever the vendor spec excludes.
const UNSUPPORTED_CURRENCIES: &[Currency] = &[Currency::JPY, Currency::KRW];
if UNSUPPORTED_CURRENCIES.contains(&router_data.request.currency) {
    return Err(IntegrationError::CurrencyNotSupported {
        message: format!("Refunds in {}", router_data.request.currency),
        connector: "{connector_name}",
        context: IntegrationErrorContext::default(),
    }
    .into());
}
```

`IntegrationError::NotImplemented` is a **tuple** variant, `NotImplemented(String,
IntegrationErrorContext)` -- not a struct variant. `ConnectorError` has only five variants
(`ResponseDeserializationFailed`, `ResponseHandlingFailed`, `UnexpectedResponseError`,
`IntegrityCheckFailed`, `ConnectorErrorResponse`); everything request-side is
`IntegrationError`.

NotSupported best practices:
- Check in `TryFrom`, before building the request
- Use format: `"{Feature} not supported by this connector"`
- Include relevant context (amounts, currency, payment method) in the message
- Document why the limitation exists with comments

## Checklist

- [ ] Request structure verified (empty vs with data)
- [ ] Response structure matches actual API (not assumed from payment response)
- [ ] URL pattern correct for both Refund and RSync
- [ ] All connector status strings mapped to `RefundStatus`
- [ ] Async processing handled (Pending + RSync, not premature Success)
- [ ] Partial refund support validated or rejected with NotSupported
- [ ] Refund ID extraction handles connector-specific source fields
- [ ] Error scenarios tested (already refunded, amount exceeded, not found)
- [ ] In-band 2xx failure returns `Err(ErrorResponse { .. })` with
      `attempt_status: Some(FlowStatus::Refund(..))` -- never `Ok` with a Failure status
- [ ] Status enum has `#[serde(other)] Unknown` AND an exhaustive mapping `match`
- [ ] `RefundsResponseData` literal lists all four fields, including
      `acquirer_reference_number`
