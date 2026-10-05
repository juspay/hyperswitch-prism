use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    consts,
    pii::Email,
    request::Method,
    types::{
        FloatMajorUnit, FloatMajorUnitForConnector, StringMajorUnit, StringMinorUnitForConnector,
    },
};
use domain_types::errors::{ConnectorError, IntegrationError, WebhookError};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PSync, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, DisputeWebhookDetailsResponse,
        DisputeWebhookReference, EventType, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentVoidData, PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, ExtendedAuthorizationResponseData, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils::split_full_name,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

pub(crate) const AIRWALLEX_INTEGRATION_DOC_URL: &str = "https://www.airwallex.com/docs/api";

/// Airwallex API version pinned on every Bearer-authenticated call (`x-api-version`). Without it
/// the wire contract is whatever version the merchant account defaults to. Same pin as the
/// Hyperswitch connector.
pub(crate) const AIRWALLEX_API_VERSION: &str = "2026-08-21";

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

/// `{prefix}_{reference}`; a fresh uuid v4 when the reference is empty (merchant_*_id is optional in
/// the proto and domain_types defaults it to ""), so no two calls share an Airwallex request_id (HS parity HP-08/18/22).
fn airwallex_request_id(prefix: &str, reference: &str) -> String {
    if reference.is_empty() {
        format!("{prefix}_{}", common_utils::fp_utils::generate_uuid_v4())
    } else {
        format!("{prefix}_{reference}")
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
    /// Issuer / scheme response code, when the error came from the card network.
    pub provider_original_response_code: Option<String>,
    pub trace_id: Option<String>,
    pub details: Option<serde_json::Value>,
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
    /// Cardholder billing details. Omitted when the request carries no billing field at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<Box<AirwallexCardBilling>>,
    /// Merchant-supplied (external) 3DS result. Only sent together with
    /// `payment_method_options.card.three_ds_action = EXTERNAL_3DS`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_three_ds: Option<Box<AirwallexExternalThreeDs>>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardBilling {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    /// Emitted only when the billing country is known: Airwallex requires
    /// `address.country_code` whenever `address` is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AirwallexCardBillingAddress>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexCardBillingAddress {
    pub country_code: common_enums::CountryAlpha2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postcode: Option<Secret<String>>,
}

/// `payment_method.card.external_three_ds` — the proof of an authentication run by the merchant's
/// own 3DS provider. There is no `cavv` or `xid` slot on the Airwallex request: the CAVV goes in
/// `authentication_value`.
#[derive(Debug, Serialize)]
pub struct AirwallexExternalThreeDs {
    pub authentication_value: Secret<String>,
    pub eci: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds_server_transaction_id: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds_exemption: Option<AirwallexThreeDsExemption>,
}

#[derive(Debug, Serialize)]
pub enum AirwallexThreeDsExemption {
    #[serde(rename = "TRA")]
    TransactionRiskAnalysis,
    #[serde(rename = "LVP")]
    LowValuePayment,
}

/// `payment_method_options.card.three_ds_action`. Only `EXTERNAL_3DS` is ever sent: native 3DS
/// leaves the field out so the merchant account's global 3DS settings apply.
#[derive(Debug, Serialize)]
pub enum AirwallexThreeDsAction {
    #[serde(rename = "EXTERNAL_3DS")]
    ExternalThreeDs,
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
) -> Result<Option<AirwallexDeviceData>, error_stack::Report<IntegrationError>> {
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

/// `payment_method.card.billing`, sourced from the billing address. The `address` object is only
/// emitted when the billing country is known (Airwallex requires `address.country_code` whenever
/// `address` is present), and the whole object is omitted when no billing field is available.
fn get_card_billing(resource_common_data: &PaymentFlowData) -> Option<AirwallexCardBilling> {
    let address = resource_common_data
        .get_optional_billing_country()
        .map(|country_code| AirwallexCardBillingAddress {
            country_code,
            state: resource_common_data.get_optional_billing_state(),
            city: resource_common_data.get_optional_billing_city(),
            street: resource_common_data.get_optional_billing_line1(),
            postcode: resource_common_data.get_optional_billing_zip(),
        });
    let billing = AirwallexCardBilling {
        first_name: resource_common_data.get_optional_billing_first_name(),
        last_name: resource_common_data.get_optional_billing_last_name(),
        email: resource_common_data.get_optional_billing_email(),
        phone_number: resource_common_data.get_optional_billing_phone_number(),
        address,
    };
    let is_empty = billing.first_name.is_none()
        && billing.last_name.is_none()
        && billing.email.is_none()
        && billing.phone_number.is_none()
        && billing.address.is_none();
    (!is_empty).then_some(billing)
}

// Shared Card conversion used by the Authorize (AirwallexPaymentRequest), confirm
// (AirwallexConfirmRequest) and SetupMandate builders so the paths cannot drift.
fn get_card_details<T: PaymentMethodDataTypes>(
    card_data: &domain_types::payment_method_data::Card<T>,
    resource_common_data: &PaymentFlowData,
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
            billing: get_card_billing(resource_common_data).map(Box::new),
            external_three_ds: None,
        },
        payment_method_type: AirwallexPaymentType::Card,
    })
}

/// External 3DS pass-through (`payment_method.card.external_three_ds`). `None` when the request
/// carries no merchant-supplied authentication result. When it does, `cavv`, `eci` and
/// `message_version` are mandatory: Airwallex cannot authorise on a partial proof, and sending
/// `EXTERNAL_3DS` without them would be rejected after the fact rather than before the call.
fn get_external_three_ds(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Result<Option<AirwallexExternalThreeDs>, error_stack::Report<IntegrationError>> {
    let Some(auth) = authentication_data else {
        return Ok(None);
    };
    let missing =
        |field_name: &'static str, airwallex_field: &str| IntegrationError::MissingRequiredField {
            field_name,
            context: aw_err_ctx(
                format!(
                    "Airwallex external 3DS (three_ds_action EXTERNAL_3DS) requires \
                     payment_method.card.external_three_ds.{airwallex_field}, sourced from \
                     {field_name}"
                ),
                "Send the complete external 3DS result (cavv, eci and message_version) in \
                 authentication_data, or omit authentication_data to let Airwallex run 3DS",
            ),
        };
    let authentication_value = auth
        .cavv
        .clone()
        .ok_or_else(|| missing("authentication_data.cavv", "authentication_value"))?;
    let eci = auth
        .eci
        .clone()
        .ok_or_else(|| missing("authentication_data.eci", "eci"))?;
    let version = auth
        .message_version
        .as_ref()
        .map(|version| version.to_string())
        .ok_or_else(|| missing("authentication_data.message_version", "version"))?;
    let three_ds_exemption = match auth.exemption_indicator {
        Some(common_enums::ExemptionIndicator::TransactionRiskAssessment) => {
            Some(AirwallexThreeDsExemption::TransactionRiskAnalysis)
        }
        Some(common_enums::ExemptionIndicator::LowValue) => {
            Some(AirwallexThreeDsExemption::LowValuePayment)
        }
        // No Airwallex equivalent: omit rather than guess (spec §7, UD-04).
        Some(_) | None => None,
    };
    Ok(Some(AirwallexExternalThreeDs {
        authentication_value,
        eci,
        ds_transaction_id: auth.ds_trans_id.clone(),
        three_ds_server_transaction_id: auth.threeds_server_transaction_id.clone(),
        version,
        three_ds_exemption,
    }))
}

/// Refuses a capture method the selected Airwallex payment method cannot honour, before any
/// request is built. Wallets, bank redirects and bank transfers carry no `auto_capture` option,
/// so a manual request would silently auto-capture; cards and pay-later support a single manual
/// capture per intent, never multiple or scheduled captures.
fn validate_capture_method(
    payment_method: &AirwallexPaymentMethod,
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let (refused, arm, supported) = match payment_method {
        AirwallexPaymentMethod::Wallets(_) => (
            matches!(
                capture_method,
                Some(common_enums::CaptureMethod::Manual)
                    | Some(common_enums::CaptureMethod::ManualMultiple)
                    | Some(common_enums::CaptureMethod::Scheduled)
            ),
            "wallet",
            "automatic",
        ),
        AirwallexPaymentMethod::BankRedirect(_) => (
            matches!(
                capture_method,
                Some(common_enums::CaptureMethod::Manual)
                    | Some(common_enums::CaptureMethod::ManualMultiple)
                    | Some(common_enums::CaptureMethod::Scheduled)
            ),
            "bank redirect",
            "automatic",
        ),
        AirwallexPaymentMethod::BankTransfer(_) => (
            matches!(
                capture_method,
                Some(common_enums::CaptureMethod::Manual)
                    | Some(common_enums::CaptureMethod::ManualMultiple)
                    | Some(common_enums::CaptureMethod::Scheduled)
            ),
            "bank transfer",
            "automatic",
        ),
        AirwallexPaymentMethod::Card(_) => (
            matches!(
                capture_method,
                Some(common_enums::CaptureMethod::ManualMultiple)
                    | Some(common_enums::CaptureMethod::Scheduled)
            ),
            "card",
            "automatic or manual (single capture)",
        ),
        AirwallexPaymentMethod::PayLater(_) => (
            matches!(
                capture_method,
                Some(common_enums::CaptureMethod::ManualMultiple)
                    | Some(common_enums::CaptureMethod::Scheduled)
            ),
            "pay later",
            "automatic or manual (single capture)",
        ),
    };
    if refused {
        return Err(error_stack::report!(
            IntegrationError::CaptureMethodNotSupported {
                context: aw_err_ctx(
                    format!(
                        "Airwallex {arm} payments do not support capture_method {capture_method:?}; \
                         Airwallex allows one capture per payment intent"
                    ),
                    format!("Send capture_method {supported} for this payment method"),
                ),
            }
        ));
    }
    Ok(())
}

