use common_utils::{consts, pii, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync,
        PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, ContinueRedirectionResponse, EventType,
        MandateReference, MandateReferenceId,
        NuveiClientAuthenticationResponse as NuveiClientAuthenticationResponseDomain,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData, WebhookDetailsResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankDebitData, BankRedirectData, BankTransferData, NetworkTokenData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use super::NuveiRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError,
};

// Nuvei's APM (Alternative Payment Method) identifier for ACH. Required literal
// per Nuvei's API; reused by both BankTransfer::AchBankTransfer and
// BankDebit::AchBankDebit. See https://docs.nuvei.com/documentation/us-and-canada-guides/ach/
const NUVEI_ACH_PAYMENT_METHOD: &str = "apmgw_ACH";

// Auth Type
#[derive(Debug, Clone)]
pub struct NuveiAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) merchant_site_id: Secret<String>,
    pub(super) merchant_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for NuveiAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Nuvei {
                merchant_id,
                merchant_site_id,
                merchant_secret,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.clone(),
                merchant_site_id: merchant_site_id.clone(),
                merchant_secret: merchant_secret.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: nuvei_error_context(
                    "Configure the Nuvei connector with merchant_id, merchant_site_id and merchant_secret",
                    "Nuvei requires ConnectorSpecificConfig::Nuvei credentials",
                ),
            }
            .into()),
        }
    }
}

impl NuveiAuthType {
    pub fn generate_checksum(&self, params: &[&str]) -> String {
        use sha2::{Digest, Sha256};

        let mut concatenated = params.join("");
        concatenated.push_str(self.merchant_secret.peek());

        let mut hasher = Sha256::new();
        hasher.update(concatenated.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get_timestamp(
    ) -> common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss> {
        // Generate timestamp in YYYYMMDDHHmmss format using common_utils date_time
        common_utils::date_time::DateTime::from(common_utils::date_time::now())
    }
}

// Session Token Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Session Token Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenResponse {
    pub session_token: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

// URL Details for redirect URLs
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiUrlDetails {
    pub success_url: String,
    pub failure_url: String,
    pub pending_url: String,
}

// Payment Request
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub user_token_id: Option<Secret<String>>,
    pub client_unique_id: String,
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    pub shipping_address: Option<NuveiShippingAddress>,
    pub url_details: Option<NuveiUrlDetails>,
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    pub is_partial_approval: Option<NuveiPartialApprovalFlag>,
    pub items: Option<Vec<NuveiItem>>,
    pub amount_details: Option<NuveiAmountDetails>,
    pub is_rebilling: Option<NuveiIsRebilling>,
    pub related_transaction_id: Option<String>,
    pub is_moto: Option<bool>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<NuveiCardPaymentOption<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative_payment_method: Option<NuveiAlternativePaymentMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_payment_option_id: Option<Secret<String>>,
}

// Serialize-only: untagged is wire-invisible, so raw-card requests keep their
// exact previous shape while network-token requests emit the externalToken form
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiCardPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Raw(NuveiCard<T>),
    NetworkToken(NuveiNetworkTokenCard),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d: Option<Box<NuveiCardThreeD>>,
}

/// paymentOption.card.threeD for the payment request. Authorize sends only the
/// external MPI block (merchant-performed 3DS); the Authenticate leg sends the
/// challenge parameters (browserDetails, notificationURL, ...).
#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCardThreeD {
    pub method_completion_ind: Option<NuveiMethodCompletion>,
    pub browser_details: Option<NuveiBrowserDetails>,
    #[serde(rename = "notificationURL")]
    pub notification_url: Option<String>,
    #[serde(rename = "merchantURL")]
    pub merchant_url: Option<String>,
    pub external_mpi: Option<NuveiExternalMpi>,
    pub platform_type: Option<NuveiPlatformType>,
    pub v2_additional_params: Option<NuveiV2AdditionalParams>,
}

/// threeD.methodCompletionInd: the 3DS method URL is not run by this integration.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiMethodCompletion {
    #[serde(rename = "U")]
    Unavailable,
}

/// threeD.platformType: browser-based authentication.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiPlatformType {
    #[serde(rename = "02")]
    Browser,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBrowserDetails {
    pub accept_header: String,
    pub ip: Secret<String, pii::IpAddress>,
    pub java_enabled: String,
    pub java_script_enabled: String,
    pub language: String,
    pub color_depth: u8,
    pub screen_height: u32,
    pub screen_width: u32,
    pub time_zone: i32,
    pub user_agent: String,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiV2AdditionalParams {
    pub challenge_window_size: Option<NuveiChallengeWindowSize>,
    pub challenge_preference: Option<NuveiChallengePreference>,
    pub rebill_expiry: Option<String>,
    pub rebill_frequency: Option<String>,
}

/// threeD.v2AdditionalParams.challengeWindowSize.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiChallengeWindowSize {
    #[serde(rename = "05")]
    FullScreen,
}

/// threeD.v2AdditionalParams.challengePreference.
#[derive(Debug, Clone, Serialize)]
pub enum NuveiChallengePreference {
    #[serde(rename = "01")]
    NoPreference,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalMpi {
    pub eci: Option<String>,
    pub cavv: Secret<String>,
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
}

/// card object used when paying with a network token: no PAN/CVV/holder name,
/// only expiry + externalToken (mirrors hyperswitch get_network_token_info)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNetworkTokenCard {
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub external_token: NuveiNetworkTokenExternalToken,
}

/// Nuvei externalToken payload for network-token payments.
/// tokenAssuranceLevel / tokenRequestorId mirror hyperswitch PR #13093, which
/// always sends None for them today; skip_serializing_none keeps them off the wire.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNetworkTokenExternalToken {
    pub network_token_number: cards::NetworkToken,
    pub network_token_cryptogram: Option<Secret<String>>,
    pub token_assurance_level: Option<String>,
    pub token_requestor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiCardType {
    Visa,
    MasterCard,
    Amex,
    Discover,
    Diners,
}

impl TryFrom<common_enums::CardNetwork> for NuveiCardType {
    type Error = Report<IntegrationError>;

    fn try_from(network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::MasterCard),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::Amex),
            common_enums::CardNetwork::Discover => Ok(Self::Discover),
            common_enums::CardNetwork::DinersClub => Ok(Self::Diners),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Card network {network:?}"),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Use a Visa, Mastercard, American Express, Discover or Diners Club card",
                    format!("Nuvei externalSchemeDetails.brand has no value for card network {network:?}"),
                ),
            }
            .into()),
        }
    }
}

impl TryFrom<&domain_types::utils::CardIssuer> for NuveiCardType {
    type Error = Report<IntegrationError>;

    fn try_from(issuer: &domain_types::utils::CardIssuer) -> Result<Self, Self::Error> {
        match issuer {
            domain_types::utils::CardIssuer::Visa => Ok(Self::Visa),
            domain_types::utils::CardIssuer::Master => Ok(Self::MasterCard),
            domain_types::utils::CardIssuer::AmericanExpress => Ok(Self::Amex),
            domain_types::utils::CardIssuer::Discover => Ok(Self::Discover),
            domain_types::utils::CardIssuer::DinersClub => Ok(Self::Diners),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Card issuer {issuer:?}"),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Use a Visa, Mastercard, American Express, Discover or Diners Club card, or send card_network",
                    format!("Nuvei externalSchemeDetails.brand has no value for card issuer {issuer:?}"),
                ),
            }
            .into()),
        }
    }
}

/// externalSchemeDetails: carries the original network transaction id (NTID)
/// and card brand for MIT network-token payments
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalSchemeDetails {
    pub transaction_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<NuveiCardType>,
}

/// Shared card mapping for network-token CIT and MIT requests
fn build_nuvei_network_token_card(token_data: &NetworkTokenData) -> NuveiNetworkTokenCard {
    NuveiNetworkTokenCard {
        expiration_month: token_data.get_network_token_expiry_month(),
        expiration_year: token_data.get_network_token_expiry_year(),
        external_token: NuveiNetworkTokenExternalToken {
            network_token_number: token_data.get_network_token(),
            network_token_cryptogram: token_data.get_cryptogram(),
            token_assurance_level: None,
            token_requestor_id: None,
        },
    }
}

