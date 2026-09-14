use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    pii::Email,
    request::Method,
    types::{FloatMajorUnit, StringMajorUnit},
};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, PostAuthenticate, PreAuthenticate,
        RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{
        ConnectorResponseData, ConnectorSpecificConfig, ExtendedAuthorizationResponseData,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::split_full_name,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

pub(crate) const AIRWALLEX_INTEGRATION_DOC_URL: &str = "https://www.airwallex.com/docs/api";

/// Builds an [`IntegrationErrorContext`] carrying why Airwallex needs the field and what the
/// caller has to change. Without this the merchant only sees "Missing required field: X", which
/// does not say which payment method demanded it or where the value is sourced from.
///
/// [`IntegrationErrorContext`]: domain_types::errors::IntegrationErrorContext
fn aw_err_ctx(
    additional_context: impl Into<String>,
    suggested_action: impl Into<String>,
) -> domain_types::errors::IntegrationErrorContext {
    domain_types::errors::IntegrationErrorContext {
        additional_context: Some(additional_context.into()),
        suggested_action: Some(suggested_action.into()),
        doc_url: Some(AIRWALLEX_INTEGRATION_DOC_URL.to_string()),
    }
}

/// Names the Airwallex payment method a shopper field is being sourced for. The shared field
/// getters below use it to report both the method and the exact JSON path Airwallex nests the
/// field under, so every payment method fails the same way instead of hand-rolling its own
/// message.
#[derive(Clone, Copy)]
struct AirwallexMethodField {
    /// Prose label used in the error message, e.g. `"PayPal"`.
    label: &'static str,
    /// JSON object key Airwallex nests the field under, e.g. `"paypal"` → `paypal.shopper_name`.
    key: &'static str,
}

const AW_PAYPAL: AirwallexMethodField = AirwallexMethodField {
    label: "PayPal",
    key: "paypal",
};
const AW_SKRILL: AirwallexMethodField = AirwallexMethodField {
    label: "Skrill",
    key: "skrill",
};
const AW_KLARNA: AirwallexMethodField = AirwallexMethodField {
    label: "Klarna",
    key: "klarna",
};
const AW_ATOME: AirwallexMethodField = AirwallexMethodField {
    label: "Atome",
    key: "atome",
};
const AW_TRUSTLY: AirwallexMethodField = AirwallexMethodField {
    label: "Trustly",
    key: "trustly",
};
const AW_BLIK: AirwallexMethodField = AirwallexMethodField {
    label: "Blik",
    key: "blik",
};
const AW_ID_BANK_TRANSFER: AirwallexMethodField = AirwallexMethodField {
    label: "Indonesian bank transfer",
    key: "bank_transfer",
};

/// Appends an optional payment-method-specific clause to a shared suggested action, so the common
/// wording stays in one place while Klarna's market list or the Indonesian `ID` requirement can
/// still be spelled out.
fn aw_suggestion(base: &str, note: Option<&str>) -> String {
    match note {
        Some(note) => format!("{base}. {note}"),
        None => base.to_string(),
    }
}

/// `shopper_name` for payment methods that take the explicit customer name and fall back to the
/// billing full name, mirroring the reference connector's sourcing.
fn get_shopper_name(
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
    method: AirwallexMethodField,
    note: Option<&str>,
) -> Result<Secret<String>, IntegrationError> {
    customer_name
        .or_else(|| resource_common_data.get_billing_full_name().ok())
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            // `field_name` is the caller-facing request path, never the Airwallex JSON key: it is
            // what a merchant has to change and what field-probe resolves through
            // `patch-config.toml`. The Airwallex-side name lives in `additional_context` below.
            field_name: "billing.first_name",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} requires {}.shopper_name, sourced from the customer name or, \
                     failing that, billing.address first_name + last_name",
                    method.label, method.key
                ),
                aw_suggestion(
                    "Send customer.name, or both billing.address.first_name and \
                     billing.address.last_name, on the payment request",
                    note,
                ),
            ),
        })
}

/// `shopper_name` for payment methods that only ever source it from the billing address. Kept
/// separate from [`get_shopper_name`] so the message never advertises `customer.name` as a source
/// for a flow that does not read it.
fn get_billing_shopper_name(
    resource_common_data: &PaymentFlowData,
    method: AirwallexMethodField,
    note: Option<&str>,
) -> Result<Secret<String>, IntegrationError> {
    resource_common_data.get_billing_full_name().map_err(|_| {
        IntegrationError::MissingRequiredField {
            field_name: "billing.first_name",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} requires {}.shopper_name, sourced from billing.address \
                     first_name + last_name",
                    method.label, method.key
                ),
                aw_suggestion(
                    "Send both billing.address.first_name and billing.address.last_name on the \
                     payment request",
                    note,
                ),
            ),
        }
    })
}

/// `shopper_email`, sourced from the billing email.
fn get_shopper_email(
    resource_common_data: &PaymentFlowData,
    method: AirwallexMethodField,
    note: Option<&str>,
) -> Result<Email, IntegrationError> {
    resource_common_data
        .get_billing_email()
        .map_err(|_| IntegrationError::MissingRequiredField {
            field_name: "billing.email",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} requires {}.shopper_email; it is sourced from billing.email",
                    method.label, method.key
                ),
                aw_suggestion("Send billing.email on the payment request", note),
            ),
        })
}

/// `country_code`, sourced from the billing country.
fn get_country_code(
    resource_common_data: &PaymentFlowData,
    method: AirwallexMethodField,
    note: Option<&str>,
) -> Result<common_enums::CountryAlpha2, IntegrationError> {
    resource_common_data
        .get_billing_country()
        .map_err(|_| IntegrationError::MissingRequiredField {
            field_name: "billing.country",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} requires {}.country_code, sourced from billing.address.country",
                    method.label, method.key
                ),
                aw_suggestion(
                    "Send billing.address.country as a two-letter ISO 3166-1 alpha-2 code (e.g. \
                     GB, DE) on the payment request",
                    note,
                ),
            ),
        })
}

/// `shopper_phone` in full international form. Airwallex needs the country code and the number
/// together, so the two sourcing failures are reported separately.
fn get_shopper_phone_with_country_code(
    resource_common_data: &PaymentFlowData,
    method: AirwallexMethodField,
) -> Result<Secret<String>, IntegrationError> {
    resource_common_data
        .get_billing_phone()
        .map_err(|_| IntegrationError::MissingRequiredField {
            field_name: "billing.phone",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} requires {}.shopper_phone; it is sourced from billing.phone",
                    method.label, method.key
                ),
                "Send billing.phone.number on the payment request",
            ),
        })?
        .get_number_with_country_code()
        .map_err(|_| IntegrationError::MissingRequiredField {
            field_name: "billing.phone.country_code",
            context: aw_err_ctx(
                format!(
                    "Airwallex {} needs the shopper phone in full international form, so \
                     billing.phone must carry a country code alongside the number",
                    method.label
                ),
                "Send billing.phone.country_code (e.g. 65) together with billing.phone.number",
            ),
        })
}