// Shared BankRedirect conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. iDeal only carries
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

// Shared wallet conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift.
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

// Shared PayLater conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. Mirrors the
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

// Shared BankTransfer conversion used by both the intent (AirwallexPaymentRequest) and
// confirm (AirwallexConfirmRequest) builders so the two paths cannot drift. Mirrors the
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

/// Whether this Authorize call is the re-entry after the shopper's browser returned from an
/// Airwallex redirect (3DS or APM), i.e. the caller supplied a `redirect_response`.
///
/// On the pinned API version there is no continuation call: Airwallex finishes the payment inside
/// its hosted page, so the return leg only **reads** the settled intent with
/// `GET /pa/payment_intents/{id}` — it never confirms a second time, so it can never charge twice.
/// Used by `get_url` / `build_request_v2` in `airwallex.rs` and by the Authorize response
/// transformer, so the URL, the method and the status handling cannot disagree.
pub(crate) fn is_three_ds_return_leg<T: PaymentMethodDataTypes>(
    request: &PaymentsAuthorizeData<T>,
) -> bool {
    request.redirect_response.is_some()
}

// Single entry point for turning the domain payment method into the Airwallex payload. Both the
// intent (AirwallexPaymentRequest) and the confirm (AirwallexConfirmRequest) builders call it, so
// the two request paths cannot drift.
fn get_payment_method_details<T: PaymentMethodDataTypes>(
    payment_method_data: &domain_types::payment_method_data::PaymentMethodData<T>,
    resource_common_data: &PaymentFlowData,
    customer_name: Option<Secret<String>>,
) -> Result<AirwallexPaymentMethod, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
            Ok(get_card_details(card_data, resource_common_data))
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
        // authorization_type only ever reaches Airwallex through this arm.
        AirwallexPaymentMethod::Card(_) => Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(auto_capture),
                authorization_type,
                three_ds_action,
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
        // G-ThreeDS-05 (every payment-method arm): a customer-initiated mandate setup needs the
        // Airwallex customer (`cus_...`) the PaymentConsent is attached to. Checked first, before
        // any payment-method specific work, so every arm refuses the same way.
        let is_cit = item
            .router_data
            .request
            .is_customer_initiated_mandate_payment();
        let cit_customer_id = if is_cit {
            Some(
                item.router_data
                    .resource_common_data
                    .connector_customer
                    .clone()
                    .ok_or_else(|| IntegrationError::MissingRequiredField {
                        field_name: "connector_customer_id",
                        context: aw_err_ctx(
                            "Airwallex attaches the PaymentConsent of a customer-initiated \
                             mandate setup to an Airwallex customer (cus_...), created by \
                             CustomerService/Create",
                            "Run CustomerService/Create first and send its \
                             connector_customer_id on the payment request",
                        ),
                    })?,
            )
        } else {
            None
        };

        let mut payment_method = get_payment_method_details(
            &item.router_data.request.payment_method_data,
            &item.router_data.resource_common_data,
            item.router_data
                .request
                .customer_name
                .clone()
                .map(Secret::new),
        )?;

        // G-ThreeDS-03 / G-ThreeDS-04: refuse a capture method the arm cannot honour.
        validate_capture_method(&payment_method, item.router_data.request.capture_method)?;

        // External 3DS pass-through (card only). G-ThreeDS-02 lives in get_external_three_ds.
        let three_ds_action = match &mut payment_method {
            AirwallexPaymentMethod::Card(card) => {
                let external_three_ds =
                    get_external_three_ds(item.router_data.request.authentication_data.as_ref())?;
                let action = external_three_ds
                    .as_ref()
                    .map(|_| AirwallexThreeDsAction::ExternalThreeDs);
                card.card.external_three_ds = external_three_ds.map(Box::new);
                action
            }
            AirwallexPaymentMethod::Wallets(_)
            | AirwallexPaymentMethod::BankRedirect(_)
            | AirwallexPaymentMethod::PayLater(_)
            | AirwallexPaymentMethod::BankTransfer(_) => None,
        };

        // Manual on Card/PayLater -> auto_capture false; everything the guard above lets through
        // on the other arms is automatic.
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

        let payment_method_options = build_payment_method_options(
            &payment_method,
            auto_capture,
            authorization_type,
            three_ds_action,
        );

        // Generate unique request_id for Authorize/confirm step
        // Different from CreateOrder to avoid Airwallex duplicate_request error
        let request_id = airwallex_request_id(
            "confirm",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        // Mirror native HS airwallex for a CIT (setup_future_usage) mandate setup: attach a
        // PaymentConsent so Airwallex returns a payment_consent_id we store as the connector
        // mandate for future MITs, send the connector customer_id, and OMIT device_data. Native
        // only collects device data for non-mandate payments — sending it alongside a consent
        // pushes Airwallex into a device-data-collection SCA path it can't complete here. Same
        // CIT detection helper (is_customer_initiated_mandate_payment) as native.
        let (payment_consent, customer_id, device_data) = match cit_customer_id {
            Some(customer_id) => (
                Some(AirwallexPaymentConsentData {
                    next_triggered_by: AirwallexTriggeredBy::Merchant,
                    merchant_trigger_reason: AirwallexMerchantTriggeredReason::Unscheduled,
                }),
                Some(customer_id),
                None,
            ),
            None => (None, None, get_device_data(&item.router_data.request)?),
        };

        // Every payment method returns to router_return_url (HS parity). On the router path the
        // browser return is settled by PSync; a caller that re-enters Authorize with a
        // redirect_response gets the read-only GET leg (see is_three_ds_return_leg).
        let return_url = item.router_data.request.get_router_return_url().ok();

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
    /// Merchant order reference echoed back (the UCS `connector_request_reference_id` set on
    /// CreateOrder). Read by the webhook path as the merchant transaction id.
    pub merchant_order_id: Option<String>,
}

