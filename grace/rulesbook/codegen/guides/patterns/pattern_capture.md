# Capture Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the capture flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations across all 22 connectors in the connector service and can be consumed by AI to generate consistent, production-ready capture flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. For traditional Hyperswitch patterns, refer to legacy documentation.

## 🚀 Quick Start Guide

To implement a new connector capture flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose capture endpoint format, request structure, and amount converter based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector Capture Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{AmountType} → StringMinorUnit (if API expects "1000" for $10.00)
{content_type} → "application/json" (if API uses JSON)
{capture_endpoint} → "v1/payments/{id}/capture" (your capture API endpoint)
{auth_type} → HeaderKey (if using Bearer token auth)
```

**✅ Result**: Complete, production-ready connector capture flow implementation in ~20 minutes

## Critical Implementation Rules (From PR Feedback)

### Status Mapping (CRITICAL)
- **NEVER hardcode `status: AttemptStatus::Charged`**
- Always map status from connector response fields
- If connector returns minimal response (only ID and timestamp):
  - Use HTTP status code (2xx = Charged, 4xx/5xx = Failure)
  - Document WHY each status is chosen
  - Example: `// Success HTTP code with valid ID -> Charged because connector accepted capture synchronously`

### Validation
- **Only add validations required by connector API spec**
- Always include comment explaining PURPOSE of validation
- Example: `// Purpose: API requires transaction reference for capture`
- Avoid unnecessary validations that add complexity without value
- If connector API will reject without validation, include it; otherwise remove it

### Field Usage
- **Remove fields that would be hardcoded to None**
- Keep request/response structures clean and minimal
- Only include fields your connector actually uses
- Example: Don't include `merchant_id: None` if connector doesn't use it

### Quick Reference
```rust
// WRONG - Never do this
status: AttemptStatus::Charged, // Hardcoded!

// CORRECT - Map from response
let status = common_enums::AttemptStatus::from(response.status.clone());

// WRONG - Unnecessary validation
if item.amount.get_amount_as_i64() <= 0 { ... } // Already validated upstream

// CORRECT - API-required validation with purpose
// Purpose: API requires original transaction reference for capture
let transaction_id = router_data.request.connector_transaction_id
    .as_ref()
    .ok_or_else(|| IntegrationError::MissingConnectorTransactionID)?;
```

## Table of Contents