#[derive(Debug, Clone)]
pub struct AirwallexAuthType {
    pub api_key: Secret<String>,
    pub client_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for AirwallexAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Airwallex {
            api_key, client_id, ..
        } = auth_type
        {
            Ok(Self {
                api_key: api_key.clone(),
                client_id: client_id.clone(),
            })
        } else {
            Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirwallexErrorResponse {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirwallexAccessTokenResponse {
    pub token: Secret<String>,
    #[serde(with = "common_utils::custom_serde::iso8601")]
    pub expires_at: time::PrimitiveDateTime,
}

// Empty request body for ServerAuthenticationToken - Airwallex requires empty JSON object {}
#[derive(Debug, Serialize)]
pub struct AirwallexAccessTokenRequest {
    // Empty struct that serializes to {} - Airwallex API requirement
}

// New unified request type for macro pattern that includes payment intent creation and confirmation
#[derive(Debug, Serialize)]
pub struct AirwallexPaymentRequest {
    // Request ID for confirm request
    pub request_id: String,
    // Payment method data for confirm step
    pub payment_method: AirwallexPaymentMethod,
    // Options for payment processing. Skipped when absent so a non-card intent does not ship
    // `"payment_method_options": null` — Airwallex treats the key as present-but-empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    pub return_url: Option<String>,
    // Device data for fraud detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_data: Option<AirwallexDeviceData>,
    // CIT (setup_future_usage) only: set up an Airwallex PaymentConsent so the confirm response
    // returns a payment_consent_id usable as the connector mandate for future MITs. Omitted for
    // one-off payments and MITs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_consent: Option<AirwallexPaymentConsentData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

/// Request body for the Authorize flow.
///
/// On [`AIRWALLEX_API_VERSION`] a 3DS card payment is exactly two Airwallex calls: this
/// `POST /pa/payment_intents/{id}/confirm`, and — after the shopper returns from the
/// Airwallex-hosted 3DS page — a bodyless `GET /pa/payment_intents/{id}`. The old
/// `confirm_continue` continuation body (`{request_id, type:"3ds_continue", three_ds:{…}}`) was
/// removed from the Airwallex contract on `2024-06-14`, so there is no second request shape to
/// model: the return leg carries no body at all and is built in `Airwallex::build_request_v2`.
///
/// [`AIRWALLEX_API_VERSION`]: super::AIRWALLEX_API_VERSION
pub type AirwallexAuthorizeRequest = AirwallexPaymentRequest;

/// Native-3DS block nested at `payment_method.card.three_ds`.
///
/// Required by Airwallex on `[2024-02-22, 2026-06-30)` for any card confirm that may run 3DS:
/// it is where the shopper is sent once the hosted authentication page finishes. Mutually
/// exclusive with [`AirwallexExternalThreeDs`] — sending both is a `validation_error`.
#[derive(Debug, Serialize)]
pub struct AirwallexCardThreeDs {
    pub return_url: String,
}

/// External-3DS pass-through block nested at `payment_method.card.external_three_ds`.
///
/// Used when the cardholder was already authenticated by a router-side 3DS provider and UCS is
/// only relaying the proof. Note the CAVV travels as `authentication_value`; there is no `cavv`
/// and no `xid` field on the request — those two names exist only on the *response* `ds_data`.
///
/// Airwallex's own description of `three_ds_action` says this block lives at
/// `payment_method_options.card.external_three_ds`. That prose is wrong: the schema (and the
/// 3DS guide's worked example) both place it under `payment_method.card`, as a sibling of
/// `number` and `cvc`.
#[derive(Debug, Serialize)]
pub struct AirwallexExternalThreeDs {
    /// The 3DS2 CAVV / 3DS1 AAV. Secret: it is a single-use authorisation credential.
    pub authentication_value: Secret<String>,
    /// Two-character ECI from the ACS or the directory server.
    pub eci: String,
    pub ds_transaction_id: Secret<String>,
    pub three_ds_server_transaction_id: Secret<String>,
    /// 3DS version as `major.minor.patch`, rendered from the caller's `SemanticVersion`.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds_exemption: Option<AirwallexThreeDsExemption>,
}

/// The three exemption values Airwallex accepts on `external_three_ds.three_ds_exemption`.
/// Every other UCS [`common_enums::ExemptionIndicator`] variant has no Airwallex counterpart and
/// the field is omitted rather than guessed — a wrong exemption claim shifts liability back to
/// the merchant.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexThreeDsExemption {
    Tra,
    Lvp,
    Anonymous,
}

/// How the card confirm should handle 3DS. The two Airwallex blocks are mutually exclusive, so
/// they are modelled as one value rather than two independently-settable `Option`s.
///
/// Neither variant sets `payment_method_options.card.three_ds_action`, and that is deliberate.
/// On the pinned API version `three_ds_action` is read only by
/// `POST /pa/payment_intents/create`; `/confirm` does not declare it, and Airwallex ignores
/// unknown keys silently rather than rejecting them, so setting it there does nothing at all.
/// It is not set on create either, because none of its three values is derivable there safely:
///
/// * `EXTERNAL_3DS` is the one that belongs with [`Self::External`], but `PaymentCreateOrderData`
///   carries no `authentication_data`, so CreateOrder cannot know an external proof is coming.
/// * `SKIP_3DS` and `EXTERNAL_3DS` are both entitlement-gated. On an account without them
///   Airwallex fails the *create* with `validation_error` ("Skip 3DS is not allowed. Contact the
///   support team if you wish to use this option."), which would turn a payment that merely
///   wanted to skip 3DS into a payment that cannot be created at all.
/// * `FORCE_3DS` would make Airwallex run its own 3DS on top of a relayed external proof.
///
/// Omitting the key is Airwallex's documented fourth behaviour — "use the default global 3DS
/// settings" — which is what this connector has always relied on. Consequence for
/// [`Self::External`]: the relayed proof is honoured only on accounts Airwallex has enabled
/// External 3DS for and whose global settings select it.
#[derive(Debug)]
pub enum AirwallexThreeDsMode {
    /// Airwallex runs 3DS itself and returns the shopper to `return_url`.
    Native { return_url: String },
    /// A router-side 3DS provider already authenticated; relay the proof.
    External(Box<AirwallexExternalThreeDs>),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexPaymentMethod {
    Card(AirwallexCardData),
    Wallets(AirwallexWalletData),
    BankRedirect(AirwallexBankRedirectData),
    PayLater(AirwallexPayLaterData),
    BankTransfer(AirwallexBankTransferData),
}

// Shared Airwallex BankTransfer enum. Each bank-transfer payment method gets its own variant so
// the connector serializes the correct nested object + `type` discriminator, mirroring the
// reference upstream `AirwallexBankTransferData::IndonesianBankTransfer(IndonesianBankTransferData)`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexBankTransferData {
    IndonesianBankTransfer(IndonesianBankTransferData),
}

#[derive(Debug, Serialize)]
pub struct IndonesianBankTransferData {
    pub bank_transfer: IndonesianBankTransferDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct IndonesianBankTransferDetails {
    pub shopper_name: Secret<String>,
    pub shopper_email: Email,
    // The Airwallex bank token (e.g. "mandiri", "cimb_niaga"), mapped from the
    // domain `BankNames` via `AirwallexIndonesianBankName` — the raw serde string
    // of `BankNames` does not match Airwallex's tokens.
    pub bank_name: String,
    pub country_code: common_enums::CountryAlpha2,
}

// Maps the domain `BankNames` to the exact Airwallex Indonesian bank_transfer token.
// Tokens sourced from Airwallex `/pa/config/banks?payment_method_type=bank_transfer&country_code=ID`.
// Banks Airwallex does not support for Indonesia are rejected as NotImplemented.
pub struct AirwallexIndonesianBankName(String);

impl TryFrom<&common_enums::BankNames> for AirwallexIndonesianBankName {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: &common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankNames::BankMandiri => Ok(Self("mandiri".to_string())),
            common_enums::BankNames::BankDanamon => Ok(Self("danamon".to_string())),
            common_enums::BankNames::BankNegaraIndonesia => Ok(Self("bni".to_string())),
            common_enums::BankNames::BankRakyatIndonesia => Ok(Self("bri".to_string())),
            common_enums::BankNames::CimbNiaga => Ok(Self("cimb_niaga".to_string())),
            common_enums::BankNames::Maybank => Ok(Self("maybank".to_string())),
            common_enums::BankNames::PermataBank => Ok(Self("permata".to_string())),
            // The payment method itself is supported — only this bank is not — so the generic
            // "Selected payment method through airwallex" message would point at the wrong thing.
            _ => Err(error_stack::report!(IntegrationError::NotImplemented(
                "Selected bank for the Airwallex Indonesian bank transfer is not supported. \
                 Airwallex accepts bank_mandiri, bank_danamon, bank_negara_indonesia, \
                 bank_rakyat_indonesia, cimb_niaga, maybank or permata_bank"
                    .to_string(),
                Default::default()
            ))),
        }
    }
}

// Shared Airwallex PayLater enum. Each PayLater payment method gets its own variant so
// the connector serializes the correct nested object + `type` discriminator, mirroring the
// reference upstream `AirwallexPayLaterData::{Klarna(Box<KlarnaData>), Atome(AtomeData)}`.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexPayLaterData {
    Klarna(Box<AirwallexKlarnaData>),
    Atome(AirwallexAtomeData),
}

#[derive(Debug, Serialize)]
pub struct AirwallexKlarnaData {
    pub klarna: AirwallexKlarnaDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexKlarnaDetails {
    pub country_code: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<AirwallexKlarnaBilling>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexKlarnaBilling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AirwallexPayLaterAddress>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPayLaterAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<common_enums::CountryAlpha2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postcode: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexAtomeData {
    pub atome: AirwallexAtomeDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexAtomeDetails {
    pub shopper_phone: Secret<String>,
}

// Shared Airwallex wallet enum. Each wallet payment method gets its own variant so
// the connector serializes the correct nested object + `type` discriminator.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexWalletData {
    GooglePay(AirwallexGooglePayData),
    Paypal(AirwallexPaypalData),
    Skrill(AirwallexSkrillData),
}

#[derive(Debug, Serialize)]
pub struct AirwallexGooglePayData {
    pub googlepay: AirwallexGooglePayDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexGooglePayDetails {
    pub encrypted_payment_token: Secret<String>,
    pub payment_data_type: AirwallexGpayPaymentDataType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexGpayPaymentDataType {
    EncryptedPaymentToken,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPaypalData {
    pub paypal: AirwallexPaypalDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPaypalDetails {
    pub shopper_name: Secret<String>,
    pub country_code: common_enums::CountryAlpha2,
}

#[derive(Debug, Serialize)]
pub struct AirwallexSkrillData {
    pub skrill: AirwallexSkrillDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexSkrillDetails {
    pub shopper_name: Secret<String>,
    pub shopper_email: Email,
    pub country_code: common_enums::CountryAlpha2,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum AirwallexBankRedirectData {
    Ideal(AirwallexIdealData),
    Trustly(AirwallexTrustlyData),
    Blik(AirwallexBlikData),
}

// Removed old AirwallexPaymentMethodData enum - now using individual Option fields for cleaner serialization

#[derive(Debug, Serialize)]
pub struct AirwallexCardData {
    pub card: AirwallexCardDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardDetails {
    pub number: Secret<String>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvc: Secret<String>,
    pub name: Option<Secret<String>>,
    /// Native 3DS return URL. Present for every card confirm that may run Airwallex-hosted 3DS;
    /// absent when `external_three_ds` is sent (the two are mutually exclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<AirwallexCardThreeDs>,
    /// External 3DS pass-through proof; see [`AirwallexExternalThreeDs`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_three_ds: Option<Box<AirwallexExternalThreeDs>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexPaymentType {
    Card,
    Googlepay,
    Paypal,
    Klarna,
    Atome,
    Trustly,
    Blik,
    Ideal,
    Skrill,
    BankTransfer,
}

// BankRedirect-specific data structures
#[derive(Debug, Serialize)]
pub struct AirwallexIdealData {
    pub ideal: AirwallexIdealDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexIdealDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<common_enums::BankNames>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexTrustlyData {
    pub trustly: AirwallexTrustlyDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexTrustlyDetails {
    pub shopper_name: Secret<String>,
    pub country_code: common_enums::CountryAlpha2,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBlikData {
    pub blik: AirwallexBlikDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBlikDetails {
    pub shopper_name: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexDeviceData {
    pub accept_header: String,
    pub browser: AirwallexBrowser,
    pub ip_address: Option<Secret<String>>,
    pub language: String,
    pub mobile: Option<AirwallexMobile>,
    pub screen_color_depth: u8,
    pub screen_height: u32,
    pub screen_width: u32,
    pub timezone: String,
}

#[derive(Debug, Serialize)]
pub struct AirwallexBrowser {
    pub java_enabled: bool,
    pub javascript_enabled: bool,
    pub user_agent: String,
}

#[derive(Debug, Serialize)]
pub struct AirwallexMobile {
    pub device_model: Option<String>,
    pub os_type: Option<String>,
    pub os_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPaymentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<AirwallexCardOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub klarna: Option<AirwallexPayLaterOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atome: Option<AirwallexPayLaterOptions>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardOptions {
    pub auto_capture: Option<bool>,
    // Omitted entirely unless extended authorization was requested, so ordinary
    // card payments keep the exact request body they had before this field existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_type: Option<AirwallexCardAuthorizationType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexCardAuthorizationType {
    PreAuth,
    FinalAuth,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPayLaterOptions {
    pub auto_capture: Option<bool>,
}

/// Builds Airwallex's `device_data` block from the caller's browser information.
///
/// Takes the `BrowserInformation` directly rather than a flow-specific request type so the
/// `Authorize` and `PreAuthenticate` legs send byte-identical device data. `None` in means no
/// `device_data` key on the wire — Airwallex treats it as optional.
fn get_device_data(
    browser_info: Option<&domain_types::router_request_types::BrowserInformation>,
) -> Option<AirwallexDeviceData> {
    let browser_info = browser_info?;

    let browser = AirwallexBrowser {
        java_enabled: browser_info.get_java_enabled().unwrap_or(false),
        javascript_enabled: browser_info.get_java_script_enabled().unwrap_or(true),
        user_agent: browser_info.get_user_agent().unwrap_or_default(),
    };

    let mobile = {
        let device_model = browser_info.device_model.clone();
        let os_type = browser_info.os_type.clone();
        let os_version = browser_info.os_version.clone();

        if device_model.is_some() || os_type.is_some() || os_version.is_some() {
            Some(AirwallexMobile {
                device_model,
                os_type,
                os_version,
            })
        } else {
            None
        }
    };

    Some(AirwallexDeviceData {
        accept_header: browser_info.get_accept_header().unwrap_or_default(),
        browser,
        ip_address: browser_info
            .get_ip_address()
            .ok()
            .map(|ip| Secret::new(ip.expose().to_string())),
        language: browser_info.get_language().unwrap_or_default(),
        mobile,
        screen_color_depth: browser_info.get_color_depth().unwrap_or(24),
        screen_height: browser_info.get_screen_height().unwrap_or(1080),
        screen_width: browser_info.get_screen_width().unwrap_or(1920),
        timezone: browser_info
            .get_time_zone()
            .map(|tz| tz.to_string())
            .unwrap_or_else(|_| "0".to_string()),
    })
}

/// Shared Card conversion used by every builder that puts a card on the Airwallex wire
/// (`Authorize`, `SetupMandate`, `PreAuthenticate`) so the paths cannot drift.
///
/// `three_ds` selects the mutually-exclusive 3DS block Airwallex nests under
/// `payment_method.card`; pass `None` only when the caller has established that no 3DS block
/// belongs on this request.
fn get_card_details<T: PaymentMethodDataTypes>(
    card_data: &domain_types::payment_method_data::Card<T>,
    three_ds: Option<AirwallexThreeDsMode>,
) -> AirwallexPaymentMethod {
    let (three_ds, external_three_ds) = match three_ds {
        Some(AirwallexThreeDsMode::Native { return_url }) => {
            (Some(AirwallexCardThreeDs { return_url }), None)
        }
        Some(AirwallexThreeDsMode::External(external)) => (None, Some(external)),
        None => (None, None),
    };
    AirwallexPaymentMethod::Card(AirwallexCardData {
        card: AirwallexCardDetails {
            number: Secret::new(card_data.card_number.peek().to_string()),
            expiry_month: card_data.card_exp_month.clone(),
            expiry_year: card_data.get_expiry_year_4_digit(),
            cvc: card_data.card_cvc.clone(),
            name: card_data
                .card_holder_name
                .clone()
                .map(|name| Secret::new(name.expose())),
            three_ds,
            external_three_ds,
        },
        payment_method_type: AirwallexPaymentType::Card,
    })
}

/// Turns router-side 3DS proof into Airwallex's `external_three_ds` block.
///
/// Every field Airwallex marks required for 3DS2 is sourced from exactly one place in
/// [`AuthenticationData`] and fails locally with `MissingRequiredField` when absent — a partial
/// `external_three_ds` is rejected by Airwallex with a `validation_error` that names a field the
/// merchant cannot act on.
///
/// [`AuthenticationData`]: domain_types::router_request_types::AuthenticationData
fn build_external_three_ds(
    authentication_data: &domain_types::router_request_types::AuthenticationData,
) -> Result<Box<AirwallexExternalThreeDs>, error_stack::Report<IntegrationError>> {
    let missing = |field_name: &'static str, airwallex_field: &str| {
        error_stack::report!(IntegrationError::MissingRequiredField {
            field_name,
            context: aw_err_ctx(
                format!(
                    "Airwallex external 3DS requires \
                     payment_method.card.external_three_ds.{airwallex_field}; it is only \
                     accepted as a complete 3DS2 proof"
                ),
                "Send the full authentication result from your 3DS provider in \
                 authentication_data (cavv, eci, ds_trans_id, threeds_server_transaction_id, \
                 message_version), or drop authentication_data entirely and let Airwallex run \
                 3DS itself",
            ),
        })
    };

    Ok(Box::new(AirwallexExternalThreeDs {
        authentication_value: authentication_data
            .cavv
            .clone()
            .ok_or_else(|| missing("authentication_data.cavv", "authentication_value"))?,
        eci: authentication_data
            .eci
            .clone()
            .ok_or_else(|| missing("authentication_data.eci", "eci"))?,
        ds_transaction_id: authentication_data
            .ds_trans_id
            .clone()
            .map(Secret::new)
            .ok_or_else(|| missing("authentication_data.ds_trans_id", "ds_transaction_id"))?,
        three_ds_server_transaction_id: authentication_data
            .threeds_server_transaction_id
            .clone()
            .map(Secret::new)
            .ok_or_else(|| {
                missing(
                    "authentication_data.threeds_server_transaction_id",
                    "three_ds_server_transaction_id",
                )
            })?,
        version: authentication_data
            .message_version
            .as_ref()
            .map(|version| version.to_string())
            .ok_or_else(|| missing("authentication_data.message_version", "version"))?,
        // Only TRA and LVP have Airwallex counterparts. Everything else is omitted rather than
        // approximated — claiming the wrong exemption moves fraud liability to the merchant.
        three_ds_exemption: authentication_data.exemption_indicator.as_ref().and_then(
            |exemption| match exemption {
                common_enums::ExemptionIndicator::TransactionRiskAssessment => {
                    Some(AirwallexThreeDsExemption::Tra)
                }
                common_enums::ExemptionIndicator::LowValue => Some(AirwallexThreeDsExemption::Lvp),
                common_enums::ExemptionIndicator::SecureCorporatePayment
                | common_enums::ExemptionIndicator::TrustedListing
                | common_enums::ExemptionIndicator::ThreeDsOutage
                | common_enums::ExemptionIndicator::ScaDelegation
                | common_enums::ExemptionIndicator::OutOfScaScope
                | common_enums::ExemptionIndicator::LowRiskProgram
                | common_enums::ExemptionIndicator::RecurringOperation
                | common_enums::ExemptionIndicator::Other => None,
            },
        ),
    }))
}

// Shared BankRedirect conversion used by every Airwallex request builder so the paths cannot
// drift. iDeal only carries
// the issuer bank; Trustly and Blik additionally need the shopper name (and, for Trustly, the
// billing country).
fn get_bankredirect_details(
    bank_redirect_data: &domain_types::payment_method_data::BankRedirectData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match bank_redirect_data {
        domain_types::payment_method_data::BankRedirectData::Ideal { bank_name } => {
            Ok(AirwallexPaymentMethod::BankRedirect(
                AirwallexBankRedirectData::Ideal(AirwallexIdealData {
                    ideal: AirwallexIdealDetails {
                        bank_name: *bank_name,
                    },
                    payment_method_type: AirwallexPaymentType::Ideal,
                }),
            ))
        }
        domain_types::payment_method_data::BankRedirectData::Trustly { .. } => {
            Ok(AirwallexPaymentMethod::BankRedirect(
                AirwallexBankRedirectData::Trustly(AirwallexTrustlyData {
                    trustly: AirwallexTrustlyDetails {
                        shopper_name: get_billing_shopper_name(
                            resource_common_data,
                            AW_TRUSTLY,
                            None,
                        )?,
                        country_code: get_country_code(resource_common_data, AW_TRUSTLY, None)?,
                    },
                    payment_method_type: AirwallexPaymentType::Trustly,
                }),
            ))
        }
        domain_types::payment_method_data::BankRedirectData::Blik { blik_code: _ } => {
            Ok(AirwallexPaymentMethod::BankRedirect(
                AirwallexBankRedirectData::Blik(AirwallexBlikData {
                    blik: AirwallexBlikDetails {
                        shopper_name: get_billing_shopper_name(
                            resource_common_data,
                            AW_BLIK,
                            None,
                        )?,
                    },
                    payment_method_type: AirwallexPaymentType::Blik,
                }),
            ))
        }
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            "Bank Redirect Payment Method".to_string(),
            Default::default()
        ))),
    }
}

// Shared wallet conversion used by every Airwallex request builder so the paths cannot drift.
fn get_wallet_details(
    wallet_data: &domain_types::payment_method_data::WalletData,
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match wallet_data {
        domain_types::payment_method_data::WalletData::GooglePay(gpay_details) => {
            let token = gpay_details
                .tokenization_data
                .get_encrypted_google_pay_token()
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_data.wallet.google_pay.tokenization_data",
                    context: aw_err_ctx(
                        "Airwallex Google Pay requires the encrypted Google Pay token from \
                         payment_method_data.wallet.google_pay.tokenization_data",
                        "Send the raw PaymentData token returned by the Google Pay API in \
                         tokenization_data; it must be the encrypted `token` string, not an \
                         already-decrypted or empty payload",
                    ),
                })
                .attach_printable("Failed to get gpay wallet token")?;
            Ok(AirwallexPaymentMethod::Wallets(
                AirwallexWalletData::GooglePay(AirwallexGooglePayData {
                    googlepay: AirwallexGooglePayDetails {
                        encrypted_payment_token: Secret::new(token),
                        payment_data_type: AirwallexGpayPaymentDataType::EncryptedPaymentToken,
                    },
                    payment_method_type: AirwallexPaymentType::Googlepay,
                }),
            ))
        }
        domain_types::payment_method_data::WalletData::PaypalRedirect(_) => {
            let shopper_name =
                get_shopper_name(resource_common_data, customer_name, AW_PAYPAL, None)?;
            let country_code = get_country_code(resource_common_data, AW_PAYPAL, None)?;
            Ok(AirwallexPaymentMethod::Wallets(
                AirwallexWalletData::Paypal(AirwallexPaypalData {
                    paypal: AirwallexPaypalDetails {
                        shopper_name,
                        country_code,
                    },
                    payment_method_type: AirwallexPaymentType::Paypal,
                }),
            ))
        }
        domain_types::payment_method_data::WalletData::Skrill(_) => {
            let shopper_name =
                get_shopper_name(resource_common_data, customer_name, AW_SKRILL, None)?;
            let shopper_email = get_shopper_email(
                resource_common_data,
                AW_SKRILL,
                Some("Airwallex uses it to identify the Skrill wallet account"),
            )?;
            let country_code = get_country_code(resource_common_data, AW_SKRILL, None)?;
            Ok(AirwallexPaymentMethod::Wallets(
                AirwallexWalletData::Skrill(AirwallexSkrillData {
                    skrill: AirwallexSkrillDetails {
                        shopper_name,
                        shopper_email,
                        country_code,
                    },
                    payment_method_type: AirwallexPaymentType::Skrill,
                }),
            ))
        }
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            "Wallet Payment Method".to_string(),
            Default::default()
        ))),
    }
}

// Shared PayLater conversion used by every Airwallex request builder so the paths cannot
// drift. Mirrors the
// reference upstream `get_paylater_details`: Klarna carries billing details + country code,
// Atome carries the shopper phone with country code.
fn get_paylater_details(
    paylater_data: &domain_types::payment_method_data::PayLaterData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match paylater_data {
        domain_types::payment_method_data::PayLaterData::KlarnaRedirect {} => {
            let country_code = get_country_code(
                resource_common_data,
                AW_KLARNA,
                Some(
                    "Airwallex uses it to select the Klarna market, so it must be one Klarna \
                      supports (e.g. GB, DE, SE)",
                ),
            )?;
            Ok(AirwallexPaymentMethod::PayLater(
                AirwallexPayLaterData::Klarna(Box::new(AirwallexKlarnaData {
                    klarna: AirwallexKlarnaDetails {
                        country_code,
                        billing: Some(AirwallexKlarnaBilling {
                            date_of_birth: None,
                            email: resource_common_data.get_optional_billing_email(),
                            first_name: resource_common_data.get_optional_billing_first_name(),
                            last_name: resource_common_data.get_optional_billing_last_name(),
                            phone_number: resource_common_data.get_optional_billing_phone_number(),
                            address: Some(AirwallexPayLaterAddress {
                                country_code: resource_common_data.get_optional_billing_country(),
                                city: resource_common_data.get_optional_billing_city(),
                                street: resource_common_data.get_optional_billing_line1(),
                                postcode: resource_common_data.get_optional_billing_zip(),
                            }),
                        }),
                    },
                    payment_method_type: AirwallexPaymentType::Klarna,
                })),
            ))
        }
        domain_types::payment_method_data::PayLaterData::AtomeRedirect {} => {
            let shopper_phone =
                get_shopper_phone_with_country_code(resource_common_data, AW_ATOME)?;
            Ok(AirwallexPaymentMethod::PayLater(
                AirwallexPayLaterData::Atome(AirwallexAtomeData {
                    atome: AirwallexAtomeDetails { shopper_phone },
                    payment_method_type: AirwallexPaymentType::Atome,
                }),
            ))
        }
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            crate::utils::get_unimplemented_payment_method_error_message("airwallex"),
            Default::default()
        ))),
    }
}

// Shared BankTransfer conversion used by every Airwallex request builder so the paths cannot
// drift. Mirrors the
// reference upstream `get_banktransfer_details`: the Indonesian bank transfer carries the
// shopper name/email, the selected bank, and the billing country code.
fn get_banktransfer_details(
    banktransfer_data: &domain_types::payment_method_data::BankTransferData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match banktransfer_data {
        domain_types::payment_method_data::BankTransferData::IndonesianBankTransfer {
            bank_name,
        } => Ok(AirwallexPaymentMethod::BankTransfer(
            AirwallexBankTransferData::IndonesianBankTransfer(IndonesianBankTransferData {
                bank_transfer: IndonesianBankTransferDetails {
                    shopper_name: get_billing_shopper_name(
                        resource_common_data,
                        AW_ID_BANK_TRANSFER,
                        None,
                    )?,
                    shopper_email: get_shopper_email(
                        resource_common_data,
                        AW_ID_BANK_TRANSFER,
                        Some("Airwallex delivers the virtual account instructions to it"),
                    )?,
                    // `bank_name` is required by Airwallex to route the Indonesian bank transfer;
                    // map the domain bank to Airwallex's exact token (rejecting unsupported banks).
                    bank_name: AirwallexIndonesianBankName::try_from(bank_name.as_ref().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "payment_method_data.bank_transfer.bank_name",
                            context: aw_err_ctx(
                                "Airwallex routes the Indonesian bank transfer to a specific \
                                 issuer, so bank_transfer.bank_name cannot be inferred",
                                "Send payment_method_data.bank_transfer.bank_name with one of \
                                 the Indonesian banks Airwallex supports: bank_mandiri, \
                                 bank_danamon, bank_negara_indonesia, bank_rakyat_indonesia, \
                                 cimb_niaga, maybank, permata_bank",
                            ),
                        },
                    )?)?
                    .0,
                    country_code: get_country_code(
                        resource_common_data,
                        AW_ID_BANK_TRANSFER,
                        Some("For the Indonesian bank transfer that country is ID"),
                    )?,
                },
                payment_method_type: AirwallexPaymentType::BankTransfer,
            }),
        )),
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            crate::utils::get_unimplemented_payment_method_error_message("airwallex"),
            Default::default()
        ))),
    }
}

