# UPI Authorize Flow Pattern for Connector Implementation

**🎯 UPI-SPECIFIC PATTERN FILE FOR INDIA PAYMENT CONNECTORS**

This document provides comprehensive, reusable patterns for implementing the UPI (Unified Payments Interface) authorize flow in payment connectors targeting the Indian market. These patterns are extracted from production implementations (Razorpay, PhonePe, Cashfree, Paytm) and enable AI to generate consistent, production-ready UPI payment code.

## Table of Contents

1. [Overview](#overview)
2. [UPI Payment Method Variants](#upi-payment-method-variants)
3. [Supported Connectors](#supported-connectors)
4. [Pattern Categories](#pattern-categories)
5. [Standard JSON Pattern](#standard-json-pattern)
6. [Two-Phase Flow Pattern](#two-phase-flow-pattern)
7. [Encrypted Payload Pattern](#encrypted-payload-pattern)
8. [Session Token Pattern](#session-token-pattern)
9. [Sub-type Variations](#sub-type-variations)
10. [Common Pitfalls](#common-pitfalls)
11. [Testing Patterns](#testing-patterns)
12. [Implementation Checklist](#implementation-checklist)

## Overview

UPI (Unified Payments Interface) is India's real-time payment system that enables instant money transfer between bank accounts using a mobile device. In the Grace-UCS system, UPI payments are supported through multiple flows:

### UPI Flow Types

| Flow | Description | Use Case |
|------|-------------|----------|
| **UPI Intent** | Redirects customer to their UPI app with pre-filled payment details | Mobile apps, web apps with deep link support |
| **UPI Collect** | Sends a payment request to customer's UPI ID (VPA) | Customer enters VPA, approves on their app |
| **UPI QR** | Generates a unique QR code for the transaction | In-store payments, desktop web |

### Key Characteristics

- **Currency**: INR only (Indian Rupees)
- **Amount Unit**: Minor units (paise) - 100 paise = 1 INR
- **Response Type**: Async (webhook-based confirmation)
- **Authentication**: Varies by connector (API keys, signatures, encryption)

## UPI Payment Method Variants

### Rust Type Definitions

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpiData {
    /// UPI Collect - Customer approves a collect request sent to their UPI app
    UpiCollect(UpiCollectData),
    /// UPI Intent - Customer is redirected to their UPI app with a pre-filled payment request
    UpiIntent(UpiIntentData),
    /// UPI QR - Unique QR generated per txn
    UpiQr(UpiQrData),
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UpiCollectData {
    pub vpa_id: Option<Secret<String, UpiVpaMaskingStrategy>>,
    pub upi_source: Option<UpiSource>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UpiIntentData {
    pub upi_source: Option<UpiSource>,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct UpiQrData {
    pub upi_source: Option<UpiSource>,
}
```

### UPI Source Types

```rust
#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UpiSource {
    UpiCc,      // UPI Credit Card (RuPay credit on UPI)
    UpiCl,      // UPI Credit Line
    UpiAccount, // UPI Bank Account (Savings)
    UpiCcCl,    // UPI Credit Card + Credit Line
    UpiPpi,     // UPI Prepaid Payment Instrument
    UpiVoucher, // UPI Voucher
}
```

## Supported Connectors

| Connector | UPI Intent | UPI Collect | UPI QR | Pattern Type | Auth Type |
|-----------|------------|-------------|--------|--------------|-----------|
| **RazorpayV2** | Yes | Yes | Yes | Standard JSON | Basic Auth |
| **PhonePe** | Yes | Yes | Yes | Encrypted Payload | HMAC Signature |
| **Cashfree** | Yes | Yes | No | Two-Phase Flow | BodyKey |
| **Paytm** | Yes | Yes | No | Session Token | AES Encryption |
| **Stripe** | Yes | No | No | Standard JSON | Bearer Token |
| **Adyen** | Yes | No | No | Standard JSON | API Key |
| **PineLabs Online** | Yes | Yes | Yes | Two-Phase Flow | API Key |
| **PayU** | Yes (generic) | Yes | Yes (generic) | Standard JSON (S2S Flow) | SHA-512 Signature |

**PineLabs Online** UPI authorization (PR #795): `PaymentMethodData::Upi(_)` is mapped to the connector wire string `"UPI"` in `get_pinelabs_payment_method_string` at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:621`. The UPI branch of `build_payment_option` at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:638` discriminates `UpiData::UpiCollect` (txn_mode `"COLLECT"`, VPA extracted from `collect_data.vpa_id` at `pinelabs_online/transformers.rs:641`) from `UpiData::UpiIntent | UpiData::UpiQr` (txn_mode `"INTENT"` at `pinelabs_online/transformers.rs:650`) and emits a `PinelabsOnlineUpiDetails { txn_mode, payer }` at `pinelabs_online/transformers.rs:653`.

**PhonePe** UPI authorization: the `PaymentMethodData::Upi(upi_data)` match arm that drives `PhonepePaymentInstrument` construction is at `crates/integrations/connector-integration/src/connectors/phonepe/transformers.rs:259`, with three sub-branches — `UpiData::UpiIntent` (builds `UPI_INTENT` with `target_app` from browser context) at `phonepe/transformers.rs:260`, `UpiData::UpiQr` emitting `UPI_QR` at `phonepe/transformers.rs:269`, and `UpiData::UpiCollect` mapping `collect_data.vpa_id` into the `vpa` field at `phonepe/transformers.rs:274`. The device-context dispatch for Intent is at `phonepe/transformers.rs:293`; all four branches are real (non-Intent/Collect/QR variants fall through to a `not_implemented` guard at `phonepe/transformers.rs:283`).

**Paytm** UPI authorization: `determine_upi_flow` branches on `PaymentMethodData::Upi(upi_data)` at `crates/integrations/connector-integration/src/connectors/paytm/transformers.rs:810`, returning `UpiFlowType::Collect` when `UpiData::UpiCollect.vpa_id` is present (`paytm/transformers.rs:812`) and `UpiFlowType::Intent` for `UpiData::UpiIntent | UpiData::UpiQr` at `paytm/transformers.rs:824`. The parallel `extract_upi_vpa` helper reads `PaymentMethodData::Upi(UpiData::UpiCollect(collect_data))` at `paytm/transformers.rs:839` to produce a validated VPA string.

**PayU** UPI authorization: `determine_upi_flow` matches `PaymentMethodData::Upi(upi_data)` at `crates/integrations/connector-integration/src/connectors/payu/transformers.rs:665` and selects wire fields `(pg="UPI", bankcode="UPI", VPA, S2S flow "2")` for `UpiData::UpiCollect` at `payu/transformers.rs:668` versus a generic-intent branch (falls through with empty bankcode) for `UpiData::UpiIntent | UpiData::UpiQr` at `payu/transformers.rs:639`. Bank-code/target-app lookup for UPI Intent/QR is the `_ => Ok(None)` at `payu/transformers.rs:650` after matching `UpiData::UpiIntent | UpiData::UpiQr` at `payu/transformers.rs:639`, and VPA extraction for Collect is at `payu/transformers.rs:644`.

**RazorpayV2** UPI authorization: the Authorize `TryFrom` builder matches `PaymentMethodData::Upi(upi_data)` at `crates/integrations/connector-integration/src/connectors/razorpayv2/transformers.rs:366`, producing `(UpiFlow::Collect, Some(vpa))` from `UpiData::UpiCollect.vpa_id` at `razorpayv2/transformers.rs:367` and `(UpiFlow::Intent, None)` for `UpiData::UpiIntent | UpiData::UpiQr` at `razorpayv2/transformers.rs:379`. The resulting `RazorpayV2UpiDetails { flow, vpa, expiry_time, upi_type, end_date }` is assembled at `razorpayv2/transformers.rs:388`.

## Pattern Categories

### 1. Standard JSON Pattern

**Applies to**: RazorpayV2, Stripe, Adyen

**Characteristics**:
- Request Format: JSON
- Response Type: Async with redirect for Intent
- Amount Unit: MinorUnit
- Single-phase flow (direct payment creation)

### 2. Two-Phase Flow Pattern

**Applies to**: Cashfree

**Characteristics**:
- Phase 1: Create Order → Returns `payment_session_id`
- Phase 2: Initiate Payment using session ID
- Request Format: JSON
- Amount Unit: FloatMajorUnit (INR as float)

### 3. Encrypted Payload Pattern

**Applies to**: PhonePe

**Characteristics**:
- Request Format: Base64-encoded JSON with checksum
- Custom HMAC-SHA256 signature
- Response Type: Async with deep links
- Amount Unit: MinorUnit

### 4. Session Token Pattern

**Applies to**: Paytm

**Characteristics**:
- Phase 1: Initiate Transaction → Returns `txn_token`
- Phase 2: Process Payment using token
- AES-CBC Encryption for signatures
- Response Type: Async

## Standard JSON Pattern

### Implementation Template

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, CreateOrder, PSync, Refund},
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
        PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId,
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
    {ConnectorName}AuthType, {ConnectorName}AuthorizeRequest,
    {ConnectorName}AuthorizeResponse, {ConnectorName}ErrorResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {ConnectorName}<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
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
        (
            flow: PSync,
            request_body: {ConnectorName}SyncRequest,
            response_body: {ConnectorName}SyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
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
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_authorization_header().into_masked(),
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
                .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?
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

// Authorize flow implementation
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
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(format!("{base_url}/v1/payments"))
        }
    }
);
```

### Transformers Implementation

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, ResponseId},
    errors::{self, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, UpiData},
    router_data::{ConnectorAuthType, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// ============================================================================
// Authentication
// ============================================================================

#[derive(Debug)]
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
}

impl {ConnectorName}AuthType {
    pub fn generate_authorization_header(&self) -> String {
        let credentials = format!("{}:{}", self.api_key.peek(), self.api_secret.peek());
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        format!("Basic {encoded}")
    }
}

impl TryFrom<&ConnectorAuthType> for {ConnectorName}AuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorAuthType::SignatureKey { api_key, api_secret, .. } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() }.into()),
        }
    }
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct {ConnectorName}AuthorizeRequest<T: PaymentMethodDataTypes + Sync + Send + 'static + Serialize> {
    pub amount: MinorUnit,
    pub currency: String,
    pub order_id: String,
    pub method: String, // "upi"
    pub upi: {ConnectorName}UpiDetails,
    pub customer_email: String,
    pub customer_contact: String,
    pub callback_url: String,
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}UpiDetails {
    pub flow: UpiFlowType, // "collect" or "intent"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpa: Option<Secret<String>>, // Required for collect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_time: Option<i32>, // In minutes
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpiFlowType {
    Collect,
    Intent,
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}AuthorizeResponse {
    pub id: String,
    pub status: {ConnectorName}PaymentStatus,
    pub amount: i64,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>, // Deep link for Intent flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum {ConnectorName}PaymentStatus {
    Created,
    Authorized,
    Captured,
    Failed,
}

#[derive(Debug, Deserialize)]
pub struct {ConnectorName}ErrorResponse {
    #[serde(rename = "error_code")]
    pub error_code: Option<String>,
    #[serde(rename = "error_description")]
    pub error_description: Option<String>,
    #[serde(rename = "error_message")]
    pub error_message: Option<String>,
    pub transaction_id: Option<String>,
}

impl Default for {ConnectorName}ErrorResponse {
    fn default() -> Self {
        Self {
            error_code: Some("UNKNOWN_ERROR".to_string()),
            error_description: Some("Unknown error occurred".to_string()),
            error_message: Some("Unknown error".to_string()),
            transaction_id: None,
        }
    }
}

// ============================================================================
// Request Transformation
// ============================================================================

pub struct {ConnectorName}RouterData<T, U> {
    pub amount: MinorUnit,
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(MinorUnit, T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((amount, router_data, connector): (MinorUnit, T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            amount,
            router_data,
            connector,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<{ConnectorName}RouterData<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        &{ConnectorName}<T>,
    >> for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            &{ConnectorName}<T>,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = item.router_data;

        // Determine UPI flow and extract VPA if needed
        let (upi_flow, vpa) = match &router_data.request.payment_method_data {
            PaymentMethodData::Upi(upi_data) => match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    let vpa_string = collect_data
                        .vpa_id
                        .as_ref()
                        .ok_or(errors::IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                        , context: Default::default() })?
                        .peek()
                        .to_string();
                    (UpiFlowType::Collect, Some(vpa_string))
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => (UpiFlowType::Intent, None),
            },
            _ => {
                return Err(errors::IntegrationError::NotSupported {
                    message: "Only UPI payment methods are supported".to_string(),
                    connector: "{ConnectorName, context: Default::default() }",
                }
                .into())
            }
        };

        let upi_details = {ConnectorName}UpiDetails {
            flow: upi_flow,
            vpa: vpa.map(Secret::new),
            expiry_time: Some(15), // Default 15 minutes
        };

        Ok(Self {
            amount: item.amount,
            currency: router_data.request.currency.to_string(),
            order_id: router_data.resource_common_data.connector_request_reference_id.clone(),
            method: "upi".to_string(),
            upi: upi_details,
            customer_email: router_data
                .resource_common_data
                .get_billing_email()
                .map(|e| e.peek().to_string())
                .unwrap_or_default(),
            customer_contact: router_data
                .resource_common_data
                .get_billing_phone_number()
                .map(|p| p.peek().to_string())
                .unwrap_or_default(),
            callback_url: router_data.request.get_webhook_url()?,
        })
    }
}

// ============================================================================
// Response Transformation
// ============================================================================

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}AuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}AuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;
        let router_data = item.router_data;

        // Map connector status to attempt status
        let status = match response.status {
            {ConnectorName}PaymentStatus::Created => common_enums::AttemptStatus::AuthenticationPending,
            {ConnectorName}PaymentStatus::Authorized => common_enums::AttemptStatus::Authorized,
            {ConnectorName}PaymentStatus::Captured => common_enums::AttemptStatus::Charged,
            {ConnectorName}PaymentStatus::Failed => common_enums::AttemptStatus::Failure,
        };

        // Handle error responses
        if let Some(error_code) = response.error_code {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error_code,
                    message: response.error_description.unwrap_or_default(),
                    reason: response.error_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::Failure),
                    connector_transaction_id: Some(response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..router_data.clone()
            });
        }

        // Create redirection data for Intent flow
        let redirection_data = response.link.map(|url| {
            Box::new(RedirectForm::Uri { uri: url })
        });

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.id),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: Some(router_data.resource_common_data.connector_request_reference_id.clone()),
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

## Two-Phase Flow Pattern

**Applies to**: Cashfree

### Overview

Cashfree uses a two-phase flow for UPI payments:
1. **Create Order**: Creates an order and returns `payment_session_id`
2. **Authorize**: Initiates payment using the session ID

### Implementation Differences

```rust
// Phase 1: Create Order Request
#[derive(Debug, Serialize)]
pub struct CashfreeOrderCreateRequest {
    pub order_id: String,
    pub order_amount: f64, // FloatMajorUnit
    pub order_currency: String,
    pub customer_details: CashfreeCustomerDetails,
    pub order_meta: CashfreeOrderMeta,
}

// Phase 2: Payment Request
#[derive(Debug, Serialize)]
pub struct CashfreePaymentRequest {
    pub payment_session_id: String, // From order creation
    pub payment_method: CashfreePaymentMethod,
}

#[derive(Debug, Serialize)]
pub struct CashfreePaymentMethod {
    pub upi: Option<CashfreeUpiDetails>,
    // ... other payment methods
}

#[derive(Debug, Serialize)]
pub struct CashfreeUpiDetails {
    pub channel: String, // "link" for Intent, "collect" for Collect
    pub upi_id: Secret<String>, // VPA for collect, empty for intent
}
```

### Channel Mapping

| UPI Type | Channel Value | Behavior |
|----------|---------------|----------|
| Intent | `"link"` | Returns deep link in response |
| Collect | `"collect"` | Sends collect request to VPA |

## Encrypted Payload Pattern

**Applies to**: PhonePe

### Overview

PhonePe requires:
1. Base64-encoded JSON payload
2. HMAC-SHA256 checksum
3. Specific header format with key index

### Key Implementation Details

```rust
// Request structure
#[derive(Debug, Serialize)]
pub struct PhonepePaymentsRequest {
    request: Secret<String>, // Base64 encoded payload
    #[serde(skip)]
    pub checksum: String, // SHA256 checksum
}

// Payload structure (before encoding)
#[derive(Debug, Serialize)]
struct PhonepePaymentRequestPayload {
    #[serde(rename = "merchantId")]
    merchant_id: Secret<String>,
    #[serde(rename = "merchantTransactionId")]
    merchant_transaction_id: String,
    amount: MinorUnit,
    #[serde(rename = "callbackUrl")]
    callback_url: String,
    #[serde(rename = "paymentInstrument")]
    payment_instrument: PhonepePaymentInstrument,
    #[serde(rename = "deviceContext")]
    device_context: Option<PhonepeDeviceContext>,
    #[serde(rename = "paymentMode")]
    payment_mode: Option<String>, // From UpiSource
}

#[derive(Debug, Serialize)]
struct PhonepePaymentInstrument {
    #[serde(rename = "type")]
    instrument_type: String, // "UPI_INTENT", "UPI_QR", "UPI_COLLECT"
    #[serde(rename = "targetApp")]
    target_app: Option<String>, // For Intent (GPay, PhonePe, etc.)
    vpa: Option<Secret<String>>, // For Collect
}

// Checksum generation
fn generate_phonepe_checksum(
    base64_payload: &str,
    api_path: &str,
    salt_key: &Secret<String>,
    key_index: &str,
) -> Result<String, Error> {
    // SHA256(base64Payload + apiPath + saltKey) + "###" + keyIndex
    let checksum_input = format!("{}{}{}", base64_payload, api_path, salt_key.peek());
    let hash = sha256(checksum_input);
    Ok(format!("{}{}{}", hash, "###", key_index))
}
```

### Target App Mapping (PhonePe)

```rust
fn get_target_app_for_phonepe(
    intent_data: &UpiIntentData,
    browser_info: &Option<BrowserInformation>,
) -> Option<String> {
    match get_mobile_os(browser_info).as_str() {
        "ANDROID" => intent_data.app_name.clone(),
        _ => map_ios_payment_source_to_target_app(intent_data.app_name.as_deref()),
    }
}

fn map_ios_payment_source_to_target_app(payment_source: Option<&str>) -> Option<String> {
    payment_source.and_then(|source| {
        let source_lower = source.to_lowercase();
        match source_lower.as_str() {
            s if s.contains("tez") => Some("GPAY".to_string()),
            s if s.contains("phonepe") => Some("PHONEPE".to_string()),
            s if s.contains("paytm") => Some("PAYTM".to_string()),
            _ => None,
        }
    })
}
```

### UPI Source to Payment Mode Mapping

```rust
impl UpiSource {
    pub fn to_payment_mode(&self) -> String {
        match self {
            Self::UpiCc | Self::UpiCl | Self::UpiCcCl | Self::UpiPpi | Self::UpiVoucher => {
                "ALL".to_string()
            }
            Self::UpiAccount => "ACCOUNT".to_string(),
        }
    }
}
```

## Session Token Pattern

**Applies to**: Paytm

### Overview

Paytm uses a session-based flow:
1. **Initiate Transaction** → Returns `txn_token`
2. **Process Payment** (Authorize) using the token

### Key Implementation Details

```rust
// Phase 1: Initiate Transaction
#[derive(Debug, Serialize)]
pub struct PaytmInitiateTxnRequest {
    pub head: PaytmRequestHeader,
    pub body: PaytmInitiateReqBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmInitiateReqBody {
    pub request_type: String, // "Payment"
    pub mid: Secret<String>, // Merchant ID
    pub order_id: String,
    pub website_name: Secret<String>,
    pub txn_amount: PaytmAmount,
    pub user_info: PaytmUserInfo,
    pub enable_payment_mode: Vec<PaytmEnableMethod>, // UPI config
    pub callback_url: String,
}

// Phase 2: Process Payment (Intent)
#[derive(Debug, Serialize)]
pub struct PaytmProcessTxnRequest {
    pub head: PaytmProcessHeadTypes,
    pub body: PaytmProcessBodyTypes,
}

#[derive(Debug, Serialize)]
pub struct PaytmProcessBodyTypes {
    pub mid: Secret<String>,
    pub order_id: String,
    pub request_type: String, // "NATIVE"
    pub payment_mode: String, // "UPI_INTENT"
    pub payment_flow: Option<String>, // "NONE"
}

// Phase 2: Process Payment (Collect) - Different structure!
#[derive(Debug, Serialize)]
pub struct PaytmNativeProcessTxnRequest {
    pub head: PaytmTxnTokenType,
    pub body: PaytmNativeProcessRequestBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmNativeProcessRequestBody {
    pub request_type: String,
    pub mid: Secret<String>,
    pub order_id: String,
    pub payment_mode: String, // "UPI"
    pub payer_account: Option<String>, // VPA
    pub channel_id: String,
    pub txn_token: Secret<String>,
}
```

### Flow Determination

```rust
pub fn determine_upi_flow<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> CustomResult<UpiFlowType, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(upi_data) => {
            match upi_data {
                UpiData::UpiCollect(collect_data) => {
                    if collect_data.vpa_id.is_some() {
                        Ok(UpiFlowType::Collect)
                    } else {
                        Err(IntegrationError::MissingRequiredField {
                            field_name: "vpa_id",
                        , context: Default::default() }.into())
                    }
                }
                UpiData::UpiIntent(_) | UpiData::UpiQr(_) => Ok(UpiFlowType::Intent),
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Only UPI payment methods are supported".to_string(),
            connector: "Paytm",
        , context: Default::default() }.into()),
    }
}
```

### VPA Validation

```rust
pub fn extract_upi_vpa<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> CustomResult<Option<String>, IntegrationError> {
    match payment_method_data {
        PaymentMethodData::Upi(UpiData::UpiCollect(collect_data)) => {
            if let Some(vpa_id) = &collect_data.vpa_id {
                let vpa = vpa_id.peek().to_string();
                // Basic VPA validation: must contain @ and be at least 4 chars
                if vpa.contains('@') && vpa.len() > 3 {
                    Ok(Some(vpa))
                } else {
                    Err(IntegrationError::RequestEncodingFailedWithReason(
                        "Invalid UPI VPA format".to_string(),
                    ).into())
                }
            } else {
                Err(IntegrationError::MissingRequiredField {
                    field_name: "vpa_id",
                , context: Default::default() }.into())
            }
        }
        _ => Ok(None),
    }
}
```

### Signature Generation (AES Encryption)

```rust
pub fn generate_paytm_signature(
    payload: &str,
    merchant_key: &str,
) -> CustomResult<String, IntegrationError> {
    // Step 1: Generate random salt
    let rng = SystemRandom::new();
    let mut salt_bytes = [0u8; 3];
    rng.fill(&mut salt_bytes).map_err(|_| {
        IntegrationError::RequestEncodingFailedWithReason("Salt generation failed".to_string())
    })?;

    // Step 2: Base64 encode salt
    let salt_b64 = general_purpose::STANDARD.encode(salt_bytes);

    // Step 3: Create hash input
    let hash_input = format!("{}|{}", payload, salt_b64);

    // Step 4: SHA-256 hash
    let hash_digest = digest::digest(&digest::SHA256, hash_input.as_bytes());
    let sha256_hash = hex::encode(hash_digest.as_ref());

    // Step 5: Create checksum
    let checksum = format!("{}{}", sha256_hash, salt_b64);

    // Step 6: AES-CBC encrypt with fixed IV
    aes_encrypt(&checksum, merchant_key)
}

fn aes_encrypt(data: &str, key: &str) -> CustomResult<String, IntegrationError> {
    let iv = b"@@@@&&&&####$$$$"; // Fixed IV from Paytm spec
    // ... AES-CBC encryption with PKCS7 padding
}
```

## Sub-type Variations

### UPI Intent

| Connector | Request Structure | Response Handling | Key Fields |
|-----------|-------------------|-------------------|------------|
| RazorpayV2 | `upi: { flow: "intent" }` | Returns `link` for redirection | `flow`, `expiry_time` |
| PhonePe | `instrument_type: "UPI_INTENT"` | Returns `intentUrl` | `targetApp`, `deviceContext` |
| Cashfree | `channel: "link"` | Returns deep link in `payload.default` | `channel` |
| Paytm | `payment_mode: "UPI_INTENT"` | Returns `deep_link_info.deep_link` | `payment_flow: "NONE"` |

### UPI Collect

| Connector | Request Structure | Response Handling | Key Fields |
|-----------|-------------------|-------------------|------------|
| RazorpayV2 | `upi: { flow: "collect", vpa: "..." }` | Returns pending status | `vpa`, `expiry_time` |
| PhonePe | `instrument_type: "UPI_COLLECT"` | Returns wait screen | `vpa` |
| Cashfree | `channel: "collect", upi_id: "..."` | Returns pending status | `upi_id` |
| Paytm | `payment_mode: "UPI", payer_account: "..."` | Returns pending status | `payer_account` |

### UPI QR

| Connector | Request Structure | Response Handling | Key Fields |
|-----------|-------------------|-------------------|------------|
| RazorpayV2 | `upi: { flow: "intent" }` | Same as Intent | - |
| PhonePe | `instrument_type: "UPI_QR"` | Returns `qr_data` | `intentUrl` for display |
| Cashfree | Not supported | - | - |
| Paytm | Not supported | - | - |

## Common Pitfalls

### 1. VPA Validation

❌ **Wrong**: No validation before sending to connector
```rust
let vpa = collect_data.vpa_id.as_ref().map(|v| v.peek().to_string());
```

✅ **Right**: Validate VPA format before sending
```rust
let vpa = collect_data
    .vpa_id
    .as_ref()
    .ok_or(IntegrationError::MissingRequiredField { field_name: "vpa_id" , context: Default::default() })?;
let vpa_str = vpa.peek().to_string();
if !vpa_str.contains('@') || vpa_str.len() <= 3 {
    return Err(IntegrationError::RequestEncodingFailedWithReason(
        "Invalid VPA format".to_string(),
    ).into());
}
```

### 2. Amount Unit Confusion

❌ **Wrong**: Using wrong amount unit
```rust
// If connector expects MinorUnit but we send MajorUnit
pub order_amount: f64, // This is wrong for MinorUnit connectors
```

✅ **Right**: Use appropriate amount converter
```rust
// In macro setup:
amount_converters: [
    amount_converter: MinorUnit  // or FloatMajorUnit for Cashfree
]
```

### 3. Status Mapping

❌ **Wrong**: Hardcoding status
```rust
let status = common_enums::AttemptStatus::Charged; // WRONG!
```

✅ **Right**: Map from connector response
```rust
let status = match response.status {
    {ConnectorName}PaymentStatus::Captured => common_enums::AttemptStatus::Charged,
    {ConnectorName}PaymentStatus::Authorized => common_enums::AttemptStatus::Authorized,
    {ConnectorName}PaymentStatus::Created => common_enums::AttemptStatus::AuthenticationPending,
    {ConnectorName}PaymentStatus::Failed => common_enums::AttemptStatus::Failure,
};
```

### 4. Deep Link Handling

❌ **Wrong**: Using deep link as-is without parsing
```rust
let redirect_form = RedirectForm::Uri { uri: deep_link };
```

✅ **Right**: Parse and handle query parameters correctly
```rust
// For Cashfree: Trim at "?" to get intent parameters
let trimmed_link = if let Some(pos) = deep_link.find('?') {
    &deep_link[(pos + 1)..]
} else {
    &deep_link
};
let redirect_form = RedirectForm::Uri { uri: trimmed_link.to_string() };
```

### 5. UPI Source Handling

❌ **Wrong**: Ignoring upi_source
```rust
// Not using upi_source for payment_mode
```

✅ **Right**: Extract and use upi_source
```rust
let payment_mode = router_data
    .request
    .payment_method_data
    .get_upi_source()
    .map(|source| source.to_payment_mode());
```

## Testing Patterns

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upi_intent_request_transformation() {
        let router_data = create_test_router_data_with_upi_intent();
        let connector_req = {ConnectorName}AuthorizeRequest::try_from(router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert_eq!(req.method, "upi");
        assert!(matches!(req.upi.flow, UpiFlowType::Intent));
        assert!(req.upi.vpa.is_none());
    }

    #[test]
    fn test_upi_collect_request_transformation() {
        let router_data = create_test_router_data_with_upi_collect();
        let connector_req = {ConnectorName}AuthorizeRequest::try_from(router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert_eq!(req.method, "upi");
        assert!(matches!(req.upi.flow, UpiFlowType::Collect));
        assert!(req.upi.vpa.is_some());
    }

    #[test]
    fn test_vpa_validation() {
        let valid_vpa = "test@upi";
        let invalid_vpa = "invalid";

        assert!(valid_vpa.contains('@') && valid_vpa.len() > 3);
        assert!(!invalid_vpa.contains('@'));
    }

    #[test]
    fn test_upi_intent_response_transformation() {
        let response = {ConnectorName}AuthorizeResponse {
            id: "test_txn_id".to_string(),
            status: {ConnectorName}PaymentStatus::Created,
            amount: 10000,
            currency: "INR".to_string(),
            link: Some("upi://pay?pa=merchant@upi&pn=Merchant&am=100.00".to_string()),
            error_code: None,
            error_description: None,
        };

        let router_data = create_test_router_data();
        let response_router_data = ResponseRouterData {
            response,
            data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert_eq!(
            router_data_result.resource_common_data.status,
            common_enums::AttemptStatus::AuthenticationPending
        );
        assert!(router_data_result.response.unwrap().redirection_data.is_some());
    }
}
```

### Integration Test Scenarios

1. **UPI Intent Flow**
   - Test deep link generation
   - Verify redirect form structure
   - Test different target apps (GPay, PhonePe, Paytm)

2. **UPI Collect Flow**
   - Test VPA validation
   - Test collect request creation
   - Verify pending status handling

3. **UPI QR Flow**
   - Test QR data generation
   - Verify display URL handling

4. **Error Scenarios**
   - Invalid VPA format
   - Expired UPI requests
   - Customer declined on UPI app

## Implementation Checklist

### Pre-Implementation

- [ ] Review connector's UPI API documentation
- [ ] Identify UPI flows supported (Intent/Collect/QR)
- [ ] Determine authentication mechanism
- [ ] Identify amount unit (Minor/FloatMajor)
- [ ] Review webhook payload structure

### Request Implementation

- [ ] Implement authentication type extraction
- [ ] Create request structures for all UPI flows
- [ ] Implement flow determination logic
- [ ] Add VPA validation for Collect flow
- [ ] Handle UPI source mapping if needed
- [ ] Implement amount conversion

### Response Implementation

- [ ] Create response structures
- [ ] Implement status mapping
- [ ] Handle deep link extraction for Intent
- [ ] Handle QR data extraction
- [ ] Implement error response handling
- [ ] Set proper attempt statuses

### Testing

- [ ] Unit tests for request transformation
- [ ] Unit tests for response transformation
- [ ] Unit tests for VPA validation
- [ ] Unit tests for flow determination
- [ ] Integration tests with sandbox
- [ ] Webhook handling tests

### Documentation

- [ ] Document supported UPI flows
- [ ] Document authentication requirements
- [ ] Document VPA validation rules
- [ ] Document status mapping
- [ ] Add code comments for complex logic

## Placeholder Reference Guide

| Placeholder | Description | Example Values |
|-------------|-------------|----------------|
| `{ConnectorName}` | Connector name in PascalCase | `RazorpayV2`, `PhonePe`, `Cashfree`, `Paytm` |
| `{connector_name}` | Connector name in snake_case | `razorpayv2`, `phonepe`, `cashfree`, `paytm` |
| `{AmountType}` | Amount type for connector | `MinorUnit`, `FloatMajorUnit` |

## Connector-Specific Notes

### RazorpayV2
- Uses Basic Auth (API Key + Secret)
- Single-phase flow
- Supports all three UPI variants
- Returns unified response structure

### PhonePe
- Uses HMAC-SHA256 checksum
- Base64 encoded payload
- Requires device context for Intent
- Supports merchant-specific endpoints (IRCTC)

### Cashfree
- Two-phase flow (Create Order → Payment)
- Uses `payment_session_id` from order
- Amount in FloatMajorUnit
- Channel-based flow differentiation

### Paytm
- Session token flow
- AES-CBC encryption for signatures
- Different request structures for Intent vs Collect
- Fixed IV for encryption: `@@@@&&&&####$$$$`

---

## Change Log

| Date | Version | Pinned SHA | Change |
|------|---------|------------|--------|
| 2026-04-20 | 1.3.0 | `60540470cf84a350cc02b0d41565e5766437eb95` | Final-polish citation pass. Added **PayU** row to the Supported Connectors table. Added per-connector `file:line` citation paragraphs for **PhonePe** (`phonepe/transformers.rs:259`, `:260`, `:269`, `:274`, `:283`, `:293`), **Paytm** (`paytm/transformers.rs:810`, `:812`, `:824`, `:839`), **PayU** (`payu/transformers.rs:639`, `:644`, `:650`, `:665`, `:668`), and **RazorpayV2** (`razorpayv2/transformers.rs:366`, `:367`, `:379`, `:388`) documenting the real `PaymentMethodData::Upi(_)` match arms in each connector's Authorize transformer. |
| 2026-04-20 | 1.2.0 | `60540470cf84a350cc02b0d41565e5766437eb95` | Bumped Version field in header metadata table (verification agent confirmed all 9 variants present: 3 `UpiData` + 6 `UpiSource`; canonical header table already in place). No substantive content changes this revision. |
| 2026-04-20 | 1.1.0 | `60540470cf84a350cc02b0d41565e5766437eb95` | Added document header metadata block. Added **PineLabs Online** row to the Supported Connectors table with `file:line` citations for `PaymentMethodData::Upi(_)` -> `"UPI"` mapping at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:621` and the collect/intent dispatch in `build_payment_option` at `pinelabs_online/transformers.rs:638-653` -- PR #795. |
| (prior) | 1.0.0 | (initial) | Initial authoring covering Standard JSON, Two-Phase, Encrypted Payload, and Session Token patterns across Razorpay, PhonePe, Cashfree, Paytm, Stripe, and Adyen. |

---

This pattern document provides comprehensive guidance for implementing UPI authorize flows across different connector types in the Grace-UCS system.