/// Brand for externalSchemeDetails: prefer the explicit card_network, fall back
/// to BIN-derived issuer
fn get_nuvei_card_brand(
    token_data: &NetworkTokenData,
) -> Result<NuveiCardType, Report<IntegrationError>> {
    match token_data.card_network.clone() {
        Some(network) => NuveiCardType::try_from(network),
        None => NuveiCardType::try_from(&token_data.get_card_issuer()?),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlternativePaymentMethodType {
    #[serde(rename = "apmgw_Giropay")]
    Giropay,
    #[serde(rename = "apmgw_Sofort")]
    Sofort,
    #[serde(rename = "apmgw_iDeal")]
    Ideal,
    #[serde(rename = "apmgw_EPS")]
    Eps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NuveiBIC {
    #[serde(rename = "ABNANL2A")]
    Abnamro,
    #[serde(rename = "ASNBNL21")]
    AsnBank,
    #[serde(rename = "BUNQNL2A")]
    Bunq,
    #[serde(rename = "INGBNL2A")]
    Ing,
    #[serde(rename = "KNABNL2H")]
    Knab,
    #[serde(rename = "RABONL2U")]
    Rabobank,
    #[serde(rename = "RBRBNL21")]
    Regiobank,
    #[serde(rename = "SNSBNL2A")]
    SnsBank,
    #[serde(rename = "TRIONL2U")]
    TriodosBank,
    #[serde(rename = "FVLBNL22")]
    VanLanschotBankiers,
    #[serde(rename = "MOYONL21")]
    Moneyou,
}

impl TryFrom<common_enums::BankNames> for NuveiBIC {
    type Error = Report<IntegrationError>;

    fn try_from(bank: common_enums::BankNames) -> Result<Self, Self::Error> {
        match bank {
            common_enums::BankNames::AbnAmro => Ok(Self::Abnamro),
            common_enums::BankNames::AsnBank => Ok(Self::AsnBank),
            common_enums::BankNames::Bunq => Ok(Self::Bunq),
            common_enums::BankNames::Ing => Ok(Self::Ing),
            common_enums::BankNames::Knab => Ok(Self::Knab),
            common_enums::BankNames::Rabobank => Ok(Self::Rabobank),
            common_enums::BankNames::Regiobank => Ok(Self::Regiobank),
            common_enums::BankNames::SnsBank => Ok(Self::SnsBank),
            common_enums::BankNames::TriodosBank => Ok(Self::TriodosBank),
            common_enums::BankNames::VanLanschot => Ok(Self::VanLanschotBankiers),
            common_enums::BankNames::Moneyou => Ok(Self::Moneyou),
            _ => Err(IntegrationError::NotSupported {
                message: format!("Bank not supported by Nuvei iDEAL: {}", bank),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Send an iDEAL bank_name Nuvei maps to a BIC (e.g. ING, Rabobank, ABN AMRO), or omit bank_name",
                    format!("Nuvei iDEAL has no BIC for bank {bank}"),
                ),
            }
            .into()),
        }
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiAlternativePaymentMethod {
    Ach {
        #[serde(rename = "paymentMethod")]
        payment_method: String,
        #[serde(rename = "AccountNumber")]
        account_number: Secret<String>,
        #[serde(rename = "RoutingNumber")]
        routing_number: Secret<String>,
        #[serde(rename = "SECCode", skip_serializing_if = "Option::is_none")]
        sec_code: Option<String>,
    },
    Redirect {
        #[serde(rename = "paymentMethod")]
        payment_method: AlternativePaymentMethodType,
        #[serde(rename = "BIC")]
        bank_id: Option<NuveiBIC>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDeviceDetails {
    pub ip_address: Secret<String, pii::IpAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBillingAddress {
    // Required fields per Nuvei documentation
    pub email: pii::Email,
    pub country: common_enums::CountryAlpha2,
    // Optional fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
}

/// 2-letter state code for the billing address (US/CA full names converted),
/// as hyperswitch `to_state_code_as_optional`; Nuvei rejects longer values.
fn get_billing_state_code(resource_data: &PaymentFlowData) -> Option<Secret<String>> {
    resource_data
        .get_optional_billing()
        .and_then(|billing| billing.address.as_ref())
        .and_then(|details| details.to_state_code_as_optional().ok().flatten())
}

fn build_billing_address(
    resource_data: &PaymentFlowData,
    email: pii::Email,
    country: common_enums::CountryAlpha2,
) -> NuveiBillingAddress {
    let address_line3 = resource_data
        .get_optional_billing()
        .and_then(|billing| billing.address.as_ref())
        .and_then(|addr| addr.line3.clone());
    NuveiBillingAddress {
        email,
        country,
        first_name: resource_data.get_optional_billing_first_name(),
        last_name: resource_data.get_optional_billing_last_name(),
        phone: resource_data.get_optional_billing_phone_number(),
        city: resource_data.get_optional_billing_city(),
        address: resource_data.get_optional_billing_line1(),
        address_line2: resource_data.get_optional_billing_line2(),
        address_line3,
        zip: resource_data.get_optional_billing_zip(),
        state: get_billing_state_code(resource_data),
    }
}

/// paymentOption.card.cardHolderName: the card's own holder name when present
/// and non-empty, else the billing full name (hyperswitch source). Used by every
/// raw-card request so initPayment, the 3DS payment and the final payment agree.
fn get_nuvei_card_holder_name<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
    resource_data: &PaymentFlowData,
) -> Option<Secret<String>> {
    card.card_holder_name
        .clone()
        .filter(|name| !name.peek().trim().is_empty())
        .or_else(|| resource_data.get_optional_billing_full_name())
}

/// Billing address for flows where Nuvei requires it: billing email (else the
/// request email) and billing country are mandatory, each with its own error.
fn get_required_billing_address(
    resource_data: &PaymentFlowData,
    fallback_email: Option<pii::Email>,
) -> Result<NuveiBillingAddress, Report<IntegrationError>> {
    let email = resource_data
        .get_optional_billing_email()
        .or(fallback_email)
        .ok_or_else(|| {
            nuvei_missing_field(
                "billing_address.email",
                "Send address.billing_address.email or customer.email",
            )
        })?;
    let country = resource_data
        .get_optional_billing_country()
        .ok_or_else(|| {
            nuvei_missing_field(
                "billing_address.country",
                "Send address.billing_address.country_alpha2_code",
            )
        })?;
    Ok(build_billing_address(resource_data, email, country))
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiShippingAddress {
    pub email: pii::Email,
    pub country: common_enums::CountryAlpha2,
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub address_line3: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
}

/// Nuvei `shippingAddress`, emitted only when a shipping address with a
/// country is present; email falls back to the billing email.
fn get_shipping_address(
    resource_data: &PaymentFlowData,
    billing_email: Option<pii::Email>,
) -> Option<NuveiShippingAddress> {
    let country = resource_data.get_optional_shipping_country()?;
    let email = resource_data
        .get_optional_shipping_email()
        .or(billing_email)?;
    Some(NuveiShippingAddress {
        email,
        country,
        first_name: resource_data.get_optional_shipping_first_name(),
        last_name: resource_data.get_optional_shipping_last_name(),
        phone: resource_data.get_optional_shipping_phone_number(),
        city: resource_data.get_optional_shipping_city(),
        address: resource_data.get_optional_shipping_line1(),
        address_line2: resource_data.get_optional_shipping_line2(),
        address_line3: resource_data.get_optional_shipping_line3(),
        zip: resource_data.get_optional_shipping_zip(),
    })
}

// Payment Response.
//
// One response shape for every /payment.do-family call (payment, settle, void,
// refund), mirroring hyperswitch's NuveiPaymentsResponse. The flow-specific
// names below are type aliases so each macro `response_body` keeps its own
// templating type while sharing one field set.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentsResponse {
    pub order_id: Option<String>,
    pub user_token_id: Option<Secret<String>>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub transaction_id: Option<String>,
    pub auth_code: Option<String>,
    pub gw_error_code: Option<i64>,
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub payment_method_error_code: Option<String>,
    pub payment_method_error_reason: Option<String>,
    pub merchant_advice_code: Option<String>,
    /// Network transaction id (NTID)
    pub external_scheme_transaction_id: Option<Secret<String>>,
    /// Mastercard transaction link id (TLID)
    pub transaction_link_id: Option<String>,
    pub session_token: Option<Secret<String>>,
    pub partial_approval: Option<NuveiPartialApproval>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
}

/// Authorize response body.
pub type NuveiPaymentResponse = NuveiPaymentsResponse;
/// Capture (/settleTransaction.do) response body.
pub type NuveiCaptureResponse = NuveiPaymentsResponse;
/// Void (/voidTransaction.do) response body.
pub type NuveiVoidResponse = NuveiPaymentsResponse;
/// Refund (/refundTransaction.do) response body.
pub type NuveiRefundResponse = NuveiPaymentsResponse;
/// SetupMandate (/payment.do, CIT) response body.
pub type NuveiSetupMandateResponse = NuveiPaymentsResponse;
/// RepeatPayment (/payment.do, MIT) response body.
pub type NuveiRepeatPaymentResponse = NuveiPaymentsResponse;

/// paymentOption as returned by Nuvei: the stored payment option id (UPO),
/// an APM redirect URL and the card block carrying AVS/CVV/3DS results.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponsePaymentOption {
    pub user_payment_option_id: Option<Secret<String>>,
    pub redirect_url: Option<String>,
    pub card: Option<NuveiResponseCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseCard {
    pub avs_code: Option<String>,
    pub cvv2_reply: Option<String>,
    pub brand: Option<String>,
    pub card_type: Option<String>,
    pub issuer_country: Option<String>,
    pub three_d: Option<NuveiResponseThreeD>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseThreeD {
    pub acs_url: Option<String>,
    pub c_req: Option<Secret<String>>,
    pub v2supported: Option<String>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
    pub version: Option<String>,
    pub result: Option<String>,
    pub is_liability_on_issuer: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPartialApproval {
    pub requested_amount: Option<StringMajorUnit>,
    pub requested_currency: Option<common_enums::Currency>,
    pub processed_amount: Option<StringMajorUnit>,
    pub processed_currency: Option<common_enums::Currency>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiPaymentStatus {
    Success,
    Failed,
    Error,
    #[default]
    Processing,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiTransactionStatus {
    #[serde(alias = "Approved", alias = "APPROVED")]
    Approved,
    #[serde(alias = "Declined", alias = "DECLINED")]
    Declined,
    #[serde(alias = "Filter Error", alias = "ERROR", alias = "Error")]
    Error,
    #[serde(alias = "Redirect", alias = "REDIRECT")]
    Redirect,
    #[serde(alias = "Pending", alias = "PENDING")]
    Pending,
    #[serde(alias = "Processing", alias = "PROCESSING")]
    #[default]
    Processing,
    #[serde(other)]
    Unknown,
}

/// transactionType echoed by Nuvei; drives the attempt-status mapping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NuveiTransactionType {
    Auth,
    Sale,
    Credit,
    Auth3D,
    InitAuth3D,
    Settle,
    Void,
    #[serde(other)]
    Unknown,
}

/// Single attempt-status mapping for Nuvei payment responses
/// (port of hyperswitch `get_payment_status`). Every arm is explicit.
pub(super) fn get_nuvei_payment_status(
    amount: Option<i64>,
    transaction_type: Option<&NuveiTransactionType>,
    transaction_status: Option<&NuveiTransactionStatus>,
    status: &NuveiPaymentStatus,
) -> common_enums::AttemptStatus {
    use common_enums::AttemptStatus;

    let from_request_status = |status: &NuveiPaymentStatus| match status {
        NuveiPaymentStatus::Failed | NuveiPaymentStatus::Error => AttemptStatus::Failure,
        NuveiPaymentStatus::Success
        | NuveiPaymentStatus::Processing
        | NuveiPaymentStatus::Unknown => AttemptStatus::Pending,
    };

    // Zero-amount authorization (card verification)
    if amount == Some(0) && transaction_type == Some(&NuveiTransactionType::Auth) {
        return match transaction_status {
            Some(NuveiTransactionStatus::Approved) => AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                AttemptStatus::AuthorizationFailed
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Unknown) => AttemptStatus::Pending,
            Some(NuveiTransactionStatus::Redirect) => AttemptStatus::AuthenticationPending,
            None => from_request_status(status),
        };
    }

    match transaction_status {
        Some(NuveiTransactionStatus::Approved) => match transaction_type {
            Some(NuveiTransactionType::InitAuth3D) | Some(NuveiTransactionType::Auth) => {
                AttemptStatus::Authorized
            }
            Some(NuveiTransactionType::Sale) | Some(NuveiTransactionType::Settle) => {
                AttemptStatus::Charged
            }
            Some(NuveiTransactionType::Void) => AttemptStatus::Voided,
            Some(NuveiTransactionType::Auth3D) => AttemptStatus::AuthenticationPending,
            Some(NuveiTransactionType::Credit) | Some(NuveiTransactionType::Unknown) | None => {
                AttemptStatus::Pending
            }
        },
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
            match transaction_type {
                Some(NuveiTransactionType::Auth) => AttemptStatus::AuthorizationFailed,
                Some(NuveiTransactionType::Void) => AttemptStatus::VoidFailed,
                Some(NuveiTransactionType::Auth3D) | Some(NuveiTransactionType::InitAuth3D) => {
                    AttemptStatus::AuthenticationFailed
                }
                Some(NuveiTransactionType::Sale)
                | Some(NuveiTransactionType::Settle)
                | Some(NuveiTransactionType::Credit)
                | Some(NuveiTransactionType::Unknown)
                | None => AttemptStatus::Failure,
            }
        }
        Some(NuveiTransactionStatus::Processing)
        | Some(NuveiTransactionStatus::Pending)
        | Some(NuveiTransactionStatus::Unknown) => AttemptStatus::Pending,
        Some(NuveiTransactionStatus::Redirect) => AttemptStatus::AuthenticationPending,
        None => from_request_status(status),
    }
}

/// Error mapping for a 2xx Nuvei payment response (port of hyperswitch
/// `build_error_response` / `get_error_response`). Returns `None` when the
/// response is not a failure.
pub(super) fn build_nuvei_error_response(
    response: &NuveiPaymentsResponse,
    http_code: u16,
    attempt_status: Option<FlowStatus>,
) -> Option<domain_types::router_data::ErrorResponse> {
    let (code, message) = match response.status {
        NuveiPaymentStatus::Error => (response.err_code, response.reason.clone()),
        NuveiPaymentStatus::Success
        | NuveiPaymentStatus::Failed
        | NuveiPaymentStatus::Processing
        | NuveiPaymentStatus::Unknown => {
            let is_failure = matches!(
                response.transaction_status,
                Some(NuveiTransactionStatus::Error) | Some(NuveiTransactionStatus::Declined)
            ) || response.gw_error_reason.as_deref() == Some("Missing argument");
            if !is_failure {
                return None;
            }
            (response.gw_error_code, response.gw_error_reason.clone())
        }
    };

    let message = message.unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
    let gw_reason =
        response
            .gw_error_reason
            .clone()
            .map(|reason| match response.gw_extended_error_code {
                Some(extended_code) => format!("{reason} (gwExtendedErrorCode {extended_code})"),
                None => reason,
            });
    let reason = response
        .payment_method_error_reason
        .clone()
        .or(gw_reason)
        .unwrap_or_else(|| message.clone());

    Some(domain_types::router_data::ErrorResponse {
        code: code
            .map(|code| code.to_string())
            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message,
        reason: Some(reason),
        status_code: http_code,
        attempt_status,
        connector_transaction_id: response.transaction_id.clone(),
        network_decline_code: response
            .issuer_decline_code
            .clone()
            .or_else(|| response.gw_error_code.map(|code| code.to_string())),
        network_advice_code: response.merchant_advice_code.clone(),
        network_error_message: response
            .issuer_decline_reason
            .clone()
            .or_else(|| response.gw_error_reason.clone()),
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    })
}

// The maximum length of the clientUniqueId field in the Nuvei API request.
const MAX_CLIENT_UNIQUE_ID_LENGTH: usize = 45;
// The maximum length of dynamicDescriptor.merchantPhone.
const MAX_DESCRIPTOR_PHONE_LENGTH: usize = 13;
// dynamicDescriptor.merchantName is truncated to this many characters.
const MAX_DESCRIPTOR_NAME_LENGTH: usize = 25;
const NUVEI_API_DOC_URL: &str = "https://docs.nuvei.com/api/main/indexMain_v1_0.html";

/// Non-default error context for Nuvei request-side refusals.
fn nuvei_error_context(
    suggested_action: &str,
    additional_context: impl Into<String>,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: Some(NUVEI_API_DOC_URL.to_string()),
        additional_context: Some(additional_context.into()),
    }
}

fn nuvei_missing_field(field_name: &'static str, suggested_action: &str) -> IntegrationError {
    IntegrationError::MissingRequiredField {
        field_name,
        context: nuvei_error_context(
            suggested_action,
            format!("Nuvei requires {field_name} on this request"),
        ),
    }
}

/// clientUniqueId is String(45) on the Nuvei API; refuse longer references
/// before any request is built (port of hyperswitch `get_valid_client_unique_id`).
pub(super) fn get_valid_client_unique_id(id: &str) -> Result<String, Report<IntegrationError>> {
    if id.len() <= MAX_CLIENT_UNIQUE_ID_LENGTH {
        Ok(id.to_string())
    } else {
        Err(IntegrationError::MaxFieldLengthViolated {
            connector: "Nuvei".to_string(),
            field_name: "client_unique_id".to_string(),
            max_length: MAX_CLIENT_UNIQUE_ID_LENGTH,
            received_length: id.len(),
            context: nuvei_error_context(
                "Send a merchant reference (connector_request_reference_id) of at most 45 characters",
                "Nuvei clientUniqueId is limited to 45 characters",
            ),
        }
        .into())
    }
}

/// The 3DS leg that wrote a NuveiMeta. The final Authorize keys the 3DS
/// completion behaviour on `Authenticate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NuveiThreeDsLeg {
    PreAuthenticate,
    Authenticate,
}

/// Stored-credential reference returned by a charging 3DS Authenticate leg
/// (userPaymentOptionId, with the CIT browser IP as mandate metadata).
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuveiMetaMandateReference {
    pub connector_mandate_id: Secret<String>,
    #[serde(default)]
    pub mandate_metadata: Option<pii::SecretSerdeValue>,
}

/// Connector state carried between Nuvei calls in connector_feature_data:
/// the order-bound sessionToken and, after a 3DS authentication leg, the
/// transaction the final payment must reference. The CIT markers are written
/// by a caller that cannot send them as request fields; the mandate reference
/// and transaction link id are written when the Authenticate leg charges.
/// Every field but session_token is optional, so older NuveiMeta JSON parses.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NuveiMeta {
    pub session_token: Secret<String>,
    #[serde(default)]
    pub related_transaction_id: Option<String>,
    #[serde(default)]
    pub three_ds_leg: Option<NuveiThreeDsLeg>,
    /// initPayment `paymentOption.card.threeD.v2supported` (PreAuthenticate leg).
    #[serde(default)]
    pub v2supported: Option<String>,
    #[serde(default)]
    pub is_customer_initiated_mandate_payment: Option<bool>,
    #[serde(default)]
    pub customer_id: Option<Secret<String>>,
    #[serde(default)]
    pub mandate_reference: Option<NuveiMetaMandateReference>,
    #[serde(default)]
    pub network_txn_link_id: Option<String>,
}

pub(super) fn parse_nuvei_meta(
    connector_feature_data: Option<&pii::SecretSerdeValue>,
) -> Option<NuveiMeta> {
    connector_feature_data
        .and_then(|data| serde_json::from_value::<NuveiMeta>(data.peek().clone()).ok())
        .filter(|meta| !meta.session_token.peek().is_empty())
}

/// Session token precedence: NuveiMeta (connector_feature_data), then the
/// request's session_token, then state.access_token (RepeatPayment backward
/// compatibility). Empty values count as absent.
pub(super) fn resolve_session_token(
    meta: Option<&NuveiMeta>,
    session_token: Option<&String>,
    access_token: Option<&domain_types::connector_types::ServerAuthenticationTokenResponseData>,
) -> Result<Secret<String>, Report<IntegrationError>> {
    meta.map(|meta| meta.session_token.peek().clone())
        .or_else(|| session_token.cloned())
        .filter(|token| !token.is_empty())
        .or_else(|| {
            access_token
                .map(|token| token.access_token.peek().to_string())
                .filter(|token| !token.is_empty())
        })
        .map(Secret::new)
        .ok_or_else(|| {
            nuvei_missing_field(
                "session_token",
                "Call MerchantAuthenticationService/CreateServerSessionAuthenticationToken first and pass its session_token",
            )
            .into()
        })
}

// ---- Shared request-side helpers ----

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDynamicDescriptor {
    pub merchant_name: Option<Secret<String>>,
    pub merchant_phone: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub enum NuveiPartialApprovalFlag {
    #[serde(rename = "1")]
    Enabled,
    #[serde(rename = "0")]
    Disabled,
}

#[derive(Debug, Clone, Serialize)]
pub enum NuveiIsRebilling {
    #[serde(rename = "1")]
    True,
    #[serde(rename = "0")]
    False,
}

#[derive(Debug, Clone, Serialize)]
pub enum NuveiItemType {
    #[serde(rename = "physical")]
    Physical,
    #[serde(rename = "digital")]
    Digital,
    #[serde(rename = "Shipping_fee")]
    ShippingFee,
}

impl From<Option<&common_enums::ProductType>> for NuveiItemType {
    fn from(value: Option<&common_enums::ProductType>) -> Self {
        match value {
            Some(common_enums::ProductType::Digital) => Self::Digital,
            Some(common_enums::ProductType::Ride)
            | Some(common_enums::ProductType::Travel)
            | Some(common_enums::ProductType::Accommodation) => Self::ShippingFee,
            Some(common_enums::ProductType::Physical)
            | Some(common_enums::ProductType::Event)
            | None => Self::Physical,
        }
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiItem {
    pub name: String,
    #[serde(rename = "type")]
    pub item_type: NuveiItemType,
    pub price: StringMajorUnit,
    pub quantity: String,
    pub group_id: Option<String>,
    pub discount: Option<StringMajorUnit>,
    pub tax: Option<StringMajorUnit>,
    pub tax_rate: Option<String>,
    pub image_url: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiAmountDetails {
    pub total_tax: Option<StringMajorUnit>,
    pub total_shipping: Option<StringMajorUnit>,
    pub total_handling: Option<StringMajorUnit>,
    pub total_discount: Option<StringMajorUnit>,
}

type NuveiAmountConverter =
    dyn common_utils::types::AmountConvertor<Output = StringMajorUnit> + Sync;

fn convert_nuvei_amount(
    converter: &NuveiAmountConverter,
    amount: common_utils::types::MinorUnit,
    currency: common_enums::Currency,
) -> Result<StringMajorUnit, Report<IntegrationError>> {
    converter
        .convert(amount, currency)
        .change_context(IntegrationError::AmountConversionFailed {
            context: nuvei_error_context(
                "Send amounts in minor units valid for the currency",
                format!("failed to convert {amount:?} {currency} to Nuvei major units"),
            ),
        })
}

/// dynamicDescriptor from billing_descriptor: name trimmed to 25 characters,
/// phone refused above 13 characters (hyperswitch `get_dynamic_descriptor`).
pub(super) fn get_dynamic_descriptor(
    billing_descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
) -> Result<Option<NuveiDynamicDescriptor>, Report<IntegrationError>> {
    let Some(descriptor) = billing_descriptor else {
        return Ok(None);
    };
    if let Some(phone) = descriptor.phone.as_ref() {
        let received_length = phone.peek().len();
        if received_length > MAX_DESCRIPTOR_PHONE_LENGTH {
            return Err(IntegrationError::MaxFieldLengthViolated {
                connector: "Nuvei".to_string(),
                field_name: "dynamic_descriptor.merchant_phone".to_string(),
                max_length: MAX_DESCRIPTOR_PHONE_LENGTH,
                received_length,
                context: nuvei_error_context(
                    "Send billing_descriptor.phone of at most 13 characters",
                    "Nuvei dynamicDescriptor.merchantPhone is limited to 13 characters",
                ),
            }
            .into());
        }
    }
    Ok(Some(NuveiDynamicDescriptor {
        merchant_name: descriptor.name.as_ref().map(|name| {
            Secret::new(
                name.peek()
                    .trim()
                    .chars()
                    .take(MAX_DESCRIPTOR_NAME_LENGTH)
                    .collect(),
            )
        }),
        merchant_phone: descriptor.phone.clone(),
    }))
}

pub(super) fn get_partial_approval_flag(
    enable_partial_authorization: Option<bool>,
) -> Option<NuveiPartialApprovalFlag> {
    enable_partial_authorization.map(|enabled| {
        if enabled {
            NuveiPartialApprovalFlag::Enabled
        } else {
            NuveiPartialApprovalFlag::Disabled
        }
    })
}

/// isMoto for mail-order / telephone-order payments.
pub(super) fn get_is_moto(payment_channel: Option<&common_enums::PaymentChannel>) -> Option<bool> {
    match payment_channel {
        Some(common_enums::PaymentChannel::MailOrder)
        | Some(common_enums::PaymentChannel::TelephoneOrder) => Some(true),
        Some(common_enums::PaymentChannel::Ecommerce) | None => None,
    }
}

/// items[] from l2_l3_data.order_info.order_details (hyperswitch `get_l2_l3_items`).
pub(super) fn get_l2_l3_items(
    l2_l3_data: Option<&domain_types::connector_types::L2L3Data>,
    currency: common_enums::Currency,
    converter: &NuveiAmountConverter,
) -> Result<Option<Vec<NuveiItem>>, Report<IntegrationError>> {
    l2_l3_data
        .and_then(|data| data.get_order_details())
        .map(|order_details| {
            order_details
                .iter()
                .map(|order| {
                    Ok(NuveiItem {
                        name: order.product_name.clone(),
                        item_type: NuveiItemType::from(order.product_type.as_ref()),
                        price: convert_nuvei_amount(converter, order.amount, currency)?,
                        quantity: order.quantity.to_string(),
                        group_id: order.product_id.clone(),
                        discount: order
                            .unit_discount_amount
                            .map(|amount| convert_nuvei_amount(converter, amount, currency))
                            .transpose()?,
                        tax: order
                            .total_tax_amount
                            .map(|amount| convert_nuvei_amount(converter, amount, currency))
                            .transpose()?,
                        tax_rate: order.tax_rate.map(|rate| rate.to_string()),
                        image_url: order.product_img_link.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Report<IntegrationError>>>()
        })
        .transpose()
}

/// amountDetails from l2_l3_data (hyperswitch `get_amount_details`).
pub(super) fn get_amount_details(
    l2_l3_data: Option<&domain_types::connector_types::L2L3Data>,
    currency: common_enums::Currency,
    converter: &NuveiAmountConverter,
) -> Result<Option<NuveiAmountDetails>, Report<IntegrationError>> {
    let convert = |amount| convert_nuvei_amount(converter, amount, currency);
    l2_l3_data
        .map(|data| {
            Ok(NuveiAmountDetails {
                total_tax: data.get_order_tax_amount().map(convert).transpose()?,
                total_shipping: data.get_shipping_cost().map(convert).transpose()?,
                total_handling: data.get_duty_amount().map(convert).transpose()?,
                total_discount: data.get_discount_amount().map(convert).transpose()?,
            })
        })
        .transpose()
}

// ---- Shared response-side helpers ----

fn get_cvv2_response_description(code: &str) -> Option<&'static str> {
    match code {
        "M" => Some("CVV2 Match"),
        "N" => Some("CVV2 No Match"),
        "P" => Some("Not Processed. For EU card-on-file (COF) and ecommerce (ECOM) network token transactions, Visa removes any CVV and sends P. If you have fraud or security concerns, Visa recommends using 3DS."),
        "U" => Some("Issuer is not certified and/or has not provided Visa the encryption keys"),
        "S" => Some("CVV2 processor is unavailable."),
        _ => None,
    }
}

fn get_avs_response_description(code: &str) -> Option<&'static str> {
    match code {
        "A" => Some("The street address matches, the ZIP code does not."),
        "W" => Some("Postal code matches, the street address does not."),
        "Y" => Some("Postal code and the street address match."),
        "X" => Some("An exact match of both the 9-digit ZIP code and the street address."),
        "Z" => Some("Postal code matches, the street code does not."),
        "U" => Some("Issuer is unavailable."),
        "S" => Some("AVS not supported by issuer."),
        "R" => Some("Retry."),
        "B" => Some("Not authorized (declined)."),
        "N" => Some("Both the street address and postal code do not match."),
        _ => None,
    }
}

/// AVS / CVV2 / card brand / 3DS results from paymentOption.card
/// (hyperswitch `convert_to_additional_payment_method_connector_response`).
pub(super) fn build_nuvei_connector_response(
    payment_option: Option<&NuveiResponsePaymentOption>,
) -> Option<ConnectorResponseData> {
    let card = payment_option?.card.as_ref()?;
    let avs_code = card.avs_code.as_deref();
    let cvv2_code = card.cvv2_reply.as_deref();
    let payment_checks = serde_json::json!({
        "avs_result": avs_code,
        "avs_description": avs_code.and_then(get_avs_response_description),
        "card_validation_result": cvv2_code,
        "card_validation_description": cvv2_code.and_then(get_cvv2_response_description),
    });
    let authentication_data = match card.three_d.as_ref().map(serde_json::to_value).transpose() {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(?error, "failed to encode Nuvei threeD response block");
            return None;
        }
    };
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data,
            payment_checks: Some(payment_checks),
            card_network: card.brand.clone(),
            domestic_network: None,
            auth_code: None,
        },
    ))
}

/// Port of hyperswitch `create_transaction_response`.
pub(super) fn build_nuvei_transaction_response(
    response: &NuveiPaymentsResponse,
    payment_method: common_enums::PaymentMethod,
    ip_address: Option<String>,
    http_code: u16,
) -> Result<PaymentsResponseData, Report<ConnectorError>> {
    let resource_id = response
        .transaction_id
        .clone()
        .or_else(|| response.order_id.clone())
        .map(ResponseId::ConnectorTransactionId)
        .ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing transactionId and orderId in Nuvei payment response".to_string()),
            ))
        })?;

    let redirection_data = match payment_method {
        common_enums::PaymentMethod::Wallet | common_enums::PaymentMethod::BankRedirect => response
            .payment_option
            .as_ref()
            .and_then(|option| option.redirect_url.clone())
            .and_then(|url| Url::parse(&url).ok())
            .map(|url| RedirectForm::from((url, Method::Get))),
        _ => response
            .payment_option
            .as_ref()
            .and_then(|option| option.card.as_ref())
            .and_then(|card| card.three_d.as_ref())
            .and_then(|three_d| three_d.acs_url.clone().zip(three_d.c_req.clone()))
            .map(|(endpoint, creq)| RedirectForm::Form {
                endpoint,
                method: Method::Post,
                form_fields: std::collections::HashMap::from([(
                    "creq".to_string(),
                    creq.peek().clone(),
                )]),
            }),
    };

    let mandate_reference = response
        .payment_option
        .as_ref()
        .and_then(|option| option.user_payment_option_id.clone())
        .map(|id| id.expose())
        .filter(|id| !id.is_empty())
        .map(|id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(id),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: ip_address
                    .map(|ip| pii::SecretSerdeValue::new(serde_json::Value::String(ip))),
            })
        });

    let connector_metadata = response
        .session_token
        .clone()
        .filter(|token| !token.peek().is_empty())
        .map(|session_token| {
            serde_json::to_value(NuveiMeta {
                session_token,
                related_transaction_id: None,
                three_ds_leg: None,
                v2supported: None,
                is_customer_initiated_mandate_payment: None,
                customer_id: None,
                mandate_reference: None,
                network_txn_link_id: None,
            })
        })
        .transpose()
        .map_err(|error| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some(format!(
                    "failed to encode Nuvei session token metadata: {error}"
                )),
            ))
        })?;

    Ok(PaymentsResponseData::TransactionResponse {
        resource_id,
        redirection_data: redirection_data.map(Box::new),
        mandate_reference,
        connector_metadata,
        network_txn_id: response
            .external_scheme_transaction_id
            .as_ref()
            .map(|ntid| ntid.peek().clone())
            .filter(|ntid| !ntid.is_empty()),
        network_txn_link_id: response.transaction_link_id.clone(),
        connector_response_reference_id: response.order_id.clone(),
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: None,
    })
}