/// Whether this Authorize call is the card 3DS return leg — the invocation that happens *after*
/// the shopper has come back from the Airwallex-hosted 3DS page.
///
/// On [`AIRWALLEX_API_VERSION`] that leg is a **read**, not a second charge: Airwallex removed
/// `three_ds` and the `3ds_continue` type from `confirm_continue` on `2024-06-14`, and its own
/// guide says the merchant must "Retrieve PaymentIntent to confirm status". So this predicate
/// selects a bodyless `GET /pa/payment_intents/{id}` instead of the `/confirm` POST. It is the
/// reason the composite `Authorize` that runs after `PostAuthenticate` cannot double-charge.
///
/// Gated on the payment method being a card **and** HS having supplied a `redirect_response`.
/// In practice only cards come back through Authorize, because the return URL routes cards to
/// `complete_authorize_url` and APMs to `router_return_url` — but that is an emergent property of
/// a different branch. Keying the endpoint and the HTTP method off one explicit condition means
/// an APM that ever did return here still gets its normal confirm POST.
///
/// Used by `get_url` and `build_request_v2` in `airwallex.rs`, so the URL and the method cannot
/// disagree about which leg is being sent.
///
/// [`AIRWALLEX_API_VERSION`]: super::AIRWALLEX_API_VERSION
pub(crate) fn is_card_three_ds_continue<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
        && matches!(
            request.payment_method_data,
            domain_types::payment_method_data::PaymentMethodData::Card(_)
        )
}

