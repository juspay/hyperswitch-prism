# UCS Type System Guide

Comprehensive guide to UCS connector-service type system, covering all payment flows and data structures.

## 🏗️ Core UCS Type Imports

```rust
// Essential UCS imports - use these in every connector
use domain_types::{
    // Flow types for all operations
    connector_flow::{
        Authorize, Capture, Void, Refund, PSync, RSync,
        CreateOrder, CreateSessionToken, SetupMandate, 
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
        SessionTokenRequestData, SessionTokenResponseData,
        SetupMandateRequestData,
        
        // Dispute types
        DisputeFlowData, DisputeResponseData,
        AcceptDisputeData, DisputeDefendData, SubmitEvidenceData,
        
        // Webhook types
        WebhookDetailsResponse, RefundWebhookDetailsResponse,
        
        // Common types
        ResponseId, RequestDetails, ConnectorSpecifications,
        SupportedPaymentMethodsExt, ConnectorWebhookSecrets,
    },
    
    // Enhanced router data
    router_data_v2::RouterDataV2,
    
    // Payment method data (all types)
    payment_method_data::{
        PaymentMethodData, PaymentMethodDataTypes, DefaultPCIHolder,
        Card, Wallet, BankTransfer, BuyNowPayLater, Voucher, 
        Crypto, GiftCard, BankRedirect, CardRedirect
    },
    
    // Address and customer data
    payment_address::{Address, AddressDetails},
    
    // Error types
    errors,
    
    // Router data and auth
    router_data::{ConnectorAuthType, ErrorResponse},
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
    connector_types::{self, ConnectorValidation, is_mandate_supported},
    events::connector_api_logs::ConnectorEvent,
};

// Common utilities
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, EventClass,
    PaymentMethod, PaymentMethodType, Currency, CountryAlpha2
};
use common_utils::{
    errors::CustomResult, 
    ext_traits::ByteSliceExt,
    pii::{SecretSerdeValue, Email, IpAddress},
    types::{StringMinorUnit, MinorUnit, StringMajorUnit, FloatMajorUnit},
    request::Method,
};

// Masking utilities
use hyperswitch_masking::{Mask, Maskable};

// Serialization
use serde::{Serialize, Deserialize};
```

## 🔄 Router Data Types

### Core Router Data Pattern
```rust
// UCS uses RouterDataV2 for all operations
type AuthorizeRouterData = RouterDataV2<
    Authorize,                    // Flow type
    PaymentsAuthorizeData,       // Request data type
    PaymentsResponseData         // Response data type
>;

type CaptureRouterData = RouterDataV2<
    Capture,
    PaymentsCaptureData,
    PaymentsResponseData
>;

type VoidRouterData = RouterDataV2<
    Void,
    PaymentVoidData,
    PaymentsResponseData
>;

type RefundRouterData = RouterDataV2<
    Refund,
    RefundsData,
    RefundsResponseData
>;

type SyncRouterData = RouterDataV2<
    PSync,
    PaymentsSyncData,
    PaymentsResponseData
>;

type RefundSyncRouterData = RouterDataV2<
    RSync,
    RefundSyncData,
    RefundsResponseData
>;

// Advanced flow types
type CreateOrderRouterData = RouterDataV2<
    CreateOrder,
    PaymentCreateOrderData,
    PaymentCreateOrderResponse
>;

type SessionTokenRouterData = RouterDataV2<
    CreateSessionToken,
    SessionTokenRequestData,
    SessionTokenResponseData
>;

type SetupMandateRouterData = RouterDataV2<
    SetupMandate,
    SetupMandateRequestData,
    PaymentsResponseData
>;

// Dispute flow types
type DefendDisputeRouterData = RouterDataV2<
    DefendDispute,
    DisputeDefendData,
    DisputeResponseData
>;

type AcceptDisputeRouterData = RouterDataV2<
    Accept,
    AcceptDisputeData,
    DisputeResponseData
>;

type SubmitEvidenceRouterData = RouterDataV2<
    SubmitEvidence,
    SubmitEvidenceData,
    DisputeResponseData
>;
```

## 💳 Payment Method Data Types

