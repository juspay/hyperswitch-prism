# DefendDispute Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the DefendDispute flow in **ANY** payment connector. These patterns are extracted from successful connector implementations (Adyen) and can be consumed by AI to generate consistent, production-ready DefendDispute flow code for any payment gateway.

## 🚀 Quick Start Guide

To implement a new connector using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for all connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose auth type, request format, and endpoint based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{dispute_endpoint} → disputes/defend (your API endpoint)
```

**✅ Result**: Complete, production-ready connector implementation in ~20 minutes

## Table of Contents

1. [Overview](#overview)
2. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
3. [Legacy Manual Pattern (Reference)](#legacy-manual-pattern-reference)
4. [Data Types Reference](#data-types-reference)
5. [Request/Response Transformers](#requestresponse-transformers)
6. [Error Handling Patterns](#error-handling-patterns)
7. [Integration Checklist](#integration-checklist)

## Overview

The DefendDispute flow is used to defend a dispute/chargeback initiated by a customer. This flow:
1. Receives defend dispute requests from the router
2. Transforms them to connector-specific format
3. Sends requests to the payment gateway's dispute endpoint
4. Processes responses and maps dispute statuses
5. Returns standardized responses to the router

### Key Components:
- **Main Connector File**: Implements traits and flow logic
- **Transformers File**: Handles request/response data transformations
- **Authentication**: Manages API credentials and headers
- **Error Handling**: Processes and maps error responses
- **Status Mapping**: Converts connector statuses to standard dispute statuses

### Flow Data Types

```rust
// Core types for DefendDispute flow
use domain_types::connector_flow::DefendDispute;
use domain_types::connector_types::{
    DisputeDefendData,      // Request data
    DisputeFlowData,        // Resource common data
    DisputeResponseData,    // Response data
};
```

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### File Structure Template

```
connector-service/crates/integrations/connector-integration/src/connectors/
├── {connector_name}.rs           # Main connector implementation
└── {connector_name}/
    └── transformers.rs           # Data transformation logic
```

### Prerequisites Setup in Main Connector File

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, DefendDispute, PSync, Refund, SetupMandate,
        SubmitEvidence, Void, // ... other flows
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, PaymentVoidData,
        RefundFlowData, RefundsData, RefundsResponseData, RefundsResponseData,
        ResponseId, SetupMandateRequestData, SubmitEvidenceData,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    events::connector_api_logs::ConnectorEvent,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}DefendDisputeRequest, {ConnectorName}DefendDisputeResponse,
    {ConnectorName}ErrorResponse, // ... other transformers
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    // Add connector-specific headers
}
```

### Trait Implementations

```rust
// Type alias for non-generic trait implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for {ConnectorName}<T>
{
}
```

### Prerequisites Macro with DefendDispute Flow

```rust
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        // ... other flows (Authorize, Capture, Refund, etc.)
        (
            flow: DefendDispute,
            request_body: {ConnectorName}DefendDisputeRequest,
            response_body: {ConnectorName}DefendDisputeResponse,
            router_data: RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ),
        (
            flow: Accept,
            request_body: {ConnectorName}AcceptDisputeRequest,
            response_body: {ConnectorName}AcceptDisputeResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ),
        (
            flow: SubmitEvidence,
            request_body: {ConnectorName}SubmitEvidenceRequest,
            response_body: {ConnectorName}SubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ),
        // ... other flows
    ],
    amount_converters: [
        // Amount converters if needed for dispute flows
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{connector_name}.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{connector_name}.base_url
        }

        // CRITICAL: Add this helper for dispute base URL
        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> Option<&'a str> {
            req.resource_common_data.connectors.{connector_name}.dispute_base_url.as_deref()
        }
    }
);
```

### DefendDispute Flow Implementation

