# Authorize Flow Pattern for Reward Payment Method

**🎯 REWARD PAYMENT METHOD IMPLEMENTATION GUIDE**

This document provides comprehensive patterns for implementing the authorize flow for **Reward** payment methods in Grace-UCS connectors. Reward payments include cash-based payment methods like Classic Reward and Evoucher, which typically require customer redirection to complete the payment.

## Table of Contents

1. [Overview](#overview)
2. [Reward Payment Method Variants](#reward-payment-method-variants)
3. [Quick Reference](#quick-reference)
4. [Supported Connectors](#supported-connectors)
5. [Implementation Patterns](#implementation-patterns)
6. [Request/Response Patterns](#requestresponse-patterns)
7. [Sub-type Variations](#sub-type-variations)
8. [Error Handling](#error-handling)
9. [Testing Patterns](#testing-patterns)
10. [Integration Checklist](#integration-checklist)

## Overview

Reward is a payment method category in Grace-UCS that represents cash-based or prepaid payment solutions. Unlike card or wallet payments, Reward payments typically:

- Require customer redirection to a payment page
- Are completed offline or through third-party cash networks
- Use unique transaction identifiers for tracking
- Support multiple sub-types (ClassicReward, Evoucher)

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Payment Flow** | Redirect-based (asynchronous) |
| **Amount Format** | FloatMajorUnit (major currency units) |
| **Request Format** | JSON |
| **Response Type** | Async/Redirect |
| **Webhook Support** | Required for payment confirmation |
| **Psync Required** | Yes (for pending transaction status) |

## Reward Payment Method Variants

From `crates/types-traits/domain_types/src/payment_method_data.rs`:

```rust
#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    // ... other variants
    Reward,
    // ... other variants
}
```

From `crates/common/common_enums/src/enums.rs`:

```rust
#[derive(...)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PaymentMethodType {
    // ... other variants
    #[serde(rename = "classic")]
    ClassicReward,
    Evoucher,
    // ... other variants
}
```

### Variant Mapping

| PaymentMethodType | Description | Use Case |
|-------------------|-------------|----------|
| `ClassicReward` | Classic cash-to-code reward system | Physical cash payments via partner network |
| `Evoucher` | Electronic voucher-based payments | Digital voucher redemption |

## Quick Reference

### Implementation Summary Table

| Connector | Sub-types Supported | Auth Type | Amount Unit | Response Type |
|-----------|---------------------|-----------|-------------|---------------|
| **CashToCode** | ClassicReward, Evoucher | CurrencyAuthKey | FloatMajorUnit | Redirect |

### Common Implementation Pattern

```rust
// Request transformation for Reward payments
match item.router_data.resource_common_data.payment_method {
    common_enums::PaymentMethod::Reward => {
        // Handle Reward payment specific logic
        // 1. Extract payment method type (ClassicReward/Evoucher)
        // 2. Get connector-specific merchant credentials
        // 3. Build redirect-based request
    },
    _ => Err(IntegrationError::NotImplemented("Payment methods".to_string(, Default::default())).into()),
}
```

## Supported Connectors

### Fully Implemented

| Connector | Status | Notes |
|-----------|--------|-------|
| **CashToCode** | ✅ Fully Supported | Supports both ClassicReward and Evoucher with redirect flow |

### Not Implemented (Return NotImplemented Error)

The following connectors have Reward listed in their match arms but return `NotImplemented` errors:

- Adyen
- Stripe
- Cybersource
- PayPal
- Braintree
- MultiSafepay
- Billwerk
- Fiserv
- Loonio
- Redsys
- Fiuu
- ACI
- Cryptopay
- Hipay
- Mifinity
- WellsFargo
- BankOfAmerica
- Noon
- Razorpay
- Volt
- Placetopay
- Bambora
- Nexinets
- Trustpay
- Dlocal
- Worldpay
- Forte

## Implementation Patterns

### CashToCode Implementation (Reference Implementation)

**File**: `crates/integrations/connector-integration/src/connectors/cashtocode/transformers.rs`

#### 1. Request Structure

```rust
#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsRequest {
    amount: FloatMajorUnit,              // Major currency units (e.g., 10.50 for $10.50)
    transaction_id: String,              // Unique transaction identifier
    user_id: Secret<id_type::CustomerId>, // Customer identifier
    currency: common_enums::Currency,    // Payment currency
    first_name: Option<Secret<String>>,  // Customer first name
    last_name: Option<Secret<String>>,   // Customer last name
    user_alias: Secret<id_type::CustomerId>, // User alias for tracking
    requested_url: String,               // Success/callback URL
    cancel_url: String,                  // Cancel URL
    email: Option<Email>,                // Customer email
    mid: Secret<String>,                 // Merchant ID (varies by sub-type)
}
```

#### 2. Sub-type Specific Merchant ID Extraction

```rust
fn get_mid(
    connector_auth_type: &ConnectorAuthType,
    payment_method_type: Option<common_enums::PaymentMethodType>,
    currency: common_enums::Currency,
) -> Result<Secret<String>, IntegrationError> {
    match CashtocodeAuth::try_from((connector_auth_type, &currency)) {
        Ok(cashtocode_auth) => match payment_method_type {
            Some(common_enums::PaymentMethodType::ClassicReward) => Ok(cashtocode_auth
                .merchant_id_classic
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?),
            Some(common_enums::PaymentMethodType::Evoucher) => Ok(cashtocode_auth
                .merchant_id_evoucher
                .ok_or(IntegrationError::FailedToObtainAuthType { context: Default::default() })?),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        },
        Err(_) => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() })?,
    }
}
```

#### 3. Authentication Structure

```rust
#[derive(Default, Debug, Deserialize)]
pub struct CashtocodeAuth {
    pub password_classic: Option<Secret<String>>,
    pub password_evoucher: Option<Secret<String>>,
    pub username_classic: Option<Secret<String>>,
    pub username_evoucher: Option<Secret<String>>,
    pub merchant_id_classic: Option<Secret<String>>,
    pub merchant_id_evoucher: Option<Secret<String>>,
}

#[derive(Default, Debug, Deserialize)]
pub struct CashtocodeAuthType {
    pub auths: HashMap<common_enums::Currency, CashtocodeAuth>,
}
```

#### 4. Request Transformation

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CashtocodeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for CashtocodePaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: CashtocodeRouterData<...>,
    ) -> Result<Self, Self::Error> {
        let customer_id = item.router_data.resource_common_data.get_customer_id()?;
        let url = item.router_data.request.get_router_return_url()?;
        let mid = get_mid(
            &item.router_data.connector_auth_type,
            item.router_data.request.payment_method_type,
            item.router_data.request.currency,
        )?;
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed)?;

        match item.router_data.resource_common_data.payment_method {
            common_enums::PaymentMethod::Reward => Ok(Self {
                amount,
                transaction_id: item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id,
                currency: item.router_data.request.currency,
                user_id: Secret::new(customer_id.to_owned()),
                first_name: None,
                last_name: None,
                user_alias: Secret::new(customer_id),
                requested_url: url.to_owned(),
                cancel_url: url,
                email: item.router_data.request.email.clone(),
                mid,
            }),
            _ => Err(IntegrationError::NotImplemented("Payment methods".to_string(, Default::default())).into()),
        }
    }
}
```

#### 5. Response Handling

```rust
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CashtocodePaymentStatus {
    Succeeded,
    #[default]
    Processing,
}

impl From<CashtocodePaymentStatus> for common_enums::AttemptStatus {
    fn from(item: CashtocodePaymentStatus) -> Self {
        match item {
            CashtocodePaymentStatus::Succeeded => Self::Charged,
            CashtocodePaymentStatus::Processing => Self::AuthenticationPending,
        }
    }
}
```

#### 6. Redirect Form Handling by Sub-type

```rust
fn get_redirect_form_data(
    payment_method_type: common_enums::PaymentMethodType,
    response_data: CashtocodePaymentsResponseData,
) -> CustomResult<RedirectForm, IntegrationError> {
    match payment_method_type {
        common_enums::PaymentMethodType::ClassicReward => Ok(RedirectForm::Form {
            // Redirect form is manually constructed because the connector
            // for this pm type expects query params in the url
            endpoint: response_data.pay_url.to_string(),
            method: Method::Post,
            form_fields: Default::default(),
        }),
        common_enums::PaymentMethodType::Evoucher => Ok(RedirectForm::from((
            // Here the pay url gets parsed, and query params are sent as form fields
            // as the connector expects
            response_data.pay_url,
            Method::Get,
        ))),
        _ => Err(IntegrationError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("CashToCode", Default::default()),
        ))?,
    }
}
```

#### 7. Sub-type Specific Authentication Headers

```rust
// From cashtocode.rs
let auth_header = match payment_method_type {
    Some(common_enums::PaymentMethodType::ClassicReward) => construct_basic_auth(
        auth_type.username_classic.to_owned(),
        auth_type.password_classic.to_owned(),
    ),
    Some(common_enums::PaymentMethodType::Evoucher) => construct_basic_auth(
        auth_type.username_evoucher.to_owned(),
        auth_type.password_evoucher.to_owned(),
    ),
    _ => return Err(errors::IntegrationError::MissingPaymentMethodType)?,
}?;
```

## Request/Response Patterns

### Request Pattern

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | FloatMajorUnit | Yes | Amount in major currency units (e.g., 10.50) |
| `transaction_id` | String | Yes | Unique transaction reference |
| `user_id` | Secret<CustomerId> | Yes | Customer identifier |
| `currency` | Currency | Yes | ISO currency code |
| `requested_url` | String | Yes | Success/callback URL |
| `cancel_url` | String | Yes | Payment cancel URL |
| `mid` | Secret<String> | Yes | Merchant ID (sub-type specific) |
| `email` | Option<Email> | No | Customer email |
| `first_name` | Option<Secret<String>> | No | Customer first name |
| `last_name` | Option<Secret<String>> | No | Customer last name |

### Response Pattern

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodePaymentsResponseData {
    pub pay_url: url::Url,  // Redirect URL for customer
}
```

### Webhook Payload

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashtocodeIncomingWebhook {
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub foreign_transaction_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub transaction_id: String,
}
```

## Sub-type Variations

### ClassicReward vs Evoucher

| Aspect | ClassicReward | Evoucher |
|--------|---------------|----------|
| **Auth Credentials** | `username_classic` / `password_classic` | `username_evoucher` / `password_evoucher` |
| **Merchant ID** | `merchant_id_classic` | `merchant_id_evoucher` |
| **Redirect Method** | POST with empty form fields | GET with query params as form fields |
| **Payment Flow** | Physical cash at partner locations | Digital voucher redemption |

### Implementation Template for Sub-type Handling

```rust
// Pattern for sub-type specific logic
match payment_method_type {
    Some(common_enums::PaymentMethodType::ClassicReward) => {
        // ClassicReward specific implementation
        handle_classic_reward(...)
    }
    Some(common_enums::PaymentMethodType::Evoucher) => {
        // Evoucher specific implementation
        handle_evoucher(...)
    }
    _ => Err(IntegrationError::MissingPaymentMethodType)?,
}
```

## Error Handling

### Common Error Patterns

```rust
// 1. Missing payment method type
_ => Err(IntegrationError::MissingPaymentMethodType)?

// 2. Failed to obtain auth type
 Err(IntegrationError::FailedToObtainAuthType { context: Default::default() })?

// 3. Currency not supported
Err(IntegrationError::CurrencyNotSupported {
    message: currency.to_string(),
    connector: "CashToCode",
})
```

### Error Response Structure

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct CashtocodeErrorResponse {
    pub error: serde_json::Value,
    pub error_description: String,
    pub errors: Option<Vec<CashtocodeErrors>>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct CashtocodeErrors {
    pub message: String,
    pub path: String,
    #[serde(rename = "type")]
    pub event_type: String,
}
```

## Testing Patterns

### Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classic_reward_request_transformation() {
        // Test ClassicReward request transformation
        let router_data = create_test_router_data(
            common_enums::PaymentMethodType::ClassicReward
        );
        let connector_req = CashtocodePaymentsRequest::try_from(&router_data);
        assert!(connector_req.is_ok());
    }

    #[test]
    fn test_evoucher_request_transformation() {
        // Test Evoucher request transformation
        let router_data = create_test_router_data(
            common_enums::PaymentMethodType::Evoucher
        );
        let connector_req = CashtocodePaymentsRequest::try_from(&router_data);
        assert!(connector_req.is_ok());
    }

    #[test]
    fn test_redirect_form_for_classic_reward() {
        let response_data = CashtocodePaymentsResponseData {
            pay_url: "https://example.com/pay?token=abc".parse().unwrap(),
        };
        let form = get_redirect_form_data(
            common_enums::PaymentMethodType::ClassicReward,
            response_data
        ).unwrap();

        match form {
            RedirectForm::Form { method, form_fields, .. } => {
                assert_eq!(method, Method::Post);
                assert!(form_fields.is_empty());
            }
            _ => panic!("Expected Form variant for ClassicReward"),
        }
    }

    #[test]
    fn test_redirect_form_for_evoucher() {
        let response_data = CashtocodePaymentsResponseData {
            pay_url: "https://example.com/pay?token=abc".parse().unwrap(),
        };
        let form = get_redirect_form_data(
            common_enums::PaymentMethodType::Evoucher,
            response_data
        ).unwrap();

        match form {
            RedirectForm::Form { method, .. } => {
                assert_eq!(method, Method::Get);
            }
            _ => panic!("Expected Form variant for Evoucher"),
        }
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_authorize_flow_for_reward() {
        let connector = Cashtocode::new();

        // Test with Reward payment method
        let request_data = create_test_authorize_request(
            PaymentMethodData::Reward,
            Some(common_enums::PaymentMethodType::ClassicReward)
        );

        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(headers.contains(&(
            "Content-Type".to_string(),
            "application/json".into()
        )));

        // Test request body generation
        let request_body = connector.get_request_body(&request_data).unwrap();
        assert!(request_body.is_some());
    }
}
```

## Integration Checklist

### Pre-Implementation

- [ ] Review connector API documentation for Reward payment support
- [ ] Confirm sub-types supported (ClassicReward, Evoucher, or both)
- [ ] Identify authentication requirements (separate credentials per sub-type?)
- [ ] Understand redirect flow (POST vs GET, form fields vs query params)
- [ ] Confirm webhook payload structure for payment confirmation

### Implementation

- [ ] **Authentication**
  - [ ] Define auth structure with sub-type specific fields if needed
  - [ ] Implement `TryFrom<&ConnectorAuthType>` for auth type
  - [ ] Implement sub-type specific credential extraction

- [ ] **Request Transformation**
  - [ ] Match on `PaymentMethodData::Reward`
  - [ ] Extract `payment_method_type` for sub-type handling
  - [ ] Build connector-specific request with FloatMajorUnit amount
  - [ ] Include customer and transaction identifiers

- [ ] **Response Handling**
  - [ ] Define response status enum
  - [ ] Implement `From<ConnectorStatus> for AttemptStatus`
  - [ ] Handle redirect URL extraction
  - [ ] Implement sub-type specific redirect form construction

- [ ] **Error Handling**
  - [ ] Define error response structure
  - [ ] Handle missing payment method type errors
  - [ ] Handle currency not supported errors
  - [ ] Map connector error codes to attempt statuses

- [ ] **Webhook Support**
  - [ ] Define incoming webhook payload structure
  - [ ] Implement webhook signature verification if required
  - [ ] Map webhook events to payment statuses

### Testing

- [ ] Unit tests for request transformation (both sub-types)
- [ ] Unit tests for response transformation
- [ ] Unit tests for redirect form construction
- [ ] Integration tests for complete authorize flow
- [ ] Webhook handling tests
- [ ] Error scenario tests

### Post-Implementation

- [ ] Update connector documentation
- [ ] Add Reward payment method to connector capabilities
- [ ] Document sub-type specific configuration
- [ ] Test with sandbox credentials for both sub-types
- [ ] Verify webhook handling in staging environment

## Best Practices

### 1. Always Check Payment Method Type

```rust
// Extract payment_method_type for sub-type specific handling
let payment_method_type = router_data
    .request
    .payment_method_type
    .ok_or(IntegrationError::MissingPaymentMethodType)?;
```

### 2. Use FloatMajorUnit for Amounts

Reward payments typically use major currency units (e.g., 10.50 for $10.50) rather than minor units (e.g., 1050 cents).

```rust
amount: FloatMajorUnit,  // Not MinorUnit or StringMinorUnit
```

### 3. Handle Sub-type Differences Explicitly

```rust
match payment_method_type {
    Some(common_enums::PaymentMethodType::ClassicReward) => {
        // ClassicReward specific logic
    }
    Some(common_enums::PaymentMethodType::Evoucher) => {
        // Evoucher specific logic
    }
    _ => Err(IntegrationError::NotImplemented(
        "Unsupported payment method type".to_string(, Default::default())
    ))?,
}
```

### 4. Use CurrencyAuthKey for Multi-Currency Support

Reward connectors often require different credentials per currency.

```rust
#[derive(Default, Debug, Deserialize)]
pub struct CashtocodeAuthType {
    pub auths: HashMap<common_enums::Currency, CashtocodeAuth>,
}
```

### 5. Implement Proper Redirect Form Construction

Different sub-types may require different redirect form constructions:

```rust
// ClassicReward: POST with query params in URL
RedirectForm::Form {
    endpoint: response_data.pay_url.to_string(),
    method: Method::Post,
    form_fields: Default::default(),
}

// Evoucher: GET with query params as form fields
RedirectForm::from((response_data.pay_url, Method::Get))
```

## Cross-References

- [pattern_authorize.md](./pattern_authorize.md) - Generic authorize flow patterns
- [utility_functions_reference.md](./utility_functions_reference.md) - Common utility functions
- Connector-specific implementations in `crates/integrations/connector-integration/src/connectors/`

---

**Document Version**: 1.0
**Last Updated**: 2026-02-19
**Applies to**: Grace-UCS Connector Service