/// Single entry point for turning the domain payment method into the Airwallex payload, shared by
/// the `Authorize` and `PreAuthenticate` request builders so the request paths cannot drift.
///
/// `three_ds` is only meaningful on the card arm; Airwallex has no 3DS block for wallets, bank
/// redirects, pay-later or bank transfers, which carry their own redirect contract.
fn get_payment_method_details<T: PaymentMethodDataTypes>(
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
    three_ds: Option<AirwallexThreeDsMode>,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
            Ok(get_card_details(card_data, three_ds))
        }
        domain_types::payment_method_data::PaymentMethodData::BankRedirect(bank_redirect_data) => {
            get_bankredirect_details(bank_redirect_data, resource_common_data)
        }
        domain_types::payment_method_data::PaymentMethodData::Wallet(wallet_data) => {
            get_wallet_details(wallet_data, resource_common_data, customer_name)
        }
        domain_types::payment_method_data::PaymentMethodData::PayLater(paylater_data) => {
            get_paylater_details(paylater_data, resource_common_data)
        }
        domain_types::payment_method_data::PaymentMethodData::BankTransfer(banktransfer_data) => {
            get_banktransfer_details(banktransfer_data, resource_common_data)
        }
        _ => Err(error_stack::report!(IntegrationError::NotImplemented(
            "Payment Method".to_string(),
            Default::default()
        ))),
    }
}

// Build the correct `payment_method_options` object for the selected payment method.
// Card/Wallet/BankRedirect keep the historical card options; PayLater emits its own
// klarna/atome options block with `auto_capture`, mirroring the reference upstream.
//
// `three_ds_action` is deliberately NOT part of this block. Airwallex only reads it on
// `POST /pa/payment_intents/create`; on `/confirm` it is an unknown key, and Airwallex silently
// ignores unknown keys rather than rejecting them. See the note on [`AirwallexThreeDsMode`].
fn build_payment_method_options(
    payment_method: &AirwallexPaymentMethod,
    auto_capture: bool,
    authorization_type: Option<AirwallexCardAuthorizationType>,
) -> Option<AirwallexPaymentOptions> {
    match payment_method {
        AirwallexPaymentMethod::PayLater(paylater) => {
            let pay_later_options = AirwallexPayLaterOptions {
                auto_capture: Some(auto_capture),
            };
            Some(match paylater {
                AirwallexPayLaterData::Klarna(_) => AirwallexPaymentOptions {
                    card: None,
                    klarna: Some(pay_later_options),
                    atome: None,
                },
                AirwallexPayLaterData::Atome(_) => AirwallexPaymentOptions {
                    card: None,
                    klarna: None,
                    atome: Some(pay_later_options),
                },
            })
        }
        // Extended authorization (pre-auth hold) is a card-only option, so the
        // authorization_type only ever reaches Airwallex through this arm.
        AirwallexPaymentMethod::Card(_) => Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(auto_capture),
                authorization_type,
            }),
            klarna: None,
            atome: None,
        }),
        // Wallets, BankRedirect and BankTransfer have no payment_method_options block
        // (mirrors the reference upstream, which only emits options for Card/Klarna/Atome).
        AirwallexPaymentMethod::Wallets(_)
        | AirwallexPaymentMethod::BankRedirect(_)
        | AirwallexPaymentMethod::BankTransfer(_) => None,
    }
}

// Implementation for new unified request type
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // UCS unified flow - always create payment intent with payment method

        // Per-method return_url (mirrors native HS): card 3DS must come back through the Authorize
        // completion leg, so point Airwallex at `complete_authorize_url`; APM redirects
        // (wallets/bank-redirect/paylater) return through PSync via `router_return_url`.
        let return_url = match &item.router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(_) => item
                .router_data
                .request
                .complete_authorize_url
                .clone()
                .or_else(|| item.router_data.request.get_router_return_url().ok()),
            _ => item.router_data.request.get_router_return_url().ok(),
        };

        // Which 3DS block goes under `payment_method.card`, if any.
        //
        // * Router-side authentication already happened (`authentication_data` populated) ->
        //   relay the proof as `external_three_ds` (see [`AirwallexThreeDsMode::External`] for the
        //   account entitlement this additionally needs).
        // * Otherwise Airwallex runs 3DS itself and needs
        //   `payment_method.card.three_ds.return_url` — on the pinned API version the top-level
        //   `return_url` is NOT the 3DS return URL (that only happens from 2026-06-30), so
        //   omitting this strands the shopper on the hosted page.
        let three_ds = match item.router_data.request.authentication_data.as_ref() {
            Some(authentication_data) => Some(AirwallexThreeDsMode::External(
                build_external_three_ds(authentication_data)?,
            )),
            None => return_url
                .clone()
                .map(|return_url| AirwallexThreeDsMode::Native { return_url }),
        };

        let payment_method = get_payment_method_details(
            &item.router_data.request.payment_method_data,
            &item.router_data.resource_common_data,
            item.router_data
                .request
                .customer_name
                .clone()
                .map(Secret::new),
            three_ds,
        )?;

        let auto_capture = matches!(
            item.router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
                | Some(common_enums::CaptureMethod::SequentialAutomatic)
                | None
        );

        // Extended authorization (pre-auth hold); build_payment_method_options only
        // applies it on the card arm, which is the only place Airwallex accepts it.
        let authorization_type = matches!(
            item.router_data.request.request_extended_authorization,
            Some(true)
        )
        .then_some(AirwallexCardAuthorizationType::PreAuth);

        let payment_method_options =
            build_payment_method_options(&payment_method, auto_capture, authorization_type);

        // Generate unique request_id for Authorize/confirm step
        // Different from CreateOrder to avoid Airwallex duplicate_request error
        let request_id = format!(
            "confirm_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        // Mirror native HS airwallex for a CIT (setup_future_usage) mandate setup: attach a
        // PaymentConsent so Airwallex returns a payment_consent_id we store as the connector
        // mandate for future MITs, send the connector customer_id, and OMIT device_data. Native
        // only collects device data for non-mandate payments — sending it alongside a consent
        // pushes Airwallex into a device-data-collection SCA path it can't complete here. Same
        // CIT detection helper (is_customer_initiated_mandate_payment) as native.
        let (payment_consent, customer_id, device_data) = if item
            .router_data
            .request
            .is_customer_initiated_mandate_payment()
        {
            (
                Some(AirwallexPaymentConsentData {
                    next_triggered_by: AirwallexTriggeredBy::Merchant,
                    merchant_trigger_reason: AirwallexMerchantTriggeredReason::Unscheduled,
                }),
                Some(
                    item.router_data
                        .resource_common_data
                        .get_connector_customer_id()?,
                ),
                None,
            )
        } else {
            (
                None,
                None,
                get_device_data(item.router_data.request.browser_info.as_ref()),
            )
        };

        Ok(Self {
            request_id,
            payment_method,
            payment_method_options,
            return_url,
            device_data,
            payment_consent,
            customer_id,
        })
    }
}

// Unified response type for all payment operations (Authorize, PSync, Capture, Void)
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentsResponse {
    pub id: String,
    pub status: AirwallexPaymentStatus,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Latest payment attempt information
    pub latest_payment_attempt: Option<AirwallexPaymentAttempt>,
    // Payment method information
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    // Next action for 3DS or other redirects
    pub next_action: Option<AirwallexNextAction>,
    // Payment intent details
    pub payment_intent_id: Option<String>,
    // Capture information
    pub captured_amount: Option<FloatMajorUnit>,
    // Authorization code from processor
    pub authorization_code: Option<String>,
    // Network transaction ID
    pub network_transaction_id: Option<String>,
    // Processor response
    pub processor_response: Option<AirwallexProcessorResponse>,
    // Risk information
    pub risk_score: Option<String>,
    // Void-specific fields
    pub cancelled_at: Option<String>,
    pub cancellation_reason: Option<String>,
    // PaymentConsent ID for SetupMandate (CIT) flow - this is the mandate token for MIT
    pub payment_consent_id: Option<Secret<String>>,
    // Customer id echoed back
    pub customer_id: Option<String>,
}

// Type alias - reuse the same response structure for PSync
pub type AirwallexSyncResponse = AirwallexPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentAttempt {
    pub id: Option<String>,
    pub status: Option<AirwallexAttemptStatus>,
    pub amount: Option<FloatMajorUnit>,
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    pub authorization_code: Option<String>,
    pub network_transaction_id: Option<String>,
    pub processor_response: Option<AirwallexProcessorResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// 3DS / risk outcome for this attempt. Present on `confirm` and on the retrieve that ends a
    /// 3DS journey; it is where the CAVV and ECI live.
    pub authentication_data: Option<AirwallexAttemptAuthenticationData>,
}

/// `latest_payment_attempt.status`, as enumerated by the `2024-06-14` PaymentIntents schema.
/// Anything outside the set deserialises to `Unknown` rather than failing the whole response.
///
/// Two neighbouring enums are easy to confuse with this one and are NOT it:
/// `latest_payment_attempt.payment_method.status` (`CREATED` / `DISABLED`) describes the stored
/// payment method, and `AirwallexPaymentStatus` describes the intent. A decline does not get its
/// own value here — it arrives as `FAILED` with a `failure_code`.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexAttemptStatus {
    Received,
    AuthenticationRedirected,
    PendingAuthorization,
    Authorized,
    CaptureRequested,
    Expired,
    Cancelled,
    Failed,
    Settled,
    Paid,
    #[serde(other)]
    Unknown,
}

/// `latest_payment_attempt.authentication_data`.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexAttemptAuthenticationData {
    pub ds_data: Option<AirwallexDsData>,
    pub fraud_data: Option<AirwallexFraudData>,
    pub avs_result: Option<String>,
    pub cvc_result: Option<String>,
}

/// `authentication_data.ds_data` — the directory-server outcome of a 3DS authentication.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDsData {
    /// 3DS message version, e.g. `"2.2.0"`. Parsed into a `SemanticVersion`, never hardcoded.
    #[serde(default, deserialize_with = "deserialize_non_empty_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_yes_no")]
    pub liability_shift_indicator: Option<AirwallexYesNo>,
    /// Two-character ECI. Airwallex documents it per-network (Visa `05`/`06`/`07`, Mastercard
    /// `02`/`01`/`00`) and the set is open-ended across networks, so it stays an opaque string.
    #[serde(default, deserialize_with = "deserialize_non_empty_string")]
    pub eci: Option<String>,
    /// The 3DS2 authentication value. Secret: it is a single-use authorisation credential.
    pub cavv: Option<Secret<String>>,
    /// 3DS1 transaction identifier, present only when Airwallex fell back to 3DS1. Secret for the
    /// same reason as `cavv`.
    pub xid: Option<Secret<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_yes_no")]
    pub frictionless: Option<AirwallexYesNo>,
}

