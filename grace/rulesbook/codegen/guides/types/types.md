# UCS Type System Guide

Comprehensive guide to UCS connector-service type system, covering all payment flows and data structures.

## 🏗️ Core UCS Type Imports

```rust
// Essential UCS imports - use these in every connector
use domain_types::{
    // Flow types for all operations
    connector_flow::{
        Authorize, Capture, Void, Refund, PSync, RSync,
        CreateOrder, ServerSessionAuthenticationToken, SetupMandate, 
        DefendDispute, SubmitEvidence, Accept
    },
    
    // Request/Response types for all flows
    connector_types::{
        // Payment flow types
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentVoidData,
        PaymentsSyncData, PaymentsResponseData,
        
        // Refund flow types
        RefundsData, RefundSyncData, RefundsResponseData,
        
        // Advanced flow types
        PaymentCreateOrderData, PaymentCreateOrderResponse,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData,
        
        // Dispute types
        DisputeFlowData, DisputeResponseData,
        AcceptDisputeData, DisputeDefendData, SubmitEvidenceData,
        
        // Webhook types
        WebhookDetailsResponse, RefundWebhookDetailsResponse,
        EventType, EventContext, WebhookResourceReference, WebhookIntegrityCheck,

        // Per-domain flow data (the `ResourceCommonData` slot of RouterDataV2)
        PaymentFlowData, RefundFlowData, DisputeFlowData,

        // Common types
        ResponseId, RequestDetails, ConnectorSpecifications,
        SupportedPaymentMethodsExt, ConnectorWebhookSecrets,
    },
    
    // Enhanced router data
    router_data_v2::RouterDataV2,
    
    // Payment method data.
    // NOTE: these are the *type* names. `Wallet`, `BankTransfer`, `BuyNowPayLater`,
    // `Voucher`, `Crypto`, `GiftCard`, `BankRedirect` and `CardRedirect` are
    // `PaymentMethodData` VARIANTS, not importable types - importing them is E0432.
    // The types are the `*Data` structs below (and the PayLater variant's type is
    // `PayLaterData`, there is no `BuyNowPayLater` type).
    payment_method_data::{
        PaymentMethodData, PaymentMethodDataTypes, DefaultPCIHolder,
        Card, WalletData, BankTransferData, PayLaterData, VoucherData,
        CryptoData, GiftCardData, BankRedirectData, CardRedirectData
    },
    
    // Address and customer data
    payment_address::{Address, AddressDetails},
    
    // Error types: ConnectorError, IntegrationError, WebhookError and their
    // `*Context` structs all live here.
    errors,

    // Router data and auth.
    // NOTE: `RouterDataV2` no longer carries `connector_auth_type`. Auth is read
    // from `req.connector_config: ConnectorSpecificConfig` via a per-connector
    // variant. `ConnectorAuthType` still exists in this module but is NOT what a
    // connector matches on.
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_response_types::Response,
    
    // Utility types
    types::{
        self, Connectors, ConnectorInfo, FeatureStatus,
        PaymentMethodDetails, PaymentMethodSpecificFeatures,
        SupportedPaymentMethods, CardSpecificFeatures,
        PaymentMethodDataType
    },
    utils,
};

// Interface types
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, is_mandate_supported, ConnectorValidation},
    // Both of these are NON-GENERIC traits - one impl per connector, not one per flow.
    decode::BodyDecoding,
    verification::SourceVerification,
};

// Common utilities
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass,
    PaymentMethod, PaymentMethodType, Currency, CountryAlpha2
};
use common_utils::{
    errors::CustomResult,
    // `events::Event` is the event-builder type in every connector signature.
    // (`interfaces::events::connector_api_logs::ConnectorEvent` is a different,
    // server-side type - do NOT use it in `build_error_response`.)
    events,
    ext_traits::ByteSliceExt,
    pii::{Email, IpAddress, SecretSerdeValue},
    request::Method,
    // All five amount unit types - pick the one matching the vendor's wire format.
    types::{
        FloatMajorUnit, MinorUnit, StringMajorUnit, StringMinorUnit, StringTwoDecimalUnit,
    },
};

// Masking utilities
use hyperswitch_masking::{Mask, Maskable};

// Serialization
use serde::{Serialize, Deserialize};
```

## 🔄 Router Data Types