// Type alias - reuse the same response structure for PSync
pub type AirwallexSyncResponse = AirwallexPaymentsResponse;

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexPaymentAttempt {
    pub id: Option<String>,
    pub status: Option<AirwallexAttemptStatus>,
    pub amount: Option<FloatMajorUnit>,
    pub captured_amount: Option<FloatMajorUnit>,
    /// Attempt currency; needed to convert `captured_amount` to minor units on the webhook path.
    pub currency: Option<Currency>,
    pub payment_intent_id: Option<String>,
    pub merchant_order_id: Option<String>,
    pub payment_consent_id: Option<Secret<String>>,
    pub payment_method: Option<AirwallexPaymentMethodInfo>,
    pub authorization_code: Option<String>,
    /// Scheme transaction id (the network transaction id used for NTID-based MITs).
    pub payment_method_transaction_id: Option<String>,
    pub provider_original_response_code: Option<String>,
    pub provider_original_response_description: Option<String>,
    pub merchant_advice_code: Option<String>,
    pub failure_code: Option<String>,
    pub failure_details: Option<AirwallexFailureDetails>,
    pub authentication_data: Option<AirwallexAuthenticationData>,
    pub processor_response: Option<AirwallexProcessorResponse>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// `latest_payment_attempt.status`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexAttemptStatus {
    Received,
    Created,
    AuthenticationRedirected,
    PendingAuthorization,
    Authorized,
    CaptureRequested,
    Captured,
    Declined,
    Expired,
    Cancelled,
    Failed,
    Settled,
    Paid,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AirwallexFailureDetails {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// `latest_payment_attempt.authentication_data`. Every child is optional: `ds_data` may be `{}`.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexAuthenticationData {
    pub avs_result: Option<String>,
    pub cvc_result: Option<String>,
    pub ds_data: Option<AirwallexDsData>,
    pub fraud_data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDsData {
    pub version: Option<String>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub xid: Option<Secret<String>>,
    pub liability_shift_indicator: Option<String>,
    pub frictionless: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexPaymentStatus {
    RequiresPaymentMethod,
    RequiresCustomerAction,
    RequiresConfirmation,
    RequiresAction,
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
    PendingReview,
    /// Any status this connector does not know yet; mapped to `Unresolved`, never guessed.
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
    #[serde(alias = "expiry_month")]
    pub exp_month: Option<Secret<String>>,
    #[serde(alias = "expiry_year")]
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

/// Human-readable meaning of an issuer / scheme response code
/// (`provider_original_response_code`). Ported from the Hyperswitch Airwallex connector so both
/// stacks surface the same `network_error_message` for the same decline.
pub(crate) fn map_issuer_code_to_message(code: &str) -> Option<String> {
    let message = match code {
        "01" => "Contact card issuer",
        "03" => "Invalid Merchant",
        "04" => "Pick up card(no fraud)",
        "05" => "Do not honor",
        "06" => "Error",
        "07" => "Pick up card, special condition (fraud account)",
        "12" => "Invalid transaction",
        "13" => "Invalid amount",
        "14" => "Invalid card number",
        "15" => "Invalid issuer",
        "19" => "Re-enter transaction",
        "21" => "No action taken",
        "22" => "Operation error",
        "30" => "Format error",
        "34" => "Fraudulent card",
        "40" => "Transaction that is not supported by the Issuer",
        "41" => "Lost card",
        "43" => "Stolen card",
        "46" => "Closed account",
        "51" => "Insufficient funds/over credit limit / Not sufficient funds",
        "52" => "No checking account",
        "53" => "No savings account",
        "54" => "Expired card",
        "55" => "Incorrect PIN",
        "57" => "Transaction not permitted to issuer/cardholder",
        "58" => "Transaction not permitted to acquirer/terminal",
        "59" => "Suspected fraud",
        "61" => "Exceeds withdrawal limit",
        "62" => "Restricted card",
        "63" => "Security violation",
        "64" => "AML requirement failure / Original transaction amount mismatch",
        "65" => "Exceeds withdrawal count limit / Additional customer authentication required",
        "6P" => "Customer ID verification failed",
        "70" => "Contact Card Issuer",
        "72" => "Account not yet activated",
        "78" => "Invalid/nonexistent account specified (general)",
        "79" => "Life Cycle",
        "80" => "Credit issuer unavailable",
        "82" => "Policy / Negative online CAM, dCVV, iCVV, CVV, or CAVV results or Offline PIN authentication interrupted",
        "83" => "Fraud / Security violation",
        "85" => "No reason to decline",
        "90" => "Decline due to daily cutoff being in progress",
        "91" => "Authorization Platform or issuer system inoperative / Issuer not available OR Issuer unavailable or switch inoperative",
        "92" => "Destination cannot be found for routing / Unable to route transaction",
        "93" => "Transaction cannot be completed; violation of law",
        "96" => "System malfunction",
        "1A" => "Authentication Required",
        "R0" => "Stop payment order",
        "R1" => "Revocation of authorisation order",
        "R3" => "Revocation of all authorisation orders",
        "N7" => "Decline for CVV2 failure",
        "5C" => "Transaction not supported / blocked by issuer",
        "9G" => "Blocked by cardholder / contact cardholder",
        "100" => "Deny / Do Not Honor",
        "101" => "Expired Card / Invalid Expiration Date",
        "109" => "Invalid merchant",
        "110" => "Invalid amount",
        "111" => "Invalid account / Invalid MICR (Travelers Cheque) / Invalid Card Number",
        "115" => "Requested function not supported",
        "116" => "Not sufficient funds",
        "119" => "Cardmember not enrolled / not permitted",
        "121" => "Limit exceeded",
        "122" => "Invalid card security code (a.k.a., CID, 4DBC, 4CSC) / Card Validity Period Exceeded",
        "130" => "Additional customer identification required",
        "181" => "Format error",
        "183" => "Invalid currency code",
        "187" => "Deny - new card issued",
        "189" => "Deny - Canceled or Closed Merchant/SE",
        "190" => "National ID mismatch",
        "200" => "Deny - Pick up card / Do Not Honor",
        "909" => "System Malfunction (Cryptographic error)",
        "912" => "Issuer not available",
        "978" => "Invalid Payment Times",
        "800.100.100" => "Transaction declined for unknown reason",
        "800.100.150" => "Transaction declined (refund on gambling tx not allowed)",
        "800.100.151" => "Transaction declined (invalid card)",
        "800.100.152" => "Transaction declined by authorization system",
        "800.100.153" => "Transaction declined (invalid CVV)",
        "800.100.154" => "Transaction declined (transaction marked as invalid)",
        "800.100.155" => "Transaction declined (amount exceeds credit)",
        "800.100.156" => "Transaction declined (format error)",
        "800.100.157" => "Transaction declined (wrong expiry date)",
        "800.100.158" => "Transaction declined (suspecting manipulation)",
        "800.100.159" => "Transaction declined (stolen card)",
        "800.100.160" => "Transaction declined (card blocked)",
        "800.100.161" => "Transaction declined (too many invalid tries)",
        "800.100.162" => "Transaction declined (limit exceeded)",
        "800.100.163" => "Transaction declined (maximum transaction frequency exceeded)",
        "800.100.164" => "Transaction declined (merchants limit exceeded)",
        "800.100.165" => "Transaction declined (card lost)",
        "800.100.168" => "Transaction declined (restricted card)",
        "800.100.169" => "Transaction declined (card type is not processed by the authorization center)",
        "800.100.170" => "Transaction declined (transaction not permitted)",
        "800.100.171" => "Transaction declined (pick up card)",
        "800.100.172" => "Transaction declined (account blocked)",
        "800.100.173" => "Transaction declined (invalid currency, not processed by authorization center)",
        "800.100.174" => "Insufficient Funds",
        "800.100.176" => "Transaction declined (account temporarily not available. Please try again later)",
        "800.100.179" => "Transaction declined (exceeds withdrawal count limit)",
        "800.100.190" => "Transaction declined (invalid configuration data)",
        "800.100.192" => "Transaction declined (invalid CVV, Amount has still been reserved on the customer's card and will be released in a few business days.)",
        "800.100.195" => "Transaction declined (UserAccount Number/ID unknown)",
        "800.100.200" => "Refer to Payer due to reason not specified",
        "800.100.201" => "Account or Bank Details Incorrect",
        "800.100.202" => "Account Closed",
        "800.100.203" => "Insufficient Funds",
        "800.100.204" => "Mandate Expired",
        "800.100.205" => "Mandate Discarded",
        "800.100.402" => "CC/bank account holder not valid",
        "800.100.403" => "Transaction declined (revocation of authorisation order)",
        "800.100.500" => "The card holder has advised his bank to stop this recurring payment",
        "800.100.501" => "Card holder has advised his bank to stop all recurring payments for this merchant",
        "081" => "Approved by Issuer",
        "102" => "Suspected Fraud",
        "103" => "Customer Authentication Required",
        "104" => "Restricted Card",
        "106" => "Allowable PIN Tries Exceeded",
        "117" => "Incorrect PIN",
        "118" => "Cycle Range Suspended",
        "120" => "Transaction Not Permitted To Originator",
        "124" => "Violation Of Law",
        "125" => "Card Not Effective",
        "129" => "Suspected Counterfeit Card",
        "163" => "Security Violations",
        "182" => "Decline Given By Issuer",
        "192" => "Restricted Merchant",
        "197" => "Card Account Verification Failed",
        "198" => "TVR or CVR Validation Failed",
        "201" => "Expired Card",
        "202" => "Suspected Fraud",
        "204" => "Restricted Card",
        "206" => "Allowable Pin Tries Exceeded",
        "207" => "Special Conditions",
        "208" => "Lost Card",
        "209" => "Stolen Card",
        "210" => "Suspected Counterfeit Card",
        _ => return None,
    };
    Some(message.to_string())
}

/// Network transaction id of the payment: `latest_payment_attempt.payment_method_transaction_id`
/// only. Never the authorization code, which is an issuer approval code and not a scheme id.
pub(crate) fn get_network_txn_id(response: &AirwallexPaymentsResponse) -> Option<String> {
    response
        .latest_payment_attempt
        .as_ref()
        .and_then(|attempt| attempt.payment_method_transaction_id.clone())
}

/// `ErrorResponse` for a 2xx PaymentIntent whose status maps to a failure. Airwallex reports the
/// decline on `latest_payment_attempt` (`failure_details`, the issuer's original response code and
/// the merchant advice code), not in an error body.
pub(crate) fn build_attempt_error_response(
    response: &AirwallexPaymentsResponse,
    http_code: u16,
    attempt_status: FlowStatus,
) -> ErrorResponse {
    let attempt = response.latest_payment_attempt.as_ref();
    let failure_details = attempt.and_then(|attempt| attempt.failure_details.as_ref());
    let code = failure_details
        .and_then(|details| details.code.clone())
        .or_else(|| attempt.and_then(|attempt| attempt.failure_code.clone()))
        .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
    let message = failure_details
        .and_then(|details| details.message.clone())
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
    let provider_code = attempt.and_then(|attempt| attempt.provider_original_response_code.clone());
    let provider_description =
        attempt.and_then(|attempt| attempt.provider_original_response_description.clone());
    let network_error_message = provider_description.clone().or_else(|| {
        provider_code
            .as_deref()
            .and_then(map_issuer_code_to_message)
    });
    ErrorResponse {
        status_code: http_code,
        code,
        message,
        reason: provider_description,
        attempt_status: Some(attempt_status),
        connector_transaction_id: Some(response.id.clone()),
        network_decline_code: provider_code,
        network_advice_code: attempt.and_then(|attempt| attempt.merchant_advice_code.clone()),
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Card result details (AVS / CVC checks, the non-secret 3DS outcome, the card brand and the
/// issuer approval code) for `ConnectorResponseData`. `cavv` / `xid` are never surfaced.
pub(crate) fn build_card_connector_response(
    response: &AirwallexPaymentsResponse,
) -> Option<AdditionalPaymentMethodConnectorResponse> {
    let attempt = response.latest_payment_attempt.as_ref();
    let authentication_data = attempt.and_then(|attempt| attempt.authentication_data.as_ref());
    let card = attempt
        .and_then(|attempt| attempt.payment_method.as_ref())
        .or(response.payment_method.as_ref())
        .and_then(|payment_method| payment_method.card.as_ref());
    // Only card intents carry any of these; anything else has no card connector response.
    if authentication_data.is_none() && card.is_none() {
        return None;
    }
    let avs_result = authentication_data.and_then(|data| data.avs_result.clone());
    let cvc_result = authentication_data.and_then(|data| data.cvc_result.clone());
    let payment_checks = (avs_result.is_some() || cvc_result.is_some()).then(|| {
        serde_json::json!({
            "avs_result": avs_result,
            "cvc_result": cvc_result,
        })
    });
    let three_ds = authentication_data
        .and_then(|data| data.ds_data.as_ref())
        .map(|ds_data| {
            serde_json::json!({
                "eci": ds_data.eci,
                "version": ds_data.version,
                "liability_shift_indicator": ds_data.liability_shift_indicator,
                "frictionless": ds_data.frictionless,
            })
        });
    let auth_code = attempt
        .and_then(|attempt| attempt.authorization_code.clone())
        .or_else(|| response.authorization_code.clone());
    Some(AdditionalPaymentMethodConnectorResponse::Card {
        authentication_data: three_ds,
        payment_checks,
        card_network: card.and_then(|card| card.brand.clone()),
        domestic_network: None,
        auth_code,
    })
}

/// Maps the PaymentIntent status. Exhaustive on purpose (no `_ =>`): a new upstream status
/// deserialises as `Unknown` and maps to `Unresolved` instead of being guessed.
fn get_payment_status(
    status: &AirwallexPaymentStatus,
    next_action: &Option<AirwallexNextAction>,
) -> AttemptStatus {
    match status {
        AirwallexPaymentStatus::Succeeded
        | AirwallexPaymentStatus::Paid
        | AirwallexPaymentStatus::CaptureRequested
        | AirwallexPaymentStatus::Settled => AttemptStatus::Charged,
        AirwallexPaymentStatus::RequiresCapture | AirwallexPaymentStatus::Authorized => {
            AttemptStatus::Authorized
        }
        AirwallexPaymentStatus::Cancelled => AttemptStatus::Voided,
        // After a confirm, REQUIRES_PAYMENT_METHOD means the attempt was declined and the intent
        // reopened for a new payment method (HS parity).
        AirwallexPaymentStatus::Failed | AirwallexPaymentStatus::RequiresPaymentMethod => {
            AttemptStatus::Failure
        }
        AirwallexPaymentStatus::Processing
        | AirwallexPaymentStatus::Pending
        | AirwallexPaymentStatus::PendingReview => AttemptStatus::Pending,
        AirwallexPaymentStatus::RequiresConfirmation => AttemptStatus::ConfirmationAwaited,
        AirwallexPaymentStatus::RequiresAction => AttemptStatus::AuthenticationPending,
        AirwallexPaymentStatus::RequiresCustomerAction => {
            next_action
                .as_ref()
                .map_or(
                    AttemptStatus::AuthenticationPending,
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
        AirwallexPaymentStatus::Unknown => AttemptStatus::Unresolved,
    }
}

// Extended-authorization result for the authorize response: applied only when it
// was requested AND the payment method is card (mirrors hyperswitch airwallex)
fn build_airwallex_extended_authorization_data(
    extended_authorization_requested: bool,
    payment_method: common_enums::PaymentMethod,
) -> ExtendedAuthorizationResponseData {
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
    ExtendedAuthorizationResponseData {
        extended_authentication_applied,
        extended_authorization_last_applied_at: None,
        capture_before: None,
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<AirwallexPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let return_leg = is_three_ds_return_leg(&item.router_data.request);
        let mapped_status = get_payment_status(&item.response.status, &item.response.next_action);

        // A 2xx intent can still be a declined attempt: FAILED, or REQUIRES_PAYMENT_METHOD after
        // the confirm. Surface it as an ErrorResponse built from latest_payment_attempt.
        if mapped_status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::Failure),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Loop guard (HS parity): the shopper already came back from the Airwallex page, yet the
        // intent still asks for customer action. Sending them back again would loop forever.
        if return_leg
            && matches!(
                item.response.status,
                AirwallexPaymentStatus::RequiresCustomerAction
            )
        {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::AuthenticationFailed),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::AuthenticationFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // The redirect is only offered on the initial confirm; the return leg reads a settled
        // intent and must never send the shopper back to Airwallex.
        let redirection_data = if return_leg {
            None
        } else {
            // Handles APM redirects (type "redirect") AND card 3DS (`redirect_iframe` /
            // "device_data_collection"), which the old type-gated version dropped.
            build_redirection_data(&item.response.next_action)
        };

        let network_txn_id = get_network_txn_id(&item.response);

        // AVS/CVC/auth-code/3DS outcome and the extended-authorization result travel together.
        let extended_authorization =
            item.router_data
                .request
                .request_extended_authorization
                .map(|requested| {
                    build_airwallex_extended_authorization_data(
                        requested,
                        item.router_data.resource_common_data.payment_method,
                    )
                });
        let card_connector_response = build_card_connector_response(&item.response);
        let connector_response = (card_connector_response.is_some()
            || extended_authorization.is_some())
        .then(|| ConnectorResponseData::new(card_connector_response, None, extended_authorization));

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

        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id),
                incremental_authorization_allowed: Some(false), // Airwallex doesn't support incremental auth
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status: mapped_status,
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
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // A 2xx intent can still carry a declined attempt (FAILED, or REQUIRES_PAYMENT_METHOD
        // after a confirm): surface it as an ErrorResponse built from latest_payment_attempt.
        if status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::Failure),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // An intent still waiting on the shopper returns its redirect on sync too (HS parity), so
        // a caller that syncs before redirecting can still send the shopper to Airwallex.
        let redirection_data = if matches!(
            status,
            AttemptStatus::AuthenticationPending | AttemptStatus::DeviceDataCollectionPending
        ) {
            build_redirection_data(&item.response.next_action)
        } else {
            None
        };

        let network_txn_id = get_network_txn_id(&item.response);
        let connector_response = build_card_connector_response(&item.response)
            .map(|card_response| ConnectorResponseData::new(Some(card_response), None, None));

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
                redirection_data,
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
                connector_response,
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
                context: aw_err_ctx(
                    "Failed to convert amount_to_capture into the Airwallex major-unit decimal \
                     for the given currency on /pa/payment_intents/{id}/capture",
                    "Ensure amount_to_capture is a valid minor-unit amount for the payment \
                     currency",
                ),
            })?;

        // Idempotency key: capture_{merchant_capture_id}, or a fresh uuid when the caller sent none.
        let request_id = airwallex_request_id(
            "capture",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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
        let status = get_payment_status(&item.response.status, &item.response.next_action);

        // A 2xx capture can still come back FAILED (or REQUIRES_PAYMENT_METHOD): surface it as an
        // ErrorResponse built from latest_payment_attempt, carrying the capture's own status.
        if status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::CaptureFailed),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Scheme transaction id only; the issuer authorization_code is not a network txn id.
        let network_txn_id = get_network_txn_id(&item.response);
        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data: None, // Capture doesn't involve redirections
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id),
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
    /// Spelling Hyperswitch's Airwallex enum carries alongside `SETTLED`.
    Succeeded,
    Failed,
    #[serde(other)]
    Unknown,
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
            .change_context(IntegrationError::RequestEncodingFailed {
                context: aw_err_ctx(
                    "Airwallex /pa/refunds/create takes the refund amount in major units; \
                     refund_amount could not be converted for the request currency",
                    "Send a refund_amount in minor units that is valid for the refund currency",
                ),
            })?;

        // Idempotency key: refund_{merchant_refund_id}, or a fresh uuid when the caller sent none.
        let request_id = airwallex_request_id(
            "refund",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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
        let (status, response) = build_refund_response(item.response, item.http_code);

        Ok(Self {
            response,
            resource_common_data: RefundFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

/// Shared by the Refund and RSync response transformers: both endpoints return the same refund
/// object, so they must map it identically. A `FAILED` refund arrives on HTTP 2xx and is turned
/// into an `ErrorResponse` carrying the refund failure status; every other status is a
/// `RefundsResponseData` with the mapped status.
pub(crate) fn build_refund_response(
    response: AirwallexRefundResponse,
    http_code: u16,
) -> (RefundStatus, Result<RefundsResponseData, ErrorResponse>) {
    let status = RefundStatus::from(response.status);
    let result = if status == RefundStatus::Failure {
        let failure_details = response.failure_details.as_ref();
        Err(ErrorResponse {
            status_code: http_code,
            code: failure_details
                .and_then(|details| details.code.clone())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: failure_details
                .and_then(|details| details.message.clone())
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
            reason: failure_details.and_then(|details| details.message.clone()),
            attempt_status: Some(FlowStatus::Refund(RefundStatus::Failure)),
            connector_transaction_id: Some(response.id),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        Ok(RefundsResponseData {
            connector_refund_id: response.id,
            refund_status: status,
            status_code: http_code,
            acquirer_reference_number: response.acquirer_reference_number,
        })
    };
    (status, result)
}

// ===== REFUND SYNC FLOW TYPES =====

// Reuse the same response structure as AirwallexRefundResponse since it's the same endpoint (GET /pa/refunds/{id})
pub type AirwallexRefundSyncResponse = AirwallexRefundResponse;

// Response transformer for RSync flow: same refund object as Refund, so the same builder.
impl TryFrom<ResponseRouterData<AirwallexRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<AirwallexRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let (status, response) = build_refund_response(item.response, item.http_code);

        Ok(Self {
            response,
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
            AirwallexRefundStatus::Settled | AirwallexRefundStatus::Succeeded => Self::Success,
            AirwallexRefundStatus::Failed => Self::Failure,
            AirwallexRefundStatus::Received
            | AirwallexRefundStatus::Accepted
            | AirwallexRefundStatus::Unknown => Self::Pending,
        }
    }
}

// ===== VOID FLOW TYPES =====

#[derive(Debug, Serialize)]
pub struct AirwallexVoidRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>, // Reason for cancellation, sent only when provided
    pub request_id: String, // Unique identifier for idempotency
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
        // Send the caller's cancellation reason as-is; omitted from the body when absent.
        let cancellation_reason = item.router_data.request.cancellation_reason.clone();

        // Idempotency key: void_{merchant_void_id}, or a fresh uuid when the caller sent none.
        let request_id = airwallex_request_id(
            "void",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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

        // A 2xx cancel can still come back FAILED (or REQUIRES_PAYMENT_METHOD): surface it as an
        // ErrorResponse built from latest_payment_attempt, carrying the void's own status.
        if status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::VoidFailed),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        // Scheme transaction id only; the issuer authorization_code is not a network txn id.
        let network_txn_id = get_network_txn_id(&item.response);
        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data: None, // Void doesn't involve redirections
                mandate_reference: None,
                connector_metadata: None, // Following hyperswitch pattern - no connector_metadata for void
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id),
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
        let request_id = airwallex_request_id(
            "create",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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
            AirwallexPaymentStatus::RequiresConfirmation => AttemptStatus::ConfirmationAwaited,
            AirwallexPaymentStatus::RequiresAction => AttemptStatus::AuthenticationPending,
            AirwallexPaymentStatus::PendingReview => AttemptStatus::Pending,
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
        let payment_method = match &item.router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                get_card_details(card_data, &item.router_data.resource_common_data)
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "SetupMandate Payment Method (only Card supported)".to_string(),
                    connector: "Airwallex",
                    context: aw_err_ctx(
                        "Airwallex sets up a mandate (payment_consent) only for card payment \
                         methods",
                        "Send a card payment method on SetupRecurring, or set up the mandate \
                         through Authorize with setup_future_usage for other payment methods",
                    ),
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
                context: aw_err_ctx(
                    "Airwallex attaches the payment_consent to a connector customer, so \
                     customer_id (cus_...) is required on the SetupMandate confirm",
                    "Run PaymentService/CreateConnectorCustomer first and send its \
                     connector_customer_id on the SetupRecurring request",
                ),
            })?;

        let request_id = airwallex_request_id(
            "confirm",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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

        // A 2xx intent can still be a declined CIT: FAILED, or REQUIRES_PAYMENT_METHOD after the
        // confirm. Surface it as an ErrorResponse built from latest_payment_attempt rather than a
        // success carrying a Failure status.
        if status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::Failure),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let redirection_data = build_redirection_data(&item.response.next_action);

        // The CIT's scheme transaction id, so a later MIT can reference it.
        let network_txn_id = get_network_txn_id(&item.response);

        // AVS/CVC/auth-code/3DS outcome of the card verification.
        let connector_response = build_card_connector_response(&item.response)
            .map(|card_response| ConnectorResponseData::new(Some(card_response), None, None));

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

        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data,
                mandate_reference,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id),
                incremental_authorization_allowed: Some(false),
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

// ===== REPEAT PAYMENT (MIT) FLOW TYPES =====
//
// Airwallex MIT: POST /pa/payment_intents/{new_intent_id}/confirm on a fresh PaymentIntent
// (CreateOrder); the CIT consent-setup intent is already consumed. Two request shapes:
// - consent MIT (MandateReferenceId::ConnectorMandateId): `payment_consent_id` (cst_...) plus
//   the stored `payment_method.id` (pm_...), `triggered_by: merchant`;
// - network-transaction-id MIT (MandateReferenceId::NetworkMandateId): raw card (no cvc) plus
//   `external_recurring_data.original_transaction_id`, and never a `payment_consent_id`
//   (Airwallex ignores external_recurring_data whenever a consent id is present).

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
#[serde(untagged)]
pub enum AirwallexRepeatPaymentRequest {
    ConsentMit(AirwallexConsentMitRequest),
    NetworkTransactionIdMit(AirwallexNetworkTransactionIdMitRequest),
}

#[derive(Debug, Serialize)]
pub struct AirwallexConsentMitRequest {
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
    // Airwallex auto-captures a card confirm unless told otherwise; carries the caller's
    // capture_method (TH-15). Only `card.auto_capture` is ever set here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
}

#[derive(Debug, Serialize)]
pub struct AirwallexNetworkTransactionIdMitRequest {
    pub request_id: String,
    pub payment_method: AirwallexNtidPaymentMethod,
    pub external_recurring_data: AirwallexExternalRecurringData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_options: Option<AirwallexPaymentOptions>,
}

/// `payment_method` of an NTID MIT: the raw card without a CVC (merchant-initiated, the
/// cardholder is not present).
#[derive(Debug, Serialize)]
pub struct AirwallexNtidPaymentMethod {
    #[serde(rename = "type")]
    pub payment_method_type: AirwallexPaymentType,
    pub card: AirwallexNtidCardDetails,
}

#[derive(Debug, Serialize)]
pub struct AirwallexNtidCardDetails {
    pub number: cards::CardNumber,
    pub expiry_month: Secret<String>,
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Secret<String>>,
}

/// `external_recurring_data` — marks the confirm as an external recurring payment that
/// references the scheme transaction id (`payment_method_transaction_id`) of the initial payment.
#[derive(Debug, Serialize)]
pub struct AirwallexExternalRecurringData {
    pub triggered_by: AirwallexTriggeredBy,
    pub initial_payment: bool,
    pub merchant_trigger_reason: AirwallexMerchantTriggeredReason,
    pub original_transaction_id: String,
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
        let router_data = &item.router_data;
        let customer_id = router_data.resource_common_data.connector_customer.clone();
        // TH-15: a MIT is a card charge; honour capture_method like the Authorize card arm
        // (G-ThreeDS-04 semantics): one manual capture per intent, never multiple/scheduled.
        let auto_capture = match router_data.request.capture_method {
            None
            | Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic) => true,
            Some(common_enums::CaptureMethod::Manual) => false,
            Some(
                other @ (common_enums::CaptureMethod::ManualMultiple
                | common_enums::CaptureMethod::Scheduled),
            ) => {
                return Err(error_stack::report!(
                    IntegrationError::CaptureMethodNotSupported {
                        context: aw_err_ctx(
                            format!(
                                "Airwallex recurring card payments do not support capture_method \
                                 {other:?}; Airwallex allows one capture per payment intent"
                            ),
                            "Send capture_method AUTOMATIC or MANUAL (single capture) on \
                             RecurringPaymentService/Charge",
                        ),
                    }
                ))
            }
        };
        let payment_method_options = Some(AirwallexPaymentOptions {
            card: Some(AirwallexCardOptions {
                auto_capture: Some(auto_capture),
                authorization_type: None,
                three_ds_action: None,
            }),
            klarna: None,
            atome: None,
        });
        let request_id = airwallex_request_id(
            "mit_confirm",
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let return_url = router_data.request.router_return_url.clone();

        match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(cm) => {
                // A consent MIT charges the stored payment method; a payment method sent
                // alongside it has no Airwallex slot on this request.
                match &router_data.request.payment_method_data {
                    domain_types::payment_method_data::PaymentMethodData::MandatePayment => {}
                    _ => {
                        return Err(IntegrationError::NotSupported {
                            message: "RepeatPayment with a connector mandate id and a payment \
                                      method other than MandatePayment"
                                .to_string(),
                            connector: "Airwallex",
                            context: aw_err_ctx(
                                "An Airwallex consent MIT charges the payment method stored \
                                 under the payment_consent; it takes no new payment method data",
                                "Send only connector_recurring_payment_id.connector_mandate_id \
                                 on RecurringPaymentService/Charge, without payment_method",
                            ),
                        }
                        .into())
                    }
                }

                // Airwallex MIT requires BOTH payment_consent_id (cst_...) AND
                // payment_method.id (pm_...). The connector rejects the request with
                // "triggered_by should not be set, payment_method.id should be provided
                // when triggered_by is set" if payment_method.id is missing.
                let connector_mandate_id =
                    cm.get_connector_mandate_id()
                        .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: aw_err_ctx(
                            "An Airwallex consent MIT sends the PaymentConsent id (cst_...) as \
                             payment_consent_id",
                            "Send the connector_mandate_id returned by \
                             PaymentService/SetupRecurring in \
                             connector_recurring_payment_id.connector_mandate_id",
                        ),
                    })?;
                // Airwallex MIT replays the Airwallex payment-method token. hyperswitch stores
                // its OWN id in payment_method_id but round-trips the connector token in
                // mandate_metadata as {"id": ...}; prefer that, falling back to
                // payment_method_id for older stored mandates.
                let payment_method_id = cm
                    .get_mandate_metadata()
                    .and_then(|meta| {
                        serde_json::from_value::<AirwallexMandateMetadata>(meta.expose()).ok()
                    })
                    .and_then(|meta| meta.id)
                    .or_else(|| cm.get_payment_method_id().cloned())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_id",
                        context: aw_err_ctx(
                            "An Airwallex consent MIT also sends the stored payment method id \
                             (pm_...) as payment_method.id, read from mandate_metadata.id or \
                             payment_method_id",
                            "Send the mandate_metadata (or payment_method_id) returned by \
                             PaymentService/SetupRecurring with the connector_mandate_id",
                        ),
                    })?;

                Ok(Self::ConsentMit(AirwallexConsentMitRequest {
                    request_id,
                    payment_method: AirwallexRepeatPaymentMethodId {
                        id: payment_method_id,
                    },
                    payment_consent_id: Secret::new(connector_mandate_id),
                    triggered_by: AirwallexTriggeredBy::Merchant,
                    customer_id,
                    return_url,
                    payment_method_options,
                }))
            }
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                let card = match &router_data.request.payment_method_data {
                    domain_types::payment_method_data::PaymentMethodData::CardDetailsForNetworkTransactionId(card) => card,
                    _ => {
                        return Err(IntegrationError::NotSupported {
                            message: "RepeatPayment with a network transaction id and a payment \
                                      method other than CardDetailsForNetworkTransactionId"
                                .to_string(),
                            connector: "Airwallex",
                            context: aw_err_ctx(
                                "Airwallex external recurring payments \
                                 (external_recurring_data) need the raw card: number and expiry \
                                 alongside the network transaction id",
                                "Send the card details for the network transaction id MIT, or \
                                 charge a connector_mandate_id from PaymentService/SetupRecurring",
                            ),
                        }
                        .into())
                    }
                };

                Ok(Self::NetworkTransactionIdMit(
                    AirwallexNetworkTransactionIdMitRequest {
                        request_id,
                        payment_method: AirwallexNtidPaymentMethod {
                            payment_method_type: AirwallexPaymentType::Card,
                            card: AirwallexNtidCardDetails {
                                number: card.card_number.clone(),
                                expiry_month: card.card_exp_month.clone(),
                                expiry_year: card.get_expiry_year_4_digit(),
                                name: card.card_holder_name.clone(),
                            },
                        },
                        external_recurring_data: AirwallexExternalRecurringData {
                            triggered_by: AirwallexTriggeredBy::Merchant,
                            initial_payment: false,
                            merchant_trigger_reason: AirwallexMerchantTriggeredReason::Unscheduled,
                            original_transaction_id: network_mandate.network_transaction_id.clone(),
                        },
                        customer_id,
                        return_url,
                        payment_method_options,
                    },
                ))
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotSupported {
                message: "RepeatPayment with a network token and network transaction id"
                    .to_string(),
                connector: "Airwallex",
                context: aw_err_ctx(
                    "Airwallex external recurring payments accept a raw card with the network \
                     transaction id; a network token MIT is not mapped",
                    "Send the raw card details with the network transaction id, or charge a \
                     connector_mandate_id from PaymentService/SetupRecurring",
                ),
            }
            .into()),
        }
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

        // A 2xx intent can still be a declined MIT: FAILED, or REQUIRES_PAYMENT_METHOD after the
        // confirm. Surface it as an ErrorResponse built from latest_payment_attempt rather than a
        // success carrying a Failure status.
        if status == AttemptStatus::Failure {
            let error = build_attempt_error_response(
                &item.response,
                item.http_code,
                FlowStatus::Payment(AttemptStatus::Failure),
            );
            return Ok(Self {
                response: Err(error),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let redirection_data = build_redirection_data(&item.response.next_action);

        // The MIT's scheme transaction id, so a later NTID MIT can reference it.
        let network_txn_id = get_network_txn_id(&item.response);

        // AVS/CVC/auth-code outcome of the MIT authorisation.
        let connector_response = build_card_connector_response(&item.response)
            .map(|card_response| ConnectorResponseData::new(Some(card_response), None, None));

        let intent_id = item.response.id;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(intent_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(intent_id),
                incremental_authorization_allowed: Some(false),
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

        let request_id = airwallex_request_id(
            "customer",
            &item
                .router_data
                .resource_common_data
                .connector_request_reference_id,
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

// ===== WEBHOOK TYPES (IncomingWebhook) =====
//
// Airwallex posts `{id, name, account_id|accountId, data: {object}, created_at?, version?}`. The
// `data.object` shape is chosen by the `name` prefix (PaymentIntent / PaymentAttempt / Refund /
// PaymentDispute), so the envelope keeps it as a raw `serde_json::Value` and it is parsed into the
// matching struct only after `name` is known. An untagged enum would wrongly bind an attempt as an
// intent (they share most fields). No `deny_unknown_fields`: `data.object` is a full retrieve body.

#[derive(Debug, Deserialize)]
pub struct AirwallexWebhookEnvelope {
    pub id: String,
    pub name: AirwallexWebhookEventName,
    /// Airwallex's own docs spell this both `account_id` and `accountId`.
    #[serde(alias = "accountId")]
    pub account_id: Option<String>,
    pub data: AirwallexWebhookData,
    pub created_at: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AirwallexWebhookData {
    pub object: serde_json::Value,
}

/// The modelled webhook `name`s. Hyperswitch's `dispute.*` spellings are accepted as aliases of
/// the documented `payment_dispute.*` names. Every other name (`payment_consent.*`, `customer.*`,
/// `payment_method.*`, `payment_link.*`, `fraud.*`, `funds_split.*`, `pos.*`, and anything new)
/// lands on `Unknown` and is reported as `IncomingWebhookEventUnspecified`.
// TODO(mandate-webhooks): `payment_consent.*` could map to the Mandate* event types.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum AirwallexWebhookEventName {
    #[serde(rename = "payment_intent.created")]
    PaymentIntentCreated,
    #[serde(rename = "payment_intent.requires_payment_method")]
    PaymentIntentRequiresPaymentMethod,
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
    #[serde(rename = "refund.accepted")]
    RefundAccepted,
    #[serde(rename = "refund.settled")]
    RefundSettled,
    /// Spelling Hyperswitch's Airwallex connector subscribes to alongside `refund.settled`.
    #[serde(rename = "refund.succeeded")]
    RefundSucceeded,
    #[serde(rename = "refund.failed")]
    RefundFailed,
    #[serde(rename = "payment_dispute.requires_response")]
    DisputeRequiresResponse,
    #[serde(
        rename = "payment_dispute.challenged",
        alias = "dispute.dispute_responded_by_merchant"
    )]
    DisputeChallenged,
    #[serde(
        rename = "payment_dispute.accepted",
        alias = "dispute.accepted",
        alias = "dispute.dispute.pre_chargeback_accepted"
    )]
    DisputeAccepted,
    #[serde(rename = "payment_dispute.expired")]
    DisputeExpired,
    #[serde(rename = "payment_dispute.pending_closure")]
    DisputePendingClosure,
    #[serde(rename = "payment_dispute.pending_decision")]
    DisputePendingDecision,
    #[serde(rename = "payment_dispute.won", alias = "dispute.won")]
    DisputeWon,
    #[serde(rename = "payment_dispute.lost", alias = "dispute.lost")]
    DisputeLost,
    #[serde(
        rename = "payment_dispute.reversed",
        alias = "dispute.dispute_reversed"
    )]
    DisputeReversed,
    #[serde(other)]
    Unknown,
}

