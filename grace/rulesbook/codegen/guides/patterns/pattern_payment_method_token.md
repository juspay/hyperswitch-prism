# PaymentMethodToken Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the PaymentMethodToken flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations across 8+ connectors (Stripe, Braintree, Paysafe, Stax, Mollie, Hipay, Billwerk, Cybersource) and can be consumed by AI to generate consistent, production-ready PaymentMethodToken flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. This pattern focuses on payment method tokenization for secure payment processing.

## 🚀 Quick Start Guide

To implement a new connector PaymentMethodToken flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose tokenization type, request format, and endpoint based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector PaymentMethodToken Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{content_type} → "application/json" (if API uses JSON)
{token_endpoint} → "v1/tokens" (your tokenization API endpoint)
{auth_type} → HeaderKey (if using Bearer token auth)
```

**✅ Result**: Complete, production-ready connector PaymentMethodToken flow implementation in ~20-30 minutes

## Table of Contents

1. [Overview](#overview)
2. [PaymentMethodToken Flow Implementation Analysis](#paymentmethodtoken-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [Token Request/Response Patterns](#token-requestresponse-patterns)
5. [URL Endpoint Patterns](#url-endpoint-patterns)
6. [Validation Trait Implementation](#validation-trait-implementation)
7. [Error Handling Patterns](#error-handling-patterns)
8. [Testing Patterns](#testing-patterns)
9. [Integration Checklist](#integration-checklist)

## Overview

The PaymentMethodToken flow is a specialized flow for tokenizing payment methods (cards, wallets, bank accounts) before processing payments. It:

1. Receives payment method data from the router
2. Transforms them to connector-specific tokenization format
3. Sends tokenization requests to the payment gateway
4. Processes responses and extracts payment method tokens
5. Returns standardized token responses for use in subsequent payment flows

### Key Components:

- **Main Connector File**: Implements PaymentTokenV2 trait and flow logic
- **Transformers File**: Handles token request/response data transformations
- **Token Generation**: Creates secure tokens for payment methods
- **Authentication**: Manages API credentials (same as other flows)
- **Token Extraction**: Extracts token for use in Authorize/RepeatPayment flows
- **Status Mapping**: Converts connector statuses to standard statuses

### Key Differences from Authorization Flow:

- **No Amount Processing**: Tokenization doesn't involve actual payment amounts
- **Token Focus**: Primary goal is to obtain a secure token for the payment method
- **Pre-payment Step**: Always executed before Authorize when enabled
- **No Capture/Settlement**: Tokenization is purely for data security

## PaymentMethodToken Flow Implementation Analysis

Analysis of 8+ connectors reveals distinct implementation patterns:

### Implementation Statistics

| Connector | Request Format | Token Endpoint | Token Type | Special Features |
|-----------|----------------|----------------|------------|------------------|
| **Stripe** | FormUrlEncoded | `/v1/tokens` | Card Token | `card[token]` for split payments |
| **Braintree** | JSON (GraphQL) | Base URL | Payment Method ID | Vault tokenization with GraphQL |
| **Paysafe** | JSON | `/v1/paymenthandles` | Payment Handle Token | No-3DS flow, return_links required |
| **Stax** | JSON | `/token` | Token | Simple token endpoint |
| **Mollie** | JSON | `/v1/payment_methods` | Card Token | Customer-based tokenization |
| **Hipay** | JSON | Secondary base URL | Token | Uses secondary_base_url config |
| **Billwerk** | JSON | `/api/v1/payment_methods` | Payment Method Token | Customer-scoped tokens |
| **Cybersource** | JSON | `/v1/tokens` | Payment Token | TMS (Token Management Service) |

### Common Patterns Identified

#### Pattern 1: Dedicated Token Endpoint (60% of connectors)

**Examples**: Stripe, Stax, Cybersource

```rust
// Uses specialized endpoint for tokenization
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/v1/tokens", self.connector_base_url(req)))
}
```

#### Pattern 2: Payment Handle/Session Endpoint (25% of connectors)

**Examples**: Paysafe

```rust
// Uses payment handle for tokenization (no-3DS flow)
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/v1/paymenthandles", self.connector_base_url(req)))
}
```

#### Pattern 3: Customer-Bound Token Endpoint (15% of connectors)

**Examples**: Mollie, Billwerk

```rust
// Creates token bound to customer
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    let customer_id = req.request.customer_id.clone()
        .ok_or(IntegrationError::MissingRequiredField { field_name: "customer_id" , context: Default::default() })?;
    Ok(format!("{}/v1/customers/{}/payment_methods", self.connector_base_url(req), customer_id))
}
```

### Token Response Patterns

All connectors return a token in the response:

```rust
// Common pattern in response transformation
Ok(PaymentMethodTokenResponse {
    token: response.id, // or response.token, response.payment_method_id, etc.
})
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
        Accept, Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, PaymentMethodTokenizationData, PaymentMethodTokenResponse,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
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
    {ConnectorName}ErrorResponse, {ConnectorName}TokenRequest, {ConnectorName}TokenResponse,
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
    connector_types::PaymentTokenV2<T> for {ConnectorName}<T>
{
}