### Comprehensive Payment Method Handling
```rust
// Handle all payment method types
match payment_method_data {
    // Card payments - most common
    PaymentMethodData::Card(card_data) => {
        // card_data fields:
        // - card_number: cards::CardNumber
        // - card_exp_month: SecretSerdeValue
        // - card_exp_year: SecretSerdeValue  
        // - card_cvc: SecretSerdeValue
        // - card_holder_name: Option<SecretSerdeValue>
        // - card_network: Option<CardNetwork>
        // - card_issuer: Option<String>
        // - card_type: Option<String>
        // - nick_name: Option<SecretSerdeValue>
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
// Address structure in UCS
pub struct Address {
    pub line1: Option<SecretSerdeValue>,
    pub line2: Option<SecretSerdeValue>,
    pub line3: Option<SecretSerdeValue>,
    pub city: Option<String>,
    pub state: Option<SecretSerdeValue>,
    pub zip: Option<SecretSerdeValue>,
    pub country: Option<CountryAlpha2>,
    pub first_name: Option<SecretSerdeValue>,
    pub last_name: Option<SecretSerdeValue>,
}

// Usage in router data
let billing_address = router_data.address.billing.as_ref();
let shipping_address = router_data.address.shipping.as_ref();

// Convert to connector format
fn build_connector_address(address: &Address) -> ConnectorAddress {
    ConnectorAddress {
        street: address.line1.clone(),
        street2: address.line2.clone(),
        city: address.city.clone(),
        state: address.state.clone(),
        postal_code: address.zip.clone(),
        country: address.country.clone(),
        first_name: address.first_name.clone(),
        last_name: address.last_name.clone(),
    }
}
```

### Customer Information
```rust
// Customer data in router data
let customer_id = router_data.customer_id.clone();
let email = router_data.request.email.clone();
let phone = router_data.request.customer.phone.clone();
let customer_name = router_data.request.customer.name.clone();

// Browser information for 3DS
let browser_info = router_data.request.browser_info.as_ref();
```

## 💰 Amount and Currency Types

### Amount Handling
```rust
// UCS amount types
use common_utils::types::{
    MinorUnit,        // For amounts in minor units (cents)
    StringMinorUnit,  // String representation of minor units
    StringMajorUnit,  // String representation of major units (dollars)
    FloatMajorUnit,   // Float representation of major units
};

// Convert between types
let minor_amount: MinorUnit = router_data.request.amount;
let amount_string = minor_amount.to_string();
let amount_f64 = utils::to_currency_base_unit(minor_amount, currency)?;

// Currency handling
use common_enums::Currency;
let currency: Currency = router_data.request.currency;
let currency_string = currency.to_string();

// Zero decimal currencies (amounts don't have decimal places)
let is_zero_decimal = matches!(currency, 
    Currency::JPY | Currency::KRW | Currency::VND | 
    Currency::CLP | Currency::ISK | Currency::XOF
);
```

## 🔐 Authentication Types

### Connector Authentication
```rust
// Define connector-specific auth type
#[derive(Debug, Clone)]
pub struct ConnectorAuthType {
    pub api_key: SecretSerdeValue,
    pub secret_key: Option<SecretSerdeValue>,
    pub username: Option<SecretSerdeValue>,
    pub password: Option<SecretSerdeValue>,
    pub certificate: Option<SecretSerdeValue>,
}

// Convert from domain auth type
impl TryFrom<&domain_types::router_data::ConnectorAuthType> for ConnectorAuthType {
    type Error = Error;
    
    fn try_from(auth_type: &domain_types::router_data::ConnectorAuthType) -> Result<Self, Self::Error> {
        match auth_type {
            domain_types::router_data::ConnectorAuthType::HeaderKey { api_key } => {
                Ok(Self {
                    api_key: api_key.clone(),
                    secret_key: None,
                    username: None,
                    password: None,
                    certificate: None,
                })
            }
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
                Ok(Self {
                    api_key: api_key.clone(),
                    secret_key: key1.clone(),
                    username: None,
                    password: None,
                    certificate: None,
                })
            }
            domain_types::router_data::ConnectorAuthType::SignatureKey { 
                api_key, 
                key1, 
                api_secret 
            } => {
                Ok(Self {
                    api_key: api_key.clone(),
                    secret_key: key1.clone(),
                    username: None,
                    password: api_secret.clone(),
                    certificate: None,
                })
            }
            // Handle other auth types
        }
    }
}
```

