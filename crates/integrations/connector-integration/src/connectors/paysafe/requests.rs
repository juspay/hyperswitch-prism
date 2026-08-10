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
    // customer_ip and stored_credential serialize as explicit nulls when absent,
    // mirroring hyperswitch's PaysafePaymentsRequest wire shape byte-for-byte
    // (verified via shadow-mode body comparison).
    pub customer_ip: Option<Secret<String>>,
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
    // threeDs/profile/billingDetails serialize as explicit nulls when absent,
    // mirroring hyperswitch's PaysafePaymentHandleRequest wire shape.
    pub three_ds: Option<ThreeDs>,
    pub profile: Option<PaysafeProfile>,
    pub billing_details: Option<PaysafeBillingDetails>,
    /// Paysafe's escape hatch for accounts provisioned `THREE_D_S_TWO`: such an account
    /// rejects a CARD payment handle that carries no `threeDs` block with
    /// `5068 "threeDs may not be null or empty"`. Paysafe's own Google Pay request
    /// examples send `skip3ds: true` for exactly this reason.
    ///
    /// Omitted entirely unless set, so the Skrill / Interac / PreAuthenticate bodies stay
    /// byte-identical to the shadow-verified wire shape.
    ///
    /// NOTE the explicit rename: `rename_all = "camelCase"` would emit `skip3Ds`.
    #[serde(rename = "skip3ds", skip_serializing_if = "Option::is_none")]
    pub skip_3ds: Option<bool>,
}

/// PreAuthenticate (card + 3DS) reuses the payment-handle wire shape; a distinct alias keeps the
/// flow's request type self-documenting.
pub type PaysafePreAuthenticateRequest<T> = PaysafeSetupMandateRequest<T>;

/// Authenticate is a body-less `GET /v1/paymenthandles?merchantRefNum=`; the empty body satisfies
/// the connector macro's request plumbing.
#[derive(Debug, Serialize)]
pub struct PaysafeAuthenticateRequest {}

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
        // Boxed to keep this variant from dominating the enum's size: the decrypted token
        // payload makes it far larger than the others. Mirrors `ApplePay` below. `Box` is
        // transparent to serde, so the wire body is unchanged.
        #[serde(rename = "googlePay")]
        google_pay: Box<PaysafeGooglePay>,
    },
    ApplePay {
        #[serde(rename = "applePay")]
        apple_pay: Box<PaysafeApplePay>,
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
    /// Consumer billing country (hyperswitch parity: sent when billing
    /// country is available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha2>,
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
    /// User-facing label. Hyperswitch sends an explicit null here (parity), so
    /// this serializes even when absent.
    pub label: Option<String>,
    /// Hyperswitch always sends `requestBillingAddress: false`.
    pub request_billing_address: Option<bool>,
    pub apple_pay_payment_token: PaysafeApplePayPaymentToken,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayPaymentToken {
    pub token: PaysafeApplePayToken,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_contact: Option<PaysafeApplePayBillingContact>,
}

/// Apple Pay billing contact forwarded alongside the token, mirroring
/// hyperswitch's `PaysafeApplePayBillingContact` field-for-field.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayBillingContact {
    pub address_lines: Vec<Option<Secret<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrative_area: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub country_code: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locality: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phonetic_family_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phonetic_given_name: Option<Secret<String>>,
    pub postal_code: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_administrative_area: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_locality: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayToken {
    pub payment_data: PaysafeApplePayPaymentData,
    pub payment_method: PaysafeApplePayPaymentMethod,
    pub transaction_identifier: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum PaysafeApplePayPaymentData {
    Encrypted(serde_json::Value),
    Decrypted(PaysafeApplePayDecryptedDataWrapper),
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayDecryptedDataWrapper {
    pub decrypted_data: PaysafeApplePayDecryptedData,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayDecryptedData {
    pub application_primary_account_number: Secret<String>,
    /// PAN expiry in YYMM format (Apple's native representation).
    pub application_expiration_date: Secret<String>,
    /// ISO 4217 alphabetic currency code.
    pub currency_code: String,
    // The optional fields below serialize as explicit nulls when absent,
    // mirroring hyperswitch's decryptedData wire shape.
    pub transaction_amount: Option<MinorUnit>,
    pub cardholder_name: Option<Secret<String>>,
    pub device_manufacturer_identifier: Option<String>,
    pub payment_data_type: Option<String>,
    pub payment_data: PaysafeApplePayDecryptedPaymentData,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeApplePayDecryptedPaymentData {
    pub online_payment_cryptogram: Secret<String>,
    pub eci_indicator: Option<String>,
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
#[serde(untagged)]
pub enum PaysafeGooglePayTokenizationData {
    /// Pre-decrypted token forwarded as `decryptedToken` (upstream decrypted).
    Decrypted {
        /// Always "PAYMENT_GATEWAY"
        #[serde(rename = "type")]
        token_type: String,
        /// The decrypted Google Pay token data
        #[serde(rename = "decryptedToken")]
        decrypted_token: PaysafeGooglePayDecryptedToken,
    },
    /// Raw encrypted Google Pay SDK token passed through for Paysafe to
    /// decrypt gateway-side (requires the merchant's Google Pay
    /// gatewayMerchantId to be provisioned with Paysafe).
    Encrypted {
        /// Always "PAYMENT_GATEWAY"
        #[serde(rename = "type")]
        token_type: String,
        /// The raw `tokenizationData.token` string from the Google Pay SDK
        token: Secret<String>,
    },
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
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<Secret<String>>,
    /// ECI indicator accompanying a network-token cryptogram. Paysafe defines it on the
    /// `tokenWith3DS` schema, so it is only meaningful next to `CRYPTOGRAM_3DS`; a
    /// `PAN_ONLY` token has none and the field is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_indicator: Option<String>,
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

/// Tokenize request body — one of two Paysafe calls:
///
/// `Handle`: mint a new payment handle (card/wallet payload). With a Paysafe
/// customer, cards go straight to the customer vault (MULTI_USE); wallets can
/// only mint SINGLE_USE handles this way (the vault endpoint rejects raw
/// applePay/googlePay objects with 5068 "CARD object must be present").
///
/// `VaultFromHandle`: wallet recurring leg 2 — convert an existing single-use
/// wallet handle into a customer-vaulted MULTI_USE (paymentType CARD) handle
/// via `POST v1/customers/{id}/paymenthandles {paymentHandleTokenFrom}`,
/// mirroring Paysafe's documented Apple Pay / Google Pay recurring flow.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PaysafePaymentMethodTokenRequest<T: PaymentMethodDataTypes> {
    VaultFromHandle(PaysafeVaultFromHandleRequest),
    Handle(Box<PaysafeSetupMandateRequest<T>>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaysafeVaultFromHandleRequest {
    pub merchant_ref_num: String,
    pub payment_handle_token_from: Secret<String>,
}

// Type aliases for flows
pub type PaysafeRepeatPaymentRequest = PaysafePaymentsRequest;
