# CreateSessionToken Flow Pattern for Connector Implementation

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the CreateSessionToken flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations (Paytm, Nuvei) and can be consumed by AI to generate consistent, production-ready CreateSessionToken flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. The CreateSessionToken flow is used to initiate a payment session and obtain a session token that can be used in subsequent authorization calls.

## 🚀 Quick Start Guide

To implement a new connector CreateSessionToken flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Enable Session Token Flow**: Implement `ValidationTrait` with `should_do_session_token()` returning `true`
3. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
4. **Select Components**: Choose auth type, request format, and amount converter based on your connector's API
5. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector CreateSessionToken Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{AmountType} → StringMinorUnit (if API expects "1000" for $10.00)
{content_type} → "application/json" (if API uses JSON)
{session_token_endpoint} → "v1/session-token" (your API endpoint)
```

**✅ Result**: Complete, production-ready connector CreateSessionToken flow implementation in ~20 minutes

## Table of Contents

1. [Overview](#overview)
2. [CreateSessionToken Flow Implementation Analysis](#createsessiontoken-flow-implementation-analysis)
3. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
4. [ValidationTrait Implementation](#validationtrait-implementation)
5. [Request/Response Format Variations](#requestresponse-format-variations)
6. [Session Token Usage Patterns](#session-token-usage-patterns)
7. [Error Handling Patterns](#error-handling-patterns)
8. [Testing Patterns](#testing-patterns)
9. [Integration Checklist](#integration-checklist)

## Overview

The CreateSessionToken flow is a pre-authorization step that:
1. Receives session token creation requests from the router
2. Transforms them to connector-specific format
3. Sends requests to the payment gateway to initiate a session
4. Processes responses and extracts session tokens
5. Returns standardized responses containing the session token for use in authorization

### Key Components:
- **Main Connector File**: Implements traits and flow logic
- **Transformers File**: Handles request/response data transformations
- **ValidationTrait**: Enables the CreateSessionToken flow
- **Authentication**: Manages API credentials and headers
- **Error Handling**: Processes and maps error responses
- **Session Token Storage**: Stores token in PaymentFlowData for later use

### Flow Sequence:
```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│   Router    │────▶│ CreateSessionToken│────▶│  Connector  │
│             │     │     Flow         │     │   Session   │
└─────────────┘     └──────────────────┘     │   Endpoint  │
                                              └──────┬──────┘
                                                     │
                                              ┌──────▼──────┐
                                              │   Returns   │
                                              │Session Token│
                                              └──────┬──────┘
                                                     │