/// Partial approval (hyperswitch `get_amount_captured`): the processed amount
/// is the captured amount for Sale/Auth3D and the capturable amount for
/// Auth/InitAuth3D. Returns (amount_captured, amount_capturable).
pub(super) fn get_amount_captured(
    response: &NuveiPaymentsResponse,
    converter: &NuveiAmountConverter,
    http_code: u16,
) -> Result<
    (
        Option<common_utils::types::MinorUnit>,
        Option<common_utils::types::MinorUnit>,
    ),
    Report<ConnectorError>,
> {
    let Some((processed_amount, processed_currency)) =
        response.partial_approval.as_ref().and_then(|approval| {
            approval
                .processed_amount
                .clone()
                .zip(approval.processed_currency)
        })
    else {
        return Ok((None, None));
    };
    let amount = converter
        .convert_back(processed_amount, processed_currency)
        .map_err(|error| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some(format!(
                    "failed to convert Nuvei partialApproval.processedAmount: {error:?}"
                )),
            ))
        })?;
    Ok(match response.transaction_type {
        Some(NuveiTransactionType::Sale) | Some(NuveiTransactionType::Auth3D) => {
            (Some(amount), None)
        }
        Some(NuveiTransactionType::Auth) | Some(NuveiTransactionType::InitAuth3D) => {
            (None, Some(amount))
        }
        Some(NuveiTransactionType::Credit)
        | Some(NuveiTransactionType::Void)
        | Some(NuveiTransactionType::Settle)
        | Some(NuveiTransactionType::Unknown)
        | None => (None, None),
    })
}

/// Applies a partialApproval to the flow data (hyperswitch runs
/// `get_amount_captured` on every payment response): processedAmount becomes
/// amount_captured / minor_amount_captured (Sale, Auth3D) or
/// minor_amount_capturable (Auth, InitAuth3D). Without a partialApproval the
/// existing values are kept.
pub(super) fn apply_nuvei_partial_approval(
    response: &NuveiPaymentsResponse,
    resource_common_data: PaymentFlowData,
    http_code: u16,
) -> Result<PaymentFlowData, Report<ConnectorError>> {
    let (amount_captured, amount_capturable) = get_amount_captured(
        response,
        &common_utils::types::StringMajorUnitForConnector,
        http_code,
    )?;
    Ok(PaymentFlowData {
        amount_captured: amount_captured
            .map(|amount| amount.get_amount_as_i64())
            .or(resource_common_data.amount_captured),
        minor_amount_captured: amount_captured.or(resource_common_data.minor_amount_captured),
        minor_amount_capturable: amount_capturable.or(resource_common_data.minor_amount_capturable),
        ..resource_common_data
    })
}

// Transaction Type for initPayment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TransactionType {
    Auth,
    #[default]
    Sale,
}

impl TransactionType {
    /// Auth for manual capture or a zero amount, else Sale. Decided on the
    /// minor-unit amount before conversion (no float parsing of the wire string).
    /// ManualMultiple and Scheduled are refused before any request is built
    /// (hyperswitch Nuvei supported_capture_methods: Automatic, Manual,
    /// SequentialAutomatic); otherwise they would silently become a Sale.
    fn get_from_capture_method(
        capture_method: Option<common_enums::CaptureMethod>,
        minor_amount: common_utils::types::MinorUnit,
    ) -> Result<Self, Report<IntegrationError>> {
        match capture_method {
            Some(
                method @ (common_enums::CaptureMethod::ManualMultiple
                | common_enums::CaptureMethod::Scheduled),
            ) => Err(IntegrationError::NotSupported {
                message: format!("capture_method {method}"),
                connector: "nuvei",
                context: nuvei_error_context(
                    "Use capture_method AUTOMATIC, MANUAL or SEQUENTIAL_AUTOMATIC",
                    "Nuvei supports one Sale or one Auth followed by settleTransaction; multiple or scheduled captures are not offered",
                ),
            }
            .into()),
            Some(common_enums::CaptureMethod::Manual) => Ok(Self::Auth),
            _ if minor_amount == common_utils::types::MinorUnit::zero() => Ok(Self::Auth),
            _ => Ok(Self::Sale),
        }
    }
}

// Sync Request (/getTransactionDetails.do). Looked up by transactionId only:
// clientUniqueId is deliberately not sent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

/// getTransactionDetails response (hyperswitch `NuveiTransactionSyncResponse`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncResponse {
    pub payment_option: Option<NuveiResponsePaymentOption>,
    pub partial_approval: Option<NuveiSyncPartialApproval>,
    pub transaction_details: Option<NuveiTransactionDetails>,
    pub transaction_link_id: Option<String>,
    pub client_unique_id: Option<String>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub client_request_id: Option<String>,
    pub merchant_advice_code: Option<String>,
}

/// partialApproval block of getTransactionDetails: only the requested side;
/// the processed side lives in transactionDetails.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncPartialApproval {
    pub requested_amount: Option<StringMajorUnit>,
    pub requested_currency: Option<common_enums::Currency>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiTransactionDetails {
    pub gw_error_code: Option<i64>,
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub auth_code: Option<String>,
    pub processed_amount: Option<StringMajorUnit>,
    pub processed_currency: Option<common_enums::Currency>,
    pub acquiring_bank_name: Option<String>,
}

/// errCode 9146 = "No transaction details returned for the provided ID":
/// a sync issued before Nuvei indexed the transaction. Not an error; the
/// attempt status is left unchanged (hyperswitch
/// `bypass_error_for_no_payments_found`).
const NUVEI_NO_TRANSACTION_FOUND_ERR_CODE: i64 = 9146;

impl NuveiSyncResponse {
    /// Partial approval: requested side from partialApproval, processed side
    /// from transactionDetails (hyperswitch `get_partial_approval`).
    fn get_partial_approval(&self) -> Option<NuveiPartialApproval> {
        let approval = self.partial_approval.as_ref()?;
        let details = self.transaction_details.as_ref()?;
        match (
            approval.requested_amount.clone(),
            approval.requested_currency,
            details.processed_amount.clone(),
            details.processed_currency,
        ) {
            (
                Some(requested_amount),
                Some(requested_currency),
                Some(processed_amount),
                Some(processed_currency),
            ) => Some(NuveiPartialApproval {
                requested_amount: Some(requested_amount),
                requested_currency: Some(requested_currency),
                processed_amount: Some(processed_amount),
                processed_currency: Some(processed_currency),
            }),
            _ => None,
        }
    }

    /// Flatten into the shared payment-response shape so the foundation
    /// helpers (status, error, transaction response, partial approval,
    /// connector response) apply unchanged (hyperswitch
    /// `NuveiPaymentResponseData::new_from_sync_response`). The sync call
    /// carries no order id, session token, NTID or issuer decline fields,
    /// and merchant_advice_code is not used on the sync error path.
    fn to_payments_response(&self) -> NuveiPaymentsResponse {
        let details = self.transaction_details.as_ref();
        NuveiPaymentsResponse {
            order_id: None,
            user_token_id: None,
            payment_option: self.payment_option.clone(),
            transaction_status: details.and_then(|d| d.transaction_status.clone()),
            transaction_type: details.and_then(|d| d.transaction_type.clone()),
            transaction_id: details.and_then(|d| d.transaction_id.clone()),
            auth_code: details.and_then(|d| d.auth_code.clone()),
            gw_error_code: details.and_then(|d| d.gw_error_code),
            gw_error_reason: details.and_then(|d| d.gw_error_reason.clone()),
            gw_extended_error_code: details.and_then(|d| d.gw_extended_error_code),
            issuer_decline_code: None,
            issuer_decline_reason: None,
            payment_method_error_code: None,
            payment_method_error_reason: None,
            merchant_advice_code: None,
            external_scheme_transaction_id: None,
            transaction_link_id: self.transaction_link_id.clone(),
            session_token: None,
            partial_approval: self.get_partial_approval(),
            client_unique_id: self.client_unique_id.clone(),
            client_request_id: self.client_request_id.clone(),
            internal_request_id: None,
            status: self.status.clone(),
            err_code: self.err_code,
            reason: self.reason.clone(),
            merchant_id: self.merchant_id.clone(),
            merchant_site_id: self.merchant_site_id.clone(),
        }
    }
}

// Capture Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Refund Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Refund Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

/// RSync (/getTransactionDetails.do) response body: the same
/// getTransactionDetails shape PSync parses, under a flow-specific alias.
pub type NuveiRefundSyncResponse = NuveiSyncResponse;

// Void Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Error Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiErrorResponse {
    pub reason: Option<String>,
    // Nuvei sends errCode as an integer; accept a string too.
    #[serde(default, deserialize_with = "str_or_i64")]
    pub err_code: Option<String>,
    pub status: Option<String>,
}

// Session Token Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
                domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for NuveiSessionTokenRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                domain_types::connector_flow::ServerSessionAuthenticationToken,
                MerchantAuthenticationFlowData,
                domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
                domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Generate checksum for getSessionToken: merchantId + merchantSiteId + clientRequestId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            time_stamp,
            checksum,
        })
    }
}

// Session Token Response Transformation
impl TryFrom<ResponseRouterData<NuveiSessionTokenResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::ServerSessionAuthenticationToken,
        MerchantAuthenticationFlowData,
        domain_types::connector_types::ServerSessionAuthenticationTokenRequestData,
        domain_types::connector_types::ServerSessionAuthenticationTokenResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiSessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check if the overall request status is SUCCESS or ERROR. No payment
        // attempt exists yet, so the error carries no attempt status.
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response
                .err_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract session token
        let session_token = response
            .session_token
            .as_ref()
            .map(|token| token.peek().clone())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("session_token missing in Nuvei response".to_string()),
                ))
            })?;

        let session_response_data =
            domain_types::connector_types::ServerSessionAuthenticationTokenResponseData {
                session_token: session_token.clone(),
            };

        Ok(Self {
            response: Ok(session_response_data),
            ..router_data.clone()
        })
    }
}

// Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for NuveiSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();

        // getTransactionDetails needs the Nuvei transactionId.
        // clientUniqueId is not sent: the sync request's own
        // reference never matches the payment's and yields errCode 9146.
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: nuvei_error_context(
                        "Pass the Nuvei transactionId returned by Authorize as connector_transaction_id",
                        "Nuvei getTransactionDetails looks the payment up by transactionId",
                    ),
                }
                .into());
            }
        };

        // checksum = sha256(merchantId + merchantSiteId + transactionId + timeStamp + merchantSecretKey)
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Authorize request refusal for a payment-method arm this integration does not offer.
fn nuvei_authorize_not_implemented(what: &str, suggested_action: &str) -> IntegrationError {
    IntegrationError::NotImplemented(
        format!("{what} is not implemented for Nuvei Authorize"),
        nuvei_error_context(
            suggested_action,
            format!(
                "This UCS Nuvei integration does not build a /payment.do request for {what} yet"
            ),
        ),
    )
}

fn nuvei_authorize_not_supported(what: String) -> IntegrationError {
    IntegrationError::NotSupported {
        message: format!("{what} is not supported by Nuvei"),
        connector: "nuvei",
        context: nuvei_error_context(
            "Use a payment method Nuvei supports (card, network token, iDEAL, Sofort, EPS, Giropay, ACH)",
            format!("Nuvei has no /payment.do payment option for {what}"),
        ),
    }
}

// Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let resource = &router_data.resource_common_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // Billing email (else request email), needed up-front because ACH arms
        // send it as userTokenId. Missing -> refused with the billing address below.
        let email = resource
            .get_optional_billing_email()
            .or_else(|| request.email.clone());

        // CIT (stores the card for future MIT): isRebilling "0" + userTokenId.
        let is_cit = request.is_customer_initiated_mandate_payment();
        let cit_user_token_id = request
            .customer_id
            .as_ref()
            .or(resource.customer_id.as_ref())
            .map(|customer_id| Secret::new(customer_id.get_string_repr().to_string()));

        let mut user_token_id: Option<Secret<String>> = None;
        let mut is_rebilling: Option<NuveiIsRebilling> = None;

        // Connector state from earlier calls (session token; after the 3DS
        // Authenticate leg also the Auth3D transaction to reference).
        let nuvei_meta = parse_nuvei_meta(
            resource
                .connector_feature_data
                .as_ref()
                .or(request.connector_feature_data.as_ref()),
        );
        let is_three_ds_final_leg = nuvei_meta
            .as_ref()
            .and_then(|meta| meta.three_ds_leg.as_ref())
            == Some(&NuveiThreeDsLeg::Authenticate);

        // Payment-method dispatch
        let payment_option = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = get_nuvei_card_holder_name(card_data, resource);

                if is_three_ds_final_leg {
                    // Final 3DS payment after the ACS challenge (G-Authorize-12):
                    // the challenge result must be present and parseable, else
                    // refuse before any HTTP call. A frictionless Authenticate
                    // that returned CHARGED/AUTHORIZED already charged and has
                    // no final leg; an abandoned or malformed challenge must not
                    // charge the Auth3D.
                    let redirect_response = request
                        .redirect_response
                        .as_ref()
                        .filter(|response| get_nuvei_redirection_response(response).is_some())
                        .ok_or_else(|| {
                            nuvei_missing_field(
                                "redirection_response",
                                "Call Authorize after an Authenticate challenge only, passing the ACS return (cres or error) unchanged in redirection_response; a CHARGED or AUTHORIZED Authenticate response is already final",
                            )
                        })?;
                    validate_nuvei_challenge_result(Some(redirect_response))?;
                } else if resource.auth_type == common_enums::AuthenticationType::ThreeDs
                    && request.authentication_data.is_none()
                {
                    // Never charge a 3DS-requested card payment unless the
                    // Authenticate leg ran (NuveiMeta three_ds_leg authenticate) or
                    // external 3DS data is sent: a session-token-only or
                    // PreAuthenticate NuveiMeta does not prove authentication.
                    return Err(IntegrationError::NotSupported {
                        message: "3DS card Authorize without PreAuthenticate/Authenticate"
                            .to_string(),
                        connector: "nuvei",
                        context: nuvei_error_context(
                            "Run PaymentMethodAuthenticationService/PreAuthenticate and Authenticate first. If Authenticate returned a challenge, pass its connector_feature_data and the ACS return (redirection_response) to Authorize; if it returned CHARGED or AUTHORIZED the payment is complete. Or send external authentication_data",
                            "Nuvei 3DS runs initPayment and payment-with-threeD before the final payment",
                        ),
                    }
                    .into());
                }

                // External 3DS (merchant-performed authentication): forward the
                // MPI result as threeD.externalMpi. cavv is mandatory there. The
                // final leg of a Nuvei-run 3DS payment carries no threeD block.
                let three_d = request
                    .authentication_data
                    .as_ref()
                    .filter(|_| !is_three_ds_final_leg)
                    .map(|auth_data| {
                        let cavv = auth_data.cavv.clone().ok_or_else(|| {
                            nuvei_missing_field(
                                "authentication_data.cavv",
                                "Send authentication_data.cavv with external 3DS data",
                            )
                        })?;
                        Ok::<_, Report<IntegrationError>>(Box::new(NuveiCardThreeD {
                            external_mpi: Some(NuveiExternalMpi {
                                eci: auth_data.eci.clone(),
                                cavv,
                                ds_trans_id: auth_data.ds_trans_id.clone(),
                            }),
                            ..Default::default()
                        }))
                    })
                    .transpose()?;

                if is_cit {
                    is_rebilling = Some(NuveiIsRebilling::False);
                    user_token_id = cit_user_token_id.clone();
                }

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            // Network-token CIT: expiry + externalToken only, no PAN/CVV/holder
            // name (a token payment must not fail on missing billing name)
            PaymentMethodData::NetworkToken(token_data) => NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::NetworkToken(
                    build_nuvei_network_token_card(token_data),
                )),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            PaymentMethodData::BankDebit(bank_debit_data) => {
                match bank_debit_data {
                    BankDebitData::AchBankDebit {
                        account_number,
                        routing_number,
                        bank_account_holder_name: _,
                        bank_holder_type,
                        ..
                    } => {
                        // SEC (Standard Entry Class) code: CCD for Business,
                        // WEB for Personal/consumer-initiated entries.
                        let sec_code = Some(
                            match bank_holder_type {
                                Some(common_enums::BankHolderType::Business) => "CCD",
                                Some(common_enums::BankHolderType::Personal) | None => "WEB",
                            }
                            .to_string(),
                        );

                        // Nuvei requires userTokenId for ACH flows.
                        user_token_id = email
                            .as_ref()
                            .map(|email| Secret::new(email.peek().to_string()));

                        NuveiPaymentOption {
                            card: None,
                            alternative_payment_method: Some(NuveiAlternativePaymentMethod::Ach {
                                payment_method: NUVEI_ACH_PAYMENT_METHOD.to_string(),
                                account_number: Secret::new(account_number.peek().to_string()),
                                routing_number: Secret::new(routing_number.peek().to_string()),
                                sec_code,
                            }),
                            user_payment_option_id: None,
                        }
                    }
                    other => {
                        return Err(
                            nuvei_authorize_not_supported(format!("bank debit {other:?}")).into(),
                        )
                    }
                }
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => {
                match bank_transfer_data.as_ref() {
                    BankTransferData::AchBankTransfer {} => {
                        // For ACH Bank Transfer, Nuvei requires account_number and routing_number
                        // These should be provided in the request metadata as ACH details
                        let metadata = request.metadata.as_ref().ok_or_else(|| {
                            nuvei_missing_field(
                                "metadata for ACH details",
                                "Send metadata {\"ach\": {\"account_number\", \"routing_number\"}}",
                            )
                        })?;

                        let ach_data = metadata.peek().get("ach").ok_or_else(|| {
                            nuvei_missing_field(
                                "ach in metadata",
                                "Send metadata {\"ach\": {\"account_number\", \"routing_number\"}}",
                            )
                        })?;

                        let account_number = ach_data
                            .get("account_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or_else(|| {
                                nuvei_missing_field(
                                    "account_number",
                                    "Send metadata.ach.account_number",
                                )
                            })?;

                        let routing_number = ach_data
                            .get("routing_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or_else(|| {
                                nuvei_missing_field(
                                    "routing_number",
                                    "Send metadata.ach.routing_number",
                                )
                            })?;

                        let sec_code = ach_data
                            .get("sec_code")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(String::from);

                        // Nuvei requires userTokenId for ACH flows.
                        user_token_id = email
                            .as_ref()
                            .map(|email| Secret::new(email.peek().to_string()));

                        NuveiPaymentOption {
                            card: None,
                            alternative_payment_method: Some(NuveiAlternativePaymentMethod::Ach {
                                payment_method: NUVEI_ACH_PAYMENT_METHOD.to_string(),
                                account_number: Secret::new(account_number.to_string()),
                                routing_number: Secret::new(routing_number.to_string()),
                                sec_code,
                            }),
                            user_payment_option_id: None,
                        }
                    }
                    other => {
                        return Err(nuvei_authorize_not_supported(format!(
                            "bank transfer {other:?}"
                        ))
                        .into())
                    }
                }
            }
            PaymentMethodData::BankRedirect(ref redirect_data) => {
                let payment_method = match redirect_data {
                    BankRedirectData::Eps { .. } => AlternativePaymentMethodType::Eps,
                    BankRedirectData::Giropay { .. } => AlternativePaymentMethodType::Giropay,
                    BankRedirectData::Ideal { bank_name } => {
                        if let Some(ref bank) = bank_name {
                            let _ = NuveiBIC::try_from(*bank)?;
                        }
                        AlternativePaymentMethodType::Ideal
                    }
                    BankRedirectData::Sofort { .. } => AlternativePaymentMethodType::Sofort,
                    other => {
                        return Err(nuvei_authorize_not_supported(format!(
                            "bank redirect {other:?}"
                        ))
                        .into())
                    }
                };

                // Bank-redirect APMs always redirect the customer: without
                // urlDetails Nuvei has nowhere to send them back.
                if request.router_return_url.is_none() {
                    return Err(nuvei_missing_field(
                        "return_url",
                        "Send return_url for Nuvei bank-redirect payments (EPS, Giropay, iDEAL, Sofort)",
                    )
                    .into());
                }

                let bank_id = match redirect_data {
                    BankRedirectData::Ideal { bank_name } => bank_name
                        .as_ref()
                        .map(|bank| NuveiBIC::try_from(*bank))
                        .transpose()?,
                    _ => None,
                };

                NuveiPaymentOption {
                    card: None,
                    alternative_payment_method: Some(NuveiAlternativePaymentMethod::Redirect {
                        payment_method,
                        bank_id,
                    }),
                    user_payment_option_id: None,
                }
            }
            PaymentMethodData::PaymentMethodToken(token_data) => {
                // Stored-token payments never run PreAuthenticate/Authenticate
                // (next_authentication_step is card-only): refuse a 3DS request
                // instead of charging it unauthenticated (G-Authorize-11).
                if resource.auth_type == common_enums::AuthenticationType::ThreeDs
                    && request.authentication_data.is_none()
                {
                    return Err(IntegrationError::NotSupported {
                        message: "3DS Authorize with a stored payment token".to_string(),
                        connector: "nuvei",
                        context: nuvei_error_context(
                            "Send auth_type NO_THREE_DS for a stored-token payment, or pay with the card through PreAuthenticate/Authenticate",
                            "Nuvei 3DS in this integration runs only for raw card payments",
                        ),
                    }
                    .into());
                }
                // A stored payment option (UPO) is bound to the userTokenId it
                // was created under: send it alongside userPaymentOptionId
                // (G-Authorize-13). Not a MIT, so isRebilling stays unset.
                user_token_id = Some(
                    cit_user_token_id
                        .clone()
                        .or_else(|| resource.connector_customer.clone().map(Secret::new))
                        .ok_or_else(|| {
                            nuvei_missing_field(
                                "customer_id",
                                "Send customer.id - the userTokenId the stored payment option was created under",
                            )
                        })?,
                );
                NuveiPaymentOption {
                    card: None,
                    alternative_payment_method: None,
                    user_payment_option_id: Some(token_data.token.clone()),
                }
            }
            PaymentMethodData::MandatePayment => {
                return Err(nuvei_authorize_not_implemented(
                    "MandatePayment",
                    "Use RecurringPaymentService/Charge for merchant-initiated payments",
                )
                .into())
            }
            PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                return Err(nuvei_authorize_not_implemented(
                    "CardDetailsForNetworkTransactionId",
                    "Use RecurringPaymentService/Charge for network-transaction-id payments",
                )
                .into())
            }
            PaymentMethodData::Wallet(_) => {
                return Err(nuvei_authorize_not_implemented(
                    "Wallet payment methods",
                    "Use a card, network token, bank redirect or ACH payment method",
                )
                .into())
            }
            PaymentMethodData::PayLater(_) => {
                return Err(nuvei_authorize_not_implemented(
                    "Pay-later payment methods",
                    "Use a card, network token, bank redirect or ACH payment method",
                )
                .into())
            }
            _ => {
                return Err(nuvei_authorize_not_implemented(
                    "This payment method",
                    "Use a card, network token, bank redirect or ACH payment method",
                )
                .into())
            }
        };

        // order-bound session token (NuveiMeta, else request session_token)
        let session_token =
            resolve_session_token(nuvei_meta.as_ref(), resource.session_token.as_ref(), None)?;

        // clientUniqueId is String(45)
        let client_request_id = resource.connector_request_reference_id.clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        // billing email and country are mandatory
        let billing_address = get_required_billing_address(resource, email.clone())?;
        let shipping_address = get_shipping_address(resource, Some(billing_address.email.clone()));

        // deviceDetails.ipAddress is mandatory
        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address)
            .ok_or_else(|| {
                nuvei_missing_field(
                    "browser_info.ip_address",
                    "Send browser_info.ip_address of the customer",
                )
            })?;
        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        // dynamicDescriptor.merchantPhone is at most 13 characters
        let dynamic_descriptor = get_dynamic_descriptor(request.billing_descriptor.as_ref())?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let currency = request.currency;
        let converter = item.connector.amount_converter_webhooks;
        let amount = convert_nuvei_amount(converter, request.minor_amount, currency)?;

        let l2_l3_data = resource.l2_l3_data.as_deref();
        let items = get_l2_l3_items(l2_l3_data, currency, converter)?;
        let amount_details = get_amount_details(l2_l3_data, currency, converter)?;

        // Determine transaction type based on capture method (Auth for manual or amount 0)
        let transaction_type =
            TransactionType::get_from_capture_method(request.capture_method, request.minor_amount)?;

        // Build urlDetails from router_return_url if available
        let url_details = request
            .router_return_url
            .as_ref()
            .map(|url| NuveiUrlDetails {
                success_url: url.clone(),
                failure_url: url.clone(),
                pending_url: url.clone(),
            });

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            user_token_id,
            client_unique_id,
            payment_option,
            transaction_type,
            device_details,
            billing_address,
            shipping_address,
            url_details,
            dynamic_descriptor,
            is_partial_approval: get_partial_approval_flag(request.enable_partial_authorization),
            items,
            amount_details,
            is_rebilling,
            related_transaction_id: nuvei_meta.and_then(|meta| meta.related_transaction_id),
            is_moto: get_is_moto(request.payment_channel.as_ref()),
            time_stamp,
            checksum,
        })
    }
}

// Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiPaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let request = &router_data.request;

        let status = get_nuvei_payment_status(
            Some(request.minor_amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = build_nuvei_connector_response(response.payment_option.as_ref());

        let resource_common_data = apply_nuvei_partial_approval(
            response,
            PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data.clone()
            },
            item.http_code,
        )?;

        // A 2xx DECLINED/ERROR is a failure
        let response_data = match build_nuvei_error_response(
            response,
            item.http_code,
            Some(FlowStatus::Payment(status)),
        ) {
            Some(error) => Err(error),
            None => Ok(build_nuvei_transaction_response(
                response,
                router_data.resource_common_data.payment_method,
                request
                    .browser_info
                    .as_ref()
                    .and_then(|browser_info| browser_info.ip_address)
                    .map(|ip| ip.to_string()),
                item.http_code,
            )?),
        };

        Ok(Self {
            resource_common_data,
            response: response_data,
            ..router_data.clone()
        })
    }
}

// ---- ThreeDS: PreAuthenticate (/initPayment.do) and Authenticate (/payment.do + threeD) ----
//
// Nuvei 3DS card payment: initPayment -> payment with threeD -> (ACS
// challenge) -> final payment without threeD referencing the Auth3D transaction.
// The legs share the order-bound sessionToken and chain relatedTransactionId through
// NuveiMeta in connector_feature_data.

/// initPayment request (hyperswitch `NuveiThreeDSInitPaymentRequest`). No
/// timeStamp/checksum, as hyperswitch.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiInitPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub payment_option: NuveiInitPaymentOption<T>,
    pub device_details: NuveiDeviceDetails,
    pub user_token_id: Option<Secret<String>>,
    pub billing_address: Option<NuveiBillingAddress>,
    pub url_details: NuveiUrlDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiInitPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: NuveiCard<T>,
}

/// initPayment response body.
pub type NuveiInitPaymentResponse = NuveiPaymentsResponse;

/// Authenticate request: the Authorize /payment.do body carrying
/// paymentOption.card.threeD and the initPayment relatedTransactionId.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct NuveiAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(pub NuveiPaymentRequest<T>);

/// Authenticate (/payment.do with threeD) response body.
pub type NuveiAuthenticateResponse = NuveiPaymentsResponse;

/// Browser return of the ACS challenge (hyperswitch `NuveiRedirectionResponse`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NuveiRedirectionResponse {
    Redirection { cres: Secret<String> },
    Error { error: Secret<String> },
}

/// Decoded `cres` (hyperswitch `NuveiACSResponse`); only transStatus is used.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NuveiAcsResponse {
    trans_status: Option<NuveiAcsTransStatus>,
}

#[derive(Debug, Deserialize, PartialEq)]
enum NuveiAcsTransStatus {
    #[serde(rename = "Y", alias = "1", alias = "y")]
    Success,
    #[serde(rename = "N", alias = "0", alias = "n")]
    Failed,
    #[serde(other)]
    Unknown,
}

/// Decoded `error` of the challenge return (hyperswitch `NuveiErrorResponse`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NuveiAcsErrorResponse {
    error_code: Option<String>,
    error_message: Option<String>,
    error_detail: Option<String>,
}

fn nuvei_three_ds_not_implemented(what: &str, flow: &str) -> IntegrationError {
    IntegrationError::NotImplemented(
        format!("{what} is not implemented for Nuvei {flow}"),
        nuvei_error_context(
            "Use a card payment method for Nuvei 3DS",
            format!("Nuvei 3DS ({flow}) authenticates card payments only"),
        ),
    )
}

/// The 3DS legs take a raw card only.
fn get_nuvei_three_ds_card<'a, T: PaymentMethodDataTypes>(
    payment_method_data: Option<&'a PaymentMethodData<T>>,
    flow: &str,
) -> Result<&'a domain_types::payment_method_data::Card<T>, Report<IntegrationError>> {
    match payment_method_data {
        Some(PaymentMethodData::Card(card)) => Ok(card),
        Some(PaymentMethodData::NetworkToken(_)) => {
            Err(nuvei_three_ds_not_implemented("NetworkToken", flow).into())
        }
        Some(PaymentMethodData::Wallet(_)) => {
            Err(nuvei_three_ds_not_implemented("Wallet payment methods", flow).into())
        }
        Some(_) => Err(nuvei_three_ds_not_implemented("This payment method", flow).into()),
        None => Err(nuvei_missing_field(
            "payment_method_data",
            "Send the card in payment_method for Nuvei 3DS",
        )
        .into()),
    }
}

/// threeD.browserDetails (hyperswitch `BrowserDetails::try_from`).
fn get_nuvei_browser_details(
    browser_info: &BrowserInformation,
) -> Result<NuveiBrowserDetails, Report<IntegrationError>> {
    let missing = |field_name: &'static str| {
        nuvei_missing_field(
            field_name,
            "Send the full browser_info of the customer for Nuvei 3DS",
        )
    };
    Ok(NuveiBrowserDetails {
        accept_header: browser_info
            .accept_header
            .clone()
            .ok_or_else(|| missing("browser_info.accept_header"))?,
        ip: browser_info
            .ip_address
            .map(|ip| Secret::new(ip.to_string()))
            .ok_or_else(|| missing("browser_info.ip_address"))?,
        java_enabled: browser_info
            .java_enabled
            .ok_or_else(|| missing("browser_info.java_enabled"))?
            .to_string()
            .to_uppercase(),
        java_script_enabled: browser_info
            .java_script_enabled
            .ok_or_else(|| missing("browser_info.java_script_enabled"))?
            .to_string()
            .to_uppercase(),
        language: browser_info
            .language
            .clone()
            .ok_or_else(|| missing("browser_info.language"))?,
        color_depth: browser_info
            .color_depth
            .ok_or_else(|| missing("browser_info.color_depth"))?,
        screen_height: browser_info
            .screen_height
            .ok_or_else(|| missing("browser_info.screen_height"))?,
        screen_width: browser_info
            .screen_width
            .ok_or_else(|| missing("browser_info.screen_width"))?,
        time_zone: browser_info
            .time_zone
            .ok_or_else(|| missing("browser_info.time_zone"))?,
        user_agent: browser_info
            .user_agent
            .clone()
            .ok_or_else(|| missing("browser_info.user_agent"))?,
    })
}

fn get_nuvei_redirection_response(
    redirect_response: &ContinueRedirectionResponse,
) -> Option<NuveiRedirectionResponse> {
    redirect_response
        .payload
        .as_ref()
        .and_then(|payload| {
            serde_json::from_value::<NuveiRedirectionResponse>(payload.peek().clone()).ok()
        })
        .or_else(|| {
            redirect_response.params.as_ref().and_then(|params| {
                let pairs: Vec<(String, String)> =
                    url::form_urlencoded::parse(params.peek().trim_start_matches('?').as_bytes())
                        .into_owned()
                        .collect();
                let value_of = |key: &str| {
                    pairs
                        .iter()
                        .find(|(name, _)| name == key)
                        .map(|(_, value)| Secret::new(value.clone()))
                };
                value_of("cres")
                    .map(|cres| NuveiRedirectionResponse::Redirection { cres })
                    .or_else(|| {
                        value_of("error").map(|error| NuveiRedirectionResponse::Error { error })
                    })
            })
        })
}

fn decode_nuvei_challenge_field<R: serde::de::DeserializeOwned>(
    value: &Secret<String>,
    field_name: &'static str,
) -> Result<R, Report<IntegrationError>> {
    let invalid = || IntegrationError::InvalidDataFormat {
        field_name,
        context: nuvei_error_context(
            "Pass the ACS challenge return (cres / error) unchanged in redirection_response",
            format!("Nuvei challenge return {field_name} is not base64-encoded JSON"),
        ),
    };
    let bytes = crate::utils::safe_base64_decode(value.peek().clone()).change_context(invalid())?;
    serde_json::from_slice::<R>(&bytes).change_context(invalid())
}

fn nuvei_three_ds_authentication_failed(detail: String) -> IntegrationError {
    IntegrationError::NotSupported {
        message: format!("Charging a payment whose 3DS authentication failed ({detail})"),
        connector: "nuvei",
        context: nuvei_error_context(
            "Do not retry the charge; start a new payment and authenticate the card again",
            format!("Nuvei 3DS authentication failed: {detail}"),
        ),
    }
}

/// Port of hyperswitch `ConnectorRedirectResponse::get_flow_type`: the
/// final payment is refused, with no HTTP call, when the decoded `cres` has a
/// transStatus other than Y or the challenge returned an `error`.
fn validate_nuvei_challenge_result(
    redirect_response: Option<&ContinueRedirectionResponse>,
) -> Result<(), Report<IntegrationError>> {
    match redirect_response.and_then(get_nuvei_redirection_response) {
        None => Ok(()),
        Some(NuveiRedirectionResponse::Redirection { cres }) => {
            let acs_response: NuveiAcsResponse =
                decode_nuvei_challenge_field(&cres, "redirect_response.cres")?;
            match acs_response.trans_status {
                Some(NuveiAcsTransStatus::Success) => Ok(()),
                Some(NuveiAcsTransStatus::Failed) | Some(NuveiAcsTransStatus::Unknown) | None => {
                    Err(nuvei_three_ds_authentication_failed(format!(
                        "ACS transStatus {:?}",
                        acs_response.trans_status
                    ))
                    .into())
                }
            }
        }
        Some(NuveiRedirectionResponse::Error { error }) => {
            let acs_error: NuveiAcsErrorResponse =
                decode_nuvei_challenge_field(&error, "redirect_response.error")?;
            let code = acs_error
                .error_code
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            let message = acs_error
                .error_detail
                .or(acs_error.error_message)
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());
            Err(nuvei_three_ds_authentication_failed(format!("{code}: {message}")).into())
        }
    }
}

/// Session token for the NuveiMeta a 3DS leg hands to the next leg: the one
/// Nuvei echoed, else the one this leg sent.
fn get_nuvei_leg_session_token(
    response: &NuveiPaymentsResponse,
    resource: &PaymentFlowData,
    http_code: u16,
) -> Result<Secret<String>, Report<ConnectorError>> {
    response
        .session_token
        .clone()
        .filter(|token| !token.peek().is_empty())
        .or_else(|| {
            resolve_session_token(
                parse_nuvei_meta(resource.connector_feature_data.as_ref()).as_ref(),
                resource.session_token.as_ref(),
                None,
            )
            .ok()
        })
        .ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("no Nuvei sessionToken to carry to the next 3DS leg".to_string()),
            ))
        })
}

fn encode_nuvei_meta(
    meta: &NuveiMeta,
    http_code: u16,
) -> Result<serde_json::Value, Report<ConnectorError>> {
    serde_json::to_value(meta).map_err(|error| {
        Report::new(ConnectorError::response_handling_failed_with_context(
            http_code,
            Some(format!("failed to encode Nuvei 3DS metadata: {error}")),
        ))
    })
}

fn get_nuvei_transaction_id(
    response: &NuveiPaymentsResponse,
    http_code: u16,
) -> Result<String, Report<ConnectorError>> {
    response.transaction_id.clone().ok_or_else(|| {
        Report::new(ConnectorError::response_handling_failed_with_context(
            http_code,
            Some("missing transactionId in Nuvei 3DS response".to_string()),
        ))
    })
}

/// initPayment status: APPROVED (InitAuth3D) only authenticates, so it maps to
/// AuthenticationPending, never Authorized.
fn get_nuvei_pre_authenticate_status(
    response: &NuveiPaymentsResponse,
) -> common_enums::AttemptStatus {
    use common_enums::AttemptStatus;
    match response.transaction_status {
        Some(NuveiTransactionStatus::Approved) | Some(NuveiTransactionStatus::Redirect) => {
            AttemptStatus::AuthenticationPending
        }
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
            AttemptStatus::AuthenticationFailed
        }
        Some(NuveiTransactionStatus::Pending)
        | Some(NuveiTransactionStatus::Processing)
        | Some(NuveiTransactionStatus::Unknown) => AttemptStatus::Pending,
        None => match response.status {
            NuveiPaymentStatus::Error | NuveiPaymentStatus::Failed => {
                AttemptStatus::AuthenticationFailed
            }
            NuveiPaymentStatus::Success
            | NuveiPaymentStatus::Processing
            | NuveiPaymentStatus::Unknown => AttemptStatus::Pending,
        },
    }
}

/// threeD.v2AdditionalParams.rebillFrequency for a CIT that stores the card.
const NUVEI_REBILL_FREQUENCY: &str = "0";

