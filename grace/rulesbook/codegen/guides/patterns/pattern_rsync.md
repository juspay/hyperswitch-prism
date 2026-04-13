# RSync Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This comprehensive guide provides battle-tested patterns for implementing RSync (Refund Sync) flows in Grace-UCS connectors, extracted from analysis of all 22 connectors in the connector service.

## 🚀 Quick Start Guide

To implement RSync flows in a new connector:

1. **Choose Pattern**: Use [Modern REST Pattern](#pattern-1-modern-rest-api-recommended) for 95% of connectors
2. **Add Trait Implementations**: Use [macro_connector_implementation!](#modern-macro-based-pattern-recommended)
3. **Define Request/Response**: Follow [Data Structures](#request-response-data-structures)
4. **Test Implementation**: Use [Testing Patterns](#testing-patterns)

### Example: Implementing "MyPayment" Connector RSync Flow

```bash
# Replace placeholders:
{ConnectorName} → MyPayment
{connector_name} → my_payment
{base_url} → https://api.mypayment.com
```

**✅ Result**: Complete, production-ready connector RSync implementation in ~15 minutes

## Table of Contents

- [Overview](#overview)
- [RSync Flow Implementation Analysis](#rsync-flow-implementation-analysis-real-world-data)
- [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
- [Implementation Patterns by Type](#implementation-patterns-by-type)
- [Authentication Patterns](#authentication-patterns)
- [URL Construction Patterns](#url-construction-patterns)
- [Status Mapping Patterns](#status-mapping-patterns)
- [Error Handling Patterns](#error-handling-patterns)
- [Testing Patterns](#testing-patterns)
- [Integration Checklist](#integration-checklist)
- [Placeholder Reference Guide](#placeholder-reference-guide)
- [Troubleshooting Guide](#troubleshooting-guide)

## Overview

RSync (Refund Sync) flows enable connectors to query the current status of refund transactions from payment processors. This is essential for:

- **Refund Status Tracking**: Monitor refund processing status
- **Reconciliation**: Ensure refund status consistency 
- **Webhook Backup**: Provide status updates when webhooks fail
- **Customer Support**: Real-time refund status for support teams

**🔍 Key Flow Characteristics:**
- **Direction**: Outbound query from UCS to payment processor
- **Trigger**: Manual sync or scheduled reconciliation
- **Response**: Current refund status and metadata
- **Frequency**: On-demand or periodic sync operations

## RSync Flow Implementation Analysis (Real-world data)

Based on comprehensive analysis of all 22 connectors, here's the implementation status:

### ✅ Full RSync Flow Implementation (12 connectors)

1. **authorizedotnet** - XML transaction details API with embedded auth
2. **braintree** - GraphQL refund search with sophisticated error handling
3. **checkout** - Payment actions endpoint with bearer token auth
4. **elavon** - Unified XML gateway with form-encoded requests
5. **fiserv** - HMAC-signed transaction inquiry with complex authentication
6. **fiuu** - PHP-based refund API with MD5 signature verification
7. **nexinets** - RESTful order/transaction endpoint with metadata dependency
8. **noon** - Reference-based order lookup with transaction parsing
9. **novalnet** - Transaction details API with webhook support
10. **razorpay** - Simple GET-based refund status API
11. **razorpayv2** - Enhanced version with dual authentication support
12. **xendit** - Standard REST refund status endpoint

### 🔧 Stub/Trait Implementation Only (10 connectors)

1. **adyen** - Empty trait implementations, no functionality
2. **bluecode** - Stub (refunds explicitly not supported)
3. **cashfree** - Stub with source verification trait only
4. **cashtocode** - Stub implementation
5. **mifinity** - Stub implementation
6. **paytm** - Stub implementation  
7. **payu** - Stub implementation
8. **phonepe** - Stub implementation
9. **volt** - Stub implementation
10. **worldpay** - Stub implementation

### 📊 Implementation Statistics

- **Complete implementations**: 12/22 (55%)
- **Stub implementations**: 10/22 (45%)
- **Most common pattern**: GET-based REST API (8/12 implementations)
- **Most common auth**: Basic Authentication (6/12 implementations)
- **Most common response format**: JSON (10/12 implementations)

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### Main Connector File Pattern (RSync Flow Addition)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

use crate::connectors::macros::macro_connector_implementation;

// RSync trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for {ConnectorName}<T>
{
    // HTTP method for RSync requests
    fn get_http_method(&self) -> Method {
        Method::Get // Most common: GET for status queries
    }

    // URL construction for RSync endpoint
    fn get_url(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let refund_id = req.request.connector_refund_id.clone();
        
        // Validate refund ID
        if refund_id.is_empty() {
            return Err(errors::IntegrationError::MissingRequiredField {
                field_name: "connector_refund_id",
            , context: Default::default() });
        }

        Ok(format!("{}/refunds/{}", self.connector_base_url_refunds(req), refund_id))
    }

    // Request body transformation (for GET requests, usually None)
    fn get_request_body(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        _connectors: &crate::Connectors,
    ) -> CustomResult<RequestContent, IntegrationError> {
        // For GET requests
        Ok(RequestContent::Json(Box::new(serde_json::Value::Null)))
        
        // For POST requests with body
        // let request = transformers::{ConnectorName}RefundSyncRequest::try_from(req)?;
        // Ok(RequestContent::Json(Box::new(request)))
    }

    // Response handling and transformation
    fn handle_response_v2(
        &self,
        data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        event_builder: Option<&mut ConnectorEvent>,
        res: types::Response,
    ) -> CustomResult<RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>, ConnectorError> {
        let response: transformers::{ConnectorName}RefundSyncResponse = res
            .response
            .parse_struct("{ConnectorName} RSync Response")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        event_builder.map(|i| i.set_response_body(&response));
        router_env::logger::info!(connector_response=?response);

        RouterDataV2::try_from(ResponseRouterData {
            response,
            data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(errors::ConnectorError::ResponseHandlingFailed)
    }

    // Error response handling
    fn get_error_response(
        &self,
        res: types::Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

// Source verification for security (optional but recommended)
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification<
        RSync,
        RefundFlowData,
        RefundSyncData,
        RefundsResponseData,
    > for {ConnectorName}<T>
{
}

// Macro implementation for boilerplate code
macro_connector_implementation!(
    {ConnectorName}<T>,
    RSync,
    RefundFlowData,
    RefundSyncData,
    RefundsResponseData,
    get_http_method,
    get_url,
    get_request_body,
    build_request_v2,
    handle_response_v2,
    get_error_response,
    get_5xx_error_response,
    text_to_json_deserialize_if_required,
    IntegrationError
);
```

### Transformers File Pattern (RSync Flow)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use serde::{Deserialize, Serialize};
use crate::{
    core::errors,
    domain_types::{RefundSyncData, RefundsResponseData},
    router_data::{ErrorResponse, ResponseRouterData, RouterDataV2},
    types::{RefundFlowData, RSync},
};

// Request structure (for POST requests)
#[derive(Debug, Serialize)]
pub struct {ConnectorName}RefundSyncRequest {
    pub refund_id: String,
    // Add other required fields based on connector API
}

// Response structure
#[derive(Debug, Clone, Deserialize)]
pub struct {ConnectorName}RefundSyncResponse {
    pub id: String,
    pub status: {ConnectorName}RefundStatus,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    // Add other response fields
}

// Status enumeration
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum {ConnectorName}RefundStatus {
    Success,
    Pending,
    Failed,
    // Add connector-specific statuses
}

// Request transformation (for POST requests)
impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for {ConnectorName}RefundSyncRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            refund_id: item.request.connector_refund_id.clone(),
        })
    }
}

// Status mapping
impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        match status {
            {ConnectorName}RefundStatus::Success => Self::Success,
            {ConnectorName}RefundStatus::Pending => Self::Pending,
            {ConnectorName}RefundStatus::Failed => Self::Failure,
        }
    }
}

// Response transformation
impl TryFrom<ResponseRouterData<RSync, {ConnectorName}RefundSyncResponse, RefundSyncData, RefundsResponseData>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ResponseRouterData<RSync, {ConnectorName}RefundSyncResponse, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: common_enums::RefundStatus::from(item.response.status),
            }),
            ..item.data
        })
    }
}
```

## Implementation Patterns by Type

### Pattern 1: Modern REST API (Recommended)

**Use for**: Modern payment processors with RESTful APIs

```rust
// GET /refunds/{refund_id}
fn get_http_method(&self) -> Method { Method::Get }
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/refunds/{}", base_url, req.request.connector_refund_id))
}
```

**Examples**: razorpay, razorpayv2, xendit, nexinets

### Pattern 2: GraphQL API (Complex)

**Use for**: Processors using GraphQL (e.g., Braintree)

```rust
fn get_request_body(&self, req) -> CustomResult<RequestContent, IntegrationError> {
    let request = BraintreeRSyncRequest {
        query: constants::REFUND_QUERY.to_string(),
        variables: RSyncInput {
            input: RefundSearchInput {
                id: IdFilter { is: refund_id },
            },
        },
    };
    Ok(RequestContent::Json(Box::new(request)))
}
```

**Examples**: braintree

### Pattern 3: XML Gateway (Legacy)

**Use for**: Legacy processors requiring XML

```rust
fn get_request_body(&self, req) -> CustomResult<RequestContent, IntegrationError> {
    let request = XMLRSyncRequest {
        get_transaction_details_request: TransactionDetails {
            merchant_authentication: auth,
            transaction_id: Some(refund_id),
        },
    };
    Ok(RequestContent::FormUrlEncoded(Box::new(request)))
}
```

**Examples**: elavon, authorizedotnet

### Pattern 4: Actions-Based API (Alternative)

**Use for**: Payment processors that expose transaction history

```rust
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/payments/{}/actions", base_url, payment_id))
}
```

**Examples**: checkout

## Authentication Patterns

### Basic Authentication (Most Common)

```rust
fn build_request(&self, req: &RouterDataV2<RSync, ...>) -> CustomResult<...> {
    let auth = {ConnectorName}AuthType::try_from(&req.connector_auth_type)?;
    let encoded_auth = base64::encode(format!("{}:{}", auth.api_key, auth.secret));
    
    Ok(RequestBuilder::new()
        .method(Method::Get)
        .url(&self.get_url(req)?)
        .attach_default_headers()
        .header("Authorization", format!("Basic {}", encoded_auth))
        .build())
}
```

### Bearer Token Authentication

```rust
fn build_request(&self, req: &RouterDataV2<RSync, ...>) -> CustomResult<...> {
    let auth = {ConnectorName}AuthType::try_from(&req.connector_auth_type)?;
    
    Ok(RequestBuilder::new()
        .method(Method::Get)
        .url(&self.get_url(req)?)
        .attach_default_headers()
        .header("Authorization", format!("Bearer {}", auth.token))
        .build())
}
```

### Custom Header Authentication

```rust
fn build_request(&self, req: &RouterDataV2<RSync, ...>) -> CustomResult<...> {
    let auth = {ConnectorName}AuthType::try_from(&req.connector_auth_type)?;
    
    Ok(RequestBuilder::new()
        .method(Method::Get)
        .url(&self.get_url(req)?)
        .attach_default_headers()
        .header("X-API-Key", auth.api_key)
        .header("X-Client-Id", auth.client_id)
        .build())
}
```

### HMAC Signature Authentication (Advanced)

```rust
fn build_request(&self, req: &RouterDataV2<RSync, ...>) -> CustomResult<...> {
    let auth = {ConnectorName}AuthType::try_from(&req.connector_auth_type)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().to_string();
    let request_id = Uuid::new_v4().to_string();
    
    let payload = self.get_request_body(req)?;
    let signature = self.generate_signature(&auth.secret, &timestamp, &payload)?;
    
    Ok(RequestBuilder::new()
        .method(Method::Post)
        .url(&self.get_url(req)?)
        .attach_default_headers()
        .header("Authorization", format!("HMAC {}", signature))
        .header("X-Timestamp", timestamp)
        .header("X-Request-ID", request_id)
        .body(payload)
        .build())
}
```

## URL Construction Patterns

### Pattern 1: RESTful Resource URL
```rust
// /refunds/{refund_id}
Ok(format!("{}/refunds/{}", base_url, connector_refund_id))
```

### Pattern 2: Nested Resource URL
```rust
// /orders/{order_id}/transactions/{transaction_id}
Ok(format!("{}/orders/{}/transactions/{}", 
    base_url, 
    order_id_from_metadata, 
    connector_refund_id))
```

### Pattern 3: Query-Based URL
```rust
// /order/getbyreference/{reference_id}
Ok(format!("{}/order/getbyreference/{}", base_url, connector_refund_id))
```

### Pattern 4: Single Endpoint URL
```rust
// /api/gateway (POST with operation type)
Ok(format!("{}/api/gateway", base_url))
```

## Status Mapping Patterns

### String-Based Status Mapping
```rust
impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        match status.as_str() {
            "completed" | "processed" | "settled" => Self::Success,
            "pending" | "processing" | "submitted" => Self::Pending,
            "failed" | "declined" | "cancelled" => Self::Failure,
            _ => Self::Pending, // Default to pending for unknown statuses
        }
    }
}
```

### Enum-Based Status Mapping
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}RefundStatus {
    Succeeded,
    Failed,
    Pending,
    Cancelled,
}

impl From<{ConnectorName}RefundStatus> for common_enums::RefundStatus {
    fn from(status: {ConnectorName}RefundStatus) -> Self {
        match status {
            {ConnectorName}RefundStatus::Succeeded => Self::Success,
            {ConnectorName}RefundStatus::Failed | 
            {ConnectorName}RefundStatus::Cancelled => Self::Failure,
            {ConnectorName}RefundStatus::Pending => Self::Pending,
        }
    }
}
```

### Code-Based Status Mapping
```rust
impl From<i32> for common_enums::RefundStatus {
    fn from(code: i32) -> Self {
        match code {
            200 | 201 => Self::Success,
            400..=499 => Self::Failure,
            500..=599 => Self::Pending, // Retry later
            _ => Self::Pending,
        }
    }
}
```

## Error Handling Patterns

### Standard JSON Error Response
```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub details: Option<serde_json::Value>,
}

impl {ConnectorName}<T> {
    fn build_error_response(
        &self,
        res: types::Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res
            .response
            .parse_struct("{ConnectorName} Error Response")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        event_builder.map(|i| i.set_error_response_body(&response));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response.error_message.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.details.map(|d| d.to_string()),
            attempt_status: None,
            connector_transaction_id: None,
        })
    }
}
```

### Complex Error Array Handling
```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub errors: Option<Vec<{ConnectorName}ErrorDetail>>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorDetail {
    pub field: Option<String>,
    pub code: String,
    pub message: String,
}

impl {ConnectorName}<T> {
    fn build_error_response(&self, res: types::Response) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response.parse_struct("Error Response")?;
        
        let error_message = if let Some(errors) = response.errors {
            errors.into_iter()
                .map(|e| format!("{}: {}", e.field.unwrap_or_default(), e.message))
                .collect::<Vec<_>>()
                .join("; ")
        } else {
            response.message.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string())
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: "CONNECTOR_ERROR".to_string(),
            message: error_message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
        })
    }
}
```

## Testing Patterns

### Unit Test Structure
```rust
#[cfg(test)]
mod rsync_tests {
    use super::*;
    use crate::connector_auth::ConnectorAuthType;

    #[test]
    fn test_rsync_request_transformation() {
        let connector_refund_id = "ref_12345".to_string();
        let router_data = RouterDataV2 {
            request: RefundSyncData {
                connector_refund_id: connector_refund_id.clone(),
            },
            connector_auth_type: ConnectorAuthType::HeaderKey {
                api_key: "test_key".to_string(),
            },
            ..Default::default()
        };

        let request = {ConnectorName}RefundSyncRequest::try_from(&router_data).unwrap();
        assert_eq!(request.refund_id, connector_refund_id);
    }

    #[test]
    fn test_rsync_response_transformation() {
        let response = {ConnectorName}RefundSyncResponse {
            id: "ref_12345".to_string(),
            status: {ConnectorName}RefundStatus::Success,
            amount: Some(100.0),
            currency: Some("USD".to_string()),
        };

        let router_data = RouterDataV2::default();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data).unwrap();
        assert!(result.response.is_ok());
        
        if let Ok(refunds_response) = result.response {
            assert_eq!(refunds_response.connector_refund_id, "ref_12345");
            assert_eq!(refunds_response.refund_status, common_enums::RefundStatus::Success);
        }
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(
            common_enums::RefundStatus::from({ConnectorName}RefundStatus::Success),
            common_enums::RefundStatus::Success
        );
        assert_eq!(
            common_enums::RefundStatus::from({ConnectorName}RefundStatus::Pending),
            common_enums::RefundStatus::Pending
        );
        assert_eq!(
            common_enums::RefundStatus::from({ConnectorName}RefundStatus::Failed),
            common_enums::RefundStatus::Failure
        );
    }

    #[test]
    fn test_url_construction() {
        let connector = {ConnectorName}::new();
        let router_data = RouterDataV2 {
            request: RefundSyncData {
                connector_refund_id: "ref_12345".to_string(),
            },
            ..Default::default()
        };

        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("ref_12345"));
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_empty_refund_id_error() {
        let connector = {ConnectorName}::new();
        let router_data = RouterDataV2 {
            request: RefundSyncData {
                connector_refund_id: String::new(),
            },
            ..Default::default()
        };

        let result = connector.get_url(&router_data);
        assert!(result.is_err());
    }
}
```

### Integration Test Pattern
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_rsync_flow_integration() {
        let connector = {ConnectorName}::new();
        let auth = ConnectorAuthType::HeaderKey {
            api_key: "test_key".to_string(),
        };
        
        // Mock successful response
        let mock_response = types::Response {
            response: r#"{
                "id": "ref_12345",
                "status": "success",
                "amount": 100.0,
                "currency": "USD"
            }"#.to_string(),
            status_code: 200,
            headers: None,
        };

        let router_data = RouterDataV2 {
            request: RefundSyncData {
                connector_refund_id: "ref_12345".to_string(),
            },
            connector_auth_type: auth,
            ..Default::default()
        };

        let result = connector.handle_response_v2(&router_data, None, mock_response).unwrap();
        assert!(result.response.is_ok());
    }
}
```

## Integration Checklist

### ✅ Implementation Completeness

- [ ] **Trait Implementations**: RefundSyncV2, ConnectorIntegrationV2<RSync>, SourceVerification
- [ ] **HTTP Method**: Defined in `get_http_method()` (usually GET)
- [ ] **URL Construction**: Implemented in `get_url()` with refund ID validation
- [ ] **Request Body**: Implemented in `get_request_body()` (None for GET, structured for POST)
- [ ] **Response Handling**: Implemented in `handle_response_v2()` with proper transformation
- [ ] **Error Handling**: Implemented in `get_error_response()` with structured error parsing
- [ ] **Authentication**: Properly integrated with connector auth type
- [ ] **Status Mapping**: Complete mapping from connector statuses to RefundStatus enum

### ✅ Data Structures

- [ ] **Request Structure**: Defined if POST method (empty for GET)
- [ ] **Response Structure**: Complete with all necessary fields
- [ ] **Status Enum**: All possible connector statuses covered
- [ ] **Error Response**: Structured error response parsing
- [ ] **Transformations**: TryFrom implementations for request/response conversion

### ✅ Quality Assurance

- [ ] **Validation**: Refund ID validation and error handling
- [ ] **Error Cases**: Comprehensive error response handling
- [ ] **Status Defaults**: Safe default status mapping for unknown cases
- [ ] **Null Safety**: Proper handling of optional fields
- [ ] **Type Safety**: Strong typing throughout the flow

### ✅ Testing Coverage

- [ ] **Unit Tests**: Request/response transformations, status mapping, URL construction
- [ ] **Integration Tests**: End-to-end flow testing with mock responses
- [ ] **Error Tests**: Error response handling and edge cases
- [ ] **Validation Tests**: Invalid input handling

### ✅ Documentation

- [ ] **Code Comments**: Clear documentation of complex logic
- [ ] **Status Mapping**: Documented mapping from connector to UCS statuses
- [ ] **Error Codes**: Documented error codes and their meanings
- [ ] **Dependencies**: Clear dependency requirements

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM FOR RSYNC FLOWS**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal` | **Always required** - Used in struct names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal` | **Always required** - Used in config keys |
| `{base_url}` | Connector API base URL | `https://api.stripe.com` | **Always required** - API endpoint base |
| `{HttpMethod}` | HTTP method for RSync | `Method::Get`, `Method::Post` | **Always required** - Request method |
| `{AuthType}` | Authentication mechanism | `Basic`, `Bearer`, `ApiKey` | **Always required** - Auth pattern |
| `{StatusEnum}` | Connector status enumeration | `StripeRefundStatus` | **Usually required** - Status mapping |
| `{AmountType}` | Amount representation type | `FloatMajorUnit`, `StringMinorUnit` | **Sometimes required** - Currency handling |

### Real-World Examples

**Example 1: Simple REST API (Stripe-style)**
```bash
{ConnectorName} → Stripe
{connector_name} → stripe
{base_url} → https://api.stripe.com
{HttpMethod} → Method::Get
{AuthType} → Basic
{StatusEnum} → StripeRefundStatus
URL Pattern: {base_url}/v1/refunds/{refund_id}
```

**Example 2: Complex Authentication (Fiserv-style)**
```bash
{ConnectorName} → Fiserv
{connector_name} → fiserv
{base_url} → https://api.fiserv.com
{HttpMethod} → Method::Post
{AuthType} → HMAC
{StatusEnum} → FiservTransactionStatus
URL Pattern: {base_url}/ch/payments/v1/transaction-inquiry
```

**Example 3: GraphQL API (Braintree-style)**
```bash
{ConnectorName} → Braintree
{connector_name} → braintree
{base_url} → https://api.braintreegateway.com
{HttpMethod} → Method::Post
{AuthType} → Basic
{StatusEnum} → BraintreeRefundStatus
URL Pattern: {base_url}/graphql
```

## Troubleshooting Guide

### Common Issues and Solutions

#### Issue 1: Empty Refund ID Error
**Error**: `MissingRequiredField { field_name: "connector_refund_id" }`
**Cause**: RSync called with empty or missing refund ID
**Solution**: Add refund ID validation in URL construction

```rust
// Before (incorrect)
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/refunds/{}", base_url, req.request.connector_refund_id))
}

// After (correct)
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    let refund_id = req.request.connector_refund_id.clone();
    if refund_id.is_empty() {
        return Err(errors::IntegrationError::MissingRequiredField {
            field_name: "connector_refund_id",
        , context: Default::default() });
    }
    Ok(format!("{}/refunds/{}", base_url, refund_id))
}
```

#### Issue 2: Unknown Status Mapping
**Error**: Connector returns unexpected status values
**Cause**: Status enum doesn't cover all possible connector statuses
**Solution**: Add comprehensive status mapping with safe defaults

```rust
// Before (incorrect - panics on unknown status)
impl From<String> for common_enums::RefundStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "success" => Self::Success,
            "failed" => Self::Failure,
            // Missing default case
        }
    }
}

// After (correct - handles unknown statuses)
impl From<String> for common_enums::RefundStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "success" | "completed" | "settled" => Self::Success,
            "failed" | "declined" | "cancelled" => Self::Failure,
            "pending" | "processing" | "submitted" => Self::Pending,
            _ => {
                router_env::logger::warn!("Unknown refund status: {}", status);
                Self::Pending // Safe default
            }
        }
    }
}
```

#### Issue 3: Response Deserialization Failed
**Error**: `ResponseDeserializationFailed`
**Cause**: Response structure doesn't match actual API response
**Solution**: Use flexible response structures with optional fields

```rust
// Before (incorrect - rigid structure)
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}RefundSyncResponse {
    pub id: String,
    pub status: String,
    pub amount: f64, // Required field might be missing
}

// After (correct - flexible structure)
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}RefundSyncResponse {
    pub id: String,
    pub status: String,
    pub amount: Option<f64>, // Optional field
    pub currency: Option<String>,
    #[serde(flatten)]
    pub additional_fields: serde_json::Value, // Catch unknown fields
}
```

#### Issue 4: Authentication Failures
**Error**: `401 Unauthorized` or `403 Forbidden`
**Cause**: Incorrect authentication header format or missing credentials
**Solution**: Follow connector-specific authentication patterns

```rust
// Before (incorrect - generic auth)
fn build_request(&self, req) -> CustomResult<RequestBuilder, IntegrationError> {
    Ok(RequestBuilder::new()
        .header("Authorization", "Bearer token"))
}

// After (correct - connector-specific auth)
fn build_request(&self, req) -> CustomResult<RequestBuilder, IntegrationError> {
    let auth = {ConnectorName}AuthType::try_from(&req.connector_auth_type)?;
    let auth_header = match auth.auth_type {
        AuthType::Basic => format!("Basic {}", base64::encode(format!("{}:{}", auth.key, auth.secret))),
        AuthType::Bearer => format!("Bearer {}", auth.token),
        AuthType::ApiKey => auth.api_key,
    };
    
    Ok(RequestBuilder::new()
        .header("Authorization", auth_header))
}
```

#### Issue 5: Metadata Dependency Errors
**Error**: Missing required metadata for URL construction
**Cause**: Some connectors require additional metadata (order_id, merchant_id, etc.)
**Solution**: Extract and validate required metadata

```rust
// Before (incorrect - assumes refund_id is sufficient)
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/refunds/{}", base_url, req.request.connector_refund_id))
}

// After (correct - handles metadata dependencies)
fn get_url(&self, req) -> CustomResult<String, IntegrationError> {
    let refund_id = req.request.connector_refund_id.clone();
    
    // Extract order_id from connector metadata if required
    let order_id = req.connector_meta_data
        .get_required_value("order_id")
        .change_context(errors::IntegrationError::MissingConnectorMetaData)?;
    
    Ok(format!("{}/orders/{}/transactions/{}", base_url, order_id, refund_id))
}
```

### Performance Considerations

#### Optimization 1: Response Preprocessing
Some connectors require response preprocessing for special formatting:

```rust
fn handle_response_v2(&self, data, event_builder, res) -> CustomResult<...> {
    // Preprocess response if needed (e.g., remove BOM, fix encoding)
    let processed_response = if self.requires_preprocessing() {
        self.preprocess_response(res.response)?
    } else {
        res.response
    };
    
    let response: {ConnectorName}RefundSyncResponse = processed_response
        .parse_struct("{ConnectorName} RSync Response")?;
    // ... rest of handling
}
```

#### Optimization 2: Caching Strategy
For high-frequency RSync operations, consider implementing request caching:

```rust
fn should_skip_sync(&self, req: &RouterDataV2<RSync, ...>) -> bool {
    // Skip sync if recently queried (within last 30 seconds)
    let cache_key = format!("rsync_{}", req.request.connector_refund_id);
    self.cache.get(&cache_key).map(|timestamp| {
        SystemTime::now().duration_since(timestamp).unwrap().as_secs() < 30
    }).unwrap_or(false)
}
```

### Best Practices Summary

1. **Always validate refund IDs** before making API calls
2. **Use safe defaults** for unknown status values
3. **Make response fields optional** when possible
4. **Implement comprehensive error handling** for all error types
5. **Follow connector-specific authentication patterns** exactly
6. **Handle metadata dependencies** gracefully
7. **Log unknown statuses** for monitoring and debugging
8. **Test edge cases** thoroughly with unit and integration tests
9. **Document status mappings** for future maintenance
10. **Consider performance implications** for high-frequency operations

> **📖 Related Patterns:** For refund flow implementation, see [`pattern_refund.md`](./pattern_refund.md). For authorization flow implementation, see [`pattern_authorize.md`](./pattern_authorize.md).

---

**✅ Complete Implementation**: Following this pattern guide will result in a fully functional, production-ready RSync flow implementation that integrates seamlessly with the Grace-UCS connector framework and provides reliable refund status synchronization capabilities.