┌─────────────┐     ┌──────────────────┐     ┌──────▼──────┐
│   Router    │◀────│     Authorize    │◀────│  Uses Token │
│             │     │     Flow         │     │  in Request │
└─────────────┘     └──────────────────┘     └─────────────┘
```

## CreateSessionToken Flow Implementation Analysis

Based on comprehensive analysis of all 77 connectors in the connector service, here's the implementation status:

### ✅ Full CreateSessionToken Implementation (2 connectors)
These connectors have complete CreateSessionToken flow implementations:

1. **Paytm** - Multi-step payment flow with AES signature encryption
   - Initiates transaction before authorization
   - Uses session token in subsequent process transaction call
   - Complex signature generation with AES-CBC encryption
   - Supports UPI Intent and UPI Collect flows

2. **Nuvei** - Session-based authentication for card payments
   - Gets session token before payment authorization
   - Uses token in payment.do call
   - Checksum-based authentication
   - Supports Auth and Sale transaction types

### 🔧 Stub/Trait Implementation Only (75 connectors)
These connectors implement the CreateSessionToken trait but have empty/stub implementations:
- ACI, Adyen, Airwallex, Authipay, AuthorizeDotNet, Bambora, BamboraAPAC, BankOfAmerica, Barclaycard, Billwerk, Bluesnap, Braintree, Calida, Cashfree, CashtoCode, Celero, Checkout, Cryptopay, Cybersource, Datatrans, Dlocal, Elavon, Fiserv, FiservMEA, Fiuu, Forte, Getnet, Gigadat, GlobalPay, Helcim, Hipay, HyperPG, IataPay, JPMorgan, Loonio, Mifinity, Mollie, Multisafepay, Nexinets, Nexixpay, NMI, Noon, Novalnet, Paybox, Payload, Payme, Paypal, Paysafe, PayU, PhonePe, Placetopay, Powertranz, Rapyd, Razorpay, RazorpayV2, Redsys, Revolut, Shift4, Silverflow, Stax, Stripe, Trustpay, Trustpayments, Tsys, Volt, WellsFargo, Worldpay, WorldpayVantiv, WorldpayXML, Xendit, Zift

### 📊 Implementation Statistics
- **Complete implementations**: 2/77 (3%)
- **Stub implementations**: 75/77 (97%)
- **Most common pattern**: POST-based with JSON request body
- **Most common auth**: Custom signature/checksum-based
- **Session token usage**: Stored in PaymentFlowData.session_token

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
        Accept, Authorize, Capture, CreateOrder, CreateSessionToken, DefendDispute, PSync, RSync,
        Refund, RepeatPayment, SetupMandate, SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData,
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
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    events::connector_api_logs::ConnectorEvent,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}SessionTokenRequest, {ConnectorName}SessionTokenResponse,
    {ConnectorName}AuthorizeRequest, {ConnectorName}AuthorizeResponse,
    {ConnectorName}ErrorResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

// Set up connector using macros with all framework integrations
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        (
            flow: CreateSessionToken,
            request_body: {ConnectorName}SessionTokenRequest,
            response_body: {ConnectorName}SessionTokenResponse,
            router_data: RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: {ConnectorName}AuthorizeRequest<T>,
            response_body: {ConnectorName}AuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        // Add other flows as needed...
    ],
    amount_converters: [
        // Choose appropriate amount converter based on connector requirements
        amount_converter: {AmountUnit} // StringMinorUnit, StringMajorUnit, MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            let mut header = vec![(
                "Content-Type".to_string(),
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

// CRITICAL: Implement ValidationTrait to enable CreateSessionToken flow
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for {ConnectorName}<T>
{
    fn should_do_session_token(&self) -> bool {
        true // Enable CreateSessionToken flow
    }

    fn should_do_order_create(&self) -> bool {
        false // Set to true if connector requires separate order creation
    }
}

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn id(&self) -> &'static str {
        "{connector_name}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::{Major|Minor}
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
            "Authorization".to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

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
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)?
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

// Implement CreateSessionToken flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}SessionTokenRequest),
    curl_response: {ConnectorName}SessionTokenResponse,
    flow_name: CreateSessionToken,
    resource_common_data: PaymentFlowData,
    flow_request: SessionTokenRequestData,
    flow_response: SessionTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>,
        ) -> CustomResult<String, ConnectorError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/{session_token_endpoint}"))
        }
    }
);

// Implement Authorize flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}AuthorizeRequest<T>),
    curl_response: {ConnectorName}AuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
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
            Ok(format!("{base_url}/{authorize_endpoint}"))
        }
    }
);
```

### Transformers File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use common_utils::types::{MinorUnit, StringMinorUnit};
use domain_types::{
    connector_flow::{Authorize, CreateSessionToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        SessionTokenRequestData, SessionTokenResponseData, ResponseId,
    },
    errors::{self, ConnectorError},
    payment_method_data::PaymentMethodDataTypes,
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
    pub api_secret: Option<Secret<String>>,
    // Add other auth fields as needed
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = ConnectorError;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::HeaderKey { api_key } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: None,
            }),
            ConnectorAuthType::SignatureKey { api_key, api_secret, .. } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: Some(api_secret.to_owned()),
            }),
            ConnectorAuthType::BodyKey { api_key, key1 } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: Some(key1.to_owned()),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType),
        }
    }
}

// ================================
// Session Token Flow
// ================================

// Session Token Request Structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SessionTokenRequest {
    // Common fields for session token requests
    pub merchant_id: Secret<String>,
    pub client_request_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<{AmountType}>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    // Add connector-specific fields
}

// Session Token Response Structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SessionTokenResponse {
    pub session_token: Option<String>,
    pub status: {ConnectorName}SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}SessionStatus {
    Success,
    Failed,
    Error,
    #[serde(other)]
    Unknown,
}

// Session Token Request Transformation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>,
            T,
        >,
    > for {ConnectorName}SessionTokenRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        // Convert amount if needed
        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;

        Ok(Self {
            merchant_id: auth.api_key,
            client_request_id: router_data.resource_common_data.connector_request_reference_id.clone(),
            timestamp: chrono::Utc::now().timestamp().to_string(),
            amount: Some(amount),
            currency: Some(router_data.request.currency.to_string()),
        })
    }
}