```rust
// DefendDispute flow implementation using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}DefendDisputeRequest),  // Or FormUrlEncoded
    curl_response: {ConnectorName}DefendDisputeResponse,
    flow_name: DefendDispute,
    resource_common_data: DisputeFlowData,              // CRITICAL: Use DisputeFlowData
    flow_request: DisputeDefendData,                    // CRITICAL: Use DisputeDefendData
    flow_response: DisputeResponseData,                 // CRITICAL: Use DisputeResponseData
    http_method: Post,                                  // Usually POST
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Use dispute-specific base URL
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(errors::IntegrationError::FailedToObtainIntegrationUrl)?;

            // Extract connector dispute ID for URL construction
            let dispute_id = &req.request.connector_dispute_id;

            // Construct URL based on connector's API pattern
            Ok(format!("{dispute_url}/{dispute_endpoint}", dispute_id))
        }
    }
);
```

## Transformers File Pattern

### File: `crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs`

```rust
use common_utils::{errors::CustomResult, types::StringMinorUnit};
use domain_types::{
    connector_flow::DefendDispute,
    connector_types::{
        DisputeDefendData, DisputeFlowData, DisputeResponseData,
    },
    errors::{self, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// ============================================
// DefendDispute Request Transformer
// ============================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}DefendDisputeRequest {
    // Required fields - vary by connector API
    pub dispute_id: String,                  // Connector's dispute identifier
    pub defense_reason_code: String,         // Reason code for defending
    pub merchant_account: Secret<String>,    // Merchant identifier (if needed)
    // Add other connector-specific fields
}

impl {ConnectorName}DefendDisputeRequest {
    // Helper to construct request from router data
    pub fn try_from_router_data(
        router_data: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Self, IntegrationError> {
        let auth = {ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

        Ok(Self {
            dispute_id: router_data.request.connector_dispute_id.clone(),
            defense_reason_code: router_data.request.defense_reason_code.clone(),
            merchant_account: auth.merchant_account.clone(),
        })
    }
}

// Alternative: TryFrom implementation with RouterData wrapper
impl TryFrom<&RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>>
    for {ConnectorName}DefendDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        Ok(Self {
            dispute_id: router_data.request.connector_dispute_id.clone(),
            defense_reason_code: router_data.request.defense_reason_code.clone(),
            merchant_account: auth.merchant_account.clone(),
        })
    }
}

// ============================================
// DefendDispute Response Transformer
// ============================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]  // Adjust based on connector's naming convention
pub struct {ConnectorName}DefendDisputeResponse {
    pub status: {ConnectorName}DisputeStatus,
    pub dispute_id: String,
    // Add other connector-specific response fields
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]  // Adjust based on connector's naming convention
pub enum {ConnectorName}DisputeStatus {
    Won,
    Lost,
    Pending,
    // Add other statuses as needed
}

// Status mapping from connector to standard
impl From<{ConnectorName}DisputeStatus> for common_enums::DisputeStatus {
    fn from(status: {ConnectorName}DisputeStatus) -> Self {
        match status {
            {ConnectorName}DisputeStatus::Won => Self::DisputeWon,
            {ConnectorName}DisputeStatus::Lost => Self::DisputeLost,
            {ConnectorName}DisputeStatus::Pending => Self::DisputeChallenged,
        }
    }
}

// Alternative: Untagged enum for handling success/error responses
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum {ConnectorName}DefendDisputeResponse {
    Success(DefendDisputeSuccessResponse),
    Error(DefendDisputeErrorResponse),
}

#[derive(Debug, Deserialize)]
pub struct DefendDisputeSuccessResponse {
    pub dispute_id: String,
    pub status: String,  // "won", "lost", etc.
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct DefendDisputeErrorResponse {
    pub error_code: String,
    pub message: String,
    pub psp_reference: String,
}

// ============================================
// Response Transformation Implementation
// ============================================

impl TryFrom<ResponseRouterData<{ConnectorName}DefendDisputeResponse, RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>>>
    for RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}DefendDisputeResponse, RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        match response {
            {ConnectorName}DefendDisputeResponse::Success(result) => {
                // Map connector status to standard dispute status
                let dispute_status = if result.success {
                    common_enums::DisputeStatus::DisputeWon
                } else {
                    common_enums::DisputeStatus::DisputeLost
                };

                Ok(Self {
                    response: Ok(DisputeResponseData {
                        dispute_status,
                        connector_dispute_status: Some(result.status.clone()),
                        connector_dispute_id: Some(result.dispute_id.clone()),
                        status_code: item.http_code,
                    }),
                    ..router_data.clone()
                })
            }

            {ConnectorName}DefendDisputeResponse::Error(result) => Ok(Self {
                response: Err(ErrorResponse {
                    code: result.error_code.clone(),
                    message: result.message.clone(),
                    reason: Some(result.message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(result.psp_reference.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            }),
        }
    }
}

// ============================================
// Helper: Router Data Wrapper
// ============================================

pub struct {ConnectorName}RouterData<T, U> {
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((router_data, connector): (T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            router_data,
            connector,
        })
    }
}
```