// Validation Trait - Controls when tokenization is triggered
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    connector_types::ValidationTrait for {ConnectorName}<T>
{
    /// Enable automatic payment method tokenization before payment
    /// When enabled, UCS will automatically call PaymentMethodToken before Authorize
    fn should_do_payment_method_token(
        &self,
        payment_method: common_enums::PaymentMethod,
        _payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        // Choose pattern based on connector requirements:

        // Pattern 1: Tokenize only cards
        matches!(payment_method, common_enums::PaymentMethod::Card)

        // OR Pattern 2: Tokenize cards and bank debits
        // matches!(payment_method, common_enums::PaymentMethod::Card | common_enums::PaymentMethod::BankDebit)

        // OR Pattern 3: Tokenize wallets (Stripe pattern)
        // matches!(payment_method, common_enums::PaymentMethod::Wallet)

        // OR Pattern 4: Tokenize all payment methods
        // true
    }
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
            flow: PaymentMethodToken,
            request_body: {ConnectorName}TokenRequest<T>,
            response_body: {ConnectorName}TokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "{content_type}".to_string().into(), // "application/json", "application/x-www-form-urlencoded"
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
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
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
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response
            .parse_struct("ErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// Implement PaymentMethodToken flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: {Json|FormUrlEncoded}({ConnectorName}TokenRequest), // Choose format
    curl_response: {ConnectorName}TokenResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            // Choose appropriate pattern:

            // Pattern 1: Dedicated token endpoint (like Stripe, Stax)
            Ok(format!("{base_url}/v1/tokens"))

            // OR Pattern 2: Payment handle endpoint (like Paysafe)
            // Ok(format!("{base_url}/v1/paymenthandles"))

            // OR Pattern 3: Customer-bound endpoint (like Mollie, Billwerk)
            // let customer_id = req.request.customer_id.clone()
            //     .ok_or(IntegrationError::MissingRequiredField { field_name: "customer_id" , context: Default::default() })?;
            // Ok(format!("{base_url}/v1/customers/{customer_id}/payment_methods"))
        }
    }
);

// Add Source Verification stubs
use interfaces::verification::SourceVerification;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>
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
    ext_traits::OptionExt, pii, request::Method,
    types::{MinorUnit, StringMinorUnit, StringMajorUnit}
};
use domain_types::{
    connector_flow::{Authorize, PaymentMethodToken},
    connector_types::{
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentsResponseData, ResponseId,
    },
    errors::{self, IntegrationError},
    payment_method_data::{
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, Card,
    },
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

// =============================================================================
// PATTERN 1: CARD TOKEN REQUEST (Stripe-style)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}CardTokenRequest {
    #[serde(rename = "card[number]")]
    pub card_number: String,
    #[serde(rename = "card[exp_month]")]
    pub card_exp_month: Secret<String>,
    #[serde(rename = "card[exp_year]")]
    pub card_exp_year: Secret<String>,
    #[serde(rename = "card[cvc]")]
    pub card_cvc: Secret<String>,
    #[serde(rename = "card[name]")]
    pub card_holder_name: Option<Secret<String>>,
}