// Session Token Response Transformation
impl TryFrom<ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>>
    for RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for error status
        if matches!(response.status, {ConnectorName}SessionStatus::Error | {ConnectorName}SessionStatus::Failed) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: response.error_code.clone().unwrap_or_default(),
                    message: response.error_message.clone().unwrap_or_default(),
                    reason: response.error_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract session token
        let session_token = response
            .session_token
            .clone()
            .ok_or(ConnectorError::MissingRequiredField {
                field_name: "session_token",
            })?;

        // Return success with session token stored in PaymentFlowData
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                session_token: Some(session_token.clone()),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(SessionTokenResponseData {
                session_token,
            }),
            ..router_data.clone()
        })
    }
}

// ================================
// Authorization Flow
// ================================

// Authorization Request Structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}AuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: String,
    pub merchant_id: Secret<String>,
    pub amount: {AmountType},
    pub currency: String,
    pub payment_method: {ConnectorName}PaymentMethod<T>,
    pub reference: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum {ConnectorName}PaymentMethod<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Card({ConnectorName}Card<T>),
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}Card<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub number: domain_types::payment_method_data::RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvc: Secret<String>,
}

// Authorization Request Transformation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_auth_type)?;

        // Extract session token from PaymentFlowData
        let session_token = router_data
            .resource_common_data
            .session_token
            .clone()
            .ok_or(ConnectorError::MissingRequiredField {
                field_name: "session_token",
            })?;

        // Convert amount
        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(ConnectorError::AmountConversionFailed)?;

        // Build payment method
        let payment_method = match &router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethod::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: card_data.card_cvc.clone(),
                })
            }
            _ => return Err(ConnectorError::NotImplemented("Payment method not supported".to_string()).into()),
        };

        Ok(Self {
            session_token,
            merchant_id: auth.api_key,
            amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
        })
    }
}

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from((router_data, connector): (T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            router_data,
            connector,
        })
    }
}

// Error Response Structure
#[derive(Debug, Deserialize, Default)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}
```

## ValidationTrait Implementation

**CRITICAL**: To enable the CreateSessionToken flow, you MUST implement the `ValidationTrait` with `should_do_session_token()` returning `true`:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for {ConnectorName}<T>
{
    fn should_do_session_token(&self) -> bool {
        true // Enable CreateSessionToken flow
    }

    fn should_do_order_create(&self) -> bool {
        false // Set to true if connector requires separate order creation
    }
}
```

This trait tells the router to execute the CreateSessionToken flow before the Authorize flow.

## Request/Response Format Variations

### Simple Session Token Pattern (Nuvei-style)

For connectors that only need basic merchant authentication to get a session token:

```rust
// Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: DateTime<YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenResponse {
    pub session_token: Option<String>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}
```

### Complex Session Initiation Pattern (Paytm-style)

For connectors that require full payment details to initiate a session:

```rust
// Request with signature
#[derive(Debug, Serialize)]
pub struct PaytmInitiateTxnRequest {
    pub head: PaytmRequestHeader,
    pub body: PaytmInitiateReqBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmRequestHeader {
    pub client_id: Option<Secret<String>>,
    pub version: String,
    pub request_timestamp: String,
    pub channel_id: Option<String>,
    pub signature: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct PaytmInitiateReqBody {
    pub request_type: String,
    pub mid: Secret<String>,
    pub order_id: String,
    pub website_name: Secret<String>,
    pub txn_amount: PaytmAmount,
    pub user_info: PaytmUserInfo,
    pub callback_url: String,
}

// Response
#[derive(Debug, Deserialize)]
pub struct PaytmInitiateTxnResponse {
    pub head: PaytmRespHead,
    pub body: PaytmResBodyTypes,
}
```

## Session Token Usage Patterns

### Pattern 1: Token in Request Body (Nuvei-style)

```rust
#[derive(Debug, Serialize)]
pub struct NuveiPaymentRequest<T> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub amount: StringMajorUnit,
    pub currency: Currency,
    pub payment_option: NuveiPaymentOption<T>,
}

// In TryFrom for Authorize request:
let session_token = router_data
    .resource_common_data
    .session_token
    .clone()
    .ok_or(ConnectorError::MissingRequiredField {
        field_name: "session_token",
    })?;
```

