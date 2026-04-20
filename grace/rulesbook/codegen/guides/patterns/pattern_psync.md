# Psync Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the Psync (Payment Sync) flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations across all 22 connectors in the connector service and can be consumed by AI to generate consistent, production-ready Psync flow code for any payment gateway.

<!-- PR #855 rename absorbed (commit c9e1025e3, 2026-04-02): `CreateSessionToken` →
`ServerSessionAuthenticationToken` in the connector import block.
See pattern_client_authentication_token.md for the full map. -->

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. For traditional Hyperswitch patterns, refer to legacy documentation.

## 🚀 Quick Start Guide

To implement a new connector Psync flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Select HTTP Method**: Choose between [GET Pattern](#get-based-psync-pattern) (most common) or [POST Pattern](#post-based-psync-pattern) based on your API
3. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
4. **Select Components**: Choose URL construction, authentication, and response parsing based on your connector's API
5. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector Psync Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{AmountType} → MinorUnit (if API uses integer cents)
{HttpMethod} → GET (if API uses RESTful status checking)
{psync_endpoint} → "v1/payments/{id}/status" (your status API endpoint)
{auth_type} → HeaderKey (if using Bearer token auth)
```

**✅ Result**: Complete, production-ready connector Psync flow implementation in ~15 minutes

## Table of Contents

1. [Overview](#overview)
2. [Psync Flow Implementation Analysis](#psync-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [GET-Based Psync Pattern](#get-based-psync-pattern)
5. [POST-Based Psync Pattern](#post-based-psync-pattern)
6. [URL Construction Patterns](#url-construction-patterns)
7. [Authentication Patterns](#authentication-patterns)
8. [Status Mapping Patterns](#status-mapping-patterns)
9. [Error Handling Patterns](#error-handling-patterns)
10. [Testing Patterns](#testing-patterns)
11. [Integration Checklist](#integration-checklist)

## Overview

The Psync (Payment Sync) flow is a critical payment processing flow that:
1. Receives payment status query requests from the router
2. Transforms them to connector-specific query format
3. Sends status requests to the payment gateway using transaction references
4. Processes status responses and maps payment states
5. Returns standardized payment status responses to the router

### Key Components:
- **Main Connector File**: Implements PSync trait and flow logic
- **Transformers File**: Handles Psync request/response data transformations
- **URL Construction**: Builds status query endpoint URLs (typically with transaction ID)
- **Authentication**: Manages API credentials (same as authorization flow)
- **Transaction ID Handling**: Extracts and uses connector transaction references
- **Status Mapping**: Converts connector payment statuses to standard statuses

## Psync Flow Implementation Analysis

Based on comprehensive analysis of all 22 connectors in the connector service, here's the implementation status:

### ✅ Full Psync Flow Implementation (20 connectors)
These connectors have complete Psync flow implementations with dedicated request/response structures:

**GET-Based Implementations (12 connectors):**
1. **Bluecode** - Simple GET with transaction ID in URL path
2. **Checkout** - RESTful GET with payment ID parameter
3. **Mifinity** - Complex URL with merchant and payment IDs
4. **Nexinets** - Hierarchical URL with order and transaction IDs
5. **Noon** - GET with reference ID in path
6. **Phonepe** - GET with custom headers for verification
7. **Razorpay** - Conditional URL patterns (payment vs order)
8. **Razorpayv2** - Enhanced version with improved response handling
9. **Volt** - OAuth-based GET with bearer token
10. **Xendit** - Simple GET with auto-capture detection

**POST-Based Implementations (8 connectors):**
1. **Adyen** - POST with redirect details and 3DS support
2. **Authorizedotnet** - POST with transaction wrapper and BOM handling
3. **Braintree** - GraphQL-based POST queries
4. **Elavon** - XML-to-form POST processing
5. **Fiserv** - JSON POST with HMAC signature authentication
6. **Fiuu** - Form-data POST with custom response parsing
7. **Novalnet** - JSON POST with redirect support
8. **Paytm** - Complex POST with AES signature encryption
9. **Payu** - Form-encoded POST with SHA-512 signatures

### 🔧 Stub/Trait Implementation Only (2 connectors)
These connectors implement the PSync trait but have empty/stub implementations:

1. **Cashfree** - Stub implementation only
2. **Cashtocode** - Trait implemented, no Psync flow

### 📊 Implementation Statistics
- **Complete implementations**: 20/22 (91%)
- **Stub implementations**: 2/22 (9%)
- **Most common pattern**: GET-based with transaction ID in URL (12/20, 60%)
- **Most common auth**: Basic Auth (6/20, 30%)
- **Most common amount format**: StringMajorUnit (8/20, 40%)

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### Main Connector File Pattern (Psync Flow Addition)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

// In the imports section, ensure PSync flow is included:
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, CreateOrder, ServerSessionAuthenticationToken, DefendDispute, PSync, RSync,
        Refund, RepeatPayment, SetupMandate, SubmitEvidence, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        // ... other imports
    },
};

// In transformers import, include Psync types:
use transformers::{
    {ConnectorName}AuthorizeRequest, {ConnectorName}AuthorizeResponse,
    {ConnectorName}SyncRequest, {ConnectorName}SyncResponse,
    {ConnectorName}ErrorResponse,
    // ... other types
};

// Implement PaymentSync trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for {ConnectorName}<T>
{
}

// Add PSync flow to the macro prerequisites
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
        // Same build_headers and connector_base_url functions as other flows
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

// Implement PSync flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // Choose appropriate request pattern:
    curl_request: Json({ConnectorName}SyncRequest), // For POST requests
    // OR
    // (no curl_request line for GET requests)
    curl_response: {ConnectorName}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: {HttpMethod}, // Get or Post
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract transaction ID from request
            let transaction_id = req.request.get_connector_transaction_id()
                .change_context(errors::IntegrationError::MissingConnectorTransactionID)?;
            
            let base_url = self.connector_base_url_payments(req);
            
            // Choose appropriate URL pattern based on connector API:
            // Pattern 1: RESTful with transaction ID in path (most common)
            Ok(format!("{base_url}/{psync_endpoint}", 
                base_url = base_url,
                psync_endpoint = "{psync_endpoint}".replace("{id}", &transaction_id)
            ))
            
            // Pattern 2: Query parameter based
            // Ok(format!("{base_url}/{endpoint}?transaction_id={transaction_id}"))
            
            // Pattern 3: Same endpoint as authorization
            // Ok(base_url.to_string())
        }
    }
);

// Add Source Verification stub for PSync flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for {ConnectorName}<T>
{
    // Stub implementation - will be replaced in Phase 10
}
```

### Transformers File Pattern (Psync Flow)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

// Add psync-specific imports to existing imports:
use domain_types::{
    connector_flow::{Authorize, PSync}, // Add PSync here
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, 
        PaymentsSyncData, ResponseId,
    },
    // ... other imports
};

// Psync Request Structure (for POST-based connectors)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}SyncRequest {
    // Common sync request fields:
    
    // Transaction reference (choose based on connector requirements)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,          // Direct transaction ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,              // Payment reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,                // Order reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_transaction_id: Option<String>, // Merchant reference
    
    // Merchant information (if required)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_account: Option<Secret<String>>, // For Adyen-style connectors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,              // For other connectors
    
    // Additional fields based on connector requirements
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_type: Option<String>,               // Type of status query
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<{ConnectorName}SyncDetails>, // Additional query details
}

// Alternative: Empty Request Structure (for GET-based connectors)
#[derive(Debug, Serialize)]
pub struct {ConnectorName}SyncRequest;

// Alternative: Wrapped Request Structure (like Authorizedotnet)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SyncRequestWrapper {
    pub get_transaction_details_request: {ConnectorName}SyncRequestInternal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SyncRequestInternal {
    pub merchant_authentication: {ConnectorName}AuthType,
    pub transaction_id: String,
}

// Psync Response Structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")] // Adjust based on connector API
pub struct {ConnectorName}SyncResponse {
    // Common response fields
    pub id: String,                           // Transaction ID
    pub status: {ConnectorName}PaymentStatus,
    pub amount: Option<{AmountType}>,
    
    // Reference fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,           // Original payment ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,            // Merchant reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,             // Order reference
    
    // Timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_at: Option<String>,
    
    // Additional connector-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    
    // Error information (for failed queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// Alternative: Response Union (like Razorpay)
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum {ConnectorName}SyncResponse {
    PaymentResponse({ConnectorName}PaymentResponse),
    OrderPaymentsCollection({ConnectorName}OrderPaymentsCollectionResponse),
    ErrorResponse({ConnectorName}ErrorResponse),
}

// Payment Status Enumeration
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")] // Adjust based on connector
pub enum {ConnectorName}PaymentStatus {
    // Common statuses across connectors
    Succeeded,
    Success,      // Alternative naming
    Completed,    // Alternative naming
    Captured,     // Alternative naming
    Charged,      // Alternative naming
    