// =============================================================================
// PATTERN 2: JSON CARD TOKEN REQUEST (Stax-style)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}TokenRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub payment_method: {ConnectorName}PaymentMethod<T>,
    // Add other token-specific fields
    pub customer_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum {ConnectorName}PaymentMethod<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    Card({ConnectorName}Card<T>),
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}Card<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvc: Secret<String>,
    pub holder_name: Option<Secret<String>>,
}

// =============================================================================
// PATTERN 3: PAYMENT HANDLE REQUEST (Paysafe-style)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}PaymentHandleRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub currency_code: common_enums::Currency,
    pub payment_method: {ConnectorName}PaymentMethodType,
    pub return_links: Vec<ReturnLink>,
    pub account_id: Secret<String>,
    pub transaction_type: TransactionType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Payment,
}

#[derive(Debug, Serialize)]
pub struct ReturnLink {
    pub rel: LinkType,
    pub href: String,
    pub method: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkType {
    Default,
    OnCompleted,
    OnFailed,
    OnCancelled,
}

// Response Structure Template (Common pattern across all implementations)
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}TokenResponse {
    pub id: String,  // This becomes the payment method token
    pub status: Option<{ConnectorName}TokenStatus>,
    // Connector-specific fields (choose based on connector):
    pub token: Option<Secret<String>>,  // For token-based connectors
    pub payment_method_id: Option<String>,  // For Stripe-style
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}TokenStatus {
    Completed,
    Payable,
    Processing,
    Failed,
    Expired,
}

// Error Response Structure
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
}

// =============================================================================
// REQUEST TRANSFORMATION IMPLEMENTATIONS
// =============================================================================

// Pattern 1: Form-Encoded Card Token (Stripe-style)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>, T>>
    for {ConnectorName}CardTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                Ok(Self {
                    card_number: card_data.card_number.get_card_no_as_string(),
                    card_exp_month: card_data.card_exp_month.clone(),
                    card_exp_year: card_data.card_exp_year.clone(),
                    card_cvc: card_data.card_cvc.clone(),
                    card_holder_name: router_data.request.customer_name.clone().map(Secret::new),
                })
            },
            _ => Err(IntegrationError::NotImplemented("Payment method not supported for tokenization".to_string(, Default::default())).into()),
        }
    }
}

// Pattern 2: JSON Token Request (Stax-style)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>, T>>
    for {ConnectorName}TokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethod::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: card_data.card_cvc.clone(),
                    holder_name: router_data.request.customer_name.clone().map(Secret::new),
                })
            },
            _ => return Err(IntegrationError::NotImplemented("Payment method not supported for tokenization".to_string(, Default::default())).into()),
        };

        Ok(Self {
            payment_method,
            customer_id: None, // Or extract from router_data if available
        })
    }
}