1. [Overview](#overview)
2. [Capture Flow Implementation Analysis](#capture-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [Status Mapping Best Practices](#status-mapping-best-practices) **NEW**
5. [Validation Guidelines](#validation-guidelines) **NEW**
6. [Legacy Manual Pattern (Reference)](#legacy-manual-pattern-reference)
7. [Capture Request/Response Patterns](#capture-requestresponse-patterns)
8. [URL Endpoint Patterns](#url-endpoint-patterns)
9. [Amount Handling Patterns](#amount-handling-patterns)
10. [Error Handling Patterns](#error-handling-patterns)
11. [Testing Patterns](#testing-patterns)
12. [Integration Checklist](#integration-checklist)

## Overview

The capture flow is a critical payment processing flow that:
1. Receives payment capture requests from the router (typically for manual capture scenarios)
2. Transforms them to connector-specific capture format
3. Sends capture requests to the payment gateway using the original transaction reference
4. Processes capture responses and maps statuses
5. Returns standardized capture responses to the router

### Key Components:
- **Main Connector File**: Implements PaymentCapture trait and flow logic
- **Transformers File**: Handles capture request/response data transformations
- **URL Construction**: Builds capture endpoint URLs (typically with transaction ID)
- **Authentication**: Manages API credentials (same as authorization flow)
- **Amount Processing**: Handles capture amount (full or partial capture)
- **Status Mapping**: Converts connector capture statuses to standard statuses

## 🔍 API Analysis Methodology

**⚠️ CRITICAL: Always perform thorough API analysis before implementation to avoid dual-endpoint issues**

Before implementing any capture flow, follow this systematic API analysis process:

### Step 1: OpenAPI/Documentation Deep Dive

1. **Identify All Capture-Related Endpoints**
   ```bash
   # Search for capture-related paths in OpenAPI spec
   grep -i "capture\|settle\|settlement" openapi.json
   ```
   
2. **Document Each Endpoint's Purpose**
   - Full capture/settlement endpoints
   - Partial capture/settlement endpoints  
   - Settlement vs capture terminology differences
   - Any specialized capture endpoints

3. **Compare Request Schemas**
   ```json
   // Example: Two different schemas found
   "SettleRequest": { "type": "object" }  // Empty for full settlements
   "PartialSettleRequest": {              // Complex for partial settlements
     "required": ["reference", "value"],
     "properties": { ... }
   }
   ```

4. **Compare Response Schemas**
   - Check if different endpoints return different response structures
   - Identify common vs endpoint-specific response fields
   - Note any response format variations

### Step 2: API Pattern Classification

Classify your connector's capture API into one of these patterns:

#### Pattern A: Single Endpoint (Standard)
- **Characteristics**: One capture endpoint handles all capture types
- **Example**: `/api/payments/{id}/capture`
- **Request**: Same structure for full and partial captures
- **When to use**: Most traditional payment APIs

#### Pattern B: Dual Endpoint (Complex)
- **Characteristics**: Separate endpoints for full vs partial captures
- **Example**: `/settlements` (empty body) vs `/partialSettlements` (complex body)
- **Request**: Different request structures per endpoint
- **When to use**: APIs like Worldpay that distinguish settlement types

#### Pattern C: Multiple Specialized Endpoints
- **Characteristics**: Multiple capture endpoints for different scenarios
- **Example**: `/capture`, `/settle`, `/finalize`
- **Request**: Endpoint-specific request structures
- **When to use**: Complex payment platforms with multiple capture methods

### Step 3: Request/Response Analysis Checklist

- [ ] **Empty Request Body Support**: Does any endpoint require `{}` as request body?
- [ ] **Conditional Fields**: Are certain fields required only for specific capture types?
- [ ] **Amount Handling**: How does the API distinguish full vs partial captures?
- [ ] **Transaction Reference**: Where does the original transaction ID go (URL path vs body)?
- [ ] **Response Variations**: Do different endpoints return different response structures?
- [ ] **Status Code Mapping**: What success/failure status codes does each endpoint return?

### Step 4: Implementation Strategy Selection

Based on your analysis, choose the appropriate implementation approach:

#### For Single Endpoint APIs (Pattern A)
```rust
// Use standard single-endpoint pattern from this guide
fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/api/payments/{}/capture", base_url, transaction_id))
}
```

#### For Dual Endpoint APIs (Pattern B)
```rust
// Implement conditional endpoint selection
fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    let transaction_id = req.request.get_connector_transaction_id()?;
    
    // Choose endpoint based on capture type
    if is_full_capture(&req.request) {
        Ok(format!("{}/api/payments/{}/settlements", base_url, transaction_id))
    } else {
        Ok(format!("{}/api/payments/{}/partialSettlements", base_url, transaction_id))
    }
}
```

#### For Multiple Endpoint APIs (Pattern C)
```rust
// Implement multi-endpoint selection logic
fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
    match determine_capture_method(&req.request) {
        CaptureMethod::Immediate => format!("{}/api/payments/{}/capture", base_url, transaction_id),
        CaptureMethod::Settlement => format!("{}/api/payments/{}/settle", base_url, transaction_id),
        CaptureMethod::Finalization => format!("{}/api/payments/{}/finalize", base_url, transaction_id),
    }
}
```

### Step 5: Pre-Implementation Validation

- [ ] **API Documentation Clarity**: Are all endpoint behaviors clearly documented?
- [ ] **Test Scenarios Identified**: Do you know how to test each endpoint?
- [ ] **Error Case Mapping**: Are error responses documented for each endpoint?
- [ ] **Endpoint Selection Logic**: Is the logic for choosing endpoints clear and testable?

### Example: Worldpay Analysis Outcome

Following this methodology for Worldpay would have revealed:

1. **Two Distinct Endpoints**:
   - `/settlements` - requires empty request body `{}`
   - `/partialSettlements` - requires complex request body with `reference`, `value`, `sequence`

2. **Different Response Schemas**:
   - Settlements response lacks `transactionReference` field
   - Different `_actions` and `_links` structures

3. **Implementation Strategy**: Pattern B (Dual Endpoint)
   - Conditional endpoint selection based on capture amount
   - Different request structures per endpoint
   - Unified response handling with schema variations

This analysis would have prevented the "bodyDoesNotMatchSchema" error and response parsing issues encountered during implementation.

## Capture Flow Implementation Analysis

Based on analysis of all 22 connectors in the connector service, here's the implementation status:

### ✅ Full Capture Flow Implementation (8 connectors)
These connectors have complete capture flow implementations with dedicated request/response structures:

1. **Adyen** - Modern macro implementation with dedicated capture endpoint
2. **Authorizedotnet** - Full capture with transaction wrapping pattern
3. **Braintree** - Complete capture flow support
4. **Fiserv** - Full implementation with reference transaction details
5. **Fiuu** - Complete capture support
6. **PayU** - Full capture implementation
7. **Razorpay** - Complete capture flow support
8. **Xendit** - Full capture implementation

### 🔧 Stub/Trait Implementation Only (14 connectors)
These connectors implement the PaymentCapture trait but have empty/stub implementations:

9. **Bluecode** - Stub implementation only
10. **Cashfree** - Trait implemented, no capture flow
11. **Cashtocode** - Stub implementation
12. **Checkout** - Trait only, no capture logic
13. **Elavon** - Stub implementation
14. **Mifinity** - Empty capture implementation
15. **Nexinets** - Stub only
16. **Noon** - Trait implementation only
17. **Novalnet** - Stub implementation
18. **Paytm** - Empty capture flow
19. **PhonePe** - Stub implementation
20. **RazorpayV2** - Trait only (separate from Razorpay)
21. **Volt** - Stub implementation
22. **Worldpay** - Empty capture implementation

### 📊 Implementation Statistics
- **Complete implementations**: 8/22 (36%)
- **Stub implementations**: 14/22 (64%)
- **Most common pattern**: Macro-based with JSON requests
- **Most common auth**: HeaderKey (Bearer token)
- **Most common amount format**: MinorUnit/StringMinorUnit

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### Main Connector File Pattern (Capture Flow Addition)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

// In the imports section, ensure Capture flow is included:
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, CreateOrder, CreateSessionToken, DefendDispute, PSync, RSync,
        Refund, RepeatPayment, SetupMandate, SubmitEvidence, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        // ... other imports
    },
};

// In transformers import, include capture types:
use transformers::{
    {ConnectorName}AuthorizeRequest, {ConnectorName}AuthorizeResponse,
    {ConnectorName}CaptureRequest, {ConnectorName}CaptureResponse,
    {ConnectorName}ErrorResponse, {ConnectorName}SyncRequest, {ConnectorName}SyncResponse,
    // ... other types
};

// Implement PaymentCapture trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for {ConnectorName}<T>
{
}

// Add Capture flow to the macro prerequisites
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
            flow: Capture,
            request_body: {ConnectorName}CaptureRequest,
            response_body: {ConnectorName}CaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: {ConnectorName}SyncRequest,
            response_body: {ConnectorName}SyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        // Add other flows as needed...
    ],
    amount_converters: [
        amount_converter: {AmountUnit} // StringMinorUnit, StringMajorUnit, MinorUnit
    ],
    member_functions: {
        // Same build_headers and connector_base_url functions as authorization flow
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "{content_type}".to_string().into(),
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

// Implement Capture flow using macro framework
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
            // Extract transaction ID from connector_transaction_id
            let transaction_id = match &req.request.connector_transaction_id {
                Some(id) => id,
                None => return Err(errors::IntegrationError::MissingConnectorTransactionID.into()),
            };
            
            let base_url = self.connector_base_url_payments(req);
            
            // Choose appropriate URL pattern based on connector API:
            // Pattern 1: REST-style with transaction ID in path
            Ok(format!("{base_url}/{capture_endpoint}", 
                base_url = base_url,
                capture_endpoint = "{capture_endpoint}".replace("{id}", transaction_id)
            ))
            
            // Pattern 2: Same endpoint as payments (for connectors like Authorizedotnet)
            // Ok(base_url.to_string())
            
            // Pattern 3: Different base + specific capture path
            // Ok(format!("{base_url}/capture/{transaction_id}"))
        }
    }
);

// Add Source Verification stub for Capture flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for {ConnectorName}<T>
{
    // Stub implementation - will be replaced in Phase 10
}
```

### Transformers File Pattern (Capture Flow)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

// Add capture-specific imports to existing imports:
use domain_types::{
    connector_flow::{Authorize, Capture, PSync}, // Add Capture here
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, 
        PaymentsSyncData, ResponseId,
    },
    // ... other imports
};

// Capture Request Structure
// IMPORTANT: Only include fields that your connector actually uses
// Remove any fields that would be hardcoded to None - keep structures clean
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}CaptureRequest {
    // Common capture request fields across connectors:

    // Amount fields (choose based on connector requirements)
    pub amount: {AmountType}, // MinorUnit, StringMinorUnit, StringMajorUnit
    pub currency: String,

    // Transaction reference (varies by connector - include if used in request body)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,        // Some connectors need this in body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,             // Original payment reference

    // Merchant information (only include if required by connector)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_account: Option<Secret<String>>, // For Adyen-style connectors

    // Additional fields (only include if connector supports them)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// Alternative: Wrapped Request Structure (like Authorizedotnet)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}CaptureRequestWrapper {
    // For connectors that wrap the actual request
    pub capture_transaction_request: {ConnectorName}CaptureRequestInternal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}CaptureRequestInternal {
    pub merchant_authentication: {ConnectorName}AuthType,
    pub transaction_request: {ConnectorName}CaptureTransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}CaptureTransactionDetails {
    pub transaction_type: {ConnectorName}TransactionType, // Usually "PriorAuthCaptureTransaction"
    pub amount: {AmountType},
    pub ref_trans_id: String, // Reference to original authorization
}

// Capture Response Structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}CaptureResponse {
    // Common response fields
    pub id: String,                    // Capture transaction ID
    pub status: {ConnectorName}CaptureStatus,
    pub amount: Option<{AmountType}>,
    
    // Reference fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,    // Original payment ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,     // Merchant reference
    
    // Timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_at: Option<String>,
    
    // Additional connector-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    
    // Error information (for failed captures)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// Alternative: Simple Response Structure (like Authorizedotnet)
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}CaptureResponse(pub {ConnectorName}PaymentsResponse);

// Capture Status Enumeration
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")] // Adjust based on connector
pub enum {ConnectorName}CaptureStatus {
    // Common statuses across connectors
    Succeeded,
    Success,      // Alternative naming
    Captured,     // Alternative naming
    Completed,    // Alternative naming
    
    Failed,
    Error,        // Alternative naming
    
    Pending,
    Processing,   // Alternative naming
    
    // Connector-specific statuses
    PartiallyRefunded,
    Cancelled,
}

// Status mapping for capture responses
// CRITICAL: Map status from connector response fields - NEVER hardcode status
impl From<{ConnectorName}CaptureStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}CaptureStatus) -> Self {
        match status {
            {ConnectorName}CaptureStatus::Succeeded
            | {ConnectorName}CaptureStatus::Success
            | {ConnectorName}CaptureStatus::Captured
            | {ConnectorName}CaptureStatus::Completed => Self::Charged,

            {ConnectorName}CaptureStatus::Failed
            | {ConnectorName}CaptureStatus::Error => Self::Failure,

            {ConnectorName}CaptureStatus::Pending
            | {ConnectorName}CaptureStatus::Processing => Self::Pending,

            {ConnectorName}CaptureStatus::Cancelled => Self::Voided,
            {ConnectorName}CaptureStatus::PartiallyRefunded => Self::PartialCharged,
        }
    }
}

// Request Transformation Implementation
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>>
    for {ConnectorName}CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Validate that we have a connector transaction ID
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingConnectorTransactionID)?;

        Ok(Self {
            amount: item.amount, // Converted amount from RouterData
            currency: router_data.request.currency.to_string(),
            transaction_id: Some(transaction_id.clone()),
            reference: Some(router_data.resource_common_data.connector_request_reference_id.clone()),
            // Only include fields that connector actually requires
            // AVOID setting fields to None - remove them from struct instead
            merchant_account: Some(get_merchant_account(&router_data.connector_auth_type)?),
            description: Some(format!("Capture for payment {}", transaction_id)),
        })
    }
}

// Alternative: Wrapped Request Transformation (for Authorizedotnet-style)
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>>
    for {ConnectorName}CaptureRequestWrapper
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingConnectorTransactionID)?;

        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        Ok(Self {
            capture_transaction_request: {ConnectorName}CaptureRequestInternal {
                merchant_authentication: auth,
                transaction_request: {ConnectorName}CaptureTransactionDetails {
                    transaction_type: {ConnectorName}TransactionType::PriorAuthCaptureTransaction,
                    amount: item.amount,
                    ref_trans_id: transaction_id.clone(),
                },
            },
        })
    }
}

// Response Transformation Implementation
impl TryFrom<ResponseRouterData<{ConnectorName}CaptureResponse, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}CaptureResponse, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map capture status from connector response to standard status
        // CRITICAL: Status MUST be mapped from response.status field, not hardcoded
        // If connector returns minimal response (only ID and timestamp), map from available fields
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

// Helper struct for router data transformation (same as authorize flow)
pub struct {ConnectorName}RouterData<T> {
    pub amount: {AmountType},
    pub router_data: T,
}

impl<T> TryFrom<({AmountType}, T)> for {ConnectorName}RouterData<T> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((amount, router_data): ({AmountType}, T)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data,
        })
    }
}
```

## Status Mapping Best Practices

### CRITICAL: Never Hardcode Status

**WARNING**: One of the most common mistakes in capture implementations is hardcoding `status: AttemptStatus::Charged`. This is NEVER correct.

**Incorrect Example (DO NOT DO THIS):**
```rust
// WRONG - hardcoded status
Ok(Self {
    resource_common_data: PaymentFlowData {
        status: AttemptStatus::Charged, // NEVER hardcode this!
        ..router_data.resource_common_data.clone()
    },
    // ...
})
```

**Correct Example:**
```rust
// CORRECT - map status from connector response
let status = common_enums::AttemptStatus::from(response.status.clone());

Ok(Self {
    resource_common_data: PaymentFlowData {
        status, // Mapped from actual connector response
        ..router_data.resource_common_data.clone()
    },
    // ...
})
```

### Handling Minimal Responses

Some connectors return minimal capture responses with only ID and timestamp. In these cases, you need to determine the appropriate status based on available information.

**Pattern for Minimal Responses:**
```rust
impl TryFrom<ResponseRouterData<{ConnectorName}CaptureResponse, RouterDataV2<Capture, ...>>>
    for RouterDataV2<Capture, ...>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // If connector only returns ID and timestamp (minimal response):
        // Status determination logic:
        // 1. If response includes error fields -> Failure
        // 2. If HTTP status code is 2xx and response has ID -> Charged
        // 3. If HTTP status code is 4xx/5xx -> Failure
        // 4. Otherwise -> Pending (for async processing)

        let status = if let Some(status_field) = &response.status {
            // If connector provides status field, use it
            common_enums::AttemptStatus::from(status_field.clone())
        } else if let Some(_error) = &response.error {
            // Error present -> Failure
            common_enums::AttemptStatus::Failure
        } else if item.http_code >= 200 && item.http_code < 300 {
            // Success HTTP code with valid ID -> Charged
            // Reason: Connector accepted capture request synchronously
            common_enums::AttemptStatus::Charged
        } else if item.http_code >= 400 {
            // Error HTTP code -> Failure
            common_enums::AttemptStatus::Failure
        } else {
            // Accepted but processing asynchronously -> Pending
            // Reason: Connector will process capture asynchronously
            common_enums::AttemptStatus::Pending
        };

        // Rest of transformation...
    }
}
```

### Status Mapping Documentation

Always document WHY a particular status is chosen, especially for edge cases:

```rust
// Define connector status enum
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}CaptureStatus {
    Captured,
    Settled,
    Completed,
    Pending,
    Processing,
    Submitted,
    Failed,
    Declined,
    Rejected,
    Cancelled,
    Voided,
    PartiallyCaptured,
}

// Map capture status from connector response
let status = match response.status {
    // Success states map to Charged
    // Reason: Capture has been completed and funds will be settled
    {ConnectorName}CaptureStatus::Captured
    | {ConnectorName}CaptureStatus::Settled
    | {ConnectorName}CaptureStatus::Completed => AttemptStatus::Charged,

    // Pending states remain Pending
    // Reason: Capture is accepted but not yet completed
    {ConnectorName}CaptureStatus::Pending
    | {ConnectorName}CaptureStatus::Processing
    | {ConnectorName}CaptureStatus::Submitted => AttemptStatus::Pending,

    // Failure states map to Failure
    // Reason: Capture was rejected or failed
    {ConnectorName}CaptureStatus::Failed
    | {ConnectorName}CaptureStatus::Declined
    | {ConnectorName}CaptureStatus::Rejected => AttemptStatus::Failure,

    // Voided captures
    // Reason: Capture was cancelled before settlement
    {ConnectorName}CaptureStatus::Cancelled
    | {ConnectorName}CaptureStatus::Voided => AttemptStatus::Voided,

    // Partial captures (when supported)
    // Reason: Only part of the authorized amount was captured
    {ConnectorName}CaptureStatus::PartiallyCaptured => AttemptStatus::PartialCharged,
};
```

### Status Mapping Table

| Connector Status | AttemptStatus | Reasoning |
|-----------------|---------------|-----------|
| `captured`, `settled`, `completed`, `success` | `Charged` | Capture completed successfully |
| `pending`, `processing`, `submitted`, `queued` | `Pending` | Capture in progress |
| `failed`, `declined`, `rejected`, `error` | `Failure` | Capture failed |
| `cancelled`, `voided`, `reversed` | `Voided` | Capture cancelled |
| `partially_captured` | `PartialCharged` | Partial capture completed |
| Unknown/unmapped | `Pending` | Safe default for retry |

## Validation Guidelines

### When to Add Validations

**Rule**: Only add validations that are explicitly required by the connector's API specification.

**DO add validations for:**
- Required fields per API spec (e.g., `connector_transaction_id`)
- API-enforced constraints (e.g., amount must be <= authorized amount)
- Critical business rules (e.g., currency consistency)

**DO NOT add validations for:**
- Fields that are already validated upstream
- Unnecessary complexity without clear API requirements
- Defensive checks that add no value

### Validation with Purpose Comments

Always explain WHY each validation exists:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>>
    for {ConnectorName}CaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Validation 1: Connector transaction ID is required
        // Purpose: API requires original transaction reference for capture
        // Consequence: Without this, capture request will fail with 400 error
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .as_ref()
            .ok_or_else(|| IntegrationError::MissingConnectorTransactionID)?;

        // Validation 2: Capture amount must not exceed authorized amount
        // Purpose: API enforces this constraint and rejects over-captures
        // Consequence: Without this check, we get error code "AMOUNT_EXCEEDS_AUTH"
        let capture_amount = router_data.request.amount_to_capture
            .unwrap_or(router_data.request.payment_amount);

        if capture_amount > router_data.request.payment_amount {
            return Err(IntegrationError::InvalidRequestData {
                message: "Capture amount cannot exceed authorized amount".to_string(),
            }.into());
        }

        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            transaction_id: transaction_id.clone(),
        })
    }
}
```

### Example: Avoid Unnecessary Validations

**Unnecessary (DO NOT DO THIS):**
```rust
// AVOID: Unnecessary validation - currency is already validated upstream
if router_data.request.currency.to_string().len() != 3 {
    return Err(IntegrationError::InvalidRequestData {
        message: "Currency must be 3 characters".to_string(),
    }.into());
}

// AVOID: Unnecessary validation - amount is already validated upstream
if item.amount.get_amount_as_i64() <= 0 {
    return Err(IntegrationError::InvalidRequestData {
        message: "Amount must be positive".to_string(),
    }.into());
}
```

**Necessary (GOOD):**
```rust
// GOOD: API-specific validation required by connector
if router_data.request.currency != Currency::USD
    && router_data.request.currency != Currency::EUR {
    return Err(IntegrationError::InvalidRequestData {
        message: "Connector only supports USD and EUR for captures".to_string(),
    }.into());
}
```

### Validation Checklist

Before adding a validation, ask:

- [ ] Is this validation required by the connector's API documentation?
- [ ] Will the API reject the request if this validation is missing?
- [ ] Does this validation prevent a specific API error code?
- [ ] Is this validation adding complexity without clear benefit?
- [ ] Is this already validated upstream in the payment flow?

If you can't answer "yes" to the first three questions, the validation is likely unnecessary.

## 🔄 Multi-Endpoint Capture Patterns

**Advanced patterns for APIs with multiple capture endpoints**

### Pattern A: Single Endpoint (Standard)

Most traditional payment APIs use a single endpoint that handles all capture scenarios.

**Implementation:**
```rust
// Single URL pattern - standard approach
fn get_url(&self, req: &RouterDataV2<Capture, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    Ok(format!("{}/api/payments/{}/capture", base_url, transaction_id))
}

// Single request structure handles all scenarios
#[derive(Debug, Serialize)]
pub struct StandardCaptureRequest {
    pub amount: AmountType,
    pub currency: String,
    pub transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_partial_capture: Option<bool>,
}
```

**When to use**: Stripe, Square, PayPal, and most modern payment APIs

### Pattern B: Dual Endpoint (Complex)

APIs that provide separate endpoints for full vs partial captures, often with different request schemas.

**Implementation:**
```rust
// Conditional endpoint selection based on capture type
fn get_url(&self, req: &RouterDataV2<Capture, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Determine if this is a full or partial capture
    let is_full_capture = req.request.amount_to_capture.is_none() || 
        req.request.amount_to_capture == Some(req.request.payment_amount);
    
    if is_full_capture {
        // Full settlement endpoint (typically requires empty body)
        Ok(format!("{}/api/payments/{}/settlements", base_url, transaction_id))
    } else {
        // Partial settlement endpoint (typically requires complex body)
        Ok(format!("{}/api/payments/{}/partialSettlements", base_url, transaction_id))
    }
}

// Different request structures for different endpoints
#[derive(Debug, Serialize)]
pub struct FullCaptureRequest {
    // Often empty object for full settlements
}

#[derive(Debug, Serialize)]
pub struct PartialCaptureRequest {
    pub reference: String,
    pub value: AmountValue,
    pub sequence: CaptureSequence,
}

// Unified transformation logic
impl TryFrom<CaptureRouterData> for ConnectorCaptureRequest {
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let is_full_capture = determine_capture_type(&item.router_data.request);
        
        if is_full_capture {
            Ok(Self::Full(FullCaptureRequest {}))
        } else {
            Ok(Self::Partial(PartialCaptureRequest {
                reference: generate_capture_reference(&item),
                value: convert_partial_amount(&item),
                sequence: calculate_sequence(&item),
            }))
        }
    }
}
```

**When to use**: Worldpay, some enterprise payment platforms with settlement-focused APIs

### Pattern C: Multiple Specialized Endpoints

APIs with multiple capture endpoints for different capture methods or scenarios.

**Implementation:**
```rust
// Multi-endpoint selection based on capture method
fn get_url(&self, req: &RouterDataV2<Capture, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Determine capture method from request metadata or amount
    let capture_method = determine_capture_method(&req.request)?;
    
    match capture_method {
        CaptureMethod::Immediate => {
            // Direct capture endpoint
            Ok(format!("{}/api/payments/{}/capture", base_url, transaction_id))
        },
        CaptureMethod::Settlement => {
            // Settlement-based capture
            Ok(format!("{}/api/payments/{}/settle", base_url, transaction_id))
        },
        CaptureMethod::Finalization => {
            // Transaction finalization endpoint
            Ok(format!("{}/api/payments/{}/finalize", base_url, transaction_id))
        },
        CaptureMethod::BatchSettle => {
            // Batch settlement endpoint
            Ok(format!("{}/api/batch/settle", base_url))
        }
    }
}

