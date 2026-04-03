# Authorize Flow Pattern for Crypto Payment Method

**🎯 CRYPTO PAYMENT METHOD PATTERN**

This document provides comprehensive, reusable patterns for implementing the authorize flow for **Crypto** payments in payment connectors. Crypto payments involve cryptocurrency transactions where customers pay using digital currencies like Bitcoin, Ethereum, etc.

## 🚀 Quick Start Guide

To implement a new connector supporting Crypto payments:

1. **Understand CryptoData**: The `CryptoData` struct contains `pay_currency` (cryptocurrency type) and optional `network` fields
2. **Choose Pattern**: Use the **Redirect Pattern** (Crypto payments typically require customer redirection to complete payment)
3. **Handle Async Confirmation**: Crypto payments are asynchronous - implement webhook handling and PSync
4. **Set Proper Amount Unit**: Most crypto connectors use `StringMajorUnit` for fiat amount representation

### Example: Crypto Payment Flow

```bash
# Key fields from CryptoData:
pay_currency: "BTC"  # Cryptocurrency to accept (Bitcoin)
network: "mainnet"   # Optional blockchain network

# Request structure:
{
  "price_amount": "100.00",     # Fiat amount (StringMajorUnit)
  "price_currency": "USD",      # Fiat currency
  "pay_currency": "BTC",        # Cryptocurrency
  "network": "mainnet",         # Optional network
  "redirect_url": "https://..." # Return URL after payment
}
```

**✅ Result**: Customer is redirected to crypto payment page to complete the transaction

## Table of Contents

