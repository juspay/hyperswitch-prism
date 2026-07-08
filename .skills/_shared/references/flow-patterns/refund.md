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

```rust
pub enum RefundStatus {
    Pending,    // Refund initiated, processing
    Success,    // Refund completed
    Failure,    // Refund failed
}
```

### Example Mappings

```rust
// Generic pattern -- adapt status strings per connector
let refund_status = match item.response.status.as_str() {
    "pending" | "processing" | "initiated" => RefundStatus::Pending,
    "completed" | "success" | "succeeded" => RefundStatus::Success,
    "failed" | "declined" | "refused" => RefundStatus::Failure,
    _ => RefundStatus::Pending, // Default to pending for unknown statuses
};
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
        let refund_status = map_connector_refund_status(&item.response.status);
        let connector_refund_id = extract_refund_id(&item.response);

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        });
        Ok(router_data)
    }
}
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
fn is_partial_refund(request: &RefundsData) -> bool {
    request.minor_refund_amount < request.payment_amount
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

```rust
// Partial refunds not supported
if is_partial_refund(&router_data.request) {
    return Err(IntegrationError::NotSupported {
        message: "Partial refunds are not supported by this connector".to_string(),
        connector: "{ConnectorName, context: Default::default() }".to_string(),
    }.into());
}

// Payment method not supported for refunds
match &router_data.request.payment_method {
    PaymentMethod::BankTransfer(_) => {
        return Err(IntegrationError::NotSupported {
            message: "Refunds for bank transfers are not supported by this connector".to_string(),
            connector: "{ConnectorName, context: Default::default() }".to_string(),
        }.into());
    }
    _ => {}
}

// Currency restriction
const UNSUPPORTED_CURRENCIES: &[&str] = &["BTC", "ETH"];
if UNSUPPORTED_CURRENCIES.contains(&router_data.request.currency.to_string().as_str()) {
    return Err(IntegrationError::NotSupported {
        message: format!(
            "Refunds for {, context: Default::default() } currency are not supported by this connector",
            router_data.request.currency
        ),
        connector: "{ConnectorName}".to_string(),
    }.into());
}
```

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