// Enum for capture method selection
#[derive(Debug, Clone)]
pub enum CaptureMethod {
    Immediate,   // Standard capture
    Settlement,  // Settlement-based
    Finalization, // Transaction finalization
    BatchSettle, // Batch processing
}

fn determine_capture_method(request: &PaymentsCaptureData) -> Result<CaptureMethod, IntegrationError> {
    // Logic to determine method based on:
    // - Capture amount vs original amount
    // - Timing requirements
    // - Merchant configuration
    // - Transaction metadata
    
    if let Some(capture_method_hint) = &request.capture_method {
        match capture_method_hint.as_str() {
            "immediate" => Ok(CaptureMethod::Immediate),
            "settlement" => Ok(CaptureMethod::Settlement),
            "finalize" => Ok(CaptureMethod::Finalization),
            "batch" => Ok(CaptureMethod::BatchSettle),
            _ => Ok(CaptureMethod::Immediate), // Default fallback
        }
    } else {
        // Default logic based on timing and amount
        Ok(CaptureMethod::Immediate)
    }
}
```

**When to use**: Enterprise platforms, complex payment processors, multi-channel payment systems

### Pattern D: Conditional Multi-Endpoint with Response Variations

Advanced pattern where different endpoints return different response schemas.

**Implementation:**
```rust
// Unified response handling for different endpoint responses
impl TryFrom<ResponseRouterData<ConnectorCaptureResponse, ...>> for RouterDataV2<Capture, ...> {
    fn try_from(item: ResponseRouterData<ConnectorCaptureResponse, ...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Handle different response structures based on endpoint used
        let (status, transaction_id, reference) = match &item.response {
            ConnectorCaptureResponse::Settlement(settlement_resp) => {
                // Settlement endpoint response structure
                let status = match settlement_resp.outcome.as_str() {
                    "sentForSettlement" => AttemptStatus::Charged,
                    _ => AttemptStatus::Pending,
                };
                let tx_id = extract_transaction_id(&settlement_resp.links.self_link.href)
                    .unwrap_or_else(|| "unknown".to_string());
                (status, tx_id, None) // Settlement responses may not have transaction reference
            },
            ConnectorCaptureResponse::PartialSettlement(partial_resp) => {
                // Partial settlement endpoint response structure
                let status = match partial_resp.outcome.as_str() {
                    "sentForSettlement" => AttemptStatus::PartialCharged,
                    _ => AttemptStatus::Pending,
                };
                let tx_id = partial_resp.transaction_reference.clone();
                (status, tx_id.clone(), Some(tx_id))
            },
            ConnectorCaptureResponse::Standard(std_resp) => {
                // Standard capture endpoint response structure
                let status = match std_resp.status.as_str() {
                    "captured" | "settled" => AttemptStatus::Charged,
                    "failed" => AttemptStatus::Failure,
                    _ => AttemptStatus::Pending,
                };
                (status, std_resp.id.clone(), std_resp.reference.clone())
            }
        };

        let mut router_data = router_data.clone();
        router_data.resource_common_data.status = status;
        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: reference,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });
        
        Ok(router_data)
    }
}
```

### Helper Functions for Multi-Endpoint Patterns

```rust
// Generic helper functions for multi-endpoint implementations

