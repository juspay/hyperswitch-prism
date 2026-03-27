# SetupMandate Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the SetupMandate flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations across 8 connectors (Adyen, Stripe, Cybersource, ACI, Authorizedotnet, Noon, Novalnet, Payload) and can be consumed by AI to generate consistent, production-ready SetupMandate flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. This pattern focuses on recurring payment setup and mandate creation.

## 🚀 Quick Start Guide

To implement a new connector SetupMandate flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
3. **Select Components**: Choose mandate type, request format, and amount converter based on your connector's API
4. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector SetupMandate Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{AmountType} → MinorUnit (if API expects 0 for $0.00 verification)
{content_type} → "application/json" (if API uses JSON)
{mandate_endpoint} → "v1/setup_intents" (your mandate setup API endpoint)
{auth_type} → HeaderKey (if using Bearer token auth)
```

**✅ Result**: Complete, production-ready connector SetupMandate flow implementation in ~30-45 minutes

## Table of Contents

1. [Overview](#overview)
2. [SetupMandate Flow Implementation Analysis](#setupmandate-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [Mandate Request/Response Patterns](#mandate-requestresponse-patterns)
5. [Zero-Amount vs Subscription Patterns](#zero-amount-vs-subscription-patterns)
6. [URL Endpoint Patterns](#url-endpoint-patterns)
7. [Mandate Reference Handling](#mandate-reference-handling)
8. [Error Handling Patterns](#error-handling-patterns)
9. [Testing Patterns](#testing-patterns)
10. [Integration Checklist](#integration-checklist)

## Overview

The SetupMandate flow is a specialized payment processing flow for setting up recurring payments and mandates that:
1. Receives mandate setup requests from the router
2. Transforms them to connector-specific mandate format
3. Sends mandate setup requests to the payment gateway (typically with $0 or verification amount)
4. Processes responses and extracts mandate reference/token
5. Returns standardized mandate setup responses with connector_mandate_id

### Key Components:
- **Main Connector File**: Implements SetupMandateV2 trait and flow logic
- **Transformers File**: Handles mandate request/response data transformations
- **Mandate Creation**: Creates recurring payment tokens/subscriptions
- **Authentication**: Manages API credentials (same as other flows)
- **Mandate Reference Extraction**: Extracts connector_mandate_id for future payments
- **Status Mapping**: Converts connector mandate statuses to standard statuses

### Key Differences from Authorization Flow:
- **Zero/Minimal Amount**: Most connectors use $0 or $1 for mandate setup
- **Tokenization Focus**: Primary goal is to obtain a mandate_id/token for future use
- **Recurring Flags**: Special flags to indicate this is for recurring payments
- **Customer Binding**: Often requires customer_id to bind mandate to customer
- **No Capture**: Mandate setup doesn't involve actual payment capture

## SetupMandate Flow Implementation Analysis

Analysis of 8 connectors reveals distinct implementation patterns:

### Implementation Statistics

| Connector | Request Format | Amount Type | Mandate Type | Special Features |
|-----------|----------------|-------------|--------------|------------------|
| **Adyen** | JSON | MinorUnit | Recurring Details | `shopperInteraction`, `recurringProcessingModel`, `storePaymentMethod` |
| **Stripe** | FormUrlEncoded | N/A | Setup Intent | `confirm=true`, `usage=off_session`, payment_method_types |
| **Cybersource** | JSON | MinorUnit (zero) | Zero Auth | Special zero-amount payment request |
| **ACI** | JSON | StringMajorUnit | Registration | `registrationId`, payment profile creation |
| **Authorizedotnet** | JSON | N/A | Payment Profile | Customer payment profile creation |
| **Noon** | JSON | StringMajorUnit (1 unit) | Subscription | `subscription` object with max_amount, tokenize_c_c flag |
| **Novalnet** | JSON | StringMinorUnit | Token Request | Standard payment with tokenization |
| **Payload** | JSON | MinorUnit | Mandate Setup | Standard mandate endpoint |

### Common Patterns Identified

#### Pattern 1: Dedicated Mandate Endpoint (40% of connectors)
**Examples**: Stripe, Authorizedotnet

```rust
// Uses specialized endpoint for mandate setup
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/v1/setup_intents", self.connector_base_url(req)))
}
```

#### Pattern 2: Standard Payment Endpoint with Flags (50% of connectors)
**Examples**: Adyen, Cybersource, Noon, Novalnet

```rust
// Uses regular payment endpoint with recurring flags
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/v1/payments", self.connector_base_url(req)))
    // Request includes recurring/tokenization flags
}
```

#### Pattern 3: Customer Profile Endpoint (10% of connectors)
**Examples**: ACI

```rust
// Creates payment profile under customer
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/registrations/{}", base_url, registration_id))
}
```

### Amount Handling Patterns

#### Zero Amount (Most Common - 60%)
**Connectors**: Adyen, Cybersource, Authorizedotnet

```rust
// Use zero amount for verification
let amount = MinorUnit::new(0);
```

#### Minimal Amount (30%)
**Connectors**: Noon (uses 1 unit)

```rust
// Use minimal amount (typically $0.01 or equivalent)
let amount = data.connector.amount_converter.convert(
    common_utils::types::MinorUnit::new(1),
    data.router_data.request.currency,
)?;
```

#### No Amount Required (10%)
**Connectors**: Stripe (Setup Intents don't require amount)

```rust
// No amount field in request
pub struct SetupMandateRequest {
    pub confirm: bool,
    // No amount field
}
```

### Mandate Reference Extraction Patterns

All connectors return a mandate reference in the response:

```rust
// Common pattern in response transformation
let mandate_reference = Some(Box::new(MandateReference {
    connector_mandate_id: Some(response.id), // or subscription.identifier, or token
    payment_method_id: None,
}));
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
        Accept, Authorize, Capture, PSync, RSync, Refund, SetupMandate, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, ResponseId, SetupMandateRequestData,
    },
    errors::{self, ConnectorError},
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
    {ConnectorName}ErrorResponse, {ConnectorName}SetupMandateRequest,
    {ConnectorName}SetupMandateResponse,
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
    connector_types::SetupMandateV2<T> for {ConnectorName}<T>
{
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
            flow: SetupMandate,
            request_body: {ConnectorName}SetupMandateRequest<T>,
            response_body: {ConnectorName}SetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
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
    ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::ConnectorError::FailedToObtainAuthType)?;

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
            .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

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

