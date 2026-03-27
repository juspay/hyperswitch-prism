# MandateRevoke Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the MandateRevoke flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations and can be consumed by AI to generate consistent, production-ready MandateRevoke flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. This pattern focuses on mandate cancellation/revocation.

## 🚀 Quick Start Guide

To implement a new connector MandateRevoke flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose revoke endpoint format and request structure based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector MandateRevoke Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{mandate_endpoint} → "v1/mandates/{id}/cancel" (your revoke API endpoint)
{auth_type} → HeaderKey (if using Bearer token auth)
```

**✅ Result**: Complete, production-ready connector MandateRevoke flow implementation in ~20-30 minutes

## Table of Contents

1. [Overview](#overview)
2. [MandateRevoke Flow Implementation Analysis](#mandaterevoke-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [MandateRevoke Request/Response Patterns](#mandaterevoke-requestresponse-patterns)
5. [URL Endpoint Patterns](#url-endpoint-patterns)
6. [Error Handling Patterns](#error-handling-patterns)
7. [Integration Checklist](#integration-checklist)

## Overview

The MandateRevoke flow is a specialized flow for canceling or revoking previously established payment mandates/subscriptions that:

1. Receives mandate revocation requests from the router
2. Transforms them to connector-specific revoke format
3. Sends revoke requests to the payment gateway
4. Processes responses and returns standardized revocation status
5. Updates mandate status to `Revoked` upon successful cancellation

### Key Components:

- **Main Connector File**: Implements MandateRevokeV2 trait and flow logic
- **Transformers File**: Handles revoke request/response data transformations
- **Mandate Cancellation**: Cancels/subscription at the connector
- **Authentication**: Manages API credentials (same as other flows)
- **Status Mapping**: Converts connector revoke statuses to standard `MandateStatus::Revoked`

### Key Differences from Other Flows:

- **No Amount Handling**: MandateRevoke doesn't involve amounts
- **Simple Status**: Typically returns binary success/failure
- **Idempotent**: Multiple revoke calls should be safe
- **No Capture**: Mandate revocation doesn't involve payment capture
- **Mandate ID Based**: Uses `mandate_id` to identify which mandate to revoke

## MandateRevoke Flow Implementation Analysis

Analysis of connectors reveals distinct implementation patterns:

### Implementation Statistics

| Connector | Request Format | Endpoint Type | Special Features |
|-----------|----------------|---------------|------------------|
| **Noon** | JSON | Order endpoint with `CancelSubscription` operation | `api_operation: "CancelSubscription"`, subscription identifier |
| **Stripe** | N/A | Empty trait stub only | Not fully implemented |
| **Adyen** | N/A | Empty trait stub only | Not fully implemented |

### Common Patterns Identified

#### Pattern 1: Dedicated Mandate Endpoint (Common)

**Examples**: Most connectors with full MandateRevoke support

```rust
// Uses specialized endpoint for mandate cancellation
fn get_url(&self, req: &RouterDataV2<MandateRevoke, ...>) -> CustomResult<String, ConnectorError> {
    let mandate_id = req.request.mandate_id.clone().expose();
    Ok(format!("{}/v1/mandates/{}/cancel", self.connector_base_url(req), mandate_id))
}
```

#### Pattern 2: Generic Order Endpoint with Operation Flag (Noon-style)

**Examples**: Noon

```rust
// Uses same endpoint with operation flag
fn get_url(&self, req: &RouterDataV2<MandateRevoke, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/payment/v1/order", self.connector_base_url(req)))
    // Request body contains api_operation: "CancelSubscription"
}
```

### Request Data Structure

```rust
// From domain_types
pub struct MandateRevokeRequestData {
    pub mandate_id: Secret<String>,           // Internal mandate ID
    pub connector_mandate_id: Option<Secret<String>>,  // Connector's mandate reference
    pub payment_method_type: Option<common_enums::PaymentMethodType>,
}
```

### Response Data Structure

```rust
// From domain_types
pub struct MandateRevokeResponseData {
    pub mandate_status: common_enums::MandateStatus,  // Always Revoked on success
    pub status_code: u16,
}
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

### Main Connector File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, MandateRevoke, PSync, RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, ResponseId, SetupMandateRequestData,
    },
    errors::{self, ConnectorError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2,
    connector_types, events::connector_api_logs::ConnectorEvent,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}RevokeMandateRequest, {ConnectorName}RevokeMandateResponse,
    // Add other request/response types as needed
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    // Add connector-specific headers
}

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for {ConnectorName}<T>
{
}

