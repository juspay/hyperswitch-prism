# RSync Flow Pattern Reference

Refund Sync (RSync) queries a connector for the current status of a refund.
It receives a connector_refund_id, sends a status request, and maps the
connector's response to a standardized `RefundStatus`.

> For macro syntax details, see `macro-reference.md`.

---

## Key Characteristics

- **Most connectors use GET** (8/12 production implementations). GET-based RSync has
  no request body -- omit `curl_request` from the macro entirely.
- POST-based RSync is used when the API requires auth in the body, complex query
  parameters, or does not offer a RESTful GET endpoint.
- The connector_refund_id is obtained via `req.request.connector_refund_id` and is
  typically embedded in the URL.
- RSync uses **RefundFlowData** (not PaymentFlowData) and **RefundSyncData** /
  **RefundsResponseData** (not PaymentsSyncData / PaymentsResponseData).

---

## Macro Implementation

### GET-Based (Most Common)

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // NOTE: No curl_request line for GET -- no request body is sent
    curl_response: {ConnectorName}RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // GET requests typically omit Content-Type
            let mut header = vec![];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}/refunds/{refund_id}"))
        }
    }
);
```

### POST-Based

Same macro shape but add `curl_request: Json({ConnectorName}RefundSyncRequest)`,
set `http_method: Post`, include `Content-Type` in `get_headers`, and point `get_url`
at the fixed inquiry endpoint. See the PSync POST-based example in `psync.md` for the
full template -- the only differences are the flow types listed above.

---

## Prerequisites Macro Entry

Add the RSync flow to `create_all_prerequisites!`:

```rust
(
    flow: RSync,
    request_body: {ConnectorName}RefundSyncRequest,
    response_body: {ConnectorName}RefundSyncResponse,
    router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
),
```

Implement the trait marker:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for {ConnectorName}<T>
{
}
```

---

## URL Construction Patterns

All patterns start by extracting the refund ID:

```rust
let refund_id = req.request.connector_refund_id.clone();
let base_url = self.connector_base_url_refunds(req);
```

| Pattern | Example | Connectors |
|---|---|---|
| RESTful path | `{base_url}/refunds/{id}` | Razorpay, Xendit |
| Hierarchical | `{base_url}/orders/{order_id}/transactions/{id}` | Nexinets |
| Reference lookup | `{base_url}/order/getbyreference/{id}` | Noon |
| Fixed endpoint (POST) | `{base_url}/api/gateway` | Authorizedotnet, Elavon |
| Payment actions | `{base_url}/payments/{payment_id}/actions` | Checkout |

For hierarchical URLs that require metadata (e.g., order_id):

```rust
let order_id = req.connector_meta_data
    .get_required_value("order_id")
    .change_context(errors::IntegrationError::MissingConnectorMetaData)?;
Ok(format!("{base_url}/orders/{order_id}/transactions/{refund_id}"))
```

---

## Transformer Structures

### Request -- GET (empty unit struct)

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}RefundSyncRequest;

impl TryFrom<
    {ConnectorName}RouterData<
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    >,
> for {ConnectorName}RefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}
```

### Request -- POST (with body fields)

For POST-based RSync, add fields to the struct and extract them from
`item.router_data.request.connector_refund_id` in `try_from`. Same TryFrom
signature as the GET variant above, but populates struct fields instead of
returning a unit struct.

### Response

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundSyncResponse {
    pub id: String,
    pub status: {ConnectorName}RefundStatus,
    // Add connector-specific fields as needed
}
```

---

## Status Mapping

Map the connector's status enum to `common_enums::RefundStatus` via the `From` trait.

**Best practices:**
- Derive status from the connector's response field, never from HTTP status code.
- Default unknown statuses to `Pending` and log a warning.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]  // adjust per connector API
pub enum {ConnectorName}RefundStatus {
    Succeeded,
    Failed,
    Pending,
    Cancelled,
}

impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        match status {
            {ConnectorName}RefundStatus::Succeeded => Self::Success,
            {ConnectorName}RefundStatus::Pending   => Self::Pending,
            {ConnectorName}RefundStatus::Failed
            | {ConnectorName}RefundStatus::Cancelled => Self::Failure,
        }
    }
}
```

Common RefundStatus targets:

| RefundStatus | When to use |
|---|---|
| `Success` | Refund completed / settled / processed |
| `Pending` | Processing / submitted / in-progress / unknown |
| `Failure` | Declined / cancelled / error |

---

## Response TryFrom Implementation

```rust
impl TryFrom<
    ResponseRouterData<
        {ConnectorName}RefundSyncResponse,
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    >,
> for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ResponseRouterData<
            {ConnectorName}RefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status: common_enums::RefundStatus::from(response.status.clone()),
            }),
            ..router_data.clone()
        })
    }
}
```

---

## Imports Checklist

Add to both the connector file and transformers file:

```rust
use domain_types::connector_flow::RSync;
use domain_types::connector_types::{
    RefundFlowData, RefundsResponseData, RefundSyncData,
};
```

---

## SourceVerification Stub

Required for every flow:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for {ConnectorName}<T>
{
}
```