1. [Overview](#overview)
2. [CryptoData Structure](#cryptodata-structure)
3. [Supported Connectors](#supported-connectors)
4. [Redirect Pattern (Primary)](#redirect-pattern-primary)
5. [Request Patterns](#request-patterns)
6. [Response Patterns](#response-patterns)
7. [Status Mapping](#status-mapping)
8. [Webhook Handling](#webhook-handling)
9. [PSync Implementation](#psync-implementation)
10. [Error Handling](#error-handling)
11. [Implementation Templates](#implementation-templates)
12. [Common Pitfalls](#common-pitfalls)
13. [Testing Patterns](#testing-patterns)
14. [Integration Checklist](#integration-checklist)

## Overview

### What is Crypto Payment?

Crypto payments allow customers to pay for goods and services using cryptocurrencies. The typical flow involves:

1. **Invoice Creation**: Merchant creates a crypto payment invoice specifying fiat amount and desired cryptocurrency
2. **Customer Redirection**: Customer is redirected to a hosted payment page
3. **Crypto Transfer**: Customer sends cryptocurrency from their wallet
4. **Blockchain Confirmation**: Payment is confirmed on the blockchain (may take minutes)
5. **Webhook Notification**: Connector notifies merchant of payment status
6. **PSync Polling**: Optional status polling for confirmation

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Payment Type** | Asynchronous with redirect |
| **Confirmation** | Blockchain-based, requires multiple confirmations |
| **Amount Format** | StringMajorUnit for fiat amount |
| **Required Fields** | `pay_currency` (crypto type), fiat amount/currency |
| **Optional Fields** | `network` (blockchain network), metadata |
| **Webhook Support** | Required for reliable status updates |
| **PSync** | Required for status polling |

## CryptoData Structure

### Domain Type Definition

```rust
// File: crates/types-traits/domain_types/src/payment_method_data.rs

#[derive(Debug, Clone, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CryptoData {
    pub pay_currency: Option<String>,  // Cryptocurrency code (e.g., "BTC", "ETH")
    pub network: Option<String>,       // Blockchain network (e.g., "mainnet", "sepolia")
}

impl CryptoData {
    pub fn get_pay_currency(&self) -> Result<String, Error> {
        self.pay_currency
            .clone()
            .ok_or_else(missing_field_err("crypto_data.pay_currency"))
    }
}
```

### PaymentMethodData Enum

```rust
pub enum PaymentMethodData<T: PaymentMethodDataTypes> {
    // ... other variants
    Crypto(CryptoData),
    // ... other variants
}
```

### Accessing Crypto Data in Connectors

```rust
// Pattern for extracting crypto data in TryFrom implementations
match &router_data.request.payment_method_data {
    PaymentMethodData::Crypto(ref crypto_data) => {
        let pay_currency = crypto_data.get_pay_currency()?;
        let network = crypto_data.network.clone();
        // ... build request
    }
    _ => Err(IntegrationError::NotImplemented(
        get_unimplemented_payment_method_error_message("ConnectorName", Default::default())
    )),
}
```

## Supported Connectors

| Connector | Crypto Support | Pattern | Webhook | PSync |
|-----------|---------------|---------|---------|-------|
| **Cryptopay** | Full | Redirect | Yes | Yes |
| Stripe | Not Implemented | - | - | - |
| Adyen | Not Implemented | - | - | - |
| PayPal | Not Implemented | - | - | - |
| Most Others | Not Implemented | - | - | - |

**Note**: Cryptopay is the primary reference implementation for Crypto payments in the Grace-UCS system.

## Redirect Pattern (Primary)

Crypto payments universally use the **Redirect Pattern** because:
- Customers need to access their crypto wallet
- Blockchain transactions require external interaction
- Payment confirmation happens outside merchant's control

### Flow Diagram

```
Merchant App        Grace-UCS           Connector           Customer Wallet
    |                   |                   |                    |
    |  1. Payment Req   |                   |                    |
    |------------------>|                   |                    |
    |                   |  2. Create Invoice|                    |
    |                   |------------------>|                    |
    |                   |  3. Invoice Data  |                    |
    |                   |<------------------|                    |
    |                   |                   |  4. Redirect URL   |
    |  5. Redirect Form |                   |                    |
    |<------------------|                   |                    |
    |  6. Open Payment  |                   |                    |
    |-------------------------------------->|                    |
    |                   |                   |  7. Send Crypto    |
    |                   |                   |<-------------------|
    |                   |  8. Webhook       |                    |
    |                   |<------------------|                    |
    |  9. Status Update |                   |                    |
    |<------------------|                   |                    |
```

## Request Patterns

### Standard Crypto Request Structure

```rust
#[derive(Default, Debug, Serialize)]
pub struct CryptoPaymentsRequest {
    // Fiat amount and currency (what merchant wants to receive)
    pub price_amount: StringMajorUnit,     // e.g., "100.00" for $100
    pub price_currency: common_enums::Currency,  // e.g., USD, EUR

    // Cryptocurrency details (what customer will pay)
    pub pay_currency: String,              // e.g., "BTC", "ETH", "USDC"

    // Optional network specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,           // e.g., "mainnet", "erc20"

    // Redirect URLs
    pub success_redirect_url: Option<String>,
    pub unsuccess_redirect_url: Option<String>,

    // Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<pii::SecretSerdeValue>,

    // Reference ID
    pub custom_id: String,
}
```

### Request Transformation Implementation

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        CryptoRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for CryptoPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: CryptoRouterData<...>,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Crypto(ref crypto_data) => {
                // Extract required fields
                let pay_currency = crypto_data.get_pay_currency()?;

                // Convert amount to StringMajorUnit
                let amount = CryptoAmountConvertor::convert(
                    item.router_data.request.minor_amount,
                    item.router_data.request.currency,
                )?;

                Ok(Self {
                    price_amount: amount,
                    price_currency: item.router_data.request.currency,
                    pay_currency,
                    network: crypto_data.network.to_owned(),
                    success_redirect_url: item.router_data.request.router_return_url.clone(),
                    unsuccess_redirect_url: item.router_data.request.router_return_url.clone(),
                    metadata: item.router_data.request.get_metadata_as_object(),
                    custom_id: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                })
            }
            // Other payment methods not supported
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("CryptoConnector", Default::default()),
            )),
        }
    }
}
```

### URL Pattern

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<String, errors::IntegrationError> {
    let base_url = self.connector_base_url_payments(req);
    // Crypto connectors typically use "/invoices" or "/payments" endpoint
    Ok(format!("{}/api/invoices", base_url))
}
```

## Response Patterns

### Standard Crypto Response Structure

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CryptoPaymentsResponse {
    pub data: CryptoPaymentResponseData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CryptoPaymentResponseData {
    pub id: String,                           // Connector transaction ID
    pub custom_id: Option<String>,            // Merchant reference ID
    pub status: CryptoPaymentStatus,          // Payment status
    pub status_context: Option<String>,       // Additional status info

    // Fiat amount details
    pub price_amount: Option<StringMajorUnit>,
    pub price_currency: Option<common_enums::Currency>,

    // Crypto amount details
    pub pay_amount: Option<StringMajorUnit>,
    pub pay_currency: Option<String>,

    // Blockchain details
    pub address: Option<Secret<String>>,      // Payment address
    pub network: Option<String>,              // Blockchain network
    pub uri: Option<String>,                  // Payment URI

    // Redirect URL
    pub hosted_page_url: Option<Url>,         // Customer redirect URL

    // Timestamps
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
}
```

### Response Transformation Implementation

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<CryptoPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<CryptoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response: crypto_response,
            router_data,
            http_code,
        } = item;

        // Map connector status to attempt status
        let status = common_enums::AttemptStatus::from(crypto_response.data.status.clone());

        // Handle failure cases
        let response = if is_payment_failure(status) {
            Err(ErrorResponse {
                code: crypto_response.data.name.clone()
                    .unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
                message: crypto_response.data.status_context.clone()
                    .unwrap_or_else(|| "Payment failed".to_string()),
                reason: crypto_response.data.status_context.clone(),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: Some(crypto_response.data.id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            // Create redirect form for successful invoice creation
            let redirection_data = crypto_response
                .data
                .hosted_page_url
                .map(|url| RedirectForm::from((url, common_utils::request::Method::Get)));

            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(crypto_response.data.id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: crypto_response
                    .data
                    .custom_id
                    .or(Some(crypto_response.data.id)),
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        };

        // Handle amount captured for successful payments
        let amount_captured_in_minor_units = match crypto_response.data.price_amount {
            Some(ref amount) => Some(CryptoAmountConvertor::convert_back(
                amount.clone(),
                router_data.request.currency,
            )?),
            None => None,
        };

        match (amount_captured_in_minor_units, status) {
            (Some(minor_amount), common_enums::AttemptStatus::Charged) => {
                let amount_captured = Some(minor_amount.get_amount_as_i64());
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        amount_captured,
                        minor_amount_captured: amount_captured_in_minor_units,
                        ..router_data.resource_common_data
                    },
                    response,
                    ..router_data
                })
            }
            _ => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data
                },
                response,
                ..router_data
            }),
        }
    }
}
```

## Status Mapping

### Crypto-Specific Status Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CryptoPaymentStatus {
    New,          // Invoice created, awaiting payment
    Completed,    // Payment confirmed on blockchain
    Unresolved,   // Payment issue requiring manual review
    Refunded,     // Payment was refunded
    Cancelled,    // Invoice expired or cancelled
}
```

### Status Mapping Implementation

```rust
impl From<CryptoPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: CryptoPaymentStatus) -> Self {
        match status {
            // Invoice created, waiting for customer to pay
            CryptoPaymentStatus::New => Self::AuthenticationPending,

            // Payment confirmed on blockchain
            CryptoPaymentStatus::Completed => Self::Charged,

            // Payment failed or expired
            CryptoPaymentStatus::Cancelled => Self::Failure,

            // Requires manual intervention or special handling
            CryptoPaymentStatus::Unresolved | CryptoPaymentStatus::Refunded => {
                Self::Unresolved
            }
        }
    }
}
```

### Status Mapping Reference

| Connector Status | Attempt Status | Description |
|-----------------|----------------|-------------|
| `New` | `AuthenticationPending` | Invoice created, awaiting customer payment |
| `Completed` | `Charged` | Crypto payment confirmed on blockchain |
| `Cancelled` | `Failure` | Invoice expired or was cancelled |
| `Unresolved` | `Unresolved` | Payment issue requiring manual review |
| `Refunded` | `Unresolved` | Refund processed (no refund API available) |

## Webhook Handling

### Webhook Structure

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CryptoWebhookDetails {
    #[serde(rename = "type")]
    pub service_type: String,       // Service type identifier
    pub event: WebhookEvent,        // Event type
    pub data: CryptoPaymentResponseData,  // Payment data
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    TransactionCreated,     // New invoice created
    TransactionConfirmed,   // Payment confirmed
    StatusChanged,          // Status updated
}
```

### Webhook Event Mapping

```rust
fn get_event_type(
    &self,
    request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorAuthType>,
) -> Result<EventType, error_stack::Report<errors::IntegrationError>> {
    let notif: CryptoWebhookDetails = request
        .body
        .parse_struct("CryptoWebhookDetails")
        .change_context(errors::IntegrationError::WebhookEventTypeNotFound)?;

    match notif.data.status {
        CryptoPaymentStatus::Completed => Ok(EventType::PaymentIntentSuccess),
        CryptoPaymentStatus::Unresolved => Ok(EventType::PaymentActionRequired),
        CryptoPaymentStatus::Cancelled => Ok(EventType::PaymentIntentFailure),
        _ => Ok(EventType::IncomingWebhookEventUnspecified),
    }
}
```

### Webhook Source Verification

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for CryptoConnector<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::IntegrationError>> {
        let signature = request
            .headers
            .get("x-crypto-signature")
            .ok_or(errors::IntegrationError::WebhookSourceVerificationFailed)
            .attach_printable("Missing webhook signature")?;

        hex::decode(signature)
            .change_context(errors::IntegrationError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<errors::IntegrationError>> {
        // Return raw body for HMAC verification
        Ok(request.body.to_vec())
    }
}
```

## PSync Implementation

Crypto payments require PSync for polling status when webhooks are delayed or missed.

### PSync URL Pattern

```rust
fn get_url(
    &self,
    req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
) -> CustomResult<String, errors::IntegrationError> {
    let custom_id = req.resource_common_data.connector_request_reference_id.clone();

    Ok(format!(
        "{}/api/invoices/custom_id/{custom_id}",
        self.connector_base_url_payments(req),
    ))
}
```

### PSync Response Handling

PSync uses the same response structure as authorize. The transformation logic is nearly identical:

```rust
impl<F> TryFrom<ResponseRouterData<CryptoPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<CryptoPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Same transformation logic as authorize response
        // Maps status and handles amount captured
    }
}
```

## Error Handling

### Crypto-Specific Error Response

```rust
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct CryptoErrorData {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct CryptoErrorResponse {
    pub error: CryptoErrorData,
}
```

### Error Response Mapping

```rust
fn build_error_response(
    &self,
    res: Response,
    event_builder: Option<&mut events::Event>,
) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
    let response: CryptoErrorResponse = res
        .response
        .parse_struct("CryptoErrorResponse")
        .change_context(errors::ConnectorResponseTransformationError::ResponseDeserializationFailed { context: Default::default() })?;

    with_error_response_body!(event_builder, response);

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: response.error.code,
        message: response.error.message,
        reason: response.error.reason,
        attempt_status: None,
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
    })
}
```

## Implementation Templates

### Complete Connector Implementation

```rust
// File: crates/integrations/connector-integration/src/connectors/cryptopay.rs

pub mod transformers;

use common_enums::CurrencyUnit;
use transformers::{
    CryptopayPaymentsRequest, CryptopayPaymentsResponse,
    CryptopayPaymentsResponse as CryptopayPaymentsSyncResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
    },
    payment_method_data::PaymentMethodDataTypes,
    types::Connectors,
};

use common_utils::{
    crypto::{GenerateDigest, SignMessage},
    errors::CustomResult,
    ext_traits::ByteSliceExt,
    request::Method,
};

use domain_types::{
    router_data::ConnectorAuthType,
    router_data_v2::RouterDataV2,
};

use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2,
    connector_types, verification::SourceVerification,
};

use hyperswitch_masking::{Maskable, PeekInterface};

use error_stack::ResultExt;
use serde::Serialize;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const DATE: &str = "Date";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Cryptopay<T>
{
    fn id(&self) -> &'static str {
        "cryptopay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base  // StringMajorUnit
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = cryptopay::CryptopayAuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.api_key.peek().to_owned().into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.cryptopay.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorResponseTransformationError> {
        // Error handling implementation
    }
}

// Trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Cryptopay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Cryptopay<T>
{
}

// Amount converter
macros::create_amount_converter_wrapper!(
    connector_name: Cryptopay,
    amount_type: StringMajorUnit
);

// Macro prerequisites
macros::create_all_prerequisites!(
    connector_name: Cryptopay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: CryptopayPaymentsRequest,
            response_body: CryptopayPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: CryptopayPaymentsSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        // Custom header building with HMAC signature
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            // HMAC-SHA1 signature implementation
            // ...
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.cryptopay.base_url
        }
    }
);

