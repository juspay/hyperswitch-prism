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
            let transaction_id = req.request.get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID)?;
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
let transaction_id = req.request.get_connector_transaction_id()
    .change_context(errors::IntegrationError::MissingConnectorTransactionID)?;
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
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID)?;

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
    // Add connector-specific fields as needed
}
```

---

## Status Mapping

Map the connector's status enum to `common_enums::AttemptStatus` via the `From` trait.

**Best practices:**
- Always derive status from the connector's response field, never from HTTP status code.
- Use the `From` trait for clean, testable mapping.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]  // adjust per connector API
pub enum {ConnectorName}PaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Authorized,
    Cancelled,
}

impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Succeeded => Self::Charged,
            {ConnectorName}PaymentStatus::Authorized => Self::Authorized,
            {ConnectorName}PaymentStatus::Pending    => Self::Pending,
            {ConnectorName}PaymentStatus::Failed     => Self::Failure,
            {ConnectorName}PaymentStatus::Cancelled  => Self::Voided,
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
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ResponseRouterData<
            {ConnectorName}SyncResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
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

---

## Error Handling in PSync Response

When the connector response contains error information, return an `Err` variant:

```rust
if let Some(error) = &response.error {
    return Ok(Self {
        resource_common_data: PaymentFlowData {
            status: common_enums::AttemptStatus::Failure,
            ..router_data.resource_common_data.clone()
        },
        response: Err(ErrorResponse {
            code: response.error_code.clone().unwrap_or_default(),
            message: error.clone(),
            reason: Some(error.clone()),
            status_code: item.http_code,
            attempt_status: Some(common_enums::AttemptStatus::Failure),
            connector_transaction_id: Some(response.id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }),
        ..router_data.clone()
    });
}
```

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
use domain_types::connector_flow::PSync;
use domain_types::connector_types::{
    PaymentFlowData, PaymentsResponseData, PaymentsSyncData, ResponseId,
};
```

---

## SourceVerification Stub

Required for every flow:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for {ConnectorName}<T>
{
}
```
