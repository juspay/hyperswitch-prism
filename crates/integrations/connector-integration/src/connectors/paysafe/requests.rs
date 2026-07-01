use common_utils::types::MinorUnit;
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaymentsRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    pub settle_with_auth: bool,
    pub payment_handle_token: Secret<String>,
    pub currency_code: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_ip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credential: Option<PaysafeStoredCredential>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeStoredCredential {
    #[serde(rename = "type")]
    pub stored_credential_type: PaysafeStoredCredentialType,
    pub occurrence: MandateOccurrence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MandateOccurrence {
    Initial,
    Subsequent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeStoredCredentialType {
    Adhoc,
    Topup,
}

/// CreateConnectorCustomer request body (`POST v1/customers`).
///
/// Registers a Paysafe customer profile so a reusable (MULTI_USE) payment handle
/// can later be minted under `v1/customers/{customerId}/paymenthandles`. Mirrors
/// hyperswitch's `PaysafeCustomerDetails`: `merchantCustomerId` is mandatory; the
/// name/email/phone fields are optional and sourced from the customer/billing
/// details the caller supplies.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeCustomerRequest {
    pub merchant_customer_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeCaptureRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeVoidRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeRefundRequest {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub merchant_ref_num: String,
    pub amount: MinorUnit,
    // Omitted for redirect wallets (e.g. Skrill) whose verified payment-handle body
    // does not include settleWithAuth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_with_auth: Option<bool>,
    #[serde(flatten)]
    pub payment_method: PaysafePaymentMethod<T>,
    pub currency_code: common_enums::Currency,
    pub payment_type: PaysafePaymentType,
    pub transaction_type: TransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_links: Option<Vec<ReturnLink>>,
    // Skrill must omit accountId entirely (sending the card accountId returns error 5068).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<ThreeDs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<PaysafeProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_details: Option<PaysafeBillingDetails>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum PaysafePaymentMethod<T: PaymentMethodDataTypes> {
    Card {
        card: PaysafeCard<T>,
    },
    Ach {
        ach: PaysafeAch,
    },
    GooglePay {
        #[serde(rename = "googlePay")]
        google_pay: PaysafeGooglePay,
    },
    ApplePay {
        #[serde(rename = "applePay")]
        apple_pay: PaysafeApplePay,
    },
    Skrill {
        skrill: PaysafeSkrill,
    },
    InteracEtransfer {
        #[serde(rename = "interacEtransfer")]
        interac_etransfer: PaysafeInterac,
    },
    Paysafecard {
        paysafecard: PaysafePaysafecard,
    },
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeSkrill {
    /// Skrill consumer email address.
    pub consumer_id: common_utils::pii::Email,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeInterac {
    /// Interac e-Transfer consumer email address.
    pub consumer_id: common_utils::pii::Email,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafePaysafecard {
    /// paysafecard consumer identifier. REQUIRED. Mapped from the merchant
    /// customer id (a stable, non-PII identifier). paysafecard's consumerId has
    /// a restricted format (alphanumeric + limited specials), so a raw billing
    /// email (containing '@') is not a valid value. Mirrors hyperswitch, which
    /// maps this field from get_customer_id() (id_type::CustomerId), reserving
    /// billing email for Skrill/Interac only.
    pub consumer_id: common_utils::id_type::CustomerId,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePay {
    pub google_pay_payment_token: PaysafeGooglePayPaymentToken,
}

/// Apple Pay payment-handle body. Paysafe expects the full (encrypted) Apple Pay
/// PKPaymentToken forwarded under `applePay.applePayPaymentToken.token`.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePay {
    /// User-facing label (Apple Pay payment-method display name).
    pub label: String,
    pub apple_pay_payment_token: PaysafeApplePayPaymentToken,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayPaymentToken {
    pub token: PaysafeApplePayToken,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayToken {
    /// The encrypted Apple Pay payment data object
    /// (`{data, signature, header, version}`), forwarded verbatim.
    pub payment_data: serde_json::Value,
    pub payment_method: PaysafeApplePayPaymentMethod,
    pub transaction_identifier: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayPaymentMethod {
    pub display_name: String,
    pub network: String,
    #[serde(rename = "type")]
    pub pm_type: String,
}

/// The full Google Pay SDK response object that Paysafe expects
/// inside `googlePay.googlePayPaymentToken`
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayPaymentToken {
    pub api_version: i32,
    pub api_version_minor: i32,
    pub payment_method_data: PaysafeGooglePayPaymentMethodData,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayPaymentMethodData {
    /// The type of payment method, e.g. "CARD"
    #[serde(rename = "type")]
    pub pm_type: String,
    /// User-facing description, e.g. "Mastercard **** 1021"
    pub description: String,
    /// Card info (network + last 4)
    pub info: PaysafeGooglePayCardInfo,
    /// Tokenization data containing the decryptedToken block
    pub tokenization_data: PaysafeGooglePayTokenizationData,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayCardInfo {
    pub card_network: String,
    pub card_details: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayTokenizationData {
    /// Always "PAYMENT_GATEWAY"
    #[serde(rename = "type")]
    pub token_type: String,
    /// The decrypted Google Pay token data
    pub decrypted_token: PaysafeGooglePayDecryptedToken,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayDecryptedToken {
    pub message_id: String,
    pub message_expiration: String,
    pub payment_method_details: PaysafeGooglePayPaymentMethodDetails,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeGooglePayPaymentMethodDetails {
    pub auth_method: PaysafeGooglePayAuthMethod,
    pub pan: Secret<String>,
    pub expiration_month: u8,
    pub expiration_year: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum PaysafeGooglePayAuthMethod {
    #[serde(rename = "PAN_ONLY")]
    PanOnly,
    #[serde(rename = "CRYPTOGRAM_3DS")]
    Cryptogram3Ds,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeCard<T: PaymentMethodDataTypes> {
    pub card_num: RawCardNumber<T>,
    pub card_expiry: PaysafeCardExpiry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder_name: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeAch {
    pub account_holder_name: Secret<String>,
    pub account_number: Secret<String>,
    pub routing_number: Secret<String>,
    pub account_type: PaysafeAchAccountType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaysafeCardExpiry {
    pub month: Secret<String>,
    pub year: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaysafePaymentType {
    Card,
    Ach,
    Skrill,
    #[serde(rename = "INTERAC_ETRANSFER")]
    InteracEtransfer,
    Paysafecard,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaysafeAchAccountType {
    Checking,
    Savings,
    Loan,
}

#[derive(Debug, Serialize)]
pub enum TransactionType {
    #[serde(rename = "PAYMENT")]
    Payment,
}

#[derive(Debug, Serialize)]
pub struct ReturnLink {
    pub rel: LinkType,
    pub href: String,
    pub method: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    OnCompleted,
    OnFailed,
    OnCancelled,
    Default,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDs {
    pub merchant_url: String,
    pub device_channel: DeviceChannel,
    pub message_category: ThreeDsMessageCategory,
    pub authentication_purpose: ThreeDsAuthenticationPurpose,
    pub requestor_challenge_preference: ThreeDsChallengePreference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeviceChannel {
    Browser,
    Sdk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDsMessageCategory {
    Payment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDsAuthenticationPurpose {
    PaymentTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThreeDsChallengePreference {
    ChallengeMandated,
    NoPreference,
    NoChallengeRequested,
    ChallengeRequested,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeProfile {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub email: common_utils::pii::Email,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeBillingDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    pub state: Secret<String>,
    pub zip: Secret<String>,
    pub country: common_enums::CountryAlpha2,
}

/// Authorize-flow request.
///
/// Non-redirect payment methods settle an existing payment handle via
/// `v1/payments` (`Payment`). Redirect APMs (Skrill, Interac e-Transfer,
/// paysafecard) instead create a payment handle via `v1/paymenthandles`
/// (`PaymentHandle`) so Paysafe returns the customer redirect link. `untagged`
/// so each variant serialises as its bare JSON body (no discriminant wrapper).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaysafeAuthorizeRequest<T: PaymentMethodDataTypes> {
    Payment(Box<PaysafePaymentsRequest>),
    PaymentHandle(Box<PaysafeSetupMandateRequest<T>>),
}

// Type aliases for flows
pub type PaysafePaymentMethodTokenRequest<T> = PaysafeSetupMandateRequest<T>;
pub type PaysafeRepeatPaymentRequest = PaysafePaymentsRequest;