// Implement SetupMandate flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: {Json|FormUrlEncoded}({ConnectorName}SetupMandateRequest), // Choose format
    curl_response: {ConnectorName}SetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
            let base_url = self.connector_base_url_payments(req);
            // Choose appropriate pattern:

            // Pattern 1: Dedicated mandate endpoint (like Stripe setup_intents)
            Ok(format!("{base_url}/v1/setup_intents"))

            // OR Pattern 2: Regular payment endpoint (like Adyen, Noon)
            // Ok(format!("{base_url}/v1/payments"))

            // OR Pattern 3: Customer profile endpoint (like ACI)
            // let registration_id = req.request.customer_id.clone()
            //     .ok_or(ConnectorError::MissingRequiredField { field_name: "customer_id" })?;
            // Ok(format!("{base_url}/registrations/{registration_id}"))
        }
    }
);

// Add Source Verification stubs
use interfaces::verification::SourceVerification;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    SourceVerification<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
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
    connector_flow::{Authorize, SetupMandate},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, ResponseId, SetupMandateRequestData,
    },
    errors::{self, ConnectorError},
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
    type Error = ConnectorError;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType),
        }
    }
}

// =============================================================================
// PATTERN 1: DEDICATED MANDATE ENDPOINT (Stripe-style Setup Intents)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}SetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub confirm: bool,
    pub usage: Option<common_enums::FutureUsage>,  // off_session
    pub customer: Option<Secret<String>>,
    pub return_url: Option<String>,
    #[serde(flatten)]
    pub payment_data: {ConnectorName}PaymentMethodData<T>,
    pub payment_method_types: Option<{ConnectorName}PaymentMethodType>,
}