/// Airwallex renders these `ds_data` flags as the literal strings `"Y"` / `"N"`.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum AirwallexYesNo {
    #[serde(rename = "Y")]
    Yes,
    #[serde(rename = "N")]
    No,
}

/// Reads a `ds_data` Y/N flag that Airwallex may not have decided yet.
///
/// While the shopper is still on the hosted 3DS page Airwallex renders these flags as the empty
/// string rather than omitting them or sending `null` — its own 3DS guide's
/// `AUTHENTICATION_REDIRECTED` example shows `"liability_shift_indicator": ""`. A plain
/// `Option<AirwallexYesNo>` fails on that, and because `ds_data` is nested inside the intent, one
/// empty flag would fail the *entire* confirm response and report a live payment as an error.
///
/// Anything that is not `Y` or `N` therefore reads as `None`, which is exactly what Airwallex
/// documents `null` to mean here: "N/A or undeterminable".
fn deserialize_optional_yes_no<'de, D>(deserializer: D) -> Result<Option<AirwallexYesNo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(match raw.as_deref() {
        Some("Y") => Some(AirwallexYesNo::Yes),
        Some("N") => Some(AirwallexYesNo::No),
        _ => None,
    })
}

/// Same problem as [`deserialize_optional_yes_no`] for the free-form `ds_data` strings: an
/// in-flight `eci`/`version` arrives as `""`, and an empty ECI is worse than no ECI because it
/// would be forwarded to the caller as if the directory server had stated one.
fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.filter(|value| !value.is_empty()))
}

/// `authentication_data.fraud_data`.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexFraudData {
    pub action: Option<String>,
    pub score: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexPaymentStatus {
    RequiresPaymentMethod,
    RequiresCustomerAction,
    RequiresCapture,
    Authorized,       // Payment authorized (from latest_payment_attempt)
    Paid,             // Payment paid/captured (from latest_payment_attempt)
    CaptureRequested, // Payment captured but settlement in progress
    Processing,
    Succeeded,
    Settled, // Payment fully settled - indicates successful completion
    Cancelled,
    Failed,
    Pending,
    /// Airwallex added a status we do not model yet. Deserialising it instead of erroring keeps
    /// the rest of the intent (id, next_action, ds_data) usable; it maps to
    /// `AttemptStatus::Unresolved`, never to `Failure` — we cannot claim the money did not move.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentMethodInfo {
    #[serde(rename = "type")]
    pub method_type: String,
    pub card: Option<AirwallexCardInfo>,
    // Bank redirect fields
    pub blik: Option<Secret<serde_json::Value>>, // For BLIK payment method details
    pub ideal: Option<Secret<serde_json::Value>>, // For iDEAL payment method details
    pub trustly: Option<Secret<serde_json::Value>>, // For Trustly payment method details
    // Additional payment method fields
    pub id: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexCardInfo {
    pub last4: Option<String>,
    pub brand: Option<String>,
    pub exp_month: Option<Secret<String>>,
    pub exp_year: Option<Secret<String>>,
    pub fingerprint: Option<String>,
}

/// `next_action.type`.
///
/// Card 3DS on [`AIRWALLEX_API_VERSION`] produces `redirect_iframe`, which lands on `Other`;
/// APM redirects produce `redirect`. Both mean the same thing to UCS — send the shopper to
/// `next_action.url` — so the two arms are kept only because `redirect` is a documented value
/// and `Other` is the catch-all.
///
/// There is deliberately **no `device_data_collection` variant**. Airwallex split the 3DS method
/// call out as its own `next_action` only before `2024-06-14`, and
/// [`AIRWALLEX_API_VERSION`] is sent on every request (see `Airwallex::build_headers`), so no
/// response this connector can receive carries it. A variant for it would be dead code, and the
/// `DeviceDataCollectionPending` status it used to produce would strand the payment: nothing in
/// this connector implements a DDC leg.
///
/// [`AIRWALLEX_API_VERSION`]: super::AIRWALLEX_API_VERSION
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexNextActionType {
    Redirect,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexNextAction {
    #[serde(rename = "type")]
    pub action_type: AirwallexNextActionType,
    /// Deserialized for completeness but deliberately **not** used to pick the redirect method —
    /// [`build_redirection_data`] always emits GET. Airwallex embeds a one-time `?key=` in the
    /// 3DS-method URL that has to stay in the query string; a POST form would move it into the
    /// body and the endpoint 401s. Do not "fix" this by honouring `method` without re-testing the
    /// card 3DS challenge end to end.
    pub method: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexProcessorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    pub decline_code: Option<String>,
    pub network_code: Option<String>,
}

/// Turns any `next_action` carrying a URL into a GET [`RedirectForm`].
///
/// Shared by all three response transformers (Authorize, SetupMandate, RepeatPayment) so redirect
/// surfacing cannot drift between them. It is deliberately **not** gated on
/// `action_type == Redirect`: card 3DS arrives as `device_data_collection` / `other`, and the
/// earlier gated version silently dropped those — which matters most on SetupMandate, the CIT card
/// path where a 3DS challenge is most likely. GET is always used; see
/// [`AirwallexNextAction::method`].
fn build_redirection_data(next_action: &Option<AirwallexNextAction>) -> Option<Box<RedirectForm>> {
    next_action.as_ref().and_then(|next_action| {
        next_action.url.as_ref().and_then(|url_str| {
            Url::parse(url_str)
                .ok()
                .map(|url| Box::new(RedirectForm::from((url, Method::Get))))
        })
    })
}

// Helper function to get payment status from Airwallex status (following Hyperswitch pattern)
pub(super) fn get_payment_status(
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
) -> AttemptStatus {
    match status {
        AirwallexPaymentStatus::Succeeded => AttemptStatus::Charged,
        AirwallexPaymentStatus::Failed => AttemptStatus::Failure,
        AirwallexPaymentStatus::Processing => AttemptStatus::Pending,
        AirwallexPaymentStatus::RequiresPaymentMethod => AttemptStatus::PaymentMethodAwaited,
        // REQUIRES_CUSTOMER_ACTION only means something if Airwallex also said what the action
        // is. With a `next_action` this is the redirect the shopper has to make; without one the
        // response contradicts itself and we must not guess a state the caller would act on —
        // hence Unresolved rather than a Failure the money may not agree with.
        AirwallexPaymentStatus::RequiresCustomerAction => {
            next_action.as_ref().map_or(AttemptStatus::Unresolved, |_| {
                AttemptStatus::AuthenticationPending
            })
        }
        AirwallexPaymentStatus::RequiresCapture => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Authorized => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Paid => AttemptStatus::Charged,
        AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
        AirwallexPaymentStatus::CaptureRequested => AttemptStatus::Charged,
        AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
        AirwallexPaymentStatus::Pending => AttemptStatus::Pending,
        AirwallexPaymentStatus::Unknown => AttemptStatus::Unresolved,
    }
}

// Extended-authorization result for the authorize response: applied only when it
// was requested AND the payment method is card (mirrors hyperswitch airwallex)
fn build_airwallex_connector_response_data(
    extended_authorization_requested: bool,
    payment_method: common_enums::PaymentMethod,
) -> Option<ConnectorResponseData> {
    let extended_authentication_applicable =
        matches!(payment_method, common_enums::PaymentMethod::Card);
    let extended_authentication_applied =
        if extended_authorization_requested && extended_authentication_applicable {
            Some(true)
        } else if extended_authorization_requested {
            Some(false)
        } else {
            None
        };
    Some(ConnectorResponseData::new(
        None,
        None,
        Some(ExtendedAuthorizationResponseData {
            extended_authentication_applied,
            extended_authorization_last_applied_at: None,
            capture_before: None,
        }),
    ))
}

// New response transformer that addresses PR #240 critical issues
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Handles APM redirects (type "redirect") AND card 3DS device-data-collection / challenge
        // (type "other" / "device_data_collection"), which the old type-gated version dropped.
        let redirection_data = build_redirection_data(&item.response.next_action);

        // Extract network transaction ID for network response fields (PR #240 Issue #4)
        let network_txn_id = item
            .response
            .network_transaction_id
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata
        let connector_metadata = None;

        // Report whether the requested extended authorization was applied;
        // absent entirely when the flag was never sent
        let connector_response = item
            .router_data
            .request
            .request_extended_authorization
            .and_then(|requested| {
                build_airwallex_connector_response_data(
                    requested,
                    item.router_data.resource_common_data.payment_method,
                )
            });

        // Surface the Airwallex PaymentConsent as the connector mandate reference for CIT payments,
        // so HS stores connector_mandate_id (payment_consent_id) + payment_method.id and can run
        // future MITs. Mirrors the SetupMandate response builder. `payment_consent_id` is only
        // present when the Authorize request set up a consent (the CIT path).
        let airwallex_payment_method_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|lpa| lpa.payment_method.as_ref())
            .and_then(|pm| pm.id.clone())
            .or_else(|| {
                item.response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.id.clone())
            });
        let mandate_reference = item
            .response
            .payment_consent_id
            .clone()
            .map(|id| MandateReference {
                connector_mandate_id: Some(id.expose()),
                payment_method_id: airwallex_payment_method_id.clone(),
                connector_mandate_request_reference_id: None,
                // Round-trip the Airwallex payment-method token via mandate_metadata as
                // {"id": ...}: hyperswitch overwrites payment_method_id with its own id, so the
                // MIT transformer reads the token back from mandate_metadata.
                mandate_metadata: airwallex_payment_method_id
                    .map(|pm_id| Secret::new(serde_json::json!({ "id": pm_id }))),
            })
            .map(Box::new);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data,
                mandate_reference,
                connector_metadata,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

impl TryFrom<ResponseRouterData<AirwallexSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Use the same simple status mapping as hyperswitch
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone());

        // Surface the Airwallex PaymentConsent as the connector mandate reference here too, so a
        // CIT (setup_future_usage) whose final state is fetched via PSync (e.g. card 3DS that
        // returns through the sync leg) still stores connector_mandate_id (payment_consent_id) +
        // payment_method.id for future MITs. Mirrors the Authorize/SetupMandate response builders;
        // `payment_consent_id` is only present when a consent was set up (the CIT path), so a plain
        // sync leaves mandate_reference None.
        let airwallex_payment_method_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|lpa| lpa.payment_method.as_ref())
            .and_then(|pm| pm.id.clone())
            .or_else(|| {
                item.response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.id.clone())
            });
        let mandate_reference = item
            .response
            .payment_consent_id
            .clone()
            .map(|id| MandateReference {
                connector_mandate_id: Some(id.expose()),
                payment_method_id: airwallex_payment_method_id.clone(),
                connector_mandate_request_reference_id: None,
                // Round-trip the Airwallex payment-method token via mandate_metadata as
                // {"id": ...}: hyperswitch overwrites payment_method_id with its own id, so the
                // MIT transformer reads the token back from mandate_metadata.
                mandate_metadata: airwallex_payment_method_id
                    .map(|pm_id| Secret::new(serde_json::json!({ "id": pm_id }))),
            })
            .map(Box::new);

        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                reference_id: Some(intent_id),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
// ===== CAPTURE FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexCaptureRequest {
    pub amount: StringMajorUnit, // Amount in major units
    pub request_id: String,      // Unique identifier for this capture request
}

// Type alias - reuse the same response structure for Capture
pub type AirwallexCaptureResponse = AirwallexPaymentsResponse;