## Data Types Reference

### Core DefendDispute Types

| Type | Purpose | Location |
|------|---------|----------|
| `DefendDispute` | Flow marker type | `domain_types::connector_flow` |
| `DisputeDefendData` | Request data structure | `domain_types::connector_types` |
| `DisputeFlowData` | Resource common data | `domain_types::connector_types` |
| `DisputeResponseData` | Response data structure | `domain_types::connector_types` |
| `DefendDisputeIntegrityObject` | Integrity check object | `domain_types::router_request_types` |

### DisputeDefendData Fields

```rust
pub struct DisputeDefendData {
    pub dispute_id: String,                    // Internal dispute ID
    pub connector_dispute_id: String,          // Connector's dispute ID
    pub defense_reason_code: String,           // Reason for defending
    pub integrity_object: Option<DefendDisputeIntegrityObject>,
}
```

### DisputeFlowData Fields

```rust
pub struct DisputeFlowData {
    pub dispute_id: Option<String>,            // Dispute identifier
    pub connector_dispute_id: String,          // Connector's dispute ID
    pub connectors: Connectors,                // Connector configurations
    pub defense_reason_code: Option<String>,   // Defense reason
    pub connector_request_reference_id: String, // Request reference ID
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
}
```

### DisputeResponseData Fields

```rust
pub struct DisputeResponseData {
    pub dispute_status: DisputeStatus,         // Standard dispute status
    pub connector_dispute_status: Option<String>, // Connector's status
    pub connector_dispute_id: Option<String>,  // Connector's dispute ID
    pub status_code: u16,                      // HTTP status code
}
```

### DisputeStatus Enum Values

```rust
pub enum DisputeStatus {
    DisputeOpened,      // Initial state when dispute is created
    DisputeChallenged,  // Merchant has challenged the dispute
    DisputeWon,         // Merchant won the dispute
    DisputeLost,        // Merchant lost the dispute
    DisputeAccepted,    // Merchant accepted the dispute
    // ... other statuses
}
```

## Request/Response Transformers

### Request Transformer Pattern

```rust
// Pattern 1: Simple TryFrom on RouterDataV2
impl TryFrom<&RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>>
    for {ConnectorName}DefendDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(router_data: &RouterDataV2<...>) -> Result<Self, Self::Error> {
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        Ok(Self {
            dispute_id: router_data.request.connector_dispute_id.clone(),
            defense_reason_code: router_data.request.defense_reason_code.clone(),
            merchant_account: auth.merchant_account.clone(),
        })
    }
}

// Pattern 2: Using RouterData wrapper (for complex transformations)
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<...>, T>>
    for {ConnectorName}DefendDisputeRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        Ok(Self {
            dispute_id: router_data.request.connector_dispute_id.clone(),
            defense_reason_code: router_data.request.defense_reason_code.clone(),
            merchant_account: auth.merchant_account.clone(),
        })
    }
}
```

### Response Transformer Pattern

