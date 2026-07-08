# Capture Flow Pattern Reference

Reference for implementing the capture flow in a UCS connector.
For macro syntax details, see `macro-reference.md`.

## Overview

The capture flow handles post-authorization fund capture. Key components:
- **Connector file**: PaymentCapture trait + macro implementation
- **Transformers file**: Request/response structs and TryFrom implementations
- **URL construction**: Endpoint with transaction ID from authorization
- **Status mapping**: Connector statuses to `AttemptStatus`

## Critical Rules

### Status Mapping
- **NEVER hardcode `status: AttemptStatus::Charged`**
- Always map status from the connector response
- Document WHY each status mapping is chosen

```rust
// WRONG
status: AttemptStatus::Charged, // Hardcoded!

// CORRECT
let status = common_enums::AttemptStatus::from(response.status.clone());
```

### Validation
- Only add validations required by the connector API spec
- Always include a comment explaining the purpose
- Do not re-validate fields already checked upstream (e.g., positive amounts, 3-char currency)

### Field Usage
- Remove fields that would be hardcoded to `None`
- Keep request/response structs minimal -- only fields the connector uses

## Connector File Pattern

```rust
// crates/integrations/connector-integration/src/connectors/{connector_name}.rs

// 1. Implement PaymentCapture trait (empty)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for {ConnectorName}<T>
{
}

// 2. Add Capture to create_all_prerequisites macro (see macro-reference.md)
//    Entry in the api array:
//        (
//            flow: Capture,
//            request_body: {ConnectorName}CaptureRequest,
//            response_body: {ConnectorName}CaptureResponse,
//            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
//        ),

// 3. Implement Capture flow via macro_connector_implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}CaptureRequest),
    curl_response: {ConnectorName}CaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = match &req.request.connector_transaction_id {
                Some(id) => id,
                None => return Err(errors::IntegrationError::MissingConnectorTransactionID.into()),
            };
            let base_url = self.connector_base_url_payments(req);
            // Adjust URL pattern to match connector API
            Ok(format!("{base_url}/payments/{transaction_id}/capture"))
        }
    }
);

// 4. SourceVerification stub
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for {ConnectorName}<T>
{
}
```

## URL Endpoint Patterns

| Pattern | Example | When to use |
|---------|---------|-------------|
| REST with txn ID in path | `/payments/{id}/capture` | Most connectors (Adyen, Razorpay) |
| Same endpoint as payments | `{base_url}` (action in body) | Authorizedotnet-style |
| Capture-specific path | `/capture/{transaction_id}` | Some enterprise APIs |

### Partial vs Full Capture -- Dual Endpoint

Some APIs use separate endpoints for full vs partial captures:

```rust
fn get_url(&self, req: &RouterDataV2<Capture, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);

    let is_full_capture = req.request.amount_to_capture.is_none()
        || req.request.amount_to_capture == Some(req.request.payment_amount);

    if is_full_capture {
        Ok(format!("{base_url}/payments/{transaction_id}/settlements"))
    } else {
        Ok(format!("{base_url}/payments/{transaction_id}/partialSettlements"))
    }
}
```

## Transformers File Pattern

### Capture Request Struct

```rust
// Only include fields the connector actually uses
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")] // Adjust per connector API
pub struct {ConnectorName}CaptureRequest {
    pub amount: {AmountType},       // StringMinorUnit, MinorUnit, etc.
    pub currency: String,
    // Include transaction_id in body only if connector requires it there
    // (otherwise it goes in the URL path)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}
```

### Capture Response Struct

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}CaptureResponse {
    pub id: String,
    pub status: {ConnectorName}CaptureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}