// Request transformer for Capture flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for AirwallexCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract capture amount from the capture data
        let capture_amount = item.router_data.request.amount_to_capture;

        // Use connector amount converter for proper amount formatting in major units (hyperswitch pattern)
        let amount = item
            .connector
            .amount_converter
            .convert(
                common_utils::MinorUnit::new(capture_amount),
                item.router_data.request.currency,
            )
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "capture_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self { amount, request_id })
    }
}

// Response transformer for Capture flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Use the same simple status mapping as hyperswitch
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Address PR #240 Issue #4: Network Specific Fields
        // Extract network transaction ID (prefer latest attempt, then main response)
        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone())
            .or_else(|| {
                item.response
                    .latest_payment_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.authorization_code.clone())
            })
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None, // Capture doesn't involve redirections
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexRefundRequest {
    // connector_transaction_id is the Airwallex payment *intent* id (int_...); the
    // /pa/refunds/create endpoint accepts it as payment_intent_id.
    pub payment_intent_id: String, // From connector_transaction_id (the intent id)
    pub amount: StringMajorUnit,   // Refund amount in major units
    pub reason: Option<String>,    // Refund reason if provided
    pub request_id: String,        // Unique identifier for idempotency
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexRefundResponse {
    pub id: String,                         // Refund ID
    pub request_id: Option<String>,         // Echo back request ID
    pub payment_intent_id: Option<String>,  // Original payment intent ID
    pub payment_attempt_id: Option<String>, // Original payment attempt ID
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,                // Currency code
    pub reason: Option<String>,                    // Refund reason
    pub status: AirwallexRefundStatus,             // RECEIVED, ACCEPTED, SETTLED, FAILED
    pub created_at: Option<String>,                // Creation timestamp
    pub updated_at: Option<String>,                // Update timestamp
    pub acquirer_reference_number: Option<String>, // Network reference
    pub failure_details: Option<AirwallexFailureDetails>, // Error details if failed
    pub metadata: Option<serde_json::Value>,       // Additional metadata
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexRefundStatus {
    Received,
    Accepted,
    Settled,
    Failed,
}

/// Airwallex's `failure_details` object, as carried by a failed Refund and by a failed
/// PaymentAttempt.
///
/// Documented shape is `{code, message, trace_id, details}`. Only `code` and `message` are
/// consumed — they are the `error_code` / `error_message` a caller can act on — so `trace_id`
/// and `details` are left unmodelled rather than deserialised and dropped.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexFailureDetails {
    pub code: Option<String>,
    pub message: Option<String>,
}

// Request transformer for Refund flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for AirwallexRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // connector_transaction_id is the Airwallex payment intent id (int_...).
        let payment_intent_id = item.router_data.request.connector_transaction_id.clone();

        // Extract refund amount from RefundsData and convert to major units (hyperswitch pattern)
        let refund_amount = item.router_data.request.refund_amount;
        let amount = item
            .connector
            .amount_converter
            .convert(
                common_utils::MinorUnit::new(refund_amount),
                item.router_data.request.currency,
            )
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "refund_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            payment_intent_id,
            amount,
            reason: item.router_data.request.reason.clone(),
            request_id,
        })
    }
}

// Response transformer for Refund flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND SYNC FLOW TYPES =====

// Reuse the same response structure as AirwallexRefundResponse since it's the same endpoint (GET /pa/refunds/{id})
pub type AirwallexRefundSyncResponse = AirwallexRefundResponse;

// Response transformer for RSync flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status: status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Simple status mapping following Hyperswitch pattern
// Trust the Airwallex API to return correct status
impl From<AirwallexRefundStatus> for RefundStatus {
    fn from(status: AirwallexRefundStatus) -> Self {
        match status {
            AirwallexRefundStatus::Settled => Self::Success,
            AirwallexRefundStatus::Failed => Self::Failure,
            AirwallexRefundStatus::Received | AirwallexRefundStatus::Accepted => Self::Pending,
        }
    }
}

// ===== VOID FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexVoidRequest {
    pub cancellation_reason: Option<String>, // Reason for cancellation
    pub request_id: String,                  // Unique identifier for idempotency
}

// Type alias - reuse the same response structure for Void
pub type AirwallexVoidResponse = AirwallexPaymentsResponse;

// Request transformer for Void flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for AirwallexVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract cancellation reason from PaymentVoidData (if available)
        let cancellation_reason = item
            .router_data
            .request
            .cancellation_reason
            .clone()
            .or_else(|| Some("Voided by merchant".to_string()));

        // Generate unique request_id for idempotency using connector_request_reference_id
        let request_id = format!(
            "void_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            cancellation_reason,
            request_id,
        })
    }
}

// Response transformer for Void flow - addresses PR #240 critical issues
impl TryFrom<ResponseRouterData<AirwallexVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // Address PR #240 Issue #4: Network Specific Fields
        // Extract network transaction ID (prefer latest attempt, then main response)
        let network_txn_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|attempt| attempt.network_transaction_id.clone())
            .or_else(|| item.response.network_transaction_id.clone())
            .or_else(|| {
                item.response
                    .latest_payment_attempt
                    .as_ref()
                    .and_then(|attempt| attempt.authorization_code.clone())
            })
            .or(item.response.authorization_code.clone());

        // Following hyperswitch pattern - no connector_metadata for void
        let connector_metadata = None;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None, // Void doesn't involve redirections
                mandate_reference: None,
                connector_metadata,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Removed over-engineered validation - use simple get_payment_status instead
// The Airwallex API is trusted to return correct status (following Hyperswitch pattern)

// ===== CREATE ORDER FLOW TYPES =====

// Referrer data to identify UCS implementation to Airwallex
#[derive(Debug, Serialize)]
pub struct AirwallexReferrerData {
    #[serde(rename = "type")]
    pub r_type: String,
    pub version: String,
}

// Order data for payment intents (required for pay-later methods)
#[derive(Debug, Serialize)]
pub struct AirwallexOrderData {
    pub products: Vec<AirwallexProductData>,
    pub shipping: Option<AirwallexShippingData>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexProductData {
    pub name: String,
    pub quantity: u16,
    pub unit_price: StringMajorUnit, // Using StringMajorUnit for amount consistency
}

#[derive(Debug, Serialize)]
pub struct AirwallexShippingData {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone_number: Option<Secret<String>>,
    pub shipping_method: Option<String>,
    pub address: Option<AirwallexAddressData>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexAddressData {
    pub country_code: String,
    pub state: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub street: Option<Secret<String>>,
    pub postcode: Option<Secret<String>>,
}

// CreateOrder request structure (Step 1 - Intent creation without payment method)
#[derive(Debug, Serialize)]
pub struct AirwallexIntentRequest {
    pub request_id: String,
    pub amount: StringMajorUnit,
    pub currency: Currency,
    pub merchant_order_id: String,
    // UCS identification for Airwallex whitelisting
    pub referrer_data: AirwallexReferrerData,
    // Optional order data for pay-later methods
    pub order: Option<AirwallexOrderData>,
}

// CreateOrder response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexIntentResponse {
    pub id: String,
    pub request_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub merchant_order_id: Option<String>,
    pub status: AirwallexPaymentStatus,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Client secret for frontend integration
    pub client_secret: Option<String>,
    // Available payment method types
    pub available_payment_method_types: Option<Vec<String>>,
}

// Request transformer for CreateOrder flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for AirwallexIntentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Create referrer data for Airwallex identification
        let referrer_data = AirwallexReferrerData {
            r_type: "hyperswitch".to_string(),
            version: "1.0.0".to_string(),
        };

        // Convert amount using the same converter as other flows
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.amount,
                item.router_data.request.currency,
            )
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        // Populate the order line items when provided. Airwallex requires `order.products`
        // at payment-intent creation for PayLater methods (e.g. Klarna); the sum of
        // (quantity * unit_price) must equal the intent amount.
        let order = match item.router_data.request.order_details.as_ref() {
            Some(order_details) if !order_details.is_empty() => {
                let products = order_details
                    .iter()
                    .map(|detail| {
                        let unit_price = item
                            .connector
                            .amount_converter
                            .convert(detail.amount, item.router_data.request.currency)
                            .map_err(|_| IntegrationError::RequestEncodingFailed {
                                context: aw_err_ctx(
                                    "Failed to convert an order line item amount into the \
                                     Airwallex minor-unit representation for order.products",
                                    "Ensure every order_details entry carries an amount valid \
                                     for the payment currency",
                                ),
                            })?;
                        Ok(AirwallexProductData {
                            name: detail.product_name.clone(),
                            quantity: detail.quantity,
                            unit_price,
                        })
                    })
                    .collect::<Result<Vec<_>, error_stack::Report<IntegrationError>>>()?;
                Some(AirwallexOrderData {
                    products,
                    shipping: None,
                })
            }
            _ => None,
        };

        // Generate unique request_id for CreateOrder step
        let request_id = format!(
            "create_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            amount,
            currency: item.router_data.request.currency,
            merchant_order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            referrer_data,
            order,
        })
    }
}

// Response transformer for CreateOrder flow
impl TryFrom<ResponseRouterData<AirwallexIntentResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::CreateOrder,
        PaymentFlowData,
        domain_types::connector_types::PaymentCreateOrderData,
        domain_types::connector_types::PaymentCreateOrderResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexIntentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        // Map intent status to order status
        let status = match item.response.status {
            AirwallexPaymentStatus::RequiresPaymentMethod => AttemptStatus::PaymentMethodAwaited,
            AirwallexPaymentStatus::RequiresCustomerAction => AttemptStatus::AuthenticationPending,
            AirwallexPaymentStatus::Processing => AttemptStatus::Pending,
            AirwallexPaymentStatus::Succeeded => AttemptStatus::Charged,
            AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
            AirwallexPaymentStatus::Failed => AttemptStatus::Failure,
            AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
            AirwallexPaymentStatus::RequiresCapture => AttemptStatus::Authorized,
            AirwallexPaymentStatus::Authorized => AttemptStatus::Authorized,
            AirwallexPaymentStatus::Paid => AttemptStatus::Charged,
            AirwallexPaymentStatus::CaptureRequested => AttemptStatus::Charged,
            AirwallexPaymentStatus::Pending => AttemptStatus::Pending,
            // Unrecognised intent status: the order may well exist, so never Failure.
            AirwallexPaymentStatus::Unknown => AttemptStatus::Unresolved,
        };

        router_data.response = Ok(domain_types::connector_types::PaymentCreateOrderResponse {
            connector_order_id: item.response.id.clone(),
            session_data: None,
        });

        // Update the flow data with the new status and store payment intent ID as reference_id (like Razorpay V2)
        router_data.resource_common_data = PaymentFlowData {
            status,
            reference_id: Some(item.response.id.clone()),
            connector_order_id: Some(item.response.id),
            connector_http_status_code: Some(item.http_code),
            ..router_data.resource_common_data
        };

        Ok(router_data)
    }
}

// Access Token Request Transformer
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                domain_types::connector_types::ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for AirwallexAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: super::AirwallexRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerAuthenticationTokenRequestData,
                domain_types::connector_types::ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Airwallex ServerAuthenticationToken requires empty JSON body {}
        // The authentication headers (x-api-key, x-client-id) are set separately
        Ok(Self {
            // Empty struct serializes to {}
        })
    }
}

// Access Token Response Transformer
impl TryFrom<ResponseRouterData<AirwallexAccessTokenResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        domain_types::connector_types::ServerAuthenticationTokenRequestData,
        domain_types::connector_types::ServerAuthenticationTokenResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;

        let expires = (item.response.expires_at - common_utils::date_time::now()).whole_seconds();

        router_data.response = Ok(
            domain_types::connector_types::ServerAuthenticationTokenResponseData {
                access_token: item.response.token,
                token_type: Some("Bearer".to_string()),
                expires_in: Some(expires),
            },
        );

        Ok(router_data)
    }
}

