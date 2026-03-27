# Void Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing void flows in **ANY** payment connector. These patterns are extracted from successful void implementations (Checkout, Fiserv, Authorizedotnet, Novalnet) and address the unique characteristics of void operations vs refunds.

## 🚀 Quick Start Guide

To implement void flows in a new connector:

1. **Understand Void vs Refund**: Voids cancel authorized payments before settlement; refunds return money after settlement
2. **Add Flow Declarations**: Add `Void` to your existing connector macro setup
3. **Create Request/Response Structures**: Often simpler than payment structures but may require session data
4. **Handle Timing Restrictions**: Most connectors only allow voids on authorized (non-captured) payments
5. **Implement Status Mapping**: Void-specific statuses (Voided vs VoidFailed)

### Example: Adding Void to Existing Connector

```bash
# Add to existing connector:
- Add Void to macro flow declarations  
- Create {ConnectorName}VoidRequest (often just reference + reason)
- Create {ConnectorName}VoidResponse (verify actual API response)
- Implement URL pattern: /payments/{id}/void or /payments/{id}/cancel
- Map void-specific statuses (Voided, VoidFailed)
```

**✅ Result**: Working void flow integrated into existing connector in ~15 minutes

## Table of Contents

1. [Overview](#overview)
2. [Void vs Refund Understanding](#void-vs-refund-understanding)
3. [Void Flow Architecture](#void-flow-architecture)
4. [Request/Response Patterns](#requestresponse-patterns)
5. [URL Construction Patterns](#url-construction-patterns)
6. [Status Mapping](#status-mapping)
7. [Error Handling](#error-handling)
8. [Integration with Existing Connectors](#integration-with-existing-connectors)
9. [Testing Strategies](#testing-strategies)
10. [Troubleshooting Guide](#troubleshooting-guide)

## Overview

Void flows cancel authorized payments before they are captured/settled. Unlike refunds which return money after settlement, voids prevent the original payment from completing, typically with no fees.

### Key Characteristics:
- **Timing Dependent**: Usually only works on authorized (not captured) payments
- **Immediate Effect**: Cancels payment authorization instantly
- **No Fees**: Most connectors don't charge fees for voids
- **Simple Requests**: Often require just payment reference and optional reason
- **Status Specific**: Unique status mapping (Voided/VoidFailed vs payment statuses)

### When to Use Void vs Refund:
- **Use Void**: Payment authorized but not captured, want to cancel
- **Use Refund**: Payment already captured/settled, want to return money
- **Edge Cases**: Some connectors allow voiding captured payments (acts like refund)

## Void vs Refund Understanding

### Critical Differences

| Aspect | Void | Refund |
|--------|------|--------|
| **Timing** | Before capture/settlement | After capture/settlement |
| **Purpose** | Cancel authorization | Return money |
| **Fees** | Usually none | May have fees |
| **Speed** | Immediate | Takes time to process |
| **API Complexity** | Simple (just reference) | Complex (amount, currency, etc.) |
| **Status Flow** | Authorized → Voided | Charged → Refunded |

### Payment Lifecycle Context
```
Authorize → [VOID POSSIBLE] → Capture → [REFUND POSSIBLE] → Settle
     ↓                           ↓
   Voided                    Refunded
```

## Void Flow Architecture

### Data Flow
1. **Void Request**: Contains payment reference, optional cancellation reason
2. **Timing Validation**: Connector checks if payment can be voided
3. **API Call**: POST/PUT to `/payments/{id}/void` or similar
4. **Authorization Cancellation**: Payment gateway cancels the authorization
5. **Status Update**: Payment marked as voided

### Flow Relationship
```
Payment Flow (Authorize) → Void ← Status Check
         ↓                  ↑
    Authorized            Voided
```

## Request/Response Patterns

### Common Request Patterns

#### Pattern 1: Simple Reference Void (Checkout, Stripe-style)
```rust
// Minimal void request - just reference to original transaction
#[derive(Debug, Clone, Serialize)]
pub struct {ConnectorName}VoidRequest {
    pub reference: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>>
    for {ConnectorName}VoidRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference: item.router_data.request.connector_transaction_id.clone(),
        })
    }
}
```

#### Pattern 2: Detailed Transaction Void (Fiserv, Authorizedotnet-style)
```rust
// Comprehensive void request with transaction details and metadata
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidRequest {
    pub transaction_details: TransactionDetails,
    pub merchant_details: MerchantDetails,
    pub reference_transaction_details: ReferenceTransactionDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub reversal_reason_code: Option<String>,
    pub merchant_transaction_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceTransactionDetails {
    pub reference_transaction_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>>
    for {ConnectorName}VoidRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth: {ConnectorName}AuthType = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;
        
        Ok(Self {
            transaction_details: TransactionDetails {
                reversal_reason_code: router_data.request.cancellation_reason.clone(),
                merchant_transaction_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            },
            merchant_details: MerchantDetails {
                merchant_id: auth.merchant_account.clone(),
                terminal_id: extract_terminal_id(router_data)?,
            },
            reference_transaction_details: ReferenceTransactionDetails {
                reference_transaction_id: router_data.request.connector_transaction_id.clone(),
            },
        })
    }
}
```

#### Pattern 3: Session-Aware Void (Requires original payment session data)
```rust
// Void request that needs session information from original payment
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidRequest {
    pub transaction_id: String,
    pub reason: Option<String>,
    pub merchant_config: MerchantConfig,
}

#[derive(Debug, Serialize)]
pub struct MerchantConfig {
    pub merchant_id: Secret<String>,
    pub terminal_id: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> 
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>>
    for {ConnectorName}VoidRequest
{
    type Error = error_stack::Report<ConnectorError>;
    
    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        
        // Extract session information from connector metadata
        let session_data = extract_session_from_metadata(
            router_data.resource_common_data.connector_meta_data.as_ref()
        )?;
        
        Ok(Self {
            transaction_id: router_data.request.connector_transaction_id.clone(),
            reason: router_data.request.cancellation_reason.clone(),
            merchant_config: MerchantConfig {
                merchant_id: session_data.merchant_id,
                terminal_id: session_data.terminal_id,
            },
        })
    }
}

// Helper function to extract session data
fn extract_session_from_metadata(
    meta_data: Option<&pii::SecretSerdeValue>
) -> Result<SessionData, ConnectorError> {
    let session_meta_value = meta_data
        .ok_or_else(|| ConnectorError::MissingRequiredField {
            field_name: "connector_meta_data for session in Void"
        })?
        .peek();

    let session_str = match session_meta_value {
        serde_json::Value::String(s) => s,
        _ => return Err(ConnectorError::InvalidConnectorConfig {
            config: "connector_meta_data was not a JSON string for session in Void",
        }),
    };

    serde_json::from_str(session_str)
        .map_err(|_| ConnectorError::InvalidConnectorConfig {
            config: "Deserializing session from connector_meta_data string in Void",
        })
}
```

### Common Response Patterns

#### Pattern 1: HTTP Status-Based Response (Checkout-style)
```rust
// Response determined by HTTP status code rather than response body content
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct {ConnectorName}VoidResponse {
    #[serde(skip)]
    pub(super) status: u16,           // Set by wrapper, not from API
    pub action_id: String,
    pub reference: String,
}

impl From<&{ConnectorName}VoidResponse> for enums::AttemptStatus {
    fn from(item: &{ConnectorName}VoidResponse) -> Self {
        if item.status == 202 {
            Self::Voided
        } else {
            Self::VoidFailed
        }
    }
}
```

#### Pattern 2: Response Field-Based (Fiserv-style)
```rust
// Response contains explicit status field indicating success/failure
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}VoidResponse {
    pub gateway_response: GatewayResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    pub gateway_transaction_id: Option<String>,
    pub transaction_state: {ConnectorName}PaymentStatus,
    pub transaction_processing_details: TransactionProcessingDetails,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}PaymentStatus {
    Voided,
    Failed,
    Processing,
}

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

#### Pattern 3: Action-Based Response (Tracking-oriented)
```rust
// Response provides action ID for tracking the void operation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct {ConnectorName}VoidResponse {
    pub action_id: String,
    pub status: String,
    pub amount: Option<MinorUnit>,
    pub reference: Option<String>,
    pub processed_at: Option<String>,
}

impl From<&{ConnectorName}VoidResponse> for enums::AttemptStatus {
    fn from(item: &{ConnectorName}VoidResponse) -> Self {
        match item.status.as_str() {
            "completed" | "successful" | "voided" => Self::Voided,
            "failed" | "declined" | "rejected" => Self::VoidFailed,
            "pending" | "processing" => Self::Pending,
            _ => Self::VoidFailed, // Default to failed for unknown statuses
        }
    }
}
```

## URL Construction Patterns

### Pattern 1: RESTful Void Subresource (Most Common - Checkout, Stripe)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/voids"))
}
```

### Pattern 2: Direct Void Endpoint (Fiserv-style)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    Ok(format!("{base_url}/ch/payments/v1/cancels"))
    // Payment ID goes in request body instead of URL
}
```

### Pattern 3: Action-Based Endpoint (Some gateways)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    let payment_id = req.request.connector_transaction_id.clone();
    Ok(format!("{base_url}/payments/{payment_id}/actions/void"))
}
```

### Pattern 4: Transaction-Based Endpoint (Novalnet-style)
```rust
fn get_url(
    &self,
    req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.base_url;
    Ok(format!("{base_url}/transaction/cancel"))
}
```

## Status Mapping

### Standard Void Statuses
```rust
pub enum AttemptStatus {
    Voided,     // Void completed successfully
    VoidFailed, // Void failed or rejected
    Pending,    // Void initiated but not completed
}
```

### Common Connector Status Mappings

#### Checkout.com
```rust
fn map_checkout_void_status(http_status: u16) -> AttemptStatus {
    match http_status {
        202 => AttemptStatus::Voided,
        _ => AttemptStatus::VoidFailed,
    }
}
```

#### Fiserv
```rust
fn map_fiserv_void_status(status: &str) -> AttemptStatus {
    match status {
        "VOIDED" => AttemptStatus::Voided,
        "FAILED" | "DECLINED" => AttemptStatus::VoidFailed,
        "PROCESSING" => AttemptStatus::Pending,
        _ => AttemptStatus::VoidFailed,
    }
}
```

#### Generic String Status
```rust
fn map_generic_void_status(status: &str) -> AttemptStatus {
    match status.to_lowercase().as_str() {
        "voided" | "cancelled" | "canceled" | "completed" | "successful" => AttemptStatus::Voided,
        "failed" | "declined" | "rejected" | "error" => AttemptStatus::VoidFailed,
        "pending" | "processing" | "initiated" => AttemptStatus::Pending,
        _ => AttemptStatus::VoidFailed, // Conservative default
    }
}
```

### Response Transformation Pattern
```rust
impl<F> TryFrom<ResponseRouterData<{ConnectorName}VoidResponse, RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}VoidResponse, RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            mut response,
            router_data,
            http_code,
        } = item;

        let mut router_data = router_data;

        // Set the HTTP status code in the response object if needed
        response.status = http_code;

        // Get the attempt status using the From implementation
        let status = enums::AttemptStatus::from(&response);
        router_data.resource_common_data.status = status;

        // Always create TransactionResponse for void (success or failure)
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

## Error Handling

### Void-Specific Error Patterns

#### Already Captured/Settled
```rust
fn handle_void_timing_errors(error_code: &str) -> Option<AttemptStatus> {
    match error_code {
        "payment_already_captured" | "transaction_settled" => Some(AttemptStatus::VoidFailed),
        "void_window_expired" | "too_late_to_void" => Some(AttemptStatus::VoidFailed),
        "payment_not_found" | "invalid_transaction_id" => Some(AttemptStatus::VoidFailed),
        "void_not_allowed" | "void_restricted" => Some(AttemptStatus::VoidFailed),
        _ => None,
    }
}
```

#### Validation Errors
```rust
fn validate_void_request(
    payment_status: &str,
    connector_transaction_id: &str,
) -> Result<(), ConnectorError> {
    if connector_transaction_id.is_empty() {
        return Err(ConnectorError::MissingRequiredField {
            field_name: "connector_transaction_id".to_string(),
        });
    }
    
    // Some connectors provide current payment status
    match payment_status {
        "captured" | "settled" => {
            return Err(ConnectorError::InvalidRequestData {
                message: "Cannot void captured/settled payment. Use refund instead.".to_string(),
            })
        }
        "voided" | "cancelled" => {
            return Err(ConnectorError::InvalidRequestData {
                message: "Payment already voided".to_string(),
            })
        }
        _ => {}
    }
    
    Ok(())
}
```

## Integration with Existing Connectors

### Adding Void to Existing Connector

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
        // Add void flow
        (
            flow: Void,
            request_body: {ConnectorName}VoidRequest,
            response_body: {ConnectorName}VoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
    ],
    // ... rest of configuration
);
```

#### Step 2: Add Trait Implementation
```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for {ConnectorName}<T>
{
}
```

#### Step 3: Add Macro Implementation
```rust
// Void implementation
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
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_payments(req);
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{base_url}/payments/{payment_id}/voids"))
        }
    }
);
```

## Testing Strategies

### Unit Test Structure
```rust
#[cfg(test)]
mod void_tests {
    use super::*;

    #[test]
    fn test_void_request_simple() {
        let router_data = create_test_void_router_data();
        let void_request = {ConnectorName}VoidRequest::try_from(&router_data);
        
        assert!(void_request.is_ok());
        let request = void_request.unwrap();
        assert_eq!(request.reference, "test_payment_id");
    }

    #[test]
    fn test_void_response_transformation() {
        let response = {ConnectorName}VoidResponse {
            action_id: "void_123".to_string(),
            reference: "test_ref".to_string(),
            status: 202,
        };

        let router_data = create_test_void_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 202,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());
        
        let result_data = result.unwrap();
        assert_eq!(result_data.resource_common_data.status, AttemptStatus::Voided);
    }

    #[test]
    fn test_void_status_mapping() {
        // Test status mapping patterns
        assert_eq!(
            map_connector_void_status("voided"),
            AttemptStatus::Voided
        );
        assert_eq!(
            map_connector_void_status("completed"),
            AttemptStatus::Voided
        );
        assert_eq!(
            map_connector_void_status("failed"),
            AttemptStatus::VoidFailed
        );
        assert_eq!(
            map_connector_void_status("pending"),
            AttemptStatus::Pending
        );
    }

    #[test]
    fn test_void_url_construction() {
        let router_data = create_test_void_router_data();
        let connector = {ConnectorName}::new();
        
        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("/payments/"));
        assert!(url.ends_with("/voids"));
    }

    #[test]
    fn test_void_error_scenarios() {
        // Test void-specific error handling
        let error_response = {ConnectorName}ErrorResponse {
            error_code: Some("payment_already_captured".to_string()),
            error_message: Some("Payment cannot be voided after capture".to_string()),
            error_description: Some("Use refund instead".to_string()),
        };

        let attempt_status = handle_void_timing_errors("payment_already_captured");
        assert_eq!(attempt_status, Some(AttemptStatus::VoidFailed));
    }
}
```

### Integration Test Pattern
```rust
#[tokio::test]
async fn test_full_void_flow() {
    // 1. Create and authorize a payment first
    let payment_response = create_test_payment().await;
    assert!(payment_response.is_ok());
    
    // 2. Ensure payment is authorized (not captured)
    let payment_data = payment_response.unwrap();
    assert_eq!(payment_data.status, AttemptStatus::Authorized);
    
    // 3. Process void
    let void_request = create_void_request(&payment_data);
    let void_response = process_void(void_request).await;
    assert!(void_response.is_ok());
    
    // 4. Verify void succeeded
    let void_data = void_response.unwrap();
    assert_eq!(void_data.status, AttemptStatus::Voided);
}

#[tokio::test]
async fn test_void_after_capture_fails() {
    // 1. Create, authorize, and capture a payment
    let payment_response = create_and_capture_payment().await;
    assert!(payment_response.is_ok());
    
    // 2. Try to void captured payment (should fail)
    let void_request = create_void_request(&payment_response.unwrap());
    let void_response = process_void(void_request).await;
    
    // 3. Verify void failed with appropriate error
    assert!(void_response.is_err());
    let error = void_response.unwrap_err();
    assert!(error.message.contains("captured") || error.message.contains("settled"));
}
```

## Troubleshooting Guide

### Common Issues and Solutions

#### Issue 1: "Payment cannot be voided" Error
**Error**: `payment_already_captured`, `transaction_settled`
**Cause**: Trying to void a payment that's already been captured
**Solution**: Check payment status before void, use refund for captured payments

```rust
// Before voiding, check payment status
fn can_void_payment(payment_status: &str) -> bool {
    matches!(payment_status, "authorized" | "pending" | "requires_action")
}

// Provide helpful error messages
if !can_void_payment(&current_status) {
    return Err(ConnectorError::InvalidRequestData {
        message: format!(
            "Cannot void payment with status '{}'. Use refund for captured payments.", 
            current_status
        ),
    });
}
```

#### Issue 2: Session/Metadata Required Error
**Error**: `missing_session_data`, `invalid_terminal_id`
**Cause**: Void requires session information from original payment
**Solution**: Ensure connector metadata is passed through from authorize to void

```rust
// Extract session data from connector metadata
fn extract_session_data(
    connector_meta_data: Option<&pii::SecretSerdeValue>
) -> Result<SessionData, ConnectorError> {
    let meta_data = connector_meta_data
        .ok_or_else(|| ConnectorError::MissingRequiredField {
            field_name: "connector_meta_data for session data in Void"
        })?;
        
    // Parse session data from metadata
    // ... implementation
}
```

#### Issue 3: Wrong HTTP Method
**Error**: `405 Method Not Allowed`
**Cause**: Using wrong HTTP method for void endpoint
**Solution**: Check connector documentation for correct method

```rust
// Some connectors use POST, others use PUT or DELETE
http_method: Post,  // Most common
// OR
http_method: Put,   // Some legacy connectors
// OR  
http_method: Delete, // RESTful approach
```

#### Issue 4: Void ID Not Found in Response
**Error**: Cannot extract void transaction ID
**Cause**: Different ID sources in void vs payment responses
**Solution**: Check multiple possible sources

```rust
fn extract_void_id(response: &VoidResponse) -> String {
    // Try multiple sources
    response.action_id.clone()
        .or_else(|| response.void_id.clone())
        .or_else(|| response.transaction_id.clone())
        .or_else(|| extract_id_from_reference(&response.reference))
        .unwrap_or_else(|| "unknown_void_id".to_string())
}
```

### API-Specific Troubleshooting

#### Checkout.com
- **Status Code Dependency**: Relies on HTTP 202 for success
- **Action ID**: Returns action_id instead of transaction_id
- **Metadata Required**: Needs processing channel ID

#### Fiserv
- **Session Required**: Requires terminal_id from original payment session
- **Complex Request**: Needs merchant details and reference transaction details
- **Status Mapping**: Uses UPPERCASE enum values

#### Authorizedotnet
- **XML Format**: Some legacy endpoints use XML instead of JSON
- **Transaction Reference**: Requires original transaction reference
- **Timing Window**: Strict timing restrictions on voids

### Testing Checklist

- [ ] **Request Structure**: Verify request format matches API documentation
- [ ] **URL Pattern**: Test correct endpoint construction for void
- [ ] **Status Mapping**: Verify all possible void statuses are handled
- [ ] **Error Handling**: Test void-specific error scenarios
- [ ] **Timing Restrictions**: Test voiding authorized vs captured payments
- [ ] **Session Data**: Test connectors requiring session/metadata
- [ ] **HTTP Methods**: Verify correct HTTP method (POST/PUT/DELETE)
- [ ] **Integration**: Test with real API in sandbox environment

### Best Practices

1. **Understand Payment Lifecycle**: Know when voids are allowed vs refunds
2. **Check Status First**: Validate payment can be voided before attempting
3. **Handle Session Data**: Preserve session information from authorize to void
4. **Timing Awareness**: Implement appropriate timing restrictions
5. **Error Context**: Provide clear error messages about why void failed
6. **Test Edge Cases**: Test void attempts on different payment statuses
7. **Documentation**: Document any special requirements or restrictions
8. **Fallback Strategy**: Consider automatic refund fallback for "too late" voids

### Integration Checklist

- [ ] **Add Flow Declaration**: Include Void in connector macro setup
- [ ] **Implement Trait**: Add PaymentVoidV2 trait implementation
- [ ] **Create Structures**: Define VoidRequest and VoidResponse structures
- [ ] **URL Construction**: Implement correct endpoint pattern
- [ ] **Status Mapping**: Map connector statuses to Voided/VoidFailed
- [ ] **Error Handling**: Handle void-specific errors appropriately
- [ ] **Session Handling**: Manage session data if required
- [ ] **Testing**: Unit and integration tests for void flow
- [ ] **Documentation**: Document any connector-specific limitations

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

Replace these placeholders with your connector-specific values:

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `PayPal`, `NewPayment` | **Always required** - Used in struct names, type names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `paypal`, `new_payment` | **Always required** - Used in file names, config keys |
| `{api_endpoint}` | Void API endpoint path | `"voids"`, `"cancel"`, `"void"` | **From API docs** - Void-specific endpoint |
| `{http_method}` | HTTP method for void | `Post`, `Put`, `Delete` | **Based on API**: Post = most common |

### Void-Specific Selection Guide

Choose the right pattern based on your connector's void API:

| API Style | Request Pattern | URL Pattern | Example |
|-----------|----------------|-------------|---------|
| Simple Reference | Pattern 1 | `/payments/{id}/voids` | Checkout.com, Stripe |
| Detailed Transaction | Pattern 2 | `/cancels` or `/void` | Fiserv, Authorizedotnet |
| Session-Aware | Pattern 3 | `/payments/{id}/cancel` | Legacy gateways |

### Real-World Examples

**Example 1: Simple Void (Checkout.com style)**
```bash
{ConnectorName} → MyPayment
{connector_name} → my_payment
{api_endpoint} → "voids"
{http_method} → Post
Request: Just reference
Response: HTTP status-based
```

**Example 2: Complex Void (Fiserv style)**
```bash
{ConnectorName} → EnterpriseGateway
{connector_name} → enterprise_gateway
{api_endpoint} → "cancels"
{http_method} → Post
Request: Full transaction details + session
Response: Gateway response object
```

This comprehensive guide provides battle-tested patterns for implementing void flows across different payment gateway architectures, ensuring consistent and robust void handling in your connector implementation.