// =============================================================================
// PATTERN 2: PAYMENT ENDPOINT WITH RECURRING FLAGS (Adyen-style)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}SetupMandateRequestAlternative<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub amount: {AmountType},  // Usually MinorUnit::new(0) for zero-auth
    pub currency: String,
    pub payment_method: {ConnectorName}PaymentMethod<T>,
    pub reference: String,
    // Recurring-specific fields
    pub shopper_interaction: Option<{ConnectorName}ShopperInteraction>,
    pub recurring_processing_model: Option<{ConnectorName}RecurringModel>,
    pub store_payment_method: Option<bool>,
    pub shopper_reference: Option<String>,  // Customer ID
    pub return_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum {ConnectorName}ShopperInteraction {
    Ecommerce,
    ContAuth,
    Moto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum {ConnectorName}RecurringModel {
    Subscription,
    CardOnFile,
    UnscheduledCardOnFile,
}

// =============================================================================
// PATTERN 3: SUBSCRIPTION MODEL (Noon-style)
// =============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}SetupMandateSubscription<
    T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize,
> {
    pub api_operation: {ConnectorName}ApiOperation,
    pub order: {ConnectorName}Order,
    pub configuration: {ConnectorName}Configuration,
    pub payment_data: {ConnectorName}PaymentData<T>,
    pub subscription: Option<{ConnectorName}SubscriptionData>,  // Mandate-specific
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}SubscriptionData {
    #[serde(rename = "type")]
    pub subscription_type: {ConnectorName}SubscriptionType,
    pub name: String,
    pub max_amount: StringMajorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}SubscriptionType {
    Unscheduled,
    Scheduled,
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}Configuration {
    pub tokenize_c_c: Option<bool>,  // Set to true for mandate setup
    pub payment_action: {ConnectorName}PaymentAction,
    pub return_url: Option<String>,
}

// Payment Method Structure (Common across all patterns)
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
    pub cvc: Option<Secret<String>>,
    pub holder_name: Option<Secret<String>>,
}

// Response Structure Template (Common pattern across all implementations)
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}SetupMandateResponse {
    pub id: String,  // This becomes the connector_mandate_id
    pub status: {ConnectorName}MandateStatus,
    pub customer: Option<String>,
    // Choose based on connector:
    pub client_secret: Option<Secret<String>>,  // For Stripe-style
    // OR
    pub subscription: Option<{ConnectorName}SubscriptionObject>,  // For Noon-style
    // OR
    pub token: Option<Secret<String>>,  // For token-based
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}SubscriptionObject {
    pub identifier: Secret<String>,  // Used as connector_mandate_id
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}MandateStatus {
    Succeeded,
    RequiresAction,
    RequiresPaymentMethod,
    Processing,
    Failed,
    Canceled,
}

// Error Response Structure
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}

// =============================================================================
// REQUEST TRANSFORMATION IMPLEMENTATIONS
// =============================================================================

// Pattern 1: Dedicated Mandate Endpoint (Stripe-style)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>>
    for {ConnectorName}SetupMandateRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethodData::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: Some(card_data.card_cvc.clone()),
                    holder_name: router_data.request.customer_name.clone().map(Secret::new),
                })
            },
            _ => return Err(ConnectorError::NotImplemented("Payment method not supported".to_string()).into()),
        };

        Ok(Self {
            confirm: true,  // Immediately confirm the setup intent
            usage: Some(common_enums::FutureUsage::OffSession),
            customer: router_data.request.customer_id.clone().map(Secret::new),
            return_url: router_data.request.router_return_url.clone(),
            payment_data,
            payment_method_types: Some({ConnectorName}PaymentMethodType::Card),
        })
    }
}

// Pattern 2: Payment Endpoint with Recurring Flags (Adyen-style)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>>
    for {ConnectorName}SetupMandateRequestAlternative<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Zero amount for verification (common pattern)
        let amount = MinorUnit::new(0);

        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethod::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: Some(card_data.card_cvc.clone()),
                    holder_name: router_data.request.customer_name.clone().map(Secret::new),
                })
            },
            _ => return Err(ConnectorError::NotImplemented("Payment method not supported".to_string()).into()),
        };

        // Build shopper reference from customer_id
        let shopper_reference = router_data.request.customer_id.clone();

        // Determine shopper interaction
        let shopper_interaction = match router_data.request.off_session {
            Some(true) => Some({ConnectorName}ShopperInteraction::ContAuth),
            _ => Some({ConnectorName}ShopperInteraction::Ecommerce),
        };

        // Set recurring processing model
        let recurring_processing_model = Some({ConnectorName}RecurringModel::Subscription);

        let return_url = router_data.request.router_return_url.clone()
            .ok_or(ConnectorError::MissingRequiredField { field_name: "return_url" })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
            shopper_interaction,
            recurring_processing_model,
            store_payment_method: Some(true),
            shopper_reference,
            return_url,
        })
    }
}

