# VoidPC (Void Post Capture) Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the VoidPC (Void Post Capture) flow in **ANY** payment connector. VoidPC is used to cancel a payment after it has been captured, effectively reversing the transaction before settlement.

## 🚀 Quick Start Guide

To implement VoidPC in a new connector:

1. **Add Trait Declaration**: Add `PaymentVoidPostCaptureV2` trait to your connector
2. **Add VoidPC Flow to Macro**: Include `VoidPC` flow in `create_all_prerequisites!` macro
3. **Implement Request/Response Types**: Create connector-specific request/response structures
4. **Handle Status Mapping**: Map connector status to `AttemptStatus::Voided` or `AttemptStatus::VoidFailed`
5. **Test with Real API**: Verify void functionality with actual connector sandbox

### Example: Adding VoidPC to Existing Connector

```bash
# Add to existing connector:
- Add PaymentVoidPostCaptureV2 trait implementation
- Add VoidPC flow to macro declarations
- Create {ConnectorName}VoidPcRequest / {ConnectorName}VoidPcResponse types
- Implement URL pattern: /payments/{id}/void or /transactions/{id}/cancel
- Map void-specific statuses
```

**✅ Result**: Working VoidPC flow integrated into existing connector

## Table of Contents

1. [Overview](#overview)
2. [VoidPC vs Void (Pre-Capture)](#voidpc-vs-void-pre-capture)
3. [Implementation Patterns](#implementation-patterns)
4. [Request/Response Patterns](#requestresponse-patterns)
5. [URL Construction Patterns](#url-construction-patterns)
6. [Status Mapping](#status-mapping)
7. [Error Handling](#error-handling)
8. [Integration with Existing Connectors](#integration-with-existing-connectors)
9. [Troubleshooting Guide](#troubleshooting-guide)

## Overview

The VoidPC (Void Post Capture) flow cancels a payment **after** it has been captured but **before** it has been settled. This is different from the regular Void flow which cancels an authorized but uncaptured payment.

### Key Differences from Void Flow

| Aspect | Void (Pre-Capture) | VoidPC (Post-Capture) |
|--------|-------------------|----------------------|
| **Timing** | Before capture | After capture, before settlement |
| **Data Type** | `PaymentVoidData` | `PaymentsCancelPostCaptureData` |
| **Flow Type** | `Void` | `VoidPC` |
| **Use Case** | Cancel authorization | Reverse captured transaction |
| **Connector Support** | Most connectors | Limited connector support |

### When to Use VoidPC

Use VoidPC when:
- A payment has been captured but needs to be reversed
- The transaction hasn't been settled yet
- The connector supports post-capture void/cancel operations

**Note**: Many connectors don't support VoidPC and require using Refund flow instead.

### Key Components

- **Trait**: `PaymentVoidPostCaptureV2` - Marker trait for VoidPC capability
- **Flow Type**: `VoidPC` from `connector_flow` module
- **Request Data**: `PaymentsCancelPostCaptureData`
- **Response Data**: `PaymentsResponseData`
- **Status**: Maps to `AttemptStatus::Voided` or `AttemptStatus::VoidFailed`

## VoidPC vs Void (Pre-Capture)

### Void (Pre-Capture) Flow

Used to cancel an authorized payment that hasn't been captured yet.

```rust
// Flow: Void
// Data: PaymentVoidData
// Use case: Cancel before capture

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Void,
        PaymentFlowData,
        PaymentVoidData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.connector_base_url_payments(req);
        let payment_id = req.request.connector_transaction_id.clone();
        Ok(format!("{base_url}/payments/{payment_id}/cancel"))
    }
}
```

### VoidPC (Post-Capture) Flow

Used to reverse a captured payment that hasn't settled yet.

```rust
// Flow: VoidPC
// Data: PaymentsCancelPostCaptureData
// Use case: Reverse after capture, before settlement

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    fn get_url(
        &self,
        req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.connector_base_url_payments(req);
        let payment_id = req.request.connector_transaction_id.clone();
        Ok(format!("{base_url}/payments/{payment_id}/void"))
    }
}
```

## Implementation Patterns

### Pattern 1: Stub Implementation (Most Common)

Most connectors don't support VoidPC and use a stub implementation. In these cases, the Refund flow should be used instead.

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

use domain_types::{
    connector_flow::{Void, VoidPC},
    connector_types::{PaymentVoidData, PaymentsCancelPostCaptureData, PaymentsResponseData},
};

// Void (Pre-Capture) - Usually supported
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Void,
        PaymentFlowData,
        PaymentVoidData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    // Implementation for pre-capture void
}

// VoidPC (Post-Capture) - Often not supported
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    // Empty stub - VoidPC not supported, use Refund instead
}
```

### Pattern 2: Full VoidPC Implementation

For connectors that support post-capture void operations (like WorldpayVantiv).

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.connector_base_url_payments(req);
        let payment_id = req.request.connector_transaction_id.clone();
        Ok(format!("{base_url}/payments/{payment_id}/void"))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = {ConnectorName}VoidPcRequest::try_from({ConnectorName}RouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Json(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ConnectorError,
    > {
        let response: {ConnectorName}VoidPcResponse = res
            .response
            .parse_struct("{ConnectorName}VoidPcResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        if let Some(i) = event_builder {
            i.set_response_body(&response);
        }

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseHandlingFailed)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}
```

### Pattern 3: Macro-Based Implementation

For connectors using the macro framework with VoidPC support.

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        // ... other flows ...
        (
            flow: Void,
            request_body: {ConnectorName}VoidRequest,
            response_body: {ConnectorName}VoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: {ConnectorName}VoidPcRequest,
            response_body: {ConnectorName}VoidPcResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        // ... other flows ...
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        // ... helper functions ...
    }
);

// VoidPC trait marker
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for {ConnectorName}<T>
{
}

// VoidPC implementation using macro
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}VoidPcRequest),
    curl_response: {ConnectorName}VoidPcResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{base_url}/payments/{payment_id}/void"))
        }
    }
);
```

## Request/Response Patterns

### VoidPC Request Types

#### Empty Body Request

Some connectors accept VoidPC requests with an empty body, using only the URL to identify the transaction.

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}VoidPcRequest {}

impl TryFrom<{ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for {ConnectorName}VoidPcRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
```

#### Minimal Request with Reference

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidPcRequest {
    pub reference: String,
    pub reason: Option<String>,
}

impl TryFrom<{ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for {ConnectorName}VoidPcRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        Ok(Self {
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
            reason: Some("Customer request".to_string()),
        })
    }
}
```

#### Full Request with Amount

For connectors that require amount confirmation even for void operations.

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidPcRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub payment_id: String,
    pub reason: Option<String>,
}

impl TryFrom<{ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for {ConnectorName}VoidPcRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        Ok(Self {
            amount: router_data.request.minor_amount,
            currency: router_data.request.currency.to_string(),
            payment_id: router_data.request.connector_transaction_id.clone(),
            reason: Some("Post-capture void".to_string()),
        })
    }
}
```