// ===== SETUP MANDATE (PaymentConsent CIT) FLOW TYPES =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexTriggeredBy {
    Merchant,
    Customer,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexMerchantTriggeredReason {
    Unscheduled,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPaymentConsentData {
    pub next_triggered_by: AirwallexTriggeredBy,
    pub merchant_trigger_reason: AirwallexMerchantTriggeredReason,
}

#[derive(Debug, Serialize)]
pub struct AirwallexSetupMandateRequest {
    pub request_id: String,
    pub payment_method: AirwallexPaymentMethod,
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    pub return_url: Option<String>,
    pub payment_consent: AirwallexPaymentConsentData,
    pub customer_id: String,
}

// Reuse the payments response for SetupMandate confirm - same endpoint shape
pub type AirwallexSetupMandateResponse = AirwallexPaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexSetupMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // A CIT card consent is the most likely place for Airwallex to raise a 3DS challenge, so
        // it needs the same `payment_method.card.three_ds.return_url` the Authorize confirm
        // sends: on the pinned API version the top-level `return_url` is not the 3DS return URL.
        let three_ds = item
            .router_data
            .request
            .router_return_url
            .clone()
            .map(|return_url| AirwallexThreeDsMode::Native { return_url });

        let payment_method = match &item.router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                get_card_details(card_data, three_ds)
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "SetupMandate Payment Method (only Card supported)".to_string(),
                    connector: "Airwallex",
                    context: Default::default(),
                }
                .into())
            }
        };

        let payment_method_options = Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(false),
                authorization_type: None,
            }),
            klarna: None,
            atome: None,
        });

        // Airwallex requires a connector-level customer_id (`cus_*`) at PaymentConsent
        // creation. SetupMandate is the CIT step — fail if it isn't populated rather
        // than silently falling back to a merchant-side id the connector would reject.
        let customer_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_customer",
                context: Default::default(),
            })?;

        let request_id = format!(
            "confirm_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            payment_method,
            payment_method_options,
            return_url: item.router_data.request.router_return_url.clone(),
            payment_consent: AirwallexPaymentConsentData {
                next_triggered_by: AirwallexTriggeredBy::Merchant,
                merchant_trigger_reason: AirwallexMerchantTriggeredReason::Unscheduled,
            },
            customer_id,
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        let redirection_data = build_redirection_data(&item.response.next_action);

        // Airwallex MIT requires `payment_method.id` (pm_...) in addition to the
        // PaymentConsent id (cst_...). The pm_... is surfaced under
        // `latest_payment_attempt.payment_method.id` (preferred, because the
        // top-level `payment_method` object may be absent on AUTHENTICATION_PENDING
        // responses). Fall back to top-level `payment_method.id`.
        let airwallex_payment_method_id = item
            .response
            .latest_payment_attempt
            .as_ref()
            .and_then(|lpa| lpa.payment_method.as_ref())
            .and_then(|pm| pm.id.clone())
            .or_else(|| {
                item.response
                    .payment_method
                    .as_ref()
                    .and_then(|pm| pm.id.clone())
            });

        let mandate_reference = item
            .response
            .payment_consent_id
            .clone()
            .map(|id| MandateReference {
                connector_mandate_id: Some(id.expose()),
                // Surface the Airwallex payment_method.id so the MIT transformer can
                // reference it as `payment_method.id`.
                payment_method_id: airwallex_payment_method_id.clone(),
                connector_mandate_request_reference_id: None,
                // Round-trip the token via mandate_metadata as {"id": ...}; hyperswitch
                // overwrites payment_method_id with its own id, so the MIT transformer reads
                // the token back from mandate_metadata.
                mandate_metadata: airwallex_payment_method_id
                    .map(|pm_id| Secret::new(serde_json::json!({ "id": pm_id }))),
            })
            .map(Box::new);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false),
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REPEAT PAYMENT (PaymentConsent MIT) FLOW TYPES =====
//
// Airwallex MIT per hyperswitch ref: POST /pa/payment_intents/{new_intent_id}/confirm
// with `payment_consent_reference: { id: <cst_...> }`, `triggered_by: merchant`,
// `payment_method: { type: "card" }` (no card details — referencing stored consent),
// and `customer_id`. A fresh PaymentIntent must be created (CreateOrder) before this
// confirm; the CIT consent-setup intent is already consumed.

#[derive(Debug, Serialize)]
pub struct AirwallexRepeatPaymentMethodId {
    pub id: String,
}

// Connector mandate metadata that hyperswitch round-trips opaquely. Carries the Airwallex
// payment-method token so a later MIT can replay it (mirrors the upstream HS airwallex connector).
#[derive(Debug, Deserialize)]
pub struct AirwallexMandateMetadata {
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexRepeatPaymentRequest {
    pub request_id: String,
    // Airwallex MIT references the stored payment_method by id (pm_...) created
    // under the PaymentConsent; no card details are sent here.
    pub payment_method: AirwallexRepeatPaymentMethodId,
    // The PaymentConsent id (cst_...) is sent top-level as payment_consent_id.
    pub payment_consent_id: Secret<String>,
    pub triggered_by: AirwallexTriggeredBy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
}

pub type AirwallexRepeatPaymentResponse = AirwallexPaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Airwallex MIT requires BOTH payment_consent_id (cst_...) AND
        // payment_method.id (pm_...). The connector rejects the request with
        // "triggered_by should not be set, payment_method.id should be provided
        // when triggered_by is set" if payment_method.id is missing.
        let (connector_mandate_id, payment_method_id, mandate_metadata) =
            match &item.router_data.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(cm) => (
                    cm.get_connector_mandate_id(),
                    cm.get_payment_method_id().cloned(),
                    cm.get_mandate_metadata(),
                ),
                _ => (None, None, None),
            };

        let connector_mandate_id =
            connector_mandate_id.ok_or(IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            })?;
        // Airwallex MIT replays the Airwallex payment-method token. hyperswitch stores its OWN id
        // in payment_method_id but round-trips the connector token in mandate_metadata as
        // {"id": ...}; prefer that, falling back to payment_method_id for older stored mandates.
        let payment_method_id = mandate_metadata
            .and_then(|meta| serde_json::from_value::<AirwallexMandateMetadata>(meta.expose()).ok())
            .and_then(|meta| meta.id)
            .or(payment_method_id)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "payment_method_id",
                context: Default::default(),
            })?;

        let customer_id = item
            .router_data
            .resource_common_data
            .connector_customer
            .clone();

        let request_id = format!(
            "mit_confirm_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            payment_method: AirwallexRepeatPaymentMethodId {
                id: payment_method_id,
            },
            payment_consent_id: Secret::new(connector_mandate_id),
            triggered_by: AirwallexTriggeredBy::Merchant,
            customer_id,
            return_url: item.router_data.request.router_return_url.clone(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        let redirection_data = build_redirection_data(&item.response.next_action);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.payment_intent_id,
                incremental_authorization_allowed: Some(false),
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== CREATE CONNECTOR CUSTOMER FLOW =====
// Airwallex POST /api/v1/pa/customers/create — mirrors the hyperswitch implementation at
// hyperswitch/crates/hyperswitch_connectors/src/connectors/airwallex.rs.

#[derive(Debug, Serialize)]
pub struct AirwallexCustomerRequest {
    pub request_id: String,
    pub merchant_customer_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexCustomerResponse {
    pub id: String,
    pub merchant_customer_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for AirwallexCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data.request;

        let merchant_customer_id =
            data.customer_id
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "merchant_customer_id",
                    context: Default::default(),
                })?;

        let email = data.email.clone().map(|e| e.expose());

        let (first_name, last_name) = split_full_name(data.name.clone());

        let request_id = format!(
            "customer_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            merchant_customer_id,
            email,
            phone_number: data.phone.clone(),
            first_name,
            last_name,
        })
    }
}

impl TryFrom<ResponseRouterData<AirwallexCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut router_data = item.router_data;
        router_data.response = Ok(ConnectorCustomerResponse {
            connector_customer_id: item.response.id,
            status_code: item.http_code,
        });
        router_data.resource_common_data.connector_http_status_code = Some(item.http_code);
        Ok(router_data)
    }
}

// =====================================================================================
// 3DS AUTHENTICATION TRIO — PreAuthenticate / PostAuthenticate
// =====================================================================================
//
// On `AIRWALLEX_API_VERSION` a 3DS card payment is exactly TWO Airwallex network calls:
//
//   PreAuthenticate   POST /pa/payment_intents/{id}/confirm   -> REQUIRES_CUSTOMER_ACTION
//                                                                 + next_action.redirect_iframe
//   (browser)         GET  next_action.url                    -> Airwallex-hosted page runs the
//                                                                 3DS method call AND, if the
//                                                                 issuer asks for it, the ACS
//                                                                 challenge; redirects back to
//                                                                 payment_method.card.three_ds.return_url
//   PostAuthenticate  GET  /pa/payment_intents/{id}           -> settled intent + ds_data
//
// There is deliberately NO `Authenticate` leg. Airwallex removed `confirm_continue`'s
// `3ds_continue` type on 2024-06-14 and hides the method call and the challenge behind the one
// hosted redirect, so there is no merchant-issued API call between `confirm` and the browser's
// return. `Airwallex::next_authentication_step` therefore never returns
// `AuthenticationStep::Authenticate`, and `PaymentAuthenticateV2` is intentionally not
// implemented: an unreachable implementation would invite a second `confirm`, and on Airwallex
// `confirm` IS the authorization — a second one is a double charge.

/// Airwallex's own `authentication_data` is nested under the attempt; both 3DS legs read it from
/// exactly here so they cannot disagree about where the CAVV came from.
fn get_attempt_ds_data(response: &AirwallexPaymentsResponse) -> Option<&AirwallexDsData> {
    response
        .latest_payment_attempt
        .as_ref()
        .and_then(|attempt| attempt.authentication_data.as_ref())
        .and_then(|authentication_data| authentication_data.ds_data.as_ref())
}

/// Derives `AuthenticationData.trans_status` from what Airwallex actually reports.
///
/// Airwallex does not expose a raw 3DS `transStatus`, so it is reconstructed from the liability
/// shift indicator and the presence of a CAVV. `TransactionStatus::default()` is `Failure`, so
/// this never falls through to a derive: with no `ds_data` the honest answer is `None`, and a
/// pending challenge is `ChallengeRequired`, not a failed authentication.
fn get_trans_status(
    ds_data: Option<&AirwallexDsData>,
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
) -> Option<common_enums::TransactionStatus> {
    match ds_data {
        Some(ds_data) if ds_data.cavv.is_some() => Some(match ds_data.liability_shift_indicator {
            // Fully authenticated with liability shift.
            Some(AirwallexYesNo::Yes) => common_enums::TransactionStatus::Success,
            // A CAVV without liability shift is proof of attempt only.
            Some(AirwallexYesNo::No) | None => common_enums::TransactionStatus::NotVerified,
        }),
        // The shopper is still on the Airwallex-hosted 3DS page.
        _ if matches!(status, AirwallexPaymentStatus::RequiresCustomerAction)
            && next_action.is_some() =>
        {
            Some(common_enums::TransactionStatus::ChallengeRequired)
        }
        // No ds_data and no challenge in flight: Airwallex has told us nothing about the
        // authentication, so say nothing rather than defaulting to Failure.
        _ => None,
    }
}

/// Projects Airwallex's `ds_data` onto the canonical UCS [`AuthenticationData`].
///
/// Fields Airwallex does not expose on native 3DS stay `None` — notably `ds_trans_id` and
/// `threeds_server_transaction_id`, which Airwallex only accepts as *request* fields under
/// `external_three_ds` and never echoes back on the native flow.
///
/// [`AuthenticationData`]: domain_types::router_request_types::AuthenticationData
fn build_authentication_data(
    response: &AirwallexPaymentsResponse,
) -> Option<domain_types::router_request_types::AuthenticationData> {
    let ds_data = get_attempt_ds_data(response);
    let trans_status = get_trans_status(ds_data, &response.status, &response.next_action);

    // Nothing to report: no directory-server data and no challenge in flight.
    if ds_data.is_none() && trans_status.is_none() {
        return None;
    }

    Some(domain_types::router_request_types::AuthenticationData {
        trans_status,
        eci: ds_data.and_then(|ds| ds.eci.clone()),
        cavv: ds_data.and_then(|ds| ds.cavv.clone()),
        ucaf_collection_indicator: None,
        // Airwallex does not surface either identifier on native 3DS.
        threeds_server_transaction_id: None,
        message_version: ds_data.and_then(|ds| {
            ds.version
                .as_ref()
                .and_then(|version| version.parse::<common_utils::types::SemanticVersion>().ok())
        }),
        ds_trans_id: None,
        acs_transaction_id: None,
        // 3DS1 XID, present only on an Airwallex 3DS1 fallback.
        transaction_id: ds_data.and_then(|ds| ds.xid.as_ref().map(|xid| xid.peek().to_string())),
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    })
}