/// Determine if this is a full capture based on amounts
fn is_full_capture(request: &PaymentsCaptureData) -> bool {
    request.amount_to_capture.is_none() || 
    request.amount_to_capture == Some(request.payment_amount)
}

/// Extract transaction ID from various link formats
fn extract_transaction_id(href: &str) -> Option<String> {
    href.split('/').last().map(|s| s.to_string())
}

/// Generate unique reference for partial captures
fn generate_capture_reference(router_data: &RouterDataV2<Capture, ...>) -> String {
    format!("capture-{}-{}", 
        router_data.request.connector_transaction_id.as_ref().unwrap(),
        chrono::Utc::now().timestamp()
    )
}

/// Calculate sequence for multi-part captures
fn calculate_sequence(router_data: &RouterDataV2<Capture, ...>) -> CaptureSequence {
    // Implementation depends on connector's sequencing requirements
    CaptureSequence {
        number: 1, // This would be calculated based on previous captures
        total: 1,  // This would be determined from capture plan
    }
}

/// Convert amounts for partial capture requests
fn convert_partial_amount(router_data: &CaptureRouterData) -> AmountValue {
    AmountValue {
        amount: router_data.amount.get_amount_as_i64(),
        currency: router_data.router_data.request.currency.to_string(),
    }
}
```

### Multi-Endpoint Testing Strategies

```rust
#[cfg(test)]
mod multi_endpoint_tests {
    use super::*;

    #[test]
    fn test_endpoint_selection_full_capture() {
        let router_data = create_full_capture_request();
        let connector = TestConnector::new();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("/settlements"));
        assert!(!url.contains("/partialSettlements"));
    }

    #[test]
    fn test_endpoint_selection_partial_capture() {
        let router_data = create_partial_capture_request();
        let connector = TestConnector::new();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("/partialSettlements"));
        assert!(!url.contains("/settlements"));
    }

    #[test]
    fn test_request_structure_variations() {
        let full_capture_data = create_full_capture_request();
        let partial_capture_data = create_partial_capture_request();
        
        let full_req = ConnectorCaptureRequest::try_from(full_capture_data).unwrap();
        let partial_req = ConnectorCaptureRequest::try_from(partial_capture_data).unwrap();
        
        match full_req {
            ConnectorCaptureRequest::Full(_) => {}, // Expected
            _ => panic!("Full capture should use Full request variant"),
        }
        
        match partial_req {
            ConnectorCaptureRequest::Partial(_) => {}, // Expected
            _ => panic!("Partial capture should use Partial request variant"),
        }
    }

    #[test]
    fn test_response_schema_variations() {
        // Test different response structures from different endpoints
        let settlement_response = create_settlement_response();
        let partial_response = create_partial_settlement_response();
        
        let settlement_result = RouterDataV2::try_from(settlement_response).unwrap();
        let partial_result = RouterDataV2::try_from(partial_response).unwrap();
        
        assert_eq!(settlement_result.resource_common_data.status, AttemptStatus::Charged);
        assert_eq!(partial_result.resource_common_data.status, AttemptStatus::PartialCharged);
    }
}
```

## Capture Request/Response Patterns

### Pattern 1: Simple REST Capture (Adyen-style)

**Request Structure:**
```rust
#[derive(Debug, Serialize)]
pub struct AdyenCaptureRequest {
    merchant_account: Secret<String>,
    amount: Amount,
    reference: String,
}
```

**Response Structure:**
```rust
#[derive(Debug, Deserialize)]
pub struct AdyenCaptureResponse {
    merchant_account: Secret<String>,
    payment_psp_reference: String,
    psp_reference: String,
    reference: String,
    status: String,
    amount: Amount,
}
```

**URL Pattern:** `{base_url}/v68/payments/{transaction_id}/captures`

### Pattern 2: Transaction Wrapper Capture (Authorizedotnet-style)

**Request Structure:**
```rust
#[derive(Debug, Serialize)]
pub struct AuthorizedotnetCaptureRequest {
    create_transaction_request: CreateCaptureTransactionRequest,
}

#[derive(Debug, Serialize)]
pub struct CreateCaptureTransactionRequest {
    merchant_authentication: AuthorizedotnetAuthType,
    transaction_request: AuthorizedotnetCaptureTransactionInternal,
}

#[derive(Debug, Serialize)]
pub struct AuthorizedotnetCaptureTransactionInternal {
    transaction_type: TransactionType, // PriorAuthCaptureTransaction
    amount: FloatMajorUnit,
    ref_trans_id: String,
}
```

**Response Structure:**
```rust
#[derive(Debug, Deserialize)]
pub struct AuthorizedotnetCaptureResponse(pub AuthorizedotnetPaymentsResponse);
```

**URL Pattern:** `{base_url}` (same endpoint as other operations)

### Pattern 3: Reference Transaction Capture (Fiserv-style)

**Request Structure:**
```rust
#[derive(Debug, Serialize)]
pub struct FiservCaptureRequest {
    amount: Amount,
    transaction_details: TransactionDetails,
    merchant_details: MerchantDetails,
    reference_transaction_details: ReferenceTransactionDetails,
}
```

**Response Structure:**
```rust
#[derive(Debug, Deserialize)]
pub struct FiservCaptureResponse {
    gateway_response: GatewayResponse,
}
```

**URL Pattern:** `{base_url}/v1/payments/{transaction_id}/capture`

## 🔹 Empty Request Body Patterns

**Handling APIs that require empty or minimal request bodies for certain capture operations**

Many payment APIs, particularly settlement-focused ones, require empty request bodies `{}` for full captures while using complex bodies for partial captures. This pattern is common in enterprise payment platforms.

### Pattern 1: Empty Body for Full Captures

Some APIs distinguish between full and partial captures by request body content rather than just endpoint.

**Implementation:**
```rust
// Empty request structure for full settlements
#[derive(Debug, Serialize)]
pub struct EmptyRequest {}

// Alternative: Use unit struct
#[derive(Debug, Serialize)]
pub struct FullSettlementRequest;

// Request enum to handle both empty and complex requests
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CaptureRequest {
    Empty {},                    // For full captures
    Complex(ComplexCaptureData), // For partial captures
}

impl TryFrom<CaptureRouterData> for CaptureRequest {
    type Error = error_stack::Report<IntegrationError>;
    
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Check if this is a full capture
        let is_full_capture = router_data.request.amount_to_capture.is_none() ||
            router_data.request.amount_to_capture == Some(router_data.request.payment_amount);
        
        if is_full_capture {
            // Return empty object for full settlements
            Ok(CaptureRequest::Empty {})
        } else {
            // Return complex object for partial captures
            Ok(CaptureRequest::Complex(ComplexCaptureData {
                amount: item.amount.get_amount_as_i64(),
                currency: router_data.request.currency.to_string(),
                reference: router_data.resource_common_data.connector_request_reference_id.clone(),
                sequence: Some(CaptureSequence {
                    number: 1,
                    total: 1,
                }),
            }))
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexCaptureData {
    pub amount: i64,
    pub currency: String,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<CaptureSequence>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSequence {
    pub number: i32,
    pub total: i32,
}
```

**When to use**: Worldpay, some banking APIs, settlement-focused payment platforms

### Pattern 2: Conditional Field Inclusion

APIs where the same endpoint accepts different request structures based on capture type.

**Implementation:**
```rust
// Single request structure with conditional fields
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalCaptureRequest {
    // Always present fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    
    // Only for partial captures
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<CaptureSequence>,
    
    // Capture type indicator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_type: Option<String>,
}

impl TryFrom<CaptureRouterData> for ConditionalCaptureRequest {
    type Error = error_stack::Report<IntegrationError>;
    
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let is_full_capture = is_full_capture_amount(&router_data.request);
        
        if is_full_capture {
            // Minimal request for full capture
            Ok(Self {
                transaction_id: None, // May go in URL instead
                amount: None,
                currency: None,
                reference: None,
                sequence: None,
                capture_type: Some("full".to_string()),
            })
        } else {
            // Full request for partial capture
            Ok(Self {
                transaction_id: Some(
                    router_data.request.connector_transaction_id
                        .as_ref()
                        .ok_or(IntegrationError::MissingConnectorTransactionID)?
                        .clone()
                ),
                amount: Some(item.amount.get_amount_as_i64()),
                currency: Some(router_data.request.currency.to_string()),
                reference: Some(router_data.resource_common_data.connector_request_reference_id.clone()),
                sequence: Some(CaptureSequence { number: 1, total: 1 }),
                capture_type: Some("partial".to_string()),
            })
        }
    }
}

// Helper function to determine capture type
fn is_full_capture_amount(request: &PaymentsCaptureData) -> bool {
    match request.amount_to_capture {
        None => true, // No amount specified = full capture
        Some(capture_amount) => capture_amount == request.payment_amount,
    }
}
```

### Pattern 3: Null vs Undefined Field Handling

APIs that distinguish between null values and absent fields in JSON.

**Implementation:**
```rust
use serde_json::Value;

// Custom request structure with explicit null handling
#[derive(Debug, Serialize)]
pub struct NullAwareCaptureRequest {
    // Required fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    
    // Null for full capture, value for partial capture
    #[serde(serialize_with = "serialize_capture_amount")]
    pub amount: CaptureAmount,
    
    // Omit for full capture, include for partial capture
    #[serde(skip_serializing_if = "should_skip_sequence")]
    pub sequence: Option<Value>,
}

#[derive(Debug)]
pub enum CaptureAmount {
    FullCapture,          // Serializes to null
    PartialCapture(i64),  // Serializes to number
}

fn serialize_capture_amount<S>(amount: &CaptureAmount, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match amount {
        CaptureAmount::FullCapture => serializer.serialize_none(),
        CaptureAmount::PartialCapture(value) => serializer.serialize_i64(*value),
    }
}

fn should_skip_sequence(sequence: &Option<Value>) -> bool {
    // Skip sequence field for certain capture types
    sequence.is_none()
}

impl TryFrom<CaptureRouterData> for NullAwareCaptureRequest {
    type Error = error_stack::Report<IntegrationError>;
    
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let is_full_capture = is_full_capture_amount(&router_data.request);
        
        let (amount, sequence) = if is_full_capture {
            (CaptureAmount::FullCapture, None)
        } else {
            (
                CaptureAmount::PartialCapture(item.amount.get_amount_as_i64()),
                Some(serde_json::json!({
                    "number": 1,
                    "total": 1
                }))
            )
        };
        
        Ok(Self {
            transaction_id: router_data.request.connector_transaction_id.clone(),
            amount,
            sequence,
        })
    }
}
```

### Pattern 4: Dynamic Request Structure

Advanced pattern for APIs with highly variable request requirements.

**Implementation:**
```rust
use serde_json::{Map, Value};

// Dynamic request builder for complex APIs
pub struct DynamicCaptureRequestBuilder {
    fields: Map<String, Value>,
}

impl DynamicCaptureRequestBuilder {
    pub fn new() -> Self {
        Self {
            fields: Map::new(),
        }
    }
    
