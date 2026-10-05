use common_utils::types::MinorUnit;
use domain_types::payment_method_data::{PaymentMethodDataTypes, RawCardNumber};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

/// Client Authentication Token request — obtains an OAuth2 access token
/// for client-side SDK initialization via JP Morgan's token endpoint.
/// Uses form-urlencoded format matching the ServerAuthenticationToken flow.
#[derive(Debug, Clone, Serialize)]
pub struct JpmorganClientAuthRequest {
    pub grant_type: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JpmorganTokenRequest {
    pub grant_type: String,
    pub scope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPaymentsRequest<T: PaymentMethodDataTypes> {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
    pub merchant: JpmorganMerchant,
    pub payment_method_type: JpmorganPaymentMethodType<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_holder: Option<JpmorganAccountHolder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<Secret<String>>,
    #[serde(flatten)]
    pub stored_credential: JpmorganStoredCredential,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPaymentMethodType<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<JpmorganCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ach: Option<JpmorganAch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub googlepay: Option<JpmorganGooglePay>,
    /// Token obtained from client-side SDK (CardToken flow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganCard<T: PaymentMethodDataTypes> {
    pub account_number: JpmorganAccountNumber<T>,
    pub expiry: Expiry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number_type: Option<JpmorganAccountNumberType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_provider: Option<JpmorganWalletProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<JpmorganAuthentication>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_network_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_transaction_link_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_authentication_request: Option<JpmorganNativeAuthentication>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_authentication_request: Option<JpmorganNativeAuthentication>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum JpmorganAccountNumber<T: PaymentMethodDataTypes> {
    Card(RawCardNumber<T>),
    Decrypted(cards::CardNumber),
    Token(cards::NetworkToken),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganAccountNumberType {
    Pan,
    DeviceToken,
    NetworkToken,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganWalletProvider {
    ApplePay,
    GooglePay,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAuthentication {
    #[serde(rename = "threeDS", skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<JpmorganThreeDs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_authentication_value: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electronic_commerce_indicator: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganThreeDs {
    pub authentication_value: Secret<String>,
    pub authentication_transaction_id: String,
    #[serde(rename = "threeDSProgramProtocol")]
    pub three_ds_program_protocol: String,
}

/// ACH Bank Debit payment method structure for JPMorgan
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAch {
    pub account_number: Secret<String>,
    pub financial_institution_routing_number: Secret<String>,
    pub account_type: JpmorganAchAccountType,
}

/// ACH Account Holder structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganAccountHolder {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<common_utils::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<JpmorganBillingAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<JpmorganPhone>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganBillingAddress {
    pub line1: Secret<String>,
    pub city: Secret<String>,
    pub postal_code: Secret<String>,
    pub country_code: common_enums::CountryAlpha3,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPhone {
    pub phone_number: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganNativeAuthentication {
    pub authentication_return_url: String,
    #[serde(rename = "threeDSRequestorAuthenticationInfo")]
    pub requestor_info: JpmorganRequestorInfo,
    #[serde(rename = "threeDSPurchaseInfo")]
    pub purchase_info: JpmorganPurchaseInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRequestorInfo {
    pub authentication_purpose: JpmorganAuthenticationPurpose,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganAuthenticationPurpose {
    PaymentTransaction,
    RecurringTransaction,
    AddCard,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPurchaseInfo {
    pub purchase_date: String,
    pub three_domain_secure_transaction_type: JpmorganThreeDsTransactionType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganThreeDsTransactionType {
    GoodsServices,
    Check,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganChallengeWindowSize {
    FullScreen,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganBrowserInfo {
    pub browser_accept_header: String,
    #[serde(rename = "deviceIPAddress")]
    pub device_ip_address: Secret<String>,
    pub browser_language: String,
    pub browser_color_depth: String,
    pub browser_screen_height: String,
    pub browser_screen_width: String,
    pub device_local_time_zone: String,
    pub browser_user_agent: String,
    pub challenge_window_size: JpmorganChallengeWindowSize,
    pub java_enabled: bool,
    pub java_script_enabled: bool,
}

/// ACH Account Type enum
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganAchAccountType {
    Checking,
    Savings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Expiry {
    pub month: Secret<i32>,
    pub year: Secret<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchant {
    pub merchant_software: JpmorganMerchantSoftware,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_merchant: Option<JpmorganSoftMerchant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchantSoftware {
    pub company_name: Secret<String>,
    pub product_name: Secret<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganSoftMerchant {
    pub merchant_purchase_description: Secret<String>,
}

#[derive(Debug, Default, Copy, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CapMethod {
    #[default]
    Now,
    Delayed,
    Manual,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganCaptureRequest {
    pub capture_method: CapMethod,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganVoidRequest {
    // As per the docs, this is not a required field
    // Since we always pass `true` in `isVoid` only during the void call, it makes more sense to have it required field
    pub is_void: bool,
}

/// VoidPC (post-capture void/reversal) request — JPMorgan uses the same PATCH endpoint
/// and the same `{"isVoid": true}` body regardless of whether the payment has been
/// captured or not.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganVoidPcRequest {
    pub is_void: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRefundRequest {
    pub merchant: JpmorganMerchantRefund,
    pub amount: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganMerchantRefund {
    pub merchant_software: JpmorganMerchantSoftware,
}

/// JPMorgan initiator type for stored credentials / MIT
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganInitiatorType {
    Cardholder,
    Merchant,
}

/// JPMorgan account on file status
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganAccountOnFile {
    ToBeStored,
    Stored,
}

/// JPMorgan recurring sequence
#[derive(Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganRecurringSequence {
    First,
    Subsequent,
}

/// JPMorgan recurring object for MIT transactions
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRecurring {
    pub recurring_sequence: JpmorganRecurringSequence,
    pub agreement_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_variable_amount: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_number: Option<u32>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganStoredCredential {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_order_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator_type: Option<JpmorganInitiatorType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_on_file: Option<JpmorganAccountOnFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring: Option<JpmorganRecurring>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_amount_final: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_info: Option<JpmorganBrowserInfo>,
}

/// Non-secret original credential context. Never store payment credentials here.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganContext {
    pub agreement_id: Option<String>,
    pub is_variable_amount: Option<bool>,
    pub recurring_number: Option<u32>,
    #[serde(default)]
    pub scheduled_recurring: bool,
    pub account_number_type: Option<JpmorganAccountNumberType>,
    pub wallet_provider: Option<JpmorganWalletProvider>,
    pub original_network_transaction_id: Option<String>,
    pub original_transaction_link_id: Option<String>,
    pub three_ds_resource: Option<JpmorganThreeDsResource>,
    pub native_capture_method: Option<CapMethod>,
    #[serde(default)]
    pub continue_three_ds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganThreeDsResource {
    pub kind: JpmorganResourceKind,
    pub id: String,
    pub merchant_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JpmorganResourceKind {
    Payment,
    Verification,
}

impl JpmorganResourceKind {
    pub fn path(self) -> &'static str {
        match self {
            Self::Payment => "payments",
            Self::Verification => "verifications",
        }
    }
}

/// Non-financial credential verification. Do not send amount or capture fields.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganSetupMandateRequest<T: PaymentMethodDataTypes> {
    pub currency: common_enums::Currency,
    pub merchant: JpmorganMerchant,
    pub payment_method_type: JpmorganPaymentMethodType<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_sequence: Option<JpmorganRecurringSequence>,
    pub initiator_type: JpmorganInitiatorType,
    pub account_on_file: JpmorganAccountOnFile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_info: Option<JpmorganBrowserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_holder: Option<JpmorganAccountHolder>,
}

pub type JpmorganRepeatPaymentRequest<T> = JpmorganPaymentsRequest<T>;

// ---- Google Pay (encrypted) request structs ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganGooglePay {
    /// Latitude/longitude string required by JPMorgan (e.g. "0,0" when unavailable)
    pub lat_long: String,
    pub encrypted_payment_bundle: JpmorganEncryptedPaymentBundle,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganEncryptedPaymentBundle {
    /// The full signedMessage JSON string from Google Pay (contains encryptedMessage, ephemeralPublicKey, tag)
    pub encrypted_payload: Secret<String>,
    pub encrypted_payment_header: JpmorganEncryptedPaymentHeader,
    /// Maps from intermediateSigningKey.signatures[0] (ECv2) or signature (ECv1) in the Google token
    pub signature: Secret<String>,
    /// e.g. "ECv1" or "ECv2"
    pub protocol_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganEncryptedPaymentHeader {
    /// The ephemeralPublicKey extracted from the Google Pay signedMessage
    pub ephemeral_public_key: Secret<String>,
}

/// Helper structs for deserializing the Google Pay token string
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePayToken {
    pub protocol_version: String,
    pub signature: Secret<String>,
    #[serde(default)]
    pub intermediate_signing_key: Option<GooglePayIntermediateSigningKey>,
    pub signed_message: Secret<String>,
}

#[derive(Debug, Deserialize)]
pub struct GooglePayIntermediateSigningKey {
    pub signatures: Vec<Secret<String>>,
}

/// The parsed signedMessage JSON inside the Google Pay token
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePaySignedMessage {
    pub ephemeral_public_key: Secret<String>,
}