### VoidPC Response Types

#### Simple Response

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidPcResponse {
    pub id: String,
    pub status: {ConnectorName}VoidPcStatus,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}VoidPcStatus {
    Voided,
    Failed,
    Pending,
}
```

#### Detailed Response

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidPcResponse {
    pub id: String,
    pub original_payment_id: String,
    pub status: {ConnectorName}VoidPcStatus,
    pub amount: MinorUnit,
    pub currency: String,
    pub voided_at: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}VoidPcStatus {
    Approved,
    Declined,
    Pending,
    Error,
}
```

### Response Transformation

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}VoidPcResponse, RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}VoidPcResponse, RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to attempt status
        let status = match response.status {
            {ConnectorName}VoidPcStatus::Voided | {ConnectorName}VoidPcStatus::Approved => {
                common_enums::AttemptStatus::Voided
            }
            {ConnectorName}VoidPcStatus::Failed | {ConnectorName}VoidPcStatus::Declined => {
                common_enums::AttemptStatus::VoidFailed
            }
            {ConnectorName}VoidPcStatus::Pending => {
                common_enums::AttemptStatus::Pending
            }
        };

        // Handle failure cases
        if matches!(status, common_enums::AttemptStatus::VoidFailed) {
            let error_response = ErrorResponse {
                code: response.status.to_string(),
                message: response.message.clone().unwrap_or_default(),
                reason: response.message.clone(),
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
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
            connector_response_reference_id: Some(response.id.clone()),
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

## URL Construction Patterns

### Pattern 1: Payment ID in Path (Most Common)

```rust
fn get_url(
    &self,
    req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/void"))
}
```

### Pattern 2: Transaction ID in Path

```rust
fn get_url(
    &self,
    req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    let transaction_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/transactions/{transaction_id}/cancel"))
}
```

### Pattern 3: Operations-Based URL

For connectors using an operations pattern (like Nexixpay).

```rust
fn get_url(
    &self,
    req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    let operation_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/operations/{operation_id}/void"))
}
```

### Pattern 4: Query Parameter Style

```rust
fn get_url(
    &self,
    req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/void?payment_id={payment_id}"))
}
```

## Status Mapping

### Standard VoidPC Status Mapping

| Connector Status | AttemptStatus | Description |
|-----------------|---------------|-------------|
| `voided`, `approved`, `success` | `Voided` | Void completed successfully |
| `failed`, `declined`, `error` | `VoidFailed` | Void operation failed |
| `pending`, `processing` | `Pending` | Void is being processed |

### Status Mapping Implementation

```rust
impl From<{ConnectorName}VoidPcStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}VoidPcStatus) -> Self {
        match status {
            {ConnectorName}VoidPcStatus::Voided | {ConnectorName}VoidPcStatus::Approved => {
                Self::Voided
            }
            {ConnectorName}VoidPcStatus::Failed | {ConnectorName}VoidPcStatus::Declined => {
                Self::VoidFailed
            }
            {ConnectorName}VoidPcStatus::Pending => Self::Pending,
        }
    }
}
```

### Manual Status Mapping Function

```rust
fn map_void_pc_status(
    connector_status: &str,
) -> common_enums::AttemptStatus {
    match connector_status.to_lowercase().as_str() {
        "voided" | "approved" | "success" | "completed" => {
            common_enums::AttemptStatus::Voided
        }
        "failed" | "declined" | "error" | "refused" | "rejected" => {
            common_enums::AttemptStatus::VoidFailed
        }
        "pending" | "processing" | "initiated" => {
            common_enums::AttemptStatus::Pending
        }
        _ => {
            // Default to pending for unknown statuses
            common_enums::AttemptStatus::Pending
        }
    }
}
```

## Error Handling

### Common VoidPC Errors

```rust
fn handle_void_pc_error(
    error_code: &str,
    error_message: &str,
) -> Option<ErrorResponse> {
    match error_code {
        "ALREADY_VOIDED" | "PAYMENT_ALREADY_VOIDED" => Some(ErrorResponse {
            code: error_code.to_string(),
            message: "Payment has already been voided".to_string(),
            reason: Some(error_message.to_string()),
            status_code: 400,
            attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }),
        "ALREADY_SETTLED" | "PAYMENT_SETTLED" => Some(ErrorResponse {
            code: error_code.to_string(),
            message: "Payment has already been settled, use refund instead".to_string(),
            reason: Some(error_message.to_string()),
            status_code: 400,
            attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }),
        "VOID_NOT_SUPPORTED" | "OPERATION_NOT_SUPPORTED" => Some(ErrorResponse {
            code: error_code.to_string(),
            message: "Void operation not supported for this payment".to_string(),
            reason: Some(error_message.to_string()),
            status_code: 400,
            attempt_status: Some(common_enums::AttemptStatus::VoidFailed),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        }),
        _ => None,
    }
}
```

### NotSupported Error Pattern

If VoidPC is not supported by the connector, return a clear error:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    fn get_request_body(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        Err(IntegrationError::NotSupported {
            message: "VoidPC (void post capture) is not supported by this connector. Use Refund flow instead.".to_string(),
            connector: "{ConnectorName, context: Default::default() }".to_string(),
        }
        .into())
    }
}
```

## Integration with Existing Connectors

### Adding VoidPC to Existing Connector

#### Step 1: Add VoidPC Flow to Macro Declaration

```rust
// Add VoidPC to the api array in create_all_prerequisites!
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        // ... existing flows ...
        (
            flow: VoidPC,
            request_body: {ConnectorName}VoidPcRequest,
            response_body: {ConnectorName}VoidPcResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        // ... other flows ...
    ],
    // ...
);
```

#### Step 2: Add Trait Declaration

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for {ConnectorName}<T>
{
}
```

