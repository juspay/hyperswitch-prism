# Wallet Authorize Flow Pattern

**Payment Method Category**: Wallet-based payments

**Description**: Digital wallets enable customers to make payments using stored payment credentials or digital payment tokens. Wallet payments span multiple implementation patterns from encrypted token-based flows (Apple Pay, Google Pay) to redirect-based authentication (PayPal, AliPay) to specialized wallet integrations.

---

## Table of Contents

1. [Overview](#overview)
2. [Wallet Variants](#wallet-variants)
3. [Supported Connectors](#supported-connectors)
4. [Implementation Patterns](#implementation-patterns)
   - [Token-Based Wallet Pattern](#token-based-wallet-pattern)
   - [Redirect-Based Wallet Pattern](#redirect-based-wallet-pattern)
   - [Redirect Form Wallet Pattern](#redirect-form-wallet-pattern)
   - [Specialized Wallet Pattern](#specialized-wallet-pattern)
   - [Per-Wallet Regional Redirect Pattern](#per-wallet-regional-redirect-pattern)
5. [Request Patterns](#request-patterns)
6. [Response Patterns](#response-patterns)
7. [Implementation Templates](#implementation-templates)
8. [Common Pitfalls](#common-pitfalls)
9. [Testing Patterns](#testing-patterns)
10. [Integration Checklist](#integration-checklist)

---

## Overview

Wallet payments in the Grace-UCS system are represented by the `WalletData` enum in `payment_method_data.rs`. Wallets generally fall into five implementation categories:

1. **Token-Based Wallets**: Apple Pay, Google Pay, Samsung Pay, Paze - Use encrypted payment tokens
2. **Redirect Wallets**: PayPal, AliPay, WeChat Pay - Redirect customer to wallet provider
3. **SDK-Based Wallets**: PayPal SDK, Google Pay SDK, Apple Pay SDK - Use provider SDKs
4. **Specialized Wallets**: Mifinity - Require additional customer data (DOB, etc.)
5. **Per-Wallet Regional Redirect Wallets**: LazyPay, PhonePe, BillDesk, Cashfree, PayU, EaseBuzz - Individual variants routed through aggregator connectors (e.g., Razorpay) that map each variant to a wallet name string

### Key Characteristics

| Characteristic | Value |
|----------------|-------|
| Default Request Format | JSON |
| Amount Unit | Varies by connector (Minor/StringMinor) |
| Response Type | Sync or Async/Redirect |
| 3DS Support | Wallet-dependent |
| Webhook Support | Required for async flows |

---

## Wallet Variants

Complete list of `WalletData` variants from `payment_method_data.rs`:

| Wallet Variant | Data Structure | Common Usage |
|----------------|----------------|--------------|
| `AliPayQr` | `Box<AliPayQr>` | QR code payments |
| `AliPayRedirect` | `AliPayRedirection` | Redirect flow |
| `AliPayHkRedirect` | `AliPayHkRedirection` | Hong Kong redirect |
| `BluecodeRedirect` | `{}` | Redirect flow |
| `AmazonPayRedirect` | `Box<AmazonPayRedirectData>` | Amazon Pay redirect |
| `MomoRedirect` | `MomoRedirection` | Redirect flow |
| `KakaoPayRedirect` | `KakaoPayRedirection` | Redirect flow |
| `GoPayRedirect` | `GoPayRedirection` | Redirect flow |
| `GcashRedirect` | `GcashRedirection` | Redirect flow |
| `ApplePay` | `ApplePayWalletData` | Token-based |
| `ApplePayRedirect` | `Box<ApplePayRedirectData>` | Redirect flow |
| `ApplePayThirdPartySdk` | `Box<ApplePayThirdPartySdkData>` | SDK-based |
| `DanaRedirect` | `{}` | Redirect flow |
| `GooglePay` | `GooglePayWalletData` | Token-based |
| `GooglePayRedirect` | `Box<GooglePayRedirectData>` | Redirect flow |
| `GooglePayThirdPartySdk` | `Box<GooglePayThirdPartySdkData>` | SDK-based |
| `MbWayRedirect` | `Box<MbWayRedirection>` | Redirect flow |
| `MobilePayRedirect` | `Box<MobilePayRedirection>` | Redirect flow |
| `PaypalRedirect` | `PaypalRedirection` | Redirect flow |
| `PaypalSdk` | `PayPalWalletData` | SDK-based |
| `Paze` | `PazeWalletData` | Token-based |
| `SamsungPay` | `Box<SamsungPayWalletData>` | Token-based |
| `TwintRedirect` | `{}` | Redirect flow |
| `VippsRedirect` | `{}` | Redirect flow |
| `TouchNGoRedirect` | `Box<TouchNGoRedirection>` | Redirect flow |
| `WeChatPayRedirect` | `Box<WeChatPayRedirection>` | Redirect flow |
| `WeChatPayQr` | `Box<WeChatPayQr>` | QR code payments |
| `CashappQr` | `Box<CashappQr>` | QR code payments |
| `SwishQr` | `SwishQrData` | QR code payments |
| `Mifinity` | `MifinityData` | Specialized (requires DOB) |
| `RevolutPay` | `RevolutPayData` | Token-based |
| `LazyPayRedirect` | `LazyPayRedirectData` `{}` | Indian wallet redirect |
| `PhonePeRedirect` | `PhonePeRedirectData` `{}` | Indian wallet redirect |
| `BillDeskRedirect` | `BillDeskRedirectData` `{}` | Indian wallet redirect |
| `CashfreeRedirect` | `CashfreeRedirectData` `{}` | Indian wallet redirect |
| `PayURedirect` | `PayURedirectData` `{}` | Indian wallet redirect |
| `EaseBuzzRedirect` | `EaseBuzzRedirectData` `{}` | Indian wallet redirect |

---

## Supported Connectors

| Connector | Supported Wallets | Implementation Pattern |
|-----------|-------------------|------------------------|
| **Stripe** | Apple Pay, Google Pay, AliPay, WeChat Pay, Cash App, Amazon Pay, Revolut Pay | Token-based + Direct |
| **Adyen** | Apple Pay, Google Pay, PayPal, AliPay | Token-based + Redirect |
| **Cybersource** | Apple Pay, Google Pay, Samsung Pay, Paze | Token-based |
| **PayPal** | PayPal (Redirect/SDK) | Redirect + SDK |
| **Mifinity** | Mifinity only | Specialized |
| **Worldpay** | Apple Pay, Google Pay, PayPal | Token-based + Redirect |
| **Bluesnap** | Apple Pay, Google Pay, PayPal | Token-based + Redirect |
| **Razorpay** | LazyPay, PhonePe, BillDesk, Cashfree, PayU, EaseBuzz | Per-Wallet Regional Redirect |

---

## Implementation Patterns

### Token-Based Wallet Pattern

**Applies to**: Apple Pay, Google Pay, Samsung Pay, Paze

**Characteristics**:
- Request Format: JSON
- Uses encrypted payment tokens
- May support pre-decrypted data from vault
- Response: Synchronous
- Amount Unit: MinorUnit / StringMinorUnit

#### Implementation Template

```rust
// In transformers.rs - Request transformation for token-based wallets

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<...> for ConnectorPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ...) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
                // Apple Pay - may use decrypted token from vault
                WalletData::ApplePay(apple_pay_data) => {
                    let apple_pay_token = match item.router_data
                        .resource_common_data
                        .payment_method_token
                        .clone()
                    {
                        Some(PaymentMethodToken::ApplePayDecrypt(decrypt_data)) => {
                            // Use pre-decrypted data
                            decrypt_data.get_four_digit_expiry_year();
                            decrypt_data.application_primary_account_number;
                            // ... create request with decrypted data
                        }
                        _ => {
                            // Use encrypted token directly
                            apple_pay_data
                                .payment_data
                                .get_encrypted_apple_pay_payment_data_mandatory()?
                        }
                    };
                    Ok(Self { /* ... */ })
                }

                // Google Pay
                WalletData::GooglePay(gpay_data) => {
                    let gpay_token = gpay_data
                        .tokenization_data
                        .get_encrypted_google_pay_token()?;
                    // Transform to connector format
                    Ok(Self { /* ... */ })
                }

                // Samsung Pay
                WalletData::SamsungPay(samsung_pay_data) => {
                    let token_data = &samsung_pay_data.payment_credential.token_data;
                    // Transform token data to connector format
                    Ok(Self { /* ... */ })
                }

                // Paze
                WalletData::Paze(paze_data) => {
                    match item.router_data
                        .resource_common_data
                        .payment_method_token
                        .clone()
                    {
                        Some(PaymentMethodToken::PazeDecrypt(paze_decrypted)) => {
                            // Use decrypted Paze data
                            Ok(Self { /* ... */ })
                        }
                        _ => Err(IntegrationError::MissingRequiredField {
                            field_name: "paze_decrypted_data",
                        , context: Default::default() })?
                    }
                }

                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(, Default::default())
                ))
            },
            _ => Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(, Default::default())
            ))
        }
    }
}
```

#### Connector Examples

**Stripe** (crates/integrations/connector-integration/src/connectors/stripe/transformers.rs):
```rust
// Stripe handles Apple Pay with potential pre-decrypted data
WalletData::ApplePay(applepay_data) => {
    let payment_method_token = item.resource_common_data.payment_method_token.clone();
    // Uses either decrypted token data or encrypted payment data
}

// Google Pay uses encrypted token
WalletData::GooglePay(gpay_data) => {
    Ok(Self::try_from(gpay_data)?)  // Extracts and transforms token
}
```

**Cybersource** (crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs):
```rust
// Cybersource handles Apple Pay with decrypted token
WalletData::ApplePay(apple_pay_data) => {
    match item.router_data.resource_common_data.payment_method_token.clone() {
        Some(PaymentMethodToken::ApplePayDecrypt(decrypt_data)) => {
            // Use decrypted card data
        }
        None => {
            // Use encrypted token
        }
    }
}

// Samsung Pay with fluid data encoding
WalletData::SamsungPay(samsung_pay_data) => {
    let payment_information = get_samsung_pay_payment_information(&samsung_pay_data)?;
}
```

---

### Redirect-Based Wallet Pattern

**Applies to**: PayPal, AliPay, WeChat Pay, GoPay, Gcash, KakaoPay, Momo, etc.

**Characteristics**:
- Request Format: JSON
- Response Type: Async/Redirect
- Returns redirect URL for customer authentication
- Requires webhook for status updates
- Amount Unit: Varies by connector

#### Implementation Template

```rust
// Request transformation for redirect-based wallets

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<...> for ConnectorPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ...) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(ref wallet_data) => match wallet_data {
                // PayPal Redirect
                WalletData::PaypalRedirect(_) => {
                    let payment_source = Some(PaymentSourceItem::Paypal(
                        PaypalRedirectionRequest {
                            experience_context: ContextStruct {
                                return_url: item.router_data.request.complete_authorize_url.clone(),
                                cancel_url: item.router_data.request.complete_authorize_url.clone(),
                                user_action: Some(UserAction::PayNow),
                                shipping_preference: ShippingPreference::SetProvidedAddress,
                            },
                            // Vault configuration for future payments
                            attributes: match item.router_data.request.setup_future_usage {
                                Some(FutureUsage::OffSession) => Some(Attributes { /* ... */ }),
                                _ => None,
                            },
                        }
                    ));
                    Ok(Self { intent, purchase_units, payment_source })
                }

                // Other redirect wallets
                WalletData::AliPayRedirect(_) |
                WalletData::WeChatPayRedirect(_) |
                WalletData::GoPayRedirect(_) |
                WalletData::GcashRedirect(_) => {
                    // Configure redirect flow
                    Ok(Self { /* ... */ })
                }

                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(, Default::default())
                ))
            },
            _ => Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(, Default::default())
            ))
        }
    }
}

// Response transformation - returns redirect URL

impl<T> TryFrom<ResponseRouterData<ConnectorAuthResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let status = get_order_status(item.response.status, item.response.intent);
        let link = get_redirect_url(item.response.links)?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: Some(Box::new(RedirectForm::from((
                    link.ok_or(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?,
                    Method::Get,
                )))),
                mandate_reference: None,
                connector_metadata: Some(connector_meta),
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
```

#### Connector Examples

**PayPal** (crates/integrations/connector-integration/src/connectors/paypal/transformers.rs):
```rust
// PayPal supports both redirect and SDK flows
WalletData::PaypalRedirect(_) => {
    let payment_source = Some(PaymentSourceItem::Paypal(
        PaypalRedirectionRequest::PaypalRedirectionStruct(PaypalRedirectionStruct {
            experience_context: ContextStruct {
                return_url: item.router_data.request.complete_authorize_url.clone(),
                cancel_url: item.router_data.request.complete_authorize_url.clone(),
                shipping_preference: /* ... */,
                user_action: Some(UserAction::PayNow),
            },
            attributes: /* vault config */,
        })
    ));
}

WalletData::PaypalSdk(_) => {
    // SDK flow with minimal context
    let payment_source = Some(PaymentSourceItem::Paypal(
        PaypalRedirectionRequest::PaypalRedirectionStruct(PaypalRedirectionStruct {
            experience_context: ContextStruct {
                return_url: None,
                cancel_url: None,
                shipping_preference: ShippingPreference::GetFromFile,
                user_action: Some(UserAction::PayNow),
            },
            // ...
        })
    ));
}
```

---

### Redirect Form Wallet Pattern

**Applies to**: Mifinity, wallets requiring special form handling

**Characteristics**:
- Returns initialization token or form data
- Customer completes payment on wallet's hosted page
- Requires polling (PSync) for status updates
- May require additional customer data

#### Implementation Template

```rust
// Response with custom redirect form

impl<F, T> TryFrom<ResponseRouterData<WalletPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let payload = item.response.payload.first();
        match payload {
            Some(payload) => {
                Ok(Self {
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(payload.trace_id.clone()),
                        redirection_data: Some(Box::new(RedirectForm::Mifinity {
                            initialization_token: payload.initialization_token.expose(),
                        })),
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(payload.trace_id),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    resource_common_data: PaymentFlowData {
                        status: AttemptStatus::AuthenticationPending,
                        ..item.router_data.resource_common_data
                    },
                    ..item.router_data
                })
            }
            None => { /* handle empty response */ }
        }
    }
}
```

---

### Specialized Wallet Pattern

**Applies to**: Mifinity (requires date of birth)

**Characteristics**:
- Requires additional customer data beyond standard fields
- May need connector-specific metadata
- Custom request structure

#### Implementation Template

```rust
// Mifinity-specific wallet data
#[derive(Debug, Serialize, PartialEq)]
pub struct MifinityPaymentsRequest {
    money: Money,
    client: MifinityClient,
    address: MifinityAddress,
    validation_key: String,
    client_reference: CustomerId,
    trace_id: String,
    description: String,
    destination_account_number: Secret<String>,
    brand_id: Secret<String>,
    return_url: String,
    language_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MifinityClient {
    first_name: Secret<String>,
    last_name: Secret<String>,
    phone: Secret<String>,
    dialing_code: String,
    nationality: CountryAlpha2,
    email_address: Email,
    dob: Secret<Date>,  // Required for Mifinity
}

// Request transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<...> for MifinityPaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ...) -> Result<Self, Self::Error> {
        match item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::Mifinity(data) => {
                    let client = MifinityClient {
                        first_name: item.router_data.resource_common_data.get_billing_first_name()?,
                        last_name: item.router_data.resource_common_data.get_billing_last_name()?,
                        phone: phone_details.get_number()?,
                        dialing_code: phone_details.get_country_code()?,
                        nationality: billing_country,
                        email_address: item.router_data.resource_common_data.get_billing_email()?,
                        dob: data.date_of_birth.clone(),  // Extract from wallet data
                    };
                    // ...
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(, Default::default())
                ))
            },
            _ => Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(, Default::default())
            ))
        }
    }
}
```

---

### Per-Wallet Regional Redirect Pattern

**Applies to**: Indian wallet providers routed through aggregator connectors (Razorpay)

**Characteristics**:
- Request Format: Form-encoded or JSON
- Each wallet provider has its own `WalletData` variant (e.g., `PhonePeRedirect`, `LazyPayRedirect`)
- The aggregator connector maps each variant to a connector-specific wallet name string
- Response Type: Async/Redirect (customer completes payment on wallet provider's page)
- Data structs are empty `{}` -- the wallet identity is carried by the variant itself, not by inner fields

| Characteristic | Value |
|----------------|-------|
| Request Format | Form-encoded / JSON |
| Amount Unit | MinorUnit |
| Response Type | Redirect (AuthenticationPending) |
| Wallet Data | Empty struct -- variant name is the identifier |
| Connector Role | Aggregator (not the wallet itself) |

#### Why Per-Wallet Variants Instead of a Catch-All

Aggregator connectors like Razorpay need to know *which* wallet the customer selected so they can pass the correct wallet name string in their API request (e.g., `"wallet": "phonepe"`). A generic `RedirectWalletDebit` variant would lose this information. Per-wallet variants preserve it through the type system without requiring runtime string fields.

#### Implementation Template

```rust
// In transformers.rs -- map each WalletData variant to the connector's wallet name string

fn extract_payment_method_and_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    _customer_name: Option<String>,
) -> Result<(PaymentMethodType, PaymentMethodSpecificData<T>), errors::ConnectorError> {
    match payment_method_data {
        PaymentMethodData::Wallet(wallet_data) => {
            let wallet_name = match wallet_data {
                WalletData::LazyPayRedirect(_) => "lazypay",
                WalletData::PhonePeRedirect(_) => "phonepe",
                WalletData::BillDeskRedirect(_) => "billdesk",
                WalletData::CashfreeRedirect(_) => "cashfree",
                WalletData::PayURedirect(_) => "payu",
                WalletData::EaseBuzzRedirect(_) => "easebuzz",
                _ => return Err(errors::ConnectorError::NotImplemented(
                    "This wallet type is not supported".to_string(),
                )),
            };
            Ok((PaymentMethodType::Wallet, PaymentMethodSpecificData::Wallet(wallet_name.to_string())))
        },
        _ => Err(errors::ConnectorError::NotImplemented(
            "Only Wallet payment methods are supported".to_string(),
        )),
    }
}
```

#### Connector Examples

**Razorpay** (crates/integrations/connector-integration/src/connectors/razorpay/transformers.rs):
```rust
// Request struct includes method + wallet name
#[derive(Debug, Serialize)]
pub struct RazorpayPaymentRequest<T: ...> {
    pub amount: MinorUnit,
    pub currency: String,
    pub method: PaymentMethodType,       // "wallet"
    pub wallet: Option<String>,          // "phonepe", "lazypay", etc.
    pub card: Option<PaymentMethodSpecificData<T>>,
    // ... other fields
}

// Wallet name is mapped from the variant, then serialized into the request
let (method, payment_method_data) = extract_payment_method_and_data(
    &item.router_data.request.payment_method_data,
    item.router_data.request.customer_name.clone(),
)?;

let (card, wallet) = match payment_method_data {
    PaymentMethodSpecificData::Card(_) => (Some(payment_method_data), None),
    PaymentMethodSpecificData::Wallet(name) => (None, Some(name)),
};
```

**Razorpay Supported Payment Methods Registration** (crates/integrations/connector-integration/src/connectors/razorpay.rs):
```rust
// Register each wallet type individually in a loop
for pmt in [
    PaymentMethodType::LazyPay,
    PaymentMethodType::PhonePe,
    PaymentMethodType::BillDesk,
    PaymentMethodType::Cashfree,
    PaymentMethodType::PayU,
    PaymentMethodType::EaseBuzz,
] {
    razorpay_supported_payment_methods.add(
        PaymentMethod::Wallet,
        pmt,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: vec![CaptureMethod::Automatic],
            specific_features: None,
        },
    );
}
```

---

## Request Patterns

### Standard Token-Based Request

```rust
#[derive(Debug, Serialize)]
pub struct TokenWalletPaymentRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub payment_method: TokenPaymentMethod,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TokenPaymentMethod {
    ApplePay(ApplePayRequestData),
    GooglePay(GooglePayRequestData),
    SamsungPay(SamsungPayRequestData),
}

#[derive(Debug, Serialize)]
pub struct ApplePayRequestData {
    pub token: Secret<String>,
    pub card_network: String,
    pub card_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GooglePayRequestData {
    pub token: Secret<String>,
    pub card_network: String,
    pub card_details: String,
}
```

### Redirect-Based Request

```rust
#[derive(Debug, Serialize)]
pub struct RedirectWalletPaymentRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub return_url: String,
    pub cancel_url: String,
    pub wallet_type: WalletType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletType {
    Paypal,
    AliPay,
    WechatPay,
    // ... other wallets
}
```

---

## Response Patterns

### Synchronous Success Response (Token Wallets)

```rust
#[derive(Debug, Deserialize)]
pub struct TokenWalletResponse {
    pub id: String,
    pub status: WalletPaymentStatus,
    pub amount: Option<i64>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletPaymentStatus {
    Succeeded,
    Pending,
    Failed,
    RequiresAction,
}

impl From<WalletPaymentStatus> for AttemptStatus {
    fn from(status: WalletPaymentStatus) -> Self {
        match status {
            WalletPaymentStatus::Succeeded => Self::Charged,
            WalletPaymentStatus::Pending => Self::Pending,
            WalletPaymentStatus::Failed => Self::Failure,
            WalletPaymentStatus::RequiresAction => Self::AuthenticationPending,
        }
    }
}
```

### Redirect Response (Async Wallets)

```rust
#[derive(Debug, Deserialize)]
pub struct RedirectWalletResponse {
    pub id: String,
    pub status: String,
    pub links: Vec<WalletLink>,
}

#[derive(Debug, Deserialize)]
pub struct WalletLink {
    pub href: Option<Url>,
    pub rel: String,  // "payer-action", "self", etc.
}

// Extract redirect URL
fn get_redirect_url(links: Vec<WalletLink>) -> Option<Url> {
    links.iter()
        .find(|link| link.rel == "payer-action")
        .and_then(|link| link.href.clone())
}
```

---

## Implementation Templates

### Complete Token Wallet Implementation

```rust
// transformers.rs

#[derive(Debug, Serialize)]
pub struct ConnectorWalletRequest {
    pub amount: MinorUnit,
    pub currency: String,
    pub wallet_data: WalletRequestData,
}

#[derive(Debug, Serialize)]
#[serde(tag = "wallet_type", rename_all = "snake_case")]
pub enum WalletRequestData {
    ApplePay { token: Secret<String> },
    GooglePay { token: Secret<String> },
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<...> for ConnectorWalletRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(item: ...) -> Result<Self, Self::Error> {
        let amount = item.amount;
        let currency = item.router_data.request.currency.to_string();

        let wallet_data = match &item.router_data.request.payment_method_data {
            PaymentMethodData::Wallet(wallet_data) => match wallet_data {
                WalletData::ApplePay(data) => {
                    let token = data.get_wallet_token()?;
                    WalletRequestData::ApplePay { token }
                }
                WalletData::GooglePay(data) => {
                    let token = Secret::new(
                        data.tokenization_data
                            .get_encrypted_google_pay_token()?
                    );
                    WalletRequestData::GooglePay { token }
                }
                _ => Err(IntegrationError::NotImplemented(
                    "Wallet not supported".to_string(, Default::default())
                ))?
            },
            _ => Err(IntegrationError::NotImplemented(
                "Payment method not supported".to_string(, Default::default())
            ))?
        };

        Ok(Self {
            amount,
            currency,
            wallet_data,
        })
    }
}

// Response transformation
#[derive(Debug, Deserialize)]
pub struct ConnectorWalletResponse {
    pub transaction_id: String,
    pub status: String,
}

impl<T> TryFrom<ResponseRouterData<ConnectorWalletResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        let status = map_wallet_status(&item.response.status)?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.transaction_id.clone()
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.transaction_id),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

fn map_wallet_status(status: &str) -> Result<AttemptStatus, IntegrationError> {
    match status {
        "succeeded" | "completed" => Ok(AttemptStatus::Charged),
        "pending" => Ok(AttemptStatus::Pending),
        "failed" => Ok(AttemptStatus::Failure),
        _ => Err(ConnectorError::ResponseDeserializationFailed { context: Default::default() })
    }
}
```

---

## Common Pitfalls

### 1. Token Extraction Errors

**Problem**: Incorrectly extracting wallet tokens leads to authentication failures.

**Solution**:
```rust
// CORRECT: Use proper token extraction methods
WalletData::ApplePay(data) => {
    let token = data.get_applepay_decoded_payment_data()?;
}

WalletData::GooglePay(data) => {
    let token = data.tokenization_data
        .get_encrypted_google_pay_token()?;
}
```

### 2. Missing Pre-Decrypted Token Handling

**Problem**: Not handling vault-provided decrypted data for Apple Pay.

**Solution**:
```rust
// Always check for pre-decrypted token first
let apple_pay_data = match item.resource_common_data.payment_method_token {
    Some(PaymentMethodToken::ApplePayDecrypt(decrypt_data)) => {
        // Use decrypted data
    }
    _ => {
        // Fall back to encrypted data
    }
};
```

### 3. Incorrect Status Mapping for Redirect Wallets

**Problem**: Hardcoding status as `Charged` for redirect flows.

**Solution**:
```rust
// CORRECT: Map from connector response, use Pending for redirects
let status = match response.status {
    "PAYER_ACTION_REQUIRED" => AttemptStatus::AuthenticationPending,
    "COMPLETED" => AttemptStatus::Charged,
    "PENDING" => AttemptStatus::Pending,
    _ => AttemptStatus::Failure,
};
```

### 4. Missing Return URL Configuration

**Problem**: Not providing return URLs for redirect wallets.

**Solution**:
```rust
// Always include return/cancel URLs
experience_context: ContextStruct {
    return_url: item.request.complete_authorize_url.clone(),
    cancel_url: item.request.complete_authorize_url.clone(),
    // ...
}
```

### 5. Samsung Pay Fluid Data Encoding

**Problem**: Incorrect encoding of Samsung Pay token data.

**Solution**:
```rust
// Correctly encode Samsung Pay data as base64 fluid data
let fluid_data_value = SamsungPayFluidDataValue {
    public_key_hash: /* from JWT header */,
    version: samsung_pay_token_data.version.clone(),
    data: Secret::new(BASE64_ENGINE.encode(samsung_pay_token_data.data.peek())),
};
let fluid_data_str = serde_json::to_string(&fluid_data_value)?;
let encoded = BASE64_ENGINE.encode(fluid_data_str);
```

### 6. Missing Exhaustive Match Arm Updates

**Problem**: Adding a new `WalletData` variant (e.g., `PhonePeRedirect`) without updating the exhaustive `match` arms in all existing connectors. The Rust compiler will catch this, but it results in a large number of compilation errors across 13+ connector transformer files.

**Solution**:
```rust
// Every connector that matches on WalletData must include new variants
// in its unsupported/catch-all arm. Example from adyen/transformers.rs:
WalletData::LazyPayRedirect(_)
| WalletData::PhonePeRedirect(_)
| WalletData::BillDeskRedirect(_)
| WalletData::CashfreeRedirect(_)
| WalletData::PayURedirect(_)
| WalletData::EaseBuzzRedirect(_) => Err(errors::ConnectorError::NotImplemented(
    "payment_method".into(),
))?,
```

**Rule**: When adding a new `WalletData` variant, update the catch-all match arm in **every** existing connector's transformers that matches on `WalletData`. Search for existing variants (e.g., `WalletData::Wero`) to find all locations.

### 7. Using Catch-All Aggregator Variants Instead of Per-Wallet Variants

**Problem**: Creating a single generic variant like `DirectWalletDebit(String)` or `RazorpayWalletRedirect` for aggregator connectors. This loses type safety and prevents the compiler from enforcing correct wallet-to-connector mappings.

**Solution**: Create individual per-wallet variants (e.g., `PhonePeRedirect`, `LazyPayRedirect`) even if their data structs are empty. The variant name itself carries the wallet identity, which the aggregator connector maps to the correct API string. This was learned from Razorpay where a single `RazorpayWalletRedirect` variant was initially created and then replaced with per-wallet variants in a subsequent fix.

---

## Testing Patterns

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_pay_request_transformation() {
        let router_data = create_test_router_data_with_wallet(
            WalletData::ApplePay(ApplePayWalletData {
                payment_data: ApplePayPaymentData::Encrypted(
                    "test_token".to_string()
                ),
                payment_method: ApplepayPaymentMethod {
                    display_name: "Visa".to_string(),
                    network: "Visa".to_string(),
                    pm_type: "debit".to_string(),
                },
                transaction_identifier: "txn_123".to_string(),
            })
        );

        let request = ConnectorWalletRequest::try_from(router_data);
        assert!(request.is_ok());

        let req = request.unwrap();
        assert_eq!(req.currency, "USD");
        assert!(matches!(req.wallet_data, WalletRequestData::ApplePay { .. }));
    }

    #[test]
    fn test_google_pay_request_transformation() {
        let gpay_wallet_data = GooglePayWalletData {
            pm_type: "CARD".to_string(),
            description: "Visa •••• 1234".to_string(),
            info: GooglePayPaymentMethodInfo {
                card_network: "VISA".to_string(),
                card_details: "1234".to_string(),
                assurance_details: None,
            },
            tokenization_data: GpayTokenizationData::Encrypted(
                GpayEcryptedTokenizationData {
                    token_type: "PAYMENT_GATEWAY".to_string(),
                    token: "encrypted_token".to_string(),
                }
            ),
        };

        let request = ConnectorWalletRequest::try_from(
            create_test_router_data_with_wallet(WalletData::GooglePay(gpay_wallet_data))
        );
        assert!(request.is_ok());
    }

    #[test]
    fn test_wallet_status_mapping() {
        assert_eq!(
            map_wallet_status("succeeded").unwrap(),
            AttemptStatus::Charged
        );
        assert_eq!(
            map_wallet_status("pending").unwrap(),
            AttemptStatus::Pending
        );
        assert_eq!(
            map_wallet_status("failed").unwrap(),
            AttemptStatus::Failure
        );
    }
}
```

### Integration Test Template

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_apple_pay_authorize_flow() {
        let connector = TestConnector::new();
        let authorize_data = create_apple_pay_authorize_data();

        // Test headers
        let headers = connector.get_headers(&authorize_data).unwrap();
        assert!(headers.iter().any(|(k, _)| k == "Content-Type"));

        // Test URL
        let url = connector.get_url(&authorize_data).unwrap();
        assert!(url.contains("/payments"));

        // Test request body
        let body = connector.get_request_body(&authorize_data).unwrap();
        assert!(body.is_some());
    }

    #[tokio::test]
    async fn test_redirect_wallet_response_handling() {
        let response = RedirectWalletResponse {
            id: "order_123".to_string(),
            status: "PAYER_ACTION_REQUIRED".to_string(),
            links: vec![
                WalletLink {
                    href: Some("https://wallet.com/pay".parse().unwrap()),
                    rel: "payer-action".to_string(),
                }
            ],
        };

        let router_data = create_test_router_data();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data,
            http_code: 200,
        });

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.resource_common_data.status, AttemptStatus::AuthenticationPending);
        assert!(data.response.unwrap().redirection_data.is_some());
    }
}
```

---

## Integration Checklist

### Pre-Implementation

- [ ] Identify wallet types supported by connector
- [ ] Determine if connector uses:
  - [ ] Token-based flow (Apple Pay, Google Pay, Samsung Pay)
  - [ ] Redirect flow (PayPal, AliPay, WeChat Pay)
  - [ ] SDK flow (PayPal SDK, Google Pay SDK)
  - [ ] Specialized flow (Mifinity)
  - [ ] Per-wallet regional redirect flow (Razorpay-style aggregator mapping per-wallet variants to wallet name strings)
- [ ] Check for pre-decrypted token support (Apple Pay, Paze)
- [ ] Understand connector's token format requirements
- [ ] Verify webhook requirements for async flows

### Implementation

- [ ] Implement `TryFrom` for request transformation
- [ ] Handle all wallet variants with proper error messages
- [ ] Implement `TryFrom` for response transformation
- [ ] Map connector statuses to standard `AttemptStatus`
- [ ] Handle redirect URLs for async wallets
- [ ] Support vault/payment method storage if applicable
- [ ] Implement proper token extraction for each wallet type
- [ ] When adding new `WalletData` variants, update exhaustive match arms in **all** existing connector transformers

### Testing

- [ ] Unit tests for each wallet type
- [ ] Test token extraction methods
- [ ] Test status mapping
- [ ] Test error handling
- [ ] Integration tests with sandbox credentials
- [ ] Test redirect URL extraction
- [ ] Test webhook handling

### Validation

- [ ] All wallet variants return proper error for unsupported types
- [ ] Token extraction doesn't expose sensitive data in logs
- [ ] Status mapping covers all connector status values
- [ ] Redirect URLs are properly formed
- [ ] Webhook signatures are validated (if applicable)

---

## Cross-References

- [pattern_authorize.md](./pattern_authorize.md) - Base authorize flow patterns
- [payment_method_data.rs](../../../../crates/types-traits/domain_types/src/payment_method_data.rs) - Wallet data structures
- [utility_functions_reference.md](./utility_functions_reference.md) - Common utility functions

---

## Appendix: Wallet Data Structures Reference

### ApplePayWalletData

```rust
pub struct ApplePayWalletData {
    pub payment_data: ApplePayPaymentData,
    pub payment_method: ApplepayPaymentMethod,
    pub transaction_identifier: String,
}

pub enum ApplePayPaymentData {
    Encrypted(String),
    Decrypted(ApplePayPredecryptData),
}

pub struct ApplePayPredecryptData {
    pub application_primary_account_number: cards::CardNumber,
    pub application_expiration_month: Secret<String>,
    pub application_expiration_year: Secret<String>,
    pub payment_data: ApplePayCryptogramData,
}
```

### GooglePayWalletData

```rust
pub struct GooglePayWalletData {
    #[serde(rename = "type")]
    pub pm_type: String,
    pub description: String,
    pub info: GooglePayPaymentMethodInfo,
    pub tokenization_data: GpayTokenizationData,
}

pub enum GpayTokenizationData {
    Decrypted(GPayPredecryptData),
    Encrypted(GpayEcryptedTokenizationData),
}

pub struct GpayEcryptedTokenizationData {
    #[serde(rename = "type")]
    pub token_type: String,
    pub token: String,
}
```

### SamsungPayWalletData

```rust
pub struct SamsungPayWalletData {
    pub payment_credential: SamsungPayWalletCredentials,
}

pub struct SamsungPayWalletCredentials {
    pub method: Option<String>,
    pub recurring_payment: Option<bool>,
    pub card_brand: SamsungPayCardBrand,
    pub dpan_last_four_digits: Option<String>,
    #[serde(rename = "card_last4digits")]
    pub card_last_four_digits: String,
    #[serde(rename = "3_d_s")]
    pub token_data: SamsungPayTokenData,
}

pub struct SamsungPayTokenData {
    #[serde(rename = "type")]
    pub three_ds_type: Option<String>,
    pub version: String,
    pub data: Secret<String>,
}
```

### MifinityData

```rust
pub struct MifinityData {
    pub date_of_birth: Secret<Date>,
    pub language_preference: Option<String>,
}
```

### Indian Wallet Redirect Data Structs

All Indian wallet redirect variants use empty data structs. The wallet identity is carried by the `WalletData` variant name, not by inner fields:

```rust
pub struct LazyPayRedirectData {}
pub struct PhonePeRedirectData {}
pub struct BillDeskRedirectData {}
pub struct CashfreeRedirectData {}
pub struct PayURedirectData {}
pub struct EaseBuzzRedirectData {}
```

These map to `PaymentMethodType` enums: `LazyPay`, `PhonePe`, `BillDesk`, `Cashfree`, `PayU`, `EaseBuzz`. Each has a corresponding proto message (e.g., `LazyPayRedirectWallet`, `PhonePeRedirectWallet`) and `ForeignTryFrom` conversion in `types.rs`.
