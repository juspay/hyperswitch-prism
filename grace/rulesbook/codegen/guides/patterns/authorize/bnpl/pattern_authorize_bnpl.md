# BNPL (Buy Now Pay Later) Authorize Flow Pattern

**Payment Method Category**: Buy Now Pay Later (BNPL)

**Last Updated**: 2026-02-19

---

## Table of Contents

1. [Overview](#overview)
2. [Supported BNPL Variants](#supported-bnpl-variants)
3. [Quick Reference](#quick-reference)
4. [Implementation Patterns](#implementation-patterns)
   - [Standard JSON Pattern](#standard-json-pattern)
   - [Redirect-Based Pattern](#redirect-based-pattern)
5. [Connector Analysis](#connector-analysis)
   - [Adyen](#adyen)
   - [Stripe](#stripe)
   - [MultiSafepay](#multisafepay)
6. [Request/Response Patterns](#requestresponse-patterns)
7. [Sub-type Variations](#sub-type-variations)
8. [Common Pitfalls](#common-pitfalls)
9. [Implementation Checklist](#implementation-checklist)
10. [Testing Patterns](#testing-patterns)

---

## Overview

BNPL (Buy Now Pay Later) is a payment method that allows customers to purchase goods and services and pay for them over time in installments. The authorize flow for BNPL typically involves:

1. **Customer Information Collection**: BNPL providers require extensive customer data (billing address, shipping address, email, phone)
2. **Credit Assessment**: Real-time customer creditworthiness checks
3. **Redirect Flow**: Most BNPL providers redirect customers to their platform for approval
4. **Async Confirmation**: Payment confirmation often happens asynchronously via webhooks

### Key Characteristics

- **Response Type**: Primarily Async/Redirect (rarely synchronous)
- **Amount Unit**: Varies by connector (MinorUnit, StringMinorUnit)
- **Required Fields**: Email, phone, billing address, shipping address
- **Authentication**: Typically requires customer authentication on BNPL provider's site

---

## Supported BNPL Variants

Based on `crates/types-traits/domain_types/src/payment_method_data.rs`:

| Variant | Description | Common Connectors |
|---------|-------------|-------------------|
| `KlarnaRedirect` | Klarna BNPL via redirect | Adyen, Stripe |
| `KlarnaSdk` | Klarna via SDK integration | Adyen |
| `AffirmRedirect` | Affirm BNPL (US) | Adyen, Stripe |
| `AfterpayClearpayRedirect` | Afterpay/Clearpay (AU, UK, EU) | Adyen, Stripe |
| `PayBrightRedirect` | PayBright (Canada) | Adyen |
| `WalleyRedirect` | Walley (Nordics) | Adyen |
| `AlmaRedirect` | Alma (France) | Adyen |
| `AtomeRedirect` | Atome (SE Asia) | Adyen |

---

## Quick Reference

### Connector Support Matrix

| Connector | Klarna | Affirm | Afterpay | Alma | Atome | PayBright | Walley |
|-----------|--------|--------|----------|------|-------|-----------|--------|
| **Adyen** | | | | | | | |
| **Stripe** | | | | | | | |
| **MultiSafepay** | | | | | | | |

**Legend**:
-  = Fully Supported
-  = Partially Supported (SDK only)
-  = Not Supported

### Request Format Summary

| Connector | Format | Amount Unit | Auth Type |
|-----------|--------|-------------|-----------|
| Adyen | JSON | StringMinorUnit | HeaderKey (X-Api-Key) |
| Stripe | FormUrlEncoded | MinorUnit | HeaderKey (Bearer) |
| MultiSafepay | JSON | MinorUnit | HeaderKey |

---

## Implementation Patterns

### Standard JSON Pattern

Applies to: **Adyen**, **MultiSafepay**

**Characteristics**:
- Request Format: JSON
- Response Type: Async/Redirect
- Amount Unit: StringMinorUnit (Adyen), MinorUnit (MultiSafepay)

#### Main Connector File Structure

```rust
// crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
    payment_method_data::{PaymentMethodData, PayLaterData},
    router_data_v2::RouterDataV2,
};

// Trait implementations
trait PaymentAuthorizeV2<T> {}

trait ConnectorServiceTrait<T> {}

// Macro setup for BNPL support
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
    ],
    amount_converters: [
        amount_converter: StringMinorUnit  // or MinorUnit
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
    }
);
```

#### Transformers Implementation

```rust
// crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use domain_types::payment_method_data::{PayLaterData, PaymentMethodData};

// BNPL-specific payment method enum
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum {ConnectorName}PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "klarna")]
    Klarna,
    #[serde(rename = "affirm")]
    Affirm,
    #[serde(rename = "afterpaytouch")]
    AfterPay,
    #[serde(rename = "clearpay")]
    ClearPay,
    #[serde(rename = "alma")]
    Alma,
    #[serde(rename = "atome")]
    Atome,
    // ... other BNPL variants
}

// Request transformation for BNPL
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        &PayLaterData,
    )> for {ConnectorName}PaymentMethod<T>
{
    type Error = Error;

    fn try_from(
        (router_data, pay_later_data): (
            &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            &PayLaterData,
        ),
    ) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => {
                // Validate required fields
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_country()?;
                Ok(Self::Klarna)
            }
            PayLaterData::AffirmRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_address()?;
                Ok(Self::Affirm)
            }
            PayLaterData::AfterpayClearpayRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_address()?;
                router_data.resource_common_data.get_shipping_address()?;
                let country = router_data.resource_common_data.get_billing_country()?;
                // Afterpay/Clearpay have regional variants
                match country {
                    CountryAlpha2::GB | CountryAlpha2::ES | CountryAlpha2::FR | CountryAlpha2::IT => {
                        Ok(Self::ClearPay)
                    }
                    _ => Ok(Self::AfterPay),
                }
            }
            PayLaterData::AlmaRedirect { .. } => {
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_address()?;
                router_data.resource_common_data.get_shipping_address()?;
                Ok(Self::Alma)
            }
            PayLaterData::AtomeRedirect { .. } => {
                router_data.resource_common_data.get_billing_email()?;
                router_data.resource_common_data.get_billing_full_name()?;
                router_data.resource_common_data.get_billing_phone_number()?;
                router_data.resource_common_data.get_billing_address()?;
                Ok(Self::Atome)
            }
            _ => Err(ConnectorError::NotImplemented(
                "BNPL variant not supported".to_string()
            )),
        }
    }
}
```

### Redirect-Based Pattern

BNPL payments typically use redirect flows. The connector returns a redirect URL where the customer completes the payment.

```rust
// Response handling for redirect-based BNPL
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}AuthorizeResponse, RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status
        let status = match response.status {
            {ConnectorName}PaymentStatus::Pending => AttemptStatus::AuthenticationPending,
            {ConnectorName}PaymentStatus::Authorized => AttemptStatus::Authorized,
            {ConnectorName}PaymentStatus::Captured => AttemptStatus::Charged,
            {ConnectorName}PaymentStatus::Failed => AttemptStatus::Failure,
            {ConnectorName}PaymentStatus::Cancelled => AttemptStatus::Voided,
        };

        // Create redirect form if URL is present
        let redirection_data = response.redirect_url.as_ref().map(|url| {
            RedirectForm::Uri {
                uri: url.clone(),
            }
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.transaction_id.clone()),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.reference.clone()),
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

## Connector Analysis

### Adyen

**Connector ID**: `adyen`

**BNPL Support**: Full (Klarna, Affirm, Afterpay/Clearpay, PayBright, Walley, Alma, Atome)

**Key Implementation Details**:

```rust
// AdyenPaymentMethod enum includes BNPL variants
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AdyenPaymentMethod<T> {
    #[serde(rename = "klarna")]
    Klarna,
    #[serde(rename = "affirm")]
    AdyenAffirm,
    #[serde(rename = "afterpaytouch")]
    AfterPay,
    #[serde(rename = "clearpay")]
    ClearPay,
    #[serde(rename = "paybright")]
    PayBright,
    #[serde(rename = "walley")]
    Walley,
    #[serde(rename = "alma")]
    AlmaPayLater,
    #[serde(rename = "atome")]
    Atome,
    // ...
}
```

**Required Fields per BNPL Type**:

| BNPL Type | Required Fields |
|-----------|-----------------|
| Klarna | billing_email, customer_id, billing_country |
| Affirm | billing_email, billing_full_name, billing_phone, billing_address |
| Afterpay/Clearpay | billing_email, billing_full_name, billing_address, shipping_address, billing_country |
| PayBright | billing_full_name, billing_phone, billing_email, billing_address, shipping_address, billing_country |
| Walley | billing_phone, billing_email |
| Alma | billing_phone, billing_email, billing_address, shipping_address |
| Atome | billing_email, billing_full_name, billing_phone, billing_address |

**Status Mapping**:

```rust
fn get_adyen_payment_status(
    is_manual_capture: bool,
    adyen_status: AdyenStatus,
    pmt: Option<PaymentMethodType>,
) -> AttemptStatus {
    match adyen_status {
        AdyenStatus::AuthenticationFinished => AttemptStatus::AuthenticationSuccessful,
        AdyenStatus::AuthenticationNotRequired | AdyenStatus::Received => AttemptStatus::Pending,
        AdyenStatus::Authorised => match is_manual_capture {
            true => AttemptStatus::Authorized,
            false => AttemptStatus::Charged,
        },
        AdyenStatus::Cancelled => AttemptStatus::Voided,
        AdyenStatus::ChallengeShopper
        | AdyenStatus::RedirectShopper
        | AdyenStatus::PresentToShopper => AttemptStatus::AuthenticationPending,
        AdyenStatus::Error | AdyenStatus::Refused => AttemptStatus::Failure,
        AdyenStatus::Pending => match pmt {
            Some(PaymentMethodType::Pix) => AttemptStatus::AuthenticationPending,
            _ => AttemptStatus::Pending,
        },
    }
}
```

### Stripe

**Connector ID**: `stripe`

**BNPL Support**: Partial (Klarna, Affirm, AfterpayClearpay)

**Implementation Pattern**:

```rust
// Stripe uses payment_method_types array for BNPL
#[derive(Debug, Eq, PartialEq, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StripePaymentMethodType {
    Affirm,
    AfterpayClearpay,
    Klarna,
    // ... other types
}

// BNPL data structure
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StripePayLaterData {
    #[serde(rename = "payment_method_data[type]")]
    pub payment_method_data_type: StripePaymentMethodType,
}

// Conversion from PayLaterData
impl TryFrom<&PayLaterData> for StripePaymentMethodType {
    type Error = ConnectorError;
    fn try_from(pay_later_data: &PayLaterData) -> Result<Self, Self::Error> {
        match pay_later_data {
            PayLaterData::KlarnaRedirect { .. } => Ok(Self::Klarna),
            PayLaterData::AffirmRedirect {} => Ok(Self::Affirm),
            PayLaterData::AfterpayClearpayRedirect { .. } => Ok(Self::AfterpayClearpay),
            // Stripe doesn't support these via direct API
            PayLaterData::KlarnaSdk { .. }
            | PayLaterData::PayBrightRedirect {}
            | PayLaterData::WalleyRedirect {}
            | PayLaterData::AlmaRedirect {}
            | PayLaterData::AtomeRedirect {} => Err(ConnectorError::NotImplemented(
                get_unimplemented_payment_method_error_message("stripe"),
            )),
        }
    }
}
```

**Important**: Stripe requires shipping address validation for AfterpayClearpay:

```rust
fn validate_shipping_address_against_payment_method(
    shipping_address: &Option<StripeShippingAddress>,
    payment_method: Option<&StripePaymentMethodType>,
) -> Result<(), error_stack::Report<ConnectorError>> {
    match payment_method {
        Some(StripePaymentMethodType::AfterpayClearpay) => match shipping_address {
            Some(address) => {
                let missing_fields = collect_missing_value_keys!(
                    ("shipping.address.line1", address.line1),
                    ("shipping.address.country", address.country),
                    ("shipping.address.zip", address.zip)
                );
                if missing_fields.is_empty() {
                    Ok(())
                } else {
                    Err(ConnectorError::MissingRequiredField {
                        field_name: format!(
                            "Missing fields in shipping address: {:?}",
                            missing_fields
                        ),
                    })?
                }
            }
            None => Err(ConnectorError::MissingRequiredField {
                field_name: "shipping address",
            })?,
        },
        _ => Ok(()),
    }
}
```

### MultiSafepay

**Connector ID**: `multisafepay`

**BNPL Support**: Redirect-based only

**Implementation Pattern**:

```rust
// MultiSafepay treats BNPL as Redirect type
fn get_order_type_from_payment_method<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> Result<Type, error_stack::Report<ConnectorError>> {
    match payment_method_data {
        // ... other variants
        PaymentMethodData::PayLater(_) => Type::Redirect,
        // ...
    }
}
```

**Note**: MultiSafepay doesn't have explicit BNPL gateway mappings - they rely on redirect flow handling.

---

## Request/Response Patterns

### Adyen BNPL Request Structure

```json
{
  "amount": {
    "currency": "USD",
    "value": 1000
  },
  "reference": "order-ref-123",
  "paymentMethod": {
    "type": "klarna"
  },
  "returnUrl": "https://example.com/return",
  "shopperEmail": "customer@example.com",
  "shopperName": {
    "firstName": "John",
    "lastName": "Doe"
  },
  "billingAddress": {
    "city": "New York",
    "country": "US",
    "houseNumberOrName": "123",
    "postalCode": "10001",
    "street": "Main Street"
  },
  "deliveryAddress": {
    "city": "New York",
    "country": "US",
    "houseNumberOrName": "123",
    "postalCode": "10001",
    "street": "Main Street"
  },
  "lineItems": [
    {
      "id": "item-1",
      "amountIncludingTax": 1000,
      "description": "Product Description",
      "quantity": 1
    }
  ],
  "merchantAccount": "YourMerchantAccount"
}
```

### Adyen BNPL Response Structure

```json
{
  "pspReference": "8516146423623123",
  "resultCode": "RedirectShopper",
  "action": {
    "type": "redirect",
    "url": "https://klarna.com/payments/...",
    "method": "GET"
  },
  "paymentMethodType": "klarna"
}
```

### Stripe BNPL Request Structure

```rust
// Form-encoded request
payment_method_data[type]=klarna
&amount=1000
&currency=usd
&payment_method_types[]=klarna
&confirm=true
&return_url=https://example.com/return
```

---

## Sub-type Variations

### Regional Variants

| BNPL Provider | Region | Connector Type Mapping |
|--------------|--------|----------------------|
| **Afterpay** | AU, NZ, US | `AfterPay` |
| **Clearpay** | UK, EU | `ClearPay` |

```rust
// Adyen handles Afterpay/Clearpay region mapping
PayLaterData::AfterpayClearpayRedirect { .. } => {
    let country = router_data.resource_common_data.get_billing_country()?;
    match country {
        // Clearpay for UK and select EU countries
        CountryAlpha2::IT | CountryAlpha2::FR | CountryAlpha2::ES | CountryAlpha2::GB => {
            Ok(Self::ClearPay)
        }
        // Afterpay for other regions (AU, NZ, US)
        _ => Ok(Self::AfterPay),
    }
}
```

### SDK vs Redirect

| Variant | Integration Type | Connector Support |
|---------|------------------|-------------------|
| `KlarnaRedirect` | Redirect flow | Adyen, Stripe |
| `KlarnaSdk` | SDK/Token-based | Adyen only |

```rust
PayLaterData::KlarnaSdk { token } => {
    if token.is_empty() {
        return Err(ConnectorError::MissingRequiredField {
            field_name: "token",
        }.into());
    }
    Ok(Self::Klarna)  // Same connector type, different validation
}
```

---

## Common Pitfalls

### 1. Missing Required Fields

BNPL providers are strict about required customer information:

```rust
// ❌ WRONG: Not validating required fields
PayLaterData::KlarnaRedirect { .. } => Ok(Self::Klarna)

// ✅ RIGHT: Validate all required fields
PayLaterData::KlarnaRedirect { .. } => {
    router_data.resource_common_data.get_billing_email()?;
    router_data.resource_common_data.get_billing_country()?;
    router_data
        .resource_common_data
        .customer_id
        .clone()
        .ok_or_else(|| ConnectorError::MissingRequiredField {
            field_name: "customer_id",
        })?;
    Ok(Self::Klarna)
}
```

### 2. Hardcoded Status Values

```rust
// ❌ WRONG: Hardcoding status
let status = AttemptStatus::AuthenticationPending;

// ✅ RIGHT: Map from connector response
let status = match response.status {
    AdyenStatus::RedirectShopper => AttemptStatus::AuthenticationPending,
    AdyenStatus::Authorised => AttemptStatus::Authorized,
    AdyenStatus::Refused => AttemptStatus::Failure,
    // ... other mappings
};
```

### 3. Not Handling Redirect Response

```rust
// ❌ WRONG: Ignoring redirect URL
let payments_response_data = PaymentsResponseData::TransactionResponse {
    redirection_data: None,  // Customer won't be redirected!
    // ...
};

// ✅ RIGHT: Include redirect data
let redirection_data = response.action.as_ref().map(|action| {
    RedirectForm::Uri {
        uri: action.url.clone(),
    }
});

let payments_response_data = PaymentsResponseData::TransactionResponse {
    redirection_data,
    // ...
};
```

### 4. Missing Shipping Address for AfterpayClearpay

```rust
// AfterpayClearpay requires shipping address validation
// Always validate before sending to connector
validate_shipping_address_against_payment_method(
    &shipping_address,
    Some(&StripePaymentMethodType::AfterpayClearpay),
)?;
```

---

## Implementation Checklist

### Pre-Implementation

- [ ] Review connector's BNPL API documentation
- [ ] Identify supported BNPL variants
- [ ] Understand required fields for each variant
- [ ] Check regional restrictions (e.g., Afterpay vs Clearpay)
- [ ] Verify webhook configuration for async confirmations

### Implementation

- [ ] Add BNPL variants to connector's payment method enum
- [ ] Implement `TryFrom<&PayLaterData>` for payment method mapping
- [ ] Add required field validation for each variant
- [ ] Handle redirect URL extraction in response
- [ ] Implement proper status mapping
- [ ] Add SDK variant support if applicable

### Testing

- [ ] Test each supported BNPL variant
- [ ] Verify redirect flow end-to-end
- [ ] Test webhook handling for status updates
- [ ] Test error scenarios (missing fields, declined payments)
- [ ] Verify regional routing (Afterpay vs Clearpay)

---

## Testing Patterns

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_klarna_redirect_mapping() {
        let router_data = create_test_router_data_with_billing();
        let pay_later_data = PayLaterData::KlarnaRedirect {};

        let result = AdyenPaymentMethod::<DefaultPCIHolder>::try_from((&router_data, &pay_later_data));

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), AdyenPaymentMethod::Klarna));
    }

    #[test]
    fn test_klarna_missing_customer_id() {
        let router_data = create_test_router_data_without_customer_id();
        let pay_later_data = PayLaterData::KlarnaRedirect {};

        let result = AdyenPaymentMethod::<DefaultPCIHolder>::try_from((&router_data, &pay_later_data));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("customer_id"));
    }

    #[test]
    fn test_afterpay_clearpay_regional_routing() {
        // Test UK -> ClearPay
        let uk_router_data = create_test_router_data_with_country(CountryAlpha2::GB);
        let afterpay_data = PayLaterData::AfterpayClearpayRedirect {};

        let result = AdyenPaymentMethod::try_from((&uk_router_data, &afterpay_data));
        assert!(matches!(result.unwrap(), AdyenPaymentMethod::ClearPay));

        // Test AU -> AfterPay
        let au_router_data = create_test_router_data_with_country(CountryAlpha2::AU);
        let result = AdyenPaymentMethod::try_from((&au_router_data, &afterpay_data));
        assert!(matches!(result.unwrap(), AdyenPaymentMethod::AfterPay));
    }
}
```

### Integration Test Example

```rust
#[tokio::test]
async fn test_bnpl_authorize_flow() {
    let connector = {ConnectorName}::<DefaultPCIHolder>::new();

    // Create BNPL authorize request
    let authorize_request = create_bnpl_authorize_request(
        PayLaterData::KlarnaRedirect {}
    );

    // Test headers
    let headers = connector.get_headers(&authorize_request).unwrap();
    assert!(headers.contains(&("Content-Type".to_string(), "application/json".into())));

    // Test URL
    let url = connector.get_url(&authorize_request).unwrap();
    assert!(url.contains("/payments"));

    // Test request body
    let request_body = connector.get_request_body(&authorize_request).unwrap();
    assert!(request_body.is_some());
}
```

---

## Cross-References

- [pattern_authorize.md](./pattern_authorize.md) - Generic authorize flow patterns
- [utility_functions_reference.md](./utility_functions_reference.md) - Helper functions for address/phone formatting
- Connector-specific implementation guides:
  - [Adyen Implementation](../connectors/adyen.md)
  - [Stripe Implementation](../connectors/stripe.md)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-02-19 | Initial BNPL pattern documentation |