    pub fn add_field_if<T: serde::Serialize>(
        mut self, 
        key: &str, 
        value: T, 
        condition: bool
    ) -> Self {
        if condition {
            if let Ok(json_value) = serde_json::to_value(value) {
                self.fields.insert(key.to_string(), json_value);
            }
        }
        self
    }
    
    pub fn build(self) -> Value {
        Value::Object(self.fields)
    }
}

impl TryFrom<CaptureRouterData> for Value {
    type Error = error_stack::Report<IntegrationError>;
    
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let is_full_capture = is_full_capture_amount(&router_data.request);
        let has_sequence_support = check_sequence_support(&router_data);
        
        let request = DynamicCaptureRequestBuilder::new()
            // Always include transaction reference if available
            .add_field_if(
                "transactionId", 
                &router_data.request.connector_transaction_id, 
                router_data.request.connector_transaction_id.is_some()
            )
            // Include amount only for partial captures
            .add_field_if(
                "amount", 
                item.amount.get_amount_as_i64(), 
                !is_full_capture
            )
            // Include currency only for partial captures
            .add_field_if(
                "currency", 
                router_data.request.currency.to_string(), 
                !is_full_capture
            )
            // Include sequence only if supported and partial capture
            .add_field_if(
                "sequence", 
                serde_json::json!({"number": 1, "total": 1}), 
                !is_full_capture && has_sequence_support
            )
            // Include reference for tracking
            .add_field_if(
                "reference", 
                &router_data.resource_common_data.connector_request_reference_id, 
                !router_data.resource_common_data.connector_request_reference_id.is_empty()
            )
            .build();
        
        Ok(request)
    }
}

fn check_sequence_support(_router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> bool {
    // Implementation-specific logic to check if API supports sequence fields
    // This could be based on API version, merchant configuration, etc.
    true
}
```

### Empty Request Body Testing

```rust
#[cfg(test)]
mod empty_request_tests {
    use super::*;
    
    #[test]
    fn test_empty_request_serialization() {
        let empty_request = CaptureRequest::Empty {};
        let json = serde_json::to_string(&empty_request).unwrap();
        assert_eq!(json, "{}");
    }
    
    #[test]
    fn test_full_capture_generates_empty_body() {
        let router_data = create_full_capture_router_data();
        let request = CaptureRequest::try_from(router_data).unwrap();
        
        match request {
            CaptureRequest::Empty {} => {}, // Expected
            _ => panic!("Full capture should generate empty request"),
        }
    }
    
    #[test]
    fn test_partial_capture_generates_complex_body() {
        let router_data = create_partial_capture_router_data();
        let request = CaptureRequest::try_from(router_data).unwrap();
        
        match request {
            CaptureRequest::Complex(_) => {}, // Expected
            _ => panic!("Partial capture should generate complex request"),
        }
    }
    
    #[test]
    fn test_conditional_fields_serialization() {
        let full_capture_data = create_full_capture_router_data();
        let partial_capture_data = create_partial_capture_router_data();
        
        let full_request = ConditionalCaptureRequest::try_from(full_capture_data).unwrap();
        let partial_request = ConditionalCaptureRequest::try_from(partial_capture_data).unwrap();
        
        let full_json = serde_json::to_value(&full_request).unwrap();
        let partial_json = serde_json::to_value(&partial_request).unwrap();
        
        // Full capture should have minimal fields
        assert!(full_json.get("amount").is_none());
        assert!(full_json.get("currency").is_none());
        
        // Partial capture should have all fields
        assert!(partial_json.get("amount").is_some());
        assert!(partial_json.get("currency").is_some());
    }
    
    #[test] 
    fn test_dynamic_request_builder() {
        let full_capture_data = create_full_capture_router_data();
        let request_value = Value::try_from(full_capture_data).unwrap();
        
        // Should only contain fields appropriate for full capture
        let obj = request_value.as_object().unwrap();
        assert!(!obj.contains_key("amount"));
        assert!(!obj.contains_key("sequence"));
    }
}
```

### Best Practices for Empty Request Bodies

1. **Clear Documentation**: Always document when empty bodies are expected vs complex bodies
2. **Validation**: Validate that the correct request structure is used for each endpoint
3. **Error Handling**: Provide clear error messages when wrong request format is used
4. **Testing**: Test both empty and complex request scenarios thoroughly
5. **Serialization**: Ensure your serialization produces exactly `{}` for empty requests
6. **Type Safety**: Use Rust's type system to prevent sending wrong request types to wrong endpoints

### Common Pitfalls to Avoid

- **Sending null instead of empty object**: `null` ≠ `{}`
- **Including optional fields with null values**: Some APIs reject null fields entirely
- **Wrong Content-Type**: Ensure `application/json` is used even for empty bodies
- **Endpoint confusion**: Sending complex body to simple endpoint or vice versa
- **Conditional logic errors**: Incorrectly determining when to use empty vs complex requests

## URL Endpoint Patterns

### Pattern 1: RESTful Transaction-Based URLs
Most modern connectors use RESTful patterns with transaction IDs in the URL path:

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.connector_transaction_id
        .as_ref()
        .ok_or(IntegrationError::MissingConnectorTransactionID)?;
    
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Adyen: "{base_url}/v68/payments/{transaction_id}/captures"
    // Braintree: "{base_url}/transactions/{transaction_id}/submit_for_settlement"
    // Xendit: "{base_url}/v2/credit_card_captures"
    
    Ok(format!("{base_url}/v1/payments/{transaction_id}/capture"))
}
```

### Pattern 2: Single Endpoint with Operation Type
Some connectors use the same endpoint for all operations, distinguishing by request body:

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    // Authorizedotnet, PayU style
    Ok(self.connector_base_url_payments(req).to_string())
}
```

### Pattern 3: Dedicated Capture Endpoints
Some connectors have specific capture-only endpoints:

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Razorpay: "{base_url}/v1/payments/{payment_id}/capture"
    // Fiuu: "{base_url}/capture"
    
    Ok(format!("{base_url}/capture"))
}
```

### Pattern 4: Conditional Multi-Endpoint Selection
Advanced pattern for APIs with different endpoints based on capture type or conditions:

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Determine endpoint based on capture characteristics
    let endpoint = determine_capture_endpoint(&req.request)?;
    
    match endpoint {
        CaptureEndpoint::FullSettlement => {
            // Full settlement endpoint - typically requires empty body
            Ok(format!("{}/api/payments/{}/settlements", base_url, transaction_id))
        },
        CaptureEndpoint::PartialSettlement => {
            // Partial settlement endpoint - typically requires complex body
            Ok(format!("{}/api/payments/{}/partialSettlements", base_url, transaction_id))
        },
        CaptureEndpoint::ImmediateCapture => {
            // Immediate capture endpoint
            Ok(format!("{}/api/payments/{}/capture", base_url, transaction_id))
        },
        CaptureEndpoint::BatchCapture => {
            // Batch capture endpoint
            Ok(format!("{}/api/batch/captures", base_url))
        }
    }
}

#[derive(Debug, Clone)]
enum CaptureEndpoint {
    FullSettlement,
    PartialSettlement,
    ImmediateCapture,
    BatchCapture,
}

fn determine_capture_endpoint(request: &PaymentsCaptureData) -> Result<CaptureEndpoint, IntegrationError> {
    // Logic to determine the appropriate endpoint
    let is_full_capture = request.amount_to_capture.is_none() || 
        request.amount_to_capture == Some(request.payment_amount);
    
    // Check for batch processing requirements
    if should_use_batch_processing(request) {
        return Ok(CaptureEndpoint::BatchCapture);
    }
    
    // Check for immediate vs settlement based on timing
    if requires_immediate_capture(request) {
        return Ok(CaptureEndpoint::ImmediateCapture);
    }
    
    // Settlement-based capture (most common for manual captures)
    if is_full_capture {
        Ok(CaptureEndpoint::FullSettlement)
    } else {
        Ok(CaptureEndpoint::PartialSettlement)
    }
}

fn should_use_batch_processing(request: &PaymentsCaptureData) -> bool {
    // Implementation-specific logic for batch detection
    // Could be based on merchant settings, amount thresholds, etc.
    false // Default implementation
}

fn requires_immediate_capture(request: &PaymentsCaptureData) -> bool {
    // Implementation-specific logic for immediate capture detection
    // Could be based on payment method, urgency flags, etc.
    false // Default implementation
}
```

## ⚙️ Conditional Implementation Patterns

**Advanced patterns for implementing capture flows with complex conditional logic**

These patterns address scenarios where capture behavior must change based on various conditions like amount, timing, merchant configuration, or API capabilities.

### Pattern 1: Conditional Request Structure Selection

Use when the same connector requires different request structures based on capture conditions.

**Implementation:**
```rust
// Enum to represent different request types
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ConditionalCaptureRequest {
    SimpleCapture(SimpleCaptureRequest),
    ComplexCapture(ComplexCaptureRequest),
    EmptyCapture,
}