// Authorize implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cryptopay,
    curl_request: Json(CryptopayPaymentsRequest),
    curl_response: CryptopayResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/api/invoices", self.connector_base_url_payments(req)))
        }
    }
);

// PSync implementation
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cryptopay,
    curl_response: CryptopayPaymentResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let custom_id = req.resource_common_data.connector_request_reference_id.clone();
            Ok(format!(
                "{}/api/invoices/custom_id/{custom_id}",
                self.connector_base_url_payments(req),
            ))
        }
    }
);
```

## Common Pitfalls

### 1. Amount Unit Confusion

**❌ WRONG**: Using MinorUnit for crypto payments
```rust
// Incorrect - crypto connectors typically use StringMajorUnit
let amount = MinorUnit::new(10000);  // Wrong for crypto
```

**✅ RIGHT**: Use StringMajorUnit for fiat amount
```rust
// Correct - StringMajorUnit represents "100.00"
let amount = StringMajorUnit::new("100.00".to_string());
```

### 2. Missing Network Field

**❌ WRONG**: Ignoring the optional network field
```rust
// Missing network can cause payment failures on some blockchains
let request = CryptoRequest {
    pay_currency: "USDC".to_string(),
    // network is required for USDC (ERC-20 vs TRC-20)
    ..Default::default()
};
```

**✅ RIGHT**: Include network when available
```rust
let request = CryptoRequest {
    pay_currency: crypto_data.get_pay_currency()?,
    network: crypto_data.network.clone(),  // Pass through if provided
    ..Default::default()
};
```

### 3. Hardcoded Status

**❌ WRONG**: Assuming payment is complete after redirect
```rust
// Never hardcode status!
let status = common_enums::AttemptStatus::Charged;  // WRONG!
```

**✅ RIGHT**: Always map from connector response
```rust
let status = common_enums::AttemptStatus::from(response.status);
```

### 4. Missing Webhook Handling

**❌ WRONG**: Not implementing webhooks
```rust
// Webhooks are required for crypto payments
impl IncomingWebhook for CryptoConnector {
    // Missing implementation
}
```

**✅ RIGHT**: Implement full webhook support
```rust
impl<T> connector_types::IncomingWebhook for CryptoConnector<T> {
    fn get_event_type(...) -> Result<EventType, ...> { ... }
    fn process_payment_webhook(...) -> Result<WebhookDetailsResponse, ...> { ... }
    fn verify_webhook_source(...) -> Result<bool, ...> { ... }
}
```

### 5. Not Handling Metadata Properly

**❌ WRONG**: Passing any metadata type
```rust
metadata: item.router_data.request.metadata.clone(),  // May not be object
```

**✅ RIGHT**: Ensure metadata is an object
```rust
// Cryptopay specifically requires object type metadata
metadata: item.router_data.request.get_metadata_as_object(),
```

## Testing Patterns

### Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_request_transformation() {
        let crypto_data = CryptoData {
            pay_currency: Some("BTC".to_string()),
            network: Some("mainnet".to_string()),
        };

        let router_data = create_test_router_data(
            PaymentMethodData::Crypto(crypto_data),
            MinorUnit::new(10000),
            common_enums::Currency::USD,
        );

        let connector_router_data = CryptopayRouterData::try_from((
            StringMajorUnit::new("100.00".to_string()),
            router_data,
        )).unwrap();

        let request = CryptopayPaymentsRequest::try_from(connector_router_data);

        assert!(request.is_ok());
        let req = request.unwrap();
        assert_eq!(req.pay_currency, "BTC");
        assert_eq!(req.network, Some("mainnet".to_string()));
        assert_eq!(req.price_amount.get_amount_as_string(), "100.00");
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(
            common_enums::AttemptStatus::from(CryptopayPaymentStatus::New),
            common_enums::AttemptStatus::AuthenticationPending
        );
        assert_eq!(
            common_enums::AttemptStatus::from(CryptopayPaymentStatus::Completed),
            common_enums::AttemptStatus::Charged
        );
        assert_eq!(
            common_enums::AttemptStatus::from(CryptopayPaymentStatus::Cancelled),
            common_enums::AttemptStatus::Failure
        );
    }

    #[test]
    fn test_missing_pay_currency() {
        let crypto_data = CryptoData {
            pay_currency: None,
            network: None,
        };

        let result = crypto_data.get_pay_currency();
        assert!(result.is_err());
    }
}
```