## 📊 Response Types

### Standard Response Pattern
```rust
// Connector response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorPaymentResponse {
    pub id: String,
    pub status: String,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub gateway_reference_id: Option<String>,
    pub redirect_url: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    // Add connector-specific fields
}

// Convert to UCS response
impl TryFrom<ResponseRouterData<Authorize, ConnectorPaymentResponse, PaymentsAuthorizeData, PaymentsResponseData>>
    for RouterDataV2<Authorize, PaymentsAuthorizeData, PaymentsResponseData>
{
    type Error = Error;
    
    fn try_from(
        item: ResponseRouterData<Authorize, ConnectorPaymentResponse, PaymentsAuthorizeData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status.as_str() {
            "authorized" => AttemptStatus::Authorized,
            "captured" => AttemptStatus::Charged,
            "failed" => AttemptStatus::Failure,
            "pending" => AttemptStatus::Pending,
            "requires_action" => AttemptStatus::AuthenticationPending,
            _ => AttemptStatus::Pending,
        };
        
        let response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id),
            redirection_data: item.response.redirect_url.map(|url| {
                Box::new(RedirectForm::Form {
                    endpoint: url,
                    method: Method::Get,
                    form_fields: HashMap::new(),
                })
            }),
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: item.response.gateway_reference_id,
            connector_response_reference_id: Some(item.response.id.clone()),
            incremental_authorization_allowed: None,
        };
        
        Ok(Self {
            response: Ok(response_data),
            status,
            ..item.data
        })
    }
}
```

## 🎣 Webhook Types

### Webhook Data Structures
```rust
// Incoming webhook structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorWebhook {
    pub event_type: String,
    pub object_type: String,
    pub object_id: String,
    pub data: serde_json::Value,
}

// Webhook object reference types
use domain_types::connector_types::{
    ObjectReferenceId, PaymentIdType, RefundIdType,
    IncomingWebhookEvent, DisputePayload
};

// Map webhook events
match webhook.event_type.as_str() {
    "payment.authorized" => IncomingWebhookEvent::PaymentIntentAuthorizationSuccess,
    "payment.captured" => IncomingWebhookEvent::PaymentIntentSuccess,
    "payment.failed" => IncomingWebhookEvent::PaymentIntentFailure,
    "payment.cancelled" => IncomingWebhookEvent::PaymentIntentCancelled,
    "refund.succeeded" => IncomingWebhookEvent::RefundSuccess,
    "refund.failed" => IncomingWebhookEvent::RefundFailure,
    "dispute.created" => IncomingWebhookEvent::DisputeOpened,
    "dispute.won" => IncomingWebhookEvent::DisputeWon,
    "dispute.lost" => IncomingWebhookEvent::DisputeLost,
    _ => IncomingWebhookEvent::EventNotSupported,
}
```

## 🔧 Utility Types

### Common Utility Functions
```rust
// Amount conversion utilities
use domain_types::utils;

// Convert minor to major units
let major_amount = utils::to_currency_base_unit(minor_amount, currency)?;

// Format amount for connector
let formatted_amount = utils::to_currency_base_unit_as_string(minor_amount, currency)?;

// Get unimplemented payment method error
let error = utils::get_unimplemented_payment_method_error_message("connector_name");
```

## 📋 Type Safety Best Practices

1. **Always use RouterDataV2** instead of RouterData
2. **Use ConnectorIntegrationV2** for all trait implementations
3. **Import from domain_types** not hyperswitch_domain_models
4. **Handle all payment method variants** explicitly
5. **Use proper error types** from domain_types::errors
6. **Implement comprehensive status mapping** for all connector states
7. **Use Secret types** for sensitive data (SecretSerdeValue)
8. **Handle Option types** properly for optional fields
9. **Use proper currency handling** for international payments
10. **Implement complete webhook event mapping** for real-time updates

This type system ensures type safety, comprehensive payment method support, and proper integration with the UCS architecture.