#[derive(Debug, Serialize)]
pub struct SimpleCaptureRequest {
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct ComplexCaptureRequest {
    pub amount: i64,
    pub currency: String,
    pub reference: String,
    pub sequence: CaptureSequence,
    pub metadata: HashMap<String, String>,
}

impl TryFrom<CaptureRouterData> for ConditionalCaptureRequest {
    type Error = error_stack::Report<IntegrationError>;
    
    fn try_from(item: CaptureRouterData) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request_context = analyze_capture_context(&router_data)?;
        
        match request_context.complexity_level {
            ComplexityLevel::Empty => {
                // Full capture with empty body
                Ok(ConditionalCaptureRequest::EmptyCapture)
            },
            ComplexityLevel::Simple => {
                // Basic capture with amount and currency
                Ok(ConditionalCaptureRequest::SimpleCapture(SimpleCaptureRequest {
                    amount: item.amount.get_amount_as_i64(),
                    currency: router_data.request.currency.to_string(),
                }))
            },
            ComplexityLevel::Complex => {
                // Advanced capture with full metadata
                Ok(ConditionalCaptureRequest::ComplexCapture(ComplexCaptureRequest {
                    amount: item.amount.get_amount_as_i64(),
                    currency: router_data.request.currency.to_string(),
                    reference: generate_capture_reference(&router_data),
                    sequence: calculate_capture_sequence(&router_data)?,
                    metadata: extract_capture_metadata(&router_data),
                }))
            }
        }
    }
}

#[derive(Debug)]
struct CaptureContext {
    complexity_level: ComplexityLevel,
    requires_sequence: bool,
    supports_metadata: bool,
    is_partial_capture: bool,
}

#[derive(Debug, PartialEq)]
enum ComplexityLevel {
    Empty,
    Simple,
    Complex,
}