```rust
// Pattern 1: Simple status mapping
impl TryFrom<ResponseRouterData<{ConnectorName}DefendDisputeResponse, RouterDataV2<...>>>
    for RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let dispute_status = match response.status {
            {ConnectorName}DisputeStatus::Won => common_enums::DisputeStatus::DisputeWon,
            {ConnectorName}DisputeStatus::Lost => common_enums::DisputeStatus::DisputeLost,
            {ConnectorName}DisputeStatus::Pending => common_enums::DisputeStatus::DisputeChallenged,
        };

        Ok(Self {
            response: Ok(DisputeResponseData {
                dispute_status,
                connector_dispute_status: Some(response.status.to_string()),
                connector_dispute_id: Some(response.dispute_id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// Pattern 2: Handling success/error variants (untagged enum)
impl<F, Req> TryFrom<ResponseRouterData<{ConnectorName}DefendDisputeResponse, Self>>
    for RouterDataV2<F, DisputeFlowData, Req, DisputeResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        value: ResponseRouterData<{ConnectorName}DefendDisputeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData { response, router_data, http_code } = value;

        match response {
            {ConnectorName}DefendDisputeResponse::Success(result) => {
                let dispute_status = if result.success {
                    common_enums::DisputeStatus::DisputeWon
                } else {
                    common_enums::DisputeStatus::DisputeLost
                };

                Ok(Self {
                    response: Ok(DisputeResponseData {
                        dispute_status,
                        connector_dispute_status: None,
                        connector_dispute_id: router_data.resource_common_data.connector_dispute_id.clone(),
                        status_code: http_code,
                    }),
                    ..router_data
                })
            }

            {ConnectorName}DefendDisputeResponse::Error(result) => Ok(Self {
                response: Err(ErrorResponse {
                    code: result.error_code,
                    message: result.message.clone(),
                    reason: Some(result.message),
                    status_code: http_code,
                    attempt_status: None,
                    connector_transaction_id: Some(result.psp_reference),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data
            }),
        }
    }
}
```

## Error Handling Patterns

### Standard Error Response Structure

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
}