```

### Capture Status Enum and Mapping

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")] // Adjust per connector
pub enum {ConnectorName}CaptureStatus {
    Succeeded,
    Captured,
    Completed,
    Failed,
    Error,
    Pending,
    Processing,
    Cancelled,
}

impl From<{ConnectorName}CaptureStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}CaptureStatus) -> Self {
        match status {
            {ConnectorName}CaptureStatus::Succeeded
            | {ConnectorName}CaptureStatus::Captured
            | {ConnectorName}CaptureStatus::Completed => Self::Charged,

            {ConnectorName}CaptureStatus::Failed
            | {ConnectorName}CaptureStatus::Error => Self::Failure,

            {ConnectorName}CaptureStatus::Pending
            | {ConnectorName}CaptureStatus::Processing => Self::Pending,

            {ConnectorName}CaptureStatus::Cancelled => Self::Voided,
        }
    }
}
```

**Status mapping table:**

| Connector Status | AttemptStatus | Reasoning |
|-----------------|---------------|-----------|
| `captured`, `settled`, `completed`, `success` | `Charged` | Capture completed |
| `pending`, `processing`, `submitted` | `Pending` | Capture in progress |
| `failed`, `declined`, `rejected` | `Failure` | Capture failed |
| `cancelled`, `voided` | `Voided` | Capture cancelled |
| `partially_captured` | `PartialCharged` | Partial capture completed |

### Minimal Response Handling

When a connector returns only an ID and timestamp (no status field):

```rust
let status = if let Some(status_field) = &response.status {
    common_enums::AttemptStatus::from(status_field.clone())
} else if response.error.is_some() {
    common_enums::AttemptStatus::Failure
} else if item.http_code >= 200 && item.http_code < 300 {
    // Success HTTP code with valid ID -> Charged (synchronous capture)
    common_enums::AttemptStatus::Charged
} else if item.http_code >= 400 {
    common_enums::AttemptStatus::Failure
} else {
    common_enums::AttemptStatus::Pending
};
```

## TryFrom Implementations

### Request: TryFrom RouterData -> CaptureRequest

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>>
    for {ConnectorName}CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Purpose: API requires original transaction reference for capture
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingConnectorTransactionID)?;

        Ok(Self {
            amount: item.amount, // Pre-converted by amount_converter
            currency: router_data.request.currency.to_string(),
            transaction_id: Some(transaction_id.clone()),
            reference: Some(
                router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
        })
    }
}
```

### Response: TryFrom ResponseRouterData -> RouterDataV2

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}CaptureResponse, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}CaptureResponse, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        let status = common_enums::AttemptStatus::from(response.status.clone());

        // Handle error responses
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

        // Success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: response.reference.clone(),
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

## Partial vs Full Capture

Determine capture type from request data:

```rust
fn is_full_capture(request: &PaymentsCaptureData) -> bool {
    request.amount_to_capture.is_none()
        || request.amount_to_capture == Some(request.payment_amount)
}
```

For dual-endpoint APIs, use an enum request:

```rust
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CaptureRequest {
    Empty {},                        // Full capture (empty body)
    Complex(ComplexCaptureData),     // Partial capture
}

impl TryFrom<CaptureRouterData> for CaptureRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        if is_full_capture(&item.router_data.request) {
            Ok(CaptureRequest::Empty {})
        } else {
            Ok(CaptureRequest::Complex(ComplexCaptureData {
                amount: item.amount.get_amount_as_i64(),
                currency: item.router_data.request.currency.to_string(),
                reference: item.router_data.resource_common_data
                    .connector_request_reference_id.clone(),
            }))
        }
    }
}
```

## Real Connector Examples

### Adyen-style (Simple REST)
- Request: `{ merchant_account, amount, reference }`
- URL: `{base_url}/v68/payments/{transaction_id}/captures`
- Response has explicit status field

### Authorizedotnet-style (Transaction Wrapper)
- Request: wrapped in `create_transaction_request` with `merchant_authentication`
- Transaction type: `PriorAuthCaptureTransaction`
- URL: same base endpoint; action determined by body
- Response: reuses `AuthorizedotnetPaymentsResponse`

### Fiserv-style (Reference Transaction)
- Request: `{ amount, transaction_details, reference_transaction_details }`
- URL: `{base_url}/v1/payments/{transaction_id}/capture`
- Response: nested in `gateway_response`