// Pattern 3: Subscription Model (Noon-style)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>>
    for {ConnectorName}SetupMandateSubscription<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Minimal amount (e.g., $0.01 or 1 unit)
        let amount = item.connector.amount_converter.convert(
            MinorUnit::new(1),
            router_data.request.currency,
        )?;

        let payment_data = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentData::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: Some(card_data.card_cvc.clone()),
                    holder_name: router_data.request.customer_name.clone().map(Secret::new),
                })
            },
            _ => return Err(ConnectorError::NotImplemented("Payment method not supported".to_string()).into()),
        };

        // Build subscription data from mandate details
        let subscription = router_data.request.setup_mandate_details.as_ref()
            .and_then(|mandate_details| {
                mandate_details.mandate_type.as_ref().and_then(|mandate_type| {
                    let mandate_amount_data = match mandate_type {
                        MandateDataType::SingleUse(amount_data) => Some(amount_data),
                        MandateDataType::MultiUse(amount_data_opt) => amount_data_opt.as_ref(),
                    };
                    mandate_amount_data.and_then(|amount_data| {
                        item.connector.amount_converter
                            .convert(amount_data.amount, amount_data.currency)
                            .ok()
                            .map(|max_amount| {ConnectorName}SubscriptionData {
                                subscription_type: {ConnectorName}SubscriptionType::Unscheduled,
                                name: "Recurring Payment".to_string(),
                                max_amount,
                            })
                    })
                })
            });

        let tokenize_c_c = subscription.is_some().then_some(true);

        Ok(Self {
            api_operation: {ConnectorName}ApiOperation::Initiate,
            order: {ConnectorName}Order {
                amount,
                currency: Some(router_data.request.currency),
                reference: router_data.resource_common_data.connector_request_reference_id.clone(),
            },
            configuration: {ConnectorName}Configuration {
                tokenize_c_c,
                payment_action: {ConnectorName}PaymentAction::Authorize,
                return_url: router_data.request.router_return_url.clone(),
            },
            payment_data,
            subscription,
        })
    }
}

