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

`RouterDataV2` has no `connector_meta_data` field. RSync metadata rides on
`req.request.refund_connector_metadata: Option<SecretSerdeValue>`, and the error variant is
`NoConnectorMetaData { context }` (there is no `MissingConnectorMetaData`).

```rust
let meta: {ConnectorName}RefundMeta = utils::to_connector_meta(
    req.request.refund_connector_metadata.clone().map(|m| m.expose()),
)?;
let order_id = meta.order_id;
Ok(format!("{base_url}/orders/{order_id}/transactions/{refund_id}"))
```

If you need to raise the error by hand:

```rust
return Err(IntegrationError::NoConnectorMetaData {
    context: IntegrationErrorContext::default(),
}
.into());
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
    /// Present only on in-band failures; drives the `Err(ErrorResponse)` branch below.
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    // Add connector-specific fields as needed
}
```

---

## Status Mapping

Map the connector's status enum to `common_enums::RefundStatus` via the `From` trait.

**Best practices:**
- Derive status from the connector's response field, never from HTTP status code.
- Absorb unknown wire values at the **deserialization** layer with `#[serde(other)]`, and keep
  the mapping `match` **exhaustive**. A catch-all `_ =>` at the status-mapping layer hides
  new vendor statuses from the compiler; reviewers require both halves.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]  // adjust per connector API
pub enum {ConnectorName}RefundStatus {
    Succeeded,
    Failed,
    Pending,
    Cancelled,
    /// Deserialization-layer catch-all -- an unrecognised status string lands here instead of
    /// failing the whole response parse.
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        // Exhaustive on purpose -- no `_ =>` arm.
        match status {
            {ConnectorName}RefundStatus::Succeeded => Self::Success,
            {ConnectorName}RefundStatus::Pending   => Self::Pending,
            {ConnectorName}RefundStatus::Failed
            | {ConnectorName}RefundStatus::Cancelled => Self::Failure,
            // Non-terminal: unknown is not proof of failure.
            {ConnectorName}RefundStatus::Unknown   => Self::Pending,
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
    // Response-side transformers fail with `ConnectorError`, not `IntegrationError`.
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            {ConnectorName}RefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let refund_status = common_enums::RefundStatus::from(response.status.clone());

        // In-band failure: a 2xx body reporting a failed refund is still a failure and must
        // come back as `Err(ErrorResponse { .. })`, not an `Ok` carrying a Failure status.
        // Branch on the status the connector reported, not on the HTTP code.
        if matches!(refund_status, common_enums::RefundStatus::Failure) {
            return Ok(Self {
                response: Err(ErrorResponse {
                    // Never `.unwrap_or_default()` -- an empty code reaches the merchant blank.
                    code: response
                        .error_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response
                        .error_message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.error_message.clone(),
                    status_code: item.http_code,
                    // Refund flow -> `FlowStatus::Refund(..)`, carrying the status this
                    // response proves. Never a hardcoded terminal Failure on a shared path,
                    // and never a blanket `None` (a hard-declined refund left Pending keeps
                    // retrying). Exemplars: `connectors/flywire.rs`, `connectors/noon.rs`.
                    attempt_status: Some(FlowStatus::Refund(refund_status)),
                    connector_transaction_id: Some(
                        router_data.request.connector_transaction_id.clone(),
                    ),
                    // `ErrorResponse` has 13 fields and implements `Default`.
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        Ok(Self {
            // `RefundsResponseData` has FOUR fields -- omitting any is E0063.
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
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
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::connector_flow::RSync;
use domain_types::connector_types::{
    RefundFlowData, RefundsResponseData, RefundSyncData,
};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::router_data::{ErrorResponse, FlowStatus};
```

---

## SourceVerification / BodyDecoding Stubs

Both traits are **non-generic**: `SourceVerification`
(`crates/types-traits/interfaces/src/verification.rs:20`) and `BodyDecoding`
(`interfaces/src/decode.rs`) take no flow type parameters. There is **one** impl per
connector, never one per flow. Writing
`SourceVerification<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>` is E0107
("trait takes 0 generic arguments but 4 were supplied").

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for {ConnectorName}<T>
{
}
```

Every method on both traits has a default body, so an empty impl is the whole stub. Override
`get_secrets` / `get_algorithm` / `get_signature` / `get_message` only when the connector
actually signs or encodes its callbacks. Exemplar:
`crates/integrations/connector-integration/src/connectors/travelhub.rs:175`.