### Core Router Data Pattern
```rust
// UCS uses RouterDataV2 for all operations.
// RouterDataV2 takes FOUR type parameters:
//   RouterDataV2<Flow, ResourceCommonData, FlowSpecificRequest, FlowSpecificResponse>
// `ResourceCommonData` is the per-domain flow data: `PaymentFlowData`,
// `RefundFlowData`, `DisputeFlowData` (domain_types::connector_types) and
// `PayoutFlowData` (domain_types::payouts::payouts_types).
// Omitting it is E0107.
type AuthorizeRouterData<T> = RouterDataV2<
    Authorize,                   // Flow marker
    PaymentFlowData,             // ResourceCommonData
    PaymentsAuthorizeData<T>,    // Request data type (generic over the PCI holder)
    PaymentsResponseData         // Response data type
>;

type CaptureRouterData = RouterDataV2<
    Capture,
    PaymentFlowData,
    PaymentsCaptureData,
    PaymentsResponseData
>;

type VoidRouterData = RouterDataV2<
    Void,
    PaymentFlowData,
    PaymentVoidData,
    PaymentsResponseData
>;

type RefundRouterData = RouterDataV2<
    Refund,
    RefundFlowData,
    RefundsData,
    RefundsResponseData
>;

type SyncRouterData = RouterDataV2<
    PSync,
    PaymentFlowData,
    PaymentsSyncData,
    PaymentsResponseData
>;

type RefundSyncRouterData = RouterDataV2<
    RSync,
    RefundFlowData,
    RefundSyncData,
    RefundsResponseData
>;

// Advanced flow types
type CreateOrderRouterData = RouterDataV2<
    CreateOrder,
    PaymentFlowData,
    PaymentCreateOrderData,
    PaymentCreateOrderResponse
>;

type SetupMandateRouterData<T> = RouterDataV2<
    SetupMandate,
    PaymentFlowData,
    SetupMandateRequestData<T>,
    PaymentsResponseData
>;

// Dispute flow types
type DefendDisputeRouterData = RouterDataV2<
    DefendDispute,
    DisputeFlowData,
    DisputeDefendData,
    DisputeResponseData
>;

type AcceptDisputeRouterData = RouterDataV2<
    Accept,
    DisputeFlowData,
    AcceptDisputeData,
    DisputeResponseData
>;

type SubmitEvidenceRouterData = RouterDataV2<
    SubmitEvidence,
    DisputeFlowData,
    SubmitEvidenceData,
    DisputeResponseData
>;
```

### Fields on `RouterDataV2` (verify before you reach for one)

```rust
// crates/types-traits/domain_types/src/router_data_v2.rs
pub struct RouterDataV2<Flow, ResourceCommonData, FlowSpecificRequest, FlowSpecificResponse> {
    pub flow: PhantomData<Flow>,
    pub resource_common_data: ResourceCommonData,
    pub connector_config: ConnectorSpecificConfig, // <- auth lives HERE
    pub request: FlowSpecificRequest,
    pub response: Result<FlowSpecificResponse, ErrorResponse>,
}
```

There is **no** `connector_auth_type` field. It was removed on 2026-03-14.
Anything reading `req.connector_auth_type` will not compile.

## 💳 Payment Method Data Types

