# Card Authorize Flow Pattern Guide

## Overview

This document provides comprehensive patterns for implementing Card payment authorization flows in Grace-UCS connectors. Card payments are the most common payment method and involve handling sensitive card data (PCI DSS compliance), 3D Secure authentication, and various authorization flows.

### Vault Proxy Card Details (Non-PCI Merchants)

In addition to the standard PCI-compliant card path, the UCS codebase exposes a parallel **proxy-card** code path for non-PCI merchants whose card data is aliased through an external vault (VGS, Basis Theory, Spreedly) before reaching the connector. This path is surfaced as a dedicated `ProxyCardDetails` proto message and consumed via the `VaultTokenHolder` PCI-holder marker type.

- **Proto message**: `ProxyCardDetails` at `crates/types-traits/grpc-api-types/proto/payment_methods.proto:240`. It has the same shape as `CardDetails` (number, expiry, CVC, holder name, metadata) but flows through the proxy endpoints rather than the raw-card authorize endpoints.
- **Domain dispatch**: `PaymentMethodDataAction::CardProxy(grpc_api_types::payments::ProxyCardDetails)` at `crates/types-traits/domain_types/src/types.rs:2392`. The `From` impl at `crates/types-traits/domain_types/src/types.rs:2400` converts the gRPC `PaymentMethod::CardProxy` arm into this variant. The `into_default_pci_payment_method_data` helper at `crates/types-traits/domain_types/src/types.rs:2449` explicitly rejects `CardProxy` with `"CardProxy not supported in this flow; use the proxy endpoint"`, enforcing that proxy-card payloads must be routed through the `PaymentServiceProxyAuthorizeRequest` flow (proto at `crates/types-traits/grpc-api-types/proto/payment.proto:4376-4410`) rather than the standard authorize flow.
- **Vault token holder**: `VaultTokenHolder` marker struct at `crates/types-traits/domain_types/src/payment_method_data.rs:50` implements `PaymentMethodDataTypes` with `type Inner = Secret<String>` at `crates/types-traits/domain_types/src/payment_method_data.rs:85-96`. A `ForeignTryFrom<ProxyCardDetails> for Card<VaultTokenHolder>` conversion at `crates/types-traits/domain_types/src/types.rs:2876` materialises the proxy card into the domain `Card` type parameterised by the vault token holder.

**How this differs from `Card<DefaultPCIHolder>`**: `Card<DefaultPCIHolder>` carries a validated `cards::CardNumber` (type alias at `crates/types-traits/domain_types/src/payment_method_data.rs:66`) representing a raw PAN handled by PCI-compliant infrastructure. `Card<VaultTokenHolder>` carries a `Secret<String>` token alias (`crates/types-traits/domain_types/src/payment_method_data.rs:86`) that the external vault substitutes with the real card before the request reaches the processor; the UCS merchant therefore never sees the raw PAN.

**Consuming connectors at this SHA**: the `VaultTokenHolder` branch is wired into `authorizedotnet` (see `AuthorizedotnetRawCardNumber<VaultTokenHolder>` at `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:234`) and referenced by `multisafepay` (`crates/integrations/connector-integration/src/connectors/multisafepay/transformers.rs:376`). No dedicated `Card<ProxyPCIHolder>` type exists at the pinned SHA.

### What is Card Payment

Card payments involve processing transactions using credit/debit card details including:
- **Card Number**: Primary Account Number (PAN)
- **Expiry Date**: Month and year of card expiration
- **CVV/CVC**: Card verification value (security code)
- **Card Holder Name**: Name on the card
- **Card Network**: Visa, Mastercard, Amex, etc.

### Card Variants in Grace-UCS

Based on `crates/types-traits/domain_types/src/payment_method_data.rs`:

| Variant | Description | Use Case |
|---------|-------------|----------|
| `Card<DefaultPCIHolder>` | Standard card with raw PAN | Direct card processing |
| `CardToken<DefaultPCIHolder>` | Tokenized card reference | PCI-compliant tokenization |
| `NetworkToken` | Network token (DPAN) | Network tokenization (Apple Pay, Google Pay) |
| `CardDetailsForNetworkTransactionId` | Card with network transaction ID | Recurring payments with network reference |

