use crate::types::ResponseRouterData;
use crate::utils;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    pii::Email,
    request::Method,
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, StringMajorUnit},
};
use domain_types::connector_types::{
    DisputeWebhookDetailsResponse, RefundWebhookDetailsResponse, WebhookDetailsResponse,
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
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
    utils::split_full_name,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

pub(crate) const AIRWALLEX_INTEGRATION_DOC_URL: &str = "https://www.airwallex.com/docs/api";

/// The API contract every `/pa/payment_intents/*` call is pinned to via the `x-api-version`
/// header. The whole 3DS contract — `next_action` without `stage`, the fused confirm-is-
/// authorization semantics, the card-scoped `three_ds.return_url` — only holds at or after
/// `2024-06-14`; without the header the account default governs, which is nondeterministic. A
/// connector-level constant (never a literal in `build_headers`) so a legacy merchant account
/// can be pinned differently without touching the request builders.
pub const AIRWALLEX_API_VERSION: &str = "2024-06-14";

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
    type Error = Report<IntegrationError>;

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

/// Request body for the Authorize flow's confirm leg. On `x-api-version: 2024-06-14` a card 3DS
/// return never POSTs again — it is repaired to a body-less `GET /pa/payment_intents/{id}` by
/// `get_http_method`/`get_url` in `airwallex.rs` gated on [`is_card_three_ds_continue`] — so the
/// body the macro builds for that re-entry is never sent, and there is exactly one body shape.
/// (The legacy `confirm_continue` body — `AirwallexCompleteRequest` with `three_ds.acs_response` —
/// is dead on the pinned contract and was deleted; leaving it reachable is a double-charge hazard.)
#[derive(Debug, Serialize)]
pub struct AirwallexAuthorizeRequest(pub AirwallexPaymentRequest);

#[derive(Debug, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum AirwallexPaymentMethod {
    Card(AirwallexCardData),
    /// The 3DS confirm leg's card block: the plain card fields plus the card-scoped
    /// `three_ds.return_url` the pinned `2024-06-14` contract honours. Serde-untagged distinct
    /// from `Card` because only this arm serializes a `card.three_ds` sub-object.
    CardWithThreeDs(Box<AirwallexPreAuthenticateCardData>),
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
    type Error = Report<IntegrationError>;
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

/// Whether Airwallex should run native 3DS on this confirm. Only ever populated by the
/// standalone-3DS trio legs (PreAuthenticate): the 3DS dispatcher has already decided this
/// payment is `AuthenticationType::ThreeDs`, so skipping is never asked for. `None` keeps the
/// plain-Authorize request body byte-identical to what it was before this field existed.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexThreeDsAction {
    Force3ds,
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
    // Omitted unless a trio PreAuthenticate leg builds this card's confirm, so a plain
    // Authorize ships no three_ds_action. See `AirwallexThreeDsAction`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds_action: Option<AirwallexThreeDsAction>,
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

// Confirm request structure for 2-step flow (only payment method data)
#[derive(Debug, Serialize)]
pub struct AirwallexConfirmRequest {
    pub request_id: String,
    pub payment_method: AirwallexPaymentMethod,
    // Mirrors AirwallexPaymentRequest: omit rather than send an explicit null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_data: Option<AirwallexDeviceData>,
}

// Helper function to extract device data from browser info (matching Hyperswitch pattern)
fn get_device_data<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<Option<AirwallexDeviceData>, Report<IntegrationError>> {
    let browser_info = match request.get_browser_info() {
        Ok(info) => info,
        Err(_) => return Ok(None), // If browser info is not available, return None instead of erroring
    };

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

    Ok(Some(AirwallexDeviceData {
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
    }))
}

// Shared Card conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift.
fn get_card_details<T: PaymentMethodDataTypes>(
    card_data: &domain_types::payment_method_data::Card<T>,
) -> AirwallexPaymentMethod {
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
        },
        payment_method_type: AirwallexPaymentType::Card,
    })
}

// Shared BankRedirect conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. iDeal only carries
// the issuer bank; Trustly and Blik additionally need the shopper name (and, for Trustly, the
// billing country).
fn get_bankredirect_details(
    bank_redirect_data: &domain_types::payment_method_data::BankRedirectData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, Report<IntegrationError>> {
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

// Shared wallet conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift.
fn get_wallet_details(
    wallet_data: &domain_types::payment_method_data::WalletData,
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
) -> Result<AirwallexPaymentMethod, Report<IntegrationError>> {
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

// Shared PayLater conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. Mirrors the
// reference upstream `get_paylater_details`: Klarna carries billing details + country code,
// Atome carries the shopper phone with country code.
fn get_paylater_details(
    paylater_data: &domain_types::payment_method_data::PayLaterData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, Report<IntegrationError>> {
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
            utils::get_unimplemented_payment_method_error_message("airwallex"),
            Default::default()
        ))),
    }
}

// Shared BankTransfer conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. Mirrors the
// reference upstream `get_banktransfer_details`: the Indonesian bank transfer carries the
// shopper name/email, the selected bank, and the billing country code.
fn get_banktransfer_details(
    banktransfer_data: &domain_types::payment_method_data::BankTransferData,
    resource_common_data: &PaymentFlowData,
) -> Result<AirwallexPaymentMethod, Report<IntegrationError>> {
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
            utils::get_unimplemented_payment_method_error_message("airwallex"),
            Default::default()
        ))),
    }
}

/// Whether this Authorize call is the card 3DS return leg, which Airwallex finishes on
/// `/confirm_continue` with a `3ds_continue` body rather than on `/confirm`.
///
/// Gated on the payment method being a card **and** HS having supplied a `redirect_response`.
/// In practice only cards come back through Authorize, because `get_return_url` routes cards to
/// `complete_authorize_url` and APMs to `router_return_url` — but that is an emergent property of
/// a different branch. Keying the endpoint and the request body off the same explicit condition
/// means an APM that ever did return here gets its normal confirm body instead of a `three_ds`
/// payload Airwallex would reject for a PayPal or Klarna intent.
///
/// Used by both [`AirwallexAuthorizeRequest::try_from`] and `get_url` in `airwallex.rs`, so the
/// URL and the body cannot disagree about which leg is being sent.
pub(crate) fn is_card_three_ds_continue<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
        && matches!(
            request.payment_method_data,
            domain_types::payment_method_data::PaymentMethodData::Card(_)
        )
}