### Comprehensive Payment Method Handling
```rust
// Handle all payment method types
match payment_method_data {
    // Card payments - most common
    PaymentMethodData::Card(card_data) => {
        // card_data is `Card<T>` - real fields, verified against
        // crates/types-traits/domain_types/src/payment_method_data.rs:123
        // - card_number: RawCardNumber<T>          // NOT cards::CardNumber
        // - card_exp_month: Secret<String>         // NOT SecretSerdeValue
        // - card_exp_year: Secret<String>
        // - card_cvc: Secret<String>
        // - card_issuer: Option<String>
        // - card_network: Option<CardNetwork>
        // - card_type: Option<String>
        // - card_issuing_country: Option<String>
        // - bank_code: Option<String>
        // - nick_name: Option<Secret<String>>
        // - card_holder_name: Option<Secret<String>>
        // - co_badged_card_data: Option<CoBadgedCardData>
    }
    
    // Digital wallets
    PaymentMethodData::Wallet(wallet_data) => match wallet_data {
        WalletData::ApplePay(apple_pay) => {
            // apple_pay fields:
            // - payment_data: SecretSerdeValue (encrypted payment data)
            // - payment_method: ApplepayPaymentMethod
            // - transaction_identifier: Option<String>
        }
        
        WalletData::GooglePay(google_pay) => {
            // google_pay fields:
            // - type_: String (e.g., "CARD", "PAYPAL")
            // - description: String
            // - info: GooglePayPaymentMethodInfo
            // - tokenization_specification: Option<GooglePayTokenizationSpecification>
        }
        
        WalletData::PaypalRedirect(paypal) => {
            // paypal fields:
            // - email: Option<Email>
        }
        
        WalletData::SamsungPay(samsung_pay) => {
            // Samsung Pay encrypted payment data
        }
        
        WalletData::WeChatPayRedirect(wechat) => {
            // WeChat Pay specific data
        }
        
        WalletData::AliPayRedirect(alipay) => {
            // Alipay specific data
        }
        
        WalletData::MbWayRedirect(mbway) => {
            // MB Way (Portuguese wallet)
            // - telephone_number: SecretSerdeValue
        }
        
        WalletData::TouchNGoRedirect(touchngo) => {
            // Touch 'n Go (Malaysian wallet)
        }
        
        WalletData::GrabPayRedirect(grabpay) => {
            // GrabPay (Southeast Asian wallet)
        }
        
        // Add all wallet variants your connector supports
    }
    
    // Bank transfers
    PaymentMethodData::BankTransfer(bank_data) => match bank_data {
        BankTransferData::AchBankTransfer => {
            // ACH bank transfer (US)
            // Requires: account_number, routing_number, account_type
        }
        
        BankTransferData::SepaBankTransfer => {
            // SEPA bank transfer (Europe)
            // Requires: iban, bic (optional)
        }
        
        BankTransferData::BacsBankTransfer => {
            // BACS bank transfer (UK)
            // Requires: account_number, sort_code
        }
        
        BankTransferData::MultibancoBankTransfer => {
            // Multibanco (Portugal)
        }
        
        BankTransferData::PermataBankTransfer => {
            // Permata Bank (Indonesia)
        }
        
        BankTransferData::BcaBankTransfer => {
            // BCA Bank (Indonesia)
        }
        
        BankTransferData::BniVaBankTransfer => {
            // BNI Virtual Account (Indonesia)
        }
        
        BankTransferData::BriVaBankTransfer => {
            // BRI Virtual Account (Indonesia)
        }
        
        BankTransferData::CimbVaBankTransfer => {
            // CIMB Virtual Account (Indonesia/Malaysia)
        }
        
        BankTransferData::DanamonVaBankTransfer => {
            // Danamon Virtual Account (Indonesia)
        }
        
        BankTransferData::LocalBankTransfer => {
            // Generic local bank transfer
            // Fields vary by country
        }
        
        // Add all bank transfer types
    }
    
    // Buy Now Pay Later
    PaymentMethodData::BuyNowPayLater(bnpl_data) => match bnpl_data {
        BuyNowPayLaterData::KlarnaRedirect => {
            // Klarna BNPL
        }
        
        BuyNowPayLaterData::AffirmRedirect => {
            // Affirm BNPL
        }
        
        BuyNowPayLaterData::AfterpayClearpayRedirect => {
            // Afterpay/Clearpay BNPL
        }
        
        BuyNowPayLaterData::AlmaRedirect => {
            // Alma BNPL (French)
        }
        
        BuyNowPayLaterData::AtomeRedirect => {
            // Atome BNPL (Southeast Asia)
        }
        
        // Add all BNPL providers
    }
    
    // Cash and voucher payments
    PaymentMethodData::Voucher(voucher_data) => match voucher_data {
        VoucherData::BoletoRedirect => {
            // Boleto (Brazil)
            // - social_security_number: Option<SecretSerdeValue>
        }
        
        VoucherData::OxxoRedirect => {
            // OXXO (Mexico)
        }
        
        VoucherData::SevenElevenRedirect => {
            // 7-Eleven convenience store
        }
        
        VoucherData::LawsonRedirect => {
            // Lawson convenience store (Japan)
        }
        
        VoucherData::FamilyMartRedirect => {
            // FamilyMart convenience store
        }
        
        VoucherData::AlfamartRedirect => {
            // Alfamart convenience store (Indonesia)
        }
        
        VoucherData::IndomaretRedirect => {
            // Indomaret convenience store (Indonesia)
        }
        
        // Add all voucher types
    }
    
    // Bank redirects (online banking)
    PaymentMethodData::BankRedirect(bank_redirect) => match bank_redirect {
        BankRedirectData::BancontactCard => {
            // Bancontact (Belgium)
        }
        
        BankRedirectData::Blik => {
            // BLIK (Poland)
            // - blik_code: Option<SecretSerdeValue>
        }
        
        BankRedirectData::Eps => {
            // EPS (Austria)
            // - bank_name: Option<String>
        }
        
        BankRedirectData::Giropay => {
            // Giropay (Germany)
            // - bank_account_bic: Option<SecretSerdeValue>
            // - bank_account_iban: Option<SecretSerdeValue>
        }
        
        BankRedirectData::Ideal => {
            // iDEAL (Netherlands)
            // - bank_name: Option<String>
        }
        
        BankRedirectData::OnlineBankingCzechRepublic => {
            // Czech Republic online banking
            // - issuer: String
        }
        
        BankRedirectData::OnlineBankingFinland => {
            // Finland online banking
        }
        
        BankRedirectData::OnlineBankingPoland => {
            // Poland online banking
            // - issuer: String
        }
        
        BankRedirectData::OnlineBankingSlovakia => {
            // Slovakia online banking
            // - issuer: String
        }
        
        BankRedirectData::Przelewy24 => {
            // Przelewy24 (Poland)
            // - bank_name: Option<String>
            // - billing_details: Option<BillingDetails>
        }
        
        BankRedirectData::Sofort => {
            // Sofort (Germany/Austria)
            // - preferred_language: Option<String>
        }
        
        BankRedirectData::Trustly => {
            // Trustly (Nordic countries)
        }
        
        // Add all bank redirect methods
    }
    
    // Card redirects (3DS and similar)
    PaymentMethodData::CardRedirect(card_redirect) => match card_redirect {
        CardRedirectData::Knet => {
            // KNET (Kuwait)
        }
        
        CardRedirectData::Benefit => {
            // Benefit (Bahrain)
        }
        
        CardRedirectData::CardRedirect => {
            // Generic card redirect for 3DS
        }
    }
    
    // Cryptocurrency
    PaymentMethodData::Crypto(crypto_data) => match crypto_data {
        CryptoData::CryptoCurrencyRedirect => {
            // Generic crypto redirect
            // - network: Option<String> (e.g., "bitcoin", "ethereum")
        }
    }
    
    // Gift cards
    PaymentMethodData::GiftCard(gift_card) => match gift_card {
        GiftCardData::Givex(givex) => {
            // Givex gift card
            // - number: cards::CardNumber
            // - cvc: SecretSerdeValue
        }
        
        GiftCardData::PaySafeCard => {
            // PaySafeCard
        }
    }
}
```

