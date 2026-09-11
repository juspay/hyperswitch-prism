# PSync Flow Pattern Reference

Payment Sync (PSync) queries a connector for the current status of a transaction.
It receives a connector_transaction_id, sends a status request, and maps the
connector's response to a standardized `AttemptStatus`.

> For macro syntax details, see `macro-reference.md`.

---

## Key Characteristics

- **Most connectors use GET** (12/20 production implementations). GET-based PSync has
  no request body -- omit `curl_request` from the macro entirely.
- POST-based PSync (8/20 connectors) is used when the API requires auth in the body,
  complex query parameters, or does not offer a RESTful GET endpoint.
- The connector_transaction_id is obtained via
  `req.request.get_connector_transaction_id()` and is typically embedded in the URL.

---

## Macro Implementation

### GET-Based (Most Common)

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // NOTE: No curl_request line for GET -- no request body is sent
    curl_response: {ConnectorName}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // GET requests typically omit Content-Type
            let mut header = vec![];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // `get_connector_transaction_id()` already returns
            // `CustomResult<String, IntegrationError>` and raises
            // `MissingConnectorTransactionID { context }` itself -- do not re-wrap it.
            let transaction_id = req.request.get_connector_transaction_id()?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/payments/{transaction_id}"))
        }
    }
);
```

### POST-Based

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}SyncRequest),   // POST sends a JSON body
    curl_response: {ConnectorName}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/v1/transaction-inquiry"))
        }
    }
);
```

---

## Prerequisites Macro Entry

Add the PSync flow to `create_all_prerequisites!`:

```rust
(
    flow: PSync,
    request_body: {ConnectorName}SyncRequest,
    response_body: {ConnectorName}SyncResponse,
    router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
),
```

Implement the trait marker:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for {ConnectorName}<T>
{
}
```

---

## URL Construction Patterns

All patterns start by extracting the transaction ID:

```rust
let transaction_id = req.request.get_connector_transaction_id()?;
let base_url = self.connector_base_url_payments(req);
```

| Pattern | Example | Connectors |
|---|---|---|
| RESTful path | `{base_url}/payments/{id}` | Checkout, Volt, Xendit |
| Status endpoint | `{base_url}/api/v1/order/{id}/status` | Bluecode |
| Hierarchical | `{base_url}/orders/{order_id}/transactions/{id}` | Nexinets |
| Query parameter | `{base_url}/status?payment_id={id}` | (less common) |
| Complex identifiers | `{base_url}/status/{merchant_id}/{id}` | PhonePe, Mifinity |
| Fixed endpoint (POST) | `{base_url}/v3/order/status` | Authorizedotnet, Fiserv |

---

## Transformer Structures

### Request -- GET (empty unit struct)

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}SyncRequest;

impl TryFrom<
    {ConnectorName}RouterData<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    >,
> for {ConnectorName}SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}
```

### Request -- POST (with body fields)

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SyncRequest {
    pub transaction_id: String,
    // Add connector-specific fields: merchant_authentication, query_type, etc.
}

impl TryFrom<
    {ConnectorName}RouterData<
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    >,
> for {ConnectorName}SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let transaction_id = router_data
            .request
            .get_connector_transaction_id()?;

        Ok(Self {
            transaction_id,
        })
    }
}
```

### Response

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SyncResponse {
    pub id: String,
    pub status: {ConnectorName}PaymentStatus,
    /// Merchant-side reference the connector echoes back; feeds
    /// `connector_response_reference_id`.
    pub reference: Option<String>,
    /// Present only on in-band failures; drives the `Err(ErrorResponse)` branch below.
    pub error: Option<String>,
    pub error_code: Option<String>,
    // Add connector-specific fields as needed
}
```

---

## Status Mapping

Map the connector's status enum to `common_enums::AttemptStatus` via the `From` trait.

**Best practices:**
- Always derive status from the connector's response field, never from HTTP status code.
- Use the `From` trait for clean, testable mapping.
- Handle unknown wire values at the **deserialization** layer with `#[serde(other)]`, and keep
  the mapping `match` **exhaustive** -- a catch-all `_ =>` at the status-mapping layer hides
  new vendor statuses from the compiler. Reviewers require both halves.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]  // adjust per connector API