/// Which resource `data.object` holds, derived from the event `name` prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirwallexWebhookResource {
    PaymentIntent,
    PaymentAttempt,
    Refund,
    Dispute,
    Unmodelled,
}

impl AirwallexWebhookEventName {
    pub fn resource(self) -> AirwallexWebhookResource {
        match self {
            Self::PaymentIntentCreated
            | Self::PaymentIntentRequiresPaymentMethod
            | Self::PaymentIntentUpdated
            | Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentIntentRequiresCapture
            | Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentIntentSucceeded
            | Self::PaymentIntentCancelled
            | Self::PaymentIntentPaymentFailed => AirwallexWebhookResource::PaymentIntent,
            Self::PaymentAttemptReceived
            | Self::PaymentAttemptAuthenticationRedirected
            | Self::PaymentAttemptAuthenticationFailed
            | Self::PaymentAttemptPendingAuthorization
            | Self::PaymentAttemptAuthorized
            | Self::PaymentAttemptAuthorizationFailed
            | Self::PaymentAttemptCaptureRequested
            | Self::PaymentAttemptCaptureFailed
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid
            | Self::PaymentAttemptCancelled
            | Self::PaymentAttemptExpired
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess => AirwallexWebhookResource::PaymentAttempt,
            Self::RefundReceived
            | Self::RefundAccepted
            | Self::RefundSettled
            | Self::RefundSucceeded
            | Self::RefundFailed => AirwallexWebhookResource::Refund,
            Self::DisputeRequiresResponse
            | Self::DisputeChallenged
            | Self::DisputeAccepted
            | Self::DisputeExpired
            | Self::DisputePendingClosure
            | Self::DisputePendingDecision
            | Self::DisputeWon
            | Self::DisputeLost
            | Self::DisputeReversed => AirwallexWebhookResource::Dispute,
            Self::Unknown => AirwallexWebhookResource::Unmodelled,
        }
    }