## 🏦 Address and Customer Types

### Address Handling
```rust
// crates/types-traits/domain_types/src/payment_address.rs
// `Address` is a WRAPPER. The street/city/zip fields live on `AddressDetails`.
pub struct Address {
    pub address: Option<AddressDetails>,
    pub phone: Option<PhoneDetails>,
    pub email: Option<Email>,
}

pub struct AddressDetails {
    pub city: Option<Secret<String>>,
    pub country: Option<common_enums::CountryAlpha2>,
    pub line1: Option<Secret<String>>,
    pub line2: Option<Secret<String>>,
    pub line3: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub origin_zip: Option<Secret<String>>,
}

// Usage: address data hangs off the flow data, not off RouterDataV2 directly.
let billing = router_data.resource_common_data.address.get_payment_billing();
let shipping = router_data.resource_common_data.address.get_shipping();

// Convert to connector format. Note `city` is `Secret<String>`, NOT `String`.
fn build_connector_address(details: &AddressDetails) -> ConnectorAddress {
    ConnectorAddress {
        street: details.line1.clone(),
        street2: details.line2.clone(),
        city: details.city.clone(),
        state: details.state.clone(),
        postal_code: details.zip.clone(),
        country: details.country,
        first_name: details.first_name.clone(),
        last_name: details.last_name.clone(),
    }
}
```

> Confirm the accessor names on `PaymentAddress` in
> `crates/types-traits/domain_types/src/payment_address.rs` before copying -
> they are helper methods, not bare public fields.

### Customer Information
```rust
// Customer data. `customer_id` lives on the flow data; the rest on the request.
let customer_id = router_data.resource_common_data.customer_id.clone();
let email = router_data.request.email.clone();

// `request.customer` is `Option<CustomerInfo>` - see
// crates/types-traits/domain_types/src/connector_types.rs
let phone = router_data
    .request
    .customer
    .as_ref()
    .and_then(|c| c.customer_phone_number.clone());
let customer_name = router_data
    .request
    .customer
    .as_ref()
    .and_then(|c| c.customer_name.clone());

// Browser information for 3DS
let browser_info = router_data.request.browser_info.as_ref();
```

## 💰 Amount and Currency Types

### Amount Handling
```rust
// crates/common/common_utils/src/types.rs - there are FIVE unit types:
use common_utils::types::{
    MinorUnit,             // i64 minor units (cents)                -> 1250
    StringMinorUnit,       // String minor units                     -> "1250"
    StringMajorUnit,       // String major units                     -> "12.50"
    FloatMajorUnit,        // f64 major units                        -> 12.5
    StringTwoDecimalUnit,  // String major units, always 2 decimals   -> "12.50"
};

// Each has a matching `AmountConvertor` in the same file:
// MinorUnitForConnector, StringMinorUnitForConnector, StringMajorUnitForConnector,
// FloatMajorUnitForConnector, StringTwoDecimalUnitForConnector.

// Convert with the shared helper (do NOT hand-roll the arithmetic):
use domain_types::utils::convert_amount;
let amount: StringMajorUnit = convert_amount(
    &common_utils::types::StringMajorUnitForConnector,
    router_data.request.minor_amount,
    router_data.request.currency,
)?;

// Currency handling
use common_enums::Currency;
let currency: Currency = router_data.request.currency;
let currency_string = currency.to_string();

// Zero decimal currencies (amounts don't have decimal places).
// The convertors already handle this - prefer `Currency::is_zero_decimal_currency()`
// over re-listing currencies by hand.
let is_zero_decimal = currency.is_zero_decimal_currency();
```