### Integration Test Pattern

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_authorize_flow_with_crypto() {
        let connector = Cryptopay::new();

        // Create test request with Crypto payment method
        let request_data = create_test_authorize_request_with_crypto();

        // Test headers include HMAC signature
        let headers = connector.get_headers(&request_data).await.unwrap();
        assert!(headers.iter().any(|(k, _)| k == "Authorization"));
        assert!(headers.iter().any(|(k, _)| k == "Date"));

        // Test URL construction
        let url = connector.get_url(&request_data).await.unwrap();
        assert!(url.contains("/api/invoices"));

        // Test request body contains crypto-specific fields
        let body = connector.get_request_body(&request_data).await.unwrap();
        // Verify JSON structure
    }
}
```

## Integration Checklist

### Pre-Implementation

- [ ] Review connector's API documentation for crypto endpoints
- [ ] Understand supported cryptocurrencies and networks
- [ ] Identify webhook signature mechanism
- [ ] Confirm amount format (StringMajorUnit typical)
- [ ] Review blockchain confirmation requirements

### Implementation

- [ ] **Main Connector File**
  - [ ] Implement `ConnectorCommon` trait
  - [ ] Set `get_currency_unit()` to `CurrencyUnit::Base`
  - [ ] Configure amount converter for `StringMajorUnit`
  - [ ] Implement custom header building (HMAC if required)

- [ ] **Transformers File**
  - [ ] Define `CryptoPaymentsRequest` with crypto-specific fields
  - [ ] Define `CryptoPaymentsResponse` with status mapping
  - [ ] Implement `TryFrom` for request transformation
  - [ ] Implement `TryFrom` for response transformation
  - [ ] Define status enum and `From` trait for mapping

- [ ] **Authorize Flow**
  - [ ] Handle `PaymentMethodData::Crypto` in match arm
  - [ ] Extract `pay_currency` using `get_pay_currency()`
  - [ ] Pass through `network` field if provided
  - [ ] Set redirect URLs for customer return
  - [ ] Return `RedirectForm` in response

- [ ] **PSync Flow**
  - [ ] Implement status polling endpoint
  - [ ] Use same response transformation as authorize
  - [ ] Handle amount captured for completed payments

- [ ] **Webhook Handling**
  - [ ] Implement `IncomingWebhook` trait
  - [ ] Configure signature verification
  - [ ] Map webhook events to `EventType`
  - [ ] Process payment webhooks

### Testing

- [ ] Unit tests for request/response transformation
- [ ] Status mapping tests for all statuses
- [ ] Webhook signature verification tests
- [ ] Error handling tests
- [ ] Integration tests with sandbox environment

### Post-Implementation

- [ ] Test with supported cryptocurrencies
- [ ] Verify webhook delivery and processing
- [ ] Test PSync polling behavior
- [ ] Document supported crypto types and networks
- [ ] Add monitoring for crypto payment flows

## Reference

### Related Patterns

- [pattern_authorize.md](./pattern_authorize.md) - Generic authorize flow patterns
- [pattern_authorize_wallet.md](./pattern_authorize_wallet.md) - Wallet payment patterns (similar redirect flow)
- [pattern_psync.md](./pattern_psync.md) - Payment sync patterns
- [pattern_IncomingWebhook_flow.md](./pattern_IncomingWebhook_flow.md) - Webhook handling patterns

### Cryptopay Reference Implementation

- **Main File**: `crates/integrations/connector-integration/src/connectors/cryptopay.rs`
- **Transformers**: `crates/integrations/connector-integration/src/connectors/cryptopay/transformers.rs`
- **Amount Converter**: Uses `StringMajorUnit` via `CryptopayAmountConvertor`
- **Auth**: HMAC-SHA1 signature with API key and secret
- **Endpoints**:
  - Authorize: `POST /api/invoices`
  - PSync: `GET /api/invoices/custom_id/{custom_id}`

### CryptoData Helper Methods

```rust
impl CryptoData {
    /// Gets the pay_currency field or returns MissingRequiredField error
    pub fn get_pay_currency(&self) -> Result<String, Error> {
        self.pay_currency
            .clone()
            .ok_or_else(missing_field_err("crypto_data.pay_currency"))
    }
}
```

---

**Document Version**: 1.0
**Last Updated**: 2026-02-19
**Applies to**: Grace-UCS Connector Integration