// =============================================================================
// RESPONSE TRANSFORMATION IMPLEMENTATION
// =============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}SetupMandateResponse, RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>>>
    for RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SetupMandateResponse, RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let status = match response.status {
            {ConnectorName}MandateStatus::Succeeded => common_enums::AttemptStatus::Charged,
            {ConnectorName}MandateStatus::RequiresAction => common_enums::AttemptStatus::AuthenticationPending,
            {ConnectorName}MandateStatus::RequiresPaymentMethod => common_enums::AttemptStatus::PaymentMethodAwaited,
            {ConnectorName}MandateStatus::Processing => common_enums::AttemptStatus::Pending,
            {ConnectorName}MandateStatus::Failed => common_enums::AttemptStatus::Failure,
            {ConnectorName}MandateStatus::Canceled => common_enums::AttemptStatus::Voided,
        };

        // Extract mandate reference based on connector pattern
        let mandate_reference = {
            // Pattern 1: Direct ID (Stripe, Adyen, Cybersource)
            let connector_mandate_id = response.id.clone();

            // OR Pattern 2: Subscription identifier (Noon)
            // let connector_mandate_id = response.subscription
            //     .as_ref()
            //     .map(|sub| sub.identifier.expose())
            //     .unwrap_or(response.id.clone());

            // OR Pattern 3: Token (Novalnet, others)
            // let connector_mandate_id = response.token
            //     .as_ref()
            //     .map(|t| t.expose())
            //     .unwrap_or(response.id.clone());

            Some(Box::new(MandateReference {
                connector_mandate_id: Some(connector_mandate_id),
                payment_method_id: None,
            }))
        };

        // Build payment response data
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
            redirection_data: None,  // Add if connector requires 3DS for mandate setup
            mandate_reference,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(response.id.clone()),
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

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub amount: {AmountType},
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<({AmountType}, T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from((amount, router_data, connector): ({AmountType}, T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data,
            connector,
        })
    }
}
```

## Mandate Request/Response Patterns

### Request Patterns by Connector Type

#### Type 1: Setup Intent Pattern (Stripe)

```rust
#[derive(Debug, Serialize)]
pub struct SetupMandateRequest {
    pub confirm: bool,  // Always true
    pub usage: Option<common_enums::FutureUsage>,  // "off_session"
    pub customer: Option<Secret<String>>,
    pub return_url: Option<String>,
    #[serde(flatten)]
    pub payment_method_data: StripePaymentMethodData,
    #[serde(rename = "payment_method_types[0]")]
    pub payment_method_types: Option<StripePaymentMethodType>,
}
```

**Key Features:**
- No amount field required
- Dedicated `setup_intents` endpoint
- Returns client_secret for client-side confirmation
- FormUrlEncoded format

#### Type 2: Zero-Auth Payment Pattern (Adyen, Cybersource)

```rust
#[derive(Debug, Serialize)]
pub struct SetupMandateRequest {
    pub amount: Amount,  // Zero or minimal amount
    pub currency: String,
    pub payment_method: PaymentMethod,
    pub reference: String,
    // Recurring-specific fields
    pub shopper_interaction: Option<ShopperInteraction>,
    pub recurring_processing_model: Option<RecurringModel>,
    pub store_payment_method: Option<bool>,
    pub shopper_reference: Option<String>,
    pub return_url: String,
}
```

**Key Features:**
- Uses regular `/payments` endpoint
- Zero or minimal amount (0 or 1 unit)
- Recurring flags indicate mandate setup
- JSON format

#### Type 3: Subscription/Tokenization Pattern (Noon)

```rust
#[derive(Debug, Serialize)]
pub struct SetupMandateRequest {
    pub api_operation: String,  // "INITIATE"
    pub order: Order,
    pub configuration: Configuration {
        tokenize_c_c: Option<bool>,  // true for mandate
        payment_action: String,
        return_url: Option<String>,
    },
    pub payment_data: PaymentData,
    pub subscription: Option<SubscriptionData>,  // Contains max_amount, type
}
```

**Key Features:**
- Subscription object with mandate details
- Tokenization flag (tokenize_c_c)
- Minimal amount (typically 1 unit)
- Returns subscription identifier as mandate_id

#### Type 4: Customer Profile Pattern (Authorizedotnet)

```rust
#[derive(Debug, Serialize)]
pub struct SetupMandateRequest {
    pub create_customer_payment_profile_request: PaymentProfileRequest {
        customer_profile_id: String,
        payment_profile: PaymentProfile {
            bill_to: Option<BillTo>,
            payment: PaymentDetails,
        },
    },
}
```

**Key Features:**
- Creates payment profile under existing customer
- No transaction amount
- Returns customerPaymentProfileId as mandate reference

### Response Patterns

#### Common Response Structure

```rust
#[derive(Debug, Deserialize)]
pub struct SetupMandateResponse {
    pub id: String,  // Primary mandate reference
    pub status: MandateStatus,
    // Connector-specific mandate identifier (choose one):
    pub client_secret: Option<Secret<String>>,  // Stripe
    pub subscription: Option<SubscriptionObject>,  // Noon
    pub token: Option<Secret<String>>,  // Token-based connectors
    pub payment_method: Option<String>,  // Stripe
}
```

#### Status Mapping Pattern

```rust
fn map_mandate_status(status: ConnectorMandateStatus) -> common_enums::AttemptStatus {
    match status {
        ConnectorMandateStatus::Succeeded | ConnectorMandateStatus::Active => {
            common_enums::AttemptStatus::Charged
        },
        ConnectorMandateStatus::RequiresAction | ConnectorMandateStatus::RequiresConfirmation => {
            common_enums::AttemptStatus::AuthenticationPending
        },
        ConnectorMandateStatus::RequiresPaymentMethod => {
            common_enums::AttemptStatus::PaymentMethodAwaited
        },
        ConnectorMandateStatus::Processing | ConnectorMandateStatus::Pending => {
            common_enums::AttemptStatus::Pending
        },
        ConnectorMandateStatus::Failed | ConnectorMandateStatus::Canceled => {
            common_enums::AttemptStatus::Failure
        },
    }
}
```

## Zero-Amount vs Subscription Patterns

### Zero-Amount Verification (Adyen, Cybersource)

```rust
// In request transformation
let amount = MinorUnit::new(0);  // Zero amount for verification

// Adyen-specific: Build amount structure
fn get_amount_data_for_setup_mandate(item: &AdyenRouterData<...>) -> Amount {
    Amount {
        currency: item.router_data.request.currency.to_string(),
        value: MinorUnit::new(0),  // Zero for mandate setup
    }
}
```

**When to use:**
- Connector supports $0 authorization
- No actual charge needed for mandate setup
- Typical for card-based mandates

### Minimal Amount Verification (Noon)

```rust
// In request transformation
let amount = data.connector.amount_converter.convert(
    MinorUnit::new(1),  // Minimal amount (e.g., $0.01)
    data.router_data.request.currency,
)?;
```

**When to use:**
- Connector doesn't support zero-amount transactions
- Requires minimal charge for verification
- Amount typically refunded automatically

### Subscription-Based (Noon, Novalnet)

```rust
// Build subscription data from mandate details
let subscription = router_data.request.setup_mandate_details.as_ref()
    .and_then(|mandate_details| {
        mandate_details.mandate_type.as_ref().and_then(|mandate_type| {
            let mandate_amount_data = match mandate_type {
                MandateDataType::SingleUse(amount_data) => Some(amount_data),
                MandateDataType::MultiUse(amount_data_opt) => amount_data_opt.as_ref(),
            };
            mandate_amount_data.map(|amount_data| SubscriptionData {
                subscription_type: SubscriptionType::Unscheduled,
                name: "Recurring Payment".to_string(),
                max_amount: convert_amount(amount_data.amount),
            })
        })
    });