    /// Event name -> UCS [`EventType`] (spec 15.4; dispute judgement calls per plan UD-08).
    pub fn event_type(self) -> EventType {
        match self {
            Self::PaymentIntentCreated
            | Self::PaymentIntentRequiresPaymentMethod
            | Self::PaymentIntentUpdated
            | Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentAttemptReceived
            | Self::PaymentAttemptPendingAuthorization => EventType::PaymentIntentProcessing,
            Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentAttemptAuthenticationRedirected => EventType::PaymentActionRequired,
            Self::PaymentIntentRequiresCapture | Self::PaymentAttemptAuthorized => {
                EventType::PaymentIntentAuthorizationSuccess
            }
            Self::PaymentIntentSucceeded
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid => EventType::PaymentIntentSuccess,
            Self::PaymentIntentCancelled | Self::PaymentAttemptCancelled => {
                EventType::PaymentIntentCancelled
            }
            Self::PaymentIntentPaymentFailed
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess => EventType::PaymentIntentFailure,
            Self::PaymentAttemptAuthenticationFailed | Self::PaymentAttemptAuthorizationFailed => {
                EventType::PaymentIntentAuthorizationFailure
            }
            Self::PaymentAttemptCaptureRequested => EventType::PaymentIntentCaptureSuccess,
            Self::PaymentAttemptCaptureFailed => EventType::PaymentIntentCaptureFailure,
            Self::PaymentAttemptExpired => EventType::PaymentIntentExpired,
            Self::RefundReceived | Self::RefundAccepted => EventType::RefundProcessing,
            Self::RefundSettled | Self::RefundSucceeded => EventType::RefundSuccess,
            Self::RefundFailed => EventType::RefundFailure,
            Self::DisputeRequiresResponse => EventType::DisputeOpened,
            Self::DisputeChallenged | Self::DisputePendingDecision => EventType::DisputeChallenged,
            Self::DisputeAccepted | Self::DisputePendingClosure => EventType::DisputeAccepted,
            Self::DisputeExpired => EventType::DisputeExpired,
            Self::DisputeWon | Self::DisputeReversed => EventType::DisputeWon,
            Self::DisputeLost => EventType::DisputeLost,
            Self::Unknown => EventType::IncomingWebhookEventUnspecified,
        }
    }