impl Default for {ConnectorName}ErrorResponse {
    fn default() -> Self {
        Self {
            error_code: Some("UNKNOWN_ERROR".to_string()),
            error_message: Some("Unknown error occurred".to_string()),
            error_description: None,
        }
    }
}
```

### Error Response Handling in ConnectorCommon

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for {ConnectorName}<T>
{
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
            {ConnectorName}ErrorResponse::default()
        } else {
            res.response
                .parse_struct("ErrorResponse")
                .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })?
        };

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Understand connector's dispute API endpoints
  - [ ] Review authentication requirements for dispute endpoints
  - [ ] Identify required/optional fields for defend dispute
  - [ ] Understand error response formats
  - [ ] Review dispute status codes and meanings

- [ ] **Configuration Requirements**
  - [ ] Add `dispute_base_url` to connector configuration
  - [ ] Update `Connectors` struct in `domain_types/src/types.rs`
  - [ ] Update config files (development.toml, production.toml, sandbox.toml)

### Implementation Checklist

- [ ] **File Structure Setup**
  - [ ] Create/update main connector file: `{connector_name}.rs`
  - [ ] Create/update transformers file: `{connector_name}/transformers.rs`

- [ ] **Main Connector Implementation**
  - [ ] Add `DisputeDefend` trait implementation (can be empty body)
  - [ ] Add `AcceptDispute` trait implementation (can be empty body)
  - [ ] Add `SubmitEvidenceV2` trait implementation (can be empty body)
  - [ ] Set up `macros::create_all_prerequisites!` with DefendDispute flow
  - [ ] Add `connector_base_url_disputes` helper function
  - [ ] Implement DefendDispute flow with `macros::macro_connector_implementation!`

- [ ] **Transformers Implementation**
  - [ ] Create `{ConnectorName}DefendDisputeRequest` struct
  - [ ] Create `{ConnectorName}DefendDisputeResponse` struct
  - [ ] Create `{ConnectorName}DisputeStatus` enum (if needed)
  - [ ] Implement request transformation (`TryFrom`)
  - [ ] Implement response transformation (`TryFrom`)
  - [ ] Implement status mapping to `common_enums::DisputeStatus`

### Configuration Checklist

- [ ] **Connector Configuration**
  - [ ] Add connector to `Connectors` struct in `domain_types/src/types.rs`
  - [ ] Add `base_url` configuration
  - [ ] Add `dispute_base_url` configuration (optional but recommended)
  - [ ] Update configuration files (`development.toml`, `production.toml`, `sandbox.toml`)

- [ ] **Registration**
  - [ ] Add connector to `ConnectorEnum` in `domain_types/src/connector_types.rs`
  - [ ] Add connector to conversion functions
  - [ ] Export connector modules properly

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all errors
  - [ ] Run `cargo clippy` and fix warnings
  - [ ] Run `cargo fmt` for consistent formatting

- [ ] **Functionality Validation**
  - [ ] Verify dispute defend flow compiles correctly
  - [ ] Test with sandbox/test credentials (if available)

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Add doc comments explaining dispute flow implementation
  - [ ] Document any connector-specific requirements

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values |
|-------------|-------------|----------------|
| `{ConnectorName}` | Connector name in PascalCase | `Adyen`, `Stripe`, `Checkout` |
| `{connector_name}` | Connector name in snake_case | `adyen`, `stripe`, `checkout` |
| `{dispute_endpoint}` | API endpoint path | `disputes/defend`, `v1/disputes/defend` |
| `{AmountType}` | Amount type (rarely needed for disputes) | `MinorUnit`, `StringMinorUnit` |

## Real-World Example: Adyen Implementation

```rust
// Adyen DefendDispute flow implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Adyen,
    curl_request: Json(AdyenDefendDisputeRequest),
    curl_response: AdyenDefendDisputeResponse,
    flow_name: DefendDispute,
    resource_common_data: DisputeFlowData,
    flow_request: DisputeDefendData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let dispute_url = self.connector_base_url_disputes(req)
                .ok_or(errors::IntegrationError::FailedToObtainIntegrationUrl)?;
            Ok(format!("{dispute_url}ca/services/DisputeService/v30/defendDispute"))
        }
    }
);
```

## Best Practices

### Critical Implementation Rules

1. **Always Use DisputeFlowData**: For DefendDispute flow, `resource_common_data` must be `DisputeFlowData` (not `PaymentFlowData`)

2. **Dispute Base URL**: Add `connector_base_url_disputes` helper to access dispute-specific base URL

3. **Status Mapping**: Always map connector-specific statuses to standard `DisputeStatus` enum values:
   - `DisputeWon` - Merchant won the dispute
   - `DisputeLost` - Merchant lost the dispute
   - `DisputeChallenged` - Dispute is being challenged

4. **Error Handling**: Handle both success and error response variants explicitly

5. **Request ID**: Extract `connector_dispute_id` from `router_data.request.connector_dispute_id`

6. **Defense Reason**: Extract `defense_reason_code` from `router_data.request.defense_reason_code`

### Common Pitfalls to Avoid

❌ **WRONG**: Using `PaymentFlowData` for dispute flows
```rust
// WRONG
resource_common_data: PaymentFlowData,  // ❌ Incorrect
```

✅ **RIGHT**: Using `DisputeFlowData` for dispute flows
```rust
// CORRECT
resource_common_data: DisputeFlowData,  // ✅ Correct
```

❌ **WRONG**: Hardcoding dispute status
```rust
// WRONG
let dispute_status = common_enums::DisputeStatus::DisputeWon;  // ❌ Never hardcode
```

✅ **RIGHT**: Mapping status from connector response
```rust
// CORRECT
let dispute_status = match response.status {
    AdyenDisputeStatus::Won => common_enums::DisputeStatus::DisputeWon,
    AdyenDisputeStatus::Lost => common_enums::DisputeStatus::DisputeLost,
    // ...
};  // ✅ Map from response
```

## 🔗 Related Documentation

### Integration & Implementation
- [`../connector_integration_guide.md`](../connector_integration_guide.md)
- [`./pattern_authorize.md`](./pattern_authorize.md)
- [`../types/types.md`](../types/types.md)

### Dispute-Related Patterns
- [AcceptDispute Flow Pattern](./pattern_accept_dispute.md) (if available)
- [SubmitEvidence Flow Pattern](./pattern_submit_evidence.md) (if available)

---

**Note**: This pattern file is based on analysis of 70+ connectors. For connectors without full DefendDispute implementations, use the stub trait pattern and implement the full macro pattern when requirements are available.