/// threeD.v2AdditionalParams.rebillExpiry for a CIT: today + 5 years as
/// YYYYMMDD (hyperswitch `get_card_info`).
fn get_nuvei_rebill_expiry() -> Result<String, Report<IntegrationError>> {
    let encoding_failed = || IntegrationError::RequestEncodingFailed {
        context: nuvei_error_context(
            "Retry the request",
            "failed to compute Nuvei threeD.v2AdditionalParams.rebillExpiry",
        ),
    };
    let now = common_utils::date_time::now();
    now.replace_year(now.year() + 5)
        .map_err(|_| encoding_failed())?
        .date()
        .format(&time::macros::format_description!("[year][month][day]"))
        .map_err(|_| Report::new(encoding_failed()))
}

// PreAuthenticate Request Transformation (initPayment)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiInitPaymentRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let resource = &router_data.resource_common_data;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // card only
        let card =
            get_nuvei_three_ds_card(request.payment_method_data.as_ref(), "PreAuthenticate")?;

        // Order-bound session token: NuveiMeta (HS carrier), else request session_token
        let session_token = resolve_session_token(
            parse_nuvei_meta(resource.connector_feature_data.as_ref()).as_ref(),
            resource.session_token.as_ref(),
            None,
        )?;

        let currency = request.currency.ok_or_else(|| {
            nuvei_missing_field("currency", "Send amount.currency for Nuvei initPayment")
        })?;
        let amount = convert_nuvei_amount(
            item.connector.amount_converter_webhooks,
            request.amount,
            currency,
        )?;

        // browser_info (deviceDetails.ipAddress) and return_url are required
        let browser_info = request.browser_info.as_ref().ok_or_else(|| {
            nuvei_missing_field(
                "browser_info",
                "Send browser_info of the customer for Nuvei 3DS",
            )
        })?;
        let ip_address = browser_info.ip_address.ok_or_else(|| {
            nuvei_missing_field(
                "browser_info.ip_address",
                "Send browser_info.ip_address of the customer",
            )
        })?;
        let return_url = request
            .router_return_url
            .as_ref()
            .map(|url| url.to_string())
            .ok_or_else(|| {
                nuvei_missing_field("return_url", "Send return_url for Nuvei initPayment")
            })?;

        let client_request_id = resource.connector_request_reference_id.clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        // billingAddress is optional on initPayment (hyperswitch get_billing().ok())
        let billing_address = resource
            .get_optional_billing_email()
            .or_else(|| request.email.clone())
            .zip(resource.get_optional_billing_country())
            .map(|(email, country)| build_billing_address(resource, email, country));

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            client_request_id,
            amount,
            currency,
            payment_option: NuveiInitPaymentOption {
                card: NuveiCard {
                    card_number: card.card_number.clone(),
                    card_holder_name: get_nuvei_card_holder_name(card, resource),
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    cvv: card.card_cvc.clone(),
                    three_d: None,
                },
            },
            device_details: NuveiDeviceDetails {
                ip_address: Secret::new(ip_address.to_string()),
            },
            user_token_id: resource
                .customer_id
                .as_ref()
                .map(|customer_id| Secret::new(customer_id.get_string_repr().to_string())),
            billing_address,
            url_details: NuveiUrlDetails {
                success_url: return_url.clone(),
                failure_url: return_url.clone(),
                pending_url: return_url,
            },
        })
    }
}

// PreAuthenticate Response Transformation (initPayment)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiInitPaymentResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiInitPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let status = get_nuvei_pre_authenticate_status(response);

        // A 2xx DECLINED/ERROR is a failure
        if let Some(error) =
            build_nuvei_error_response(response, item.http_code, Some(FlowStatus::Payment(status)))
        {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error),
                ..router_data.clone()
            });
        }

        let transaction_id = get_nuvei_transaction_id(response, item.http_code)?;
        let meta = NuveiMeta {
            session_token: get_nuvei_leg_session_token(
                response,
                &router_data.resource_common_data,
                item.http_code,
            )?,
            related_transaction_id: Some(transaction_id.clone()),
            three_ds_leg: Some(NuveiThreeDsLeg::PreAuthenticate),
            v2supported: response
                .payment_option
                .as_ref()
                .and_then(|option| option.card.as_ref())
                .and_then(|card| card.three_d.as_ref())
                .and_then(|three_d| three_d.v2supported.clone()),
            is_customer_initiated_mandate_payment: None,
            customer_id: None,
            mandate_reference: None,
            network_txn_link_id: None,
        };
        let connector_feature_data = encode_nuvei_meta(&meta, item.http_code)?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_feature_data: Some(pii::SecretSerdeValue::new(connector_feature_data)),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id)),
                authentication_data: None,
                redirection_data: None,
                connector_response_reference_id: response.order_id.clone(),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// Authenticate Request Transformation (payment.do with threeD)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiAuthenticateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let resource = &router_data.resource_common_data;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // card only
        let card = get_nuvei_three_ds_card(request.payment_method_data.as_ref(), "Authenticate")?;

        // the initPayment NuveiMeta (sessionToken + relatedTransactionId)
        let nuvei_meta = parse_nuvei_meta(resource.connector_feature_data.as_ref());
        let related_transaction_id = nuvei_meta
            .as_ref()
            .and_then(|meta| meta.related_transaction_id.clone())
            .ok_or_else(|| {
                nuvei_missing_field(
                    "connector_feature_data.related_transaction_id",
                    "Pass the connector_feature_data returned by PaymentMethodAuthenticationService/PreAuthenticate",
                )
            })?;
        let session_token =
            resolve_session_token(nuvei_meta.as_ref(), resource.session_token.as_ref(), None)?;

        // browser_info and the challenge URLs are required
        let browser_info = request.browser_info.as_ref().ok_or_else(|| {
            nuvei_missing_field(
                "browser_info",
                "Send browser_info of the customer for Nuvei 3DS",
            )
        })?;
        let browser_details = get_nuvei_browser_details(browser_info)?;
        let notification_url = request
            .continue_redirection_url
            .as_ref()
            .map(|url| url.to_string())
            .ok_or_else(|| {
                nuvei_missing_field(
                    "continue_redirection_url",
                    "Send continue_redirection_url (the 3DS challenge notification URL)",
                )
            })?;
        let return_url = request
            .router_return_url
            .as_ref()
            .map(|url| url.to_string())
            .ok_or_else(|| nuvei_missing_field("return_url", "Send return_url for Nuvei 3DS"))?;

        let email = resource
            .get_optional_billing_email()
            .or_else(|| request.email.clone());
        let billing_address = get_required_billing_address(resource, email)?;
        let shipping_address = get_shipping_address(resource, Some(billing_address.email.clone()));

        let client_request_id = resource.connector_request_reference_id.clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        let currency = request
            .currency
            .ok_or_else(|| nuvei_missing_field("currency", "Send amount.currency for Nuvei 3DS"))?;
        let amount = convert_nuvei_amount(
            item.connector.amount_converter_webhooks,
            request.amount,
            currency,
        )?;
        let transaction_type =
            TransactionType::get_from_capture_method(request.capture_method, request.amount)?;

        // dynamicDescriptor.merchantPhone is at most 13 characters
        let dynamic_descriptor = get_dynamic_descriptor(request.billing_descriptor.as_ref())?;
        let converter = item.connector.amount_converter_webhooks;
        let l2_l3_data = resource.l2_l3_data.as_deref();
        let items = get_l2_l3_items(l2_l3_data, currency, converter)?;
        let amount_details = get_amount_details(l2_l3_data, currency, converter)?;

        // CIT (stores the card for future MIT) when this leg charges: isRebilling
        // "0" + userTokenId, and the rebill parameters on the 3DS request. A
        // caller without the typed fields sends the markers in NuveiMeta.
        let is_cit = request.is_customer_initiated_mandate_payment()
            || nuvei_meta
                .as_ref()
                .and_then(|meta| meta.is_customer_initiated_mandate_payment)
                == Some(true);
        let (user_token_id, is_rebilling) = if is_cit {
            let user_token_id = resource
                .customer_id
                .as_ref()
                .map(|customer_id| Secret::new(customer_id.get_string_repr().to_string()))
                .or_else(|| {
                    nuvei_meta
                        .as_ref()
                        .and_then(|meta| meta.customer_id.clone())
                });
            (user_token_id, Some(NuveiIsRebilling::False))
        } else {
            (None, None)
        };

        let time_stamp = NuveiAuthType::get_timestamp();
        // checksum: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        // External 3DS (merchant-performed authentication) takes precedence over
        // the challenge parameters: only threeD.externalMpi is sent, cavv is
        // mandatory there. authentication_data without an MPI result (eci, cavv
        // or dsTransID), e.g. only a 3DS transaction reference, is not external
        // 3DS data and keeps the Nuvei challenge.
        let external_mpi_data = request.authentication_data.as_ref().filter(|auth_data| {
            auth_data.cavv.is_some() || auth_data.eci.is_some() || auth_data.ds_trans_id.is_some()
        });
        let three_d = match external_mpi_data {
            Some(auth_data) => {
                let cavv = auth_data.cavv.clone().ok_or_else(|| {
                    nuvei_missing_field(
                        "authentication_data.cavv",
                        "Send authentication_data.cavv with external 3DS data",
                    )
                })?;
                NuveiCardThreeD {
                    external_mpi: Some(NuveiExternalMpi {
                        eci: auth_data.eci.clone(),
                        cavv,
                        ds_trans_id: auth_data.ds_trans_id.clone(),
                    }),
                    ..Default::default()
                }
            }
            None => {
                let (rebill_expiry, rebill_frequency) = if is_cit {
                    (
                        Some(get_nuvei_rebill_expiry()?),
                        Some(NUVEI_REBILL_FREQUENCY.to_string()),
                    )
                } else {
                    (None, None)
                };
                NuveiCardThreeD {
                    method_completion_ind: Some(NuveiMethodCompletion::Unavailable),
                    browser_details: Some(browser_details),
                    notification_url: Some(notification_url),
                    merchant_url: Some(return_url.clone()),
                    external_mpi: None,
                    platform_type: Some(NuveiPlatformType::Browser),
                    v2_additional_params: Some(NuveiV2AdditionalParams {
                        challenge_window_size: Some(NuveiChallengeWindowSize::FullScreen),
                        challenge_preference: Some(NuveiChallengePreference::NoPreference),
                        rebill_expiry,
                        rebill_frequency,
                    }),
                }
            }
        };

        Ok(Self(NuveiPaymentRequest {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            user_token_id,
            client_unique_id,
            payment_option: NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                    card_number: card.card_number.clone(),
                    card_holder_name: get_nuvei_card_holder_name(card, resource),
                    expiration_month: card.card_exp_month.clone(),
                    expiration_year: card.card_exp_year.clone(),
                    cvv: card.card_cvc.clone(),
                    three_d: Some(Box::new(three_d)),
                })),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            transaction_type,
            device_details: NuveiDeviceDetails {
                ip_address: browser_info
                    .ip_address
                    .map(|ip| Secret::new(ip.to_string()))
                    .ok_or_else(|| {
                        nuvei_missing_field(
                            "browser_info.ip_address",
                            "Send browser_info.ip_address of the customer",
                        )
                    })?,
            },
            billing_address,
            shipping_address,
            url_details: Some(NuveiUrlDetails {
                success_url: return_url.clone(),
                failure_url: return_url.clone(),
                pending_url: return_url,
            }),
            dynamic_descriptor,
            is_partial_approval: get_partial_approval_flag(request.enable_partial_authorization),
            items,
            amount_details,
            is_rebilling,
            related_transaction_id: Some(related_transaction_id),
            is_moto: get_is_moto(request.payment_channel.as_ref()),
            time_stamp,
            checksum,
        }))
    }
}

// Authenticate Response Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let request = &router_data.request;

        // REDIRECT / APPROVED+Auth3D -> AuthenticationPending; frictionless
        // APPROVED+Sale -> Charged, +Auth -> Authorized; DECLINED/ERROR+Auth3D ->
        // AuthenticationFailed
        let status = get_nuvei_payment_status(
            Some(request.amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = build_nuvei_connector_response(response.payment_option.as_ref());

        let resource_common_data = apply_nuvei_partial_approval(
            response,
            PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data.clone()
            },
            item.http_code,
        )?;

        // A 2xx DECLINED/ERROR is a failure
        if let Some(error) =
            build_nuvei_error_response(response, item.http_code, Some(FlowStatus::Payment(status)))
        {
            return Ok(Self {
                resource_common_data,
                response: Err(error),
                ..router_data.clone()
            });
        }

        let transaction_id = get_nuvei_transaction_id(response, item.http_code)?;
        let three_d = response
            .payment_option
            .as_ref()
            .and_then(|option| option.card.as_ref())
            .and_then(|card| card.three_d.as_ref());

        // Challenge: POST the cReq to the ACS
        let redirection_data = three_d
            .and_then(|three_d| three_d.acs_url.clone().zip(three_d.c_req.clone()))
            .map(|(endpoint, creq)| {
                Box::new(RedirectForm::Form {
                    endpoint,
                    method: Method::Post,
                    form_fields: std::collections::HashMap::from([(
                        "creq".to_string(),
                        creq.peek().clone(),
                    )]),
                })
            });

        // Frictionless: the 3DS result Nuvei returned
        let authentication_data = three_d
            .filter(|three_d| {
                three_d.eci.is_some() || three_d.cavv.is_some() || three_d.ds_trans_id.is_some()
            })
            .map(|three_d| AuthenticationData {
                trans_status: None,
                eci: three_d.eci.clone(),
                cavv: three_d.cavv.clone(),
                ucaf_collection_indicator: None,
                threeds_server_transaction_id: None,
                message_version: None,
                ds_trans_id: three_d.ds_trans_id.clone(),
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
            });

        // Frictionless Sale/Auth (or external 3DS): this leg charged, so it
        // returns the stored-credential reference, NTID and link id the same way
        // the Authorize response does.
        let (mandate_reference, network_txn_id, network_txn_link_id) = match status {
            common_enums::AttemptStatus::Charged
            | common_enums::AttemptStatus::Authorized
            | common_enums::AttemptStatus::PartialCharged => {
                match build_nuvei_transaction_response(
                    response,
                    router_data.resource_common_data.payment_method,
                    request
                        .browser_info
                        .as_ref()
                        .and_then(|browser_info| browser_info.ip_address)
                        .map(|ip| ip.to_string()),
                    item.http_code,
                )? {
                    PaymentsResponseData::TransactionResponse {
                        mandate_reference,
                        network_txn_id,
                        network_txn_link_id,
                        ..
                    } => (mandate_reference, network_txn_id, network_txn_link_id),
                    _ => (None, None, None),
                }
            }
            _ => (None, None, None),
        };
        let meta_mandate_reference = mandate_reference.as_ref().and_then(|mandate| {
            mandate
                .connector_mandate_id
                .clone()
                .map(|connector_mandate_id| NuveiMetaMandateReference {
                    connector_mandate_id: Secret::new(connector_mandate_id),
                    mandate_metadata: mandate.mandate_metadata.clone(),
                })
        });

        // The final Authorize references this (Auth3D) transaction
        let meta = NuveiMeta {
            session_token: get_nuvei_leg_session_token(
                response,
                &router_data.resource_common_data,
                item.http_code,
            )?,
            related_transaction_id: Some(transaction_id.clone()),
            three_ds_leg: Some(NuveiThreeDsLeg::Authenticate),
            v2supported: None,
            is_customer_initiated_mandate_payment: None,
            customer_id: None,
            mandate_reference: meta_mandate_reference,
            network_txn_link_id: network_txn_link_id.clone(),
        };
        let connector_feature_data = encode_nuvei_meta(&meta, item.http_code)?;

        Ok(Self {
            resource_common_data,
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id)),
                redirection_data,
                authentication_data,
                connector_feature_data: Some(connector_feature_data),
                connector_response_reference_id: response.order_id.clone(),
                status_code: item.http_code,
                mandate_reference,
                network_txn_id,
                network_txn_link_id,
            }),
            ..router_data.clone()
        })
    }
}

// Capture Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for NuveiCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        // clientUniqueId is String(45)
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // relatedTransactionId (the Auth transactionId) is required
        let related_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: nuvei_error_context(
                        "Pass the Nuvei transactionId returned by Authorize as connector_transaction_id",
                        "Nuvei settleTransaction references the Auth by relatedTransactionId",
                    ),
                }
                .into());
            }
        };

        // Amount to settle (partial capture passes through); StringMajorUnit
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Send a valid minor_amount_to_capture and currency",
                    "Nuvei settleTransaction amount could not be converted to a major-unit string",
                ),
            })?;

        let currency = router_data.request.currency;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// PSync Response Transformation (hyperswitch NuveiTransactionSyncResponse handler)
impl TryFrom<ResponseRouterData<NuveiSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiSyncResponse, Self>) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // errCode 9146: transaction not indexed yet. Not an error; status is
        // left unchanged and the requested transaction id is echoed back.
        if item.response.err_code == Some(NUVEI_NO_TRANSACTION_FOUND_ERR_CODE) {
            return Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: request.connector_transaction_id.clone(),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
                ..router_data.clone()
            });
        }

        let response = item.response.to_payments_response();

        // Request-level failure: the lookup itself failed (checksum, timestamp,
        // rate limit) and says nothing about the payment. The status stays as
        // received and the Err carries errCode/reason with no attempt status.
        let is_request_level_failure = matches!(
            response.status,
            NuveiPaymentStatus::Error | NuveiPaymentStatus::Failed
        ) && response.transaction_status.is_none();
        if is_request_level_failure {
            let error_view = NuveiPaymentsResponse {
                status: NuveiPaymentStatus::Error,
                ..response.clone()
            };
            let error =
                build_nuvei_error_response(&error_view, item.http_code, None).ok_or_else(|| {
                    Report::new(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("Nuvei getTransactionDetails request-level failure".to_string()),
                    ))
                })?;
            return Ok(Self {
                response: Err(error),
                ..router_data.clone()
            });
        }

        let status = get_nuvei_payment_status(
            Some(request.amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = build_nuvei_connector_response(response.payment_option.as_ref());

        let resource_common_data = apply_nuvei_partial_approval(
            &response,
            PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data.clone()
            },
            item.http_code,
        )?;

        // status ERROR, or a 2xx DECLINED/ERROR transaction, is a failure
        let response_data = match build_nuvei_error_response(
            &response,
            item.http_code,
            Some(FlowStatus::Payment(status)),
        ) {
            Some(error) => Err(error),
            None => Ok(build_nuvei_transaction_response(
                &response,
                router_data.resource_common_data.payment_method,
                None,
                item.http_code,
            )?),
        };

        Ok(Self {
            resource_common_data,
            response: response_data,
            ..router_data.clone()
        })
    }
}

// Capture Response Transformation (hyperswitch generic NuveiPaymentsResponse handler)
impl TryFrom<ResponseRouterData<NuveiCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // HS passes the capturable amount; transactionStatus absent + SUCCESS -> Pending
        let status = get_nuvei_payment_status(
            router_data
                .resource_common_data
                .minor_amount_capturable
                .map(|amount| amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        // status ERROR, or a 2xx DECLINED/ERROR settle, is a failure
        let response_data = match build_nuvei_error_response(
            response,
            item.http_code,
            Some(FlowStatus::Payment(status)),
        ) {
            Some(error) => Err(error),
            None => Ok(build_nuvei_transaction_response(
                response,
                router_data.resource_common_data.payment_method,
                None,
                item.http_code,
            )?),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: response_data,
            ..router_data.clone()
        })
    }
}