    Failed,
    Error,        // Alternative naming
    Declined,     // Alternative naming
    
    Pending,
    Processing,   // Alternative naming
    InProgress,   // Alternative naming
    
    // Authentication states
    RequiresAction,
    AuthenticationRequired,
    ChallengePending,
    
    // Authorization states
    Authorized,
    PartiallyAuthorized,
    
    // Settlement states
    Settled,
    PartiallySettled,
    
    // Reversal states
    Cancelled,
    Voided,
    Refunded,
    PartiallyRefunded,
    
    // Connector-specific statuses
    Unknown,
}

// Status mapping for sync responses
impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            // Success statuses
            {ConnectorName}PaymentStatus::Succeeded
            | {ConnectorName}PaymentStatus::Success
            | {ConnectorName}PaymentStatus::Completed
            | {ConnectorName}PaymentStatus::Captured
            | {ConnectorName}PaymentStatus::Charged
            | {ConnectorName}PaymentStatus::Settled => Self::Charged,
            
            // Partial success statuses
            {ConnectorName}PaymentStatus::PartiallySettled
            | {ConnectorName}PaymentStatus::PartiallyRefunded => Self::PartialCharged,
            
            // Authorization statuses
            {ConnectorName}PaymentStatus::Authorized
            | {ConnectorName}PaymentStatus::PartiallyAuthorized => Self::Authorized,
            
            // Failure statuses
            {ConnectorName}PaymentStatus::Failed
            | {ConnectorName}PaymentStatus::Error
            | {ConnectorName}PaymentStatus::Declined => Self::Failure,
            
            // Pending statuses
            {ConnectorName}PaymentStatus::Pending
            | {ConnectorName}PaymentStatus::Processing
            | {ConnectorName}PaymentStatus::InProgress => Self::Pending,
            
            // Authentication statuses
            {ConnectorName}PaymentStatus::RequiresAction
            | {ConnectorName}PaymentStatus::AuthenticationRequired
            | {ConnectorName}PaymentStatus::ChallengePending => Self::AuthenticationPending,
            
            // Reversal statuses
            {ConnectorName}PaymentStatus::Cancelled
            | {ConnectorName}PaymentStatus::Voided => Self::Voided,
            
            {ConnectorName}PaymentStatus::Refunded => Self::Charged, // Successful refund
            
            // Unknown/default
            {ConnectorName}PaymentStatus::Unknown => Self::Pending,
        }
    }
}

// Request Transformation Implementation (for POST-based connectors)
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>>
    for {ConnectorName}SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Extract transaction ID - this is required for sync operations
        let transaction_id = router_data
            .request
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID)?;

        Ok(Self {
            transaction_id: Some(transaction_id),
            payment_id: router_data.request.connector_transaction_id.clone(),
            order_id: None, // Set if connector uses order-based queries
            merchant_transaction_id: Some(router_data.resource_common_data.connector_request_reference_id.clone()),
            merchant_account: None, // Set if required by connector
            merchant_id: None,      // Set if required by connector
            query_type: Some("payment_status".to_string()), // Connector-specific
            details: None,
        })
    }
}

// Alternative: Empty Request Transformation (for GET-based connectors)
impl TryFrom<{ConnectorName}RouterData<RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>>
    for {ConnectorName}SyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        // Empty request for GET-based sync
        Ok(Self)
    }
}

// Response Transformation Implementation
impl TryFrom<ResponseRouterData<{ConnectorName}SyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SyncResponse, RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // BEST PRACTICE: Map status from connector's operation_result/status field using From trait
        // NEVER assume status based on HTTP code alone
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

        // BEST PRACTICE: Build clean response transformers with proper field mapping
        // Map all fields from connector response - never hardcode to None
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

// Helper struct for router data transformation (same as other flows)
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

## GET-Based Psync Pattern

GET-based Psync is the most common pattern (used by 12/20 implemented connectors). It's simple, cacheable, and follows RESTful principles.

### When to Use GET Pattern
- Connector provides RESTful status endpoints
- No sensitive data needs to be sent in request body
- Status checking is idempotent and safe
- URL length limits are not a concern

### GET Pattern Implementation

```rust
// Main connector file - GET implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    // No curl_request for GET - body is empty
    curl_response: {ConnectorName}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get, // Specify GET method
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // GET requests typically don't need Content-Type
            let mut header = vec![];
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
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
            
            // Choose appropriate GET URL pattern:
            match "{url_pattern}" {
                "rest_with_id" => {
                    // Most common: /payments/{id} or /payments/{id}/status
                    Ok(format!("{}/v1/payments/{}/status", base_url, transaction_id))
                },
                "query_parameter" => {
                    // Alternative: /status?payment_id={id}
                    Ok(format!("{}/v1/status?payment_id={}", base_url, transaction_id))
                },
                "hierarchical" => {
                    // Complex: /orders/{order_id}/transactions/{txn_id}
                    let order_id = req.request.connector_request_reference_id.clone();
                    Ok(format!("{}/v1/orders/{}/transactions/{}", base_url, order_id, transaction_id))
                },
                _ => {
                    // Default pattern
                    Ok(format!("{}/payments/{}", base_url, transaction_id))
                }
            }
        }
    }
);

// Transformers - Empty request structure for GET
#[derive(Debug, Serialize)]
pub struct {ConnectorName}SyncRequest;

impl TryFrom<{ConnectorName}RouterData<RouterDataV2<PSync, ...>>> for {ConnectorName}SyncRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: {ConnectorName}RouterData<RouterDataV2<PSync, ...>>,
    ) -> Result<Self, Self::Error> {
        // No request body needed for GET
        Ok(Self)
    }
}
```

### GET Pattern Examples

**Simple RESTful Pattern (Checkout-style):**
```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    Ok(format!("{}/payments/{}", self.connector_base_url_payments(req), transaction_id))
}
```

**Status Endpoint Pattern (Bluecode-style):**
```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    Ok(format!("{}/api/v1/order/{}/status", self.connector_base_url_payments(req), transaction_id))
}
```

**Complex Path Pattern (Nexinets-style):**
```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let order_id = req.request.connector_request_reference_id.clone();
    Ok(format!("{}/orders/{}/transactions/{}", 
        self.connector_base_url_payments(req), order_id, transaction_id))
}
```

## POST-Based Psync Pattern

POST-based Psync is used when the API requires complex queries, authentication in the request body, or doesn't support RESTful GET endpoints.

### When to Use POST Pattern
- Connector requires complex query parameters
- Authentication must be in request body
- Sensitive data needs to be sent securely
- API doesn't support RESTful GET endpoints
- Query requires multiple parameters or filters

### POST Pattern Implementation

```rust
// Main connector file - POST implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}SyncRequest), // Specify JSON request body
    curl_response: {ConnectorName}SyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post, // Specify POST method
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
            let mut auth_header = self.get_auth_header(&req.connector_auth_type)?;
            header.append(&mut auth_header);
            Ok(header)
        }
        
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            
            // Choose appropriate POST URL pattern:
            match "{post_pattern}" {
                "inquiry_endpoint" => {
                    // Dedicated inquiry endpoint
                    Ok(format!("{}/v1/transaction-inquiry", base_url))
                },
                "status_endpoint" => {
                    // General status endpoint
                    Ok(format!("{}/v1/payment-status", base_url))
                },
                "same_as_auth" => {
                    // Same endpoint as authorization (operation determined by request body)
                    Ok(base_url.to_string())
                },
                _ => {
                    // Default pattern
                    Ok(format!("{}/sync", base_url))
                }
            }
        }
    }
);

// Transformers - Complex request structure for POST
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SyncRequest {
    pub merchant_authentication: {ConnectorName}AuthType,
    pub transaction_id: String,
    pub query_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data: Option<HashMap<String, String>>,
}

impl TryFrom<{ConnectorName}RouterData<RouterDataV2<PSync, ...>>> for {ConnectorName}SyncRequest {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<PSync, ...>>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        let transaction_id = router_data.request.get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID)?;

        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        Ok(Self {
            merchant_authentication: auth,
            transaction_id,
            query_type: "payment_status".to_string(),
            additional_data: None,
        })
    }
}
```

### POST Pattern Examples

**Transaction Inquiry Pattern (Fiserv-style):**
```rust
#[derive(Debug, Serialize)]
pub struct FiservSyncRequest {
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/ch/payments/v1/transaction-inquiry", 
        self.connector_base_url_payments(req)))
}
```

**Wrapped Request Pattern (Authorizedotnet-style):**
```rust
#[derive(Debug, Serialize)]
pub struct AuthorizedotnetSyncRequest {
    pub get_transaction_details_request: TransactionDetails,
}

fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    // Same endpoint as authorization
    Ok(self.connector_base_url_payments(req).to_string())
}
```