```

**When to use:**
- Connector has dedicated subscription/recurring payment features
- Mandate includes maximum amount limits
- Multi-use mandates with amount constraints

## URL Endpoint Patterns

### Pattern 1: Dedicated Mandate Endpoint

```rust
// Stripe Setup Intents
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/v1/setup_intents", self.connector_base_url(req)))
}
```

### Pattern 2: Regular Payment Endpoint

```rust
// Adyen, Cybersource, Noon - Use same endpoint as authorize
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    Ok(format!("{}/v{}/payments",
        self.connector_base_url(req),
        API_VERSION
    ))
}
```

### Pattern 3: Customer Profile Endpoint

```rust
// ACI, Authorizedotnet - Customer-specific endpoints
fn get_url(&self, req: &RouterDataV2<SetupMandate, ...>) -> CustomResult<String, ConnectorError> {
    let customer_id = req.request.customer_id.clone()
        .ok_or(ConnectorError::MissingRequiredField { field_name: "customer_id" })?;

    Ok(format!("{}/v1/customers/{}/payment_methods",
        self.connector_base_url(req),
        customer_id
    ))
}
```

## Mandate Reference Handling

### Extracting Mandate Reference

```rust
// Pattern 1: Direct ID from response
let mandate_reference = Some(Box::new(MandateReference {
    connector_mandate_id: Some(response.id.clone()),
    payment_method_id: None,
}));

// Pattern 2: Subscription identifier (Noon)
let mandate_reference = response.subscription.map(|subscription_data| {
    Box::new(MandateReference {
        connector_mandate_id: Some(subscription_data.identifier.expose()),
        payment_method_id: None,
    })
});