    /// Event name -> [`common_enums::DisputeStatus`]; `None` for non-dispute names.
    fn dispute_status(self) -> Option<common_enums::DisputeStatus> {
        match self {
            Self::DisputeRequiresResponse => Some(common_enums::DisputeStatus::DisputeOpened),
            Self::DisputeChallenged | Self::DisputePendingDecision => {
                Some(common_enums::DisputeStatus::DisputeChallenged)
            }
            Self::DisputeAccepted | Self::DisputePendingClosure => {
                Some(common_enums::DisputeStatus::DisputeAccepted)
            }
            Self::DisputeExpired => Some(common_enums::DisputeStatus::DisputeExpired),
            Self::DisputeWon | Self::DisputeReversed => {
                Some(common_enums::DisputeStatus::DisputeWon)
            }
            Self::DisputeLost => Some(common_enums::DisputeStatus::DisputeLost),
            Self::PaymentIntentCreated
            | Self::PaymentIntentRequiresPaymentMethod
            | Self::PaymentIntentUpdated
            | Self::PaymentIntentRequiresCustomerAction
            | Self::PaymentIntentRequiresCapture
            | Self::PaymentIntentPending
            | Self::PaymentIntentPendingReview
            | Self::PaymentIntentSucceeded
            | Self::PaymentIntentCancelled
            | Self::PaymentIntentPaymentFailed
            | Self::PaymentAttemptReceived
            | Self::PaymentAttemptAuthenticationRedirected
            | Self::PaymentAttemptAuthenticationFailed
            | Self::PaymentAttemptPendingAuthorization
            | Self::PaymentAttemptAuthorized
            | Self::PaymentAttemptAuthorizationFailed
            | Self::PaymentAttemptCaptureRequested
            | Self::PaymentAttemptCaptureFailed
            | Self::PaymentAttemptSettled
            | Self::PaymentAttemptPaid
            | Self::PaymentAttemptCancelled
            | Self::PaymentAttemptExpired
            | Self::PaymentAttemptRiskDeclined
            | Self::PaymentAttemptFailedToProcess
            | Self::RefundReceived
            | Self::RefundAccepted
            | Self::RefundSettled
            | Self::RefundSucceeded
            | Self::RefundFailed
            | Self::Unknown => None,
        }
    }
}