/// `latest_payment_attempt.status` -> UCS. Used only to refine a NON-terminal intent status: when
/// the intent and the attempt disagree the intent wins, because the intent is what Airwallex
/// settles against.
fn get_attempt_status(attempt_status: AirwallexAttemptStatus) -> AttemptStatus {
    match attempt_status {
        AirwallexAttemptStatus::Received => AttemptStatus::Started,
        AirwallexAttemptStatus::AuthenticationRedirected => AttemptStatus::AuthenticationPending,
        // The card is with the acquirer and no decision exists yet. Non-terminal on purpose: a
        // terminal reading here would let the caller abandon a payment that is still live.
        AirwallexAttemptStatus::PendingAuthorization => AttemptStatus::Pending,
        AirwallexAttemptStatus::Authorized => AttemptStatus::Authorized,
        AirwallexAttemptStatus::CaptureRequested
        | AirwallexAttemptStatus::Settled
        | AirwallexAttemptStatus::Paid => AttemptStatus::Charged,
        AirwallexAttemptStatus::Expired | AirwallexAttemptStatus::Cancelled => {
            AttemptStatus::Voided
        }
        // Covers declines too — Airwallex has no separate DECLINED value; the reason is in
        // `failure_code`, which the error body carries.
        AirwallexAttemptStatus::Failed => AttemptStatus::Failure,
        AirwallexAttemptStatus::Unknown => AttemptStatus::Unresolved,
    }
}

/// Resolves the status for a 3DS leg from the intent, refined by the attempt.
///
/// Terminal intent states are authoritative and are returned as-is. Only when the intent is
/// non-terminal (still awaiting an action, or Unresolved) does the attempt get to say something
/// more specific — that is how a `REQUIRES_CUSTOMER_ACTION` intent with an
/// `AUTHENTICATION_REDIRECTED` attempt still reports `AuthenticationPending`.
fn get_three_ds_leg_status(response: &AirwallexPaymentsResponse) -> AttemptStatus {
    let intent_status = get_payment_status(&response.status, &response.next_action);
    let intent_is_terminal = matches!(
        intent_status,
        AttemptStatus::Charged
            | AttemptStatus::Authorized
            | AttemptStatus::Voided
            | AttemptStatus::Failure
    );
    if intent_is_terminal {
        return intent_status;
    }
    match response
        .latest_payment_attempt
        .as_ref()
        .and_then(|attempt| attempt.status)
    {
        Some(attempt_status) => match get_attempt_status(attempt_status) {
            // The attempt knows no more than the intent does; keep the intent's reading so a
            // missing attempt status can never downgrade AuthenticationPending to Unresolved.
            AttemptStatus::Unresolved => intent_status,
            refined => refined,
        },
        None => intent_status,
    }
}

/// Resolves the Airwallex PaymentIntent id a confirm-style flow must act on.
///
/// `PaymentFlowData.connector_order_id` is the primary source — it is what `CreateOrder` stores.
/// Some gRPC entry points have no proto field able to carry it (neither
/// `RecurringPaymentServiceChargeRequest` nor
/// `PaymentMethodAuthenticationServicePreAuthenticateRequest` has an order-id field, and both
/// `ForeignTryFrom`s therefore leave `connector_order_id = None`), so those callers pass it
/// through the connector's general-purpose escape hatch, `connector_feature_data`, as
/// `{"connector_order_id": "int_..."}`. Both are request-carried; there is no config copy.
///
/// Kept in one place so every flow that confirms an intent agrees on where the id comes from.
pub(crate) fn get_connector_order_id(
    resource_common_data: &PaymentFlowData,
) -> Result<String, error_stack::Report<IntegrationError>> {
    resource_common_data
        .connector_order_id
        .clone()
        .or_else(|| {
            resource_common_data
                .connector_feature_data
                .as_ref()
                .and_then(|feature_data| {
                    feature_data
                        .peek()
                        .get("connector_order_id")
                        .and_then(|id| id.as_str().map(|id| id.to_string()))
                })
        })
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: aw_err_ctx(
                    "Airwallex confirms an existing PaymentIntent, so the intent created by \
                     CreateOrder must be identified on this call",
                    "Run CreateOrder first and pass its id, either as connector_order_id or \
                     through connector_feature_data as {\"connector_order_id\": \"int_...\"}",
                ),
            })
        })
}

// ===== PreAuthenticate: POST /pa/payment_intents/{intent_id}/confirm =====

/// The `confirm` body sent by the `PreAuthenticate` leg.
///
/// It is the same Airwallex call the inline `Authorize` path makes — on Airwallex there is no
/// authenticate-without-charging endpoint, `confirm` *is* the authorization. That is why the
/// `Authorize` that the composite loop runs after `PostAuthenticate` must be a read
/// (`is_card_three_ds_continue`), and why no arm of `next_authentication_step` may lead to a
/// second `confirm`.
#[derive(Debug, Serialize)]
pub struct AirwallexPreAuthenticateRequest {
    pub request_id: String,
    pub payment_method: AirwallexPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_data: Option<AirwallexDeviceData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

/// Both 3DS legs read the same PaymentIntent shape the `PSync` leg already deserialises.
pub type AirwallexPreAuthenticateResponse = AirwallexPaymentsResponse;
/// `GET /pa/payment_intents/{id}` returns the identical object; see
/// [`AirwallexPreAuthenticateResponse`].
pub type AirwallexPostAuthenticateResponse = AirwallexPaymentsResponse;

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: super::AirwallexRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;

        let payment_method_data = request.payment_method_data.as_ref().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "payment_method",
                context: aw_err_ctx(
                    "Airwallex authenticates by confirming the PaymentIntent, so PreAuthenticate \
                     must carry the card being authenticated",
                    "Send payment_method.card on the PreAuthenticate request",
                ),
            }
        })?;

        // Airwallex needs somewhere to send the shopper when its hosted 3DS page finishes. On the
        // pinned API version that is `payment_method.card.three_ds.return_url` and nothing else —
        // the top-level `return_url` does not take over until 2026-06-30. Prefer the
        // continuation URL the caller nominated for the post-redirect leg; fall back to the
        // router return URL. Fail locally rather than silently sending a 3DS request the shopper
        // could never return from.
        let return_url = request
            .continue_redirection_url
            .as_ref()
            .or(request.router_return_url.as_ref())
            .map(|url| url.to_string())
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: aw_err_ctx(
                    "Airwallex 3DS redirects the shopper back to \
                     payment_method.card.three_ds.return_url once the hosted authentication page \
                     completes; without it the payment cannot be finished",
                    "Send continue_redirection_url (or return_url) on the PreAuthenticate request",
                ),
            })?;

        let payment_method = get_payment_method_details(
            payment_method_data,
            &item.router_data.resource_common_data,
            None,
            Some(AirwallexThreeDsMode::Native { return_url }),
        )?;

        let auto_capture = request.is_auto_capture()?;

        let payment_method_options =
            build_payment_method_options(&payment_method, auto_capture, None);

        // Derived from the caller's reference id, not a fresh UUID: Airwallex replays the prior
        // response for a repeated request_id, which is exactly the idempotency we want if the
        // caller retries this leg. It is prefixed so it cannot collide with the CreateOrder
        // (`create_`) or Authorize (`confirm_`) request ids for the same payment.
        let request_id = format!(
            "preauth_{}",
            item.router_data
                .resource_common_data
                .connector_request_reference_id
        );

        Ok(Self {
            request_id,
            payment_method,
            payment_method_options,
            device_data: get_device_data(request.browser_info.as_ref()),
            customer_id: item
                .router_data
                .resource_common_data
                .connector_customer
                .clone(),
        })
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_three_ds_leg_status(&item.response);
        // Always a GET redirect; see `AirwallexNextAction::method` for why the connector-supplied
        // method is ignored.
        let redirection_data = build_redirection_data(&item.response.next_action);
        // Populated immediately on the frictionless path (no next_action, ds_data already
        // carries the CAVV) so the composite loop can terminate without a second network call.
        let authentication_data = build_authentication_data(&item.response);
        let intent_id = item.response.id.clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(intent_id.clone())),
                authentication_data,
                redirection_data,
                connector_response_reference_id: Some(intent_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                // Round-trip the Airwallex intent id so PostAuthenticate and the composite
                // Authorize address the same intent instead of re-deriving it.
                reference_id: Some(intent_id.clone()),
                connector_order_id: Some(intent_id),
                connector_http_status_code: Some(item.http_code),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PostAuthenticate: GET /pa/payment_intents/{intent_id} =====

/// Resolves which PaymentIntent the `PostAuthenticate` leg must retrieve.
///
/// `connector_order_reference_id` is the single source of truth: it is the id the caller carried
/// forward from `CreateOrder`/`PreAuthenticate`. Airwallex also echoes `payment_intent_id` on the
/// return URL, but a browser-supplied query parameter is not a trustworthy identifier, so it is
/// used only as a correlation check (see the response transformer) and — when the caller supplied
/// no reference at all — as the last resort that keeps the composite flow able to complete.
pub(crate) fn get_post_authenticate_intent_id<T: PaymentMethodDataTypes>(
    request: &PaymentsPostAuthenticateData<T>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    request
        .connector_order_reference_id
        .clone()
        .or_else(|| get_redirect_payment_intent_id(request))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_reference_id",
                context: aw_err_ctx(
                    "PostAuthenticate retrieves the Airwallex PaymentIntent that PreAuthenticate \
                     confirmed; without its id there is nothing to retrieve",
                    "Send connector_order_reference_id with the payment intent id returned by \
                     PreAuthenticate",
                ),
            })
        })
}

/// The `payment_intent_id` Airwallex appends to the 3DS return URL, if the caller forwarded the
/// query string. Advisory only.
fn get_redirect_payment_intent_id<T: PaymentMethodDataTypes>(
    request: &PaymentsPostAuthenticateData<T>,
) -> Option<String> {
    let params = request.redirect_response.as_ref()?.params.as_ref()?;
    serde_urlencoded::from_str::<AirwallexReturnUrlParams>(params.peek())
        .ok()?
        .payment_intent_id
}

/// The query parameters Airwallex appends to `three_ds.return_url`.
///
/// `succeeded`, `error_code` and `error_message` are deliberately not mapped to a status: the
/// Airwallex guide says to "Retrieve PaymentIntent to confirm status", and a value the browser
/// carried is not an authority on whether money moved.
#[derive(Debug, Deserialize)]
pub struct AirwallexReturnUrlParams {
    pub payment_intent_id: Option<String>,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Correlation check: the intent we retrieved must be the one the browser came back from.
        // A mismatch means we are reporting on the wrong payment, which is an unknown outcome —
        // never a Failure, because the real intent may well have been charged.
        let correlated = get_redirect_payment_intent_id(&item.router_data.request)
            .is_none_or(|redirect_intent_id| redirect_intent_id == item.response.id);

        let status = if correlated {
            get_three_ds_leg_status(&item.response)
        } else {
            AttemptStatus::Unresolved
        };
        let authentication_data = correlated
            .then(|| build_authentication_data(&item.response))
            .flatten();
        let intent_id = item.response.id.clone();

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: Some(intent_id.clone()),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                reference_id: Some(intent_id.clone()),
                connector_order_id: Some(intent_id),
                connector_http_status_code: Some(item.http_code),
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