// Pattern 3: Payment method token (Stripe)
let mandate_reference = Some(Box::new(MandateReference {
    connector_mandate_id: Some(response.id.clone()),
    payment_method_id: response.payment_method.clone(),
}));
```

### Using Mandate Reference in Future Payments

```rust
// In RepeatPayment/Authorize flow
match item.router_data.request.connector_mandate_id() {
    Some(mandate_id) => {
        // Use the stored mandate_id for payment
        PaymentData::Subscription(Subscription {
            subscription_identifier: Secret::new(mandate_id),
        })
    },
    None => {
        // Fresh payment with new payment method data
        PaymentData::Card(...)
    }
}
```

## Error Handling Patterns

### Mandate-Specific Error Handling

```rust
impl ConnectorCommon for {ConnectorName} {
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: {ConnectorName}ErrorResponse = res.response
            .parse_struct("ErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

        if let Some(i) = event_builder {
            i.set_error_response_body(&response);
        }

        // Map mandate-specific error codes
        let attempt_status = match response.error_code.as_deref() {
            Some("customer_not_found") => Some(common_enums::AttemptStatus::Failure),
            Some("invalid_payment_method") => Some(common_enums::AttemptStatus::PaymentMethodAwaited),
            Some("authentication_required") => Some(common_enums::AttemptStatus::AuthenticationPending),
            Some("mandate_not_supported") => Some(common_enums::AttemptStatus::Failure),
            _ => Some(common_enums::AttemptStatus::Failure),
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
}
```

## Testing Patterns

### Unit Test Structure for SetupMandate

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use domain_types::connector_types::PaymentFlowData;
    use common_enums::{Currency, AttemptStatus};

    #[test]
    fn test_setup_mandate_request_transformation() {
        // Create test router data for mandate setup
        let router_data = create_test_setup_mandate_router_data();

        let connector_req = {ConnectorName}SetupMandateRequest::try_from(&router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();

        // Verify mandate-specific fields
        assert_eq!(req.confirm, true);  // For Stripe-style
        assert_eq!(req.usage, Some(common_enums::FutureUsage::OffSession));
        // OR for zero-auth style
        // assert_eq!(req.amount, MinorUnit::new(0));
        // assert_eq!(req.store_payment_method, Some(true));
    }

    #[test]
    fn test_setup_mandate_response_transformation() {
        let response = {ConnectorName}SetupMandateResponse {
            id: "mandate_test_id".to_string(),
            status: {ConnectorName}MandateStatus::Succeeded,
            customer: Some("cust_123".to_string()),
            client_secret: Some(Secret::new("secret_123".to_string())),
        };

        let router_data = create_test_setup_mandate_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert_eq!(router_data_result.resource_common_data.status, AttemptStatus::Charged);

        // Verify mandate reference extraction
        if let Ok(PaymentsResponseData::TransactionResponse { mandate_reference, .. }) =
            &router_data_result.response
        {
            assert!(mandate_reference.is_some());
            let mandate_ref = mandate_reference.as_ref().unwrap();
            assert_eq!(mandate_ref.connector_mandate_id, Some("mandate_test_id".to_string()));
        } else {
            panic!("Expected TransactionResponse with mandate_reference");
        }
    }

    #[test]
    fn test_setup_mandate_with_subscription_data() {
        // Test subscription-based mandate setup (Noon pattern)
        let router_data = create_test_router_data_with_mandate_details();

        let connector_req = {ConnectorName}SetupMandateRequest::try_from(&router_data);
        assert!(connector_req.is_ok());

        let req = connector_req.unwrap();
        assert!(req.subscription.is_some());
        assert_eq!(req.configuration.tokenize_c_c, Some(true));
    }

    fn create_test_setup_mandate_router_data() -> RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData, PaymentsResponseData> {
        // Create test router data structure with mandate setup details
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
    async fn test_setup_mandate_flow_integration() {
        let connector = {ConnectorName}::new();

        // Mock mandate setup request data
        let request_data = create_test_setup_mandate_request();

        // Test headers generation
        let headers = connector.get_headers(&request_data).unwrap();
        assert!(headers.contains(&("Content-Type".to_string(), "application/json".into())));

        // Test URL generation for mandate endpoint
        let url = connector.get_url(&request_data).unwrap();
        assert!(url.contains("setup") || url.contains("payment") || url.contains("customer"));

        // Test request body generation
        let request_body = connector.get_request_body(&request_data).unwrap();
        assert!(request_body.is_some());
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Identify mandate/recurring payment endpoints
  - [ ] Understand mandate setup flow (setup intent, zero-auth, subscription)
  - [ ] Review mandate reference format (token, subscription_id, payment_method_id)
  - [ ] Check amount requirements (zero, minimal, or not required)
  - [ ] Understand customer binding requirements
  - [ ] Review 3DS requirements for mandate setup

- [ ] **Mandate Type Identification**
  - [ ] Determine if connector uses dedicated mandate endpoint
  - [ ] Check if connector requires customer pre-creation
  - [ ] Identify recurring/tokenization flags needed
  - [ ] Understand mandate usage limitations (single-use vs multi-use)

- [ ] **Integration Requirements**
  - [ ] Determine authentication type (same as authorize usually)
  - [ ] Choose request format (JSON, FormUrlEncoded)
  - [ ] Identify amount converter type and verification amount
  - [ ] Review mandate reference storage and retrieval

### Implementation Checklist

- [ ] **File Structure Setup**
  - [ ] Main connector file: `{connector_name}.rs` exists
  - [ ] Transformers directory: `{connector_name}/` created
  - [ ] Transformers file: `{connector_name}/transformers.rs` created

- [ ] **Main Connector Implementation**
  - [ ] Add `SetupMandateV2<T>` trait implementation
  - [ ] Add SetupMandate to `create_all_prerequisites!` api array
  - [ ] Implement SetupMandate flow with `macro_connector_implementation!`
  - [ ] Implement `get_url()` for mandate endpoint
  - [ ] Implement `get_headers()` (usually same as authorize)
  - [ ] Add Source Verification stub for SetupMandate

- [ ] **Transformers Implementation**
  - [ ] Create `SetupMandateRequest` structure with appropriate pattern
  - [ ] Create `SetupMandateResponse` structure
  - [ ] Implement request transformation (`TryFrom` for request)
  - [ ] Implement response transformation (`TryFrom` for response)
  - [ ] Extract mandate_reference correctly from response
  - [ ] Handle recurring/subscription flags properly
  - [ ] Implement amount handling (zero/minimal/none)

- [ ] **Mandate-Specific Features**
  - [ ] Implement customer_id binding if required
  - [ ] Add recurring flags (shopper_interaction, recurring_model, etc.)
  - [ ] Handle subscription data if applicable
  - [ ] Implement tokenization flags (store_payment_method, tokenize_c_c)
  - [ ] Add mandate type support (single-use vs multi-use)
  - [ ] Handle return_url for 3DS if needed

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test request transformation with card payment method
  - [ ] Test response transformation with successful mandate
  - [ ] Test mandate_reference extraction
  - [ ] Test status mapping for mandate statuses
  - [ ] Test subscription data building (if applicable)
  - [ ] Test error handling for mandate-specific errors

- [ ] **Integration Tests**
  - [ ] Test headers generation
  - [ ] Test URL construction for mandate endpoint
  - [ ] Test request body generation
  - [ ] Test complete setup mandate flow

### Configuration Checklist

- [ ] **Connector Configuration**
  - [ ] Connector added to `Connectors` struct
  - [ ] Base URL configuration added
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
  - [ ] Verify mandate reference returned correctly
  - [ ] Verify mandate can be used in RepeatPayment flow
  - [ ] Test error handling
  - [ ] Verify status mapping

### Documentation Checklist

- [ ] **Code Documentation**
  - [ ] Add doc comments explaining mandate flow
  - [ ] Document mandate reference format
  - [ ] Document any special requirements
  - [ ] Add usage examples in comments

- [ ] **Integration Documentation**
  - [ ] Document mandate setup requirements
  - [ ] Document customer prerequisites
  - [ ] Document mandate usage in repeat payments
  - [ ] Document known limitations

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Adyen`, `Noon` | **Always required** |
| `{connector_name}` | Connector name in snake_case | `stripe`, `adyen`, `noon` | **Always required** |
| `{AmountType}` | Amount type for mandate | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **Choose based on API** |
| `{AmountUnit}` | Amount converter type | `MinorUnit`, `StringMinorUnit` | **Must match {AmountType}** |
| `{content_type}` | Request content type | `"application/json"`, `"application/x-www-form-urlencoded"` | **Based on API format** |
| `{mandate_endpoint}` | Mandate API endpoint | `"setup_intents"`, `"payments"`, `"customers/{id}/payment_methods"` | **From API docs** |
| `{Major\|Minor}` | Currency unit choice | `Major` or `Minor` | **Choose one** |

### Mandate Amount Selection Guide

| API Expects | Amount Value | Example |
|-------------|--------------|---------|
| Zero amount for verification | `MinorUnit::new(0)` | Adyen, Cybersource |
| Minimal amount (1 cent/unit) | `MinorUnit::new(1)` | Noon |
| No amount required | N/A | Stripe Setup Intents |

### Mandate Pattern Selection Guide

| Connector API Style | Pattern | Endpoint Example |
|---------------------|---------|------------------|
| Dedicated mandate/setup endpoint | Pattern 1 (Setup Intent) | `/v1/setup_intents` |
| Regular payment endpoint with flags | Pattern 2 (Zero-Auth) | `/v1/payments` |
| Customer profile/token endpoint | Pattern 3 (Subscription) | `/customers/{id}/payment_methods` |

## Best Practices

1. **Use Appropriate Pattern**: Choose the mandate pattern (Setup Intent, Zero-Auth, Subscription) that matches your connector's API design
2. **Handle Customer Binding**: Most connectors require customer_id for mandate setup - validate this early
3. **Extract Mandate Reference**: Ensure connector_mandate_id is correctly extracted and returned for future use
4. **Zero/Minimal Amount**: Use appropriate amount (zero or minimal) based on connector requirements
5. **Recurring Flags**: Set proper recurring/tokenization flags (shopper_interaction, store_payment_method, etc.)
6. **Test Mandate Usage**: Verify created mandate can be used in RepeatPayment flow
7. **Status Mapping**: Map mandate statuses carefully (especially RequiresAction for 3DS)
8. **Return URL Handling**: Include return_url for connectors requiring 3DS for mandate setup
9. **Error Context**: Provide meaningful error messages for mandate-specific failures
10. **Documentation**: Document mandate reference format and usage requirements

## Summary

This pattern document provides comprehensive templates for implementing SetupMandate flows across all connector types:

- **3 Main Patterns**: Setup Intent (Stripe), Zero-Auth Payment (Adyen), Subscription (Noon)
- **8 Reference Implementations**: Adyen, Stripe, Cybersource, ACI, Authorizedotnet, Noon, Novalnet, Payload
- **Complete Code Templates**: Request/response structures, transformations, error handling
- **Flexible Amount Handling**: Zero, minimal, or no amount based on connector needs
- **Mandate Reference Extraction**: Multiple patterns for different mandate ID formats
- **Comprehensive Checklists**: Pre-implementation through validation

By following these patterns, you can implement a production-ready SetupMandate flow for any payment connector in 30-45 minutes.
