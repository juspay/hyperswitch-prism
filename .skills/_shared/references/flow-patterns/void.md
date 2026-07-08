# Void Flow Pattern Reference

Reference for implementing the void flow in a UCS connector.
For macro syntax details, see `macro-reference.md`.

## Overview

The void flow cancels an authorized payment before capture/settlement. Key components:
- **Connector file**: PaymentVoidV2 trait + macro implementation
- **Transformers file**: Request/response structs and TryFrom implementations
- **URL construction**: Endpoint referencing the original transaction ID
- **Status mapping**: Connector statuses to `AttemptStatus` (Voided/VoidFailed)

Uses **PaymentFlowData** and **PaymentVoidData** (not PaymentsAuthorizeData).

### Void vs Refund

| Aspect | Void | Refund |
|--------|------|--------|
| **Timing** | Before capture/settlement | After capture/settlement |
| **Purpose** | Cancel authorization | Return money |
| **Request complexity** | Simple (reference + optional reason) | Complex (amount, currency, etc.) |
| **Status flow** | Authorized -> Voided | Charged -> Refunded |

```
Authorize -> [VOID POSSIBLE] -> Capture -> [REFUND POSSIBLE] -> Settle
```

## Critical Rules

- **NEVER hardcode `status: AttemptStatus::Voided`** -- always map from the response
- Only include fields the connector actually requires
- Common request fields: `connector_transaction_id`, `cancellation_reason`

## Connector File Pattern

```rust
// 1. Implement PaymentVoidV2 trait (empty)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for {ConnectorName}<T>
{
}

// 2. Add Void to create_all_prerequisites macro (see macro-reference.md)
//    Entry in the api array:
//        (
//            flow: Void,
//            request_body: {ConnectorName}VoidRequest,
//            response_body: {ConnectorName}VoidResponse,
//            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
//        ),

// 3. Implement Void flow via macro_connector_implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}VoidRequest),
    curl_response: {ConnectorName}VoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{base_url}/payments/{payment_id}/voids"))
        }
    }
);
```

## URL Endpoint Patterns

| Pattern | Example | When to use |
|---------|---------|-------------|
| REST with txn ID in path | `/payments/{id}/voids` | Most connectors (Checkout, Stripe) |
| Direct void endpoint | `/v1/cancels` (ID in body) | Fiserv-style |
| Action-based path | `/payments/{id}/actions/void` | Some gateways |
| Transaction cancel | `/transaction/cancel` | Novalnet-style |

## Transformers: Request Patterns

### Available PaymentVoidData Fields

From `router_data.request`:
- `connector_transaction_id: String` -- original transaction reference (always available)
- `cancellation_reason: Option<String>` -- reason for void

From `router_data.resource_common_data`:
- `connector_request_reference_id: String`
- `connector_meta_data: Option<SecretSerdeValue>` -- metadata from authorize

### Pattern 1: Simple Reference Void (Checkout, Stripe-style)

```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}VoidRequest {
    pub reference: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>>
    for {ConnectorName}VoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference: item.router_data.request.connector_transaction_id.clone(),
        })
    }
}
```

### Pattern 2: Detailed Transaction Void (Fiserv-style)

Includes auth extraction, merchant details, and reversal reason:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidRequest {
    pub transaction_details: TransactionDetails,
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

// In TryFrom: extract auth, cancellation_reason, connector_request_reference_id,
// and connector_transaction_id into the nested structs above.
```

### Pattern 3: Session-Aware Void (requires metadata from authorize)

Extract session data stored in `connector_meta_data` during authorize:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidRequest {
    pub transaction_id: String,
    pub reason: Option<String>,
    pub merchant_config: MerchantConfig,
}

// Helper to parse session from connector_meta_data
fn extract_session_from_metadata(
    meta_data: Option<&pii::SecretSerdeValue>,
) -> Result<SessionData, IntegrationError> {
    let value = meta_data
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "connector_meta_data for session in Void",
        , context: Default::default() })?
        .peek();
    // Parse JSON string -> SessionData
}
```

## Transformers: Response Patterns

### Status Mapping Table

| Connector Status | AttemptStatus | Reasoning |
|-----------------|---------------|-----------|
| `voided`, `cancelled`, `canceled`, `completed` | `Voided` | Void completed |
| `pending`, `processing`, `initiated` | `Pending` | Void in progress |
| `failed`, `declined`, `rejected`, `error` | `VoidFailed` | Void failed |

### Pattern 1: HTTP Status-Based (Checkout-style)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct {ConnectorName}VoidResponse {
    #[serde(skip)]
    pub(super) status: u16,   // Set from http_code, not API body
    pub action_id: String,
    pub reference: String,
}

impl From<&{ConnectorName}VoidResponse> for enums::AttemptStatus {
    fn from(item: &{ConnectorName}VoidResponse) -> Self {
        if item.status == 202 { Self::Voided } else { Self::VoidFailed }
    }
}
```

### Pattern 2: Response Field-Based (Fiserv-style)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}PaymentStatus { Voided, Failed, Processing }

impl From<{ConnectorName}PaymentStatus> for enums::AttemptStatus {
    fn from(item: {ConnectorName}PaymentStatus) -> Self {
        match item {
            {ConnectorName}PaymentStatus::Voided => Self::Voided,
            {ConnectorName}PaymentStatus::Failed => Self::VoidFailed,
            {ConnectorName}PaymentStatus::Processing => Self::Pending,
        }
    }
}
```

### Pattern 3: String Status-Based

```rust
match item.status.as_str() {
    "completed" | "successful" | "voided" => Self::Voided,
    "failed" | "declined" | "rejected" => Self::VoidFailed,
    "pending" | "processing" => Self::Pending,
    _ => Self::VoidFailed, // Conservative default
}
```

### Response TryFrom Implementation

```rust
impl<F> TryFrom<ResponseRouterData<{ConnectorName}VoidResponse, RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}VoidResponse, RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData { mut response, router_data, http_code } = item;
        let mut router_data = router_data;

        response.status = http_code; // If using HTTP status-based pattern
        let status = enums::AttemptStatus::from(&response);
        router_data.resource_common_data.status = status;

        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.action_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: response.reference.clone(),
            incremental_authorization_allowed: None,
            status_code: http_code,
        });

        Ok(router_data)
    }
}
```

## Real Connector Examples

### Checkout.com-style (Simple REST)
- Request: `{ reference }` (just payment reference)
- URL: `{base_url}/payments/{payment_id}/voids`
- Response: HTTP 202 = Voided, else VoidFailed

### Fiserv-style (Reference Transaction)
- Request: `{ transaction_details, merchant_details, reference_transaction_details }`
- URL: `{base_url}/ch/payments/v1/cancels`
- Response: nested `gateway_response` with explicit status field

### Novalnet-style (Transaction Cancel)
- Request: `{ transaction_id, reason }` with session data from metadata
- URL: `{base_url}/transaction/cancel`
- Response: string-based status field