// Refund Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for NuveiRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        // clientUniqueId is String(45): refuse longer references before building
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        let currency = router_data.request.currency;

        // Typed minor refund amount -> Nuvei major-unit string
        let amount = convert_nuvei_amount(
            item.connector.amount_converter_webhooks,
            router_data.request.minor_refund_amount,
            currency,
        )?;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Sync Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for NuveiRefundSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // the refund's own Nuvei transactionId (Credit) is the
        // only id getTransactionDetails can resolve for a refund.
        let transaction_id = router_data.request.connector_refund_id.clone();
        if transaction_id.is_empty() {
            return Err(IntegrationError::MissingConnectorRefundID {
                context: nuvei_error_context(
                    "Pass the connector_refund_id returned by the Nuvei refund call",
                    "Nuvei getTransactionDetails looks a refund up by its transactionId",
                ),
            }
            .into());
        }

        let time_stamp = NuveiAuthType::get_timestamp();

        // getTransactionDetails checksum: merchantId + merchantSiteId + transactionId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Refund Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiRefundResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // HS get_refund_response: status ERROR or transactionStatus ERROR is a
        // failed refund returned as Err, attempt_status Refund(Failure).
        let is_error = matches!(response.status, NuveiPaymentStatus::Error)
            || matches!(
                response.transaction_status,
                Some(NuveiTransactionStatus::Error)
            );
        if let Some(error) = is_error
            .then(|| {
                build_nuvei_error_response(
                    response,
                    item.http_code,
                    Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                )
            })
            .flatten()
        {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error),
                ..router_data.clone()
            });
        }

        // HS refund status map; an absent transactionStatus is a Failure (never Success)
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Unknown) => common_enums::RefundStatus::Pending,
            None => common_enums::RefundStatus::Failure,
        };

        // Get connector refund ID
        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei refund response".to_string()),
            ))
        })?;

        let refunds_response_data = RefundsResponseData {
            connector_refund_id,
            refund_status,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Refund Sync Response Transformation (hyperswitch RSync NuveiTransactionSyncResponse handler)
impl TryFrom<ResponseRouterData<NuveiRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;

        // errCode 9146: transaction not indexed yet. Not an error; the refund
        // status is left unchanged and the requested refund id is echoed back.
        if item.response.err_code == Some(NUVEI_NO_TRANSACTION_FOUND_ERR_CODE) {
            return Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: request.connector_refund_id.clone(),
                    refund_status: request.refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..router_data.clone()
            });
        }

        let response = item.response.to_payments_response();

        // Request-level failure: status ERROR with no transactionStatus means the
        // lookup itself failed (checksum, timestamp, rate limit) and says nothing
        // about the refund. The refund status stays as received and the Err
        // carries errCode/reason with no attempt status (HS get_error_response).
        let is_request_level_failure = matches!(response.status, NuveiPaymentStatus::Error)
            && response.transaction_status.is_none();
        if is_request_level_failure {
            let error =
                build_nuvei_error_response(&response, item.http_code, None).ok_or_else(|| {
                    Report::new(ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("Nuvei getTransactionDetails request-level failure".to_string()),
                    ))
                })?;
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: request.refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error),
                ..router_data.clone()
            });
        }

        // HS: status ERROR or transactionDetails.transactionStatus ERROR of the
        // Credit transaction is a failed refund returned as Err, attempt_status
        // Refund(Failure).
        let is_error = matches!(response.status, NuveiPaymentStatus::Error)
            || matches!(
                response.transaction_status,
                Some(NuveiTransactionStatus::Error)
            );
        if let Some(error) = is_error
            .then(|| {
                build_nuvei_error_response(
                    &response,
                    item.http_code,
                    Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                )
            })
            .flatten()
        {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error),
                ..router_data.clone()
            });
        }

        // HS refund status map. An absent transactionStatus is Pending (never
        // Success, never a terminal Failure): a later RSync or the refund DMN
        // settles it (deliberate divergence from HS, plan PL-12).
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Unknown) => common_enums::RefundStatus::Pending,
            None => common_enums::RefundStatus::Pending,
        };

        // connector_refund_id = transactionDetails.transactionId
        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some(
                    "transactionDetails.transactionId missing in Nuvei refund sync response"
                        .to_string(),
                ),
            ))
        })?;

        Ok(Self {
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..router_data.clone()
        })
    }
}

// Void Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for NuveiVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        // clientUniqueId is String(45)
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // relatedTransactionId: the Auth transactionId
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Nuvei voids the original amount and currency
        let minor_amount = router_data.request.amount.ok_or_else(|| {
            nuvei_missing_field(
                "amount",
                "Send the original authorized amount on the Void request",
            )
        })?;

        let currency = router_data.request.currency.ok_or_else(|| {
            nuvei_missing_field(
                "currency",
                "Send the original authorized currency on the Void request",
            )
        })?;

        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Send a valid amount and currency",
                    "Nuvei voidTransaction amount could not be converted to a major-unit string",
                ),
            })?;

        // Checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + timeStamp + merchantSecretKey (no authCode)
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            time_stamp,
            checksum,
        })
    }
}

// Void Response Transformation
impl TryFrom<ResponseRouterData<NuveiVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiVoidResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // APPROVED+Void -> Voided; APPROVED without transactionType -> Pending;
        // DECLINED/ERROR+Void -> VoidFailed
        let status = get_nuvei_payment_status(
            router_data
                .request
                .amount
                .map(|amount| amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        // status ERROR, or a 2xx DECLINED/ERROR void, is a failure;
        // a failed void is VoidFailed
        let (status, response_data) = match build_nuvei_error_response(
            response,
            item.http_code,
            Some(FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed)),
        ) {
            Some(error) => (common_enums::AttemptStatus::VoidFailed, Err(error)),
            None => (
                status,
                Ok(build_nuvei_transaction_response(
                    response,
                    router_data.resource_common_data.payment_method,
                    None,
                    item.http_code,
                )?),
            ),
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: response_data,
            ..router_data.clone()
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Creates a Nuvei session token for client-side SDK initialization.
/// Uses the same /getSessionToken.do endpoint as ServerSessionAuthenticationToken
/// but returns the response in the ClientAuthenticationToken format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiClientAuthRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

/// Nuvei session token response for ClientAuthenticationToken flow.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiClientAuthResponse {
    pub session_token: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

// ClientAuthenticationToken Request Transformation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiClientAuthRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Generate checksum for getSessionToken: merchantId + merchantSiteId + clientRequestId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            time_stamp,
            checksum,
        })
    }
}

// ClientAuthenticationToken Response Transformation
impl TryFrom<ResponseRouterData<NuveiClientAuthResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        MerchantAuthenticationFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiClientAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;

        // Check if the overall request status is ERROR. No payment attempt
        // exists yet, so the error carries no attempt status.
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response
                .err_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        }

        // Extract session token
        let session_token = response.session_token.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("session_token missing in Nuvei response".to_string()),
            ))
        })?;

        let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
            ConnectorSpecificClientAuthenticationResponse::Nuvei(
                NuveiClientAuthenticationResponseDomain { session_token },
            ),
        ));

        Ok(Self {
            response: Ok(PaymentsResponseData::ClientAuthenticationTokenResponse {
                session_data,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ============================================================================
// OpenOrder (CreateOrder) Request/Response Types
// ============================================================================

/// OpenOrder request — creates a Nuvei order session and returns a sessionToken + orderId.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiOpenOrderRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub client_request_id: String,
    pub currency: common_enums::Currency,
    pub amount: StringMajorUnit,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TransactionType>,
}

/// OpenOrder response — returns sessionToken and orderId for subsequent payment flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiOpenOrderResponse {
    pub session_token: Option<Secret<String>>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub order_id: Option<String>,
    pub client_unique_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub client_request_id: Option<String>,
}

/// Nuvei's `openOrder.do` returns `orderId` as a bare JSON integer despite docs
/// declaring it as String(20). Mirrors the Bambora `str_or_i32` pattern.
fn str_or_i64<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrI64 {
        Str(String),
        I64(i64),
    }

    Ok(
        Option::<StrOrI64>::deserialize(deserializer)?.map(|v| match v {
            StrOrI64::Str(s) => s,
            StrOrI64::I64(n) => n.to_string(),
        }),
    )
}

// --- TryFrom: RouterDataV2 -> NuveiOpenOrderRequest (via macro wrapper) ---

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    > for NuveiOpenOrderRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: nuvei_error_context(
                    "Send a valid minor amount and currency",
                    "Nuvei openOrder amount could not be converted to a major-unit string",
                ),
            })?;

        let currency = router_data.request.currency;

        // Generate checksum for openOrder: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            client_request_id,
            currency,
            amount,
            time_stamp,
            checksum,
            transaction_type: Some(TransactionType::Auth),
        })
    }
}

// --- TryFrom: NuveiOpenOrderResponse -> PaymentCreateOrderResponse ---

/// A SUCCESS openOrder answer without an orderId is a response-handling error,
/// never an empty connector_order_id.
fn get_nuvei_open_order_response(
    response: &NuveiOpenOrderResponse,
    http_code: u16,
) -> Result<PaymentCreateOrderResponse, Report<ConnectorError>> {
    let connector_order_id = response
        .order_id
        .clone()
        .filter(|order_id| !order_id.is_empty())
        .ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_code,
                Some("missing orderId in Nuvei openOrder response".to_string()),
            ))
        })?;
    Ok(PaymentCreateOrderResponse {
        connector_order_id,
        session_data: None,
    })
}

// --- TryFrom: ResponseRouterData -> RouterDataV2 (CreateOrder response handler) ---

impl TryFrom<ResponseRouterData<NuveiOpenOrderResponse, Self>>
    for RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiOpenOrderResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = item.response;

        // Check if the request status is ERROR
        if matches!(
            response.status,
            NuveiPaymentStatus::Error | NuveiPaymentStatus::Failed
        ) {
            let error_code = response
                .err_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string());
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

            // openOrder precedes the payment: no attempt status on its error
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..item.router_data
            });
        }

        let order_response = get_nuvei_open_order_response(&response, item.http_code)?;

        // Extract order_id to store for Authorize flow
        let order_id = order_response.connector_order_id.clone();

        // Store session_token in session_token field for use by Authorize flow
        let session_token = response
            .session_token
            .as_ref()
            .map(|token| token.peek().clone());

        Ok(Self {
            response: Ok(order_response),
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::Pending,
                reference_id: Some(order_id.clone()),
                connector_order_id: Some(order_id),
                // Store session_token for use by subsequent payment flows
                session_token,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== SetupMandate (SetupRecurring) flow =====
//
// Nuvei SetupRecurring uses the same /ppp/api/v1/payment.do endpoint as the
// Authorize flow. The request reuses the Authorize builder pieces with
// isRebilling "0" + userTokenId, marking the initial customer-initiated
// transaction; a successful response returns a userPaymentOptionId which is
// the connector_mandate_id for subsequent MIT RepeatPayment calls.

/// SetupMandate request - the Authorize /payment.do body for a CIT (no
/// authenticationOnlyType).
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub client_unique_id: String,
    /// userTokenId binds the card to a reusable userPaymentOptionId.
    pub user_token_id: Secret<String>,
    pub payment_option: NuveiPaymentOption<T>,
    /// "0" marks the initial CIT transaction of a recurring series.
    pub is_rebilling: NuveiIsRebilling,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    pub shipping_address: Option<NuveiShippingAddress>,
    pub url_details: Option<NuveiUrlDetails>,
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    pub is_partial_approval: Option<NuveiPartialApprovalFlag>,
    pub items: Option<Vec<NuveiItem>>,
    pub amount_details: Option<NuveiAmountDetails>,
    pub is_moto: Option<bool>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

fn nuvei_setup_mandate_not_supported(what: &str, suggested_action: &str) -> IntegrationError {
    IntegrationError::NotSupported {
        message: format!("{what} is not supported for Nuvei SetupRecurring"),
        connector: "nuvei",
        context: nuvei_error_context(
            suggested_action,
            format!("Nuvei SetupRecurring (/payment.do CIT) does not offer {what}"),
        ),
    }
}

// Build the SetupMandate request from the router data.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiSetupMandateRequest<T>
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let request = &router_data.request;
        let resource = &router_data.resource_common_data;

        // SetupRecurring has no authentication-leg dispatch
        if resource.auth_type == common_enums::AuthenticationType::ThreeDs {
            return Err(nuvei_setup_mandate_not_supported(
                "3DS card-on-file setup",
                "Nuvei 3DS card-on-file setup runs through CompositePaymentService/Authorize with setup_future_usage=off_session",
            )
            .into());
        }

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // Payment-method dispatch: Card only
        let payment_option = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = get_nuvei_card_holder_name(card_data, resource);
                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d: None,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            PaymentMethodData::NetworkToken(_) => {
                return Err(nuvei_setup_mandate_not_supported(
                    "Network token",
                    "Send raw card details to SetupRecurring",
                )
                .into())
            }
            PaymentMethodData::Wallet(_) => {
                return Err(nuvei_setup_mandate_not_supported(
                    "Wallet payment methods",
                    "Send raw card details to SetupRecurring",
                )
                .into())
            }
            _ => {
                return Err(nuvei_setup_mandate_not_supported(
                    "This payment method",
                    "Send raw card details to SetupRecurring",
                )
                .into())
            }
        };

        // order-bound session token (NuveiMeta, else request session_token)
        let nuvei_meta = parse_nuvei_meta(resource.connector_feature_data.as_ref());
        let session_token =
            resolve_session_token(nuvei_meta.as_ref(), resource.session_token.as_ref(), None)?;

        // userTokenId (customer_id) is required for UPO creation
        let user_token_id = resource
            .customer_id
            .as_ref()
            .or(request.customer_id.as_ref())
            .map(|customer_id| Secret::new(customer_id.get_string_repr().to_string()))
            .ok_or_else(|| {
                nuvei_missing_field(
                    "customer_id",
                    "Send customer.id; Nuvei stores the card under userTokenId",
                )
            })?;

        // deviceDetails.ipAddress is mandatory
        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address)
            .ok_or_else(|| {
                nuvei_missing_field(
                    "browser_info.ip_address",
                    "Send browser_info.ip_address of the customer",
                )
            })?;
        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        // clientUniqueId is String(45)
        let client_request_id = resource.connector_request_reference_id.clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        // Billing email and country are mandatory; shipping only when present
        let billing_address = get_required_billing_address(resource, request.email.clone())?;
        let shipping_address = get_shipping_address(resource, Some(billing_address.email.clone()));

        let dynamic_descriptor = get_dynamic_descriptor(request.billing_descriptor.as_ref())?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let currency = request.currency;
        let converter = item.connector.amount_converter_webhooks;
        // Amount is optional for SetupMandate: absent -> zero-amount verification
        let minor_amount = request
            .minor_amount
            .unwrap_or(common_utils::types::MinorUnit::new(0));
        let amount = convert_nuvei_amount(converter, minor_amount, currency)?;

        let l2_l3_data = resource.l2_l3_data.as_deref();
        let items = get_l2_l3_items(l2_l3_data, currency, converter)?;
        let amount_details = get_amount_details(l2_l3_data, currency, converter)?;

        // Auth for manual capture or amount 0, else Sale
        let transaction_type =
            TransactionType::get_from_capture_method(request.capture_method, minor_amount)?;

        let url_details = request
            .router_return_url
            .as_ref()
            .map(|url| NuveiUrlDetails {
                success_url: url.clone(),
                failure_url: url.clone(),
                pending_url: url.clone(),
            });

        // Checksum: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            client_unique_id,
            user_token_id,
            payment_option,
            is_rebilling: NuveiIsRebilling::False,
            transaction_type,
            device_details,
            billing_address,
            shipping_address,
            url_details,
            dynamic_descriptor,
            is_partial_approval: get_partial_approval_flag(request.enable_partial_authorization),
            items,
            amount_details,
            is_moto: get_is_moto(request.payment_channel.as_ref()),
            time_stamp,
            checksum,
        })
    }
}

// Map the Nuvei SetupMandate response onto the SetupMandate RouterDataV2
// (hyperswitch SetupMandate response handler).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let request = &router_data.request;

        // Zero-amount Auth APPROVED -> Charged (card verification)
        let status = get_nuvei_payment_status(
            Some(
                request
                    .minor_amount
                    .map(|amount| amount.get_amount_as_i64())
                    .unwrap_or(0),
            ),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = build_nuvei_connector_response(response.payment_option.as_ref());
        let resource_common_data = apply_nuvei_partial_approval(
            response,
            PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data.clone()
            },
            item.http_code,
        )?;

        // A 2xx DECLINED/ERROR is a failure
        let response_data = match build_nuvei_error_response(
            response,
            item.http_code,
            Some(FlowStatus::Payment(status)),
        ) {
            Some(error) => Err(error),
            None => {
                // mandate_metadata carries the CIT browser IP for the MIT deviceDetails
                let ip_address = request
                    .browser_info
                    .as_ref()
                    .and_then(|browser_info| browser_info.ip_address)
                    .map(|ip| ip.to_string())
                    .ok_or_else(|| {
                        Report::new(ConnectorError::response_handling_failed_with_context(
                            item.http_code,
                            Some(
                                "missing browser_info.ip_address for Nuvei mandate metadata"
                                    .to_string(),
                            ),
                        ))
                    })?;
                Ok(build_nuvei_transaction_response(
                    response,
                    router_data.resource_common_data.payment_method,
                    Some(ip_address),
                    item.http_code,
                )?)
            }
        };

        Ok(Self {
            resource_common_data,
            response: response_data,
            ..router_data.clone()
        })
    }
}

// ===== RepeatPayment (MIT) flow =====
//
// Merchant-initiated charges reuse the /ppp/api/v1/payment.do endpoint of
// Authorize/SetupMandate. Three arms (hyperswitch NuveiPaymentsRequest):
// - ConnectorMandateId: the stored userPaymentOptionId from SetupMandate,
//   isRebilling "1" + userTokenId, deviceDetails from the CIT mandate_metadata;
// - NetworkMandateId + raw card: card without CVV, externalSchemeDetails
//   (NTID + brand) and transactionLinkId, isRebilling "1" + userTokenId;
// - NetworkTokenWithNTI + network token: externalToken card and
//   externalSchemeDetails.

/// RepeatPayment request - the /payment.do body for an MIT (no rebillingType,
/// no relatedTransactionId).
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRequest {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub client_unique_id: String,
    /// userTokenId must match the value used on the initial SetupMandate so
    /// Nuvei resolves the stored payment option correctly. Not sent for
    /// network-token (NTID) MITs, which carry the token itself.
    pub user_token_id: Option<Secret<String>>,
    pub payment_option: NuveiRepeatPaymentOptionTypes,
    /// "1" marks a merchant-initiated rebilling transaction (stored-credential
    /// and NTID-card MIT; the network token itself marks a network-token MIT).
    pub is_rebilling: Option<NuveiIsRebilling>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    /// Original-transaction reference (NTID + brand) for NTID MITs.
    pub external_scheme_details: Option<NuveiExternalSchemeDetails>,
    /// Mastercard transaction link id (TLID) of the original CIT.
    pub transaction_link_id: Option<String>,
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    pub is_partial_approval: Option<NuveiPartialApprovalFlag>,
    pub is_moto: Option<bool>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Serialize-only untagged enum: each arm emits its own paymentOption shape
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiRepeatPaymentOptionTypes {
    StoredCredential(NuveiRepeatPaymentOption),
    NetworkTransactionCard(NuveiRepeatPaymentNtidCardOption),
    NetworkToken(NuveiRepeatPaymentCardOption),
}

/// paymentOption payload for stored-credential MIT - only userPaymentOptionId
/// is required; Nuvei reuses the stored card bound to this id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentOption {
    pub user_payment_option_id: Secret<String>,
}