### Choosing the unit type

**Do not default to `StringMinorUnit`.** Read the vendor's API spec and match the
wire format it documents. On HEAD the actual distribution across connectors is:

| Unit type | Connectors on HEAD | Wire example |
|-----------|--------------------|--------------|
| `StringMajorUnit` | 34 | `"12.50"` |
| `FloatMajorUnit` | 26 | `12.5` |
| `MinorUnit` | 21 | `1250` |
| `StringMinorUnit` | 19 | `"1250"` |
| `StringTwoDecimalUnit` | (rare) | `"12.50"`, always 2dp |

If the spec shows `"amount": "10.00"` use `StringMajorUnit`; `"amount": 1000` use
`MinorUnit`; `"amount": 10.0` use `FloatMajorUnit`. Guessing is wrong roughly four
times out of five.

## 🔐 Authentication Types

### Connector Authentication

`RouterDataV2` has **no** `connector_auth_type` field (removed 2026-03-14). Auth
credentials arrive on `req.connector_config: ConnectorSpecificConfig` as a
**per-connector enum variant**, and the connector's own auth struct is built by
`TryFrom<&ConnectorSpecificConfig>`.

```rust
// transformers.rs - define the connector's own auth struct
use domain_types::{
    errors::{IntegrationError, IntegrationErrorContext},
    router_data::ConnectorSpecificConfig,
};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};

#[derive(Debug, Clone)]
pub struct ConnectorNameAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ConnectorNameAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            // One variant per connector, generated alongside the connector.
            ConnectorSpecificConfig::ConnectorName {
                username,
                password,
                merchant_id,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Ensure the connector account is configured with ConnectorName credentials"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "ConnectorSpecificConfig variant mismatch - the request may have been routed to the wrong connector"
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}
```

`ConnectorCommon::get_auth_header` takes `&ConnectorSpecificConfig`, not
`&ConnectorAuthType`. Using the old signature is **E0407** (method not a member
of the trait):

```rust
// crates/types-traits/interfaces/src/api.rs:25 - real signature
fn get_auth_header(
    &self,
    auth_type: &ConnectorSpecificConfig,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = connector_name::ConnectorNameAuthType::try_from(auth_type).change_context(
        IntegrationError::FailedToObtainAuthType {
            context: IntegrationErrorContext::default(),
        },
    )?;
    Ok(vec![(
        headers::AUTHORIZATION.to_string(),
        auth.generate_authorization_header().into_masked(),
    )])
}
```

Inside a transformer the config is reached through the router data:

```rust
let auth = ConnectorNameAuthType::try_from(&item.connector_config)?;
```

> **Working exemplar:** `crates/integrations/connector-integration/src/connectors/travelhub.rs`
> (trait impl) and `.../connectors/travelhub/transformers.rs` (auth struct).

## 📊 Response Types

### Standard Response Pattern
```rust
// Connector response structure.
// Status is a TYPED enum with `#[serde(other)]`, never a bare String - see
// "Status mapping" below for why both halves are mandatory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorPaymentStatus {
    Authorized,
    Captured,
    Failed,
    Pending,
    RequiresAction,
    /// Catch-all at the DESERIALIZATION layer so an unknown wire value does not
    /// fail the whole parse.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorPaymentResponse {
    pub id: String,
    pub status: ConnectorPaymentStatus,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub gateway_reference_id: Option<String>,
    pub redirect_url: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    // Add connector-specific fields
}