fn analyze_capture_context(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> Result<CaptureContext, IntegrationError> {
    let is_partial = router_data.request.amount_to_capture.is_some() &&
        router_data.request.amount_to_capture != Some(router_data.request.payment_amount);
    
    let complexity_level = if !is_partial && supports_empty_body_capture(&router_data) {
        ComplexityLevel::Empty
    } else if is_simple_capture_scenario(&router_data) {
        ComplexityLevel::Simple
    } else {
        ComplexityLevel::Complex
    };
    
    Ok(CaptureContext {
        complexity_level,
        requires_sequence: is_partial && supports_sequence_tracking(&router_data),
        supports_metadata: supports_metadata_fields(&router_data),
        is_partial_capture: is_partial,
    })
}

fn supports_empty_body_capture(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> bool {
    // Check if connector/merchant supports empty body captures
    // This could be based on API version, merchant configuration, etc.
    true // Default implementation
}

fn is_simple_capture_scenario(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> bool {
    // Determine if this is a simple capture scenario
    // Could be based on amount, merchant type, payment method, etc.
    router_data.request.payment_amount.get_amount_as_i64() < 10000 // Example: amounts under $100
}

fn supports_sequence_tracking(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> bool {
    // Check if this merchant/API version supports sequence tracking
    true // Default implementation
}

fn supports_metadata_fields(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> bool {
    // Check if metadata fields are supported
    true // Default implementation
}
```

### Pattern 2: Conditional URL and Request Coordination

Coordinate endpoint selection with request structure for maximum API compatibility.

**Implementation:**
```rust
// Coordinated endpoint and request selection
pub struct CaptureConfiguration {
    pub endpoint: CaptureEndpoint,
    pub request_type: RequestType,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum RequestType {
    Empty,
    Minimal,
    Standard,
    Extended,
}

impl CaptureConfiguration {
    pub fn determine(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> Result<Self, IntegrationError> {
        let capture_context = analyze_capture_context(router_data)?;
        
        let (endpoint, request_type) = match (
            capture_context.is_partial_capture,
            capture_context.complexity_level,
            get_api_capabilities(router_data)
        ) {
            // Full capture with settlement API
            (false, ComplexityLevel::Empty, ApiCapabilities::Settlement) => {
                (CaptureEndpoint::FullSettlement, RequestType::Empty)
            },
            // Partial capture with settlement API
            (true, _, ApiCapabilities::Settlement) => {
                (CaptureEndpoint::PartialSettlement, RequestType::Standard)
            },
            // Simple capture API
            (_, ComplexityLevel::Simple, ApiCapabilities::SimpleCapture) => {
                (CaptureEndpoint::ImmediateCapture, RequestType::Minimal)
            },
            // Complex capture scenarios
            (_, ComplexityLevel::Complex, _) => {
                (CaptureEndpoint::ImmediateCapture, RequestType::Extended)
            },
            // Default fallback
            _ => {
                (CaptureEndpoint::ImmediateCapture, RequestType::Standard)
            }
        };
        
        let headers = generate_headers_for_configuration(&endpoint, &request_type)?;
        
        Ok(CaptureConfiguration {
            endpoint,
            request_type,
            headers,
        })
    }
}

#[derive(Debug)]
enum ApiCapabilities {
    Settlement,
    SimpleCapture,
    AdvancedCapture,
}

fn get_api_capabilities(router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>) -> ApiCapabilities {
    // Determine API capabilities based on connector configuration
    // This could be from connector metadata, API version, etc.
    ApiCapabilities::Settlement // Default implementation
}

fn generate_headers_for_configuration(endpoint: &CaptureEndpoint, request_type: &RequestType) -> Result<Vec<(String, String)>, IntegrationError> {
    let mut headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
    ];
    
    // Add endpoint-specific headers
    match endpoint {
        CaptureEndpoint::FullSettlement | CaptureEndpoint::PartialSettlement => {
            headers.push(("X-Settlement-Mode".to_string(), "true".to_string()));
        },
        CaptureEndpoint::ImmediateCapture => {
            headers.push(("X-Capture-Mode".to_string(), "immediate".to_string()));
        },
        CaptureEndpoint::BatchCapture => {
            headers.push(("X-Batch-Processing".to_string(), "true".to_string()));
        }
    }
    
    // Add request-type-specific headers
    match request_type {
        RequestType::Empty => {
            headers.push(("X-Request-Type".to_string(), "empty".to_string()));
        },
        RequestType::Extended => {
            headers.push(("X-Extended-Fields".to_string(), "enabled".to_string()));
        },
        _ => {}
    }
    
    Ok(headers)
}

// Updated get_url function using configuration
fn get_url(
    &self,
    req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let config = CaptureConfiguration::determine(req)?;
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    match config.endpoint {
        CaptureEndpoint::FullSettlement => {
            Ok(format!("{}/api/payments/{}/settlements", base_url, transaction_id))
        },
        CaptureEndpoint::PartialSettlement => {
            Ok(format!("{}/api/payments/{}/partialSettlements", base_url, transaction_id))
        },
        CaptureEndpoint::ImmediateCapture => {
            Ok(format!("{}/api/payments/{}/capture", base_url, transaction_id))
        },
        CaptureEndpoint::BatchCapture => {
            Ok(format!("{}/api/batch/captures", base_url))
        }
    }
}
```

### Pattern 3: Dynamic Response Handling

Handle different response structures based on the configuration used for the request.

**Implementation:**
```rust
// Unified response handling for conditional implementations
impl TryFrom<ResponseRouterData<Value, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>> 
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData> 
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: ResponseRouterData<Value, RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    ) -> Result<Self, Self::Error> {
        let response_json = &item.response;
        let router_data = &item.router_data;
        
        // Determine which configuration was used based on request context
        let config = CaptureConfiguration::determine(router_data)?;
        
        // Parse response based on configuration
        let (status, transaction_id, reference) = match config.endpoint {
            CaptureEndpoint::FullSettlement => {
                parse_settlement_response(response_json)?
            },
            CaptureEndpoint::PartialSettlement => {
                parse_partial_settlement_response(response_json)?
            },
            CaptureEndpoint::ImmediateCapture => {
                parse_immediate_capture_response(response_json)?
            },
            CaptureEndpoint::BatchCapture => {
                parse_batch_capture_response(response_json)?
            }
        };
        
        let mut router_data = router_data.clone();
        router_data.resource_common_data.status = status;
        router_data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(transaction_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: reference,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });
        
        Ok(router_data)
    }
}

fn parse_settlement_response(response: &Value) -> Result<(AttemptStatus, String, Option<String>), IntegrationError> {
    let outcome = response.get("outcome")
        .and_then(|v| v.as_str())
        .ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;
    
    let status = match outcome {
        "sentForSettlement" => AttemptStatus::Charged,
        "refused" => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };
    
    let transaction_id = response
        .get("_links")
        .and_then(|links| links.get("self"))
        .and_then(|self_link| self_link.get("href"))
        .and_then(|href| href.as_str())
        .and_then(|href| href.split('/').last())
        .unwrap_or("unknown")
        .to_string();
    
    // Settlement responses typically don't have transaction reference
    Ok((status, transaction_id, None))
}

fn parse_partial_settlement_response(response: &Value) -> Result<(AttemptStatus, String, Option<String>), IntegrationError> {
    let outcome = response.get("outcome")
        .and_then(|v| v.as_str())
        .ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;
    
    let status = match outcome {
        "sentForSettlement" => AttemptStatus::PartialCharged,
        "refused" => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };
    
    let transaction_id = response.get("transactionReference")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    
    let reference = Some(transaction_id.clone());
    
    Ok((status, transaction_id, reference))
}

fn parse_immediate_capture_response(response: &Value) -> Result<(AttemptStatus, String, Option<String>), IntegrationError> {
    let status_str = response.get("status")
        .and_then(|v| v.as_str())
        .ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;
    
    let status = match status_str {
        "captured" | "success" => AttemptStatus::Charged,
        "failed" | "error" => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };
    
    let transaction_id = response.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    
    let reference = response.get("reference")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    Ok((status, transaction_id, reference))
}

fn parse_batch_capture_response(response: &Value) -> Result<(AttemptStatus, String, Option<String>), IntegrationError> {
    let batch_status = response.get("batchStatus")
        .and_then(|v| v.as_str())
        .ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;
    
    let status = match batch_status {
        "processed" => AttemptStatus::Charged,
        "failed" => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };
    
    let batch_id = response.get("batchId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    
    Ok((status, batch_id, None))
}
```

### Pattern 4: Configuration-Based Testing

Test all conditional paths and configurations systematically.

**Implementation:**
```rust
#[cfg(test)]
mod conditional_implementation_tests {
    use super::*;
    
    #[test]
    fn test_configuration_determination() {
        let test_cases = vec![
            (create_full_capture_data(), CaptureEndpoint::FullSettlement, RequestType::Empty),
            (create_partial_capture_data(), CaptureEndpoint::PartialSettlement, RequestType::Standard),
            (create_simple_capture_data(), CaptureEndpoint::ImmediateCapture, RequestType::Minimal),
            (create_complex_capture_data(), CaptureEndpoint::ImmediateCapture, RequestType::Extended),
        ];
        
        for (router_data, expected_endpoint, expected_request_type) in test_cases {
            let config = CaptureConfiguration::determine(&router_data).unwrap();
            assert_eq!(config.endpoint, expected_endpoint);
            assert_eq!(config.request_type, expected_request_type);
        }
    }
    
    #[test]
    fn test_conditional_request_structures() {
        let configs = vec![
            (create_full_capture_data(), "ConditionalCaptureRequest::EmptyCapture"),
            (create_simple_capture_data(), "ConditionalCaptureRequest::SimpleCapture"),
            (create_complex_capture_data(), "ConditionalCaptureRequest::ComplexCapture"),
        ];
        
        for (router_data, expected_variant) in configs {
            let request = ConditionalCaptureRequest::try_from(router_data).unwrap();
            match (&request, expected_variant) {
                (ConditionalCaptureRequest::EmptyCapture, "ConditionalCaptureRequest::EmptyCapture") => {},
                (ConditionalCaptureRequest::SimpleCapture(_), "ConditionalCaptureRequest::SimpleCapture") => {},
                (ConditionalCaptureRequest::ComplexCapture(_), "ConditionalCaptureRequest::ComplexCapture") => {},
                _ => panic!("Unexpected request variant for test case"),
            }
        }
    }
    
    #[test]
    fn test_response_parsing_for_all_endpoints() {
        let test_responses = vec![
            (create_settlement_response(), CaptureEndpoint::FullSettlement),
            (create_partial_settlement_response(), CaptureEndpoint::PartialSettlement),
            (create_immediate_capture_response(), CaptureEndpoint::ImmediateCapture),
            (create_batch_capture_response(), CaptureEndpoint::BatchCapture),
        ];
        
        for (response_data, endpoint) in test_responses {
            // Test that each response type can be parsed correctly
            let (status, tx_id, reference) = match endpoint {
                CaptureEndpoint::FullSettlement => parse_settlement_response(&response_data).unwrap(),
                CaptureEndpoint::PartialSettlement => parse_partial_settlement_response(&response_data).unwrap(),
                CaptureEndpoint::ImmediateCapture => parse_immediate_capture_response(&response_data).unwrap(),
                CaptureEndpoint::BatchCapture => parse_batch_capture_response(&response_data).unwrap(),
            };
            
            // Validate that parsing produces reasonable results
            assert!(!tx_id.is_empty());
            assert!(matches!(status, AttemptStatus::Charged | AttemptStatus::PartialCharged | AttemptStatus::Pending | AttemptStatus::Failure));
        }
    }
}
```

## Amount Handling Patterns

### Pattern 1: Same Amount Format as Authorization
Most connectors use the same amount format for capture as authorization:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>> for {ConnectorName}CaptureRequest {
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: item.amount, // Use pre-converted amount
            // ...
        })
    }
}
```

### Pattern 2: Full vs Partial Capture Handling
Some connectors require special handling for partial captures:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>> for {ConnectorName}CaptureRequest {
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Handle partial vs full capture
        let capture_amount = match router_data.request.amount_to_capture {
            Some(amount) => amount, // Partial capture
            None => router_data.request.payment_amount, // Full capture
        };
        
        Ok(Self {
            amount: item.amount, // Already converted
            is_partial_capture: router_data.request.amount_to_capture.is_some(),
            // ...
        })
    }
}
```

### Pattern 3: Currency Validation
Some connectors validate currency consistency between auth and capture:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>> for {ConnectorName}CaptureRequest {
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Validate currency matches original authorization
        if let Some(original_currency) = &router_data.request.currency {
            if original_currency != &router_data.request.currency {
                return Err(IntegrationError::InvalidRequestData {
                    message: "Capture currency must match authorization currency".to_string(),
                }.into());
            }
        }
        
        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            // ...
        })
    }
}
```

## Error Handling Patterns

### Pattern 1: Missing Transaction ID Validation
All capture implementations must validate the presence of connector_transaction_id:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>> for {ConnectorName}CaptureRequest {
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        let transaction_id = router_data
            .request
            .connector_transaction_id
            .as_ref()
            .ok_or_else(|| {
                IntegrationError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                , context: Default::default() }
            })?;
        
        // Continue with request building...
    }
}
```

### Pattern 2: Capture-Specific Error Responses
Map capture-specific error codes to appropriate attempt statuses:

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}CaptureResponse, ...>> for RouterDataV2<Capture, ...> {
    fn try_from(item: ResponseRouterData<{ConnectorName}CaptureResponse, ...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        
        // Handle capture-specific errors
        if let Some(error_code) = &response.error_code {
            let attempt_status = match error_code.as_str() {
                "ALREADY_CAPTURED" => common_enums::AttemptStatus::Charged,
                "INSUFFICIENT_AUTHORIZATION" => common_enums::AttemptStatus::PartialCharged,
                "EXPIRED_AUTHORIZATION" => common_enums::AttemptStatus::Failure,
                "INVALID_TRANSACTION_STATE" => common_enums::AttemptStatus::Failure,
                _ => common_enums::AttemptStatus::Failure,
            };
            
            return Ok(Self {
                response: Err(ErrorResponse {
                    attempt_status: Some(attempt_status),
                    // ... other error fields
                }),
                // ...
            });
        }
        
        // Handle success case...
    }
}
```

### Pattern 3: Idempotency Handling
Some connectors support idempotent captures:

```rust
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<Capture, ...>>> for {ConnectorName}CaptureRequest {
    fn try_from(item: {ConnectorName}RouterData<...>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        Ok(Self {
            // Use a deterministic idempotency key
            idempotency_key: Some(format!(
                "capture-{}-{}", 
                router_data.request.connector_transaction_id.as_ref().unwrap(),
                router_data.resource_common_data.connector_request_reference_id
            )),
            // ... other fields
        })
    }
}
```

## Testing Patterns

### Unit Test Structure for Capture Flow

```rust
#[cfg(test)]
mod capture_tests {
    use super::*;
    use common_enums::{Currency, AttemptStatus};
    use domain_types::connector_types::PaymentFlowData;

    #[test]
    fn test_capture_request_transformation() {
        // Test capture request transformation
        let router_data = create_test_capture_router_data();
        let connector_req = {ConnectorName}CaptureRequest::try_from(&router_data);
        
        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert_eq!(req.amount, MinorUnit::new(1000));
        assert_eq!(req.currency, "USD");
        assert!(req.transaction_id.is_some());
    }

    #[test]
    fn test_capture_response_transformation_success() {
        // Test successful capture response
        let response = {ConnectorName}CaptureResponse {
            id: "capture_123".to_string(),
            status: {ConnectorName}CaptureStatus::Succeeded,
            amount: Some(MinorUnit::new(1000)),
            payment_id: Some("payment_456".to_string()),
            reference: Some("test_ref".to_string()),
            error: None,
            error_code: None,
        };

        let router_data = create_test_capture_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());
        
        let router_data_result = result.unwrap();
        assert_eq!(router_data_result.resource_common_data.status, AttemptStatus::Charged);
    }

    #[test]
    fn test_capture_response_transformation_failure() {
        // Test failed capture response
        let response = {ConnectorName}CaptureResponse {
            id: "capture_789".to_string(),
            status: {ConnectorName}CaptureStatus::Failed,
            amount: None,
            payment_id: Some("payment_456".to_string()),
            reference: None,
            error: Some("Insufficient authorization amount".to_string()),
            error_code: Some("INSUFFICIENT_AUTHORIZATION".to_string()),
        };

        let router_data = create_test_capture_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 400,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());
        
        let router_data_result = result.unwrap();
        assert_eq!(router_data_result.resource_common_data.status, AttemptStatus::Failure);
        assert!(router_data_result.response.is_err());
    }

    #[test]
    fn test_missing_transaction_id_error() {
        // Test error when connector_transaction_id is missing
        let mut router_data = create_test_capture_router_data();
        router_data.request.connector_transaction_id = None;

        let connector_req = {ConnectorName}CaptureRequest::try_from(&router_data);
        assert!(connector_req.is_err());
    }

    #[test]
    fn test_capture_url_construction() {
        // Test URL construction with transaction ID
        let connector = {ConnectorName}::new();
        let router_data = create_test_capture_router_data();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains(&router_data.request.connector_transaction_id.unwrap()));
    }

    fn create_test_capture_router_data() -> RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData> {
        // Create test router data structure for capture
        RouterDataV2 {
            resource_common_data: PaymentFlowData {
                // ... test data
            },
            request: PaymentsCaptureData {
                connector_transaction_id: Some("test_txn_123".to_string()),
                currency: Currency::USD,
                payment_amount: MinorUnit::new(1000),
                amount_to_capture: Some(MinorUnit::new(1000)), // Full capture
                // ... other test fields
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                // ... response data
            }),
            // ... other router data fields
        }
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod capture_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_capture_flow_integration() {
        let connector = {ConnectorName}::new();
        
        // Mock capture request data
        let request_data = create_test_capture_request();
        
        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".into())));
        
        // Test URL generation
        let url = connector.get_url(&request_data).unwrap();
        assert!(url.contains("capture") || url.contains(&request_data.request.connector_transaction_id.unwrap()));
        
        // Test request body generation
        let request_body = connector.get_request_body(&request_data).unwrap();
        assert!(request_body.is_some());
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

**⚠️ CRITICAL: Complete API Analysis Methodology before implementation**

- [ ] **API Analysis Methodology (MANDATORY)**
  - [ ] **Step 1: OpenAPI/Documentation Deep Dive**
    - [ ] Search for all capture-related endpoints (`grep -i "capture\|settle\|settlement"`)
    - [ ] Document each endpoint's purpose (full vs partial capture)
    - [ ] Compare request schemas between different endpoints
    - [ ] Compare response schemas between different endpoints
    - [ ] Identify settlement vs capture terminology differences
  - [ ] **Step 2: API Pattern Classification**
    - [ ] Classify as Pattern A (Single Endpoint), B (Dual Endpoint), or C (Multiple Endpoints)
    - [ ] Determine if endpoints require different request structures
    - [ ] Check for empty request body requirements (`{}`)
    - [ ] Identify response schema variations between endpoints
  - [ ] **Step 3: Request/Response Analysis Checklist**
    - [ ] **Empty Request Body Support**: Does any endpoint require `{}` as request body?
    - [ ] **Conditional Fields**: Are certain fields required only for specific capture types?
    - [ ] **Amount Handling**: How does the API distinguish full vs partial captures?
    - [ ] **Transaction Reference**: Where does the original transaction ID go (URL path vs body)?
    - [ ] **Response Variations**: Do different endpoints return different response structures?
    - [ ] **Status Code Mapping**: What success/failure status codes does each endpoint return?
  - [ ] **Step 4: Implementation Strategy Selection**
    - [ ] Choose appropriate pattern (Single, Dual, or Multi-endpoint)
    - [ ] Plan conditional endpoint selection logic
    - [ ] Design request structure variations handling
    - [ ] Plan response schema variation handling
  - [ ] **Step 5: Pre-Implementation Validation**
    - [ ] **API Documentation Clarity**: Are all endpoint behaviors clearly documented?
    - [ ] **Test Scenarios Identified**: Do you know how to test each endpoint?
    - [ ] **Error Case Mapping**: Are error responses documented for each endpoint?
    - [ ] **Endpoint Selection Logic**: Is the logic for choosing endpoints clear and testable?

- [ ] **Traditional API Documentation Review** (after API Analysis)
  - [ ] Understand connector's capture API endpoints (all of them)
  - [ ] Review capture authentication requirements (usually same as auth)
  - [ ] Identify capture-specific required/optional fields for each endpoint
  - [ ] Understand capture error response formats for each endpoint
  - [ ] Review capture status codes and meanings for each endpoint
  - [ ] Check if partial capture is supported and which endpoint handles it
  - [ ] Verify transaction ID requirements for each endpoint
  - [ ] **NEW**: Document dual-endpoint patterns (if applicable)
  - [ ] **NEW**: Identify empty request body scenarios
  - [ ] **NEW**: Map response schema variations between endpoints

- [ ] **Enhanced Capture Flow Requirements**
  - [ ] Determine if capture uses same endpoint as other operations
  - [ ] Identify URL pattern for capture (RESTful vs single endpoint vs dual endpoint)
  - [ ] Check if transaction ID goes in URL path or request body
  - [ ] Verify amount format requirements for capture
  - [ ] Review capture response structure(s) - may be multiple
  - [ ] Check for capture-specific error codes for each endpoint
  - [ ] **NEW**: Determine if full and partial captures use different endpoints
  - [ ] **NEW**: Identify conditional endpoint selection criteria
  - [ ] **NEW**: Plan for empty vs complex request body handling
  - [ ] **NEW**: Design response parsing strategy for different endpoint responses

### Implementation Checklist

- [ ] **Main Connector File Updates**
  - [ ] Add `Capture` to connector_flow imports
  - [ ] Add `PaymentsCaptureData` to connector_types imports
  - [ ] Import capture request/response types from transformers
  - [ ] Implement `PaymentCapture` trait
  - [ ] Add Capture flow to `macros::create_all_prerequisites!`
  - [ ] Implement capture flow with `macros::macro_connector_implementation!`
  - [ ] Add Source Verification stub for Capture flow

- [ ] **Transformers Implementation**
  - [ ] Add `Capture` to connector_flow imports
  - [ ] Add `PaymentsCaptureData` to connector_types imports
  - [ ] Create capture request structure
  - [ ] Create capture response structure
  - [ ] Create capture status enumeration
  - [ ] Implement status mapping for capture responses
  - [ ] Implement capture request transformation (`TryFrom`)
  - [ ] Implement capture response transformation (`TryFrom`)
  - [ ] Add transaction ID validation
  - [ ] Handle capture-specific error cases

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test capture request transformation
  - [ ] Test capture response transformation (success)
  - [ ] Test capture response transformation (failure)
  - [ ] Test missing transaction ID error handling
  - [ ] Test capture URL construction
  - [ ] Test capture status mapping
  - [ ] Test capture-specific error codes

- [ ] **Integration Tests**
  - [ ] Test capture headers generation
  - [ ] Test capture URL construction
  - [ ] Test capture request body generation
  - [ ] Test complete capture flow
  - [ ] Test partial capture scenarios (if supported)

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all capture-related errors
  - [ ] Run `cargo test` and ensure capture tests pass
  - [ ] Run `cargo clippy` and fix capture-related warnings
  - [ ] Verify capture flow compiles with macro framework

- [ ] **Functionality Validation**
  - [ ] Test capture with sandbox/test credentials
  - [ ] Verify successful capture processing
  - [ ] Verify capture error handling works correctly
  - [ ] Test partial capture scenarios (if supported)
  - [ ] Verify capture status mapping is correct
  - [ ] Test capture idempotency (if supported)

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Add comprehensive doc comments for capture structures
  - [ ] Document capture-specific requirements or limitations
  - [ ] Add usage examples for capture flow
  - [ ] Document partial capture support (if any)

- [ ] **Integration Documentation**
  - [ ] Document capture endpoint URL pattern
  - [ ] Document capture request/response format
  - [ ] Document capture-specific error codes
  - [ ] Document capture amount handling
  - [ ] Document any capture flow limitations

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM FOR CAPTURE FLOWS**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal`, `NewPayment` | **Always required** - Used in struct names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal`, `new_payment` | **Always required** - Used in config keys |
| `{AmountType}` | Amount type (same as auth flow) | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **Must match auth flow** |
| `{content_type}` | Request content type | `"application/json"`, `"application/x-www-form-urlencoded"` | **Same as auth flow** |
| `{capture_endpoint}` | Capture API endpoint path | `"v1/payments/{id}/capture"`, `"captures"`, `"submit_for_settlement"` | **From API docs** |
| `{auth_type}` | Authentication type | `HeaderKey`, `SignatureKey`, `BodyKey` | **Same as auth flow** |

### Capture Endpoint Selection Guide

Choose the right endpoint pattern based on your connector's API:

| API Style | Endpoint Pattern | Example |
|-----------|------------------|---------|
| RESTful with transaction ID | `"v1/payments/{id}/capture"` | Adyen, Stripe-style |
| RESTful with separate endpoint | `"v1/captures"` | Some modern APIs |
| Same endpoint as auth | `""` (empty, uses base URL) | Authorizedotnet, PayU |
| Dedicated capture path | `"capture"` | Simple capture-only APIs |

### Real-World Examples

**Example 1: Modern REST API (Adyen-style)**
```bash
{ConnectorName} → MyPayment
{connector_name} → my_payment
{AmountType} → MinorUnit
{content_type} → "application/json"
{capture_endpoint} → "v68/payments/{id}/captures"
URL Pattern: {base_url}/v68/payments/{transaction_id}/captures
```

**Example 2: Transaction Wrapper API (Authorizedotnet-style)**
```bash
{ConnectorName} → LegacyBank
{connector_name} → legacy_bank
{AmountType} → StringMajorUnit
{content_type} → "application/json"
{capture_endpoint} → "" (uses base URL)
URL Pattern: {base_url}
Request: Wrapped with transaction_type: "PriorAuthCaptureTransaction"
```

**Example 3: Simple Capture API**
```bash
{ConnectorName} → SimplePay
{connector_name} → simple_pay
{AmountType} → StringMinorUnit
{content_type} → "application/json"
{capture_endpoint} → "capture"
URL Pattern: {base_url}/capture
```

## Best Practices

### 🔥 Critical Lessons from Real-World Issues

**The Worldpay Dual-Endpoint Issue taught us:**

11. **ALWAYS Complete API Analysis First**: The #1 cause of capture implementation issues is incomplete API analysis. Always use the API Analysis Methodology before writing any code.
12. **Never Assume Single Endpoint**: Many modern APIs use different endpoints for full vs partial captures. Look for `/settlements` vs `/partialSettlements` patterns.
13. **Verify Empty Request Body Requirements**: Some APIs require exactly `{}` for full captures. Test this explicitly.
4. **Response Schema Variations Are Common**: Different endpoints often return different response structures. Plan for this from the start.
15. **OpenAPI Analysis is Critical**: Use `grep -i "capture\|settle\|settlement"` on OpenAPI specs to find all relevant endpoints before implementation.

### Traditional Best Practices

1. **Reuse Authorization Patterns**: Capture flows should follow the same authentication, headers, and base patterns as authorization flows
2. **Transaction ID Validation**: Always validate the presence of `connector_transaction_id` before building capture requests (required by API spec)
3. **Amount Consistency**: Use the same amount format and validation as the authorization flow
4. **Error Mapping**: Map capture-specific error codes to appropriate attempt statuses
5. **URL Construction**: Choose URL patterns that match the connector's REST API style
6. **Partial Capture Support**: Handle partial captures if the connector supports them
7. **Idempotency**: Implement idempotent captures when supported by the connector
8. **Status Mapping**: NEVER hardcode status - always map from connector response fields
9. **Testing**: Write comprehensive tests for both success and failure scenarios
10. **Documentation**: Document any capture-specific requirements or limitations
11. **Clean Request Structures**: Remove fields that would be hardcoded to None - keep structures minimal
12. **Purposeful Validations**: Only add validations required by connector API spec, with comments explaining WHY

### Enhanced Best Practices for Multi-Endpoint APIs

16. **Endpoint Selection Logic**: Implement clear, testable logic for choosing between different capture endpoints
17. **Request Structure Validation**: Ensure the correct request structure is used for each endpoint (empty vs complex)
18. **Response Parser Selection**: Use the appropriate response parser based on which endpoint was called
19. **Configuration-Based Implementation**: Use configuration objects to coordinate endpoint selection, request structure, and response parsing
20. **Generic Helper Functions**: Create reusable helper functions for common patterns like transaction ID extraction and status mapping

This pattern document provides a comprehensive template for implementing capture flows in payment connectors, ensuring consistency and completeness across all implementations while accommodating the diverse API styles found across different payment gateways.