/// Payment status for a payment webhook, derived from the event **name** (the attempt `status`
/// collapses five failure kinds into `FAILED`). Only `payment_intent.updated` reads the object's
/// own status. Non-payment names -> `WebhookEventTypeNotFound`.
fn airwallex_webhook_attempt_status(
    name: AirwallexWebhookEventName,
    intent: Option<&AirwallexPaymentsResponse>,
) -> Result<AttemptStatus, error_stack::Report<WebhookError>> {
    use AirwallexWebhookEventName as N;
    Ok(match name {
        N::PaymentIntentCreated | N::PaymentIntentRequiresPaymentMethod => {
            AttemptStatus::PaymentMethodAwaited
        }
        N::PaymentIntentUpdated => {
            let intent = intent
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookResourceObjectNotFound))?;
            get_payment_status(&intent.status, &intent.next_action)
        }
        N::PaymentIntentRequiresCustomerAction | N::PaymentAttemptAuthenticationRedirected => {
            AttemptStatus::AuthenticationPending
        }
        N::PaymentIntentRequiresCapture | N::PaymentAttemptAuthorized => AttemptStatus::Authorized,
        N::PaymentIntentPending | N::PaymentIntentPendingReview | N::PaymentAttemptReceived => {
            AttemptStatus::Pending
        }
        N::PaymentIntentSucceeded
        | N::PaymentAttemptCaptureRequested
        | N::PaymentAttemptSettled
        | N::PaymentAttemptPaid => AttemptStatus::Charged,
        N::PaymentIntentCancelled | N::PaymentAttemptCancelled => AttemptStatus::Voided,
        N::PaymentIntentPaymentFailed
        | N::PaymentAttemptRiskDeclined
        | N::PaymentAttemptFailedToProcess => AttemptStatus::Failure,
        N::PaymentAttemptAuthenticationFailed => AttemptStatus::AuthenticationFailed,
        N::PaymentAttemptPendingAuthorization => AttemptStatus::Authorizing,
        N::PaymentAttemptAuthorizationFailed => AttemptStatus::AuthorizationFailed,
        N::PaymentAttemptCaptureFailed => AttemptStatus::CaptureFailed,
        N::PaymentAttemptExpired => AttemptStatus::Expired,
        N::RefundReceived
        | N::RefundAccepted
        | N::RefundSettled
        | N::RefundSucceeded
        | N::RefundFailed
        | N::DisputeRequiresResponse
        | N::DisputeChallenged
        | N::DisputeAccepted
        | N::DisputeExpired
        | N::DisputePendingClosure
        | N::DisputePendingDecision
        | N::DisputeWon
        | N::DisputeLost
        | N::DisputeReversed
        | N::Unknown => {
            return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Airwallex webhook: event name is not a payment event");
        }
    })
}

/// `data.object` of a `payment_dispute.*` event. The one published webhook sample carries only
/// `id`, `amount`, `currency`, `due_at`, `stage` and `status`, so everything else is optional.
#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDisputeObject {
    pub id: String,
    pub payment_intent_id: Option<String>,
    /// Major-unit decimal, like every Airwallex amount.
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub stage: AirwallexDisputeStage,
    pub status: AirwallexDisputeStatus,
    pub reason: Option<AirwallexDisputeReason>,
    pub merchant_order_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AirwallexDisputeStage {
    Rfi,
    PreChargeback,
    Chargeback,
    /// Hyperswitch's spelling of the chargeback stage.
    Dispute,
    PreArbitration,
    Arbitration,
    #[serde(other)]
    Unknown,
}

/// Deserialised for completeness; the webhook status is taken from the event name.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct AirwallexDisputeReason {
    pub original_code: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub reason_type: Option<String>,
}

/// Airwallex dispute stage -> UCS stage. `ARBITRATION` folds into `PreArbitration` (UCS has no
/// arbitration stage; plan UD-09). An unknown stage is refused rather than guessed.
fn airwallex_dispute_stage(
    stage: AirwallexDisputeStage,
) -> Result<common_enums::DisputeStage, error_stack::Report<WebhookError>> {
    match stage {
        AirwallexDisputeStage::Rfi | AirwallexDisputeStage::PreChargeback => {
            Ok(common_enums::DisputeStage::PreDispute)
        }
        AirwallexDisputeStage::Chargeback | AirwallexDisputeStage::Dispute => {
            Ok(common_enums::DisputeStage::Dispute)
        }
        AirwallexDisputeStage::PreArbitration | AirwallexDisputeStage::Arbitration => {
            Ok(common_enums::DisputeStage::PreArbitration)
        }
        AirwallexDisputeStage::Unknown => {
            Err(error_stack::report!(WebhookError::WebhookProcessingFailed))
                .attach_printable("Airwallex dispute webhook: unrecognised data.object.stage")
        }
    }
}

/// `data.object` parsed by the event name's resource class.
#[derive(Debug)]
pub enum AirwallexWebhookObject {
    PaymentIntent(Box<AirwallexPaymentsResponse>),
    PaymentAttempt(Box<AirwallexPaymentAttempt>),
    Refund(Box<AirwallexRefundResponse>),
    Dispute(Box<AirwallexDisputeObject>),
    Unmodelled,
}

fn parse_webhook_object<O: serde::de::DeserializeOwned>(
    object: &serde_json::Value,
    what: &'static str,
) -> Result<O, error_stack::Report<WebhookError>> {
    serde_json::from_value(object.clone())
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable_lazy(|| format!("Airwallex webhook: data.object is not a {what}"))
}

impl AirwallexWebhookEnvelope {
    pub fn from_body(body: &[u8]) -> Result<Self, error_stack::Report<WebhookError>> {
        serde_json::from_slice(body)
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Airwallex webhook: body is not a webhook envelope")
    }