// Convert to UCS response.
// `ResponseRouterData` takes TWO type parameters: <ConnectorResponse, RouterData>.
// Its fields are `response`, `router_data` and `http_code`.
//   crates/integrations/connector-integration/src/types.rs
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<ConnectorPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ConnectorPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Status mapping is EXHAUSTIVE over the typed enum - no `_ =>` arm.
        // The compiler then tells you when the connector adds a status.
        let status = match item.response.status {
            ConnectorPaymentStatus::Authorized => AttemptStatus::Authorized,
            ConnectorPaymentStatus::Captured => AttemptStatus::Charged,
            ConnectorPaymentStatus::Failed => AttemptStatus::Failure,
            ConnectorPaymentStatus::Pending => AttemptStatus::Pending,
            ConnectorPaymentStatus::RequiresAction => AttemptStatus::AuthenticationPending,
            ConnectorPaymentStatus::Unknown => AttemptStatus::Pending,
        };

        // In-band 2xx failure: the HTTP call succeeded but the payment did not.
        // Return `Err(ErrorResponse{..})` so the failure is surfaced, not swallowed.
        // `impl Default for ErrorResponse` exists, so use functional update.
        if domain_types::utils::is_payment_failure(status) {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: item
                        .response
                        .error_code
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: item
                        .response
                        .error_message
                        .clone()
                        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: item.response.error_message.clone(),
                    status_code: item.http_code,
                    // Flow-aware and non-terminal by default. Do NOT hardcode
                    // `Some(AttemptStatus::Failure)` (wrong type AND it reports a
                    // charged payment as FAILURE); do NOT blanket-`None` either.
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(item.response.id.clone()),
                    ..Default::default()
                }),
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // `PaymentsResponseData::TransactionResponse` is an enum STRUCT variant:
        // there is no functional-update (`..Default::default()`) syntax for it,
        // so every one of its 11 fields must be listed or you get E0063.
        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: item.response.redirect_url.clone().map(|url| {
                    Box::new(RedirectForm::Form {
                        endpoint: url,
                        method: Method::Get,
                        form_fields: HashMap::new(),
                    })
                }),
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: item.response.gateway_reference_id.clone(),
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.id.clone()),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            // `status` lives on the flow data, NOT on RouterDataV2 itself.
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
```

### `PaymentsResponseData::TransactionResponse` - real field list

```rust
// crates/types-traits/domain_types/src/connector_types.rs
TransactionResponse {
    resource_id: ResponseId,
    redirection_data: Option<Box<RedirectForm>>,
    connector_metadata: Option<serde_json::Value>,
    mandate_reference: Option<Box<MandateReference>>,
    network_txn_id: Option<String>,
    network_txn_link_id: Option<String>,
    connector_response_reference_id: Option<String>,
    incremental_authorization_allowed: Option<bool>,
    splits: Option<ConnectorSplitResponseData>,
    status_code: u16,
    payment_account_reference: Option<String>,
}
```

### `RefundsResponseData` - real field list

```rust
// crates/types-traits/domain_types/src/connector_types.rs
pub struct RefundsResponseData {
    pub connector_refund_id: String,
    pub refund_status: common_enums::RefundStatus,
    pub status_code: u16,
    pub acquirer_reference_number: Option<String>,
}
```

This one is a plain struct, so `..Default::default()` is not available either
unless the struct derives `Default` - list all four fields.

### `ErrorResponse` - 13 fields, in this order

```rust
// crates/types-traits/domain_types/src/router_data.rs
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
    pub status_code: u16,
    pub attempt_status: Option<FlowStatus>,      // NOT Option<AttemptStatus>
    pub connector_transaction_id: Option<String>,
    pub network_decline_code: Option<String>,
    pub network_advice_code: Option<String>,
    pub network_error_message: Option<String>,
    pub typed_connector_response: Option<String>,
    pub raw_connector_response: Option<Secret<String>>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
}

// There IS an `impl Default for ErrorResponse` in that file, so prefer
// functional update over listing all 13 fields.

