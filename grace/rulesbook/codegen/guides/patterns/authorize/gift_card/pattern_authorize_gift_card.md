# Gift Card Authorize Flow Pattern

**Payment Method Category**: Prepaid/Gift Card

**Payment Method Variants**: Givex, PaySafeCard

This document provides comprehensive patterns for implementing Gift Card payment processing in Grace-UCS connectors.

## Table of Contents

1. [Overview](#overview)
2. [Gift Card Variants](#gift-card-variants)
3. [Supported Connectors](#supported-connectors)
4. [Request Patterns](#request-patterns)
5. [Response Patterns](#response-patterns)
6. [Implementation Templates](#implementation-templates)
7. [Sub-type Variations](#sub-type-variations)
8. [Common Pitfalls](#common-pitfalls)
9. [Testing Patterns](#testing-patterns)
10. [Integration Checklist](#integration-checklist)

## Overview

Gift Card payments are prepaid payment methods where customers use stored value cards to make purchases. The Grace-UCS system supports two primary gift card variants:

- **Givex**: A gift card and loyalty program provider requiring card number and CVC
- **PaySafeCard**: A prepaid payment method popular in Europe

### Key Characteristics

| Aspect | Details |
|--------|---------|
| Payment Flow | Typically synchronous |
| Authentication | Card number + CVC verification |
| Refund Support | Connector-dependent |
| Partial Redemption | Supported by some connectors |
| Balance Check | May be required before authorization |

## Gift Card Variants

### Domain Types Definition

```rust
// crates/types-traits/domain_types/src/payment_method_data.rs

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GiftCardData {
    Givex(GiftCardDetails),
    PaySafeCard {},
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GiftCardDetails {
    /// The gift card number
    pub number: Secret<String>,
    /// The card verification code.
    pub cvc: Secret<String>,
}
```

### Proto Definition

```protobuf
// crates/types-traits/grpc-api-types/proto/payment_methods.proto

// Givex - Gift card and loyalty program provider
message Givex {
  // The gift card number
  SecretString number = 1;

  // The card verification code
  SecretString cvc = 2;
}

// Paysafecard - Prepaid payment method
message PaySafeCard {
  // Fields will be added as needed for Paysafecard integration
}
```

## Supported Connectors

| Connector | Givex | PaySafeCard | Request Format | Response Type |
|-----------|-------|-------------|----------------|---------------|
| **Adyen** | Supported | Supported | JSON | Synchronous |
| Stripe | NotImplemented | NotImplemented | - | - |
| PayPal | NotImplemented | NotImplemented | - | - |
| Worldpay | NotImplemented | NotImplemented | - | - |
| CyberSource | NotImplemented | NotImplemented | - | - |
| Braintree | NotImplemented | NotImplemented | - | - |
| Fiserv | NotImplemented | NotImplemented | - | - |

**Note**: Most connectors currently mark Gift Card as `NotImplemented`. Adyen is the primary reference implementation.

## Request Patterns

### JSON Pattern (Adyen Implementation)

**Applies to**: Adyen

**Characteristics**:
- Request Format: JSON
- Response Type: Synchronous
- Amount Unit: MinorUnit

#### Request Structure

```rust
// Connector-specific Gift Card Data Structure
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenGiftCardData {
    brand: GiftCardBrand,
    number: Secret<String>,
    cvc: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GiftCardBrand {
    Givex,
    Auriga,
    Babygiftcard,
}

// Payment Method Enum
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum AdyenPaymentMethod<T> {
    // ... other variants
    #[serde(rename = "giftcard")]
    AdyenGiftCard(Box<AdyenGiftCardData>),
    #[serde(rename = "paysafecard")]
    PaySafeCard,
    // ... other variants
}
```

#### Request Transformation

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GiftCardData> for AdyenPaymentMethod<T>
{
    type Error = Error;
    fn try_from(gift_card_data: &GiftCardData) -> Result<Self, Self::Error> {
        match gift_card_data {
            GiftCardData::PaySafeCard {} => Ok(Self::PaySafeCard),
            GiftCardData::Givex(givex_data) => {
                let gift_card_pm = AdyenGiftCardData {
                    brand: GiftCardBrand::Givex,
                    number: givex_data.number.clone(),
                    cvc: givex_data.cvc.clone(),
                };
                Ok(Self::AdyenGiftCard(Box::new(gift_card_pm)))
            }
        }
    }
}
```

#### Full Request Construction

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        AdyenRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
        &GiftCardData,
    )> for AdyenPaymentRequest<T>
{
    type Error = Error;
    fn try_from(
        value: (
            AdyenRouterData<
                RouterDataV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<T>,
                    PaymentsResponseData,
                >,
                T,
            >,
            &GiftCardData,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, gift_card_data) = value;
        let amount = get_amount_data(&item);
        let auth_type = AdyenAuthType::try_from(&item.router_data.connector_auth_type)?;
        let payment_method = PaymentMethod::AdyenPaymentMethod(Box::new(
            AdyenPaymentMethod::try_from(gift_card_data)?,
        ));
        let shopper_interaction = AdyenShopperInteraction::from(&item.router_data);
        let return_url = item.router_data.request.get_router_return_url()?;

        Ok(Self {
            amount,
            merchant_account: auth_type.merchant_account,
            payment_method,
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            return_url,
            shopper_interaction,
            recurring_processing_model: None,
            browser_info: None,
            additional_data: None,
            mpi_data: None,
            telephone_number: item
                .router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            shopper_name: None,
            shopper_email: item
                .router_data
                .resource_common_data
                .get_optional_billing_email(),
            // ... other fields
        })
    }
}
```

### NotImplemented Pattern

**Applies to**: Stripe, PayPal, Worldpay, CyberSource, Braintree, and others

For connectors that do not support Gift Card payments:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GiftCardData> for {ConnectorName}PaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(value: &GiftCardData) -> Result<Self, Self::Error> {
        match value {
            GiftCardData::Givex(_) | GiftCardData::PaySafeCard {} => {
                Err(IntegrationError::NotImplemented(
                    get_unimplemented_payment_method_error_message("{ConnectorName}", Default::default()),
                )
                .into())
            }
        }
    }
}
```

## Response Patterns

### Synchronous Response (Adyen)

Gift Card payments typically return synchronous responses since they don't require customer redirection or async processing.

```rust
#[derive(Debug, Deserialize)]
pub struct AdyenAuthorizeResponse {
    pub id: String,
    pub status: AdyenPaymentStatus,
    pub amount: Option<Amount>,
    pub reference: Option<String>,
    // ... other fields
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdyenPaymentStatus {
    Pending,
    Succeeded,
    Failed,
    // ... other statuses
}
```

### Status Mapping

```rust
impl From<AdyenPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: AdyenPaymentStatus) -> Self {
        match status {
            AdyenPaymentStatus::Succeeded => Self::Charged,
            AdyenPaymentStatus::Pending => Self::Pending,
            AdyenPaymentStatus::Failed => Self::Failure,
            // ... other mappings
        }
    }
}
```

## Implementation Templates

### Full Connector Implementation (Adyen Pattern)

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, PSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundsData, RefundsResponseData,
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
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2,
    connector_types, events::connector_api_logs::ConnectorEvent,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}AuthorizeRequest, {ConnectorName}AuthorizeResponse,
    {ConnectorName}ErrorResponse, {ConnectorName}AuthType,
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations with generic type parameters
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for {ConnectorName}<T>
{
}

// Set up connector using macros
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
        amount_converter: MinorUnit
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
        common_enums::CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.{connector_name}.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = {ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

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
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// Implement Authorize flow
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/v68/payments"))
        }
    }
);
```

### Transformers Implementation

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use domain_types::{
    payment_method_data::{GiftCardData, GiftCardDetails, PaymentMethodData},
    // ... other imports
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// Gift Card Brand Enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GiftCardBrand {
    Givex,
    Auriga,
    Babygiftcard,
}

// Connector-specific Gift Card Data
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}GiftCardData {
    brand: GiftCardBrand,
    number: Secret<String>,
    cvc: Secret<String>,
}

// Payment Method Enum
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum {ConnectorName}PaymentMethod<T> {
    // ... other variants
    #[serde(rename = "giftcard")]
    GiftCard(Box<{ConnectorName}GiftCardData>),
    #[serde(rename = "paysafecard")]
    PaySafeCard,
    // ... other variants
}

// Request Transformation for Gift Card
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GiftCardData> for {ConnectorName}PaymentMethod<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(gift_card_data: &GiftCardData) -> Result<Self, Self::Error> {
        match gift_card_data {
            GiftCardData::PaySafeCard {} => Ok(Self::PaySafeCard),
            GiftCardData::Givex(givex_data) => {
                let gift_card_pm = {ConnectorName}GiftCardData {
                    brand: GiftCardBrand::Givex,
                    number: givex_data.number.clone(),
                    cvc: givex_data.cvc.clone(),
                };
                Ok(Self::GiftCard(Box::new(gift_card_pm)))
            }
        }
    }
}

// Main Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract payment method data
        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::GiftCard(gift_card_data) => {
                {ConnectorName}PaymentMethod::try_from(gift_card_data.as_ref())?
            }
            _ => return Err(IntegrationError::NotImplemented(
                "Only Gift Card payments are supported".to_string(, Default::default())
            ).into()),
        };

        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
            // ... other fields
        })
    }
}
```

## Sub-type Variations

| Sub-type | Connector | URL Pattern | Request Structure | Response Handling |
|----------|-----------|-------------|-------------------|-------------------|
| Givex | Adyen | /v68/payments | `{"type": "giftcard", "brand": "givex", "number": "...", "cvc": "..."}` | Synchronous |
| PaySafeCard | Adyen | /v68/payments | `{"type": "paysafecard"}` | Synchronous |
| Givex | Stripe | N/A | NotImplemented | N/A |
| PaySafeCard | Stripe | N/A | NotImplemented | N/A |

### Variant-Specific Handling

```rust
// In the connector's TryFrom implementation
match gift_card_data {
    GiftCardData::PaySafeCard {} => {
        // PaySafeCard typically requires no additional card data
        // as it's a redirect-based or tokenized payment method
        Ok(Self::PaySafeCard)
    }
    GiftCardData::Givex(givex_data) => {
        // Givex requires card number and CVC
        let gift_card_pm = {ConnectorName}GiftCardData {
            brand: GiftCardBrand::Givex,
            number: givex_data.number.clone(),
            cvc: givex_data.cvc.clone(),
        };
        Ok(Self::GiftCard(Box::new(gift_card_pm)))
    }
}
```

## Common Pitfalls

### 1. Missing Gift Card Data Validation

**Problem**: Not validating required fields before sending to connector.

**Solution**: Always validate gift card data:

```rust
impl GiftCardDetails {
    pub fn validate(&self) -> Result<(), IntegrationError> {
        if self.number.peek().is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "gift_card.number",
            , context: Default::default() });
        }
        if self.cvc.peek().is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "gift_card.cvc",
            , context: Default::default() });
        }
        Ok(())
    }
}
```

### 2. Incorrect Payment Type Mapping

**Problem**: Mapping Givex to generic payment type instead of giftcard-specific type.

**Solution**: Use connector-specific payment types:

```rust
// Correct
common_enums::PaymentMethodType::Givex => Ok(Self::Giftcard),

