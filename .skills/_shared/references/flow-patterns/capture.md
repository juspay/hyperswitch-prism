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
            // `PaymentsCaptureData::connector_transaction_id` is a `ResponseId`, not an
            // `Option<String>`. Use the accessor -- it already returns
            // `CustomResult<String, IntegrationError>` and raises
            // `MissingConnectorTransactionID { context }` on the wrong variant.
            let transaction_id = req.request.get_connector_transaction_id()?;
            let base_url = self.connector_base_url_payments(req);
            // Adjust URL pattern to match connector API
            Ok(format!("{base_url}/payments/{transaction_id}/capture"))
        }
    }
);

// 4. SourceVerification / BodyDecoding stubs -- BOTH TRAITS ARE NON-GENERIC.
//    They take no flow type parameters, so there is ONE impl per connector, not one per
//    flow. `SourceVerification<Capture, PaymentFlowData, ...>` is E0107.
//    Definitions: interfaces/src/verification.rs and interfaces/src/decode.rs.
//    Exemplar: connectors/travelhub.rs:175.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
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

    // Partial-capture detection is connector-specific. `PaymentsCaptureData` carries the
    // amount being captured now (`amount_to_capture: i64`, NON-Option, and
    // `minor_amount_to_capture: MinorUnit`) but NOT the originally authorized amount --
    // there is no `payment_amount` field on it. Compare `amount_to_capture` against the
    // authorized amount the connector itself reported (echoed in the authorize response,
    // or stashed in `connector_metadata` at authorize time and read back here from
    // `req.request.connector_feature_data`). Do not invent a field.
    let is_full_capture = is_full_capture_for_{connector_name}(req)?;

    if is_full_capture {
        Ok(format!("{base_url}/payments/{transaction_id}/settlements"))
    } else {
        Ok(format!("{base_url}/payments/{transaction_id}/partialSettlements"))
    }
}
```

### `PaymentsCaptureData` -- the real fields

`crates/types-traits/domain_types/src/connector_types.rs`:

```rust
pub struct PaymentsCaptureData {
    pub amount_to_capture: i64,              // NOT an Option
    pub minor_amount_to_capture: MinorUnit,
    pub currency: Currency,
    pub connector_transaction_id: ResponseId, // NOT an Option<String>
    pub multiple_capture_data: Option<MultipleCaptureRequestData>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub integrity_object: Option<CaptureIntegrityObject>,
    pub browser_info: Option<BrowserInformation>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub metadata: Option<SecretSerdeValue>,
    pub order_tax_amount: Option<MinorUnit>,
    pub merchant_order_id: Option<String>,
    pub split_payments: Option<SplitPaymentsDetails>,
    pub split_settlement: Option<Box<SplitSettlement>>,
}
```

There is **no** `payment_amount` (E0609) and `amount_to_capture` is not an `Option`, so
`.is_none()` on it is E0599. `is_multiple_capture()` and `get_connector_transaction_id()`
are the two accessors on this type.

## Transformers File Pattern

### Capture Request Struct

```rust
// Only include fields the connector actually uses
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")] // Adjust per connector API
pub struct {ConnectorName}CaptureRequest {
    // One of the five types in `common_utils::types`: MinorUnit, StringMinorUnit,
    // StringMajorUnit, FloatMajorUnit, StringTwoDecimalUnit. Match the vendor's wire
    // format -- do not default to StringMinorUnit.
    pub amount: {AmountType},
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
    /// Deserialization-layer catch-all so a status the vendor adds later does not fail the
    /// whole response parse. Required alongside an exhaustive mapping `match` -- reviewers
    /// check for both halves, and a catch-all `_ =>` in the `match` below is rejected.
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}CaptureStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}CaptureStatus) -> Self {
        // Exhaustive -- no `_ =>` arm, so a new variant breaks the build instead of being
        // silently misclassified.
        match status {
            {ConnectorName}CaptureStatus::Succeeded
            | {ConnectorName}CaptureStatus::Captured
            | {ConnectorName}CaptureStatus::Completed => Self::Charged,

            {ConnectorName}CaptureStatus::Failed
            | {ConnectorName}CaptureStatus::Error => Self::CaptureFailed,

            {ConnectorName}CaptureStatus::Pending
            | {ConnectorName}CaptureStatus::Processing => Self::Pending,

            {ConnectorName}CaptureStatus::Cancelled => Self::Voided,

            // Non-terminal: an unrecognised status is not proof the capture failed.
            {ConnectorName}CaptureStatus::Unknown => Self::Pending,
        }
    }
}
```

**Status mapping table:**

| Connector Status | AttemptStatus | Reasoning |
|-----------------|---------------|-----------|
| `captured`, `settled`, `completed`, `success` | `Charged` | Capture completed |
| `pending`, `processing`, `submitted` | `Pending` | Capture in progress |
| `failed`, `declined`, `rejected` | `CaptureFailed` | Capture failed (flow-specific -- not the generic `Failure`) |
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

        // Purpose: API requires original transaction reference for capture.
        // `connector_transaction_id` is a `ResponseId`; the accessor unwraps the
        // `ConnectorTransactionId` variant and errors otherwise.
        let transaction_id = router_data.request.get_connector_transaction_id()?;

        Ok(Self {
            amount: item.amount, // Pre-converted by amount_converter
            currency: router_data.request.currency.to_string(),
            transaction_id: Some(transaction_id),
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

        // In-band failure: a 2xx body carrying a failed status is still a failure and must
        // come back as `Err(ErrorResponse { .. })`. Gate on the status the connector reported,
        // via `domain_types::utils::is_payment_failure` -- not on the HTTP code, and not on
        // the mere presence of an `error` field.
        if domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    // `.unwrap_or_default()` yields an empty string that reaches the merchant
                    // as a blank code/message. Use the shared sentinels instead.
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
                    // `attempt_status` is `Option<FlowStatus>`, not `Option<AttemptStatus>`.
                    // Hardcoding `Some(AttemptStatus::Failure)` is both a type error and the
                    // bug that reports a charged payment as FAILURE. Carry the status actually
                    // derived from this response. A blanket `None` is equally wrong -- a hard
                    // decline that stays Pending keeps retrying. See `connectors/flywire.rs`
                    // (full form) and `connectors/noon.rs` (minimal form).
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(response.id.clone()),
                    // `ErrorResponse` has 13 fields and implements `Default`.
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        // Success response. `TransactionResponse` is an enum struct-variant: no
        // `..Default::default()` is possible, so all 11 fields must be listed (E0063).
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,
            connector_metadata: None,
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.reference.clone(),
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

Imports for the transformers file:

```rust
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
use domain_types::{
    errors::{ConnectorError, IntegrationError},
    router_data::{ErrorResponse, FlowStatus},
};
```

## Partial vs Full Capture

`PaymentsCaptureData` tells you **what is being captured now** (`amount_to_capture: i64`,
`minor_amount_to_capture: MinorUnit`) but **not what was authorized**. There is no
`payment_amount` field on it, and `amount_to_capture` is not an `Option`, so neither
`request.payment_amount` nor `request.amount_to_capture.is_none()` compiles.

The authorized amount has to come from the connector's own data:

```rust
// Compare against the authorized amount the connector reported. Depending on the API that is
// either echoed in the capture/authorize response body, or stashed into `connector_metadata`
// during Authorize and read back here from `request.connector_feature_data`.
fn is_full_capture(
    request: &PaymentsCaptureData,
    authorized_minor_amount: MinorUnit,
) -> bool {
    request.minor_amount_to_capture == authorized_minor_amount
}
```

If the connector exposes no authorized amount at all, do not guess: send the partial-capture
shape unconditionally, or document that the API does not distinguish the two.

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
        // The authorized amount is not on `PaymentsCaptureData` -- read it back from whatever
        // Authorize published, e.g. `request.connector_feature_data`:
        //   let meta: {ConnectorName}CaptureMeta =
        //       utils::to_connector_meta(item.router_data.request.connector_feature_data
        //           .clone().map(|m| m.expose()))?;
        //   let authorized_minor_amount = meta.authorized_minor_amount;
        if is_full_capture(&item.router_data.request, authorized_minor_amount) {
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