**GraphQL Pattern (Braintree-style):**
```rust
pub type BraintreeSyncRequest = GenericBraintreeRequest<PSyncInput>;

#[derive(Debug, Serialize)]
pub struct PSyncInput {
    pub query: String,
    pub variables: TransactionSearchInput,
}

fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    // GraphQL endpoint
    Ok(self.connector_base_url_payments(req).to_string())
}
```

## URL Construction Patterns

### Pattern 1: RESTful Resource Pattern (Most Common)

Used by connectors that follow REST principles with transaction IDs as path parameters.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Checkout: "{base_url}/payments/{id}"
    // Volt: "{base_url}/payments/{id}"
    // Xendit: "{base_url}/payment_requests/{id}"
    
    Ok(format!("{}/payments/{}", base_url, transaction_id))
}
```

### Pattern 2: Status Endpoint Pattern

Used by connectors with dedicated status checking endpoints.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Bluecode: "{base_url}/api/v1/order/{id}/status"
    // PhonePe: "{base_url}/status/{merchant_id}/{transaction_id}"
    
    Ok(format!("{}/api/v1/order/{}/status", base_url, transaction_id))
}
```

### Pattern 3: Hierarchical Resource Pattern

Used by connectors with hierarchical resource structures.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let order_id = extract_order_id(&req.request)?;
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Nexinets: "{base_url}/orders/{order_id}/transactions/{transaction_id}"
    // Noon: "{base_url}/payment/v1/order/getbyreference/{reference_id}"
    
    Ok(format!("{}/orders/{}/transactions/{}", base_url, order_id, transaction_id))
}

fn extract_order_id(request: &PaymentsSyncData) -> Result<String, IntegrationError> {
    // Extract order ID from connector metadata or request reference
    request.connector_request_reference_id.clone()
        .ok_or(IntegrationError::MissingRequiredField { field_name: "order_id" , context: Default::default() })
}
```

### Pattern 4: Query Parameter Pattern

Used by connectors that prefer query parameters over path parameters.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Custom API: "{base_url}/status?payment_id={id}"
    // Legacy API: "{base_url}/query?txn={id}&type=status"
    
    Ok(format!("{}/status?payment_id={}", base_url, transaction_id))
}
```

### Pattern 5: Complex Identifier Pattern

Used by connectors requiring multiple identifiers in the URL.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let transaction_id = req.request.get_connector_transaction_id()?;
    let merchant_id = extract_merchant_id(&req.connector_auth_type)?;
    let payment_id = extract_payment_id(&req.request)?;
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Mifinity: "{base_url}/api/gateway/payment-status/payment_validation_key_{merchant_id}_{payment_id}"
    // PhonePe: "{base_url}/status/{merchant_id}/{transaction_id}"
    
    Ok(format!("{}/api/gateway/payment-status/payment_validation_key_{}_{}", 
        base_url, merchant_id, payment_id))
}
```

### Pattern 6: Fixed Endpoint Pattern

Used by connectors with single endpoints that handle multiple operations.

```rust
fn get_url(&self, req: &RouterDataV2<PSync, ...>) -> CustomResult<String, IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    
    // Examples:
    // Authorizedotnet: Same endpoint for all operations
    // Paytm: "{base_url}/v3/order/status"
    // Fiserv: "{base_url}/ch/payments/v1/transaction-inquiry"
    
    Ok(format!("{}/v3/order/status", base_url))
}
```

### URL Construction Helper Functions

```rust
// Helper functions for URL construction
pub fn build_psync_url(
    base_url: &str,
    pattern: PsyncUrlPattern,
    identifiers: &PsyncIdentifiers,
) -> Result<String, IntegrationError> {
    match pattern {
        PsyncUrlPattern::RestfulResource => {
            Ok(format!("{}/payments/{}", base_url, identifiers.transaction_id))
        },
        PsyncUrlPattern::StatusEndpoint => {
            Ok(format!("{}/api/v1/order/{}/status", base_url, identifiers.transaction_id))
        },
        PsyncUrlPattern::Hierarchical => {
            let order_id = identifiers.order_id.as_ref()
                .ok_or(IntegrationError::MissingRequiredField { field_name: "order_id" , context: Default::default() })?;
            Ok(format!("{}/orders/{}/transactions/{}", 
                base_url, order_id, identifiers.transaction_id))
        },
        PsyncUrlPattern::QueryParameter => {
            Ok(format!("{}/status?payment_id={}", base_url, identifiers.transaction_id))
        },
        PsyncUrlPattern::FixedEndpoint => {
            Ok(format!("{}/transaction-inquiry", base_url))
        },
    }
}

#[derive(Debug)]
pub enum PsyncUrlPattern {
    RestfulResource,
    StatusEndpoint,
    Hierarchical,
    QueryParameter,
    FixedEndpoint,
}

#[derive(Debug)]
pub struct PsyncIdentifiers {
    pub transaction_id: String,
    pub order_id: Option<String>,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
}

impl PsyncIdentifiers {
    pub fn from_request(request: &PaymentsSyncData) -> Result<Self, IntegrationError> {
        let transaction_id = request.get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID)?;
        
        Ok(Self {
            transaction_id,
            order_id: Some(request.connector_request_reference_id.clone()),
            merchant_id: None, // Extract from auth if needed
            payment_id: None,  // Extract from metadata if needed
        })
    }
}
```

## Authentication Patterns

### Pattern 1: Basic Authentication (Most Common)

Used by 6 connectors including Razorpay, Braintree, and Nexinets.

```rust
impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = IntegrationError;
    
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// In get_auth_header:
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    
    Ok(vec![(
        "Authorization".to_string(),
        format!("Basic {}", 
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, 
                format!("{}:", auth.api_key.peek())
            )
        ).into_masked(),
    )])
}
```

### Pattern 2: Bearer Token Authentication

Used by connectors like Checkout and Volt.

```rust
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    
    Ok(vec![(
        "Authorization".to_string(),
        format!("Bearer {}", auth.api_token.peek()).into_masked(),
    )])
}
```

### Pattern 3: Custom Header Authentication

Used by connectors like Adyen, Mifinity, and PhonePe.

```rust
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    
    Ok(vec![
        ("X-API-Key".to_string(), auth.api_key.into_masked()),
        ("X-Merchant-ID".to_string(), auth.merchant_id.into_masked()),
    ])
}
```

### Pattern 4: Signature-Based Authentication

Used by enterprise connectors like Fiserv, Paytm, and PayU.

```rust
// HMAC Signature (Fiserv-style)
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let payload = ""; // Request body for signature
    
    let signature = generate_hmac_signature(&auth.api_secret, &timestamp, payload)?;
    
    Ok(vec![
        ("Api-Key".to_string(), auth.api_key.into_masked()),
        ("Timestamp".to_string(), timestamp.into()),
        ("Message-Signature".to_string(), signature.into_masked()),
    ])
}

fn generate_hmac_signature(secret: &str, timestamp: &str, payload: &str) -> Result<String, IntegrationError> {
    let message = format!("{}{}{}", auth.api_key, timestamp, payload);
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| IntegrationError::RequestEncodingFailed)?;
    mac.update(message.as_bytes());
    let result = mac.finalize();
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, result.into_bytes()))
}
```

### Pattern 5: No Authentication

Used by simple connectors like Elavon and Fiuu.

```rust
fn get_auth_header(&self, _auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    // No authentication headers needed
    Ok(vec![])
}
```

### Pattern 6: Request Body Authentication

Used by connectors like Authorizedotnet where auth goes in request body.

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}SyncRequest {
    pub merchant_authentication: MerchantAuthentication,
    pub transaction_id: String,
}

#[derive(Debug, Serialize)]
pub struct MerchantAuthentication {
    pub name: String,
    pub transaction_key: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for MerchantAuthentication {
    type Error = IntegrationError;
    
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::SignatureKey { api_key, api_secret, .. } => Ok(Self {
                name: api_key.peek().to_string(),
                transaction_key: api_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// No special auth headers needed
fn get_auth_header(&self, _auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    Ok(vec![])
}
```

## Status Mapping Patterns

**CRITICAL STATUS MAPPING BEST PRACTICES:**

1. **Always map status from connector's operation_result or equivalent field** - Never assume status based on HTTP code alone
2. **Use From trait implementation or explicit match statement** - Use the From trait pattern shown below for clean status mapping to AttemptStatus
3. **Never hardcode status values** - Always derive status from the connector's actual response field

### Pattern 1: Simple Enum Mapping (Most Common)

Direct mapping from connector status enum to standard AttemptStatus using From trait.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}PaymentStatus {
    Succeeded,
    Failed,
    Pending,
    Cancelled,
}