/// paymentOption payload for NTID MIT with raw card details.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentNtidCardOption {
    pub card: NuveiNtidCard,
}

/// Card for an NTID MIT: no CVV (hyperswitch `get_ntid_card_info`).
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNtidCard {
    pub card_number: cards::CardNumber,
    pub card_holder_name: Option<Secret<String>>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
}

/// paymentOption payload for network-token MIT.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentCardOption {
    pub card: NuveiNetworkTokenCard,
}

fn nuvei_repeat_payment_not_supported(what: &str, suggested_action: &str) -> IntegrationError {
    IntegrationError::NotSupported {
        message: format!("{what} is not supported for Nuvei RecurringPaymentService/Charge"),
        connector: "nuvei",
        context: nuvei_error_context(
            suggested_action,
            format!("Nuvei MIT (/payment.do) does not offer {what}"),
        ),
    }
}

/// externalSchemeDetails.brand must be one of
/// VISA/MASTERCARD/AMEX/DISCOVER/DINERS (card_network, else BIN issuer).
fn get_nuvei_mit_card_brand(
    brand: Result<NuveiCardType, Report<IntegrationError>>,
) -> Result<NuveiCardType, Report<IntegrationError>> {
    brand.change_context(nuvei_repeat_payment_not_supported(
        "This card network",
        "Nuvei NTID MIT supports Visa, Mastercard, American Express, Discover and Diners Club",
    ))
}

// Build the RepeatPayment request from the router data.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NuveiRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NuveiRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: NuveiRouterData<
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
        let request = &router_data.request;
        let resource = &router_data.resource_common_data;

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // All arms: a non-empty Charge request session_token first (a fresh
        // CreateServerSessionAuthenticationToken; a forwarded SetupRecurring
        // NuveiMeta carries that order's spent session, errCode 1058/1064),
        // then NuveiMeta (the HS carrier; HS sends no request session_token),
        // then state.access_token.
        let nuvei_meta = parse_nuvei_meta(resource.connector_feature_data.as_ref());
        let request_session_token = resource
            .session_token
            .as_ref()
            .filter(|token| !token.is_empty());
        let session_token = resolve_session_token(
            nuvei_meta
                .as_ref()
                .filter(|_| request_session_token.is_none()),
            request_session_token,
            resource.access_token.as_ref(),
        )?;

        // ConnectorMandateId and NetworkMandateIdCard arms:
        // userTokenId = customer_id, else connector_customer.
        let get_user_token_id = || {
            resource
                .customer_id
                .as_ref()
                .map(|customer_id| customer_id.get_string_repr().to_string())
                .or_else(|| resource.connector_customer.clone())
                .map(Secret::new)
                .ok_or_else(|| {
                    nuvei_missing_field(
                        "customer_id",
                        "Send customer.id (or connector_customer_id) used as userTokenId on the initial SetupRecurring",
                    )
                })
        };
        let browser_ip = request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address)
            .map(|ip| ip.to_string());

        let (
            payment_option,
            external_scheme_details,
            transaction_link_id,
            user_token_id,
            is_rebilling,
            ip_address,
        ) = match &request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                let user_payment_option_id =
                    connector_mandate.get_connector_mandate_id().ok_or_else(|| {
                        nuvei_missing_field(
                            "mandate_reference.connector_mandate_id",
                            "Send the userPaymentOptionId returned by SetupRecurring as connector_mandate_id",
                        )
                    })?;
                let user_token_id = get_user_token_id()?;
                // CIT browser IP from mandate_metadata, else browser_info
                let ip_address = connector_mandate
                    .get_mandate_metadata()
                    .and_then(|metadata| metadata.peek().as_str().map(str::to_string))
                    .filter(|ip| !ip.is_empty())
                    .or(browser_ip)
                    .ok_or_else(|| {
                        nuvei_missing_field(
                            "browser_info.ip_address",
                            "Send connector_mandate_id.mandate_metadata from SetupRecurring or browser_info.ip_address",
                        )
                    })?;
                (
                    NuveiRepeatPaymentOptionTypes::StoredCredential(NuveiRepeatPaymentOption {
                        user_payment_option_id: Secret::new(user_payment_option_id),
                    }),
                    None,
                    None,
                    Some(user_token_id),
                    Some(NuveiIsRebilling::True),
                    ip_address,
                )
            }
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                let card = match &request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => card,
                    _ => {
                        return Err(nuvei_repeat_payment_not_supported(
                            "network_mandate_id without card details",
                            "Send payment_method.card_details_for_network_transaction_id with network_mandate_id",
                        )
                        .into())
                    }
                };
                // externalSchemeDetails.brand: Nuvei-supported networks only
                let brand = get_nuvei_mit_card_brand(match card.card_network.clone() {
                    Some(network) => NuveiCardType::try_from(network),
                    None => card
                        .get_card_issuer()
                        .and_then(|issuer| NuveiCardType::try_from(&issuer)),
                })?;
                let user_token_id = get_user_token_id()?;
                let ip_address = browser_ip.ok_or_else(|| {
                    nuvei_missing_field(
                        "browser_info.ip_address",
                        "Send browser_info.ip_address of the customer",
                    )
                })?;
                (
                    NuveiRepeatPaymentOptionTypes::NetworkTransactionCard(
                        NuveiRepeatPaymentNtidCardOption {
                            card: NuveiNtidCard {
                                card_number: card.card_number.clone(),
                                card_holder_name: card.card_holder_name.clone(),
                                expiration_month: card.card_exp_month.clone(),
                                expiration_year: card.card_exp_year.clone(),
                            },
                        },
                    ),
                    Some(NuveiExternalSchemeDetails {
                        transaction_id: Secret::new(network_mandate.network_transaction_id.clone()),
                        brand: Some(brand),
                    }),
                    network_mandate.transaction_link_id.clone(),
                    Some(user_token_id),
                    Some(NuveiIsRebilling::True),
                    ip_address,
                )
            }
            MandateReferenceId::NetworkTokenWithNTI(nti_ref) => {
                let token_data = match &request.payment_method_data {
                    PaymentMethodData::NetworkToken(token_data) => token_data,
                    _ => {
                        return Err(nuvei_repeat_payment_not_supported(
                            "network_token_with_nti without a network token",
                            "Send payment_method.network_token with network_token_with_nti",
                        )
                        .into())
                    }
                };
                // externalSchemeDetails.brand: Nuvei-supported networks only
                let brand = get_nuvei_mit_card_brand(get_nuvei_card_brand(token_data))?;
                let ip_address = browser_ip.ok_or_else(|| {
                    nuvei_missing_field(
                        "browser_info.ip_address",
                        "Send browser_info.ip_address of the customer",
                    )
                })?;
                (
                    NuveiRepeatPaymentOptionTypes::NetworkToken(NuveiRepeatPaymentCardOption {
                        card: build_nuvei_network_token_card(token_data),
                    }),
                    Some(NuveiExternalSchemeDetails {
                        transaction_id: Secret::new(nti_ref.network_transaction_id.clone()),
                        brand: Some(brand),
                    }),
                    None,
                    None,
                    None,
                    ip_address,
                )
            }
        };
        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address),
        };

        // All arms: billing email and country are mandatory
        let billing_address = get_required_billing_address(resource, request.email.clone())?;

        // clientUniqueId is String(45)
        let client_request_id = resource.connector_request_reference_id.clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        let dynamic_descriptor = get_dynamic_descriptor(request.billing_descriptor.as_ref())?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let currency = request.currency;
        let amount = convert_nuvei_amount(
            item.connector.amount_converter_webhooks,
            request.minor_amount,
            currency,
        )?;

        // Auth for manual capture or amount 0, else Sale
        let transaction_type =
            TransactionType::get_from_capture_method(request.capture_method, request.minor_amount)?;

        // Checksum: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            session_token,
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            client_unique_id,
            user_token_id,
            payment_option,
            is_rebilling,
            transaction_type,
            device_details,
            billing_address,
            external_scheme_details,
            transaction_link_id,
            dynamic_descriptor,
            is_partial_approval: get_partial_approval_flag(request.enable_partial_authorization),
            is_moto: get_is_moto(request.payment_channel.as_ref()),
            time_stamp,
            checksum,
        })
    }
}

// Map the Nuvei RepeatPayment response onto the RepeatPayment RouterDataV2
// (hyperswitch payment response handler).
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NuveiRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let request = &router_data.request;

        let status = get_nuvei_payment_status(
            Some(request.minor_amount.get_amount_as_i64()),
            response.transaction_type.as_ref(),
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = build_nuvei_connector_response(response.payment_option.as_ref());
        // A partially approved MIT reports processedAmount, not the requested amount
        let resource_common_data = apply_nuvei_partial_approval(
            response,
            PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data.clone()
            },
            item.http_code,
        )?;

        // A 2xx DECLINED/ERROR is a failure; merchantAdviceCode -> network_advice_code
        let response_data = match build_nuvei_error_response(
            response,
            item.http_code,
            Some(FlowStatus::Payment(status)),
        ) {
            Some(error) => Err(error),
            None => {
                let ip_address = request
                    .browser_info
                    .as_ref()
                    .and_then(|browser_info| browser_info.ip_address)
                    .map(|ip| ip.to_string());
                Ok(build_nuvei_transaction_response(
                    response,
                    router_data.resource_common_data.payment_method,
                    ip_address,
                    item.http_code,
                )?)
            }
        };

        Ok(Self {
            resource_common_data,
            response: response_data,
            ..router_data.clone()
        })
    }
}

// ===== IncomingWebhook (DMN) =====
//
// Payment DMNs arrive as application/x-www-form-urlencoded; chargeback DMNs
// from the Control Panel arrive as JSON with a `checksum` header
// (hyperswitch `NuveiWebhook`, transformers.rs:3182-3512).

/// Any webhook notification Nuvei sends.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NuveiWebhook {
    PaymentDmn(PaymentDmnNotification),
    Chargeback(ChargebackNotification),
}

/// Overall status of the DMN (`Status`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DmnStatus {
    Success,
    Approved,
    Error,
    Pending,
    Declined,
    #[serde(other)]
    Unknown,
}

impl DmnStatus {
    /// Upper-case wire spelling used in the advanceResponseChecksum message.
    fn as_checksum_str(&self) -> Option<&'static str> {
        match self {
            Self::Success => Some("SUCCESS"),
            Self::Approved => Some("APPROVED"),
            Self::Error => Some("ERROR"),
            Self::Pending => Some("PENDING"),
            Self::Declined => Some("DECLINED"),
            Self::Unknown => None,
        }
    }
}

/// Status of the API call behind the DMN (`ppp_status`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DmnApiTransactionStatus {
    Ok,
    Fail,
    Pending,
    #[serde(other)]
    Unknown,
}

/// Payment Direct Merchant Notification (DMN).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDmnNotification {
    #[serde(rename = "ppp_status")]
    pub ppp_status: DmnApiTransactionStatus,
    #[serde(rename = "PPP_TransactionID")]
    pub ppp_transaction_id: String,
    pub total_amount: String,
    pub currency: String,
    #[serde(rename = "TransactionID")]
    pub transaction_id: Option<String>,
    pub transaction_link_id: Option<String>,
    #[serde(rename = "Status")]
    pub status: Option<DmnStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    #[serde(rename = "ErrCode")]
    pub err_code: Option<String>,
    #[serde(rename = "Reason")]
    pub reason: Option<String>,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "user_token_id")]
    pub user_token_id: Option<Secret<String>>,
    #[serde(rename = "payment_method")]
    pub payment_method: Option<String>,
    pub response_time_stamp: String,
    #[serde(rename = "merchant_id")]
    pub merchant_id: Option<Secret<String>>,
    #[serde(rename = "merchant_site_id")]
    pub merchant_site_id: Option<Secret<String>>,
    pub advance_response_checksum: Option<String>,
    pub product_id: Option<String>,
    pub merchant_advice_code: Option<String>,
    #[serde(rename = "AuthCode")]
    pub auth_code: Option<String>,
    pub acquirer_bank: Option<String>,
    pub client_request_id: Option<String>,
    pub related_transaction_id: Option<String>,
}

/// Chargeback notification from the Nuvei Control Panel.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChargebackNotification {
    pub client_name: Option<String>,
    #[serde(rename = "EventDateUTC")]
    pub event_date_utc: Option<String>,
    pub event_correlation_id: Option<String>,
    pub chargeback: ChargebackData,
    pub transaction_details: ChargebackTransactionDetails,
    pub event_id: Option<String>,
    pub processing_entity_type: Option<String>,
    pub processing_entity_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChargebackData {
    pub date: Option<String>,
    pub chargeback_status_category: Option<ChargebackStatusCategory>,
    #[serde(rename = "Type")]
    pub webhook_type: Option<ChargebackType>,
    pub status: Option<String>,
    pub amount: common_utils::types::FloatMajorUnit,
    pub currency: String,
    pub reported_amount: common_utils::types::FloatMajorUnit,
    pub reported_currency: String,
    pub chargeback_reason: Option<String>,
    pub chargeback_reason_category: Option<String>,
    pub reason_message: Option<String>,
    pub dispute_id: Option<String>,
    pub dispute_due_date: Option<String>,
    pub dispute_event_id: Option<String>,
    pub dispute_unified_status_code: Option<DisputeUnifiedStatusCode>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ChargebackTransactionDetails {
    pub transaction_id: i64,
    pub transaction_date: Option<String>,
    pub client_unique_id: Option<String>,
    pub acquirer_name: Option<String>,
    pub masked_card_number: Option<String>,
    pub arn: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChargebackType {
    Chargeback,
    Retrieval,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChargebackStatusCategory {
    #[serde(rename = "Regular")]
    Regular,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "Duplicate")]
    Duplicate,
    #[serde(rename = "RDR-Refund")]
    RdrRefund,
    #[serde(rename = "Soft_CB")]
    SoftCb,
    #[serde(other)]
    Unknown,
}

/// Nuvei's unified dispute status code (hyperswitch `DisputeUnifiedStatusCode`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisputeUnifiedStatusCode {
    #[serde(rename = "FC")]
    FirstChargebackInitiatedByIssuer,
    #[serde(rename = "CC")]
    CreditChargebackInitiatedByIssuer,
    #[serde(rename = "CC-A-ACPT")]
    CreditChargebackAcceptedAutomatically,
    #[serde(rename = "FC-A-EPRD")]
    FirstChargebackNoResponseExpired,
    #[serde(rename = "FC-M-ACPT")]
    FirstChargebackAcceptedByMerchant,
    #[serde(rename = "FC-A-ACPT")]
    FirstChargebackAcceptedAutomatically,
    #[serde(rename = "FC-A-ACPT-MCOLL")]
    FirstChargebackAcceptedAutomaticallyMcoll,
    #[serde(rename = "FC-M-PART")]
    FirstChargebackPartiallyAcceptedByMerchant,
    #[serde(rename = "FC-M-PART-EXP")]
    FirstChargebackPartiallyAcceptedByMerchantExpired,
    #[serde(rename = "FC-M-RJCT")]
    FirstChargebackRejectedByMerchant,
    #[serde(rename = "FC-M-RJCT-EXP")]
    FirstChargebackRejectedByMerchantExpired,
    #[serde(rename = "FC-A-RJCT")]
    FirstChargebackRejectedAutomatically,
    #[serde(rename = "FC-A-RJCT-EXP")]
    FirstChargebackRejectedAutomaticallyExpired,
    #[serde(rename = "IPA")]
    PreArbitrationInitiatedByIssuer,
    #[serde(rename = "MPA-I-ACPT")]
    MerchantPreArbitrationAcceptedByIssuer,
    #[serde(rename = "MPA-I-RJCT")]
    MerchantPreArbitrationRejectedByIssuer,
    #[serde(rename = "MPA-I-PART")]
    MerchantPreArbitrationPartiallyAcceptedByIssuer,
    #[serde(rename = "FC-CLSD-MF")]
    FirstChargebackClosedMerchantFavour,
    #[serde(rename = "FC-CLSD-CHF")]
    FirstChargebackClosedCardholderFavour,
    #[serde(rename = "FC-CLSD-RCL")]
    FirstChargebackClosedRecall,
    #[serde(rename = "FC-I-RCL")]
    FirstChargebackRecalledByIssuer,
    #[serde(rename = "PA-CLSD-MF")]
    PreArbitrationClosedMerchantFavour,
    #[serde(rename = "PA-CLSD-CHF")]
    PreArbitrationClosedCardholderFavour,
    #[serde(rename = "RDR")]
    Rdr,
    #[serde(rename = "FC-SPCSE")]
    FirstChargebackDisputeResponseNotAllowed,
    #[serde(rename = "MCC")]
    McCollaborationInitiatedByIssuer,
    #[serde(rename = "MCC-A-RJCT")]
    McCollaborationPreviouslyRefundedAuto,
    #[serde(rename = "MCC-M-ACPT")]
    McCollaborationRefundedByMerchant,
    #[serde(rename = "MCC-EXPR")]
    McCollaborationExpired,
    #[serde(rename = "MCC-M-RJCT")]
    McCollaborationRejectedByMerchant,
    #[serde(rename = "MCC-A-ACPT")]
    McCollaborationAutomaticAccept,
    #[serde(rename = "MCC-CLSD-MF")]
    McCollaborationClosedMerchantFavour,
    #[serde(rename = "MCC-CLSD-CHF")]
    McCollaborationClosedCardholderFavour,
    #[serde(rename = "INQ")]
    InquiryInitiatedByIssuer,
    #[serde(rename = "INQ-M-RSP")]
    InquiryRespondedByMerchant,
    #[serde(rename = "INQ-EXPR")]
    InquiryExpired,
    #[serde(rename = "INQ-A-RJCT")]
    InquiryAutomaticallyRejected,
    #[serde(rename = "INQ-A-CNLD")]
    InquiryCancelledAfterRefund,
    #[serde(rename = "INQ-M-RFND")]
    InquiryAcceptedFullRefund,
    #[serde(rename = "INQ-M-P-RFND")]
    InquiryPartialAcceptedPartialRefund,
    #[serde(rename = "INQ-UPD")]
    InquiryUpdated,
    #[serde(rename = "IPA-M-ACPT")]
    PreArbitrationAcceptedByMerchant,
    #[serde(rename = "IPA-M-PART")]
    PreArbitrationPartiallyAcceptedByMerchant,
    #[serde(rename = "IPA-M-PART-EXP")]
    PreArbitrationPartiallyAcceptedByMerchantExpired,
    #[serde(rename = "IPA-M-RJCT")]
    PreArbitrationRejectedByMerchant,
    #[serde(rename = "IPA-M-RJCT-EXP")]
    PreArbitrationRejectedByMerchantExpired,
    #[serde(rename = "IPA-A-ACPT")]
    PreArbitrationAutomaticallyAcceptedByMerchant,
    #[serde(rename = "PA-CLSD-RC")]
    PreArbitrationClosedRecall,
    #[serde(rename = "IPAR-M-ACPT")]
    RejectedPreArbAcceptedByMerchant,
    #[serde(rename = "IPAR-A-ACPT")]
    RejectedPreArbExpiredAutoAccepted,
    #[serde(rename = "CC-I-RCLL")]
    CreditChargebackRecalledByIssuer,
    #[serde(other)]
    Unknown,
}

/// Parse a DMN body: form-encoded first (payment DMN), JSON fallback
/// (chargeback) — hyperswitch `get_webhook_object_from_body`.
pub fn parse_nuvei_webhook(body: &[u8]) -> Result<NuveiWebhook, Report<WebhookError>> {
    match serde_urlencoded::from_bytes::<NuveiWebhook>(body) {
        Ok(webhook) => Ok(webhook),
        Err(_) => serde_json::from_slice::<NuveiWebhook>(body)
            .change_context(WebhookError::WebhookBodyDecodingFailed),
    }
}