---

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Supported Connectors](#supported-connectors)
3. [Pattern Categories](#pattern-categories)
   - [Standard JSON Pattern](#1-standard-json-pattern)
   - [Form-Encoded Pattern](#2-form-encoded-pattern)
   - [XML/SOAP Pattern](#3-xmlsoap-pattern)
   - [Redirect Pattern](#4-redirect-pattern)
   - [3D Secure Pattern](#5-3d-secure-pattern)
4. [Request Patterns](#request-patterns)
5. [Response Patterns](#response-patterns)
6. [Implementation Templates](#implementation-templates)
7. [Common Pitfalls](#common-pitfalls)
8. [Testing Patterns](#testing-patterns)

---

## Quick Reference

### Card Data Structure

```rust
// From crates/types-traits/domain_types/src/payment_method_data.rs
pub struct Card<CD: PCIHolder> {
    pub card_number: CD::CardNumberType,
    pub card_exp_month: Secret<String>,
    pub card_exp_year: Secret<String>,
    pub card_cvc: Secret<String>,
    pub card_issuer: Option<Secret<String>>,
    pub card_network: Option<common_enums::CardNetwork>,
    pub card_type: Option<common_enums::CardType>,
    pub card_issuing_country: Option<String>,
    pub bank_code: Option<Secret<String>>,
    pub nick_name: Option<Secret<String>>,
}
```

### Card Helper Methods

```rust
// Extract 2-digit expiry year
let year = card.get_card_expiry_year_2_digit()?.expose();

// Extract 2-digit expiry month
let month = card.get_card_expiry_month_2_digit()?.expose();

// Combine for YYMM format
let expiry_date = Secret::new(format!("{year}{month}"));
```

### Authorization Flow Types

| Flow Type | Description | Example Connectors |
|-----------|-------------|-------------------|
| **Sync Authorization** | Immediate success/failure response | Nuvei, Bank of America |
| **Async Authorization** | Pending status requiring PSync | Redsys, ACI |
| **3DS Challenge** | Requires customer authentication | Adyen, Stripe, Checkout |
| **Redirect Flow** | Customer redirected to issuer | Trustpay, Worldpay |

---

## Supported Connectors

| Connector | Request Format | Amount Unit | 3DS Support | Token Support |
|-----------|---------------|-------------|-------------|---------------|
| **Stripe** | FormUrlEncoded | MinorUnit | Yes | Yes |
| **Adyen** | JSON | MinorUnit | Yes | Yes |
| **Checkout** | JSON | MinorUnit | Yes | Yes |
| **Cybersource** | JSON | StringMajorUnit | Yes | Yes |
| **Bank of America** | JSON | StringMajorUnit | No | No |
| **Nuvei** | JSON | StringMajorUnit | Yes | No |
| **Redsys** | JSON/XML | StringMinorUnit | Yes | Limited |
| **Authorizedotnet** | JSON | StringMinorUnit | Yes | Yes |
| **Trustpay** | JSON | MinorUnit | Yes | No |
| **ACI** | FormUrlEncoded | MinorUnit | Limited | No |
| **Paysafe** | JSON | MinorUnit | Yes | Yes |
| **Barclaycard** | JSON | MinorUnit | Yes | No |
| **Bluesnap** | XML | MinorUnit | Yes | Yes |
| **Tsys** | JSON | MinorUnit | No | Limited |
| **Worldpay** | JSON/XML | MinorUnit | Yes | Yes |
| **Paybox** | FormUrlEncoded | MinorUnit | Yes | No |
| **HyperPG** | JSON | MinorUnit | Yes | No |
| **Getnet** | JSON | MinorUnit | Yes | No |
| **HiPay** | JSON | MinorUnit | Yes | Yes |
| **Trustpayments** | JSON | MinorUnit | No | No |
| **Loonio** | JSON | MinorUnit | Yes | No |
| **PineLabs Online** | JSON | MinorUnit | No | No |

**PineLabs Online** card authorization (PR #795): `PaymentMethodData::Card(_)` is mapped to the connector wire string `"CARD"` in `get_pinelabs_payment_method_string` at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:622`, and the card data is materialised into `PinelabsOnlineCardDetails` via `build_card_details` at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:662` (card branch of `build_payment_option` beginning at `crates/integrations/connector-integration/src/connectors/pinelabs_online/transformers.rs:662`).

---

## Pattern Categories

### 1. Standard JSON Pattern

**Applies to**: Adyen, Checkout, Nuvei, Bank of America, Cybersource, Authorizedotnet

**Characteristics**:
- Request Format: JSON
- Response Type: Sync/Async
- Amount Unit: MinorUnit or StringMajorUnit
- Content-Type: `application/json`

#### Implementation Template

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPaymentRequest<T: PaymentMethodDataTypes> {
    pub card: CardDetails<T>,
    pub amount: Amount,
    // ... other fields
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetails<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub security_code: Secret<String>,
    pub card_holder_name: Option<Secret<String>>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ConnectorRouterData<Authorize, PaymentsAuthorizeData<T>>>
    for CardPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ConnectorRouterData<Authorize, PaymentsAuthorizeData<T>>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let card_details = CardDetails {
                    card_number: card.card_number.clone(),
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    security_code: card.card_cvc.clone(),
                    card_holder_name: router_data.request.get_billing_full_name(),
                };

                Ok(Self {
                    card: card_details,
                    amount: Amount {
                        value: item.amount,
                        currency: router_data.request.currency,
                    },
                })
            }
            _ => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message("connector_name", Default::default())
            ).into()),
        }
    }
}
```

#### Connector Example: Nuvei

```rust
// From crates/integrations/connector-integration/src/connectors/nuvei/transformers.rs

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCard<T: PaymentMethodDataTypes> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<NuveiRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>>
    for NuveiPaymentRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(item: NuveiRouterData<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, T>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => {
                let card_holder_name = router_data
                    .resource_common_data
                    .get_billing_full_name()
                    .unwrap_or(Secret::new("Unknown".to_string()));

                let payment_option = NuveiPaymentOption {
                    card: NuveiCard {
                        card_number: card.card_number.clone(),
                        card_holder_name,
                        expiration_month: card.card_exp_month.clone(),
                        expiration_year: card.card_exp_year.clone(),
                        cvv: card.card_cvc.clone(),
                    },
                };

                // ... build rest of request
            }
            _ => Err(errors::IntegrationError::NotImplemented(
                domain_types::utils::get_unimplemented_payment_method_error_message("Nuvei", Default::default())
            ).into()),
        }
    }
}
```

---

### 2. Form-Encoded Pattern

**Applies to**: Stripe, ACI, Paybox

**Characteristics**:
- Request Format: Form URL Encoded (`application/x-www-form-urlencoded`)
- Response Type: Sync/Redirect
- Amount Unit: MinorUnit
- Uses `serde_url_params` for serialization

#### Implementation Template

```rust
#[derive(Debug, Serialize)]
pub struct FormEncodedCardRequest {
    #[serde(rename = "card[number]")]
    pub card_number: String,
    #[serde(rename = "card[exp_month]")]
    pub exp_month: String,
    #[serde(rename = "card[exp_year]")]
    pub exp_year: String,
    #[serde(rename = "card[cvc]")]
    pub cvc: String,
    #[serde(rename = "amount")]
    pub amount: MinorUnit,
    #[serde(rename = "currency")]
    pub currency: String,
}
```

#### Connector Example: Stripe

```rust
// From crates/integrations/connector-integration/src/connectors/stripe/transformers.rs

impl<T: PaymentMethodDataTypes> TryFrom<&Card<T>> for StripeCardData {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(card: &Card<T>) -> Result<Self, Self::Error> {
        Ok(Self {
            number: card.card_number.clone(),
            exp_month: card.card_exp_month.clone(),
            exp_year: card.card_exp_year.clone(),
            cvc: card.card_cvc.clone(),
        })
    }
}

// Stripe connector uses FormUrlEncoded content type
impl<T: PaymentMethodDataTypes> ConnectorCommon for Stripe<T> {
    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }
    // ...
}
```

---

### 3. XML/SOAP Pattern

**Applies to**: Redsys, Worldpay XML, Bluesnap

**Characteristics**:
- Request Format: XML/SOAP
- Response Type: Async
- Amount Unit: Minor or StringMinorUnit
- Content-Type: `text/xml` or `application/xml`

#### Implementation Template

```rust
#[derive(Debug, Serialize)]
#[serde(rename = "Card")]
pub struct XmlCardData {
    #[serde(rename = "Number")]
    pub number: String,
    #[serde(rename = "ExpiryDate")]
    pub expiry_date: String,
    #[serde(rename = "CVV")]
    pub cvv: String,
}

// For SOAP envelopes
#[derive(Debug, Serialize)]
#[serde(rename = "soap:Envelope")]
pub struct SoapRequest<T> {
    #[serde(rename = "soap:Body")]
    pub body: T,
}
```

#### Connector Example: Redsys

```rust
// From crates/integrations/connector-integration/src/connectors/redsys/transformers.rs

pub const DS_VERSION: &str = "0.0";
pub const SIGNATURE_VERSION: &str = "HMAC_SHA256_V1";

/// Signed transaction envelope sent to Redsys
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedsysTransaction {
    #[serde(rename = "Ds_SignatureVersion")]
    pub ds_signature_version: String,
    #[serde(rename = "Ds_MerchantParameters")]
    pub ds_merchant_parameters: Secret<String>,
    #[serde(rename = "Ds_Signature")]
    pub ds_signature: Secret<String>,
}

impl TryFrom<&Option<PaymentMethodData<T>>> for requests::RedsysCardData<T>
where T: PaymentMethodDataTypes,
{
    type Error = Error;
    fn try_from(payment_method_data: &Option<PaymentMethodData<T>>) -> Result<Self, Self::Error> {
        match payment_method_data {
            Some(PaymentMethodData::Card(card)) => {
                let year = card.get_card_expiry_year_2_digit()?.expose();
                let month = card.get_card_expiry_month_2_digit()?.expose();
                let expiry_date = Secret::new(format!("{year}{month}"));
                Ok(Self {
                    card_number: card.card_number.clone(),
                    cvv2: card.card_cvc.clone(),
                    expiry_date,
                })
            }
            _ => Err(errors::IntegrationError::NotImplemented(
                domain_types::utils::get_unimplemented_payment_method_error_message("redsys", Default::default()),
            ).into()),
        }
    }
}
```

---

### 4. Redirect Pattern

**Applies to**: Adyen (some card variants), Trustpay, Worldpay

**Characteristics**:
- Returns `RedirectForm` for customer redirection
- Used for 3DS1 or issuer-hosted authentication
- Requires webhook or redirect handling

#### Implementation Template

```rust
fn build_redirect_response(
    redirect_url: String,
    transaction_id: String,
) -> Result<PaymentsResponseData, Error> {
    let mut form_fields = std::collections::HashMap::new();
    form_fields.insert("transaction_id".to_string(), transaction_id);

    let redirect_form = RedirectForm::Form {
        endpoint: redirect_url,
        method: Method::Post,
        form_fields,
    };

    Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(transaction_id),
        redirection_data: Some(Box::new(redirect_form)),
        mandate_reference: None,
        connector_metadata: None,
        network_txn_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code: 200,
    })
}
```

#### Connector Example: Redsys 3DS

```rust
// From crates/integrations/connector-integration/src/connectors/redsys/transformers.rs

fn build_threeds_form(
    ds_emv3ds: &responses::RedsysEmv3DSResponseData,
) -> Result<router_response_types::RedirectForm, Error> {
    let creq = ds_emv3ds
        .creq
        .clone()
        .ok_or(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

    let endpoint = ds_emv3ds
        .acs_u_r_l
        .clone()
        .ok_or(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

    let mut form_fields = std::collections::HashMap::new();
    form_fields.insert("creq".to_string(), creq);

    Ok(router_response_types::RedirectForm::Form {
        endpoint,
        method: common_utils::request::Method::Post,
        form_fields,
    })
}
```

---

### 5. 3D Secure Pattern

**Applies to**: Most connectors supporting Card (Adyen, Stripe, Checkout, Cybersource, Redsys)

**Characteristics**:
- Supports 3DS1, 3DS2 (frictionless & challenge)
- Requires PreAuthenticate, Authenticate, PostAuthenticate flows
- Handles CReq/CRes for challenge flows

#### 3DS Flow Overview

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ PreAuth     │────▶│ 3DS Method   │────▶│ Authenticate│
│ (check)     │     │ (fingerprint)│     │ (challenge) │
└─────────────┘     └──────────────┘     └─────────────┘
                                                │
                                                ▼
                                        ┌──────────────┐
                                        │ PostAuth     │
                                        │ (complete)   │
                                        └──────────────┘
```

#### 3DS Data Structures

```rust
// From crates/types-traits/domain_types/src/router_request_types.rs

pub struct AuthenticationData {
    pub threeds_server_transaction_id: Option<String>,
    pub message_version: Option<SemanticVersion>,
    pub trans_status: Option<TransactionStatus>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub ucaf_collection_indicator: Option<Secret<String>>,
    pub ds_trans_id: Option<String>,
    pub acs_transaction_id: Option<String>,
    pub transaction_id: Option<String>,
    pub exemption_indicator: Option<String>,
    pub network_params: Option<NetworkTransactionReference>,
}

pub enum TransactionStatus {
    Success,                        // Y - Fully authenticated
    Failure,                        // N - Failed authentication
    NotVerified,                    // A - Attempted authentication
    VerificationNotPerformed,       // U - Unable to perform
    ChallengeRequired,              // C - Challenge required
    ChallengeRequiredDecoupledAuthentication, // D
    InformationOnly,                // I - Information only
    Rejected,                       // R - Rejected
}
```

#### Connector Example: Adyen

```rust
// From crates/integrations/connector-integration/src/connectors/adyen/transformers.rs

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenThreeDSData {
    #[serde(rename = "threeDS2RequestData")]
    pub three_ds2_request_data: ThreeDS2RequestData,
    pub three_ds2_in_response_to: Option<String>,
    pub three_ds2_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDS2RequestData {
    pub device_channel: String,  // "browser" or "app"
    pub notification_url: String,
    pub three_ds_comp_ind: String,  // "Y" or "N"
    // ... browser info fields
}
```

#### Connector Note: NMI — 3DS Completion Embedded in Authorize

Unlike the Adyen/Redsys pattern that routes 3DS through dedicated `PreAuthenticate`/`Authenticate`/`PostAuthenticate` flow-marker implementations, **NMI** handles 3DS **completion** inside its Card **Authorize** flow. The separate `Authenticate` and `PostAuthenticate` `ConnectorIntegrationV2` impls in `crates/integrations/connector-integration/src/connectors/nmi.rs:732` and `crates/integrations/connector-integration/src/connectors/nmi.rs:742` are empty stubs (pass-through) — they do not register request/response bodies via `macro_connector_implementation!`. NMI's `PreAuthenticate` impl at `crates/integrations/connector-integration/src/connectors/nmi.rs:583` is a Customer-Vault registration (`NmiVaultRequest`), not a 3DS enrolment check.

Instead, the 3DS CRes data arrives with the Authorize call via `PaymentsAuthorizeData.redirect_response` and is deserialised by `NmiRedirectResponseData` inside the Authorize `TryFrom` branch at `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:373` (3DS completion flow), which extracts and forwards the following 3DS fields into `NmiPaymentsRequest`:

| Field | NmiPaymentsRequest line | Source on redirect_response |
|-------|-------------------------|-----------------------------|
| `cardholder_auth` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:304` | `three_ds_data.card_holder_auth` at `nmi/transformers.rs:439` |
| `cavv` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:306` | `three_ds_data.cavv` at `nmi/transformers.rs:440` |
| `xid` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:308` | `three_ds_data.xid` at `nmi/transformers.rs:441` |
| `eci` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:310` | `three_ds_data.eci` at `nmi/transformers.rs:442` |
| `three_ds_version` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:314` | `three_ds_data.three_ds_version` at `nmi/transformers.rs:444` |
| `directory_server_id` | `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:316` | `three_ds_data.directory_server_id` at `nmi/transformers.rs:445` |

For non-3DS (or 3DS-not-yet-started) Authorize calls, those six fields are emitted as `None` at `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:584-590`, so the same `NmiPaymentsRequest` schema covers both single-message and 3DS-completion paths. The corresponding response-side 3DS echo fields (`cavv`, `xid`, `eci`, `three_ds_version`, `directory_server_id`) are declared on the response struct at `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:1350-1356`. Implementation PR: #760.

---

## Request Patterns

### Card Network Mapping

Most connectors require mapping card networks to their internal codes:

```rust
// From crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs

fn card_issuer_to_string(card_issuer: CardIssuer) -> String {
    match card_issuer {
        CardIssuer::AmericanExpress => "003",
        CardIssuer::Master => "002",
        CardIssuer::Maestro => "042",
        CardIssuer::Visa => "001",
        CardIssuer::Discover => "004",
        CardIssuer::DinersClub => "005",
        CardIssuer::CarteBlanche => "006",
        CardIssuer::JCB => "007",
        CardIssuer::CartesBancaires => "036",
        CardIssuer::UnionPay => "062",
    }.to_string()
}

// Adyen card brand mapping
impl TryFrom<&domain_utils::CardIssuer> for CardBrand {
    type Error = Error;
    fn try_from(card_issuer: &domain_utils::CardIssuer) -> Result<Self, Self::Error> {
        match card_issuer {
            domain_utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            domain_utils::CardIssuer::Master => Ok(Self::MC),
            domain_utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_utils::CardIssuer::Maestro => Ok(Self::Maestro),
            domain_utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_utils::CardIssuer::DinersClub => Ok(Self::Diners),
            domain_utils::CardIssuer::JCB => Ok(Self::Jcb),
            domain_utils::CardIssuer::CarteBlanche => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::CartesBancaires => Ok(Self::Cartebancaire),
            domain_utils::CardIssuer::UnionPay => Ok(Self::Cup),
        }
    }
}
```

### Expiry Date Formats

| Format | Description | Example |
|--------|-------------|---------|
| MM/YY | Standard display | 12/25 |
| YYYYMM | Some connectors | 202512 |
| YYMM | Redsys format | 2512 |
| YYYY-MM | ISO format | 2025-12 |

```rust
// Helper for YYMM format (e.g., Redsys)
let year = card.get_card_expiry_year_2_digit()?.expose();
let month = card.get_card_expiry_month_2_digit()?.expose();
let expiry_date = Secret::new(format!("{year}{month}"));

// Helper for MM/YY format
let exp_month = card.card_exp_month.peek();
let exp_year = card.card_exp_year.peek();
let expiry = format!("{exp_month}/{}", &exp_year[exp_year.len()-2..]);
```

---

## Response Patterns

### Status Mapping

```rust
// Common pattern for mapping connector status to AttemptStatus
fn map_connector_status(
    connector_status: &str,
    capture_method: Option<CaptureMethod>,
) -> Result<AttemptStatus, IntegrationError> {
    match connector_status {
        "approved" | "success" | "AUTHORIZED" => {
            match capture_method {
                Some(CaptureMethod::Automatic) | None => Ok(AttemptStatus::Charged),
                Some(CaptureMethod::Manual) => Ok(AttemptStatus::Authorized),
                _ => Err(IntegrationError::CaptureMethodNotSupported),
            }
        }
        "pending" | "PENDING" => Ok(AttemptStatus::Pending),
        "declined" | "failure" => Ok(AttemptStatus::Failure),
        "requires_action" => Ok(AttemptStatus::AuthenticationPending),
        _ => Err(ConnectorError::ResponseHandlingFailed),
    }
}
```

### Response Data Construction

```rust
// Standard response construction pattern
impl TryFrom<ResponseRouterData<ConnectorAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<ConnectorAuthorizeResponse, Self>) -> Result<Self, Self::Error> {
        let response = item.response;
        let status = map_connector_status(&response.status, item.router_data.request.capture_method)?;

        let payment_response_data = if is_payment_failure(status) {
            Err(ErrorResponse {
                code: response.error_code.unwrap_or_default(),
                message: response.error_message.unwrap_or_default(),
                reason: response.decline_reason,
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(response.transaction_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(response.transaction_id),
                redirection_data: response.redirect_url.map(|url| {
                    Box::new(RedirectForm::Form {
                        endpoint: url,
                        method: Method::Post,
                        form_fields: std::collections::HashMap::new(),
                    })
                }),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: response.network_transaction_id,
                connector_response_reference_id: Some(response.reference_id),
                incremental_authorization_allowed: Some(false),
                status_code: item.http_code,
            })
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data.clone()
            },
            response: payment_response_data,
            ..item.router_data.clone()
        })
    }
}
```

---

## Implementation Templates

### Complete Macro-Based Implementation

```rust
use macros;

pub struct MyConnector<T: PaymentMethodDataTypes> {
    _phantom: std::marker::PhantomData<T>,
}

macros::create_amount_converter_wrapper!(
    connector_name: MyConnector,
    amount_type: MinorUnit  // or StringMajorUnit, StringMinorUnit
);

macros::create_all_prerequisites!(
    connector_name: MyConnector,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: MyConnectorAuthorizeRequest,
            response_body: MyConnectorAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: MyConnectorCaptureRequest,
            response_body: MyConnectorCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: MyConnectorVoidRequest,
            response_body: MyConnectorVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: MyConnectorRefundRequest,
            response_body: MyConnectorRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: PSync,
            response_body: MyConnectorSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![
                ("Content-Type".to_string(), "application/json".to_string().into()),
                ("Authorization".to_string(), self.get_auth_header(&req.connector_auth_type)?),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.my_connector.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes> ConnectorCommon for MyConnector<T> {
    fn id(&self) -> &'static str {
        "my_connector"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.my_connector.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Implement authentication header logic
        todo!()
    }
}
```

### Manual Implementation (Non-Macro)

For connectors requiring custom logic, implement traits directly:

```rust
impl<T: PaymentMethodDataTypes> ConnectorIntegrationV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<T>,
    PaymentsResponseData,
> for MyConnector<T> {
    fn get_headers(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Custom header logic
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        // Custom URL logic
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    ) -> CustomResult<RequestContent, IntegrationError> {
        // Custom request body construction
    }

    fn handle_response(
        &self,
        data: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>, ConnectorError> {
        // Custom response handling
    }
}
```

---

## Common Pitfalls

### 1. Amount Unit Confusion

**Problem**: Using wrong amount unit (minor vs major)

**Solution**: Always check the connector's expected unit:

```rust
// For StringMajorUnit (e.g., Bank of America, Cybersource)
let amount = StringMajorUnit::from_minor_unit(item.amount, item.router_data.request.currency)?;

// For MinorUnit (most connectors)
let amount = item.amount;  // Already in MinorUnit
```

### 2. Missing Card Network Mapping

**Problem**: Not mapping card networks to connector-specific codes

**Solution**: Always include card network mapping:

```rust
let card_type = card
    .card_network
    .clone()
    .and_then(get_connector_card_type)
    .unwrap_or_else(|| {
        domain_types::utils::get_card_issuer(&card_number_string)
            .ok()
            .map(card_issuer_to_string)
    });
```

### 3. 3DS Response Handling

**Problem**: Not handling 3DS challenge responses correctly

**Solution**: Check for `AuthenticationPending` status and build redirect form:

```rust
match status {
    AttemptStatus::AuthenticationPending => {
        let redirect_form = build_threeds_form(&response.three_ds_data)?;
        redirection_data: Some(Box::new(redirect_form)),
    }
    _ => redirection_data: None,
}
```

### 4. Expiry Date Format Issues

**Problem**: Wrong expiry date format for connector

**Solution**: Use connector-specific formatting:

```rust
// For YYMM format
let year = card.get_card_expiry_year_2_digit()?.expose();
let month = card.get_card_expiry_month_2_digit()?.expose();
let expiry = format!("{year}{month}");

// For YYYY-MM format
let expiry = format!("{}-{}",
    card.card_exp_year.peek(),
    card.card_exp_month.peek()
);
```

### 5. CVV Handling for Tokenized Cards

**Problem**: Requiring CVV for tokenized transactions

**Solution**: Make CVV optional for token/network token payments:

```rust
#[derive(Debug, Serialize)]
pub struct CardDetails {
    pub number: String,
    pub exp_month: String,
    pub exp_year: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<String>,  // Optional for tokenized cards
}
```

---

## Testing Patterns

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_request_transformation() {
        let card = Card::<DefaultPCIHolder> {
            card_number: RawCardNumber::from_str("4111111111111111").unwrap(),
            card_exp_month: Secret::new("12".to_string()),
            card_exp_year: Secret::new("2025".to_string()),
            card_cvc: Secret::new("123".to_string()),
            card_issuer: None,
            card_network: Some(CardNetwork::Visa),
            card_type: Some(CardType::Credit),
            card_issuing_country: None,
            bank_code: None,
            nick_name: None,
        };

        let request = MyConnectorCardRequest::try_from(&card).unwrap();
        assert_eq!(request.expiry_date, "2512");
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(
            map_connector_status("approved", Some(CaptureMethod::Automatic)).unwrap(),
            AttemptStatus::Charged
        );
        assert_eq!(
            map_connector_status("approved", Some(CaptureMethod::Manual)).unwrap(),
            AttemptStatus::Authorized
        );
    }
}
```

### Integration Test Scenarios

| Scenario | Test Case | Expected Result |
|----------|-----------|-----------------|
| **Successful Authorization** | Valid Visa card | `AttemptStatus::Charged` |
| **Manual Capture** | Valid card with `CaptureMethod::Manual` | `AttemptStatus::Authorized` |
| **Declined Card** | Card with insufficient funds | `AttemptStatus::Failure` |
| **Expired Card** | Card with past expiry date | `AttemptStatus::Failure` |
| **Invalid CVV** | Card with wrong CVV | `AttemptStatus::Failure` |
| **3DS Frictionless** | 3DS2 card with low risk | `AttemptStatus::Charged` |
| **3DS Challenge** | 3DS2 card requiring challenge | `AttemptStatus::AuthenticationPending` + redirect |
| **Network Token** | Apple Pay token | Successful authorization |
| **Card Token** | Stored card token | Successful authorization |

---

## Cross-References

- [pattern_authorize.md](./pattern_authorize.md) - General authorize flow patterns
- [Authorize Flow Documentation](../../flows/authorize.md) - Authorize flow implementation guide
- [3DS Flow Documentation](../../flows/threeds.md) - 3D Secure implementation guide
- [Capture Flow Documentation](../../flows/capture.md) - Capture flow for manual capture
- [Refund Flow Documentation](../../flows/refund.md) - Refund implementation patterns

---

## Implementation Checklist

When implementing Card payments for a new connector:

- [ ] Identify connector's request format (JSON, Form, XML)
- [ ] Identify connector's expected amount unit
- [ ] Map card networks to connector codes
- [ ] Implement card data transformation
- [ ] Handle expiry date format correctly
- [ ] Implement status mapping
- [ ] Support capture method (automatic vs manual)
- [ ] Handle 3DS responses if supported
- [ ] Handle redirect responses if applicable
- [ ] Implement error response handling
- [ ] Add comprehensive unit tests
- [ ] Test with real card numbers in sandbox
- [ ] Document any connector-specific quirks

---

## Change Log

| Date | Version | Pinned SHA | Change |
|------|---------|------------|--------|
| 2026-04-20 | 1.2.0 | `60540470cf84a350cc02b0d41565e5766437eb95` | Final-polish citation pass. Added **Connector Note: NMI — 3DS Completion Embedded in Authorize** subsection to the 3DS pattern category, documenting that NMI's `Authenticate`/`PostAuthenticate` `ConnectorIntegrationV2` impls at `crates/integrations/connector-integration/src/connectors/nmi.rs:732` and `crates/integrations/connector-integration/src/connectors/nmi.rs:742` are empty stubs, and that 3DS completion fields (`cardholder_auth`, `cavv`, `xid`, `eci`, `three_ds_version`, `directory_server_id`) are instead driven into `NmiPaymentsRequest` inside the Authorize `TryFrom` branch spanning `crates/integrations/connector-integration/src/connectors/nmi/transformers.rs:298-445` (per-field citations included; response-echo fields at `nmi/transformers.rs:1350-1356`) -- PR #760. |
| 2026-04-20 | 1.1.0 | `60540470cf84a350cc02b0d41565e5766437eb95` | Added document header metadata block. Added **Vault Proxy Card Details** subsection to Overview, documenting the `ProxyCardDetails` proto message (`crates/types-traits/grpc-api-types/proto/payment_methods.proto:240`), the `PaymentMethodDataAction::CardProxy` dispatch arm (`crates/types-traits/domain_types/src/types.rs:2392`), the proxy-only rejection at `crates/types-traits/domain_types/src/types.rs:2449`, and the `Card<VaultTokenHolder>` conversion path (`crates/types-traits/domain_types/src/types.rs:2876`) -- PR #801, commit `2dcbcf76f`. Added **PineLabs Online** row to the Supported Connectors table with `file:line` citation of `pinelabs_online/transformers.rs:622` (Card→"CARD" mapping) and `pinelabs_online/transformers.rs:662` (card-details builder) -- PR #795. |
| (prior) | 1.0.0 | (initial) | Initial authoring covering Standard JSON, Form-Encoded, XML/SOAP, Redirect, and 3DS patterns across the card-payment connector set. |
