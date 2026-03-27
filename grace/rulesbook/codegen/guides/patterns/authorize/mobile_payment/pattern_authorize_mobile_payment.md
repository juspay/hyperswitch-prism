# Mobile Payment Authorize Flow Pattern

**Payment Method Category**: Mobile Payment
**Primary Variant**: Direct Carrier Billing (DCB)
**Pattern Type**: Telecommunications-based mobile payments

## Table of Contents

1. [Overview](#overview)
2. [Quick Reference](#quick-reference)
3. [Data Model](#data-model)
4. [Implementation Patterns](#implementation-patterns)
5. [Request Patterns](#request-patterns)
6. [Response Patterns](#response-patterns)
7. [Connector Examples](#connector-examples)
8. [Implementation Checklist](#implementation-checklist)
9. [Common Pitfalls](#common-pitfalls)
10. [Testing Patterns](#testing-patterns)

---

## Overview

Mobile Payment is a payment method category that enables customers to make purchases charged directly to their mobile phone bill or prepaid balance. The primary variant is **Direct Carrier Billing (DCB)**, where the payment amount is added to the user's monthly mobile phone bill or deducted from their prepaid balance.

### Key Characteristics

| Attribute | Description |
|-----------|-------------|
| **Payment Flow** | Async - requires carrier confirmation |
| **Authentication** | MSISDN (phone number) validation |
| **Settlement** | Deferred - carrier collects from user, then settles with merchant |
| **Risk Profile** | High fraud risk - requires additional verification |
| **Use Cases** | Digital goods, subscriptions, micro-transactions |

### Current Implementation Status

Most connectors in the Grace-UCS codebase currently return `NotImplemented` for Mobile Payment requests. This pattern document provides the implementation template for connectors that DO support Direct Carrier Billing.

### Direct Carrier Billing Data Structure

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobilePaymentData {
    DirectCarrierBilling {
        /// The phone number of the user (MSISDN format)
        msisdn: String,
        /// Unique user identifier (optional)
        client_uid: Option<String>,
    },
}
```

---

## Quick Reference

### Supported Connectors

| Connector | Status | Notes |
|-----------|--------|-------|
| Stripe | Not Supported | Returns `NotImplemented` |
| Adyen | Not Supported | Returns `NotImplemented` |
| ACI | Not Supported | Returns `NotImplemented` |
| *Most Connectors* | Not Supported | Returns `NotImplemented` |

### Amount Handling

| Aspect | Recommendation |
|--------|----------------|
| Amount Unit | `StringMinorUnit` or `MinorUnit` (connector-specific) |
| Currency | Limited to currencies supported by carrier |
| Amount Limits | Usually micro-transactions (< $50 USD equivalent) |

### Error Handling Pattern

```rust
PaymentMethodData::MobilePayment(_) => {
    Err(ConnectorError::NotImplemented(
        "Direct Carrier Billing is not supported by {ConnectorName}".to_string()
    ))?
}
```

---

## Data Model

### Rust Type Definition

```rust
// PaymentMethodData::MobilePayment variant
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    // ... other variants
    MobilePayment(MobilePaymentData),
    // ... other variants
}

// MobilePaymentData enum
pub enum MobilePaymentData {
    DirectCarrierBilling {
        msisdn: String,              // Phone number in international format
        client_uid: Option<String>,  // Optional client identifier
    },
}
```

### Field Descriptions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `msisdn` | `String` | Yes | Mobile Station International Subscriber Directory Number (phone number) in international format (e.g., +1234567890) |
| `client_uid` | `Option<String>` | No | Unique identifier for the client/user in the merchant's system |

### Proto Definition Reference

```protobuf
// From payment_methods.proto (commented - not yet active)
message DirectCarrierBilling {
  // The phone number of the user
  SecretString msisdn = 1 [(validate.rules).string = {min_len: 5}];

  // Unique user identifier
  optional string client_uid = 2;
}
```

---

## Implementation Patterns

### Pattern 1: Standard Async Flow (Recommended)

**Characteristics**:
- Async payment confirmation
- Webhook-based status updates
- PSync support for status polling

**Implementation Template**:

```rust
// transformers.rs - Request Structure
#[derive(Debug, Serialize)]
pub struct {ConnectorName}MobilePaymentRequest {
    pub amount: StringMinorUnit,
    pub currency: String,
    pub phone_number: String,        // MSISDN
    pub client_reference: Option<String>,
    pub callback_url: String,
    // Connector-specific fields
    pub merchant_id: String,
    pub product_description: Option<String>,
}

// Response Structure
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}MobilePaymentResponse {
    pub transaction_id: String,
    pub status: {ConnectorName}MobilePaymentStatus,
    pub phone_number: String,
    pub amount: StringMinorUnit,
    pub currency: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}MobilePaymentStatus {
    Pending,        // Awaiting carrier confirmation
    Approved,       // Carrier approved
    Rejected,       // Carrier rejected
    Completed,      // Payment completed
    Failed,         // Payment failed
    Refunded,       // Payment refunded
}
```

### Pattern 2: Unsupported Payment Method

For connectors that do NOT support Mobile Payment:

```rust
// In transformers.rs - Request transformation match arm
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data.clone() {
            PaymentMethodData::Card(ref card_data) => {
                // Card implementation...
            }
            PaymentMethodData::Wallet(ref wallet_data) => {
                // Wallet implementation...
            }
            // ... other supported methods
            PaymentMethodData::MobilePayment(_) => {
                Err(ConnectorError::NotImplemented(
                    "Direct Carrier Billing is not supported by {ConnectorName}".to_string()
                ))?
            }
            // ... other unsupported methods
        }
    }
}
```

---

## Request Patterns

### JSON Request Structure

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobilePaymentAuthorizeRequest {
    // Transaction details
    pub amount: StringMinorUnit,
    pub currency: String,
    pub reference: String,

    // Mobile payment specific
    pub msisdn: String,
    pub client_uid: Option<String>,

    // Required for async notifications
    pub callback_url: String,

    // Optional metadata
    pub description: Option<String>,
    pub merchant_id: String,
}
```

### Form-Encoded Request Structure

```rust
#[derive(Debug, Serialize)]
pub struct MobilePaymentFormRequest {
    #[serde(rename = "amount")]
    pub amount: String,
    #[serde(rename = "currency")]
    pub currency: String,
    #[serde(rename = "msisdn")]
    pub phone_number: String,
    #[serde(rename = "client_uid")]
    pub client_uid: Option<String>,
    #[serde(rename = "callback_url")]
    pub callback_url: String,
}
```

### Request Transformation Implementation

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        match &router_data.request.payment_method_data {
            PaymentMethodData::MobilePayment(mobile_payment_data) => {
                match mobile_payment_data {
                    MobilePaymentData::DirectCarrierBilling { msisdn, client_uid } => {
                        // Validate MSISDN format
                        validate_msisdn(msisdn)?;

                        Ok(Self {
                            amount: item.amount,
                            currency: router_data.request.currency.to_string(),
                            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
                            msisdn: msisdn.clone(),
                            client_uid: client_uid.clone(),
                            callback_url: router_data.request.router_return_url.clone()
                                .ok_or(ConnectorError::MissingRequiredField {
                                    field_name: "router_return_url",
                                })?,
                            description: router_data.request.description.clone(),
                            merchant_id: get_merchant_id(&router_data.connector_auth_type)?,
                        })
                    }
                }
            }
            // ... other payment methods
        }
    }
}

// Helper function to validate MSISDN format
fn validate_msisdn(msisdn: &str) -> Result<(), ConnectorError> {
    // Basic validation: starts with + and contains only digits after
    if !msisdn.starts_with('+') || !msisdn[1..].chars().all(|c| c.is_ascii_digit()) {
        return Err(ConnectorError::InvalidRequestData {
            message: format!("Invalid MSISDN format: {}. Must start with + followed by digits", msisdn),
        });
    }

    // Minimum length check (e.g., +1XXXXXXXXXX = 12 chars minimum)
    if msisdn.len() < 7 {
        return Err(ConnectorError::InvalidRequestData {
            message: "MSISDN too short".to_string(),
        });
    }

    Ok(())
}
```

---

## Response Patterns

### Status Mapping

```rust
// Map connector-specific status to standard AttemptStatus
impl From<{ConnectorName}MobilePaymentStatus> for common_enums::AttemptStatus {
    fn from(status: {ConnectorName}MobilePaymentStatus) -> Self {
        match status {
            {ConnectorName}MobilePaymentStatus::Pending => Self::Pending,
            {ConnectorName}MobilePaymentStatus::Approved => Self::Authorized,
            {ConnectorName}MobilePaymentStatus::Completed => Self::Charged,
            {ConnectorName}MobilePaymentStatus::Rejected => Self::Failure,
            {ConnectorName}MobilePaymentStatus::Failed => Self::Failure,
            {ConnectorName}MobilePaymentStatus::Refunded => Self::RefundApplied,
        }
    }
}
```

### Response Transformation

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            {ConnectorName}AuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<
            {ConnectorName}AuthorizeResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let status = common_enums::AttemptStatus::from(response.status.clone());

        // Handle error responses
        if status == common_enums::AttemptStatus::Failure {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: "MOBILE_PAYMENT_FAILED".to_string(),
                    message: "Mobile payment was rejected by carrier".to_string(),
                    reason: Some(format!("Status: {:?}", response.status)),
                    status_code: item.http_code,
                    attempt_status: Some(status),
                    connector_transaction_id: Some(response.transaction_id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Success/pending response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
            redirection_data: None, // DCB typically doesn't require redirect
            mandate_reference: None,
            connector_metadata: Some(
                serde_json::json!({
                    "phone_number": response.phone_number,
                    "carrier": response.carrier_name.clone(),
                })
            ),
            network_txn_id: None,
            connector_response_reference_id: Some(response.transaction_id.clone()),
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

---

## Connector Examples

### Example 1: Unsupported Implementation (Most Connectors)

**Connectors**: Stripe, Adyen, ACI, and most others

```rust
// In transformers.rs - Request transformation
PaymentMethodData::MobilePayment(_) => {
    Err(ConnectorError::NotImplemented(
        "Direct Carrier Billing is not supported".to_string()
    ))?
}
```

### Example 2: Full Implementation Template

For connectors that DO support Direct Carrier Billing:

```rust
// Main connector file: {connector_name}.rs

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}AuthorizeRequest),
    curl_response: {ConnectorName}AuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/v1/mobile-payments"))
        }
    }
);
```

---

## Implementation Checklist

### Pre-Implementation

- [ ] Verify connector supports Direct Carrier Billing
- [ ] Confirm supported carriers/regions
- [ ] Understand carrier-specific requirements
- [ ] Review fraud prevention requirements
- [ ] Confirm async webhook support

### Request Implementation

- [ ] Add `MobilePayment` match arm in request transformation
- [ ] Implement MSISDN validation
- [ ] Extract and validate `client_uid` if provided
- [ ] Format amount according to connector requirements
- [ ] Include callback URL for async notifications
- [ ] Add carrier-specific fields if required

### Response Implementation

- [ ] Define connector-specific status enum
- [ ] Implement `From<ConnectorStatus>` for `AttemptStatus`
- [ ] Handle pending status correctly
- [ ] Handle carrier rejection scenarios
- [ ] Include carrier metadata in response

### Error Handling

- [ ] Invalid MSISDN format errors
- [ ] Carrier rejection errors
- [ ] Timeout scenarios
- [ ] Invalid amount/currency combinations
- [ ] Missing required fields

### PSync Implementation

- [ ] Implement status polling for pending transactions
- [ ] Handle carrier-specific status codes
- [ ] Map carrier status to standard status

### Webhook Handling

- [ ] Implement webhook signature verification
- [ ] Parse carrier webhook payload
- [ ] Update transaction status based on webhook
- [ ] Handle duplicate webhook scenarios

---

## Common Pitfalls

### 1. MSISDN Format Validation

**Problem**: Carriers require specific MSISDN formats (usually E.164).

**Solution**:
```rust
fn validate_msisdn(msisdn: &str) -> Result<(), ConnectorError> {
    // E.164 format: +[country code][national number]
    if !msisdn.starts_with('+') {
        return Err(ConnectorError::InvalidRequestData {
            message: "MSISDN must start with +".to_string(),
        });
    }

    let digits_only = &msisdn[1..];
    if !digits_only.chars().all(|c| c.is_ascii_digit()) {
        return Err(ConnectorError::InvalidRequestData {
            message: "MSISDN must contain only digits after +".to_string(),
        });
    }

    // E.164 allows 7-15 digits for the national number
    if digits_only.len() < 7 || digits_only.len() > 15 {
        return Err(ConnectorError::InvalidRequestData {
            message: "MSISDN length must be 8-16 characters including +".to_string(),
        });
    }

    Ok(())
}
```

### 2. Async Status Handling

**Problem**: Treating pending mobile payments as final states.

**Solution**:
```rust
// Always return Pending for carrier confirmation
{ConnectorName}MobilePaymentStatus::Pending => common_enums::AttemptStatus::Pending,