// Incorrect
common_enums::PaymentMethodType::Givex => Ok(Self::Scheme), // Wrong!
```

### 3. Partial Redemption Handling

**Problem**: Not handling partial redemptions where gift card balance is less than transaction amount.

**Solution**: Check for additional action fields in response:

```rust
// Some connectors support partial redemption
if let Some(remaining_amount) = response.remaining_amount {
    // Handle partial payment scenario
    // May need to request additional payment method
}
```

### 4. Sensitive Data Logging

**Problem**: Logging gift card numbers or CVCs.

**Solution**: Use `Secret<String>` and proper masking:

```rust
// Always use Secret wrapper for sensitive data
pub number: Secret<String>,
pub cvc: Secret<String>,

// Never log raw values
logger::debug!("Gift card number: {:?}", gift_card.number.peek()); // Wrong!
logger::debug!("Processing gift card payment"); // Correct
```

## Testing Patterns

### Unit Test for Gift Card Request Transformation

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use domain_types::payment_method_data::{GiftCardData, GiftCardDetails};
    use hyperswitch_masking::Secret;

    #[test]
    fn test_givex_request_transformation() {
        let givex_data = GiftCardData::Givex(GiftCardDetails {
            number: Secret::new("1234567890".to_string()),
            cvc: Secret::new("123".to_string()),
        });

        let result = AdyenPaymentMethod::<DefaultPCIHolder>::try_from(&givex_data);

        assert!(result.is_ok());
        if let Ok(AdyenPaymentMethod::AdyenGiftCard(data)) = result {
            assert_eq!(data.brand, GiftCardBrand::Givex);
            assert_eq!(data.number.peek(), "1234567890");
            assert_eq!(data.cvc.peek(), "123");
        } else {
            panic!("Expected AdyenGiftCard variant");
        }
    }

    #[test]
    fn test_paysafecard_request_transformation() {
        let paysafecard_data = GiftCardData::PaySafeCard {};

        let result = AdyenPaymentMethod::<DefaultPCIHolder>::try_from(&paysafecard_data);

        assert!(result.is_ok());
        if let Ok(AdyenPaymentMethod::PaySafeCard) = result {
            // PaySafeCard has no additional data
        } else {
            panic!("Expected PaySafeCard variant");
        }
    }

    #[test]
    fn test_not_implemented_connector() {
        let givex_data = GiftCardData::Givex(GiftCardDetails {
            number: Secret::new("1234567890".to_string()),
            cvc: Secret::new("123".to_string()),
        });

        // For connectors that don't support Gift Card
        let result = StripePaymentMethod::try_from(&givex_data);

        assert!(result.is_err());
    }
}
```