pub enum {ConnectorName}PaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Authorized,
    Cancelled,
    /// Deserialization-layer catch-all: an unrecognised status string lands here instead of
    /// failing the whole response parse.
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        // Exhaustive on purpose -- no `_ =>` arm.
        match status {
            {ConnectorName}PaymentStatus::Succeeded => Self::Charged,
            {ConnectorName}PaymentStatus::Authorized => Self::Authorized,
            {ConnectorName}PaymentStatus::Pending    => Self::Pending,
            {ConnectorName}PaymentStatus::Failed     => Self::Failure,
            {ConnectorName}PaymentStatus::Cancelled  => Self::Voided,
            // Non-terminal: a status we do not recognise is not proof of failure.
            {ConnectorName}PaymentStatus::Unknown    => Self::Pending,
        }
    }
}
```

Common AttemptStatus targets:

| AttemptStatus | When to use |
|---|---|
| `Charged` | Payment captured / settled / succeeded |
| `Authorized` | Authorized but not yet captured |
| `Pending` | Processing / in-progress / unknown |
| `Failure` | Declined / error / failed |
| `Voided` | Cancelled / voided |
| `AuthenticationPending` | 3DS challenge / requires_action |
| `PartialCharged` | Partial capture or partial settlement |

---

## Response TryFrom Implementation

```rust
impl TryFrom<
    ResponseRouterData<
        {ConnectorName}SyncResponse,
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    >,
> for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    // Response-side transformers fail with `ConnectorError`, not `IntegrationError`.
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            {ConnectorName}SyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        // Carry identifiers through instead of hardcoding `None`. PSync must echo back the same
        // reference Authorize published, otherwise the identifier round-trip breaks and the
        // caller cannot correlate the sync with the original attempt.
        // Prefer the reference the connector echoes in this response; fall back to the
        // connector transaction id. Never `None` when the connector gave you either.
        let connector_response_reference_id = response
            .reference
            .clone()
            .or_else(|| Some(response.id.clone()));

        // Re-publish whatever metadata the connector still needs on later flows. Read it from
        // the inbound carrier (`router_data.request.connector_feature_data`) or rebuild it from
        // this response -- do not drop it.
        let connector_metadata = router_data
            .request
            .connector_feature_data
            .clone()
            .map(|meta| meta.expose());

        // Enum struct-variant: no `..Default::default()`, all 11 fields are mandatory (E0063).
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            connector_metadata,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
```

**Identifier carrier chain:** `TransactionResponse.connector_metadata` (written by Authorize)
-> `PaymentsSyncData.connector_feature_data` on the PSync request (accessor
`PaymentsSyncData::get_connector_meta()`) -> gRPC `connector_feature_data`. Emitting
`connector_metadata: None` / `connector_response_reference_id: None` from PSync severs that
chain -- that is the identifier round-trip bug, not a harmless stub.

---

## Error Handling in PSync Response

A 2xx body carrying a declined/failed status is still a failure and must come back as
`response: Err(ErrorResponse { .. })`. Branch on a success predicate derived from the
connector's own status -- `domain_types::utils::is_payment_failure` -- not on the HTTP code:

```rust
let status = common_enums::AttemptStatus::from(response.status.clone());

if domain_types::utils::is_payment_failure(status) {
    return Ok(Self {
        resource_common_data: PaymentFlowData {
            status,
            ..router_data.resource_common_data.clone()
        },
        response: Err(ErrorResponse {
            // Never `.unwrap_or_default()` -- an empty code reaches the merchant blank.
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .error
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error.clone(),
            status_code: item.http_code,
            // Flow-aware, derived from the connector response. Do NOT hardcode
            // `Some(AttemptStatus::Failure)`: it is a type error now (the field is
            // `Option<FlowStatus>`) and it is the bug that reports a charged payment as
            // FAILURE. Do not blanket-`None` it either -- a hard decline that stays Pending
            // keeps retrying forever. Set what the response actually proves.
            attempt_status: Some(FlowStatus::Payment(status)),
            connector_transaction_id: Some(response.id.clone()),
            // `ErrorResponse` has 13 fields and implements `Default`.
            ..Default::default()
        }),
        ..router_data.clone()
    });
}
```

Exemplars in tree: `connectors/flywire.rs` (full flow-aware classification, including
`FlowStatus::Refund`) and `connectors/noon.rs` (minimal form -- terminal only for one proven
error code, `None` otherwise).

---

## Imports Checklist

Ensure these are present in the connector file:

```rust
use domain_types::connector_flow::PSync;
use domain_types::connector_types::{
    PaymentFlowData, PaymentsResponseData, PaymentsSyncData, ResponseId,
};
```

And in the transformers file:

```rust
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::connector_flow::PSync;
use domain_types::connector_types::{
    PaymentFlowData, PaymentsResponseData, PaymentsSyncData, ResponseId,
};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::router_data::{ErrorResponse, FlowStatus};
use hyperswitch_masking::ExposeInterface;   // for `SecretSerdeValue::expose()`
```

---

## SourceVerification / BodyDecoding Stubs

`SourceVerification` (`crates/types-traits/interfaces/src/verification.rs`) and
`BodyDecoding` (`interfaces/src/decode.rs`) are **non-generic** traits -- they take no flow
type parameters. There is **one** impl per connector, not one per flow. Writing
`SourceVerification<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>` is
E0107 ("trait takes 0 generic arguments but 4 were supplied").

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

Both traits are fully defaulted, so an empty impl is the whole stub. Override
`get_secrets` / `get_algorithm` / `get_signature` / `get_message` only when the connector
actually signs or encodes its callbacks. Exemplar:
`crates/integrations/connector-integration/src/connectors/travelhub.rs:175`.