/// A `payout_`-prefixed clientRequestId marks a payout DMN (hyperswitch
/// `has_payout_prefix`); payouts are out of scope and refused.
pub fn is_payout_dmn(notification: &PaymentDmnNotification) -> bool {
    notification
        .client_request_id
        .as_deref()
        .is_some_and(|id| id.starts_with("payout_"))
}

/// advanceResponseChecksum message: secret + totalAmount + currency +
/// responseTimeStamp + PPP_TransactionID + UPPERCASE(Status) + (productId or "NA").
pub fn payment_dmn_checksum_message(
    notification: &PaymentDmnNotification,
    secret: &str,
) -> Result<String, Report<WebhookError>> {
    let status = notification
        .status
        .as_ref()
        .and_then(DmnStatus::as_checksum_str)
        .ok_or(WebhookError::WebhookSourceVerificationFailed)
        .attach_printable("Nuvei DMN Status missing or unrecognised")?;
    Ok([
        secret,
        notification.total_amount.as_str(),
        notification.currency.as_str(),
        notification.response_time_stamp.as_str(),
        notification.ppp_transaction_id.as_str(),
        status,
        notification.product_id.as_deref().unwrap_or("NA"),
    ]
    .concat())
}

/// A JSON object whose members keep their document order and raw text.
struct OrderedRawObject(Vec<Box<serde_json::value::RawValue>>);

impl<'de> Deserialize<'de> for OrderedRawObject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = OrderedRawObject;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some((_key, value)) =
                    map.next_entry::<String, Box<serde_json::value::RawValue>>()?
                {
                    values.push(value);
                }
                Ok(OrderedRawObject(values))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

/// Append every JSON leaf value of `raw` in document order: strings unquoted,
/// null as "", numbers and booleans as their raw JSON text.
fn push_json_leaf_values(
    raw: &serde_json::value::RawValue,
    out: &mut String,
) -> Result<(), serde_json::Error> {
    let text = raw.get().trim();
    match text.as_bytes().first() {
        Some(b'{') => {
            let object: OrderedRawObject = serde_json::from_str(text)?;
            for value in &object.0 {
                push_json_leaf_values(value, out)?;
            }
        }
        Some(b'[') => {
            let items: Vec<Box<serde_json::value::RawValue>> = serde_json::from_str(text)?;
            for item in &items {
                push_json_leaf_values(item, out)?;
            }
        }
        Some(b'"') => out.push_str(&serde_json::from_str::<String>(text)?),
        Some(_) if text == "null" => {}
        Some(_) | None => out.push_str(text),
    }
    Ok(())
}

/// Chargeback DMN checksum message: secret + the body's JSON leaf values
/// concatenated in document order.
pub fn chargeback_checksum_message(
    body: &[u8],
    secret: &str,
) -> Result<String, Report<WebhookError>> {
    let raw: Box<serde_json::value::RawValue> =
        serde_json::from_slice(body).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let mut message = secret.to_string();
    push_json_leaf_values(&raw, &mut message)
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    Ok(message)
}

/// Payment/refund DMN -> event (hyperswitch `map_notification_to_event`).
pub fn map_notification_to_event(
    status: &DmnStatus,
    transaction_type: &NuveiTransactionType,
) -> Result<EventType, Report<WebhookError>> {
    match (status, transaction_type) {
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Auth) => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Sale) => {
            Ok(EventType::PaymentIntentSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Settle) => {
            Ok(EventType::PaymentIntentCaptureSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Void) => {
            Ok(EventType::PaymentIntentCancelled)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Credit) => {
            Ok(EventType::RefundSuccess)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Auth) => {
            Ok(EventType::PaymentIntentAuthorizationFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Sale) => {
            Ok(EventType::PaymentIntentFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Settle) => {
            Ok(EventType::PaymentIntentCaptureFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Void) => {
            Ok(EventType::PaymentIntentCancelFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Credit) => {
            Ok(EventType::RefundFailure)
        }
        (
            DmnStatus::Pending,
            NuveiTransactionType::Auth | NuveiTransactionType::Sale | NuveiTransactionType::Settle,
        ) => Ok(EventType::PaymentIntentProcessing),
        (
            DmnStatus::Success | DmnStatus::Approved | DmnStatus::Error | DmnStatus::Declined,
            NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        )
        | (
            DmnStatus::Pending,
            NuveiTransactionType::Credit
            | NuveiTransactionType::Void
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        )
        | (
            DmnStatus::Unknown,
            NuveiTransactionType::Auth
            | NuveiTransactionType::Sale
            | NuveiTransactionType::Settle
            | NuveiTransactionType::Void
            | NuveiTransactionType::Credit
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        ) => Err(WebhookError::WebhookEventTypeNotFound.into()),
    }
}

/// Unified status code -> dispute status (hyperswitch
/// `map_dispute_notification_to_event`, first half; each hyperswitch
/// `Dispute*` event is the same-named `DisputeStatus`).
fn dispute_status_from_unified_code(
    code: &DisputeUnifiedStatusCode,
) -> Option<common_enums::DisputeStatus> {
    use common_enums::DisputeStatus;
    use DisputeUnifiedStatusCode as C;
    match code {
        C::FirstChargebackInitiatedByIssuer
        | C::CreditChargebackInitiatedByIssuer
        | C::McCollaborationInitiatedByIssuer
        | C::FirstChargebackClosedRecall
        | C::InquiryInitiatedByIssuer => Some(DisputeStatus::DisputeOpened),
        C::CreditChargebackAcceptedAutomatically
        | C::FirstChargebackAcceptedAutomatically
        | C::FirstChargebackAcceptedAutomaticallyMcoll
        | C::FirstChargebackAcceptedByMerchant
        | C::FirstChargebackDisputeResponseNotAllowed
        | C::Rdr
        | C::McCollaborationRefundedByMerchant
        | C::McCollaborationAutomaticAccept
        | C::InquiryAcceptedFullRefund
        | C::PreArbitrationAcceptedByMerchant
        | C::PreArbitrationPartiallyAcceptedByMerchant
        | C::PreArbitrationAutomaticallyAcceptedByMerchant
        | C::RejectedPreArbAcceptedByMerchant
        | C::RejectedPreArbExpiredAutoAccepted => Some(DisputeStatus::DisputeAccepted),
        C::FirstChargebackNoResponseExpired
        | C::FirstChargebackPartiallyAcceptedByMerchant
        | C::FirstChargebackClosedCardholderFavour
        | C::PreArbitrationClosedCardholderFavour
        | C::McCollaborationClosedCardholderFavour => Some(DisputeStatus::DisputeLost),
        C::FirstChargebackRejectedByMerchant
        | C::FirstChargebackRejectedAutomatically
        | C::PreArbitrationInitiatedByIssuer
        | C::MerchantPreArbitrationRejectedByIssuer
        | C::InquiryRespondedByMerchant
        | C::PreArbitrationRejectedByMerchant => Some(DisputeStatus::DisputeChallenged),
        C::FirstChargebackRejectedAutomaticallyExpired
        | C::FirstChargebackPartiallyAcceptedByMerchantExpired
        | C::FirstChargebackRejectedByMerchantExpired
        | C::McCollaborationExpired
        | C::InquiryExpired
        | C::PreArbitrationPartiallyAcceptedByMerchantExpired
        | C::PreArbitrationRejectedByMerchantExpired => Some(DisputeStatus::DisputeExpired),
        C::MerchantPreArbitrationAcceptedByIssuer
        | C::MerchantPreArbitrationPartiallyAcceptedByIssuer
        | C::FirstChargebackClosedMerchantFavour
        | C::McCollaborationClosedMerchantFavour
        | C::PreArbitrationClosedMerchantFavour => Some(DisputeStatus::DisputeWon),
        C::FirstChargebackRecalledByIssuer
        | C::InquiryCancelledAfterRefund
        | C::PreArbitrationClosedRecall
        | C::CreditChargebackRecalledByIssuer => Some(DisputeStatus::DisputeCancelled),
        C::McCollaborationPreviouslyRefundedAuto
        | C::McCollaborationRejectedByMerchant
        | C::InquiryAutomaticallyRejected
        | C::InquiryPartialAcceptedPartialRefund
        | C::InquiryUpdated
        | C::Unknown => None,
    }
}

/// Chargeback -> dispute status: unified status code first, then
/// ChargebackStatusCategory (hyperswitch `map_dispute_notification_to_event`).
pub fn get_dispute_status(
    chargeback: &ChargebackData,
) -> Result<common_enums::DisputeStatus, Report<WebhookError>> {
    use common_enums::DisputeStatus;
    chargeback
        .dispute_unified_status_code
        .as_ref()
        .and_then(dispute_status_from_unified_code)
        .or(match chargeback.chargeback_status_category.as_ref() {
            Some(ChargebackStatusCategory::Cancelled)
            | Some(ChargebackStatusCategory::Duplicate) => Some(DisputeStatus::DisputeCancelled),
            Some(ChargebackStatusCategory::RdrRefund) => Some(DisputeStatus::DisputeAccepted),
            Some(ChargebackStatusCategory::Regular)
            | Some(ChargebackStatusCategory::SoftCb)
            | Some(ChargebackStatusCategory::Unknown)
            | None => None,
        })
        .ok_or_else(|| WebhookError::WebhookEventTypeNotFound.into())
}

/// Dispute status -> webhook event.
pub fn dispute_event_from_status(status: common_enums::DisputeStatus) -> EventType {
    match status {
        common_enums::DisputeStatus::DisputeOpened => EventType::DisputeOpened,
        common_enums::DisputeStatus::DisputeExpired => EventType::DisputeExpired,
        common_enums::DisputeStatus::DisputeAccepted => EventType::DisputeAccepted,
        common_enums::DisputeStatus::DisputeCancelled => EventType::DisputeCancelled,
        common_enums::DisputeStatus::DisputeChallenged => EventType::DisputeChallenged,
        common_enums::DisputeStatus::DisputeWon => EventType::DisputeWon,
        common_enums::DisputeStatus::DisputeLost => EventType::DisputeLost,
    }
}

/// Unified status code -> dispute stage (hyperswitch
/// `From<DisputeUnifiedStatusCode> for DisputeStage`).
fn dispute_stage_from_unified_code(
    code: &DisputeUnifiedStatusCode,
) -> Option<common_enums::DisputeStage> {
    use common_enums::DisputeStage;
    use DisputeUnifiedStatusCode as C;
    match code {
        C::Rdr
        | C::InquiryInitiatedByIssuer
        | C::InquiryRespondedByMerchant
        | C::InquiryExpired
        | C::InquiryAutomaticallyRejected
        | C::InquiryCancelledAfterRefund
        | C::InquiryAcceptedFullRefund
        | C::InquiryPartialAcceptedPartialRefund
        | C::InquiryUpdated => Some(DisputeStage::PreDispute),
        C::FirstChargebackInitiatedByIssuer
        | C::CreditChargebackInitiatedByIssuer
        | C::FirstChargebackNoResponseExpired
        | C::FirstChargebackAcceptedByMerchant
        | C::FirstChargebackAcceptedAutomatically
        | C::FirstChargebackAcceptedAutomaticallyMcoll
        | C::FirstChargebackPartiallyAcceptedByMerchant
        | C::FirstChargebackPartiallyAcceptedByMerchantExpired
        | C::FirstChargebackRejectedByMerchant
        | C::FirstChargebackRejectedByMerchantExpired
        | C::FirstChargebackRejectedAutomatically
        | C::FirstChargebackRejectedAutomaticallyExpired
        | C::FirstChargebackClosedMerchantFavour
        | C::FirstChargebackClosedCardholderFavour
        | C::FirstChargebackClosedRecall
        | C::FirstChargebackRecalledByIssuer
        | C::FirstChargebackDisputeResponseNotAllowed
        | C::McCollaborationInitiatedByIssuer
        | C::McCollaborationPreviouslyRefundedAuto
        | C::McCollaborationRefundedByMerchant
        | C::McCollaborationExpired
        | C::McCollaborationRejectedByMerchant
        | C::McCollaborationAutomaticAccept
        | C::McCollaborationClosedMerchantFavour
        | C::McCollaborationClosedCardholderFavour
        | C::CreditChargebackAcceptedAutomatically => Some(DisputeStage::Dispute),
        C::PreArbitrationInitiatedByIssuer
        | C::MerchantPreArbitrationAcceptedByIssuer
        | C::MerchantPreArbitrationRejectedByIssuer
        | C::MerchantPreArbitrationPartiallyAcceptedByIssuer
        | C::PreArbitrationClosedMerchantFavour
        | C::PreArbitrationClosedCardholderFavour
        | C::PreArbitrationAcceptedByMerchant
        | C::PreArbitrationPartiallyAcceptedByMerchant
        | C::PreArbitrationPartiallyAcceptedByMerchantExpired
        | C::PreArbitrationRejectedByMerchant
        | C::PreArbitrationRejectedByMerchantExpired
        | C::PreArbitrationAutomaticallyAcceptedByMerchant
        | C::PreArbitrationClosedRecall
        | C::RejectedPreArbAcceptedByMerchant
        | C::RejectedPreArbExpiredAutoAccepted => Some(DisputeStage::PreArbitration),
        // hyperswitch DisputeReversal: the UCS DisputeStage (proto payment.proto
        // DisputeStage) has no reversal stage; the reversal stays in the
        // chargeback's Dispute stage and its status is DisputeCancelled.
        C::CreditChargebackRecalledByIssuer => Some(DisputeStage::Dispute),
        C::Unknown => None,
    }
}

/// Dispute stage: unified status code, then Type, then ChargebackStatusCategory
/// (hyperswitch `get_dispute_stage`).
pub fn get_dispute_stage(
    chargeback: &ChargebackData,
) -> Result<common_enums::DisputeStage, Report<WebhookError>> {
    use common_enums::DisputeStage;
    chargeback
        .dispute_unified_status_code
        .as_ref()
        .and_then(dispute_stage_from_unified_code)
        .or(match chargeback.webhook_type {
            Some(ChargebackType::Retrieval) => Some(DisputeStage::PreDispute),
            Some(ChargebackType::Chargeback) | Some(ChargebackType::Unknown) | None => None,
        })
        .or(match chargeback.chargeback_status_category {
            Some(ChargebackStatusCategory::Cancelled)
            | Some(ChargebackStatusCategory::Duplicate)
            | Some(ChargebackStatusCategory::Regular) => Some(DisputeStage::Dispute),
            Some(ChargebackStatusCategory::RdrRefund) => Some(DisputeStage::PreDispute),
            Some(ChargebackStatusCategory::SoftCb) => Some(DisputeStage::PreArbitration),
            Some(ChargebackStatusCategory::Unknown) | None => None,
        })
        .ok_or_else(|| WebhookError::WebhookEventTypeNotFound.into())
}

impl PaymentDmnNotification {
    /// DMN -> getTransactionDetails shape (hyperswitch
    /// `From<PaymentDmnNotification> for NuveiTransactionSyncResponse`), so the
    /// shared status and error helpers apply unchanged.
    fn to_payments_response(&self) -> NuveiPaymentsResponse {
        let sync = NuveiSyncResponse {
            payment_option: None,
            partial_approval: None,
            transaction_details: Some(NuveiTransactionDetails {
                gw_error_code: self
                    .reason_code
                    .as_ref()
                    .and_then(|code| code.parse::<i64>().ok()),
                gw_error_reason: self.reason.clone(),
                gw_extended_error_code: None,
                transaction_id: self.transaction_id.clone(),
                transaction_status: self.status.as_ref().map(|status| match status {
                    DmnStatus::Success | DmnStatus::Approved => NuveiTransactionStatus::Approved,
                    DmnStatus::Declined => NuveiTransactionStatus::Declined,
                    DmnStatus::Pending => NuveiTransactionStatus::Pending,
                    DmnStatus::Error => NuveiTransactionStatus::Error,
                    DmnStatus::Unknown => NuveiTransactionStatus::Unknown,
                }),
                transaction_type: self.transaction_type.clone(),
                auth_code: self.auth_code.clone(),
                processed_amount: None,
                processed_currency: None,
                acquiring_bank_name: self.acquirer_bank.clone(),
            }),
            transaction_link_id: self.transaction_link_id.clone(),
            client_unique_id: None,
            status: match self.ppp_status {
                DmnApiTransactionStatus::Ok => NuveiPaymentStatus::Success,
                DmnApiTransactionStatus::Fail => NuveiPaymentStatus::Failed,
                DmnApiTransactionStatus::Pending => NuveiPaymentStatus::Processing,
                DmnApiTransactionStatus::Unknown => NuveiPaymentStatus::Unknown,
            },
            err_code: self
                .err_code
                .as_ref()
                .and_then(|code| code.parse::<i64>().ok()),
            reason: self.reason.clone(),
            merchant_id: self.merchant_id.clone(),
            merchant_site_id: self.merchant_site_id.clone(),
            client_request_id: self.client_request_id.clone(),
            merchant_advice_code: self.merchant_advice_code.clone(),
        };
        NuveiPaymentsResponse {
            merchant_advice_code: self.merchant_advice_code.clone(),
            ..sync.to_payments_response()
        }
    }
}

/// Payment DMN -> webhook payment details: status via the shared
/// get_nuvei_payment_status, error code/message via build_nuvei_error_response.
pub fn build_payment_webhook_details(
    notification: &PaymentDmnNotification,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
    let response = notification.to_payments_response();
    // Only the zero-amount check of get_nuvei_payment_status reads the amount.
    let amount = notification
        .total_amount
        .parse::<f64>()
        .ok()
        .filter(|amount| *amount == 0.0)
        .map(|_| 0);
    let status = get_nuvei_payment_status(
        amount,
        response.transaction_type.as_ref(),
        response.transaction_status.as_ref(),
        &response.status,
    );
    let error = build_nuvei_error_response(&response, 200, None);
    let connector_transaction_id = notification
        .transaction_id
        .clone()
        .ok_or(WebhookError::WebhookReferenceIdNotFound)?;

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(connector_transaction_id)),
        status,
        connector_response_reference_id: None,
        connector_request_reference_id: None,
        mandate_reference: None,
        error_code: error.as_ref().map(|error| error.code.clone()),
        error_message: error.as_ref().map(|error| error.message.clone()),
        error_reason: error.and_then(|error| error.reason),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: None,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// Refund (Credit) DMN -> webhook refund details, with the Refund/RSync
/// status map.
pub fn build_refund_webhook_details(
    notification: &PaymentDmnNotification,
    raw_body: &[u8],
) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
    let response = notification.to_payments_response();
    let status = match response.transaction_status {
        Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
            common_enums::RefundStatus::Failure
        }
        Some(NuveiTransactionStatus::Pending)
        | Some(NuveiTransactionStatus::Processing)
        | Some(NuveiTransactionStatus::Redirect)
        | Some(NuveiTransactionStatus::Unknown) => common_enums::RefundStatus::Pending,
        None => common_enums::RefundStatus::Failure,
    };
    let error = build_nuvei_error_response(&response, 200, None);
    let connector_refund_id = notification
        .transaction_id
        .clone()
        .ok_or(WebhookError::WebhookReferenceIdNotFound)?;

    Ok(RefundWebhookDetailsResponse {
        connector_refund_id: Some(connector_refund_id),
        merchant_transaction_id: None,
        status,
        connector_response_reference_id: None,
        error_code: error.as_ref().map(|error| error.code.clone()),
        error_message: error.map(|error| error.message),
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}