#### Step 3: Create Request/Response Types

```rust
// In transformers.rs

#[derive(Debug, Serialize)]
pub struct {ConnectorName}VoidPcRequest {
    // Fields based on connector API
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}VoidPcResponse {
    // Fields based on connector API
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}VoidPcStatus {
    Voided,
    Failed,
    Pending,
}
```

#### Step 4: Implement Transformations

```rust
// Request transformation
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for {ConnectorName}VoidPcRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        // Build request from router data
        Ok(Self { })
    }
}

// Response transformation
impl TryFrom<ResponseRouterData<{ConnectorName}VoidPcResponse, RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}VoidPcResponse, RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        // Map response to router data
        // Handle status mapping
        // Return appropriate AttemptStatus
    }
}
```

#### Step 5: Add Macro Implementation (if using macros)

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}VoidPcRequest),
    curl_response: {ConnectorName}VoidPcResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{base_url}/payments/{payment_id}/void"))
        }
    }
);
```

## Troubleshooting Guide

### Common Issues and Solutions

#### Issue 1: VoidPC Not Supported by Connector

**Error**: Connector API doesn't have void/cancel endpoint for captured payments

**Solution**:
- Use stub implementation for VoidPC
- Document that Refund flow should be used instead
- Return `NotSupported` error with helpful message

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for {ConnectorName}<T>
{
    fn get_request_body(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        Err(IntegrationError::NotSupported {
            message: "VoidPC not supported. Use Refund instead.".to_string(),
            connector: "{ConnectorName, context: Default::default() }".to_string(),
        }
        .into())
    }
}
```