// BEST PRACTICE: Use From trait to map connector status to AttemptStatus
impl From<{ConnectorName}PaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Succeeded => Self::Charged,
            {ConnectorName}PaymentStatus::Failed => Self::Failure,
            {ConnectorName}PaymentStatus::Pending => Self::Pending,
            {ConnectorName}PaymentStatus::Cancelled => Self::Voided,
        }
    }
}
```

### Pattern 2: String-Based Status Mapping

Used when connector returns status as strings rather than structured enums.

```rust
pub fn map_status_string_to_attempt_status(status: &str) -> common_enums::AttemptStatus {
    match status.to_lowercase().as_str() {
        "success" | "succeeded" | "completed" | "captured" | "charged" => {
            common_enums::AttemptStatus::Charged
        },
        "failed" | "error" | "declined" | "rejected" => {
            common_enums::AttemptStatus::Failure
        },
        "pending" | "processing" | "in_progress" => {
            common_enums::AttemptStatus::Pending
        },
        "requires_action" | "authentication_required" | "challenge_pending" => {
            common_enums::AttemptStatus::AuthenticationPending
        },
        "authorized" | "approved" => {
            common_enums::AttemptStatus::Authorized
        },
        "cancelled" | "voided" | "canceled" => {
            common_enums::AttemptStatus::Voided
        },
        "refunded" | "settled" => {
            common_enums::AttemptStatus::Charged, // Successful final state
        },
        _ => common_enums::AttemptStatus::Pending, // Default for unknown statuses
    }
}

// Usage in response transformation
impl TryFrom<ResponseRouterData<{ConnectorName}SyncResponse, ...>> for RouterDataV2<PSync, ...> {
    fn try_from(item: ResponseRouterData<{ConnectorName}SyncResponse, ...>) -> Result<Self, Self::Error> {
        let status_string = &item.response.status;
        let status = map_status_string_to_attempt_status(status_string);
        
        // ... rest of transformation
    }
}
```

### Pattern 3: Code-Based Status Mapping

Used by connectors that return numeric or alphanumeric status codes.

```rust
pub fn map_status_code_to_attempt_status(code: &str) -> common_enums::AttemptStatus {
    match code {
        // Success codes
        "00" | "000" | "200" | "OK" => common_enums::AttemptStatus::Charged,
        
        // Pending codes
        "01" | "102" | "PENDING" => common_enums::AttemptStatus::Pending,
        
        // Authentication required codes
        "05" | "AUTH_REQUIRED" => common_enums::AttemptStatus::AuthenticationPending,
        
        // Authorization codes
        "10" | "AUTHORIZED" => common_enums::AttemptStatus::Authorized,
        
        // Failure codes
        "12" | "14" | "51" | "DECLINED" => common_enums::AttemptStatus::Failure,
        
        // Cancelled codes
        "99" | "CANCELLED" => common_enums::AttemptStatus::Voided,
        
        _ => {
            // Log unknown status code for debugging
            logger::warn!("Unknown status code: {}", code);
            common_enums::AttemptStatus::Pending
        }
    }
}
```

### Pattern 4: Boolean-Based Status Mapping

Used by simple connectors that return success/failure booleans.

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}SyncResponse {
    pub success: bool,
    pub status: Option<String>,
    pub error_code: Option<String>,
}

impl TryFrom<ResponseRouterData<{ConnectorName}SyncResponse, ...>> for RouterDataV2<PSync, ...> {
    fn try_from(item: ResponseRouterData<{ConnectorName}SyncResponse, ...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        
        let status = if response.success {
            // Further refinement based on additional status field
            match response.status.as_deref() {
                Some("authorized") => common_enums::AttemptStatus::Authorized,
                Some("settled") | Some("captured") => common_enums::AttemptStatus::Charged,
                _ => common_enums::AttemptStatus::Charged, // Default success
            }
        } else {
            // Check error code for more specific failure reason
            match response.error_code.as_deref() {
                Some("insufficient_funds") => common_enums::AttemptStatus::Failure,
                Some("authentication_failed") => common_enums::AttemptStatus::AuthenticationFailed,
                _ => common_enums::AttemptStatus::Failure, // Default failure
            }
        };
        
        // ... rest of transformation
    }
}
```

### Pattern 5: Contextual Status Mapping

Used when status depends on transaction type, amount, or other context.

```rust
pub fn map_contextual_status(
    status: &str,
    transaction_type: &str,
    amount: Option<i64>,
    original_amount: i64,
) -> common_enums::AttemptStatus {
    match (status.to_lowercase().as_str(), transaction_type.to_lowercase().as_str()) {
        ("completed", "authorization") => common_enums::AttemptStatus::Authorized,
        ("completed", "capture") => common_enums::AttemptStatus::Charged,
        ("completed", "settlement") => common_enums::AttemptStatus::Charged,
        ("completed", "refund") => common_enums::AttemptStatus::Charged, // Successful refund
        ("completed", "void") => common_enums::AttemptStatus::Voided,
        
        ("pending", _) => common_enums::AttemptStatus::Pending,
        ("failed", _) => common_enums::AttemptStatus::Failure,
        
        // Partial capture detection
        ("completed", "capture") if amount.is_some() && amount.unwrap() < original_amount => {
            common_enums::AttemptStatus::PartialCharged
        },
        
        _ => common_enums::AttemptStatus::Pending, // Safe default
    }
}
```

### Pattern 6: Multi-Response Status Mapping

Used by connectors like Razorpay that return different response types.

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum {ConnectorName}SyncResponse {
    PaymentResponse({ConnectorName}PaymentResponse),
    OrderPaymentsCollection({ConnectorName}OrderPaymentsCollectionResponse),
    ErrorResponse({ConnectorName}ErrorResponse),
}

impl TryFrom<ResponseRouterData<{ConnectorName}SyncResponse, ...>> for RouterDataV2<PSync, ...> {
    fn try_from(item: ResponseRouterData<{ConnectorName}SyncResponse, ...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        
        let status = match response {
            {ConnectorName}SyncResponse::PaymentResponse(payment) => {
                map_payment_status_to_attempt_status(&payment.status)
            },
            {ConnectorName}SyncResponse::OrderPaymentsCollection(order) => {
                // Take status from the first payment in the collection
                order.items.first()
                    .map(|payment| map_payment_status_to_attempt_status(&payment.status))
                    .unwrap_or(common_enums::AttemptStatus::Pending)
            },
            {ConnectorName}SyncResponse::ErrorResponse(_) => {
                common_enums::AttemptStatus::Failure
            },
        };
        
        // ... rest of transformation
    }
}
```

### Status Mapping Helper Functions

```rust
// Standardized status mapping interface
pub trait StatusMapper {
    type ConnectorStatus;
    
    fn map_to_attempt_status(&self, status: Self::ConnectorStatus) -> common_enums::AttemptStatus;
    fn is_final_status(&self, status: Self::ConnectorStatus) -> bool;
    fn is_success_status(&self, status: Self::ConnectorStatus) -> bool;
}

// Generic status mapper implementation
pub struct StandardStatusMapper;

impl StatusMapper for StandardStatusMapper {
    type ConnectorStatus = String;
    
    fn map_to_attempt_status(&self, status: String) -> common_enums::AttemptStatus {
        map_status_string_to_attempt_status(&status)
    }
    
    fn is_final_status(&self, status: String) -> bool {
        matches!(
            self.map_to_attempt_status(status),
            common_enums::AttemptStatus::Charged
            | common_enums::AttemptStatus::Failure
            | common_enums::AttemptStatus::Voided
            | common_enums::AttemptStatus::PartialCharged
        )
    }
    
    fn is_success_status(&self, status: String) -> bool {
        matches!(
            self.map_to_attempt_status(status),
            common_enums::AttemptStatus::Charged
            | common_enums::AttemptStatus::Authorized
            | common_enums::AttemptStatus::PartialCharged
        )
    }
}

// Status mapping with logging for debugging
pub fn map_status_with_logging(
    connector_name: &str,
    connector_status: &str,
    mapped_status: common_enums::AttemptStatus,
) -> common_enums::AttemptStatus {
    logger::debug!(
        "Status mapping for {}: '{}' -> {:?}",
        connector_name,
        connector_status,
        mapped_status
    );
    mapped_status
}
```

## Error Handling Patterns

### Pattern 1: Standard Error Response Structure

Most connectors use a basic error response with code and message fields.

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

impl Default for {ConnectorName}ErrorResponse {
    fn default() -> Self {
        Self {
            error_code: Some("UNKNOWN_ERROR".to_string()),
            error_message: Some("Unknown error occurred".to_string()),
            error_description: None,
            transaction_id: None,
        }
    }
}

// In ConnectorCommon implementation
fn build_error_response(
    &self,
    res: Response,
    event_builder: Option<&mut ConnectorEvent>,
) -> CustomResult<ErrorResponse, errors::ConnectorError> {
    let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
        {ConnectorName}ErrorResponse::default()
    } else {
        res.response
            .parse_struct("ErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?
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
        connector_transaction_id: response.transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    })
}
```