pub enum FlowStatus {
    Payment(common_enums::enums::AttemptStatus),
    Refund(common_enums::enums::RefundStatus),
    Dispute(common_enums::enums::DisputeStatus),
    Payout(common_enums::enums::PayoutStatus),
}
```

`attempt_status` is `Option<FlowStatus>`. `Some(AttemptStatus::Failure)` does not
typecheck; wrap it: `Some(FlowStatus::Payment(AttemptStatus::Failure))`. And pick
the status per flow - a hard-declined refund must be
`Some(FlowStatus::Refund(RefundStatus::Failure))`, not `None`, or it stays Pending
and keeps retrying.

### `PaymentsCaptureData` - real field list

```rust
// crates/types-traits/domain_types/src/connector_types.rs
pub struct PaymentsCaptureData {
    pub amount_to_capture: i64,              // NOT Option
    pub minor_amount_to_capture: MinorUnit,
    pub currency: Currency,
    pub connector_transaction_id: ResponseId,
    pub multiple_capture_data: Option<MultipleCaptureRequestData>,
    pub connector_feature_data: Option<SecretSerdeValue>,
    pub integrity_object: Option<CaptureIntegrityObject>,
    pub browser_info: Option<BrowserInformation>,
    pub capture_method: Option<common_enums::CaptureMethod>,
    pub metadata: Option<SecretSerdeValue>,
    pub order_tax_amount: Option<MinorUnit>,
    pub merchant_order_id: Option<String>,
    pub split_payments: Option<SplitPaymentsDetails>,
    pub split_settlement: Option<Box<SplitSettlement>>,
}
```

There is **no** `payment_amount` field (`request.payment_amount` is E0609) and
`amount_to_capture` is not an `Option` (`request.amount_to_capture.is_none()` is
E0599). Partial-capture detection is connector-specific: compare
`request.amount_to_capture` against the authorized amount the connector itself
returned (from its authorize response or from connector metadata). Do not invent
a field for it.

### `ConnectorError` - exactly five variants

```rust
// crates/types-traits/domain_types/src/errors.rs
pub enum ConnectorError {
    ResponseDeserializationFailed { context: ResponseTransformationErrorContext },
    ResponseHandlingFailed        { context: ResponseTransformationErrorContext },
    UnexpectedResponseError       { context: ResponseTransformationErrorContext },
    IntegrityCheckFailed {
        context: ResponseTransformationErrorContext,
        field_names: String,
        connector_transaction_id: Option<String>,
    },
    ConnectorErrorResponse(Box<ErrorResponse>),
}
```

`ConnectorError::InvalidData`, `::NotImplemented(..)` and `::InvalidCard` **do not
exist** (E0599). Those names belong to `IntegrationError` in the same file
(`NotImplemented(String, IntegrationErrorContext)`, `NotSupported { .. }`,
`InvalidDataFormat { field_name, context }`, `FailedToObtainAuthType { context }`,
`MissingRequiredField { .. }`, ...). Read the real variant list before
substituting, and keep the `context` field.

`ConnectorError` is the response-side error type (transformers, `handle_response_v2`,
`build_error_response`). `IntegrationError` is the request-side error type
(`get_url`, `get_headers`, `get_request_body`, `get_auth_header`).

### `SourceVerification` and `BodyDecoding` are NON-GENERIC

```rust
// crates/types-traits/interfaces/src/verification.rs:20
pub trait SourceVerification { /* all methods have defaults */ }
// crates/types-traits/interfaces/src/decode.rs
pub trait BodyDecoding { /* all methods have defaults */ }
```

Write **one** impl per connector, not one per flow. A per-flow
`impl<T> SourceVerification<Flow, Data, Req, Resp> for X<T> {}` is **E0107**
(wrong number of generic arguments).

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for ConnectorName<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for ConnectorName<T>
{
}
```

Exemplar: `crates/integrations/connector-integration/src/connectors/travelhub.rs:175`.

## 🎣 Webhook Types

### Webhook trait signatures (argument counts matter)

```rust
// crates/types-traits/interfaces/src/connector_types.rs
// The error type is WebhookError, NOT IntegrationError.

fn get_event_type(
    &self,
    _request: RequestDetails,                                     // ONE arg besides &self
) -> Result<EventType, error_stack::Report<WebhookError>>;

fn get_webhook_event_reference(
    &self,
    _request: RequestDetails,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>>;

fn process_payment_webhook(
    &self,
    _request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorSpecificConfig>,
    _event_context: Option<domain_types::connector_types::EventContext>, // FOUR args besides &self
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>>;

fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck>;
```

Calling `get_event_type` with three arguments, or `process_payment_webhook` with
three, is **E0061**. There is no `transformation_status` field and no
`WebhookTransformationStatus` type anywhere in the tree - using either is **E0560**.

### Webhook Data Structures

```rust
// Incoming webhook payload as the connector sends it.
// Event type is a TYPED enum with `#[serde(other)]`, not a bare String.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorWebhookEventType {
    #[serde(rename = "payment.authorized")]
    PaymentAuthorized,
    #[serde(rename = "payment.captured")]
    PaymentCaptured,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "payment.cancelled")]
    PaymentCancelled,
    #[serde(rename = "refund.succeeded")]
    RefundSucceeded,
    #[serde(rename = "refund.failed")]
    RefundFailed,
    #[serde(rename = "dispute.created")]
    DisputeCreated,
    #[serde(rename = "dispute.won")]
    DisputeWon,
    #[serde(rename = "dispute.lost")]
    DisputeLost,
    /// Deserialization-layer catch-all: an unrecognised event must not fail the parse.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorWebhook {
    pub event_type: ConnectorWebhookEventType,
    pub object_type: String,
    pub object_id: String,
    pub data: serde_json::Value,
}
```

### Mapping webhook events

`get_event_type` returns `domain_types::connector_types::EventType`. Read the real
variant list in `crates/types-traits/domain_types/src/connector_types.rs`
(`pub enum EventType`) before mapping - there is no `EventNotSupported` variant.

```rust
use domain_types::{connector_types::EventType, errors::WebhookError};