#### Issue 2: Payment Already Settled

**Error**: `ALREADY_SETTLED` or similar error from connector

**Solution**:
- Map to `VoidFailed` status
- Return error suggesting to use Refund flow
- Check settlement status before attempting void

#### Issue 3: Wrong Transaction ID

**Error**: `TRANSACTION_NOT_FOUND` or `INVALID_PAYMENT_ID`

**Solution**:
- Verify correct ID field is being used (connector_transaction_id vs payment_id)
- Check if connector uses different ID for void vs other operations
- Ensure ID is properly extracted from previous response

#### Issue 4: Status Mapping Issues

**Error**: Payment marked as failed when void succeeded, or vice versa

**Solution**:
- Verify status mapping matches connector's actual response values
- Check for case sensitivity issues
- Handle all possible status values from connector

```rust
// Debug: Log actual response status
fn try_from(
    item: ResponseRouterData<{ConnectorName}VoidPcResponse, ...>,
) -> Result<Self, Self::Error> {
    // Add logging to see actual status
    println!("VoidPC response status: {:?}", item.response.status);

    // Map status
    let status = match item.response.status {
        // ... mapping logic
    };
}
```

### Testing Checklist

- [ ] **Stub Implementation**: Verify stub returns appropriate NotSupported error
- [ ] **Full Implementation**: Test complete void flow with actual API
- [ ] **Status Mapping**: Verify all connector statuses map correctly
- [ ] **Error Cases**: Test error scenarios (already voided, already settled, etc.)
- [ ] **URL Construction**: Verify correct endpoint is called
- [ ] **Request Format**: Verify request body matches connector expectations
- [ ] **Response Parsing**: Verify response is parsed correctly
- [ ] **Transaction ID**: Verify correct ID is used and returned

### Best Practices

1. **Check Connector API First**: Not all connectors support VoidPC - verify API documentation
2. **Use Refund as Fallback**: If VoidPC fails due to settlement, guide users to Refund
3. **Clear Error Messages**: When VoidPC is not supported, clearly indicate to use Refund
4. **Status Consistency**: Ensure status mapping is consistent with other flows
5. **Documentation**: Document if connector supports VoidPC or requires Refund instead

### Connectors with VoidPC Support

Based on analysis of the codebase, most connectors use **stub implementations** for VoidPC:

- **WorldpayVantiv**: Full VoidPC implementation with XML-based API
- **Stripe, Adyen, Checkout**: Stub implementations (use Refund instead)
- **Cybersource, PayPal, Bluesnap**: Stub implementations
- **Most Other Connectors**: Stub implementations

### Summary

VoidPC is a specialized flow that most connectors don't support. The typical pattern is:

1. Implement `PaymentVoidPostCaptureV2` trait as a marker
2. Provide either:
   - **Full implementation** if connector supports post-capture void
   - **Stub implementation** that returns `NotSupported` error
3. Guide users to use Refund flow when VoidPC is not supported
4. Map statuses correctly for connectors that do support VoidPC

For most connectors, the Refund flow is the preferred method for reversing captured payments.