// Single entry point for turning the domain payment method into the Airwallex payload. Both the
// intent (AirwallexPaymentRequest) and the confirm (AirwallexConfirmRequest) builders call it, so
// the two request paths cannot drift.
fn get_payment_method_details<T: PaymentMethodDataTypes>(
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
) -> Result<AirwallexPaymentMethod, Report<IntegrationError>> {
    match payment_method_data {
        domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
            Ok(get_card_details(card_data))
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
fn build_payment_method_options(
    payment_method: &AirwallexPaymentMethod,
    auto_capture: bool,
    authorization_type: Option<AirwallexCardAuthorizationType>,
    three_ds_action: Option<AirwallexThreeDsAction>,
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
        // authorization_type only ever reaches Airwallex through this arm. Same for
        // three_ds_action: it is only meaningful on a card confirm. `CardWithThreeDs` is the
        // trio confirm leg's card arm — it carries the same options block.
        AirwallexPaymentMethod::Card(_) | AirwallexPaymentMethod::CardWithThreeDs(_) => {
            Some(AirwallexPaymentOptions {
                card: Some(AirwallexCardOptions {
                    auto_capture: Some(auto_capture),
                    authorization_type,
                    three_ds_action,
                }),
                klarna: None,
                atome: None,
            })
        }
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
    type Error = Report<IntegrationError>;

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

        let payment_method = get_payment_method_details(
            &item.router_data.request.payment_method_data,
            &item.router_data.resource_common_data,
            item.router_data
                .request
                .customer_name
                .clone()
                .map(Secret::new),
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
            build_payment_method_options(&payment_method, auto_capture, authorization_type, None);

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
            (None, None, get_device_data(&item.router_data.request)?)
        };

        // Per-method return_url (mirrors native HS): card 3DS must come back through the Authorize
        // completion leg (`confirm_continue`), so point Airwallex at `complete_authorize_url`; APM
        // redirects (wallets/bank-redirect/paylater) return through PSync via `router_return_url`.
        let return_url = match &item.router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(_) => item
                .router_data
                .request
                .complete_authorize_url
                .clone()
                .or_else(|| item.router_data.request.get_router_return_url().ok()),
            _ => item.router_data.request.get_router_return_url().ok(),
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

/// Build the Authorize request body. Every reachable Authorize invocation on the pinned
/// `2024-06-14` contract POSTs the plain confirm body; the 3DS return leg (`redirect_response`
/// populated for a card) is served by the body-less `GET /pa/payment_intents/{id}` that
/// `get_http_method` in `airwallex.rs` substitutes, so this body is never sent in that case.
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
    > for AirwallexAuthorizeRequest
{
    type Error = Report<IntegrationError>;

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
        Ok(Self(AirwallexPaymentRequest::try_from(item)?))
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
    pub status: Option<String>, // Changed from AirwallexPaymentStatus to String to handle different values
    pub amount: Option<FloatMajorUnit>,
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    pub authorization_code: Option<String>,
    pub network_transaction_id: Option<String>,
    pub processor_response: Option<AirwallexProcessorResponse>,
    // 3DS outcome block — the only place Airwallex surfaces cavv/eci/version/xid. The
    // standalone-3DS trio (PreAuthenticate/PostAuthenticate) builds `AuthenticationData` from it.
    pub authentication_data: Option<AirwallexAttemptAuthenticationData>,
    pub failure_code: Option<String>,
    pub failure_details: Option<String>,
    pub payment_method_transaction_id: Option<String>,
    pub provider_original_response_code: Option<String>,
    pub provider_original_response_description: Option<String>,
    pub capture_requested_at: Option<String>,
    pub payment_intent_id: Option<String>,
    pub merchant_order_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// `latest_payment_attempt.authentication_data` on the pinned `2024-06-14` contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexAttemptAuthenticationData {
    pub ds_data: Option<AirwallexDsData>,
    /// Fraud-screening verdict (`{ action, score }` on the wire; kept as a raw object because
    /// the connector never branches on it — it only round-trips for observability).
    pub fraud_data: Option<serde_json::Value>,
    pub avs_result: Option<String>,
    pub cvc_result: Option<String>,
}

/// The native-3DS Directory Server payload. `ds_trans_id` / `three_ds_server_transaction_id`
/// are NOT exposed by Airwallex on native 3DS (only echoed on external 3DS), so there are no
/// fields for them here; `liability_shift_indicator` and `frictionless` feed the derived
/// `TransactionStatus`, never `AuthenticationData` directly (no slot exists for them).
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDsData {
    pub version: Option<String>,
    pub liability_shift_indicator: Option<String>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub xid: Option<Secret<String>>,
    pub frictionless: Option<String>,
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
    // Any status string Airwallex adds after this mapping ships. Without the catch-all an
    // unrecognised status is a deserialisation error that surfaces as a generic failure — a
    // claim the money did not move, which we cannot make. It maps to `Unresolved` in
    // `get_payment_status`; never to `Failure`.
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AirwallexNextActionType {
    Redirect,
    DeviceDataCollection,
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
fn get_payment_status(
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
) -> AttemptStatus {
    match status {
        AirwallexPaymentStatus::Succeeded => AttemptStatus::Charged,
        AirwallexPaymentStatus::Failed => AttemptStatus::Failure,
        AirwallexPaymentStatus::Processing => AttemptStatus::Pending,
        AirwallexPaymentStatus::RequiresPaymentMethod => AttemptStatus::PaymentMethodAwaited,
        AirwallexPaymentStatus::RequiresCustomerAction => {
            next_action.as_ref().map_or(
                // "Action required" with no action is a contract violation. Guessing
                // `AuthenticationPending` here would park the payment waiting for a
                // redirect Airwallex never handed out; `Unresolved` (not `Failure` —
                // the money state is unknown) makes it an observable contract breach.
                AttemptStatus::Unresolved,
                |action| match action.action_type {
                    AirwallexNextActionType::DeviceDataCollection => {
                        AttemptStatus::DeviceDataCollectionPending
                    }
                    AirwallexNextActionType::Redirect | AirwallexNextActionType::Other => {
                        AttemptStatus::AuthenticationPending
                    }
                },
            )
        }
        AirwallexPaymentStatus::RequiresCapture => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Authorized => AttemptStatus::Authorized,
        AirwallexPaymentStatus::Paid => AttemptStatus::Charged,
        AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
        AirwallexPaymentStatus::CaptureRequested => AttemptStatus::Charged,
        AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
        AirwallexPaymentStatus::Pending => AttemptStatus::Pending,
        // Caught by the `#[serde(other)]` catch-all: a status Airwallex added after this
        // mapping shipped. Unknown is unknown — never `Failure`.
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
    type Error = Report<ConnectorError>;

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
    type Error = Report<ConnectorError>;

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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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
    pub currency: Option<Currency>,                 // Currency code
    pub reason: Option<String>,                     // Refund reason
    pub status: AirwallexRefundStatus,              // RECEIVED, ACCEPTED, SETTLED, FAILED
    pub created_at: Option<String>,                 // Creation timestamp
    pub updated_at: Option<String>,                 // Update timestamp
    pub acquirer_reference_number: Option<String>,  // Network reference
    pub failure_details: Option<serde_json::Value>, // Error details if failed
    pub metadata: Option<serde_json::Value>,        // Additional metadata
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexRefundStatus {
    Received,
    Accepted,
    Settled,
    Failed,
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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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
    type Error = Report<ConnectorError>;

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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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

// Implementation for confirm request type (2-step flow)
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
    > for AirwallexConfirmRequest
{
    type Error = Report<IntegrationError>;

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
        // Confirm flow for 2-step process (not currently used in UCS)

        let payment_method = get_payment_method_details(
            &item.router_data.request.payment_method_data,
            &item.router_data.resource_common_data,
            item.router_data
                .request
                .customer_name
                .clone()
                .map(Secret::new),
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
            build_payment_method_options(&payment_method, auto_capture, authorization_type, None);

        let device_data = get_device_data(&item.router_data.request)?;

        Ok(Self {
            request_id: format!(
                "confirm_{}",
                item.router_data.resource_common_data.payment_id
            ),
            payment_method,
            payment_method_options,
            return_url: item.router_data.request.get_router_return_url().ok(),
            device_data,
        })
    }
}

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
    type Error = Report<IntegrationError>;

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
                    .collect::<Result<Vec<_>, Report<IntegrationError>>>()?;
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
    type Error = Report<ConnectorError>;

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
            // Caught by the `#[serde(other)]` catch-all; unknown is never terminal,
            // never a failure claim.
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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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
    type Error = Report<IntegrationError>;

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
        let payment_method = match &item.router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                get_card_details(card_data)
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
                three_ds_action: None,
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
    type Error = Report<ConnectorError>;

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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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
    type Error = Report<IntegrationError>;

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
    type Error = Report<ConnectorError>;

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

// ===== STANDALONE 3DS TRIO (PreAuthenticate / PostAuthenticate) =====
//
// Mechanism A — standalone 3DS trio, on the pinned `x-api-version: 2024-06-14` contract.
// Airwallex fuses authentication and authorization: `POST /confirm` IS the authorization — it
// charges — so the trio maps to exactly two calls:
//
// * `PreAuthenticate` → `POST /pa/payment_intents/{id}/confirm` (the charging leg). There is no
//   Airwallex counterpart for `Authenticate` on this contract; the marker stays
//   `not_implemented` and `next_authentication_step` never returns it.
// * `PostAuthenticate` → `GET /pa/payment_intents/{id}` (a read: never charges), run when the
//   browser returns from Airwallex's hosted DDC/ACS page.
//
// The composite loop's PreAuthenticate break fires only on `redirection_data.is_some()`, so the
// confirm response ALWAYS emits a redirect: the challenge gets Airwallex's GET `next_action`
// URL; a frictionless-settled or issuer-declined intent gets a `RedirectForm::Uri` at the
// merchant's `return_url`, converting the frictionless journey into a redirect journey whose
// return runs PostAuthenticate and finishes with the read-only Authorize re-entry. Without that
// fallback the loop would fall through to an Authorize that re-confirms an already-settled
// intent — a double-charge hazard the gateway only blunts, not removes.

/// Device-data builder for the trio legs. [`get_device_data`] swallows a missing
/// `browser_info` into `None` because on the plain Authorize path device data is best-effort
/// fraud context; on the confirm-for-3DS leg (G-threeds-03@Card) it is a hard requirement —
/// without it Airwallex cannot run DDC/challenge correctly.
fn get_device_data_required(
    browser_info: &domain_types::router_request_types::BrowserInformation,
) -> Result<Option<AirwallexDeviceData>, Report<IntegrationError>> {
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

    Ok(Some(AirwallexDeviceData {
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
    }))
}

/// The one URL the whole 3DS dance returns to and the redirect fallback is pointed at:
/// `continue_redirection_url`, falling back to `router_return_url`. On the composite journey
/// the PreAuthenticate forward fills `continue_redirection_url`; the granular RPC fills
/// `router_return_url`.
fn resolve_threeds_return_url(
    router_return_url: Option<&Url>,
    continue_redirection_url: Option<&Url>,
) -> Result<Url, Report<IntegrationError>> {
    continue_redirection_url
        .or(router_return_url)
        .cloned()
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: aw_err_ctx(
                    "The 3DS confirm needs the URL the shopper's browser returns to after \
                     Airwallex's hosted DDC/challenge page (payment_method.card.three_ds.\
                     return_url), and the frictionless fallback redirect is pointed at the \
                     same URL",
                    "Send continue_redirection_url (or router_return_url on the granular RPC)",
                ),
            })
        })
}

/// 3DS outcome → `TransactionStatus`, derived from the settle state plus `ds_data` (Airwallex
/// never exposes a raw transStatus). Never defaults: `TransactionStatus::default()` is
/// `Failure`, which would claim the authentication failed when it merely cannot be read.
fn airwallex_three_ds_trans_status(
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
    ds: Option<&AirwallexDsData>,
    attempt_declined: bool,
) -> Option<common_enums::TransactionStatus> {
    use common_enums::TransactionStatus;
    match ds {
        // / The challenge (or DDC) is still in flight: the redirect is out but the browser
        // has not come back.
        _ if matches!(status, AirwallexPaymentStatus::RequiresCustomerAction)
            && next_action.is_some() =>
        {
            Some(TransactionStatus::ChallengeRequired)
        }
        // Issuer/provider decline at the confirm — authentication-level failure.
        _ if attempt_declined => Some(TransactionStatus::Failure),
        Some(ds) if ds.cavv.is_some() => {
            if ds.liability_shift_indicator.as_deref() == Some("Y") {
                Some(TransactionStatus::Success)
            } else {
                // Proof of attempt without a liability shift.
                Some(TransactionStatus::NotVerified)
            }
        }
        // No CAVV on a settled intent: the network/issuer could not or need not verify.
        Some(_) => Some(TransactionStatus::VerificationNotPerformed),
        None => None,
    }
}

/// Build the canonical `AuthenticationData` from `latest_payment_attempt.authentication_data.
/// ds_data`. Field sourcing follows the connector's 3DS contract reference table: cavv is a
/// secret; `message_version` is parsed (never hardcoded); `ds_trans_id` /
/// `three_ds_server_transaction_id` are not exposed by Airwallex on native 3DS, so they stay
/// `None`; `transaction_id` carries the 3DS1 XID when Airwallex fell back to 3DS1.
fn build_ds_authentication_data(
    http_code: u16,
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
    attempt: Option<&AirwallexPaymentAttempt>,
) -> Result<Option<AuthenticationData>, Report<ConnectorError>> {
    let Some(attempt) = attempt else {
        return Ok(None);
    };
    let attempt_declined = attempt.status.as_deref() == Some("DECLINED");
    let Some(auth_data) = attempt.authentication_data.as_ref() else {
        // A decline may land with no authentication_data block at all; still report the N.
        return Ok(attempt_declined.then(|| AuthenticationData {
            trans_status: Some(common_enums::TransactionStatus::Failure),
            eci: None,
            cavv: None,
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: None,
            message_version: None,
            ds_trans_id: None,
            acs_transaction_id: None,
            transaction_id: None,
            network_params: None,
            exemption_indicator: None,
            created_at: None,
            challenge_code: None,
            challenge_cancel: None,
            challenge_code_reason: None,
            message_extension: None,
            authentication_type: None,
        }));
    };
    let ds = auth_data.ds_data.as_ref();
    let message_version = ds
        .and_then(|ds| ds.version.as_deref())
        .map(|version| {
            std::str::FromStr::from_str(version).map_err(|_| {
                let detail =
                    "airwallex: ds_data.version did not parse as a semantic version; confirm \
                     the pinned API contract has not changed"
                        .to_string();
                Report::new(utils::response_deserialization_fail(
                    http_code,
                    detail.clone(),
                ))
                .attach_printable(format!(
                    "failed to parse ds_data.version '{version}' as SemanticVersion"
                ))
            })
        })
        .transpose()?;
    Ok(Some(AuthenticationData {
        trans_status: airwallex_three_ds_trans_status(status, next_action, ds, attempt_declined),
        eci: ds.and_then(|ds| ds.eci.clone()),
        cavv: ds.and_then(|ds| ds.cavv.clone()),
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: None,
        message_version,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id: ds.and_then(|ds| ds.xid.as_ref().map(|xid| xid.peek().to_string())),
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }))
}

/// The confirm response always emits a redirect (see the trio module comment): the challenge
/// leg gets Airwallex's hosted GET page; a terminal intent gets the merchant's return URL.
fn pre_authenticate_redirection_data(
    next_action: &Option<AirwallexNextAction>,
    return_url: &Url,
) -> Option<Box<RedirectForm>> {
    build_redirection_data(next_action).or_else(|| {
        Some(Box::new(RedirectForm::Uri {
            uri: return_url.to_string(),
        }))
    })
}

/// Request body for the trio `PreAuthenticate` leg — the charging `confirm`, shaped exactly
/// like a card `Authorize` confirm plus `three_ds_action` and the 3DS return URL.
#[derive(Debug, Serialize)]
pub struct AirwallexPreAuthenticateRequest {
    pub request_id: String,
    pub payment_method: AirwallexPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
    /// Top-level `return_url`: kept for parity with the Authorize confirm body. On
    /// `2024-06-14` the 3DS return URL Airwallex honours is the CARD-scoped
    /// `card.three_ds.return_url`; the top-level key only becomes the 3DS return URL on later
    /// versions — Duplicated here so the two confirm shapes cannot drift apart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_data: Option<AirwallexDeviceData>,
}

/// Response of the trio's `PreAuthenticate` leg — the same PaymentIntent object as the
/// Authorize confirm response. A DISTINCT NAME (not the `AirwallexPaymentsResponse` alias):
/// `create_all_prerequisites!` stamps one templating helper per `response_body` type, so two
/// api rows sharing a type would collide.
#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AirwallexPreAuthenticateResponse(pub AirwallexPaymentsResponse);

/// Response of the trio's `PostAuthenticate` leg — the same PaymentIntent object the PSync
/// retrieve deserialises. Distinct name for the same macro-collide reason as
/// [`AirwallexPreAuthenticateResponse`] (an api row sharing `AirwallexSyncResponse` would
/// redefine its templating helper).
#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AirwallexPostAuthenticateResponse(pub AirwallexPaymentsResponse);

/// PostAuthenticate is a body-less `GET /pa/payment_intents/{id}`. The connector macro arm
/// requires a `curl_request` body type to accept a generic `flow_request`
/// (`PaymentsPostAuthenticateData<T>`), so this stands in and the macro sends it as the unused
/// GET body shell (serialized `{}` content on a GET; no semantic payload).
#[derive(Debug, Serialize)]
pub struct AirwallexPostAuthenticateRequest {}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        super::AirwallexRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for AirwallexPostAuthenticateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        _item: super::AirwallexRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// Card-scoped 3DS options nested under `payment_method.card`, the ONLY place the pinned
/// `2024-06-14` contract reads the 3DS return URL from.
#[derive(Debug, Serialize)]
pub struct AirwallexThreeDsOptions {
    pub return_url: String,
}

/// Card payment-method block for the trio confirm — the plain card fields plus
/// `card.three_ds.return_url`.
#[derive(Debug, Serialize)]
pub struct AirwallexPreAuthenticateCardData {
    pub card: AirwallexPreAuthenticateCardDetails,
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
}

#[derive(Debug, Serialize)]
pub struct AirwallexPreAuthenticateCardDetails {
    pub number: Secret<String>,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    pub cvc: Secret<String>,
    pub name: Option<Secret<String>>,
    pub three_ds: AirwallexThreeDsOptions,
}

/// `PaymentMethodData::Card` with the 3DS return URL folded in, for the trio confirm leg.
fn get_card_details_with_return_url<T: PaymentMethodDataTypes>(
    card_data: &domain_types::payment_method_data::Card<T>,
    return_url: &Url,
) -> AirwallexPaymentMethod {
    AirwallexPaymentMethod::CardWithThreeDs(Box::new(AirwallexPreAuthenticateCardData {
        card: AirwallexPreAuthenticateCardDetails {
            number: Secret::new(card_data.card_number.peek().to_string()),
            expiry_month: card_data.card_exp_month.clone(),
            expiry_year: card_data.get_expiry_year_4_digit(),
            cvc: card_data.card_cvc.clone(),
            name: card_data
                .card_holder_name
                .clone()
                .map(|name| Secret::new(name.expose())),
            three_ds: AirwallexThreeDsOptions {
                return_url: return_url.to_string(),
            },
        },
        payment_method_type: AirwallexPaymentType::Card,
    }))
}

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
    type Error = Report<IntegrationError>;

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

        // G-threeds-01@Card: the confirm is the authorization, so the capture disposition is
        // locked in here; the capture methods Airwallex cannot serve must fail pre-HTTP.
        let auto_capture = request.is_auto_capture().map_err(|err| {
            err.change_context(IntegrationError::CaptureMethodNotSupported {
                context: aw_err_ctx(
                    "A 3DS confirm carries auto_capture; Airwallex serves only Automatic, \
                     Manual and SequentialAutomatic captures",
                    "Use capture_method automatic, sequential_automatic or manual",
                ),
            })
        })?;

        // The trio is a card-only path: `next_authentication_step` only opens it for
        // (ThreeDs, Card), and a non-card payment that reached the granular RPC directly has
        // no 3DS leg to run.
        let payment_method = match request.payment_method_data.as_ref() {
            Some(domain_types::payment_method_data::PaymentMethodData::Card(card_data)) => {
                // G-threeds-04@Card (also the frictionless fallback's target).
                let return_url = resolve_threeds_return_url(
                    request.router_return_url.as_ref(),
                    request.continue_redirection_url.as_ref(),
                )?;
                get_card_details_with_return_url(card_data, &return_url)
            }
            _ => {
                return Err(error_stack::report!(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("airwallex"),
                    Default::default()
                )));
            }
        };

        // G-threeds-03@Card: device data is a hard requirement on the 3DS confirm.
        let browser_info =
            request
                .browser_info
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "browser_info",
                    context: aw_err_ctx(
                        "The 3DS confirm must carry device_data so Airwallex can run device data \
                     collection and, when the issuer demands it, the ACS challenge",
                        "Send browser_info (user agent, accept header, language, screen metrics)",
                    ),
                })?;
        let device_data = get_device_data_required(&browser_info)?;

        let payment_method_options = build_payment_method_options(
            &payment_method,
            auto_capture,
            None,
            Some(AirwallexThreeDsAction::Force3ds),
        );

        let return_url = request
            .continue_redirection_url
            .as_ref()
            .or(request.router_return_url.as_ref())
            .map(ToString::to_string);

        Ok(Self {
            request_id: format!(
                "confirm_{}",
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
            ),
            payment_method,
            payment_method_options,
            return_url,
            device_data,
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
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Transparent newtype over `AirwallexPaymentsResponse`; unwrap once and work on the
        // intent object directly.
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let item = ResponseRouterData::<AirwallexPaymentsResponse, Self> {
            response: response.0,
            router_data,
            http_code,
        };
        let attempt = item.response.latest_payment_attempt.as_ref();
        // The attempt refines only a NON-terminal intent: the intent wins terminal
        // decisions. A challenge arm (REQUIRES_CUSTOMER_ACTION) with an attempt that has
        // already reached AUTHORIZED/CAPTURED is settled — the shopper never left.
        let attempt_terminal = match attempt.and_then(|attempt| attempt.status.as_deref()) {
            Some("AUTHORIZED") => Some(AttemptStatus::Authorized),
            Some("CAPTURED") => Some(AttemptStatus::Charged),
            Some("DECLINED") => Some(AttemptStatus::AuthorizationFailed),
            Some("FAILED") => Some(AttemptStatus::Failure),
            _ => None,
        };
        let status = if matches!(
            item.response.status,
            AirwallexPaymentStatus::RequiresCustomerAction
        ) {
            attempt_terminal.unwrap_or(AttemptStatus::AuthenticationPending)
        } else {
            get_payment_status(&item.response.status, &item.response.next_action)
        };

        // G-threeds-04@Card: the frictionless/decline fallback redirect is pointed at the
        // URL the confirm was told to return to; missing at this point means no 3DS journey
        // was ever requested, so a challenge arm cannot be served either.
        let return_url = resolve_threeds_return_url(
            item.router_data.request.router_return_url.as_ref(),
            item.router_data.request.continue_redirection_url.as_ref(),
        )
        .map_err(|err| {
            err.change_context(utils::response_deserialization_fail(
                item.http_code,
                "airwallex: confirm response could not be mapped without the 3DS return URL \
                 (challenge redirect target and frictionless fallback are both anchored to it)",
            ))
        })?;

        let redirection_data =
            pre_authenticate_redirection_data(&item.response.next_action, &return_url);

        let authentication_data = build_ds_authentication_data(
            item.http_code,
            &item.response.status,
            &item.response.next_action,
            attempt,
        )?;

        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(intent_id.clone())),
                authentication_data,
                redirection_data,
                connector_response_reference_id: item
                    .response
                    .payment_intent_id
                    .or(Some(intent_id)),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Parse the `payment_intent_id` the browser-leg redirect handed back: verbatim from the
/// `params` query string when present (the lightweight return), otherwise recovered from the
/// `payload` HTML document by lifting the raw value around the parameter name — a plain
/// string scan, tolerant of however the URL was percent-encoded.
fn extract_payment_intent_id_from_redirect(
    redirect: &domain_types::connector_types::ContinueRedirectionResponse,
) -> Option<String> {
    let from_query = |query: &str| -> Option<String> {
        query.split(['?', '&']).find_map(|kv| {
            kv.strip_prefix("payment_intent_id=")
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
    };
    redirect
        .params
        .as_ref()
        .and_then(|params| from_query(params.peek()))
        .or_else(|| {
            redirect.payload.as_ref().and_then(|payload| {
                payload
                    .peek()
                    .to_string()
                    .split(['?', '&', '"'])
                    .find_map(from_query)
            })
        })
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let item = ResponseRouterData::<AirwallexPaymentsResponse, Self> {
            response: response.0,
            router_data,
            http_code,
        };
        let attempt = item.response.latest_payment_attempt.as_ref();

        // Same refinement rule as PreAuthenticate: the intent wins terminal decisions; the
        // attempt refines a non-terminal intent.
        let attempt_terminal = match attempt.and_then(|attempt| attempt.status.as_deref()) {
            Some("AUTHORIZED") => Some(AttemptStatus::Authorized),
            Some("CAPTURED") => Some(AttemptStatus::Charged),
            Some("DECLINED") => Some(AttemptStatus::AuthorizationFailed),
            Some("FAILED") => Some(AttemptStatus::Failure),
            _ => None,
        };
        let mut status = if matches!(
            item.response.status,
            AirwallexPaymentStatus::RequiresCustomerAction
        ) {
            attempt_terminal.unwrap_or(AttemptStatus::AuthenticationPending)
        } else {
            get_payment_status(&item.response.status, &item.response.next_action)
        };

        // G-threeds-05@Card: the intent the retrieve returned must be the intent the browser
        // handed back. The `succeeded`/`error_*` return-URL parameters are advisory — the
        // retrieved intent is the only authority — but a MISMATCHED intent id means we are
        // reading the wrong payment entirely; never trust that as terminal. A retrieval that
        // cannot be correlated is `Unresolved`, per the unknown-status rule.
        if let Some(returned_intent_id) = item
            .router_data
            .request
            .redirect_response
            .as_ref()
            .and_then(extract_payment_intent_id_from_redirect)
        {
            if returned_intent_id != item.response.id {
                status = AttemptStatus::Unresolved;
            }
        }

        // This is the one leg expected to always carry populated `authentication_data` on
        // success: the ds_data is fully populated on the settled intent.
        let authentication_data = build_ds_authentication_data(
            item.http_code,
            &item.response.status,
            &item.response.next_action,
            attempt,
        )?;

        let intent_id = item.response.id;

        // PostAuthenticate carries the settled transaction status in
        // `resource_common_data.status` so the very next read (PSync/Capture) lines up with
        // what the confirm actually settled to — while the trio's AuthenticationData carries
        // the 3DS outcome the caller needs to finalise.
        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                // `PostAuthenticateResponse` has no resource_id; the intent id — the single
                // id this payment runs on across the trio — goes out here.
                connector_response_reference_id: Some(intent_id),
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== INCOMING WEBHOOK TYPES =====
//
// Airwallex webhook transport (spec §15): a POST whose body is the envelope
// `AirwallexWebhookEnvelope`, signed with `x-signature` = lowercase-hex HMAC-SHA256 of
// `x-timestamp header bytes ++ raw body bytes` under the per-notification-URL secret. The
// resource lives at `data.object`; `name` (the event) decides which struct binding it gets —
// PaymentIntent and PaymentAttempt share id/amount/currency/status keys, so an untagged parse
// of an attempt silently binds as an intent. Never `#[serde(untagged)]`, never
// `deny_unknown_fields` (the object is a full Retrieve-API body that grows with API versions).

/// The webhook envelope. `id` is an opaque dedupe key (two documented forms: `evt_100_…` and a
/// bare 32-hex string) — parse or validate nothing about it. `account_id` arrives spelled both
/// `account_id` and `accountId` across Airwallex's own docs, hence the alias.
#[derive(Debug, Deserialize)]
pub struct AirwallexWebhookEnvelope {
    pub id: String,
    pub name: AirwallexWebhookEvent,
    #[serde(default, alias = "accountId")]
    pub account_id: Option<String>,
    pub data: AirwallexWebhookData,
    pub created_at: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AirwallexWebhookData {
    /// The resource, bound per family by `name` (never untagged — see the module comment).
    pub object: serde_json::Value,
}

/// The full closed-set event catalogue of spec §15.4. The four modelled families
/// (`payment_intent.*`, `payment_attempt.*`, `refund.*`, `payment_dispute.*`) drive the
/// payment/refund/dispute webhooks; the remaining families are recognised (so an unknown name
/// is never a hard error) but not modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AirwallexWebhookEvent {
    #[serde(rename = "payment_intent.created")]
    PaymentIntentCreated,
    #[serde(rename = "payment_intent.requires_payment_method")]
    PaymentIntentRequiresPaymentMethod,
    /// The one event whose `AttemptStatus` derives from `data.object.status` rather than the
    /// name (the name carries no terminal information).
    #[serde(rename = "payment_intent.updated")]
    PaymentIntentUpdated,
    #[serde(rename = "payment_intent.requires_customer_action")]
    PaymentIntentRequiresCustomerAction,
    #[serde(rename = "payment_intent.requires_capture")]
    PaymentIntentRequiresCapture,
    #[serde(rename = "payment_intent.pending")]
    PaymentIntentPending,
    #[serde(rename = "payment_intent.pending_review")]
    PaymentIntentPendingReview,
    #[serde(rename = "payment_intent.succeeded")]
    PaymentIntentSucceeded,
    #[serde(rename = "payment_intent.cancelled")]
    PaymentIntentCancelled,
    /// Not terminal for the intent — an attempt failed; the shopper can retry. Reported as
    /// `Failure` for the attempt UCS tracks.
    #[serde(rename = "payment_intent.payment_failed")]
    PaymentIntentPaymentFailed,

    #[serde(rename = "payment_attempt.received")]
    PaymentAttemptReceived,
    #[serde(rename = "payment_attempt.authentication_redirected")]
    PaymentAttemptAuthenticationRedirected,
    #[serde(rename = "payment_attempt.authentication_failed")]
    PaymentAttemptAuthenticationFailed,
    #[serde(rename = "payment_attempt.pending_authorization")]
    PaymentAttemptPendingAuthorization,
    #[serde(rename = "payment_attempt.authorized")]
    PaymentAttemptAuthorized,
    #[serde(rename = "payment_attempt.authorization_failed")]
    PaymentAttemptAuthorizationFailed,
    #[serde(rename = "payment_attempt.capture_requested")]
    PaymentAttemptCaptureRequested,
    #[serde(rename = "payment_attempt.capture_failed")]
    PaymentAttemptCaptureFailed,
    #[serde(rename = "payment_attempt.settled")]
    PaymentAttemptSettled,
    #[serde(rename = "payment_attempt.paid")]
    PaymentAttemptPaid,
    #[serde(rename = "payment_attempt.cancelled")]
    PaymentAttemptCancelled,
    #[serde(rename = "payment_attempt.expired")]
    PaymentAttemptExpired,
    #[serde(rename = "payment_attempt.risk_declined")]
    PaymentAttemptRiskDeclined,
    #[serde(rename = "payment_attempt.failed_to_process")]
    PaymentAttemptFailedToProcess,

    #[serde(rename = "refund.received")]
    RefundReceived,
    /// `Accepted` maps to `RefundStatus::Pending`, never `Success` — the exact mapping RSync
    /// already reports through the shared `From<AirwallexRefundStatus>` block, so a refund can
    /// never flap Pending → Success → Pending as the two paths race.
    #[serde(rename = "refund.accepted")]
    RefundAccepted,
    #[serde(rename = "refund.settled")]
    RefundSettled,
    #[serde(rename = "refund.failed")]
    RefundFailed,

    #[serde(rename = "payment_dispute.requires_response")]
    PaymentDisputeRequiresResponse,
    #[serde(rename = "payment_dispute.challenged")]
    PaymentDisputeChallenged,
    #[serde(rename = "payment_dispute.accepted")]
    PaymentDisputeAccepted,
    #[serde(rename = "payment_dispute.expired")]
    PaymentDisputeExpired,
    /// Judgement call (spec §15.11.9): UCS has no pending-closure variant; Airwallex
    /// auto-accepts the pre-arbitration, so the map lands on `DisputeAccepted`.
    #[serde(rename = "payment_dispute.pending_closure")]
    PaymentDisputePendingClosure,
    /// Judgement call (spec §15.11.9): evidence is with the scheme → `DisputeChallenged`.
    #[serde(rename = "payment_dispute.pending_decision")]
    PaymentDisputePendingDecision,
    #[serde(rename = "payment_dispute.won")]
    PaymentDisputeWon,
    #[serde(rename = "payment_dispute.lost")]
    PaymentDisputeLost,
    /// Judgement call (spec §15.11.9): the dispute was withdrawn and the merchant credited →
    /// `DisputeCancelled`.
    #[serde(rename = "payment_dispute.reversed")]
    PaymentDisputeReversed,

    // --- Recognised but NOT modelled in this unit → IncomingWebhookEventUnspecified ---
    // payment_consent.* is the mandate lifecycle; UCS has EventType::{MandateActive, …} slots
    // but this unit maps them to unspecified deliberately.
    // TODO(mandate-webhooks): wire payment_consent.* to the mandate events.
    #[serde(rename = "payment_consent.created")]
    PaymentConsentCreated,
    #[serde(rename = "payment_consent.updated")]
    PaymentConsentUpdated,
    #[serde(rename = "payment_consent.pending")]
    PaymentConsentPending,
    #[serde(rename = "payment_consent.verified")]
    PaymentConsentVerified,
    #[serde(rename = "payment_consent.disabled")]
    PaymentConsentDisabled,
    #[serde(rename = "payment_consent.paused")]
    PaymentConsentPaused,
    #[serde(rename = "payment_consent.requires_payment_method")]
    PaymentConsentRequiresPaymentMethod,
    #[serde(rename = "payment_consent.requires_customer_action")]
    PaymentConsentRequiresCustomerAction,
    #[serde(rename = "payment_consent.verification_failed")]
    PaymentConsentVerificationFailed,
    #[serde(rename = "customer.created")]
    CustomerCreated,
    #[serde(rename = "customer.updated")]
    CustomerUpdated,
    #[serde(rename = "payment_method.created")]
    PaymentMethodCreated,
    #[serde(rename = "payment_method.updated")]
    PaymentMethodUpdated,
    #[serde(rename = "payment_method.attached")]
    PaymentMethodAttached,
    #[serde(rename = "payment_method.detached")]
    PaymentMethodDetached,
    #[serde(rename = "payment_method.disabled")]
    PaymentMethodDisabled,
    #[serde(rename = "payment_link.created")]
    PaymentLinkCreated,
    #[serde(rename = "payment_link.paid")]
    PaymentLinkPaid,
    #[serde(rename = "fraud.merchant_notified")]
    FraudMerchantNotified,
    #[serde(rename = "funds_split.created")]
    FundsSplitCreated,
    #[serde(rename = "funds_split.failed")]
    FundsSplitFailed,
    #[serde(rename = "funds_split.released")]
    FundsSplitReleased,
    #[serde(rename = "funds_split.settled")]
    FundsSplitSettled,
    #[serde(rename = "pos.terminal.activated")]
    PosTerminalActivated,
    #[serde(rename = "pos.terminal.deactivated")]
    PosTerminalDeactivated,
    #[serde(rename = "pos.terminal.terminated")]
    PosTerminalTerminated,
    #[serde(rename = "pos.terminal.updated")]
    PosTerminalUpdated,
    #[serde(rename = "pos.terminal.admin_password_status.reset_requested")]
    PosTerminalAdminPasswordResetRequested,
    #[serde(rename = "pos.terminal.admin_password_status.activated")]
    PosTerminalAdminPasswordActivated,
    #[serde(rename = "pos.terminal.admin_password_status.locked")]
    PosTerminalAdminPasswordLocked,
    #[serde(rename = "pos.terminal.refund_password_status.reset_requested")]
    PosTerminalRefundPasswordResetRequested,
    #[serde(rename = "pos.terminal.refund_password_status.activated")]
    PosTerminalRefundPasswordActivated,
    #[serde(rename = "pos.terminal.refund_password_status.locked")]
    PosTerminalRefundPasswordLocked,
    #[serde(rename = "pos.terminal.refund_password_status.opted_out")]
    PosTerminalRefundPasswordOptedOut,
    /// An event name Airwallex added after this mapping shipped. Recognition is the contract:
    /// get_event_type maps it to `IncomingWebhookEventUnspecified`, never an error.
    #[serde(other)]
    Unknown,
}

/// Wire status of the `data.object` of a `payment_attempt.*` event (spec §15.5.2) — the same
/// closed set as the in-tree attempt `status` string, modelled here as an enum so the
/// cross-check in the webhook payment transformer is exhaustive. Five terminal failures
/// collapse into `FAILED` on the wire, which is why the webhook `AttemptStatus` derives from
/// the event NAME first and from this status only as a cross-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexAttemptWebhookStatus {
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

/// `data.object` of a `payment_attempt.*` event (spec §15.5.2). Fields absent from the
/// verbatim sample are Option. `id` is the `att_…` attempt id and must NEVER be written into a
/// UCS connector_transaction_id — `payment_intent_id` is the transaction id, and
/// [`super::Airwallex`] refuses a webhook that lacks it (G-incomingwebhook-03@Card).
#[derive(Debug, Deserialize)]
pub struct AirwallexAttemptWebhookObject {
    pub id: String,
    pub payment_intent_id: Option<String>,
    pub merchant_order_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub status: Option<AirwallexAttemptWebhookStatus>,
    pub captured_amount: Option<FloatMajorUnit>,
    pub failure_code: Option<String>,
    #[serde(default)]
    pub failure_details: Option<AirwallexFailureDetails>,
    pub payment_method_transaction_id: Option<String>,
    pub provider_original_response_description: Option<String>,
    pub payment_consent_id: Option<String>,
}

/// The `{code, message, trace_id}` failure block of a failed attempt, and the
/// `{code, message, trace_id, details}` block of a failed refund (spec §15.5.2/15.5.3).
#[derive(Debug, Deserialize)]
pub struct AirwallexFailureDetails {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// `data.object` of a `payment_dispute.*` event (spec §15.5.4). The one real published sample
/// carries only id/amount/currency/stage/status/due_at, so everything else is Option — and the
/// dispute reference must therefore tolerate a missing `payment_intent_id`.
#[derive(Debug, Deserialize)]
pub struct AirwallexDisputeObject {
    pub id: String,
    pub payment_intent_id: Option<String>,
    pub merchant_order_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub stage: Option<AirwallexDisputeStage>,
    pub status: Option<AirwallexDisputeStatus>,
    pub due_at: Option<String>,
    pub reason: Option<AirwallexDisputeReason>,
}

/// Airwallex's dispute stage (spec §15.5.4). `Arbitration` folds into UCS's `PreArbitration` —
/// lossy by necessity: `common_enums::DisputeStage` has no `Arbitration` variant (spec
/// §15.11.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexDisputeStage {
    Rfi,
    PreChargeback,
    Chargeback,
    PreArbitration,
    Arbitration,
    #[serde(other)]
    Unknown,
}

impl From<AirwallexDisputeStage> for common_enums::DisputeStage {
    fn from(stage: AirwallexDisputeStage) -> Self {
        match stage {
            AirwallexDisputeStage::Rfi | AirwallexDisputeStage::PreChargeback => Self::PreDispute,
            AirwallexDisputeStage::Chargeback => Self::Dispute,
            // Lossy fold, pinned in the spec's dispute map — never back-mappable.
            AirwallexDisputeStage::PreArbitration | AirwallexDisputeStage::Arbitration => {
                Self::PreArbitration
            }
            // An unrecognised stage is no dispute at all that UCS can reason about; the
            // `Dispute` neutral is wrong too, but it is the enum's own default arm.
            AirwallexDisputeStage::Unknown => Self::Dispute,
        }
    }
}

/// Airwallex's dispute status (spec §15.5.4). The mixed judgement-call rows
/// (`PendingClosure`/`PendingDecision`/`Reversed`) are pinned by the spec's status map
/// (§15.8), not re-derived here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexDisputeStatus {
    RequiresResponse,
    Challenged,
    Accepted,
    Reversed,
    Won,
    Lost,
    PendingClosure,
    Expired,
    PendingDecision,
    #[serde(other)]
    Unknown,
}

impl From<AirwallexDisputeStatus> for common_enums::DisputeStatus {
    fn from(status: AirwallexDisputeStatus) -> Self {
        match status {
            AirwallexDisputeStatus::RequiresResponse => Self::DisputeOpened,
            AirwallexDisputeStatus::Challenged => Self::DisputeChallenged,
            AirwallexDisputeStatus::Accepted => Self::DisputeAccepted,
            // Judgement call per the spec's §15.8 map: the dispute was withdrawn and the
            // merchant credited.
            AirwallexDisputeStatus::Reversed => Self::DisputeCancelled,
            AirwallexDisputeStatus::Won => Self::DisputeWon,
            AirwallexDisputeStatus::Lost => Self::DisputeLost,
            // Judgement call: Airwallex auto-accepts the pre-arbitration.
            AirwallexDisputeStatus::PendingClosure => Self::DisputeAccepted,
            AirwallexDisputeStatus::Expired => Self::DisputeExpired,
            // Judgement call: evidence is with the scheme.
            AirwallexDisputeStatus::PendingDecision => Self::DisputeChallenged,
            // Unknown is not "open" — but no DisputeStatus variant states the unknown; default
            // to the enum's initial state.
            AirwallexDisputeStatus::Unknown => Self::DisputeOpened,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AirwallexDisputeReason {
    pub original_code: Option<String>,
    pub description: Option<String>,
}

/// The UCS event the envelope's `name` reports. Name-first, per spec §15.7: statuses derive
/// from the event NAME because an attempt's `status` string collapses five terminal failures
/// into one `FAILED` on the wire, and `payment_intent.updated` on purpose derives nothing (the
/// name carries no terminal information). The `payment_consent.*` family and everything not
/// recognised as a payment/refund/dispute lifecycle event reports `IncomingWebhookEventUnspecified`
/// — never an error (the webhook's contract is recognition, not completeness).
pub fn map_webhook_event_type(
    event: &AirwallexWebhookEvent,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;
    match event {
        AirwallexWebhookEvent::PaymentIntentSucceeded => EventType::PaymentIntentSuccess,
        AirwallexWebhookEvent::PaymentIntentCancelled => EventType::PaymentIntentCancelled,
        AirwallexWebhookEvent::PaymentIntentPaymentFailed => EventType::PaymentIntentFailure,
        AirwallexWebhookEvent::PaymentIntentCreated
        | AirwallexWebhookEvent::PaymentIntentRequiresPaymentMethod
        | AirwallexWebhookEvent::PaymentIntentUpdated
        | AirwallexWebhookEvent::PaymentIntentPending
        | AirwallexWebhookEvent::PaymentIntentPendingReview => EventType::PaymentIntentProcessing,
        AirwallexWebhookEvent::PaymentIntentRequiresCustomerAction => {
            EventType::PaymentActionRequired
        }
        AirwallexWebhookEvent::PaymentIntentRequiresCapture => {
            EventType::PaymentIntentAuthorizationSuccess
        }

        AirwallexWebhookEvent::PaymentAttemptSettled
        | AirwallexWebhookEvent::PaymentAttemptPaid => EventType::PaymentIntentSuccess,
        AirwallexWebhookEvent::PaymentAttemptAuthorized => {
            EventType::PaymentIntentAuthorizationSuccess
        }
        AirwallexWebhookEvent::PaymentAttemptCancelled
        | AirwallexWebhookEvent::PaymentAttemptExpired => EventType::PaymentIntentCancelled,
        AirwallexWebhookEvent::PaymentAttemptAuthenticationFailed
        | AirwallexWebhookEvent::PaymentAttemptAuthorizationFailed
        | AirwallexWebhookEvent::PaymentAttemptCaptureFailed
        | AirwallexWebhookEvent::PaymentAttemptRiskDeclined
        | AirwallexWebhookEvent::PaymentAttemptFailedToProcess => EventType::PaymentIntentFailure,
        AirwallexWebhookEvent::PaymentAttemptReceived
        | AirwallexWebhookEvent::PaymentAttemptAuthenticationRedirected
        | AirwallexWebhookEvent::PaymentAttemptPendingAuthorization
        | AirwallexWebhookEvent::PaymentAttemptCaptureRequested => {
            EventType::PaymentIntentProcessing
        }

        AirwallexWebhookEvent::RefundSettled => EventType::RefundSuccess,
        AirwallexWebhookEvent::RefundFailed => EventType::RefundFailure,
        AirwallexWebhookEvent::RefundReceived | AirwallexWebhookEvent::RefundAccepted => {
            EventType::RefundProcessing
        }

        AirwallexWebhookEvent::PaymentDisputeRequiresResponse => EventType::DisputeOpened,
        AirwallexWebhookEvent::PaymentDisputeChallenged
        | AirwallexWebhookEvent::PaymentDisputePendingDecision => EventType::DisputeChallenged,
        AirwallexWebhookEvent::PaymentDisputeAccepted
        | AirwallexWebhookEvent::PaymentDisputePendingClosure => EventType::DisputeAccepted,
        AirwallexWebhookEvent::PaymentDisputeExpired => EventType::DisputeExpired,
        AirwallexWebhookEvent::PaymentDisputeWon => EventType::DisputeWon,
        AirwallexWebhookEvent::PaymentDisputeLost => EventType::DisputeLost,
        AirwallexWebhookEvent::PaymentDisputeReversed => EventType::DisputeCancelled,

        AirwallexWebhookEvent::PaymentConsentCreated
        | AirwallexWebhookEvent::PaymentConsentUpdated
        | AirwallexWebhookEvent::PaymentConsentPending
        | AirwallexWebhookEvent::PaymentConsentVerified
        | AirwallexWebhookEvent::PaymentConsentDisabled
        | AirwallexWebhookEvent::PaymentConsentPaused
        | AirwallexWebhookEvent::PaymentConsentRequiresPaymentMethod
        | AirwallexWebhookEvent::PaymentConsentRequiresCustomerAction
        | AirwallexWebhookEvent::PaymentConsentVerificationFailed
        | AirwallexWebhookEvent::CustomerCreated
        | AirwallexWebhookEvent::CustomerUpdated
        | AirwallexWebhookEvent::PaymentMethodCreated
        | AirwallexWebhookEvent::PaymentMethodUpdated
        | AirwallexWebhookEvent::PaymentMethodAttached
        | AirwallexWebhookEvent::PaymentMethodDetached
        | AirwallexWebhookEvent::PaymentMethodDisabled
        | AirwallexWebhookEvent::PaymentLinkCreated
        | AirwallexWebhookEvent::PaymentLinkPaid
        | AirwallexWebhookEvent::FraudMerchantNotified
        | AirwallexWebhookEvent::FundsSplitCreated
        | AirwallexWebhookEvent::FundsSplitFailed
        | AirwallexWebhookEvent::FundsSplitReleased
        | AirwallexWebhookEvent::FundsSplitSettled
        | AirwallexWebhookEvent::PosTerminalActivated
        | AirwallexWebhookEvent::PosTerminalDeactivated
        | AirwallexWebhookEvent::PosTerminalTerminated
        | AirwallexWebhookEvent::PosTerminalUpdated
        | AirwallexWebhookEvent::PosTerminalAdminPasswordResetRequested
        | AirwallexWebhookEvent::PosTerminalAdminPasswordActivated
        | AirwallexWebhookEvent::PosTerminalAdminPasswordLocked
        | AirwallexWebhookEvent::PosTerminalRefundPasswordResetRequested
        | AirwallexWebhookEvent::PosTerminalRefundPasswordActivated
        | AirwallexWebhookEvent::PosTerminalRefundPasswordLocked
        | AirwallexWebhookEvent::PosTerminalRefundPasswordOptedOut
        | AirwallexWebhookEvent::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// Non-secret fields of a `payment_intent.*` webhook's `data.object` (spec §15.5.1). The full
/// shape reuses the in-tree `AirwallexPaymentsResponse`; this thin struct is what the
/// stateless reference-resolution pass needs without dragging in `next_action` et al.
#[derive(Debug, Deserialize)]
pub struct AirwallexIntentWebhookObject {
    pub id: String,
    pub merchant_order_id: Option<String>,
    pub status: Option<AirwallexPaymentStatus>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub captured_amount: Option<FloatMajorUnit>,
    pub payment_consent_id: Option<Secret<String>>,
    pub latest_payment_attempt: Option<AirwallexPaymentAttempt>,
}

/// Non-secret fields of a `refund.*` webhook's `data.object` (spec §15.5.3). The full shape
/// is `AirwallexRefundResponse`; this carries what the reference pass and the refund response
/// need. `request_id` is the merchant echo this connector wrote as `refund_{reference}` —
/// [`resolve_refund_merchant_id`] recovers it.
#[derive(Debug, Deserialize)]
pub struct AirwallexRefundWebhookObject {
    pub id: String,
    pub request_id: Option<String>,
    pub payment_intent_id: Option<String>,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub status: AirwallexRefundStatus,
    #[serde(default)]
    pub failure_details: Option<AirwallexFailureDetails>,
}

/// Recover the UCS merchant refund id from the `request_id` echo: this connector writes
/// `refund_{connector_request_reference_id}`, so strip exactly that document-fixed prefix; a
/// refund created by another caller has no prefix and passes through unchanged (never an
/// error).
pub(crate) fn resolve_refund_merchant_id(request_id: Option<&str>) -> Option<String> {
    request_id.map(|id| {
        id.strip_prefix("refund_")
            .map_or_else(|| id.to_string(), ToOwned::to_owned)
    })
}

/// Convert a webhook major-unit amount to `[MinorUnit]` via the connector's single converter —
/// never `f64 * 100.0 as i64`. A conversion failure surfaces as
/// `WebhookAmountConversionFailed`, not a silent zero.
pub(crate) fn webhook_minor_amount(
    amount: FloatMajorUnit,
    currency: Currency,
) -> Result<common_utils::MinorUnit, Report<domain_types::errors::WebhookError>> {
    FloatMajorUnitForConnector
        .convert_back(amount, currency)
        .change_context(
            domain_types::errors::WebhookError::WebhookAmountConversionFailed {
                reason: "airwallex: webhook amount did not convert to minor units".to_string(),
            },
        )
}

// --- Webhook process helpers (stateless: each takes the parsed envelope, never re-reads the
// repository) ---

/// Build UCS `WebhookDetailsResponse` (17 fields) from a `payment_intent.*` or
/// `payment_attempt.*` envelope. `event`-first status derivation (spec §15.7); the attempt
/// family's connector transaction id is the INTENT id at `object.payment_intent_id` — never the
/// `att_…` object id (G-incomingwebhook-03@Card), and a missing one is a 400-style webhook
/// error, not a fallback.
pub(crate) fn build_webhook_payment_response(
    envelope: &AirwallexWebhookEnvelope,
    event: AirwallexWebhookEvent,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, Report<domain_types::errors::WebhookError>> {
    let (resource_id, status, error_code, error_message, amount_captured) = match event {
        // Payment-intent family: `data.object` is the full Retrieve-API body; re-parse the
        // full unused shape only for the shared amount/id/status keys (the thin struct).
        e @ (AirwallexWebhookEvent::PaymentIntentCreated
        | AirwallexWebhookEvent::PaymentIntentRequiresPaymentMethod
        | AirwallexWebhookEvent::PaymentIntentUpdated
        | AirwallexWebhookEvent::PaymentIntentRequiresCustomerAction
        | AirwallexWebhookEvent::PaymentIntentRequiresCapture
        | AirwallexWebhookEvent::PaymentIntentPending
        | AirwallexWebhookEvent::PaymentIntentPendingReview
        | AirwallexWebhookEvent::PaymentIntentSucceeded
        | AirwallexWebhookEvent::PaymentIntentCancelled
        | AirwallexWebhookEvent::PaymentIntentPaymentFailed) => {
            let intent: AirwallexIntentWebhookObject = serde_json::from_value(
                envelope.data.object.clone(),
            )
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

            let status = intent_status_from_event(e, &intent)?;

            let amount_captured = optional_minor(intent.captured_amount, intent.currency)?;

            (
                Some(ResponseId::ConnectorTransactionId(intent.id)),
                status,
                None,
                None,
                amount_captured,
            )
        }

        // Payment-attempt family: connector ID comes from `payment_intent_id`, status from the
        // event name with the wire status as a cross-check.
        e @ (AirwallexWebhookEvent::PaymentAttemptReceived
        | AirwallexWebhookEvent::PaymentAttemptAuthenticationRedirected
        | AirwallexWebhookEvent::PaymentAttemptAuthenticationFailed
        | AirwallexWebhookEvent::PaymentAttemptPendingAuthorization
        | AirwallexWebhookEvent::PaymentAttemptAuthorized
        | AirwallexWebhookEvent::PaymentAttemptAuthorizationFailed
        | AirwallexWebhookEvent::PaymentAttemptCaptureRequested
        | AirwallexWebhookEvent::PaymentAttemptCaptureFailed
        | AirwallexWebhookEvent::PaymentAttemptSettled
        | AirwallexWebhookEvent::PaymentAttemptPaid
        | AirwallexWebhookEvent::PaymentAttemptCancelled
        | AirwallexWebhookEvent::PaymentAttemptExpired
        | AirwallexWebhookEvent::PaymentAttemptRiskDeclined
        | AirwallexWebhookEvent::PaymentAttemptFailedToProcess) => {
            let attempt: AirwallexAttemptWebhookObject = serde_json::from_value(
                envelope.data.object.clone(),
            )
            .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

            let intent_id = attempt.payment_intent_id.clone().ok_or(
                domain_types::errors::WebhookError::WebhookMissingRequiredField {
                    field: "payment_intent_id",
                },
            )?;

            let status = attempt_status_from_event(e);

            let (error_code, error_message) = attempt
                .failure_details
                .as_ref()
                .map_or((None, None), |f| (f.code.clone(), f.message.clone()));

            let amount_captured = optional_minor(attempt.captured_amount, attempt.currency)?;

            (
                Some(ResponseId::ConnectorTransactionId(intent_id)),
                status,
                error_code,
                error_message,
                amount_captured,
            )
        }

        _ => {
            return Err(domain_types::errors::WebhookError::WebhookBodyDecodingFailed.into());
        }
    };

    Ok(WebhookDetailsResponse {
        connector_returned_payment_method_details: None,
        resource_id,
        status,
        connector_response_reference_id: Some(envelope.id.clone()),
        connector_request_reference_id: None,
        mandate_reference: None,
        error_code,
        error_message,
        error_reason: None,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured,
        minor_amount_captured: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}

/// Map an intent-family event name to its `AttemptStatus`. The named terminal states are safe;
/// `payment_intent.updated` never derives — the name carries no terminal information, so the
/// status comes from `object.status` through the same match the sync path uses
/// (`get_payment_status`), so webhook and sync can never disagree about a status string.
fn intent_status_from_event(
    event: AirwallexWebhookEvent,
    intent: &AirwallexIntentWebhookObject,
) -> Result<AttemptStatus, Report<domain_types::errors::WebhookError>> {
    match event {
        AirwallexWebhookEvent::PaymentIntentSucceeded => Ok(AttemptStatus::Charged),
        AirwallexWebhookEvent::PaymentIntentCancelled => Ok(AttemptStatus::Voided),
        AirwallexWebhookEvent::PaymentIntentPaymentFailed => Ok(AttemptStatus::Failure),
        AirwallexWebhookEvent::PaymentIntentRequiresCustomerAction => {
            Ok(AttemptStatus::AuthenticationPending)
        }
        AirwallexWebhookEvent::PaymentIntentRequiresCapture => Ok(AttemptStatus::Authorized),
        // Name carries no terminal information: the status string decides, through the exact
        // match the sync path uses. A webhook carries no next_action, so the wire status alone
        // must be enough — the `RequiresCustomerAction`-without-next_action arm lands on
        // `Unresolved` there, which is the deliberate "contract breach" state, never a guess.
        AirwallexWebhookEvent::PaymentIntentUpdated => intent
            .status
            .as_ref()
            .map(|status| get_payment_status(status, &None))
            .ok_or_else(|| {
                domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "status" }
                    .into()
            }),
        // Non-terminal lifecycle names the payment lives in mid-flight.
        AirwallexWebhookEvent::PaymentIntentCreated
        | AirwallexWebhookEvent::PaymentIntentRequiresPaymentMethod
        | AirwallexWebhookEvent::PaymentIntentPending
        | AirwallexWebhookEvent::PaymentIntentPendingReview => Ok(AttemptStatus::Pending),
        _ => Err(domain_types::errors::WebhookError::WebhookBodyDecodingFailed.into()),
    }
}

/// Map an attempt-family event name to `AttemptStatus`. Five terminal failures collapse into
/// one `status: "FAILED"` on the wire, so the event NAME is primary; the wire `status` is a
/// cross-check only. The exit still derives from the event name, so a name→wire disagreement
/// never flips the outcome — it just means the wire status slid ahead/behind, which is noise.
fn attempt_status_from_event(event: AirwallexWebhookEvent) -> AttemptStatus {
    match event {
        AirwallexWebhookEvent::PaymentAttemptSettled
        | AirwallexWebhookEvent::PaymentAttemptPaid => AttemptStatus::Charged,
        AirwallexWebhookEvent::PaymentAttemptAuthorized => AttemptStatus::Authorized,
        AirwallexWebhookEvent::PaymentAttemptCancelled
        | AirwallexWebhookEvent::PaymentAttemptExpired => AttemptStatus::Voided,
        AirwallexWebhookEvent::PaymentAttemptAuthenticationFailed
        | AirwallexWebhookEvent::PaymentAttemptAuthorizationFailed
        | AirwallexWebhookEvent::PaymentAttemptCaptureFailed
        | AirwallexWebhookEvent::PaymentAttemptRiskDeclined
        | AirwallexWebhookEvent::PaymentAttemptFailedToProcess => AttemptStatus::Failure,
        AirwallexWebhookEvent::PaymentAttemptReceived
        | AirwallexWebhookEvent::PaymentAttemptAuthenticationRedirected
        | AirwallexWebhookEvent::PaymentAttemptPendingAuthorization
        | AirwallexWebhookEvent::PaymentAttemptCaptureRequested => AttemptStatus::Pending,
        // Non-attempt events never reach here — the caller arm-matched them out. `Failure` is
        // the one honest default for "named differently"; never a guess.
        _ => AttemptStatus::Failure,
    }
}

/// Convert one optional (`amount`, `currency`) pair to minor units. Both or neither may be
/// absent; when only one arrives the webhook body is malformed (spec §15.5 fixtures always
/// carry both).
fn optional_minor(
    amount: Option<FloatMajorUnit>,
    currency: Option<Currency>,
) -> Result<Option<i64>, Report<domain_types::errors::WebhookError>> {
    match (amount, currency) {
        (Some(major), Some(cur)) => {
            webhook_minor_amount(major, cur).map(|m| Some(m.get_amount_as_i64()))
        }
        (None, None) => Ok(None),
        _ => Err(
            domain_types::errors::WebhookError::WebhookMissingRequiredField {
                field: "amount+currency",
            }
            .into(),
        ),
    }
}

/// Build the UCS refund webhook response. Reuses the existing `[AirwallexRefundResponse]`
/// mapping path (never a second `RefundStatus` table) so RSync and the webhook can never
/// disagree about what `ACCEPTED` means.
pub(crate) fn build_webhook_refund_response(
    envelope: &AirwallexWebhookEnvelope,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, Report<domain_types::errors::WebhookError>> {
    let refund: AirwallexRefundWebhookObject = serde_json::from_value(envelope.data.object.clone())
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

    let status = RefundStatus::from(refund.status);
    let merchant_refund_id = resolve_refund_merchant_id(refund.request_id.as_deref());

    let (error_code, error_message) = refund
        .failure_details
        .as_ref()
        .map_or((None, None), |f| (f.code.clone(), f.message.clone()));

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(refund.id),
        merchant_transaction_id: merchant_refund_id,
        status,
        connector_response_reference_id: Some(envelope.id.clone()),
        error_code,
        error_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Build the UCS dispute webhook response (11 fields). Amount/currency/dispute_id are
/// documented as always present in the §15.5.4 fixture; they are the non-optional fields UCS
/// requires back, so a missing one is a 400-style body error — never a guess. `payment_intent_id`
/// stays optional (the published fixture may omit it), and the mapper records the fold of
/// `Arbitration` → `PreArbitration` on the wire stage (never here).
pub(crate) fn build_webhook_dispute_response(
    envelope: &AirwallexWebhookEnvelope,
    raw_body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, Report<domain_types::errors::WebhookError>> {
    let dispute: AirwallexDisputeObject = serde_json::from_value(envelope.data.object.clone())
        .change_context(domain_types::errors::WebhookError::WebhookBodyDecodingFailed)?;

    let amount = dispute.amount.ok_or(
        domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "amount" },
    )?;
    let currency = dispute.currency.ok_or(
        domain_types::errors::WebhookError::WebhookMissingRequiredField { field: "currency" },
    )?;
    let minor = webhook_minor_amount(amount, currency)?;
    let amount = domain_types::utils::convert_amount_for_webhook(
        &common_utils::types::StringMinorUnitForConnector,
        minor,
        currency,
    )
    .change_context(
        domain_types::errors::WebhookError::WebhookAmountConversionFailed {
            reason: "airwallex: dispute amount did not render as a minor-unit string".to_string(),
        },
    )?;

    Ok(DisputeWebhookDetailsResponse {
        amount,
        currency,
        dispute_id: dispute.id.clone(),
        status: dispute
            .status
            .map(common_enums::DisputeStatus::from)
            .unwrap_or(common_enums::DisputeStatus::DisputeOpened),
        stage: dispute
            .stage
            .map(common_enums::DisputeStage::from)
            .unwrap_or(common_enums::DisputeStage::Dispute),
        connector_response_reference_id: dispute.payment_intent_id.clone(),
        dispute_message: dispute.reason.as_ref().and_then(|r| r.description.clone()),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        connector_reason_code: dispute
            .reason
            .as_ref()
            .and_then(|r| r.original_code.clone()),
    })
}