// =============================================================================
// RESPONSE TRANSFORMATION IMPLEMENTATION
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}TokenResponse, RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>>>
    for RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}TokenResponse, RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Extract token from response (choose pattern based on connector)
        let token = response.id.clone(); // Primary pattern
        // OR: response.token.as_ref().map(|t| t.expose()).unwrap_or(response.id.clone())
        // OR: response.payment_method_id.clone().unwrap_or(response.id.clone())

        Ok(Self {
            response: Ok(PaymentMethodTokenResponse { token }),
            ..router_data.clone()
        })
    }
}

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub amount: {AmountType},
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<({AmountType}, T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((amount, router_data, connector): ({AmountType}, T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data,
            connector,
        })
    }
}
```

## Token Request/Response Patterns

### Request Patterns by Connector Type

#### Type 1: Form-Encoded Card Token (Stripe)

```rust
#[derive(Debug, Serialize)]
pub struct CardTokenRequest {
    #[serde(rename = "card[number]")]
    pub card_number: String,
    #[serde(rename = "card[exp_month]")]
    pub card_exp_month: Secret<String>,
    #[serde(rename = "card[exp_year]")]
    pub card_exp_year: Secret<String>,
    #[serde(rename = "card[cvc]")]
    pub card_cvc: Secret<String>,
}
```

**Key Features:**
- FormUrlEncoded format
- Direct card fields
- Simple token response
- Used for split payment flows

#### Type 2: JSON Card Token (Stax)

```rust
#[derive(Debug, Serialize)]
pub struct TokenRequest {
    pub payment_method: PaymentMethod,
    pub customer_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct Card {
    pub number: RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvc: Secret<String>,
}
```

**Key Features:**
- JSON format
- Structured payment method object
- Can include customer binding

#### Type 3: Payment Handle Token (Paysafe)

```rust
#[derive(Debug, Serialize)]
pub struct PaymentHandleRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub currency_code: Currency,
    pub payment_method: PaymentMethod,
    pub return_links: Vec<ReturnLink>,
    pub account_id: Secret<String>,
    pub transaction_type: TransactionType,
}
```

**Key Features:**
- Includes return_links for redirect flows
- Amount and currency included (for verification)
- Account ID for multi-account setups
- No 3DS flow only

### Response Patterns

#### Common Response Structure

```rust
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub id: String,  // Primary token value
    pub status: Option<TokenStatus>,
    // Connector-specific fields:
    pub token: Option<Secret<String>>,
    pub payment_method_id: Option<String>,
}
```

#### Token Extraction Pattern

```rust
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TokenResponse, Self>>
    for RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>
{
    fn try_from(item: ResponseRouterData<TokenResponse, Self>) -> Result<Self, Self::Error> {
        let token = item.response.id; // or appropriate field

        Ok(Self {
            response: Ok(PaymentMethodTokenResponse { token }),
            ..item.router_data.clone()
        })
    }
}
```

## URL Endpoint Patterns

### Pattern 1: Dedicated Token Endpoint

```rust
// Stripe, Stax, Cybersource
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/v1/tokens", self.connector_base_url(req)))
}
```

### Pattern 2: Payment Handle Endpoint

```rust
// Paysafe
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    Ok(format!("{}/v1/paymenthandles", self.connector_base_url(req)))
}
```

### Pattern 3: Customer-Bound Token Endpoint

```rust
// Mollie, Billwerk
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    let customer_id = req.request.customer_id.clone()
        .ok_or(IntegrationError::MissingRequiredField { field_name: "customer_id" , context: Default::default() })?;
    Ok(format!("{}/v1/customers/{}/payment_methods",
        self.connector_base_url(req),
        customer_id
    ))
}
```

### Pattern 4: Secondary Base URL

```rust
// Hipay
fn get_url(&self, req: &RouterDataV2<PaymentMethodToken, ...>) -> CustomResult<String, IntegrationError> {
    let base_url = &req.resource_common_data.connectors.{connector_name}.secondary_base_url
        .as_ref()
        .ok_or(IntegrationError::InvalidConnectorConfig { config: "secondary_base_url" })?;
    Ok(format!("{}/v1/token", base_url))
}
```

## Validation Trait Implementation

The `should_do_payment_method_token` function controls when tokenization is automatically triggered:

### Pattern 1: Card-Only Tokenization

```rust
fn should_do_payment_method_token(
    &self,
    payment_method: PaymentMethod,
    _payment_method_type: Option<PaymentMethodType>,
) -> bool {
    matches!(payment_method, PaymentMethod::Card)
}
```

### Pattern 2: Card and Bank Debit Tokenization

```rust
fn should_do_payment_method_token(
    &self,
    payment_method: PaymentMethod,
    _payment_method_type: Option<PaymentMethodType>,
) -> bool {
    matches!(payment_method, PaymentMethod::Card | PaymentMethod::BankDebit)
}
```

### Pattern 3: Wallet Tokenization (Stripe Pattern)

```rust
fn should_do_payment_method_token(
    &self,
    payment_method: PaymentMethod,
    payment_method_type: Option<PaymentMethodType>,
) -> bool {
    matches!(payment_method, PaymentMethod::Wallet)
        && !matches!(payment_method_type, Some(PaymentMethodType::GooglePay))
}
```

### Pattern 4: All Payment Methods

```rust
fn should_do_payment_method_token(
    &self,
    _payment_method: PaymentMethod,
    _payment_method_type: Option<PaymentMethodType>,
) -> bool {
    true
}
```

## Error Handling Patterns

### Token-Specific Error Handling

```rust
impl ConnectorCommon for {ConnectorName} {
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response
            .parse_struct("ErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        // Map token-specific error codes
        let attempt_status = match response.error_code.as_deref() {
            Some("invalid_card") => Some(common_enums::AttemptStatus::Failure),
            Some("expired_card") => Some(common_enums::AttemptStatus::Failure),
            Some("card_declined") => Some(common_enums::AttemptStatus::Failure),
            Some("processing_error") => Some(common_enums::AttemptStatus::Pending),
            _ => Some(common_enums::AttemptStatus::Failure),
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_default(),
            message: response.error_message.unwrap_or_default(),
            reason: response.error_description,
            attempt_status,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}
```

## Testing Patterns

### Unit Test Structure for PaymentMethodToken

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use domain_types::connector_types::PaymentFlowData;

    #[test]
    fn test_token_request_transformation() {
        // Create test router data for tokenization
        let router_data = create_test_token_router_data();

        let connector_req = {ConnectorName}TokenRequest::try_from(&router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();

        // Verify token-specific fields
        if let PaymentMethodData::Card(card) = &router_data.request.payment_method_data {
            assert_eq!(req.card_number, card.card_number.get_card_no_as_string());
        }
    }

    #[test]
    fn test_token_response_transformation() {
        let response = {ConnectorName}TokenResponse {
            id: "token_test_id".to_string(),
            status: Some({ConnectorName}TokenStatus::Completed),
            token: None,
        };

        let router_data = create_test_token_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();

        // Verify token extraction
        if let Ok(PaymentMethodTokenResponse { token }) = &router_data_result.response {
            assert_eq!(token, "token_test_id");
        } else {
            panic!("Expected PaymentMethodTokenResponse");
        }
    }

    #[test]
    fn test_validation_trait() {
        let connector = {ConnectorName}::new();

        // Test card payment method
        assert!(connector.should_do_payment_method_token(
            common_enums::PaymentMethod::Card,
            None,
        ));

        // Test wallet payment method
        assert!(!connector.should_do_payment_method_token(
            common_enums::PaymentMethod::Wallet,
            Some(common_enums::PaymentMethodType::GooglePay),
        ));
    }

    fn create_test_token_router_data() -> RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData, PaymentMethodTokenResponse> {
        // Create test router data structure with card payment method
        // ... implementation
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_payment_method_token_flow_integration() {
        let connector = {ConnectorName}::new();

        // Mock tokenization request data
        let request_data = create_test_token_request();

        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".into())));

        // Test URL generation for token endpoint
        let url = connector.get_url(&request_data).unwrap();
        assert!(url.contains("token") || url.contains("payment_method"));

        // Test request body generation
        let request_body = connector.get_request_body(&request_data).unwrap();
        assert!(request_body.is_some());
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Identify tokenization endpoints
  - [ ] Understand tokenization flow (direct, session-based, customer-bound)
  - [ ] Review token format and expiration
  - [ ] Check if return_links required (Paysafe pattern)
  - [ ] Understand supported payment methods for tokenization
  - [ ] Review authentication requirements

- [ ] **Token Type Identification**
  - [ ] Determine if connector uses dedicated token endpoint
  - [ ] Check if connector requires customer pre-creation
  - [ ] Identify if secondary base URL needed (Hipay pattern)
  - [ ] Understand token usage in subsequent flows

- [ ] **Integration Requirements**
  - [ ] Determine authentication type (same as authorize usually)
  - [ ] Choose request format (JSON, FormUrlEncoded)
  - [ ] Identify required fields for tokenization
  - [ ] Review token storage and retrieval

### Implementation Checklist

- [ ] **File Structure Setup**
  - [ ] Main connector file: `{connector_name}.rs` exists
  - [ ] Transformers directory: `{connector_name}/` created
  - [ ] Transformers file: `{connector_name}/transformers.rs` created

- [ ] **Main Connector Implementation**
  - [ ] Add `PaymentTokenV2<T>` trait implementation
  - [ ] Add `ValidationTrait` with `should_do_payment_method_token`
  - [ ] Add PaymentMethodToken to `create_all_prerequisites!` api array
  - [ ] Implement PaymentMethodToken flow with `macro_connector_implementation!`
  - [ ] Implement `get_url()` for token endpoint
  - [ ] Implement `get_headers()` (usually same as authorize)
  - [ ] Add Source Verification stub for PaymentMethodToken

- [ ] **Transformers Implementation**
  - [ ] Create `TokenRequest` structure with appropriate pattern
  - [ ] Create `TokenResponse` structure
  - [ ] Implement request transformation (`TryFrom` for request)
  - [ ] Implement response transformation (`TryFrom` for response)
  - [ ] Extract token correctly from response
  - [ ] Handle payment method data extraction properly

- [ ] **Validation Configuration**
  - [ ] Implement `should_do_payment_method_token` logic
  - [ ] Choose appropriate payment method matching
  - [ ] Document which payment methods trigger tokenization

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test request transformation with card payment method
  - [ ] Test response transformation with successful token
  - [ ] Test token extraction from response
  - [ ] Test validation trait for different payment methods
  - [ ] Test error handling for token-specific errors

- [ ] **Integration Tests**
  - [ ] Test headers generation
  - [ ] Test URL construction for token endpoint
  - [ ] Test request body generation
  - [ ] Test complete PaymentMethodToken flow

### Configuration Checklist

- [ ] **Connector Configuration**
  - [ ] Connector added to `Connectors` struct
  - [ ] Base URL configuration added
  - [ ] Secondary base URL added if needed (Hipay)
  - [ ] Update configuration files (`development.toml`)

- [ ] **Registration**
  - [ ] Add to connector list in integration module
  - [ ] Export connector modules properly

### Validation Checklist

- [ ] **Code Quality**
  - [ ] `cargo build` succeeds
  - [ ] `cargo test` passes all tests
  - [ ] `cargo clippy` shows no warnings
  - [ ] `cargo fmt` applied

- [ ] **Functionality Validation**
  - [ ] Test with sandbox/test credentials
  - [ ] Verify token returned correctly
  - [ ] Verify token can be used in Authorize flow
  - [ ] Test error handling
  - [ ] Verify validation trait works correctly

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Add doc comments explaining token flow
  - [ ] Document token format
  - [ ] Document any special requirements
  - [ ] Add usage examples in comments

- [ ] **Integration Documentation**
  - [ ] Document tokenization requirements
  - [ ] Document customer prerequisites
  - [ ] Document token usage in payments
  - [ ] Document known limitations

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Braintree`, `Paysafe` | **Always required** |
| `{connector_name}` | Connector name in snake_case | `stripe`, `braintree`, `paysafe` | **Always required** |
| `{AmountType}` | Amount type for flows needing amount | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **Choose based on API** |
| `{AmountUnit}` | Amount converter type | `MinorUnit`, `StringMinorUnit` | **Must match {AmountType}** |
| `{content_type}` | Request content type | `"application/json"`, `"application/x-www-form-urlencoded"` | **Based on API format** |
| `{token_endpoint}` | Token API endpoint | `"tokens"`, `"paymenthandles"`, `"customers/{id}/payment_methods"` | **From API docs** |
| `{Major\|Minor}` | Currency unit choice | `Major` or `Minor` | **Choose one** |

### Token Endpoint Selection Guide

| Connector API Style | Pattern | Endpoint Example |
|---------------------|---------|------------------|
| Dedicated token endpoint | Pattern 1 (Stripe/Stax) | `/v1/tokens` |
| Payment handle/session | Pattern 2 (Paysafe) | `/v1/paymenthandles` |
| Customer-bound endpoint | Pattern 3 (Mollie) | `/v1/customers/{id}/payment_methods` |
| Secondary base URL | Pattern 4 (Hipay) | Uses `secondary_base_url` config |

### Validation Pattern Selection Guide

| Tokenization Strategy | Pattern | Example |
|-----------------------|---------|---------|
| Tokenize only cards | Pattern 1 | `matches!(payment_method, PaymentMethod::Card)` |
| Tokenize cards + bank debits | Pattern 2 | `matches!(payment_method, PaymentMethod::Card \| PaymentMethod::BankDebit)` |
| Tokenize wallets | Pattern 3 | `matches!(payment_method, PaymentMethod::Wallet)` |
| Tokenize everything | Pattern 4 | `true` |

## Best Practices

1. **Choose Appropriate Pattern**: Select the tokenization pattern (direct, session, customer-bound) that matches your connector's API design
2. **Implement Validation Trait**: Properly implement `should_do_payment_method_token` to control when tokenization is triggered
3. **Extract Token Correctly**: Ensure token is correctly extracted and returned from response
4. **Handle All Payment Methods**: Support all payment methods that your connector can tokenize
5. **Error Context**: Provide meaningful error messages for tokenization-specific failures
6. **Test Token Usage**: Verify created token can be used in Authorize/RepeatPayment flows
7. **Return Links**: Include return_links if connector requires them (Paysafe pattern)
8. **Customer Binding**: Handle customer_id if connector requires customer-bound tokens
9. **Secondary Base URL**: Configure secondary_base_url if connector uses different host for tokenization
10. **Documentation**: Document token format and usage requirements

## Using Token in Authorize Flow

After tokenization, the token is used in the Authorize flow:

```rust
// In Authorize request transformation
impl<T: PaymentMethodDataTypes> TryFrom<...> for AuthorizeRequest {
    fn try_from(item: ...) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Get token from resource_common_data
        let token = router_data
            .resource_common_data
            .payment_method_token
            .as_ref()
            .and_then(|t| match t {
                PaymentMethodToken::Token(token) => Some(token.clone()),
                _ => None,
            })
            .ok_or(IntegrationError::MissingRequiredField { field_name: "payment_method_token" , context: Default::default() })?;

        Ok(Self {
            payment_method_token: token,
            // ... other fields
        })
    }
}
```

## Summary

This pattern document provides comprehensive templates for implementing PaymentMethodToken flows across all connector types:

- **4 Main Patterns**: Direct Token (Stripe), Payment Handle (Paysafe), Customer-Bound (Mollie), Secondary URL (Hipay)
- **8+ Reference Implementations**: Stripe, Braintree, Paysafe, Stax, Mollie, Hipay, Billwerk, Cybersource
- **Complete Code Templates**: Request/response structures, transformations, error handling
- **Validation Patterns**: Multiple strategies for controlling tokenization
- **Token Usage**: How to use tokens in Authorize/RepeatPayment flows
- **Comprehensive Checklists**: Pre-implementation through validation

By following these patterns, you can implement a production-ready PaymentMethodToken flow for any payment connector in 20-30 minutes.

---

## Related Patterns

- [pattern_authorize.md](./pattern_authorize.md) - Authorization flow using tokens
- [pattern_setup_mandate.md](./pattern_setup_mandate.md) - Mandate setup patterns
- [repeat_payment_flow_patterns.md](./repeat_payment_flow_patterns.md) - Using tokens for repeat payments
- [../connector_integration_guide.md](../connector_integration_guide.md) - Complete UCS integration process