### Integration Test

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_gift_card_authorize_flow() {
        let connector = TestConnector::new();

        // Create test request with Gift Card
        let request = create_gift_card_authorize_request();

        // Test headers
        let headers = connector.get_headers(&request).await.unwrap();
        assert!(headers.iter().any(|(k, _)| k == "Content-Type"));

        // Test URL
        let url = connector.get_url(&request).await.unwrap();
        assert!(url.contains("/v68/payments"));

        // Test request body generation
        let body = connector.get_request_body(&request).await.unwrap();
        assert!(body.is_some());
    }

    fn create_gift_card_authorize_request() -> RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<DefaultPCIHolder>, PaymentsResponseData> {
        // Create test data
        todo!("Implement test data creation")
    }
}
```

## Integration Checklist

### Pre-Implementation

- [ ] Review connector's API documentation for Gift Card support
- [ ] Identify supported Gift Card variants (Givex, PaySafeCard, or custom)
- [ ] Determine authentication requirements
- [ ] Understand balance check requirements
- [ ] Identify partial redemption support

### Implementation

- [ ] Add `GiftCardData` enum handling in `TryFrom` implementations
- [ ] Implement variant-specific request structures
- [ ] Add payment type mapping for Gift Card variants
- [ ] Implement proper error handling for unsupported variants
- [ ] Add Secret<String> wrappers for sensitive data

### Testing

- [ ] Unit tests for each Gift Card variant
- [ ] Integration tests with connector sandbox
- [ ] Error handling tests
- [ ] Partial redemption tests (if supported)
- [ ] Balance check tests (if applicable)

### Security

- [ ] Verify no sensitive data in logs
- [ ] Ensure proper masking in error messages
- [ ] Validate input data before sending to connector
- [ ] Review PCI compliance requirements

---

## Related Patterns

- [pattern_authorize.md](./pattern_authorize.md) - General authorize flow patterns
- [pattern_refund.md](./pattern_refund.md) - Refund flow patterns for Gift Card refunds
- [utility_functions_reference.md](../utility_functions_reference.md) - Common utility functions

## Cross-References

- [Adyen Connector](/crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1394) - Full Gift Card implementation reference
- [Payment Method Data](/crates/types-traits/domain_types/src/payment_method_data.rs:308) - GiftCardData enum definition
- [Proto Definitions](/crates/types-traits/grpc-api-types/proto/payment_methods.proto:1220) - Givex and PaySafeCard proto messages