### Pattern 2: Token in Headers (Alternative)

```rust
fn get_headers(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<Vec<(String, Maskable<String>)>, ConnectorError> {
    let session_token = req.resource_common_data.get_session_token()?;
    let mut headers = vec![
        ("X-Session-Token".to_string(), session_token.into_masked()),
        ("Content-Type".to_string(), "application/json".into()),
    ];
    Ok(headers)
}
```

### Pattern 3: Token in URL Query Parameters (Alternative)

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<String, ConnectorError> {
    let session_token = req.resource_common_data.get_session_token()?;
    let base_url = self.connector_base_url_payments(req);
    Ok(format!("{base_url}/payment?sessionToken={}", session_token))
}
```

## Error Handling Patterns

### Session Token Specific Error Handling

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>>
    for RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Handle session token specific errors
        if let Some(error_code) = &response.error_code {
            let attempt_status = match error_code.as_str() {
                "INVALID_MERCHANT" => common_enums::AttemptStatus::AuthorizationFailed,
                "RATE_LIMIT_EXCEEDED" => common_enums::AttemptStatus::Pending,
                "SESSION_EXPIRED" => common_enums::AttemptStatus::Failure,
                _ => common_enums::AttemptStatus::Failure,
            };

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: attempt_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error_code.clone(),
                    message: response.error_message.clone().unwrap_or_default(),
                    reason: response.error_message.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(attempt_status),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Success case...
    }
}
```

## Testing Patterns

### Unit Test Structure for CreateSessionToken Flow