### Pattern 2: Unified Error Response (Multiple Formats)

Used when connector returns different error formats for different scenarios.

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum {ConnectorName}ErrorResponse {
    StandardError {
        error: {ConnectorName}Error,
    },
    SimpleError {
        message: String,
        code: Option<String>,
    },
    ValidationError {
        errors: Vec<{ConnectorName}FieldError>,
    },
    NetworkError {
        network_error: String,
        network_code: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}Error {
    pub code: String,
    pub message: String,
    pub description: Option<String>,
    pub transaction_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

impl Default for {ConnectorName}ErrorResponse {
    fn default() -> Self {
        Self::SimpleError {
            message: "Unknown error occurred".to_string(),
            code: Some("UNKNOWN_ERROR".to_string()),
        }
    }
}
```

### Pattern 3: Status Code Based Error Handling

Used when HTTP status codes provide error classification.

```rust
fn build_error_response(
    &self,
    res: Response,
    event_builder: Option<&mut ConnectorEvent>,
) -> CustomResult<ErrorResponse, errors::ConnectorError> {
    let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
        {ConnectorName}ErrorResponse::default()
    } else {
        res.response
            .parse_struct("ErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?
    };

    if let Some(i) = event_builder {
        i.set_error_response_body(&response);
    }

    // Map HTTP status codes to attempt statuses
    let attempt_status = match res.status_code {
        400 => Some(common_enums::AttemptStatus::Failure), // Bad Request
        401 => Some(common_enums::AttemptStatus::AuthenticationFailed), // Unauthorized
        402 => Some(common_enums::AttemptStatus::Failure), // Payment Required
        403 => Some(common_enums::AttemptStatus::AuthorizationFailed), // Forbidden
        404 => Some(common_enums::AttemptStatus::Failure), // Not Found (transaction not found)
        409 => Some(common_enums::AttemptStatus::Failure), // Conflict (duplicate transaction)
        422 => Some(common_enums::AttemptStatus::Failure), // Unprocessable Entity
        429 => Some(common_enums::AttemptStatus::Pending), // Rate Limited (retry)
        500..=599 => Some(common_enums::AttemptStatus::Pending), // Server errors (retry)
        _ => None,
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
```

### Pattern 4: Psync-Specific Error Handling

Used when sync operations have special error scenarios.

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}SyncResponse, ...>> for RouterDataV2<PSync, ...> {
    fn try_from(item: ResponseRouterData<{ConnectorName}SyncResponse, ...>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Handle sync-specific error cases
        if let Some(error_code) = &response.error_code {
            let attempt_status = match error_code.as_str() {
                "TRANSACTION_NOT_FOUND" => common_enums::AttemptStatus::Failure,
                "TRANSACTION_EXPIRED" => common_enums::AttemptStatus::Failure,
                "INSUFFICIENT_PERMISSIONS" => common_enums::AttemptStatus::AuthorizationFailed,
                "RATE_LIMIT_EXCEEDED" => common_enums::AttemptStatus::Pending, // Retry later
                "TEMPORARY_UNAVAILABLE" => common_enums::AttemptStatus::Pending, // Retry later
                _ => common_enums::AttemptStatus::Failure,
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: attempt_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    status_code: item.http_code,
                    code: error_code.clone(),
                    message: response.error_message.clone().unwrap_or_default(),
                    reason: response.error_description.clone(),
                    attempt_status: Some(attempt_status),
                    connector_transaction_id: Some(response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Continue with success case handling...
    }
}
```

### Pattern 5: Timeout and Network Error Handling

Used for handling network-related issues in sync operations.

```rust
fn handle_network_errors(
    res: Response,
    event_builder: Option<&mut ConnectorEvent>,
) -> CustomResult<ErrorResponse, errors::IntegrationError> {
    let error_response = if res.response.is_empty() {
        {ConnectorName}ErrorResponse::default()
    } else {
        // Try to parse error response, fallback to default
        res.response
            .parse_struct("ErrorResponse")
            .unwrap_or_else(|_| {ConnectorName}ErrorResponse::default())
    };

    if let Some(i) = event_builder {
        i.set_error_response_body(&error_response);
    }

    // Handle specific network error scenarios
    let (attempt_status, network_error_message) = match res.status_code {
        0 => {
            // Network timeout or connection failure
            (Some(common_enums::AttemptStatus::Pending), Some("Network connection timeout".to_string()))
        },
        408 => {
            // Request timeout
            (Some(common_enums::AttemptStatus::Pending), Some("Request timeout".to_string()))
        },
        502 | 503 | 504 => {
            // Gateway errors - usually temporary
            (Some(common_enums::AttemptStatus::Pending), Some("Gateway error - please retry".to_string()))
        },
        _ => (None, None),
    };

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: error_response.error_code.unwrap_or("NETWORK_ERROR".to_string()),
        message: error_response.error_message.unwrap_or("Network error occurred".to_string()),
        reason: error_response.error_description,
        attempt_status,
        connector_transaction_id: error_response.transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message,
    })
}
```

### Pattern 6: Transaction State Error Handling

Used when sync operations reveal transaction state issues.

```rust
pub fn handle_transaction_state_errors(
    sync_response: &{ConnectorName}SyncResponse,
) -> Option<ErrorResponse> {
    // Check for transaction state issues
    match (&sync_response.status, &sync_response.transaction_state) {
        (status, Some(state)) if is_error_state(state) => {
            let (error_code, error_message, attempt_status) = match state.as_str() {
                "EXPIRED" => (
                    "TRANSACTION_EXPIRED",
                    "Transaction has expired and cannot be processed",
                    common_enums::AttemptStatus::Failure,
                ),
                "CANCELLED_BY_USER" => (
                    "USER_CANCELLED",
                    "Transaction was cancelled by the user",
                    common_enums::AttemptStatus::Voided,
                ),
                "INSUFFICIENT_FUNDS" => (
                    "INSUFFICIENT_FUNDS",
                    "Insufficient funds for transaction",
                    common_enums::AttemptStatus::Failure,
                ),
                "FRAUD_DETECTED" => (
                    "FRAUD_DETECTED",
                    "Transaction blocked due to fraud detection",
                    common_enums::AttemptStatus::Failure,
                ),
                _ => (
                    "TRANSACTION_ERROR",
                    "Transaction is in an error state",
                    common_enums::AttemptStatus::Failure,
                ),
            };

            Some(ErrorResponse {
                status_code: 200, // HTTP OK but business logic error
                code: error_code.to_string(),
                message: error_message.to_string(),
                reason: Some(format!("Transaction state: {}", state)),
                attempt_status: Some(attempt_status),
                connector_transaction_id: Some(sync_response.id.clone()),
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            })
        },
        _ => None,
    }
}

fn is_error_state(state: &str) -> bool {
    matches!(
        state.to_uppercase().as_str(),
        "EXPIRED" | "CANCELLED_BY_USER" | "INSUFFICIENT_FUNDS" | "FRAUD_DETECTED" | "BLOCKED"
    )
}
```

### Error Handling Helper Functions

```rust
// Generic error response builder
pub fn build_standard_error_response(
    connector_name: &str,
    res: Response,
    default_error_code: &str,
    default_error_message: &str,
) -> CustomResult<ErrorResponse, IntegrationError> {
    let parsed_error = parse_error_response(&res.response)
        .unwrap_or_else(|| create_default_error(default_error_code, default_error_message));

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: parsed_error.code,
        message: parsed_error.message,
        reason: parsed_error.description,
        attempt_status: determine_attempt_status_from_error(&parsed_error, res.status_code),
        connector_transaction_id: parsed_error.transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
    })
}

fn parse_error_response(response_body: &str) -> Option<ParsedError> {
    // Try multiple parsing strategies
    if let Ok(standard_error) = serde_json::from_str::<StandardErrorFormat>(response_body) {
        return Some(ParsedError {
            code: standard_error.error_code,
            message: standard_error.error_message,
            description: standard_error.error_description,
            transaction_id: standard_error.transaction_id,
        });
    }

    // Try alternative error format
    if let Ok(alt_error) = serde_json::from_str::<AlternativeErrorFormat>(response_body) {
        return Some(ParsedError {
            code: alt_error.code,
            message: alt_error.message,
            description: None,
            transaction_id: None,
        });
    }

    None
}

struct ParsedError {
    code: String,
    message: String,
    description: Option<String>,
    transaction_id: Option<String>,
}

fn determine_attempt_status_from_error(
    error: &ParsedError,
    status_code: u16,
) -> Option<common_enums::AttemptStatus> {
    // Combine error code and HTTP status code for better classification
    match (error.code.as_str(), status_code) {
        ("AUTHENTICATION_FAILED", _) | (_, 401) => Some(common_enums::AttemptStatus::AuthenticationFailed),
        ("AUTHORIZATION_FAILED", _) | (_, 403) => Some(common_enums::AttemptStatus::AuthorizationFailed),
        ("RATE_LIMITED", _) | (_, 429) => Some(common_enums::AttemptStatus::Pending),
        ("SERVER_ERROR", _) | (_, 500..=599) => Some(common_enums::AttemptStatus::Pending),
        _ => Some(common_enums::AttemptStatus::Failure),
    }
}
```

## Testing Patterns

### Unit Test Structure for Psync Flow

```rust
#[cfg(test)]
mod psync_tests {
    use super::*;
    use common_enums::{Currency, AttemptStatus};
    use domain_types::connector_types::PaymentFlowData;

    #[test]
    fn test_psync_request_transformation_get() {
        // Test GET-based sync request (empty request body)
        let router_data = create_test_psync_router_data();
        let connector_req = {ConnectorName}SyncRequest::try_from(&router_data);
        
        assert!(connector_req.is_ok());
        // For GET requests, request should be empty or minimal
    }

    #[test]
    fn test_psync_request_transformation_post() {
        // Test POST-based sync request transformation
        let router_data = create_test_psync_router_data();
        let connector_req = {ConnectorName}SyncRequest::try_from(&router_data);
        
        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert!(req.transaction_id.is_some());
        assert_eq!(req.transaction_id.unwrap(), "test_txn_123");
    }

    #[test]
    fn test_psync_response_transformation_success() {
        // Test successful sync response
        let response = {ConnectorName}SyncResponse {
            id: "sync_123".to_string(),
            status: {ConnectorName}PaymentStatus::Succeeded,
            amount: Some(MinorUnit::new(1000)),
            payment_id: Some("payment_456".to_string()),
            reference: Some("test_ref".to_string()),
            error: None,
            error_code: None,
        };

        let router_data = create_test_psync_router_data();
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
    fn test_psync_response_transformation_pending() {
        // Test pending sync response
        let response = {ConnectorName}SyncResponse {
            id: "sync_456".to_string(),
            status: {ConnectorName}PaymentStatus::Pending,
            amount: Some(MinorUnit::new(1000)),
            payment_id: Some("payment_789".to_string()),
            reference: Some("test_ref_2".to_string()),
            error: None,
            error_code: None,
        };

        let router_data = create_test_psync_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());
        
        let router_data_result = result.unwrap();
        assert_eq!(router_data_result.resource_common_data.status, AttemptStatus::Pending);
    }

    #[test]
    fn test_psync_response_transformation_failure() {
        // Test failed sync response
        let response = {ConnectorName}SyncResponse {
            id: "sync_789".to_string(),
            status: {ConnectorName}PaymentStatus::Failed,
            amount: None,
            payment_id: Some("payment_456".to_string()),
            reference: None,
            error: Some("Payment processing failed".to_string()),
            error_code: Some("PAYMENT_FAILED".to_string()),
        };

        let router_data = create_test_psync_router_data();
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
        let mut router_data = create_test_psync_router_data();
        router_data.request.connector_transaction_id = None;

        let connector_req = {ConnectorName}SyncRequest::try_from(&router_data);
        
        // For POST requests, this should error
        // For GET requests, the error should occur in URL construction
        if is_post_based_connector() {
            assert!(connector_req.is_err());
        } else {
            // GET requests fail in URL construction, not request transformation
            assert!(connector_req.is_ok());
        }
    }

    #[test]
    fn test_psync_url_construction_get() {
        // Test URL construction for GET requests
        let connector = {ConnectorName}::new();
        let router_data = create_test_psync_router_data();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains(&router_data.request.connector_transaction_id.unwrap()));
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_psync_url_construction_post() {
        // Test URL construction for POST requests
        let connector = {ConnectorName}::new();
        let router_data = create_test_psync_router_data();
        
        let url = connector.get_url(&router_data).unwrap();
        // POST URLs may not contain transaction ID (it's in the body)
        assert!(url.starts_with("https://"));
        assert!(url.contains("sync") || url.contains("inquiry") || url.contains("status"));
    }

    #[test]
    fn test_status_mapping_all_variants() {
        // Test all status mapping variants
        let test_cases = vec![
            ({ConnectorName}PaymentStatus::Succeeded, AttemptStatus::Charged),
            ({ConnectorName}PaymentStatus::Failed, AttemptStatus::Failure),
            ({ConnectorName}PaymentStatus::Pending, AttemptStatus::Pending),
            ({ConnectorName}PaymentStatus::Cancelled, AttemptStatus::Voided),
            ({ConnectorName}PaymentStatus::RequiresAction, AttemptStatus::AuthenticationPending),
            ({ConnectorName}PaymentStatus::Authorized, AttemptStatus::Authorized),
        ];

        for (connector_status, expected_status) in test_cases {
            let mapped_status = AttemptStatus::from(connector_status);
            assert_eq!(mapped_status, expected_status);
        }
    }

    #[test]
    fn test_error_response_handling() {
        // Test error response handling
        let error_response = {ConnectorName}ErrorResponse {
            error_code: Some("TRANSACTION_NOT_FOUND".to_string()),
            error_message: Some("Transaction not found".to_string()),
            error_description: Some("The specified transaction ID was not found".to_string()),
            transaction_id: Some("txn_123".to_string()),
        };

        // Test error response transformation
        let response = Response {
            response: serde_json::to_string(&error_response).unwrap().into(),
            status_code: 404,
            headers: None,
        };

        let connector = {ConnectorName}::new();
        let error_result = connector.build_error_response(response, None);
        
        assert!(error_result.is_ok());
        let error = error_result.unwrap();
        assert_eq!(error.code, "TRANSACTION_NOT_FOUND");
        assert_eq!(error.message, "Transaction not found");
        assert_eq!(error.status_code, 404);
    }

    #[test]
    fn test_headers_generation() {
        // Test headers generation for sync requests
        let connector = {ConnectorName}::new();
        let router_data = create_test_psync_router_data();
        
        let headers = connector.get_headers(&router_data).unwrap();
        
        // Check authentication headers are present
        assert!(headers.iter().any(|(key, _)| key == "Authorization" || key.starts_with("X-")));
        
        // For POST requests, check Content-Type
        if is_post_based_connector() {
            assert!(headers.iter().any(|(key, value)| 
                key == "Content-Type" && value.expose() == "application/json"
            ));
        }
    }

    fn create_test_psync_router_data() -> RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData> {
        // Create test router data structure for sync
        RouterDataV2 {
            resource_common_data: PaymentFlowData {
                // ... test data
            },
            request: PaymentsSyncData {
                connector_transaction_id: Some("test_txn_123".to_string()),
                encoded_data: None,
                capture_method: None,
                connector_meta: None,
                sync_type: None,
                // ... other test fields
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                // ... response data
            }),
            // ... other router data fields
        }
    }

    fn is_post_based_connector() -> bool {
        // Return true if this connector uses POST for sync, false for GET
        // This helps with conditional test logic
        false // Update based on actual implementation
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod psync_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_psync_flow_integration_get() {
        let connector = {ConnectorName}::new();
        
        // Mock sync request data for GET
        let request_data = create_test_psync_request();
        
        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(!headers.is_empty());
        
        // Test URL generation
        let url = connector.get_url(&request_data).unwrap();
        assert!(url.contains(&request_data.request.connector_transaction_id.unwrap()));
        
        // For GET requests, no request body should be generated
        if !is_post_based_connector() {
            // GET requests typically don't have request bodies in the macro framework
            assert!(true); // Placeholder for GET-specific tests
        }
    }

    #[tokio::test]
    async fn test_psync_flow_integration_post() {
        let connector = {ConnectorName}::new();
        
        // Mock sync request data for POST
        let request_data = create_test_psync_request();
        
        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(headers.iter().any(|(key, value)| 
            key == "Content-Type" && value.expose() == "application/json"
        ));
        
        // Test URL generation
        let url = connector.get_url(&request_data).unwrap();
        assert!(url.contains("sync") || url.contains("inquiry") || url.contains("status"));
        
        // Test request body generation (for POST)
        if is_post_based_connector() {
            let request_body = connector.get_request_body(&request_data).unwrap();
            assert!(request_body.is_some());
        }
    }

    #[tokio::test]
    async fn test_response_processing() {
        let connector = {ConnectorName}::new();
        let request_data = create_test_psync_request();
        
        // Test successful response processing
        let success_response = create_mock_success_response();
        let processed_response = connector.handle_response_v2(
            &request_data,
            None,
            success_response,
        );
        
        assert!(processed_response.is_ok());
        let result = processed_response.unwrap();
        assert_eq!(result.resource_common_data.status, AttemptStatus::Charged);
        
        // Test error response processing
        let error_response = create_mock_error_response();
        let processed_error = connector.handle_response_v2(
            &request_data,
            None,
            error_response,
        );
        
        assert!(processed_error.is_ok());
        let error_result = processed_error.unwrap();
        assert_eq!(error_result.resource_common_data.status, AttemptStatus::Failure);
    }

    fn create_mock_success_response() -> Response {
        let response_body = {ConnectorName}SyncResponse {
            id: "test_123".to_string(),
            status: {ConnectorName}PaymentStatus::Succeeded,
            amount: Some(MinorUnit::new(1000)),
            payment_id: Some("payment_456".to_string()),
            reference: Some("test_ref".to_string()),
            error: None,
            error_code: None,
        };

        Response {
            response: serde_json::to_string(&response_body).unwrap().into(),
            status_code: 200,
            headers: None,
        }
    }

    fn create_mock_error_response() -> Response {
        let error_body = {ConnectorName}ErrorResponse {
            error_code: Some("PAYMENT_FAILED".to_string()),
            error_message: Some("Payment processing failed".to_string()),
            error_description: Some("Insufficient funds".to_string()),
            transaction_id: Some("test_123".to_string()),
        };

        Response {
            response: serde_json::to_string(&error_body).unwrap().into(),
            status_code: 400,
            headers: None,
        }
    }
}
```

### Performance and Edge Case Tests

```rust
#[cfg(test)]
mod psync_edge_case_tests {
    use super::*;

    #[test]
    fn test_large_transaction_id() {
        // Test with very long transaction IDs
        let mut router_data = create_test_psync_router_data();
        router_data.request.connector_transaction_id = Some("a".repeat(1000));

        let connector = {ConnectorName}::new();
        let url_result = connector.get_url(&router_data);
        
        // Should handle long IDs gracefully
        assert!(url_result.is_ok());
    }

    #[test]
    fn test_special_characters_in_transaction_id() {
        // Test with special characters in transaction ID
        let test_cases = vec![
            "txn-123",
            "txn_456",
            "txn.789",
            "txn#123",
            "txn%456",
        ];

        for transaction_id in test_cases {
            let mut router_data = create_test_psync_router_data();
            router_data.request.connector_transaction_id = Some(transaction_id.to_string());

            let connector = {ConnectorName}::new();
            let url_result = connector.get_url(&router_data);
            
            // URL should be properly encoded
            assert!(url_result.is_ok());
            let url = url_result.unwrap();
            
            // Check that special characters are properly URL encoded
            if transaction_id.contains('#') {
                assert!(url.contains("%23"));
            }
            if transaction_id.contains('%') {
                assert!(url.contains("%25"));
            }
        }
    }

    #[test]
    fn test_empty_response_handling() {
        // Test handling of empty responses
        let connector = {ConnectorName}::new();
        let empty_response = Response {
            response: "".into(),
            status_code: 200,
            headers: None,
        };

        let error_result = connector.build_error_response(empty_response, None);
        assert!(error_result.is_ok());
        
        let error = error_result.unwrap();
        assert!(!error.code.is_empty());
        assert!(!error.message.is_empty());
    }

    #[test]
    fn test_malformed_response_handling() {
        // Test handling of malformed JSON responses
        let connector = {ConnectorName}::new();
        let malformed_response = Response {
            response: "{ invalid json }".into(),
            status_code: 200,
            headers: None,
        };

        let error_result = connector.build_error_response(malformed_response, None);
        assert!(error_result.is_ok());
    }

    #[test]
    fn test_concurrent_psync_requests() {
        // Test that multiple sync requests can be processed concurrently
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let connector = Arc::new({ConnectorName}::new());
        let mut join_set = JoinSet::new();

        for i in 0..10 {
            let connector = connector.clone();
            join_set.spawn(async move {
                let mut router_data = create_test_psync_router_data();
                router_data.request.connector_transaction_id = Some(format!("txn_{}", i));
                
                let url = connector.get_url(&router_data);
                assert!(url.is_ok());
                url.unwrap()
            });
        }

        // All requests should complete successfully
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            results.push(result.unwrap());
        }

        assert_eq!(results.len(), 10);
        
        // All URLs should be unique (contain different transaction IDs)
        let unique_urls: std::collections::HashSet<_> = results.into_iter().collect();
        assert_eq!(unique_urls.len(), 10);
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

**⚠️ CRITICAL: Complete API Analysis Methodology BEFORE implementation to avoid dual-endpoint issues**

- [ ] **API Analysis Methodology (MANDATORY)**
  - [ ] **Step 1: OpenAPI/Documentation Deep Dive**
    - [ ] Search for all sync-related endpoints (`grep -i "payment.*status\|sync\|inquiry\|retrieve\|query"`)
    - [ ] Document each endpoint's purpose (status vs inquiry vs retrieval)
    - [ ] Compare request schemas between authorization and sync endpoints
    - [ ] **CRITICAL**: Compare response schemas between authorization and sync endpoints
    - [ ] Identify any terminology differences (status vs inquiry vs query)
  - [ ] **Step 2: Response Schema Validation (MANDATORY)**
    - [ ] **Different Field Names**: Check for `outcome` vs `lastEvent` vs `status` vs `state`
    - [ ] **Different Field Values**: Check for `"authorized"` vs `"Authorized"` vs `"AUTHORIZED"`
    - [ ] **Missing Fields**: Verify if sync responses lack `transaction_reference` or other auth fields
    - [ ] **Additional Fields**: Check for sync-specific fields like `_actions`, `_links`
    - [ ] **Nested Structure Differences**: Verify object vs string vs array variations
  - [ ] **Step 3: Psync-Specific Analysis Checklist**
    - [ ] **Response Structure Identity**: Are auth and sync responses identical? (Usually NO!)
    - [ ] **Field Name Consistency**: Do status fields use same names?
    - [ ] **Status Value Format**: Same casing and values between flows?
    - [ ] **Required vs Optional Fields**: Which fields are guaranteed in sync responses?
    - [ ] **Transaction ID Location**: Where is transaction ID in sync response vs auth response?
    - [ ] **Error Response Format**: Same error structure for sync vs auth endpoints?
  - [ ] **Step 4: Implementation Strategy Selection**
    - [ ] Choose Strategy A (Identical), B (Different), or C (Unified Enum) based on analysis
    - [ ] Plan separate response structures if needed
    - [ ] Design field mapping strategy for different field names
    - [ ] Plan transaction ID extraction strategy
  - [ ] **Step 5: Pre-Implementation Validation**
    - [ ] **Response Sample Testing**: Test with actual API responses if available
    - [ ] **Field Mapping Documentation**: Document all field differences between auth and sync
    - [ ] **Status Mapping Verification**: Confirm status value mappings for sync responses
    - [ ] **Error Case Documentation**: Document sync-specific error responses
    - [ ] **Transaction ID Extraction**: Verify how to extract transaction ID from sync response

- [ ] **Traditional API Documentation Review** (after API Analysis)
  - [ ] Understand connector's sync/status API endpoints (all of them)
  - [ ] Review sync authentication requirements (usually same as auth)
  - [ ] Identify sync-specific required/optional fields for each endpoint
  - [ ] Understand sync error response formats for each endpoint
  - [ ] Review sync status codes and meanings for each endpoint
  - [ ] Check if sync supports multiple query types
  - [ ] Verify transaction ID requirements for each endpoint
  - [ ] Document any sync rate limits or restrictions

- [ ] **Enhanced Psync Flow Requirements**
  - [ ] Determine HTTP method (GET vs POST) based on API design
  - [ ] Identify URL pattern for sync (RESTful vs query parameters vs fixed endpoint)
  - [ ] Check if transaction ID goes in URL path or request body
  - [ ] **CRITICAL**: Verify sync response structure differs from auth response
  - [ ] Review sync response structure and fields (may be different from auth)
  - [ ] Check for sync-specific error codes
  - [ ] Understand transaction state lifecycle
  - [ ] Document any special sync requirements (webhooks, polling, etc.)

### Implementation Checklist

- [ ] **Main Connector File Updates**
  - [ ] Add `PSync` to connector_flow imports
  - [ ] Add `PaymentsSyncData` to connector_types imports
  - [ ] Import sync request/response types from transformers
  - [ ] Implement `PaymentSyncV2` trait
  - [ ] Add PSync flow to `macros::create_all_prerequisites!`
  - [ ] Implement sync flow with `macros::macro_connector_implementation!`
  - [ ] Choose correct HTTP method (Get or Post)
  - [ ] Add Source Verification stub for PSync flow

- [ ] **Transformers Implementation**
  - [ ] Add `PSync` to connector_flow imports
  - [ ] Add `PaymentsSyncData` to connector_types imports
  - [ ] Create sync request structure (or empty struct for GET)
  - [ ] Create sync response structure
  - [ ] Create payment status enumeration
  - [ ] Implement status mapping for sync responses
  - [ ] Implement sync request transformation (`TryFrom`)
  - [ ] Implement sync response transformation (`TryFrom`)
  - [ ] Add transaction ID validation and extraction
  - [ ] Handle sync-specific error cases

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test sync request transformation (empty for GET, populated for POST)
  - [ ] Test sync response transformation (success cases)
  - [ ] Test sync response transformation (failure cases)
  - [ ] Test sync response transformation (pending cases)
  - [ ] Test missing transaction ID error handling
  - [ ] Test sync URL construction
  - [ ] Test sync status mapping for all status variants
  - [ ] Test sync-specific error codes
  - [ ] Test headers generation

- [ ] **Integration Tests**
  - [ ] Test sync headers generation
  - [ ] Test sync URL construction
  - [ ] Test sync request body generation (POST only)
  - [ ] Test complete sync flow
  - [ ] Test error response handling
  - [ ] Test malformed response handling

- [ ] **Edge Case Tests**
  - [ ] Test large transaction IDs
  - [ ] Test special characters in transaction IDs
  - [ ] Test empty responses
  - [ ] Test timeout scenarios
  - [ ] Test concurrent sync requests

### Configuration Checklist

- [ ] **Connector Configuration**
  - [ ] Ensure sync uses same base URL configuration as other flows
  - [ ] Verify sync endpoint paths are correctly configured
  - [ ] Add any sync-specific configuration if needed
  - [ ] Test with development/sandbox credentials

- [ ] **Registration**
  - [ ] Verify connector is properly registered with PSync flow
  - [ ] Test sync flow is accessible through routing
  - [ ] Verify sync responses are properly processed by router

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all sync-related errors
  - [ ] Run `cargo test` and ensure sync tests pass
  - [ ] Run `cargo clippy` and fix sync-related warnings
  - [ ] Verify sync flow compiles with macro framework

- [ ] **Functionality Validation**
  - [ ] Test sync with sandbox/test credentials
  - [ ] Verify successful payment status retrieval
  - [ ] Verify sync error handling works correctly
  - [ ] Test sync with various payment states (pending, success, failure)
  - [ ] Verify sync status mapping is correct
  - [ ] Test sync idempotency (multiple calls return same result)
  - [ ] Verify sync performance (reasonable response times)

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Add comprehensive doc comments for sync structures
  - [ ] Document sync-specific requirements or limitations
  - [ ] Add usage examples for sync flow
  - [ ] Document status mapping logic

- [ ] **Integration Documentation**
  - [ ] Document sync endpoint URL pattern
  - [ ] Document sync request/response format
  - [ ] Document sync-specific error codes
  - [ ] Document sync rate limits (if any)
  - [ ] Document sync polling intervals (if applicable)
  - [ ] Document any sync flow limitations

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM FOR PSYNC FLOWS**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal`, `NewPayment` | **Always required** - Used in struct names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal`, `new_payment` | **Always required** - Used in config keys |
| `{AmountType}` | Amount type (same as auth flow) | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **Must match auth flow** |
| `{HttpMethod}` | HTTP method for sync requests | `Get`, `Post` | **Choose based on API**: GET for simple status, POST for complex queries |
| `{content_type}` | Request content type (POST only) | `"application/json"`, `"application/x-www-form-urlencoded"` | **POST requests only** |
| `{psync_endpoint}` | Sync API endpoint path | `"v1/payments/{id}/status"`, `"transaction-inquiry"`, `"sync"` | **From API docs** |
| `{auth_type}` | Authentication type | `HeaderKey`, `SignatureKey`, `BodyKey` | **Same as auth flow** |
| `{url_pattern}` | URL construction pattern | `"rest_with_id"`, `"query_parameter"`, `"hierarchical"` | **Based on API style** |

### HTTP Method Selection Guide

Choose the right HTTP method based on your connector's API:

| API Characteristics | HTTP Method | When to Use |
|-------------------|-------------|-------------|
| Simple status endpoint with transaction ID in URL | `Get` | Most RESTful APIs (Checkout, Volt, Xendit) |
| Status query with no sensitive parameters | `Get` | Simple payment gateways |
| Complex query parameters required | `Post` | Enterprise APIs (Fiserv, Paytm) |
| Authentication must be in request body | `Post` | Legacy APIs (Authorizedotnet) |
| GraphQL-based queries | `Post` | Modern graph APIs (Braintree) |

### Psync Endpoint Selection Guide

Choose the right endpoint pattern based on your connector's API:

| API Style | Endpoint Pattern | Example |
|-----------|------------------|---------|
| RESTful status | `"v1/payments/{id}/status"` | Most modern APIs |
| RESTful resource | `"v1/payments/{id}"` | REST-compliant APIs |
| Dedicated inquiry | `"v1/transaction-inquiry"` | Enterprise APIs |
| Status service | `"v1/status"` | Service-oriented APIs |
| Same as auth | `""` (empty, uses base URL) | Single-endpoint APIs |

### Real-World Examples

**Example 1: Simple GET API (Checkout-style)**
```bash
{ConnectorName} → MyPayment
{connector_name} → my_payment
{AmountType} → MinorUnit
{HttpMethod} → Get
{psync_endpoint} → "payments/{id}"
{auth_type} → HeaderKey
URL Pattern: {base_url}/payments/{transaction_id}
```

**Example 2: Complex POST API (Fiserv-style)**
```bash
{ConnectorName} → EnterprisePay
{connector_name} → enterprise_pay
{AmountType} → FloatMajorUnit
{HttpMethod} → Post
{content_type} → "application/json"
{psync_endpoint} → "ch/payments/v1/transaction-inquiry"
{auth_type} → SignatureKey
URL Pattern: {base_url}/ch/payments/v1/transaction-inquiry
```

**Example 3: Status Endpoint API (Bluecode-style)**
```bash
{ConnectorName} → StatusPay
{connector_name} → status_pay
{AmountType} → FloatMajorUnit
{HttpMethod} → Get
{psync_endpoint} → "api/v1/order/{id}/status"
{auth_type} → HeaderKey
URL Pattern: {base_url}/api/v1/order/{transaction_id}/status
```

## Best Practices

1. **HTTP Method Selection**: Use GET for simple status queries, POST for complex queries or when authentication requires request body
2. **Transaction ID Validation**: Always validate the presence of `connector_transaction_id` before building sync requests
3. **URL Construction**: Follow RESTful principles when possible, use path parameters for resource identification
4. **Status Mapping (CRITICAL)**:
   - **Always map status from connector's operation_result or equivalent field** - Never assume status based on HTTP code alone
   - **Use From trait implementation** - Implement the From trait to map connector status enum to AttemptStatus
   - **Use explicit match statements** - When From trait isn't suitable, use explicit match statements for clarity
   - Create comprehensive status mapping that covers all possible connector statuses
5. **Field Mapping (CRITICAL)**:
   - **Never hardcode fields to None** - Always map actual fields from connector response
   - **Show clean response transformers** - Build response transformers that properly extract and map all available fields
   - Map connector_response_reference_id, network_txn_id, and other fields when available
6. **Error Handling**: Implement robust error handling for network issues, timeouts, and business logic errors
7. **Authentication Consistency**: Use the same authentication method as authorization flow
8. **Response Processing**: Handle both success and error responses gracefully
9. **Idempotency**: Ensure sync operations are idempotent and can be safely retried
10. **Performance**: Keep sync operations lightweight and fast for real-time status checking
11. **Documentation**: Document all status codes, error codes, and special sync requirements

### Common Pitfalls to Avoid

- **Missing Transaction ID**: Always check for `connector_transaction_id` before proceeding
- **Wrong HTTP Method**: Don't use POST for simple status queries unless required by API
- **Incomplete Status Mapping**: Map all possible connector statuses to appropriate `AttemptStatus` values
- **Status from HTTP Code**: NEVER assume status based on HTTP code alone - always map from connector's operation_result or status field
- **Hardcoded None Fields**: NEVER hardcode response fields to None - always map actual values from connector response
- **Authentication Mismatch**: Ensure sync uses same auth method as other flows
- **URL Encoding**: Properly encode special characters in transaction IDs for URL construction
- **Error Response Parsing**: Handle both structured and unstructured error responses
- **Timeout Handling**: Implement appropriate timeout handling for sync operations
- **Rate Limiting**: Respect connector rate limits for sync operations

This pattern document provides a comprehensive template for implementing Psync flows in payment connectors, ensuring consistency and completeness across all implementations while accommodating the diverse API styles found across different payment gateways.