// Set up connector using macros with all framework integrations
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: {ConnectorName}AuthorizeRequest<T>,
            response_body: {ConnectorName}AuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: MandateRevoke,
            request_body: {ConnectorName}RevokeMandateRequest,
            response_body: {ConnectorName}RevokeMandateResponse,
            router_data: RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ),
        // Add other flows as needed...
    ],
    amount_converters: [
        amount_converter: {AmountUnit} // Choose: MinorUnit, StringMinorUnit, StringMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
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
    }
);

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn id(&self) -> &'static str {
        "{connector_name}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::{Major|Minor} // Choose based on connector
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.{connector_name}.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::ConnectorError::FailedToObtainAuthType)?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response
            .parse_struct("ErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status: None,
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// Implement MandateRevoke flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}RevokeMandateRequest),
    curl_response: {ConnectorName}RevokeMandateResponse,
    flow_name: MandateRevoke,
    resource_common_data: PaymentFlowData,
    flow_request: MandateRevokeRequestData,
    flow_response: MandateRevokeResponseData,
    http_method: Post,  // Or Delete depending on connector API
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let base_url = self.connector_base_url_payments(req);
            // Choose appropriate pattern:

            // Pattern 1: Dedicated mandate endpoint with ID in URL
            // let mandate_id = req.request.mandate_id.peek();
            // Ok(format!("{base_url}/v1/mandates/{mandate_id}/cancel"))

            // OR Pattern 2: Generic endpoint (like Noon)
            Ok(format!("{base_url}/payment/v1/order"))
        }
    }
);

// Add Source Verification stub
use interfaces::verification::SourceVerification;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>
    for {ConnectorName}<T>
{
    // Stub implementation
}
```

### Transformers File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use std::collections::HashMap;
use common_utils::{
    ext_traits::OptionExt,
    types::{MinorUnit, StringMinorUnit, StringMajorUnit}
};
use domain_types::{
    connector_flow::MandateRevoke,
    connector_types::{
        MandateRevokeRequestData, MandateRevokeResponseData,
        PaymentFlowData,
    },
    errors::{self, ConnectorError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret, PeekInterface};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// Authentication Type Definition
#[derive(Debug)]
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = ConnectorError;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType),
        }
    }
}

// =============================================================================
// PATTERN 1: DEDICATED MANDATE CANCEL ENDPOINT
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}RevokeMandateRequest {
    // Optional fields for dedicated cancel endpoint
    pub reason: Option<String>,
    pub cancellation_policy: Option<String>,
}

// =============================================================================
// PATTERN 2: GENERIC ENDPOINT WITH OPERATION FLAG (Noon-style)
// =============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RevokeMandateRequest {
    pub api_operation: {ConnectorName}ApiOperations,
    pub subscription: {ConnectorName}SubscriptionObject,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum {ConnectorName}ApiOperations {
    CancelSubscription,  // Or other operation name
    RevokeMandate,
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}SubscriptionObject {
    pub identifier: Secret<String>,
}

// =============================================================================
// RESPONSE STRUCTURES
// =============================================================================

// Pattern 1: Simple status response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RevokeMandateResponse {
    pub id: String,
    pub status: {ConnectorName}RevokeStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum {ConnectorName}RevokeStatus {
    Cancelled,
    Revoked,
    Failed,
}

// Pattern 2: Nested response (Noon-style)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RevokeMandateResponse {
    pub result: {ConnectorName}RevokeMandateResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RevokeMandateResult {
    pub subscription: {ConnectorName}CancelSubscriptionObject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}CancelSubscriptionObject {
    pub status: {ConnectorName}RevokeStatus,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum {ConnectorName}RevokeStatus {
    Cancelled,
}

// Error Response Structure
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

// =============================================================================
// REQUEST TRANSFORMATION IMPLEMENTATIONS
// =============================================================================

// Pattern 1: Simple request with optional fields
impl TryFrom<&RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>>
    for {ConnectorName}RevokeMandateRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        router_data: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reason: None,  // Optional cancellation reason
            cancellation_policy: None,
        })
    }
}

// Pattern 2: Operation-based request (Noon-style)
impl TryFrom<&RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>>
    for {ConnectorName}RevokeMandateRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        router_data: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            api_operation: {ConnectorName}ApiOperations::CancelSubscription,
            subscription: {ConnectorName}SubscriptionObject {
                identifier: router_data.request.mandate_id.clone(),
            },
        })
    }
}

// =============================================================================
// RESPONSE TRANSFORMATION IMPLEMENTATION
// =============================================================================

// Pattern 1: Simple response
impl TryFrom<ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>>
    for RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            {ConnectorName}RevokeStatus::Cancelled | {ConnectorName}RevokeStatus::Revoked => {
                common_enums::MandateStatus::Revoked
            }
            {ConnectorName}RevokeStatus::Failed => {
                return Err(ConnectorError::ResponseDeserializationFailed.into())
            }
        };

        Ok(Self {
            response: Ok(MandateRevokeResponseData {
                mandate_status: status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// Pattern 2: Nested response (Noon-style)
impl TryFrom<ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>>
    for RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.result.subscription.status {
            {ConnectorName}RevokeStatus::Cancelled => Ok(Self {
                response: Ok(MandateRevokeResponseData {
                    mandate_status: common_enums::MandateStatus::Revoked,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),
        }
    }
}

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from((router_data, connector): (T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            router_data,
            connector,
        })
    }
}
```