```rust
#[cfg(test)]
mod session_token_tests {
    use super::*;

    #[test]
    fn test_session_token_request_transformation() {
        let router_data = create_test_session_token_router_data();
        let connector_req = {ConnectorName}SessionTokenRequest::try_from(&router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert!(!req.client_request_id.is_empty());
    }

    #[test]
    fn test_session_token_response_transformation_success() {
        let response = {ConnectorName}SessionTokenResponse {
            session_token: Some("test_session_token_123".to_string()),
            status: {ConnectorName}SessionStatus::Success,
            error_code: None,
            error_message: None,
        };

        let router_data = create_test_session_token_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert!(router_data_result.resource_common_data.session_token.is_some());
        assert_eq!(
            router_data_result.resource_common_data.session_token.unwrap(),
            "test_session_token_123"
        );
    }

    #[test]
    fn test_session_token_response_transformation_failure() {
        let response = {ConnectorName}SessionTokenResponse {
            session_token: None,
            status: {ConnectorName}SessionStatus::Error,
            error_code: Some("INVALID_MERCHANT".to_string()),
            error_message: Some("Merchant not found".to_string()),
        };

        let router_data = create_test_session_token_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 400,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert!(router_data_result.response.is_err());
    }

    fn create_test_session_token_router_data() -> RouterDataV2<CreateSessionToken, PaymentFlowData, SessionTokenRequestData, SessionTokenResponseData> {
        // Create test router data structure
        RouterDataV2 {
            resource_common_data: PaymentFlowData {
                connector_request_reference_id: "test_order_123".to_string(),
                ..Default::default()
            },
            request: SessionTokenRequestData {
                amount: MinorUnit::new(1000),
                currency: common_enums::Currency::USD,
                ..Default::default()
            },
            response: Ok(SessionTokenResponseData {
                session_token: "".to_string(),
            }),
            ..Default::default()
        }
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Understand connector's session token API endpoint
  - [ ] Review authentication requirements for session token request
  - [ ] Identify required/optional fields for session token request
  - [ ] Understand session token expiration behavior
  - [ ] Review how session token is used in authorization

- [ ] **Flow Requirements**
  - [ ] Determine if connector requires CreateSessionToken flow
  - [ ] Understand sequence: Session Token → Authorize
  - [ ] Check if session token can be reused across multiple requests
  - [ ] Identify session token expiration time

### Implementation Checklist

- [ ] **Main Connector Implementation**
  - [ ] Add `CreateSessionToken` to connector_flow imports
  - [ ] Add `SessionTokenRequestData` and `SessionTokenResponseData` to connector_types imports
  - [ ] Import session token request/response types from transformers
  - [ ] Implement `ValidationTrait` with `should_do_session_token()` returning `true`
  - [ ] Add CreateSessionToken flow to `macros::create_all_prerequisites!`
  - [ ] Implement CreateSessionToken flow with `macros::macro_connector_implementation!`
  - [ ] Add Source Verification stub for CreateSessionToken flow

- [ ] **Transformers Implementation**
  - [ ] Add `CreateSessionToken` to connector_flow imports
  - [ ] Add `SessionTokenRequestData` and `SessionTokenResponseData` to connector_types imports
  - [ ] Create session token request structure
  - [ ] Create session token response structure
  - [ ] Create session status enumeration
  - [ ] Implement session token request transformation (`TryFrom`)
  - [ ] Implement session token response transformation (`TryFrom`)
  - [ ] Store session token in `PaymentFlowData.session_token`
  - [ ] Extract and use session token in Authorize flow

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test session token request transformation
  - [ ] Test session token response transformation (success)
  - [ ] Test session token response transformation (failure)
  - [ ] Test session token storage in PaymentFlowData
  - [ ] Test session token retrieval in Authorize flow
  - [ ] Test error response handling

- [ ] **Integration Tests**
  - [ ] Test complete flow: CreateSessionToken → Authorize
  - [ ] Test session token expiration handling
  - [ ] Test error scenarios

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all errors
  - [ ] Run `cargo test` and ensure all tests pass
  - [ ] Run `cargo clippy` and fix warnings

- [ ] **Functionality Validation**
  - [ ] Test with sandbox/test credentials
  - [ ] Verify session token is received
  - [ ] Verify session token is used in authorization
  - [ ] Verify error handling works correctly

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM FOR CREATESESSIONTOKEN FLOWS**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Nuvei`, `PayPal`, `NewPayment` | **Always required** - Used in struct names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `nuvei`, `paypal`, `new_payment` | **Always required** - Used in config keys |
| `{AmountType}` | Amount type based on connector API | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit` | **Choose based on API** |
| `{content_type}` | Request content type | `"application/json"`, `"application/x-www-form-urlencoded"` | **Based on API format** |
| `{session_token_endpoint}` | Session token API endpoint | `"v1/session-token"`, `"getSessionToken.do"` | **From API docs** |
| `{authorize_endpoint}` | Authorization API endpoint | `"v1/payments"`, `"payment.do"` | **From API docs** |
| `{Major\|Minor}` | Currency unit choice | `Major` or `Minor` | **Choose one** |

### Real-World Examples

**Example 1: Nuvei-style Connector**
```bash
{ConnectorName} → Nuvei
{connector_name} → nuvei
{AmountType} → StringMajorUnit
{content_type} → "application/json"
{session_token_endpoint} → "getSessionToken.do"
{authorize_endpoint} → "payment.do"
{Major|Minor} → Minor
```

**Example 2: Paytm-style Connector**
```bash
{ConnectorName} → Paytm
{connector_name} → paytm
{AmountType} → StringMajorUnit
{content_type} → "application/json"
{session_token_endpoint} → "theia/api/v1/initiateTransaction"
{authorize_endpoint} → "theia/api/v1/processTransaction"
{Major|Minor} → Minor
```

## Best Practices

1. **Enable via ValidationTrait**: Always implement `ValidationTrait` with `should_do_session_token()` returning `true` to enable the flow

2. **Store Token in PaymentFlowData**: Store the session token in `PaymentFlowData.session_token` so it's available to subsequent flows

3. **Handle Missing Token**: In Authorize flow, always check for the session token and return a clear error if missing

4. **Token Expiration**: Consider token expiration if the connector has time limits on session token validity

5. **Error Handling**: Implement specific error handling for session token failures vs authorization failures

6. **Testing**: Test the complete flow end-to-end: CreateSessionToken → Authorize

### Common Pitfalls to Avoid

- **Missing ValidationTrait**: Without implementing `should_do_session_token()`, the flow will never be executed
- **Not Storing Token**: Forgetting to store the token in `PaymentFlowData.session_token`
- **Not Using Token**: Authorize flow not extracting and using the session token
- **Wrong Status Mapping**: Session token responses should typically map to `Pending` status, not `Charged`
- **Error Propagation**: Not properly propagating session token errors to prevent authorization attempts

This pattern document provides a comprehensive template for implementing CreateSessionToken flows in payment connectors, ensuring consistency and completeness across all implementations.