// Implement PSync to poll for final status
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for {ConnectorName}<T>
{
    // PSync implementation for polling carrier status
}
```

### 3. Amount Limits

**Problem**: DCB typically has strict amount limits per transaction and monthly.

**Solution**:
```rust
fn validate_dcb_amount(amount: MinorUnit, currency: Currency) -> Result<(), ConnectorError> {
    let amount_in_usd = convert_to_usd(amount, currency)?;

    // Typical DCB limits: $10-50 per transaction
    if amount_in_usd > 50.0 {
        return Err(ConnectorError::InvalidRequestData {
            message: "Direct Carrier Billing amount exceeds maximum limit".to_string(),
        });
    }

    Ok(())
}
```

### 4. Fraud Prevention

**Problem**: High fraud risk with DCB requires additional verification.

**Solution**:
- Implement PIN verification flows
- Use client_uid for device fingerprinting
- Implement velocity checks
- Require additional authentication for high-value transactions

---

## Testing Patterns

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msisdn_validation_valid() {
        let valid_numbers = vec![
            "+1234567890",
            "+441234567890",
            "+919876543210",
        ];

        for number in valid_numbers {
            assert!(validate_msisdn(number).is_ok());
        }
    }

    #[test]
    fn test_msisdn_validation_invalid() {
        let invalid_numbers = vec![
            "1234567890",      // Missing +
            "+123456",         // Too short
            "+1234567890123456", // Too long
            "+123-456-7890",   // Contains non-digits
        ];

        for number in invalid_numbers {
            assert!(validate_msisdn(number).is_err());
        }
    }

    #[test]
    fn test_mobile_payment_request_transformation() {
        let router_data = create_test_router_data_with_mobile_payment(
            "+1234567890",
            Some("client_123"),
        );

        let request = {ConnectorName}AuthorizeRequest::try_from(router_data);
        assert!(request.is_ok());

        let req = request.unwrap();
        assert_eq!(req.msisdn, "+1234567890");
        assert_eq!(req.client_uid, Some("client_123".to_string()));
    }

    #[test]
    fn test_pending_status_mapping() {
        let connector_status = {ConnectorName}MobilePaymentStatus::Pending;
        let attempt_status: common_enums::AttemptStatus = connector_status.into();
        assert_eq!(attempt_status, common_enums::AttemptStatus::Pending);
    }

    #[test]
    fn test_completed_status_mapping() {
        let connector_status = {ConnectorName}MobilePaymentStatus::Completed;
        let attempt_status: common_enums::AttemptStatus = connector_status.into();
        assert_eq!(attempt_status, common_enums::AttemptStatus::Charged);
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_mobile_payment_authorize_flow() {
        let connector = {ConnectorName}::new();

        // Create test data
        let router_data = create_test_authorize_request(
            PaymentMethodData::MobilePayment(MobilePaymentData::DirectCarrierBilling {
                msisdn: "+1234567890".to_string(),
                client_uid: Some("test_client".to_string()),
            })
        );

        // Test request generation
        let headers = connector.get_headers(&router_data).unwrap();
        assert!(headers.iter().any(|(k, _)| k == "Content-Type"));

        let url = connector.get_url(&router_data).unwrap();
        assert!(url.contains("mobile-payments"));

        let body = connector.get_request_body(&router_data).unwrap();
        assert!(body.is_some());
    }

    #[tokio::test]
    async fn test_mobile_payment_psync_flow() {
        let connector = {ConnectorName}::new();

        // Create pending transaction sync request
        let sync_data = create_test_sync_request(
            "txn_123",
            Some(common_enums::AttemptStatus::Pending),
        );

        let url = connector.get_url(&sync_data).unwrap();
        assert!(url.contains("txn_123"));
    }
}
```

---

## Related Patterns

- [Pattern: Authorize Flow - Base Pattern](./pattern_authorize.md)
- [Pattern: Async Payment Handling](./pattern_async_payments.md)
- [Pattern: Webhook Handling](./pattern_webhooks.md)
- [Pattern: PSync Flow](./pattern_psync.md)

---

## References

- [GSMA Mobile Money API](https://developer.gsma.com/mobile-money-api/)
- [E.164 Number Format](https://en.wikipedia.org/wiki/E.164)
- [Direct Carrier Billing Best Practices](https://www.gsma.com/)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-19
**Maintained By**: Connector Integration Team