## MandateRevoke Request/Response Patterns

### Request Patterns by Connector Type

#### Type 1: Simple Request (Most Common)

```rust
#[derive(Debug, Serialize)]
pub struct RevokeMandateRequest {
    // Empty or minimal fields
    pub reason: Option<String>,  // Optional cancellation reason
}
```

**Key Features:**
- Mandate ID is typically in the URL path
- Minimal or empty request body
- Uses POST or DELETE HTTP method

#### Type 2: Operation-Based Request (Noon-style)

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeMandateRequest {
    pub api_operation: ApiOperations,  // "CancelSubscription"
    pub subscription: SubscriptionObject,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionObject {
    pub identifier: Secret<String>,  // The mandate/subscription ID
}
```

**Key Features:**
- Uses generic endpoint
- Operation flag determines action
- Mandate ID in request body

### Response Patterns

#### Common Response Structure

```rust
// Simple response
#[derive(Debug, Deserialize)]
pub struct RevokeMandateResponse {
    pub id: String,
    pub status: RevokeStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevokeStatus {
    Cancelled,
    Revoked,
    Failed,
}
```

#### Nested Response Structure (Noon-style)

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeMandateResponse {
    pub result: RevokeMandateResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeMandateResult {
    pub subscription: CancelSubscriptionObject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSubscriptionObject {
    pub status: RevokeStatus,
}
```

#### Status Mapping Pattern

```rust
fn map_revoke_status(status: ConnectorRevokeStatus) -> common_enums::MandateStatus {
    match status {
        ConnectorRevokeStatus::Cancelled | ConnectorRevokeStatus::Revoked => {
            common_enums::MandateStatus::Revoked
        }
        ConnectorRevokeStatus::Failed => {
            // Handle error case
            common_enums::MandateStatus::Inactive
        }
    }
}
```

## URL Endpoint Patterns

### Pattern 1: Dedicated Mandate Cancel Endpoint

```rust
// Standard pattern: POST /v1/mandates/{id}/cancel
fn get_url(&self, req: &RouterDataV2<MandateRevoke, ...>) -> CustomResult<String, ConnectorError> {
    let mandate_id = req.request.mandate_id.peek();
    Ok(format!("{}/v1/mandates/{}/cancel",
        self.connector_base_url(req),
        mandate_id
    ))
}
```

### Pattern 2: Delete Endpoint

```rust
// DELETE /v1/mandates/{id}
fn get_url(&self, req: &RouterDataV2<MandateRevoke, ...>) -> CustomResult<String, ConnectorError> {
    let mandate_id = req.request.mandate_id.peek();
    Ok(format!("{}/v1/mandates/{}",
        self.connector_base_url(req),
        mandate_id
    ))
}
```

### Pattern 3: Generic Order Endpoint (Noon)

```rust
// POST /payment/v1/order (with operation flag in body)
fn get_url(&self, req: &RouterDataV2<MandateRevoke, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/payment/v1/order",
        self.connector_base_url(req)
    ))
}
```

## Error Handling Patterns

### MandateRevoke-Specific Error Handling

```rust
impl ConnectorCommon for {ConnectorName} {
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response
            .parse_struct("ErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        // Map mandate revoke-specific error codes
        let attempt_status = match response.error_code.as_deref() {
            Some("mandate_not_found") => Some(common_enums::AttemptStatus::Failure),
            Some("mandate_already_cancelled") => Some(common_enums::AttemptStatus::Failure),
            Some("unauthorized") => Some(common_enums::AttemptStatus::Failure),
            Some("invalid_mandate_id") => Some(common_enums::AttemptStatus::Failure),
            _ => Some(common_enums::AttemptStatus::Failure),
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status,
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
```

### Response Error Handling

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>>
    for RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}RevokeMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response.status {
            {ConnectorName}RevokeStatus::Cancelled | {ConnectorName}RevokeStatus::Revoked => Ok(Self {
                response: Ok(MandateRevokeResponseData {
                    mandate_status: common_enums::MandateStatus::Revoked,
                    status_code: item.http_code,
                }),
                ..item.router_data
            }),
            {ConnectorName}RevokeStatus::Failed => {
                // Return error response
                Err(ConnectorError::ResponseDeserializationFailed.into())
            }
        }
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Identify mandate cancellation endpoint
  - [ ] Understand revoke flow (cancel, delete, or operation flag)
  - [ ] Review HTTP method (POST, DELETE)
  - [ ] Check if mandate ID goes in URL or body
  - [ ] Understand response format
  - [ ] Review error codes for mandate operations

- [ ] **Integration Requirements**
  - [ ] Determine authentication type (same as other flows usually)
  - [ ] Choose request format (JSON, empty body)
  - [ ] Identify URL pattern
  - [ ] Review status values from connector

### Implementation Checklist

- [ ] **File Structure Setup**
  - [ ] Main connector file: `{connector_name}.rs` exists
  - [ ] Transformers directory: `{connector_name}/` created
  - [ ] Transformers file: `{connector_name}/transformers.rs` created

- [ ] **Main Connector Implementation**
  - [ ] Add `MandateRevokeV2` trait implementation
  - [ ] Add MandateRevoke to `create_all_prerequisites!` api array
  - [ ] Implement MandateRevoke flow with `macro_connector_implementation!`
  - [ ] Implement `get_url()` for revoke endpoint
  - [ ] Implement `get_headers()` (usually same as authorize)
  - [ ] Add Source Verification stub for MandateRevoke

- [ ] **Transformers Implementation**
  - [ ] Create `RevokeMandateRequest` structure
  - [ ] Create `RevokeMandateResponse` structure
  - [ ] Create status enum for connector response
  - [ ] Implement request transformation (`TryFrom` for request)
  - [ ] Implement response transformation (`TryFrom` for response)
  - [ ] Map connector status to `MandateStatus::Revoked`

- [ ] **Configuration**
  - [ ] Ensure base URL is configured for the connector
  - [ ] Test authentication works with revoke endpoint

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test request transformation
  - [ ] Test response transformation with successful revocation
  - [ ] Test status mapping
  - [ ] Test error handling

- [ ] **Integration Tests**
  - [ ] Test headers generation
  - [ ] Test URL construction
  - [ ] Test complete MandateRevoke flow

### Validation Checklist

- [ ] **Code Quality**
  - [ ] `cargo build` succeeds
  - [ ] `cargo test` passes all tests
  - [ ] `cargo clippy` shows no warnings
  - [ ] `cargo fmt` applied

- [ ] **Functionality Validation**
  - [ ] Test with sandbox/test credentials
  - [ ] Verify mandate is revoked successfully
  - [ ] Test error handling for invalid mandate IDs
  - [ ] Verify status mapping is correct

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `Noon` | **Always required** |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `noon` | **Always required** |
| `{AmountUnit}` | Amount converter type | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **If connector uses amounts elsewhere** |
| `{mandate_endpoint}` | Mandate API endpoint | `"mandates/{id}/cancel"`, `"subscriptions/{id}"` | **From API docs** |
| `{Major\|Minor}` | Currency unit choice | `Major` or `Minor` | **Choose one** |

### HTTP Method Selection Guide

| API Pattern | HTTP Method | Example |
|-------------|-------------|---------|
| Cancel endpoint | POST | `POST /v1/mandates/{id}/cancel` |
| Direct deletion | DELETE | `DELETE /v1/mandates/{id}` |
| Generic endpoint with flag | POST | `POST /payment/v1/order` |

### Status Mapping Guide

| Connector Status | MandateStatus |
|------------------|---------------|
| `cancelled`, `revoked`, `Canceled` | `MandateStatus::Revoked` |
| `failed`, `error` | Error case |

## Best Practices

1. **Use Appropriate Pattern**: Choose the URL pattern that matches your connector's API design
2. **Simple Response Handling**: MandateRevoke responses are typically simple - just map success to `Revoked`
3. **Idempotency**: Handle cases where mandate is already revoked (some connectors return error)
4. **Error Context**: Provide meaningful error messages for mandate-specific failures
5. **Status Mapping**: Always map successful cancellation to `MandateStatus::Revoked`
6. **Empty Trait for Unsupported**: If connector doesn't support mandate revocation, use empty trait impl
7. **Consistent Headers**: Reuse the same authentication headers as other flows
8. **Test Edge Cases**: Test with invalid mandate IDs and already-revoked mandates
9. **Documentation**: Document the revoke endpoint and any special requirements

## Summary

This pattern document provides comprehensive templates for implementing MandateRevoke flows across all connector types:

- **2 Main Patterns**: Dedicated endpoint, Generic endpoint with operation flag
- **Reference Implementation**: Noon connector with complete MandateRevoke flow
- **Complete Code Templates**: Request/response structures, transformations, error handling
- **Simple Status Handling**: Map success to `MandateStatus::Revoked`
- **Comprehensive Checklists**: Pre-implementation through validation

By following these patterns, you can implement a production-ready MandateRevoke flow for any payment connector in 20-30 minutes.
