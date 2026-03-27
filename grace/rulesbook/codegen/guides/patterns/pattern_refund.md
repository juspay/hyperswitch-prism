# Refund Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing refund flows in **ANY** payment connector. These patterns are extracted from successful refund implementations (Worldpay, Adyen, Stripe) and include real-world solutions to common issues encountered during refund flow integration.

## 🚀 Quick Start Guide

To implement refund flows in a new connector:

1. **Add Flow Declarations**: Add `Refund` and `RSync` to your existing connector macro setup
2. **Create Request/Response Structures**: Often simpler than payment structures
3. **Handle Empty Bodies**: Many connectors require empty request bodies for full refunds
4. **Verify API Responses**: Refund responses typically differ from payment responses
5. **Implement Status Mapping**: Refund statuses need separate mapping logic

### Example: Adding Refunds to Existing Connector

```bash
# Add to existing connector:
- Add Refund/RSync to macro flow declarations
- Create {ConnectorName}RefundRequest {} (often empty)
- Create {ConnectorName}RefundResponse (verify actual API response)
- Implement URL pattern: /payments/{id}/refunds
- Map refund-specific statuses
```

**✅ Result**: Working refund flow integrated into existing connector in ~20 minutes

## Table of Contents

1. [Overview](#overview)
2. [Refund Flow Architecture](#refund-flow-architecture)
3. [Request/Response Patterns](#requestresponse-patterns)
4. [Common API Variations](#common-api-variations)
5. [URL Construction Patterns](#url-construction-patterns)
6. [Status Mapping](#status-mapping)
7. [Error Handling](#error-handling)
8. [Testing Strategies](#testing-strategies)
9. [Integration with Existing Connectors](#integration-with-existing-connectors)
10. [Troubleshooting Guide](#troubleshooting-guide)

## Overview

Refund flows process refund requests for previously successful payments. Unlike payment flows, refund flows are typically simpler but have unique characteristics:

- **Simpler Request Structure**: Often just amount and currency (or empty for full refunds)
- **Different Response Schema**: Usually simpler than payment responses
- **Unique Status Mapping**: Refund statuses (pending, success, failure) vs payment statuses
- **Different URL Patterns**: Usually `/payments/{id}/refunds` format
- **Sync Requirements**: RSync flow for checking refund status

### Key Components:
- **Refund Flow**: Process refund requests
- **RSync Flow**: Check refund status
- **Empty Body Handling**: Many connectors require empty bodies for full refunds
- **Transaction ID Extraction**: From response links or direct fields

## Refund Flow Architecture

### Flow Relationship
```
Payment Flow (Authorize/Sale) → Capture → Refund ← RSync
                                   ↓        ↑
                               Original Txn  Status Check
```

### Data Flow
1. **Refund Request**: Contains payment ID, amount (optional), currency
2. **Transform to Connector Format**: Often empty body or minimal data
3. **API Call**: POST to `/payments/{id}/refunds`
4. **Process Response**: Map connector status to standard refund status
5. **RSync**: GET request to check refund status

## Request/Response Patterns

### Common Request Patterns

#### Pattern 1: Empty Body Refunds (Worldpay, PayPal, many others)
```rust
// For full refunds - empty request body (most common for simple refunds)
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for {ConnectorName}RefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        _item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        // Empty body for full refunds - connector handles amount internally
        Ok(Self {})
    }
}
```

#### Pattern 2: Amount-Required Refunds (Adyen, Stripe, Square)
```rust
// Always require amount and reference - even for full refunds
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundRequest {
    pub merchant_account: Secret<String>,
    pub amount: Amount,
    pub merchant_refund_reason: Option<String>,
    pub reference: String,
}

// Amount structure for detailed connectors
#[derive(Debug, Clone, Serialize)]
pub struct Amount {
    pub currency: String,
    pub value: MinorUnit,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for {ConnectorName}RefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let auth_type = AuthType::try_from(&item.router_data.connector_auth_type)?;
        let router_data = &item.router_data;
        
        Ok(Self {
            merchant_account: auth_type.merchant_account,
            amount: Amount {
                currency: router_data.request.currency.to_string(),
                value: router_data.request.minor_refund_amount,
            },
            merchant_refund_reason: router_data.request.reason.clone(),
            reference: router_data.request.refund_id.clone(),
        })
    }
}
```

#### Pattern 3: Metadata-Rich Refunds (Checkout.com, Authorize.Net)
```rust
// Support extensive metadata, idempotency, and partial refund tracking
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundRequest {
    pub amount: Option<MinorUnit>,
    pub currency: Option<String>,
    pub reason: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub idempotency_key: Option<String>,
    pub refund_type: Option<RefundType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RefundType {
    Full,
    Partial,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for {ConnectorName}RefundRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        Ok(Self {
            amount: Some(router_data.request.minor_refund_amount),
            currency: Some(router_data.request.currency.to_string()),
            reason: router_data.request.reason.clone(),
            description: None,
            metadata: router_data.request.metadata.clone(),
            idempotency_key: Some(router_data.request.refund_id.clone()),
            refund_type: Some(RefundType::Full), // Determine based on amount vs original
        })
    }
}
```

### Common Response Patterns

#### Pattern 1: Simple Response (Minimal Fields)
```rust
// Simple style: Only essential fields like status and links
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}RefundResponse {
    pub outcome: String,
    #[serde(rename = "_links")]
    pub links: {ConnectorName}Links,
}
```

#### Pattern 2: Detailed Response (Full Details)
```rust
// Comprehensive style: Full refund details and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}RefundResponse {
    pub id: String,
    pub status: {ConnectorName}RefundStatus,
    pub amount: MinorUnit,
    pub currency: String,
    pub created: Option<i64>,
    pub reason: Option<String>,
    pub receipt_number: Option<String>,
}
```

### Critical Response Schema Rule
**⚠️ IMPORTANT**: Always verify actual API responses vs documentation. Refund responses often differ significantly from payment responses.

**Example Issue**: Some connectors include transaction references in payment responses but not in refund responses.

```rust
// WRONG - Assumed consistency with payment response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}RefundResponse {
    pub status: String,
    pub transaction_reference: String, // ❌ This field might not exist!
    pub links: {ConnectorName}Links,
}

// CORRECT - Based on actual API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {ConnectorName}RefundResponse {
    pub status: String, // ✅ Only fields that actually exist
    pub links: {ConnectorName}Links,
}
```

## Common API Variations

### Variation 1: Empty Body Refunds
**Connectors**: Worldpay, PayPal, many others
**Characteristic**: Full refunds require empty POST body
**Implementation**:
```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {}

// URL: POST /payments/{payment_id}/refunds
// Body: {} (empty)
```

### Variation 2: Amount-Required Refunds
**Connectors**: Stripe, Adyen, Square
**Characteristic**: Always require amount (even for full refunds)
**Implementation**:
```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {
    pub amount: MinorUnit,
    pub currency: String,
}

// URL: POST /refunds
// Body: {"amount": 1000, "currency": "USD", "charge": "ch_xxx"}
```

### Variation 3: Metadata-Rich Refunds
**Connectors**: Checkout.com, Authorize.Net
**Characteristic**: Support extensive metadata and reason codes
**Implementation**:
```rust
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}RefundRequest {
    pub amount: Option<MinorUnit>,
    pub reason: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub idempotency_key: Option<String>,
}
```

## Authentication Patterns for Refunds

### Pattern 1: Bearer Token Authentication (Checkout, Stripe)
```rust
#[derive(Debug, Clone)]
pub struct {ConnectorName}AuthType {
    pub api_secret: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::BodyKey { api_key, .. } => Ok(Self {
                api_secret: api_key.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Header construction
fn get_auth_header(
    &self,
    auth_type: &ConnectorAuthType,
) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    Ok(vec![(
        "Authorization".to_string(),
        format!("Bearer {}", auth.api_secret.peek()).into_masked(),
    )])
}
```

### Pattern 2: API Key Header Authentication (Adyen)
```rust
#[derive(Debug, Clone)]
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
    pub merchant_account: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self {
                api_key: api_key.to_owned(),
                merchant_account: key1.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Header construction
fn get_auth_header(
    &self,
    auth_type: &ConnectorAuthType,
) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    Ok(vec![(
        "X-Api-Key".to_string(),
        auth.api_key.into_masked(),
    )])
}
```

### Pattern 3: Basic Authentication (Worldpay, traditional gateways)
```rust
#[derive(Debug, Clone)]
pub struct {ConnectorName}AuthType {
    pub basic_auth: Secret<String>,
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::BodyKey { api_key, key1 } => {
                let credentials = format!("{}:{}", key1.peek(), api_key.peek());
                let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
                Ok(Self {
                    basic_auth: Secret::new(format!("Basic {}", encoded)),
                })
            }
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// Header construction
fn get_auth_header(
    &self,
    auth_type: &ConnectorAuthType,
) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
    let auth = {ConnectorName}AuthType::try_from(auth_type)?;
    Ok(vec![(
        "Authorization".to_string(),
        auth.basic_auth.into_masked(),
    )])
}
```

## URL Construction Patterns

### Pattern 1: RESTful Payment Subresource (Most Common - Adyen, Checkout, Worldpay)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/refunds"))
}
```

### Pattern 2: API Versioned Endpoints (Adyen style)
```rust
const API_VERSION: &str = "v68";

fn get_url(
    &self,
    req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}{API_VERSION}/payments/{payment_id}/refunds"))
}
```

### Pattern 3: Dedicated Refunds Endpoint (Stripe style)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    Ok(format!("{base_url}/refunds"))
    // Payment ID goes in request body instead
}
```

### Pattern 4: Transaction-Based Endpoint (Alternative)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let transaction_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/transactions/{transaction_id}/refund"))
}
```

### RSync URL Patterns

#### Pattern 1: Direct Refund ID (Worldpay, Stripe)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let refund_id = req.request.connector_refund_id.clone();
    Ok(format!("{base_url}/refunds/{refund_id}"))
}
```

#### Pattern 2: Actions-Based Sync (Checkout - unique!)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/actions"))
    // Returns all actions including refunds
}
```

#### Pattern 3: Payment + Refund ID (Adyen style)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    let refund_id = req.request.connector_refund_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/refunds/{refund_id}"))
}
```

#### Pattern 4: Empty Implementation (Not all connectors support RSync)
```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for {ConnectorName}<T>
{
    // Empty - connector doesn't support refund status checking
    // Rely on webhooks or assume success after successful refund initiation
}
```

## Status Mapping

### Standard Refund Statuses
```rust
pub enum RefundStatus {
    Pending,    // Refund initiated, processing
    Success,    // Refund completed
    Failure,    // Refund failed
}
```

### CRITICAL: Understanding Pending vs Success Status

**⚠️ IMPORTANT - ASYNCHRONOUS REFUND PROCESSING**

Many payment connectors accept refund requests but process them **asynchronously**. This is a critical distinction that affects status mapping:

**Key Principle**: A `200 OK` response often means **"refund accepted"** NOT **"refund completed"**

#### When to Use RefundStatus::Pending

Use `Pending` when the connector:
- Returns a minimal response (typically just an ID or status like "accepted")
- Processes refunds asynchronously in the background
- Requires a subsequent RSync call to verify actual completion
- Returns status values like: "sentForRefund", "pending", "processing", "accepted", "initiated"

**Example Pattern**:
```rust
// Connector returns minimal response - use Pending
#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: String, // "accepted" or "pending"
}

impl TryFrom<ResponseRouterData<RefundResponse, ...>> for RouterDataV2<...> {
    fn try_from(item: ResponseRouterData<RefundResponse, ...>) -> Result<Self, Self::Error> {
        // Response is minimal - connector will process asynchronously
        let refund_status = match item.response.status.as_str() {
            "accepted" | "pending" | "sentForRefund" => RefundStatus::Pending,
            // ... other mappings
        };

        // Return Pending - actual status verified via RSync later
        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.id,
            refund_status, // Pending - not yet completed
            status_code: item.http_code,
        });
        Ok(router_data)
    }
}
```

#### When to Use RefundStatus::Success

Use `Success` when the connector:
- Returns detailed refund confirmation in the initial response
- Provides clear "completed" or "succeeded" status
- Includes full refund details (amount, timestamp, receipt number, etc.)
- Processes refunds synchronously and confirms completion immediately

**Example Pattern**:
```rust
// Connector returns detailed response - can use Success
#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: String, // "succeeded" or "completed"
    pub amount: MinorUnit,
    pub currency: String,
    pub receipt_number: Option<String>,
    pub completed_at: Option<i64>,
}

impl TryFrom<ResponseRouterData<RefundResponse, ...>> for RouterDataV2<...> {
    fn try_from(item: ResponseRouterData<RefundResponse, ...>) -> Result<Self, Self::Error> {
        // Response is detailed - refund completed synchronously
        let refund_status = match item.response.status.as_str() {
            "succeeded" | "completed" | "refunded" => RefundStatus::Success,
            "pending" | "processing" => RefundStatus::Pending,
            // ... other mappings
        };

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id: item.response.id,
            refund_status,
            status_code: item.http_code,
        });
        Ok(router_data)
    }
}
```

#### Decision Flow Chart

```
Refund Response Received (200 OK)
    |
    ├─> Response has minimal data (only ID/status)?
    |   └─> YES: Use RefundStatus::Pending
    |       └─> Implement RSync to verify actual status later
    |
    └─> Response has detailed confirmation?
        └─> YES: Check status field
            ├─> "succeeded"/"completed"/"refunded" → RefundStatus::Success
            ├─> "pending"/"processing"/"initiated" → RefundStatus::Pending
            └─> "failed"/"declined"/"refused" → RefundStatus::Failure
```

#### Real-World Examples

**Worldpay (Async Processing)**:
```rust
// Worldpay accepts refund but processes asynchronously
RefundResponse {
    outcome: "sentForRefund", // ← Not yet completed!
    _links: {...}
}
// Map to: RefundStatus::Pending
// Verify later via RSync which returns actual "refunded" status
```

**Stripe (Can be Sync or Async)**:
```rust
// Stripe may process immediately or asynchronously
RefundResponse {
    id: "re_123",
    status: "succeeded", // ← Completed immediately
    amount: 1000,
    // ... full details
}
// Map to: RefundStatus::Success

// OR async processing:
RefundResponse {
    id: "re_456",
    status: "pending", // ← Will process later
    // ... minimal details
}
// Map to: RefundStatus::Pending
```

**Adyen (Async with Confirmation)**:
```rust
// Adyen returns confirmation but processes asynchronously
RefundResponse {
    psp_reference: "123456",
    status: "[refund-received]", // ← Accepted, not completed
    // ... minimal response
}
// Map to: RefundStatus::Pending
// Actual completion confirmed via webhook or RSync
```

### Common Connector Status Mappings

#### Worldpay
```rust
fn map_worldpay_refund_status(status: &str) -> RefundStatus {
    match status {
        "sentForRefund" => RefundStatus::Pending,
        "refunded" => RefundStatus::Success,
        "refused" | "failed" => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}
```

#### Stripe
```rust
fn map_stripe_refund_status(status: &str) -> RefundStatus {
    match status {
        "pending" => RefundStatus::Pending,
        "succeeded" => RefundStatus::Success,
        "failed" | "canceled" => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}
```

#### Adyen
```rust
fn map_adyen_refund_status(status: &str) -> RefundStatus {
    match status {
        "[refund-received]" => RefundStatus::Pending,
        "received" => RefundStatus::Success,
        "error" | "refused" => RefundStatus::Failure,
        _ => RefundStatus::Pending,
    }
}
```

### Response Transformation Pattern
```rust
impl TryFrom<ResponseRouterData<{ConnectorName}RefundResponse, RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}RefundResponse, RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>,
    ) -> Result<Self, Self::Error> {
        // Map connector-specific status to standard RefundStatus
        let refund_status = match item.response.status.as_str() {
            "pending" | "processing" | "initiated" => RefundStatus::Pending,
            "completed" | "success" | "succeeded" => RefundStatus::Success,
            "failed" | "declined" | "refused" => RefundStatus::Failure,
            _ => RefundStatus::Pending, // Default to pending for unknown statuses
        };

        // Extract refund ID - be flexible about sources
        let connector_refund_id = extract_refund_id(&item.response)
            .unwrap_or_else(|| "unknown".to_string());

        let mut router_data = item.router_data;
        router_data.response = Ok(RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
        });
        Ok(router_data)
    }
}

// Helper function to extract refund ID from various sources
fn extract_refund_id(response: &{ConnectorName}RefundResponse) -> Option<String> {
    // Try multiple possible sources for refund ID
    response.id.clone()
        .or_else(|| response.refund_id.clone())
        .or_else(|| extract_id_from_links(&response.links))
        .or_else(|| response.reference.clone())
}
```

## Error Handling

### Refund-Specific Error Patterns

#### Pattern 1: Already Refunded Errors
```rust
fn handle_refund_errors(error_code: &str) -> Option<RefundStatus> {
    match error_code {
        "already_refunded" | "charge_already_refunded" => Some(RefundStatus::Failure),
        "insufficient_funds" | "refund_amount_exceeds_charge" => Some(RefundStatus::Failure),
        "invalid_charge" | "charge_not_found" => Some(RefundStatus::Failure),
        _ => None,
    }
}
```

#### Pattern 2: Amount Validation Errors
```rust
fn validate_refund_amount(
    refund_amount: MinorUnit,
    original_amount: MinorUnit,
) -> Result<(), ConnectorError> {
    if refund_amount > original_amount {
        return Err(ConnectorError::InvalidRequestData {
            message: "Refund amount cannot exceed original payment amount".to_string(),
        });
    }
    if refund_amount <= MinorUnit::zero() {
        return Err(ConnectorError::InvalidRequestData {
            message: "Refund amount must be greater than zero".to_string(),
        });
    }
    Ok(())
}
```

#### Pattern 3: NotSupported Errors (CRITICAL)

**⚠️ IMPORTANT**: Use specific `NotSupported` errors when a connector doesn't support certain refund scenarios. Always specify **what aspect** of the refund is not supported.

**Common NotSupported Scenarios**:

```rust
// Example: Connector doesn't support partial refunds
fn try_from(
    item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
) -> Result<Self, Self::Error> {
    let router_data = &item.router_data;

    // Check if this is a partial refund
    if is_partial_refund(&router_data.request) {
        return Err(ConnectorError::NotSupported {
            message: "Partial refunds are not supported by this connector".to_string(),
            connector: "{ConnectorName}".to_string(),
        }
        .into());
    }

    // Continue with full refund processing
    Ok(Self {})
}

fn is_partial_refund(request: &RefundsData) -> bool {
    // Compare refund amount with original payment amount
    request.minor_refund_amount < request.payment_amount
}
```

**Example: Refund reasons not supported**:
```rust
fn try_from(
    item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
) -> Result<Self, Self::Error> {
    let router_data = &item.router_data;

    // Check if refund reason is provided but not supported
    if router_data.request.reason.is_some() {
        return Err(ConnectorError::NotSupported {
            message: "Refund reasons are not supported by this connector".to_string(),
            connector: "{ConnectorName}".to_string(),
        }
        .into());
    }

    Ok(Self {
        // ... build request without reason
    })
}
```

**Example: Refunds not supported for certain payment methods**:
```rust
fn try_from(
    item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
) -> Result<Self, Self::Error> {
    let router_data = &item.router_data;

    // Check if payment method supports refunds
    match &router_data.request.payment_method {
        PaymentMethod::Wallet(WalletData::ApplePay) => {
            return Err(ConnectorError::NotSupported {
                message: "Refunds for Apple Pay are not supported by this connector".to_string(),
                connector: "{ConnectorName}".to_string(),
            }
            .into());
        }
        PaymentMethod::BankTransfer(_) => {
            return Err(ConnectorError::NotSupported {
                message: "Refunds for bank transfers are not supported by this connector".to_string(),
                connector: "{ConnectorName}".to_string(),
            }
            .into());
        }
        _ => {
            // Supported payment method, continue
        }
    }

    Ok(Self {
        // ... build request
    })
}
```

**Example: Time-based refund restrictions**:
```rust
fn try_from(
    item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
) -> Result<Self, Self::Error> {
    let router_data = &item.router_data;

    // Check if payment is too old for refund (e.g., 180 days)
    if let Some(payment_date) = router_data.request.payment_created_at {
        let days_since_payment = calculate_days_since(payment_date);

        if days_since_payment > 180 {
            return Err(ConnectorError::NotSupported {
                message: format!(
                    "Refunds are not supported for payments older than 180 days (payment is {} days old)",
                    days_since_payment
                ),
                connector: "{ConnectorName}".to_string(),
            }
            .into());
        }
    }

    Ok(Self {
        // ... build request
    })
}
```

**Example: Currency-based restrictions**:
```rust
fn try_from(
    item: {ConnectorName}RouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
) -> Result<Self, Self::Error> {
    let router_data = &item.router_data;

    // Some connectors don't support refunds for certain currencies
    const UNSUPPORTED_CURRENCIES: &[&str] = &["BTC", "ETH", "USDT"];

    if UNSUPPORTED_CURRENCIES.contains(&router_data.request.currency.to_string().as_str()) {
        return Err(ConnectorError::NotSupported {
            message: format!(
                "Refunds for {} currency are not supported by this connector",
                router_data.request.currency
            ),
            connector: "{ConnectorName}".to_string(),
        }
        .into());
    }

    Ok(Self {
        // ... build request
    })
}
```

#### Best Practices for NotSupported Errors

1. **Be Specific**: Always explain what aspect is not supported
   - ❌ Bad: "Operation not supported"
   - ✅ Good: "Partial refunds are not supported by this connector"

2. **Check Early**: Validate support before making API calls
   ```rust
   // Check support in try_from, not after API call
   fn try_from(...) -> Result<Self, Self::Error> {
       // Validate support first
       validate_refund_support(&item)?;

       // Then build request
       Ok(Self { ... })
   }
   ```

3. **Provide Context**: Include relevant details in error message
   ```rust
   ConnectorError::NotSupported {
       message: format!(
           "Refund amount ${} exceeds maximum refundable amount ${} for this connector",
           refund_amount, max_amount
       ),
       connector: "{ConnectorName}".to_string(),
   }
   ```

4. **Document Limitations**: Add comments explaining why something isn't supported
   ```rust
   // {ConnectorName} only supports full refunds within 90 days of payment
   if is_partial_refund(request) || is_payment_too_old(request) {
       return Err(ConnectorError::NotSupported { ... });
   }
   ```

5. **Use Consistent Messaging**: Follow a standard format for error messages
   ```rust
   // Format: "{Feature} {qualifier} not supported by this connector"
   "Partial refunds are not supported by this connector"
   "Refund reasons are not supported by this connector"
   "Refunds for bank transfers are not supported by this connector"
   ```

## Testing Strategies

### Unit Test Structure
```rust
#[cfg(test)]
mod refund_tests {
    use super::*;

    #[test]
    fn test_refund_request_empty_body() {
        let router_data = create_test_refund_router_data();
        let refund_request = {ConnectorName}RefundRequest::try_from(&router_data);
        
        assert!(refund_request.is_ok());
        // For empty body refunds, verify serialization produces {}
        let serialized = serde_json::to_string(&refund_request.unwrap()).unwrap();
        assert_eq!(serialized, "{}");
    }

    #[test]
    fn test_refund_response_transformation() {
        let response = {ConnectorName}RefundResponse {
            status: "pending".to_string(),
            id: "refund_123".to_string(),
            links: create_test_links(),
        };

        let router_data = create_test_refund_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());
        
        let result_data = result.unwrap();
        match result_data.response {
            Ok(RefundsResponseData { refund_status, .. }) => {
                assert_eq!(refund_status, RefundStatus::Pending);
            }
            _ => panic!("Expected successful refund response"),
        }
    }

    #[test]
    fn test_refund_status_mapping() {
        // Test common status patterns across different connectors
        assert_eq!(
            map_connector_refund_status("pending"),
            RefundStatus::Pending
        );
        assert_eq!(
            map_connector_refund_status("processing"),
            RefundStatus::Pending
        );
        assert_eq!(
            map_connector_refund_status("completed"),
            RefundStatus::Success
        );
        assert_eq!(
            map_connector_refund_status("succeeded"),
            RefundStatus::Success
        );
        assert_eq!(
            map_connector_refund_status("failed"),
            RefundStatus::Failure
        );
        assert_eq!(
            map_connector_refund_status("declined"),
            RefundStatus::Failure
        );
    }

    #[test]
    fn test_url_construction() {
        let router_data = create_test_refund_router_data();
        let connector = {ConnectorName}::new();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("/payments/"));
        assert!(url.ends_with("/refunds"));
    }
}
```

### Integration Test Pattern
```rust
#[tokio::test]
async fn test_full_refund_flow() {
    // 1. Create a payment first
    let payment_response = create_test_payment().await;
    assert!(payment_response.is_ok());
    
    // 2. Process refund
    let refund_request = create_refund_request(&payment_response.unwrap());
    let refund_response = process_refund(refund_request).await;
    assert!(refund_response.is_ok());
    
    // 3. Check refund status with RSync
    let sync_response = sync_refund_status(&refund_response.unwrap()).await;
    assert!(sync_response.is_ok());
}
```

## Integration with Existing Connectors

### Adding Refunds to Existing Connector

#### Step 1: Update Connector Declaration
```rust
// Add to existing macro setup
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        // Existing flows...
        (
            flow: Authorize,
            request_body: {ConnectorName}AuthorizeRequest<T>,
            response_body: {ConnectorName}AuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        // Add refund flows
        (
            flow: Refund,
            request_body: {ConnectorName}RefundRequest,
            response_body: {ConnectorName}RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: {ConnectorName}RefundSyncRequest,
            response_body: {ConnectorName}RefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
    ],
    // ... rest of configuration
);
```

#### Step 2: Add Trait Implementations
```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::RefundExecuteV2<T> for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::RefundSyncV2 for {ConnectorName}<T>
{
}
```

#### Step 3: Add Macro Implementations
```rust
// Refund implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}RefundRequest),
    curl_response: {ConnectorName}RefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_refunds(req);
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{base_url}/payments/{payment_id}/refunds"))
        }
    }
);

// RSync implementation  
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}RefundSyncRequest),
    curl_response: {ConnectorName}RefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_refunds(req);
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{base_url}/refunds/{refund_id}"))
        }
    }
);
```

## Troubleshooting Guide

### Common Issues and Solutions

#### Issue 1: Response Deserialization Failed
**Error**: `missing field 'transactionReference'`
**Cause**: Assuming refund response matches payment response structure
**Solution**: Verify actual API response and create accurate response struct

```rust
// Before (incorrect assumption)
#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub outcome: String,
    pub transaction_reference: String, // ❌ Field doesn't exist
}

// After (verified against API)
#[derive(Debug, Deserialize)]
pub struct RefundResponse {
    pub outcome: String, // ✅ Only actual fields
    #[serde(rename = "_links")]
    pub links: Links,
}
```

#### Issue 2: Empty Request Body Not Accepted
**Error**: `400 Bad Request` with empty body
**Cause**: Connector requires specific fields even for full refunds
**Solution**: Include required fields based on connector requirements

```rust
// If empty body fails, try minimal required fields
#[derive(Debug, Serialize)]
pub struct RefundRequest {
    pub amount: MinorUnit, // Full amount from original payment
    pub currency: String,
}
```

#### Issue 3: Wrong URL Pattern
**Error**: `404 Not Found`
**Cause**: Incorrect URL construction for refunds
**Solution**: Check connector documentation for exact endpoint pattern

```rust
// Common patterns to try:
// /payments/{id}/refunds  (most common)
// /refunds (with payment_id in body)
// /transactions/{id}/refund
// /v1/charges/{id}/refunds
```

#### Issue 4: Refund ID Not Found
**Error**: Cannot extract refund ID from response
**Cause**: Different ID sources in refund vs payment responses
**Solution**: Check multiple possible sources

```rust
fn extract_refund_id(response: &RefundResponse) -> String {
    // Try multiple sources
    response.id.clone()
        .or_else(|| extract_id_from_href(&response.links.self_link.href))
        .or_else(|| response.reference.clone())
        .unwrap_or_else(|| "unknown".to_string())
}
```

### Testing Checklist

- [ ] **Request Structure**: Verify request format (empty vs with data)
- [ ] **Response Structure**: Check actual API response vs documentation
- [ ] **URL Pattern**: Test correct endpoint construction
- [ ] **Status Mapping**: Verify all possible refund statuses are handled
- [ ] **Error Handling**: Test refund-specific error scenarios
- [ ] **Full vs Partial**: Test both full and partial refunds if supported
- [ ] **RSync Flow**: Verify refund status checking works
- [ ] **Integration**: Test with real API in sandbox environment

### Best Practices

1. **Always Test with Real API**: Documentation can be outdated or incomplete
2. **Start with Empty Bodies**: Many connectors prefer empty bodies for full refunds
3. **Verify Response Schema**: Don't assume consistency with payment responses
4. **Handle Multiple ID Sources**: Refund IDs may come from different response fields
5. **Test Error Scenarios**: Refunds have unique error conditions
6. **Check Partial Refund Support**: Not all connectors support partial refunds
7. **Validate Amounts**: Ensure refund amount doesn't exceed original payment
8. **Use Proper Status Mapping**: Refund statuses differ from payment statuses

This comprehensive guide captures real-world experience implementing refund flows and provides patterns that work across multiple payment connectors. The key insight is that refund flows, while conceptually simpler, have unique characteristics that require careful attention to API specifications and response handling.