// Exhaustive over the typed enum: no `_ =>` arm at the mapping layer.
let event = match webhook.event_type {
    ConnectorWebhookEventType::PaymentAuthorized => EventType::PaymentIntentAuthorizationSuccess,
    ConnectorWebhookEventType::PaymentCaptured   => EventType::PaymentIntentCaptureSuccess,
    ConnectorWebhookEventType::PaymentFailed     => EventType::PaymentIntentFailure,
    ConnectorWebhookEventType::PaymentCancelled  => EventType::PaymentIntentCancelled,
    ConnectorWebhookEventType::RefundSucceeded   => EventType::RefundSuccess,
    ConnectorWebhookEventType::RefundFailed      => EventType::RefundFailure,
    ConnectorWebhookEventType::DisputeCreated    => EventType::DisputeOpened,
    ConnectorWebhookEventType::DisputeWon        => EventType::DisputeWon,
    ConnectorWebhookEventType::DisputeLost       => EventType::DisputeLost,
    // An unknown wire value parsed cleanly, but we cannot claim a meaning for it.
    ConnectorWebhookEventType::Unknown => {
        return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
    }
};
```

### Webhook reference and response types

```rust
// crates/types-traits/domain_types/src/connector_types.rs
pub enum WebhookResourceReference {
    Payment(PaymentWebhookReference),
    Refund(RefundWebhookReference),
    Dispute(DisputeWebhookReference),
    Mandate(MandateWebhookReference),
    Payout(PayoutWebhookReference),
}
```

`WebhookDetailsResponse` is a large struct (`resource_id`, `status`, `error_code`,
`error_message`, `status_code`, `amount_captured`, ...). Read it in
`connector_types.rs` and fill every field - it is a plain struct with no `Default`
shortcut assumed here.

> `IncomingWebhookEvent` lives in `interfaces::webhooks`, **not** in
> `domain_types::connector_types`. Connectors implement the `EventType`-returning
> trait above; do not import `IncomingWebhookEvent` from `connector_types`.

## 🔧 Utility Types

### Common Utility Functions
```rust
// Amount conversion utilities
use domain_types::utils;

// Convert minor units to a major-unit String
// pub fn to_currency_base_unit(amount: MinorUnit, currency: Currency)
//     -> Result<String, Report<IntegrationError>>
let major_amount: String = utils::to_currency_base_unit(minor_amount, currency)?;

// Generic conversion via an AmountConvertor (preferred - picks the right wire shape)
// pub fn convert_amount<T>(convertor: &dyn AmountConvertor<Output = T>,
//                          amount: MinorUnit, currency: Currency)
//     -> Result<T, Report<IntegrationError>>
let amount = utils::convert_amount(&StringMajorUnitForConnector, minor_amount, currency)?;

// Webhook variant returns a WebhookError instead of an IntegrationError
let amount = utils::convert_amount_for_webhook(&MinorUnitForConnector, minor, currency)?;

// Get unimplemented payment method error message
let error = utils::get_unimplemented_payment_method_error_message("connector_name");
```

> `to_currency_base_unit_as_string` does not exist. Verify any helper against
> `crates/types-traits/domain_types/src/utils.rs` before using it.

## 📋 Type Safety Best Practices

1. **Always use RouterDataV2** instead of RouterData - and give it all **four**
   type parameters (`Flow, ResourceCommonData, Request, Response`).
2. **Use ConnectorIntegrationV2** for all trait implementations.
3. **Import from domain_types**, not hyperswitch_domain_models.
4. **Handle all payment method variants** explicitly.
5. **Use the right error type per side**: `IntegrationError` for request building,
   `ConnectorError` for response handling, `WebhookError` for webhooks. Every
   variant carries a `context` - fill it in.
6. **Status mapping must be exhaustive** over a typed connector-status enum
   (no `_ =>` at the mapping layer), with `#[serde(other)] Unknown` at the
   deserialization layer. Reviewers require **both** halves.
7. **Use Secret types** for sensitive data.
8. **Handle Option types** properly for optional fields.
9. **Pick the amount unit from the vendor spec** - all five types exist; do not
   default to `StringMinorUnit`.
10. **Implement complete webhook event mapping** with the correct argument counts.
11. **Never `unwrap_or_default()` an error code or message.** Use
    `common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE}`:
    `.unwrap_or_else(|| NO_ERROR_CODE.to_string())`.
12. **Never hardcode a terminal `attempt_status` on the shared error path.**
    `attempt_status` is `Option<FlowStatus>` and must be flow-aware; exemplar
    `connectors/flywire.rs:362-370`, minimal form `connectors/noon.rs:499-512`.
13. **Return `Err(ErrorResponse{..})` for in-band 2xx failures**, branching on a
    success predicate such as `domain_types::utils::is_payment_failure`.
14. **Auth comes from `req.connector_config: ConnectorSpecificConfig`**, never
    from a `connector_auth_type` field - that field no longer exists.
15. **`SourceVerification` and `BodyDecoding` are non-generic** - one impl per
    connector, not one per flow.

This type system ensures type safety, comprehensive payment method support, and proper integration with the UCS architecture.