    pub fn object(&self) -> Result<AirwallexWebhookObject, error_stack::Report<WebhookError>> {
        let object = &self.data.object;
        Ok(match self.name.resource() {
            AirwallexWebhookResource::PaymentIntent => AirwallexWebhookObject::PaymentIntent(
                parse_webhook_object(object, "PaymentIntent")?,
            ),
            AirwallexWebhookResource::PaymentAttempt => AirwallexWebhookObject::PaymentAttempt(
                parse_webhook_object(object, "PaymentAttempt")?,
            ),
            AirwallexWebhookResource::Refund => {
                AirwallexWebhookObject::Refund(parse_webhook_object(object, "Refund")?)
            }
            AirwallexWebhookResource::Dispute => {
                AirwallexWebhookObject::Dispute(parse_webhook_object(object, "PaymentDispute")?)
            }
            AirwallexWebhookResource::Unmodelled => AirwallexWebhookObject::Unmodelled,
        })
    }
}

/// The refund `request_id` this connector sends is `refund_{connector_request_reference_id}`;
/// strip the prefix to recover the merchant refund id. A value without the prefix came from a
/// non-UCS caller and passes through unchanged.
fn merchant_refund_id_from_request_id(request_id: &str) -> String {
    match request_id.strip_prefix("refund_") {
        Some(stripped) => stripped.to_string(),
        None => request_id.to_string(),
    }
}

/// `EventService.ParseEvent` reference (spec 15.7). The attempt id (`att_...`) is never used as
/// the transaction id: every Airwallex `resource_id` is the intent id (`int_...`).
pub fn get_airwallex_webhook_reference(
    envelope: &AirwallexWebhookEnvelope,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    Ok(match envelope.object()? {
        AirwallexWebhookObject::PaymentIntent(intent) => {
            Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(intent.id),
                merchant_transaction_id: intent.merchant_order_id,
            }))
        }
        AirwallexWebhookObject::PaymentAttempt(attempt) => {
            Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: attempt.payment_intent_id,
                merchant_transaction_id: attempt.merchant_order_id,
            }))
        }
        AirwallexWebhookObject::Refund(refund) => {
            Some(WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(refund.id),
                merchant_refund_id: refund
                    .request_id
                    .as_deref()
                    .map(merchant_refund_id_from_request_id),
                connector_transaction_id: refund.payment_intent_id,
                merchant_transaction_id: None,
            }))
        }
        AirwallexWebhookObject::Dispute(dispute) => {
            Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                connector_dispute_id: Some(dispute.id),
                connector_transaction_id: dispute.payment_intent_id,
            }))
        }
        AirwallexWebhookObject::Unmodelled => None,
    })
}

/// Payment webhook (`payment_intent.*` / `payment_attempt.*`) -> [`WebhookDetailsResponse`]
/// (spec 15.8). Status comes from the event name, never from `data.object.status` alone.
pub fn get_airwallex_payment_webhook_details(
    envelope: &AirwallexWebhookEnvelope,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let object = envelope.object()?;
    let (
        intent_id,
        merchant_order_id,
        attempt,
        captured_amount,
        currency,
        consent_id,
        pm_id,
        status,
    ) = match &object {
        AirwallexWebhookObject::PaymentIntent(intent) => {
            let attempt = intent.latest_payment_attempt.as_ref();
            let pm_id = attempt
                .and_then(|lpa| lpa.payment_method.as_ref())
                .and_then(|pm| pm.id.clone())
                .or_else(|| intent.payment_method.as_ref().and_then(|pm| pm.id.clone()));
            (
                intent.id.clone(),
                intent.merchant_order_id.clone(),
                attempt,
                intent.captured_amount,
                intent.currency,
                intent.payment_consent_id.clone(),
                pm_id,
                airwallex_webhook_attempt_status(envelope.name, Some(intent))?,
            )
        }
        AirwallexWebhookObject::PaymentAttempt(attempt) => {
            let intent_id = attempt
                .payment_intent_id
                .clone()
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))
                .attach_printable("Airwallex payment_attempt webhook: missing payment_intent_id")?;
            (
                intent_id,
                attempt.merchant_order_id.clone(),
                Some(attempt.as_ref()),
                attempt.captured_amount,
                attempt.currency,
                attempt.payment_consent_id.clone(),
                attempt.payment_method.as_ref().and_then(|pm| pm.id.clone()),
                airwallex_webhook_attempt_status(envelope.name, None)?,
            )
        }
        AirwallexWebhookObject::Refund(_)
        | AirwallexWebhookObject::Dispute(_)
        | AirwallexWebhookObject::Unmodelled => {
            return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Airwallex webhook: event name is not a payment event");
        }
    };

    // Captured amount only on the charged outcomes; Airwallex amounts are major-unit decimals.
    let minor_amount_captured = match (status, captured_amount, currency) {
        (AttemptStatus::Charged, Some(amount), Some(currency)) => Some(
            domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
                &FloatMajorUnitForConnector,
                amount,
                currency,
            )?,
        ),
        _ => None,
    };

    let is_failure = matches!(
        status,
        AttemptStatus::Failure
            | AttemptStatus::AuthenticationFailed
            | AttemptStatus::AuthorizationFailed
            | AttemptStatus::CaptureFailed
    );
    let (error_code, error_message, error_reason) = match (is_failure, attempt) {
        (true, Some(attempt)) => (
            attempt.failure_code.clone().or_else(|| {
                attempt
                    .failure_details
                    .as_ref()
                    .and_then(|details| details.code.clone())
            }),
            attempt
                .failure_details
                .as_ref()
                .and_then(|details| details.message.clone()),
            attempt.provider_original_response_description.clone(),
        ),
        _ => (None, None, None),
    };

    let mandate_reference = consent_id
        .map(|id| MandateReference {
            connector_mandate_id: Some(id.expose()),
            payment_method_id: pm_id.clone(),
            connector_mandate_request_reference_id: None,
            // Same {"id": ...} round-trip as the Authorize/SetupMandate response builders.
            mandate_metadata: pm_id.map(|pm_id| Secret::new(serde_json::json!({ "id": pm_id }))),
        })
        .map(Box::new);

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(intent_id.clone())),
        status,
        connector_response_reference_id: Some(intent_id),
        connector_request_reference_id: merchant_order_id,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: minor_amount_captured.map(|amount| amount.get_amount_as_i64()),
        minor_amount_captured,
        network_txn_id: attempt.and_then(|attempt| attempt.payment_method_transaction_id.clone()),
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Refund webhook (`refund.*`) -> [`RefundWebhookDetailsResponse`]. Status goes through the same
/// `From<AirwallexRefundStatus>` RSync uses, so the two paths cannot disagree.
pub fn get_airwallex_refund_webhook_details(
    envelope: &AirwallexWebhookEnvelope,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let refund = match envelope.object()? {
        AirwallexWebhookObject::Refund(refund) => refund,
        AirwallexWebhookObject::PaymentIntent(_)
        | AirwallexWebhookObject::PaymentAttempt(_)
        | AirwallexWebhookObject::Dispute(_)
        | AirwallexWebhookObject::Unmodelled => {
            return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Airwallex webhook: event name is not a refund event");
        }
    };
    let refund = *refund;
    let (error_code, error_message) = match refund.failure_details {
        Some(details) => (details.code, details.message),
        None => (None, None),
    };
    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(refund.id),
        merchant_transaction_id: refund
            .request_id
            .as_deref()
            .map(merchant_refund_id_from_request_id),
        status: RefundStatus::from(refund.status),
        connector_response_reference_id: refund.payment_intent_id,
        error_code,
        error_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Dispute webhook (`payment_dispute.*`, HS `dispute.*`) -> [`DisputeWebhookDetailsResponse`].
/// Status from the event name, stage from `data.object.stage`, amount major -> minor.
pub fn get_airwallex_dispute_webhook_details(
    envelope: &AirwallexWebhookEnvelope,
    raw_body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let dispute = match envelope.object()? {
        AirwallexWebhookObject::Dispute(dispute) => *dispute,
        AirwallexWebhookObject::PaymentIntent(_)
        | AirwallexWebhookObject::PaymentAttempt(_)
        | AirwallexWebhookObject::Refund(_)
        | AirwallexWebhookObject::Unmodelled => {
            return Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Airwallex webhook: event name is not a dispute event");
        }
    };
    let status = envelope
        .name
        .dispute_status()
        .ok_or_else(|| error_stack::report!(WebhookError::WebhookEventTypeNotFound))?;
    let stage = airwallex_dispute_stage(dispute.stage)?;
    let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        &FloatMajorUnitForConnector,
        dispute.amount,
        dispute.currency,
    )?;
    let amount = domain_types::utils::convert_amount_for_webhook(
        &StringMinorUnitForConnector,
        minor_amount,
        dispute.currency,
    )?;
    let (dispute_message, connector_reason_code) = match dispute.reason {
        Some(reason) => (reason.description, reason.original_code),
        None => (None, None),
    };
    Ok(DisputeWebhookDetailsResponse {
        amount,
        currency: dispute.currency,
        dispute_id: dispute.id,
        status,
        stage,
        connector_response_reference_id: dispute.payment_intent_id,
        dispute_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        connector_reason_code,
    })
}

/// `payment_intent.succeeded` fixture for the field probe (`sample_webhook_body`).
pub(crate) const AIRWALLEX_SAMPLE_WEBHOOK_BODY: &[u8] = br#"{"id":"evt_100_2019102201549020043_8321220011893766","name":"payment_intent.succeeded","accountId":"78814faa-1b30-4598-a9c8-f0583db8d09d","data":{"object":{"request_id":"d6a92e2a-02e5-c37b-c977-13796ec7443a","id":"int_aaaat9w2hgh8mzi1111","merchant_order_id":"0000000000","amount":16.66,"currency":"USD","captured_amount":16.66,"status":"SUCCEEDED","created_at":"2023-01-13T07:32:05+0000","updated_at":"2023-01-13T07:32:05+0000"}}}"#;
