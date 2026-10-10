use common_utils::{
    consts, pii,
    request::Method,
    types::{FloatMajorUnit, SemanticVersion, StringMajorUnit},
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync,
        PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, ContinueRedirectionResponse, EventType,
        MandateReference, MandateReferenceId,
        NuveiClientAuthenticationResponse as NuveiClientAuthenticationResponseDomain,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookReference, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
        SetupMandateRequestData, WebhookResourceReference,
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
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

use super::NuveiRouterData;
use crate::{types::ResponseRouterData, utils::safe_base64_decode};
use domain_types::errors::{
    ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError,
};

// Nuvei's APM (Alternative Payment Method) identifier for ACH. Required literal
// per Nuvei's API; reused by both BankTransfer::AchBankTransfer and
// BankDebit::AchBankDebit. See https://docs.nuvei.com/documentation/us-and-canada-guides/ach/
const NUVEI_ACH_PAYMENT_METHOD: &str = "apmgw_ACH";

// clientUniqueId is String(45) on every Nuvei request; a longer merchant
// reference is refused rather than truncated (mirrors hyperswitch).
const NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH: usize = 45;
// dynamicDescriptor limits: merchantName is cut to 25 characters (gateway
// filter 1181 rejects longer names), merchantPhone is String(13).
const NUVEI_MAX_DESCRIPTOR_NAME_LENGTH: usize = 25;
const NUVEI_MAX_DESCRIPTOR_PHONE_LENGTH: usize = 13;
// gwErrorReason Nuvei returns for a request it rejected without a
// DECLINED / ERROR transactionStatus.
const NUVEI_MISSING_ARGUMENT_REASON: &str = "Missing argument";
// gwErrorCode of every gateway filter error; the specific code is in
// gwExtendedErrorCode.
const NUVEI_FILTER_ERROR_CODE: i64 = -1100;
// gwErrorCode of a declined transaction.
const NUVEI_DECLINE_CODE: i64 = -1;
// errCode of /getTransactionDetails.do for "No transaction details returned
// for the provided id": the transaction is not readable yet, which happens
// when the lookup follows the payment call closely.
const NUVEI_TRANSACTION_NOT_FOUND_ERR_CODE: i64 = 9146;

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
                context: Default::default(),
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
    pub session_token: Option<String>,
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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_unique_id: Option<String>,
    /// "0" marks the initial CIT of a stored-credential series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rebilling: Option<String>,
    /// transactionId of the 3DS payment this call completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transaction_id: Option<String>,
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: TransactionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_partial_approval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_moto: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<NuveiItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_details: Option<NuveiAmountDetails>,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<NuveiShippingAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDynamicDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_phone: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub enum NuveiItemType {
    #[serde(rename = "physical")]
    Physical,
    #[serde(rename = "digital")]
    Digital,
    #[serde(rename = "Shipping_fee")]
    ShippingFee,
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
    pub total_discount: Option<StringMajorUnit>,
    pub total_handling: Option<StringMajorUnit>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiShippingAddress {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub address_line2: Option<Secret<String>>,
    pub address_line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<common_enums::CountryAlpha2>,
    pub email: Option<pii::Email>,
    pub phone: Option<Secret<String>>,
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
    pub three_d: Option<NuveiThreeD>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_credentials: Option<NuveiStoredCredentials>,
}

/// threeD block of the card object. Only the external-MPI form is sent on
/// /payment.do by Authorize.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeD {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_mpi: Option<NuveiExternalMpi>,
}

/// Result of a 3DS authentication done by a third-party MPI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalMpi {
    pub eci: String,
    pub cavv: Secret<String>,
    #[serde(rename = "dsTransID", skip_serializing_if = "Option::is_none")]
    pub ds_trans_id: Option<String>,
}

/// storedCredentialsMode: "0" first use, "1" subsequent use of
/// merchant-stored credentials.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiStoredCredentials {
    pub stored_credentials_mode: String,
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
                context: Default::default(),
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
                context: Default::default(),
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
                context: Default::default(),
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
    pub country: String,
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

/// Build a Nuvei `billingAddress` block from `PaymentFlowData`. Returns
/// `None` if either of the two fields Nuvei requires (email + country)
/// is missing, so MIT flows can treat a missing block as "skip" while
/// CIT flows `.ok_or(...)` a specific error.
fn get_billing_address(
    resource_data: &PaymentFlowData,
    fallback_email: Option<pii::Email>,
) -> Option<NuveiBillingAddress> {
    let email = resource_data
        .get_optional_billing_email()
        .or(fallback_email)?;
    let country = resource_data.get_optional_billing_country()?;
    let address_line3 = resource_data
        .get_optional_billing()
        .and_then(|billing| billing.address.as_ref())
        .and_then(|addr| addr.line3.clone());
    Some(NuveiBillingAddress {
        email,
        country: country.to_string(),
        first_name: resource_data.get_optional_billing_first_name(),
        last_name: resource_data.get_optional_billing_last_name(),
        phone: resource_data.get_optional_billing_phone_number(),
        city: resource_data.get_optional_billing_city(),
        address: resource_data.get_optional_billing_line1(),
        address_line2: resource_data.get_optional_billing_line2(),
        address_line3,
        zip: resource_data.get_optional_billing_zip(),
        state: resource_data.get_optional_billing_state(),
    })
}

// Payment Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPaymentResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i64>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub merchant_advice_code: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub payment_method_error_code: Option<String>,
    pub payment_method_error_reason: Option<String>,
    /// Network transaction id (NTID) of the scheme, used by later MITs.
    pub external_scheme_transaction_id: Option<Secret<String>>,
    pub transaction_link_id: Option<String>,
    pub partial_approval: Option<NuveiPartialApproval>,
    pub auth_code: Option<String>,
    pub session_token: Option<Secret<String>>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    #[serde(rename = "paymentOption")]
    pub payment_option: Option<PaymentOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentOption {
    #[serde(rename = "redirectUrl")]
    pub redirect_url: Option<String>,
    /// Stored payment option (UPO) id, returned when the request carried a userTokenId.
    pub user_payment_option_id: Option<Secret<String>>,
    pub card: Option<NuveiResponseCard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseCard {
    pub avs_code: Option<String>,
    pub cvv2_reply: Option<String>,
    #[serde(alias = "brand")]
    pub card_brand: Option<String>,
    pub three_d: Option<NuveiResponseThreeD>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseThreeD {
    pub acs_url: Option<String>,
    pub c_req: Option<Secret<String>>,
    // The sandbox sends "true" / "false" as strings; the docs show a boolean.
    #[serde(default, deserialize_with = "str_or_bool")]
    pub v2supported: Option<String>,
    pub version: Option<String>,
    pub server_trans_id: Option<String>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    pub result: Option<String>,
    pub flow: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub three_d_reason_id: Option<String>,
    pub three_d_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiPartialApproval {
    pub requested_amount: Option<StringMajorUnit>,
    pub requested_currency: Option<common_enums::Currency>,
    pub processed_amount: Option<StringMajorUnit>,
    pub processed_currency: Option<common_enums::Currency>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiPaymentStatus {
    Success,
    Failed,
    Error,
    Processing,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    Processing,
    #[serde(other)]
    Unknown,
}

// transactionType of a response. The API reference spells the values both
// capitalised ("Sale") and lower-case ("sale"), so both are accepted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NuveiTransactionType {
    #[serde(alias = "auth", alias = "AUTH")]
    Auth,
    #[serde(alias = "sale", alias = "SALE")]
    Sale,
    #[serde(alias = "settle", alias = "SETTLE")]
    Settle,
    #[serde(alias = "void", alias = "VOID")]
    Void,
    #[serde(alias = "credit", alias = "CREDIT")]
    Credit,
    #[serde(alias = "refund", alias = "REFUND")]
    Refund,
    #[serde(alias = "initAuth3D", alias = "initauth3d", alias = "INITAUTH3D")]
    InitAuth3D,
    #[serde(alias = "auth3D", alias = "auth3d", alias = "AUTH3D")]
    Auth3D,
    #[serde(alias = "sale3D", alias = "sale3d", alias = "SALE3D")]
    Sale3D,
    #[serde(other)]
    Unknown,
}

/// Attempt status of a /payment-type response, following the hyperswitch
/// table (spec: Status Mappings). `zero_amount` is true when the request
/// amount was 0: an approved zero-amount Auth is a completed verification.
pub(super) fn get_nuvei_payment_status(
    zero_amount: bool,
    transaction_type: Option<NuveiTransactionType>,
    transaction_status: Option<&NuveiTransactionStatus>,
    status: &NuveiPaymentStatus,
) -> common_enums::AttemptStatus {
    let status_without_transaction = || match status {
        NuveiPaymentStatus::Failed | NuveiPaymentStatus::Error => {
            common_enums::AttemptStatus::Failure
        }
        NuveiPaymentStatus::Success
        | NuveiPaymentStatus::Processing
        | NuveiPaymentStatus::Pending
        | NuveiPaymentStatus::Unknown => common_enums::AttemptStatus::Pending,
    };

    if zero_amount && transaction_type == Some(NuveiTransactionType::Auth) {
        return match transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::AttemptStatus::AuthorizationFailed
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Unknown) => common_enums::AttemptStatus::Pending,
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            None => status_without_transaction(),
        };
    }

    match transaction_status {
        Some(NuveiTransactionStatus::Approved) => match transaction_type {
            Some(NuveiTransactionType::Auth) | Some(NuveiTransactionType::InitAuth3D) => {
                common_enums::AttemptStatus::Authorized
            }
            Some(NuveiTransactionType::Sale) | Some(NuveiTransactionType::Settle) => {
                common_enums::AttemptStatus::Charged
            }
            Some(NuveiTransactionType::Void) => common_enums::AttemptStatus::Voided,
            Some(NuveiTransactionType::Auth3D) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            // An approval whose type is unknown or absent is not treated as captured.
            Some(NuveiTransactionType::Credit)
            | Some(NuveiTransactionType::Refund)
            | Some(NuveiTransactionType::Sale3D)
            | Some(NuveiTransactionType::Unknown)
            | None => common_enums::AttemptStatus::Pending,
        },
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
            match transaction_type {
                Some(NuveiTransactionType::Auth) => {
                    common_enums::AttemptStatus::AuthorizationFailed
                }
                Some(NuveiTransactionType::Void) => common_enums::AttemptStatus::VoidFailed,
                Some(NuveiTransactionType::Auth3D) | Some(NuveiTransactionType::InitAuth3D) => {
                    common_enums::AttemptStatus::AuthenticationFailed
                }
                Some(NuveiTransactionType::Sale)
                | Some(NuveiTransactionType::Settle)
                | Some(NuveiTransactionType::Credit)
                | Some(NuveiTransactionType::Refund)
                | Some(NuveiTransactionType::Sale3D)
                | Some(NuveiTransactionType::Unknown)
                | None => common_enums::AttemptStatus::Failure,
            }
        }
        Some(NuveiTransactionStatus::Pending)
        | Some(NuveiTransactionStatus::Processing)
        | Some(NuveiTransactionStatus::Unknown) => common_enums::AttemptStatus::Pending,
        Some(NuveiTransactionStatus::Redirect) => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        None => status_without_transaction(),
    }
}

// Transaction Type for initPayment
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TransactionType {
    Auth,
    #[default]
    Sale,
}

impl TransactionType {
    fn get_from_capture_method(
        capture_method: Option<common_enums::CaptureMethod>,
        amount: &StringMajorUnit,
    ) -> Self {
        let amount_value = amount.get_amount_as_string().parse::<f64>();
        if capture_method == Some(common_enums::CaptureMethod::Manual) || amount_value == Ok(0.0) {
            Self::Auth
        } else {
            Self::Sale
        }
    }
}

// Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Sync Response (getTransactionDetails has different structure than payment response)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSyncResponse {
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    pub internal_request_id: Option<i64>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub version: Option<String>,
    pub transaction_details: Option<NuveiTransactionDetails>,
    pub partial_approval: Option<NuveiPartialApproval>,
    pub transaction_link_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiTransactionDetails {
    pub transaction_id: Option<String>,
    /// The ID of the original transaction.
    pub related_transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub auth_code: Option<String>,
    pub client_unique_id: Option<String>,
    pub date: Option<String>,
    pub original_transaction_date: Option<String>,
    pub credited: Option<String>,
    pub credit_type: Option<String>,
    // The API reference lists isVoided without a type; "credited" beside it
    // is sent as the string "True" / "False", so no type is assumed here.
    pub is_voided: Option<serde_json::Value>,
    pub acquiring_bank_name: Option<String>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub gw_error_code: Option<i64>,
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    pub processed_amount: Option<StringMajorUnit>,
    pub processed_currency: Option<common_enums::Currency>,
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

// Capture Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureResponse {
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub internal_request_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub status: NuveiPaymentStatus,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i64>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub merchant_advice_code: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub payment_method_error_reason: Option<String>,
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

// Refund Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i64>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub merchant_advice_code: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub payment_method_error_reason: Option<String>,
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

// Refund Sync Response (separate type to avoid macro conflicts): the
// /getTransactionDetails shape, with the refund under transactionDetails.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncResponse {
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    pub merchant_id: Option<Secret<String>>,
    pub merchant_site_id: Option<Secret<String>>,
    pub transaction_details: Option<NuveiTransactionDetails>,
}

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

// Void Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiVoidResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub transaction_type: Option<NuveiTransactionType>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i64>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub merchant_advice_code: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub payment_method_error_reason: Option<String>,
}

// Error Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiErrorResponse {
    pub reason: Option<String>,
    // Nuvei sends errCode / gwErrorCode as JSON integers.
    #[serde(default, deserialize_with = "str_or_i64")]
    pub err_code: Option<String>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "str_or_i64")]
    pub gw_error_code: Option<String>,
    pub gw_error_reason: Option<String>,
}

/// The fields of a 2xx Nuvei response that classify it as a failure. A
/// /payment-type response carries them at the top level; /getTransactionDetails
/// carries the gateway fields under `transactionDetails`.
pub(super) struct NuveiErrorFields<'a> {
    pub status: &'a NuveiPaymentStatus,
    pub err_code: Option<i64>,
    pub reason: Option<&'a String>,
    pub transaction_status: Option<&'a NuveiTransactionStatus>,
    pub transaction_id: Option<&'a String>,
    pub gw_error_code: Option<i64>,
    pub gw_extended_error_code: Option<i64>,
    pub gw_error_reason: Option<&'a String>,
    pub merchant_advice_code: Option<&'a String>,
    pub issuer_decline_code: Option<&'a String>,
    pub issuer_decline_reason: Option<&'a String>,
    pub payment_method_error_reason: Option<&'a String>,
}

/// Error of a 2xx /payment-type response, or `None` when the response is not a
/// failure. A rejected request (`status` ERROR) reports errCode / reason; a
/// gateway decline or error reports gwErrorCode / gwErrorReason (spec: Error
/// mapping used by Hyperswitch). `attempt_status` is the failure status of
/// the calling flow.
pub(super) fn build_nuvei_error_response(
    response: &NuveiPaymentResponse,
    http_code: u16,
    attempt_status: common_enums::AttemptStatus,
) -> Option<domain_types::router_data::ErrorResponse> {
    build_nuvei_error_response_from_fields(
        NuveiErrorFields {
            status: &response.status,
            err_code: response.err_code,
            reason: response.reason.as_ref(),
            transaction_status: response.transaction_status.as_ref(),
            transaction_id: response.transaction_id.as_ref(),
            gw_error_code: response.gw_error_code,
            gw_extended_error_code: response.gw_extended_error_code,
            gw_error_reason: response.gw_error_reason.as_ref(),
            merchant_advice_code: response.merchant_advice_code.as_ref(),
            issuer_decline_code: response.issuer_decline_code.as_ref(),
            issuer_decline_reason: response.issuer_decline_reason.as_ref(),
            payment_method_error_reason: response.payment_method_error_reason.as_ref(),
        },
        http_code,
        attempt_status,
    )
}

/// The classifier behind `build_nuvei_error_response`, over the fields alone,
/// so that /getTransactionDetails is classified by the same rules.
pub(super) fn build_nuvei_error_response_from_fields(
    fields: NuveiErrorFields<'_>,
    http_code: u16,
    attempt_status: common_enums::AttemptStatus,
) -> Option<domain_types::router_data::ErrorResponse> {
    let (code, message) = match fields.status {
        NuveiPaymentStatus::Error => (fields.err_code, fields.reason.cloned()),
        NuveiPaymentStatus::Success
        | NuveiPaymentStatus::Failed
        | NuveiPaymentStatus::Processing
        | NuveiPaymentStatus::Pending
        | NuveiPaymentStatus::Unknown => {
            let is_gateway_failure = match fields.transaction_status {
                Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                    true
                }
                Some(NuveiTransactionStatus::Approved)
                | Some(NuveiTransactionStatus::Redirect)
                | Some(NuveiTransactionStatus::Pending)
                | Some(NuveiTransactionStatus::Processing)
                | Some(NuveiTransactionStatus::Unknown)
                | None => {
                    fields.gw_error_reason.map(String::as_str)
                        == Some(NUVEI_MISSING_ARGUMENT_REASON)
                }
            };
            if !is_gateway_failure {
                return None;
            }
            // Every filter error is gwErrorCode -1100; the documented, specific
            // code is gwExtendedErrorCode. A lookup of a declined transaction
            // carries no gwErrorCode: -1 is Nuvei's decline code.
            let code = match (fields.gw_error_code, fields.gw_extended_error_code) {
                (Some(NUVEI_FILTER_ERROR_CODE), Some(extended)) if extended > 0 => Some(extended),
                (None, _)
                    if matches!(
                        fields.transaction_status,
                        Some(NuveiTransactionStatus::Declined)
                    ) =>
                {
                    Some(NUVEI_DECLINE_CODE)
                }
                (code, _) => code,
            };
            (code, fields.gw_error_reason.cloned())
        }
    };

    // The issuer's own decline code and text when Nuvei returns them, else the
    // APM provider's reason.
    let issuer_reason = [
        fields.issuer_decline_code.map(String::as_str),
        fields.issuer_decline_reason.map(String::as_str),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(": ");
    let reason = Some(issuer_reason)
        .filter(|reason| !reason.is_empty())
        .or_else(|| {
            fields
                .payment_method_error_reason
                .cloned()
                .filter(|reason| !reason.is_empty())
        });

    Some(domain_types::router_data::ErrorResponse {
        code: code
            .map(|code| code.to_string())
            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message: message
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
        reason,
        status_code: http_code,
        attempt_status: Some(FlowStatus::Payment(attempt_status)),
        connector_transaction_id: fields.transaction_id.cloned(),
        network_decline_code: fields.gw_error_code.map(|code| code.to_string()),
        network_advice_code: fields
            .merchant_advice_code
            .cloned()
            .filter(|code| !code.is_empty()),
        network_error_message: fields
            .gw_error_reason
            .cloned()
            .filter(|reason| !reason.is_empty()),
        ..Default::default()
    })
}

/// clientUniqueId for a request: the connector request reference id, refused
/// when it does not fit Nuvei's String(45).
pub(super) fn get_valid_client_unique_id(
    connector_request_reference_id: &str,
) -> Result<String, Report<IntegrationError>> {
    let received_length = connector_request_reference_id.len();
    if received_length > NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH {
        return Err(IntegrationError::MaxFieldLengthViolated {
            connector: "Nuvei".to_string(),
            field_name: "client_unique_id".to_string(),
            max_length: NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH,
            received_length,
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Send a merchant_transaction_id of at most {NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH} characters"
                )),
                doc_url: None,
                additional_context: Some(format!(
                    "Nuvei clientUniqueId is String({NUVEI_MAX_CLIENT_UNIQUE_ID_LENGTH}); the reference id has {received_length} characters"
                )),
            },
        }
        .into());
    }
    Ok(connector_request_reference_id.to_string())
}

/// sessionToken for a /payment-type request: the session_token of the request
/// when it carries one, else `state.access_token`, which is where flows
/// without a session_token field receive it.
pub(super) fn get_nuvei_session_token(
    resource_data: &PaymentFlowData,
) -> Result<String, Report<IntegrationError>> {
    resource_data
        .session_token
        .clone()
        .filter(|token| !token.is_empty())
        .or_else(|| {
            resource_data
                .access_token
                .as_ref()
                .map(|token| token.access_token.peek().to_string())
                .filter(|token| !token.is_empty())
        })
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Call CreateServerSessionAuthenticationToken first and pass its session_token"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /payment.do needs the sessionToken returned by /getSessionToken.do; neither session_token nor state.access_token is set"
                            .to_string(),
                    ),
                },
            }
            .into()
        })
}

/// Description of a Nuvei `avsCode`, as in hyperswitch; none for any other value.
fn get_nuvei_avs_description(code: &str) -> Option<&'static str> {
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

/// Description of a Nuvei `cvv2Reply`, as in hyperswitch; none for any other value.
fn get_nuvei_cvv2_description(code: &str) -> Option<&'static str> {
    match code {
        "M" => Some("CVV2 Match"),
        "N" => Some("CVV2 No Match"),
        "P" => Some("Not Processed. For EU card-on-file (COF) and ecommerce (ECOM) network token transactions, Visa removes any CVV and sends P. If you have fraud or security concerns, Visa recommends using 3DS."),
        "U" => Some("Issuer is not certified and/or has not provided Visa the encryption keys"),
        "S" => Some("CVV2 processor is unavailable."),
        _ => None,
    }
}

/// AVS / CVV results and the card brand of a /payment-type response as
/// connector response data. They come back on approvals and declines alike.
/// payment_checks carries the four hyperswitch keys whenever the response has
/// a card, with the values as received: an empty string stays an empty string.
pub(super) fn get_nuvei_connector_response(
    response: &NuveiPaymentResponse,
) -> Option<ConnectorResponseData> {
    let response_card = response
        .payment_option
        .as_ref()
        .and_then(|payment_option| payment_option.card.as_ref());
    let card_network = response_card
        .and_then(|card| card.card_brand.clone())
        .filter(|brand| !brand.is_empty());
    let payment_checks = response_card.map(|card| {
        serde_json::json!({
            "avs_result": card.avs_code,
            "avs_description": card.avs_code.as_deref().and_then(get_nuvei_avs_description),
            "card_validation_result": card.cvv2_reply,
            "card_validation_description": card
                .cvv2_reply
                .as_deref()
                .and_then(get_nuvei_cvv2_description),
        })
    });
    (payment_checks.is_some() || card_network.is_some()).then(|| {
        ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks,
                card_network,
                domestic_network: None,
                auth_code: None,
            },
        )
    })
}

/// dynamicDescriptor from the request's billing descriptor.
fn get_dynamic_descriptor(
    billing_descriptor: Option<&domain_types::connector_types::BillingDescriptor>,
) -> Result<Option<NuveiDynamicDescriptor>, Report<IntegrationError>> {
    let Some(descriptor) = billing_descriptor else {
        return Ok(None);
    };
    if let Some(phone) = descriptor.phone.as_ref() {
        let received_length = phone.peek().len();
        if received_length > NUVEI_MAX_DESCRIPTOR_PHONE_LENGTH {
            return Err(IntegrationError::MaxFieldLengthViolated {
                connector: "Nuvei".to_string(),
                field_name: "dynamic_descriptor.merchant_phone".to_string(),
                max_length: NUVEI_MAX_DESCRIPTOR_PHONE_LENGTH,
                received_length,
                context: IntegrationErrorContext {
                    suggested_action: Some(format!(
                        "Send a billing_descriptor.phone of at most {NUVEI_MAX_DESCRIPTOR_PHONE_LENGTH} characters"
                    )),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Nuvei dynamicDescriptor.merchantPhone is String({NUVEI_MAX_DESCRIPTOR_PHONE_LENGTH})"
                    )),
                },
            }
            .into());
        }
    }
    let merchant_name = descriptor.name.as_ref().map(|name| {
        Secret::new(
            name.peek()
                .trim()
                .chars()
                .take(NUVEI_MAX_DESCRIPTOR_NAME_LENGTH)
                .collect::<String>(),
        )
    });
    if merchant_name.is_none() && descriptor.phone.is_none() {
        return Ok(None);
    }
    Ok(Some(NuveiDynamicDescriptor {
        merchant_name,
        merchant_phone: descriptor.phone.clone(),
    }))
}

/// shippingAddress from the request's shipping address, when it has one.
fn get_shipping_address(resource_data: &PaymentFlowData) -> Option<NuveiShippingAddress> {
    resource_data
        .get_optional_shipping()
        .map(|_| NuveiShippingAddress {
            first_name: resource_data.get_optional_shipping_first_name(),
            last_name: resource_data.get_optional_shipping_last_name(),
            address: resource_data.get_optional_shipping_line1(),
            address_line2: resource_data.get_optional_shipping_line2(),
            address_line3: resource_data.get_optional_shipping_line3(),
            city: resource_data.get_optional_shipping_city(),
            zip: resource_data.get_optional_shipping_zip(),
            country: resource_data.get_optional_shipping_country(),
            email: resource_data.get_optional_shipping_email(),
            phone: resource_data.get_optional_shipping_phone_number(),
        })
}

impl From<Option<common_enums::ProductType>> for NuveiItemType {
    fn from(product_type: Option<common_enums::ProductType>) -> Self {
        match product_type {
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

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
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
        let session_token = response.session_token.clone().ok_or_else(|| {
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

        // Per Hyperswitch pattern: ALWAYS send both transaction_id AND client_unique_id
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the connector_transaction_id returned by the payment call"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei /getTransactionDetails.do looks a transaction up by its transactionId"
                                .to_string(),
                        ),
                    },
                }
                .into());
            }
        };

        // Generate checksum for getTransactionDetails: merchantId + merchantSiteId + transactionId + clientUniqueId + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &transaction_id,
            &client_unique_id,
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_unique_id,
            transaction_id,
            time_stamp,
            checksum,
        })
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

        // Extract auth data
        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // clientUniqueId carries the merchant reference; Nuvei caps it at 45 characters.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // Nuvei only knows Sale (captured now) and Auth (captured later by
        // /settleTransaction.do), so any other capture intent is refused
        // instead of being sent as a Sale.
        match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic)
            | Some(common_enums::CaptureMethod::Manual)
            | None => {}
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                return Err(IntegrationError::CaptureMethodNotSupported {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use capture_method AUTOMATIC or MANUAL for Nuvei".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei /payment.do supports transactionType Sale and Auth only"
                                .to_string(),
                        ),
                    },
                }
                .into())
            }
        }

        // A payment returning from a 3DS challenge that did not pass is not charged.
        validate_nuvei_challenge_result(router_data.request.redirect_response.as_ref())?;

        // State of the Authenticate leg, when this call completes a 3DS payment.
        let three_ds_state = router_data
            .request
            .connector_feature_data
            .as_ref()
            .and_then(|feature_data| {
                serde_json::from_value::<NuveiThreeDsState>(feature_data.peek().clone()).ok()
            });

        // The ServerSessionAuthenticationToken flow runs before Authorize and
        // provides the sessionToken every /payment.do call needs. The final
        // call of a 3DS payment stays on the session of its earlier legs.
        let session_token = match three_ds_state
            .as_ref()
            .and_then(|state| state.session_token.as_ref())
            .map(|token| token.peek().to_string())
            .filter(|token| !token.is_empty())
        {
            Some(session_token) => session_token,
            None => get_nuvei_session_token(&router_data.resource_common_data)?,
        };

        let dynamic_descriptor =
            get_dynamic_descriptor(router_data.request.billing_descriptor.as_ref())?;

        // Authentication done by a third-party MPI: both cavv and eci are
        // needed for Nuvei to accept it.
        let external_mpi = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|auth_data| {
                auth_data
                    .cavv
                    .clone()
                    .zip(auth_data.eci.clone())
                    .map(|(cavv, eci)| NuveiExternalMpi {
                        eci,
                        cavv,
                        ds_trans_id: auth_data.ds_trans_id.clone(),
                    })
            });

        // transactionId of the 3DS payment made by the Authenticate leg, which
        // this call completes.
        let related_transaction_id = three_ds_state
            .map(|state| state.related_transaction_id)
            .filter(|id| !id.is_empty());

        // Extract billing email up-front so ACH arms can populate `user_token_id`
        // from it. Nuvei requires `userTokenId` for ACH (BankDebit/BankTransfer)
        // but not for cards, so it is populated conditionally below.
        let email = router_data
            .resource_common_data
            .get_optional_billing_email()
            .or_else(|| router_data.request.email.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_address.email",
                context: Default::default(),
            })?;

        let mut user_token_id: Option<String> = None;
        let mut is_rebilling: Option<String> = None;
        let mut card_related_transaction_id: Option<String> = None;

        // Authorize is the charging call only. A 3DS-requested card payment
        // (card, network token or stored token) is charged here after the
        // authentication legs, or with external MPI data; otherwise it would
        // be charged with no authentication at all.
        if router_data.resource_common_data.is_three_ds()
            && external_mpi.is_none()
            && related_transaction_id.is_none()
            && matches!(
                router_data.request.payment_method_data,
                PaymentMethodData::Card(_)
                    | PaymentMethodData::NetworkToken(_)
                    | PaymentMethodData::PaymentMethodToken(_)
            )
        {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.related_transaction_id",
                context: IntegrationErrorContext {
                    suggested_action: Some("run PreAuthenticate and Authenticate first".to_string()),
                    doc_url: None,
                    additional_context: Some(
                        "auth_type THREE_DS needs the Authenticate-leg transaction id in connector_feature_data.related_transaction_id, or authentication_data with cavv and eci"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        // Extract payment method data
        let payment_option = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Customer-initiated stored credential: isRebilling "0" with
                // a userTokenId makes Nuvei return a userPaymentOptionId for
                // later merchant-initiated payments.
                if router_data.request.is_customer_initiated_mandate_payment() {
                    is_rebilling = Some("0".to_string());
                    user_token_id = router_data
                        .resource_common_data
                        .customer_id
                        .as_ref()
                        .or(router_data.request.customer_id.as_ref())
                        .map(|customer_id| customer_id.get_string_repr().to_string());
                }
                card_related_transaction_id = related_transaction_id;

                let card_holder_name = router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .or(router_data.request.customer_name.clone().map(Secret::new))
                    .filter(|name| !name.peek().is_empty());

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d: external_mpi.map(|external_mpi| NuveiThreeD {
                            external_mpi: Some(external_mpi),
                        }),
                        stored_credentials: None,
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
                        user_token_id = Some(email.peek().to_string());

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
                        return Err(IntegrationError::NotSupported {
                            message: format!("{:?} is not supported for Nuvei", other),
                            connector: "nuvei",
                            context: Default::default(),
                        }
                        .into())
                    }
                }
            }
            PaymentMethodData::BankTransfer(bank_transfer_data) => {
                match bank_transfer_data.as_ref() {
                    BankTransferData::AchBankTransfer {} => {
                        // For ACH Bank Transfer, Nuvei requires account_number and routing_number
                        // These should be provided in the request metadata as ACH details
                        let metadata = router_data.request.metadata.as_ref().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "metadata for ACH details",
                                context: Default::default(),
                            },
                        )?;

                        let ach_data = metadata.peek().get("ach").ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "ach in metadata",
                                context: Default::default(),
                            },
                        )?;

                        let account_number = ach_data
                            .get("account_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "account_number",
                                context: Default::default(),
                            })?;

                        let routing_number = ach_data
                            .get("routing_number")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "routing_number",
                                context: Default::default(),
                            })?;

                        let sec_code = ach_data
                            .get("sec_code")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(String::from);

                        // Nuvei requires userTokenId for ACH flows.
                        user_token_id = Some(email.peek().to_string());

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
                        return Err(IntegrationError::NotSupported {
                            message: format!("{:?} is not supported for Nuvei", other),
                            connector: "nuvei",
                            context: Default::default(),
                        }
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
                        return Err(IntegrationError::NotSupported {
                            message: format!(
                                "Bank redirect method {:?} not supported by Nuvei",
                                other
                            ),
                            connector: "nuvei",
                            context: Default::default(),
                        }
                        .into())
                    }
                };

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
            PaymentMethodData::PaymentMethodToken(token_data) => NuveiPaymentOption {
                card: None,
                alternative_payment_method: None,
                user_payment_option_id: Some(token_data.token.clone()),
            },
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method not supported by Nuvei in this transformer".to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        // Nuvei requires firstName, lastName, and country for the billing address.
        // (`email` was already extracted above so ACH arms could populate `user_token_id`.)
        let country = router_data
            .resource_common_data
            .get_optional_billing_country()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "billing_address.country",
                context: Default::default(),
            })?;

        // Get first and last name from billing (optional fields)
        let first_name = router_data
            .resource_common_data
            .get_optional_billing_first_name();

        let last_name = router_data
            .resource_common_data
            .get_optional_billing_last_name();

        // Use state code conversion (e.g., "California" -> "CA") for US/CA
        let state = router_data
            .resource_common_data
            .get_optional_billing_state();

        // Get address_line3 directly from billing address
        let address_line3 = router_data
            .resource_common_data
            .get_optional_billing()
            .and_then(|billing| billing.address.as_ref())
            .and_then(|addr| addr.line3.clone());

        let billing_address = NuveiBillingAddress {
            email,
            first_name,
            last_name,
            country: country.to_string(),
            phone: router_data
                .resource_common_data
                .get_optional_billing_phone_number(),
            city: router_data.resource_common_data.get_optional_billing_city(),
            address: router_data
                .resource_common_data
                .get_optional_billing_line1(),
            address_line2: router_data
                .resource_common_data
                .get_optional_billing_line2(),
            address_line3,
            zip: router_data.resource_common_data.get_optional_billing_zip(),
            state,
        };

        let shipping_address = get_shipping_address(&router_data.resource_common_data);

        // Get device details - ipAddress is required by Nuvei
        let ip_address = router_data
            .request
            .browser_info
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info",
                context: Default::default(),
            })?
            .ip_address
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: Default::default(),
            })?;

        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let currency = router_data.request.currency;

        // Level 2 / level 3 data: order lines and the tax, shipping, discount
        // and duty totals, all in major units like `amount`.
        let to_major_unit = |minor_amount| {
            item.connector
                .amount_converter_webhooks
                .convert(minor_amount, currency)
                .change_context(IntegrationError::AmountConversionFailed {
                    context: IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(
                            "Failed to convert an l2_l3_data amount to Nuvei major units"
                                .to_string(),
                        ),
                    },
                })
        };
        let l2_l3_data = router_data.resource_common_data.l2_l3_data.as_deref();
        let items = l2_l3_data
            .and_then(|data| data.get_order_details())
            .map(|order_details| {
                order_details
                    .into_iter()
                    .map(|order| {
                        Ok(NuveiItem {
                            name: order.product_name,
                            item_type: NuveiItemType::from(order.product_type),
                            price: to_major_unit(order.amount)?,
                            quantity: order.quantity.to_string(),
                            group_id: order.product_id,
                            discount: order.unit_discount_amount.map(to_major_unit).transpose()?,
                            tax: order.total_tax_amount.map(to_major_unit).transpose()?,
                            tax_rate: order.tax_rate.map(|rate| rate.to_string()),
                            image_url: order.product_img_link,
                        })
                    })
                    .collect::<Result<Vec<_>, Report<IntegrationError>>>()
            })
            .transpose()?;
        let amount_details = l2_l3_data
            .map(|data| {
                Ok::<_, Report<IntegrationError>>(NuveiAmountDetails {
                    total_tax: data.get_order_tax_amount().map(to_major_unit).transpose()?,
                    total_shipping: data.get_shipping_cost().map(to_major_unit).transpose()?,
                    total_discount: data.get_discount_amount().map(to_major_unit).transpose()?,
                    // Nuvei has no duty field; hyperswitch sends duty as totalHandling.
                    total_handling: data.get_duty_amount().map(to_major_unit).transpose()?,
                })
            })
            .transpose()?
            .filter(|details| {
                details.total_tax.is_some()
                    || details.total_shipping.is_some()
                    || details.total_discount.is_some()
                    || details.total_handling.is_some()
            });

        let is_partial_approval = router_data
            .request
            .enable_partial_authorization
            .map(|enabled| match enabled {
                true => "1".to_string(),
                false => "0".to_string(),
            });

        let is_moto = match router_data.request.payment_channel {
            Some(common_enums::PaymentChannel::MailOrder)
            | Some(common_enums::PaymentChannel::TelephoneOrder) => Some(true),
            Some(common_enums::PaymentChannel::Ecommerce) | None => None,
        };

        // Determine transaction type based on capture method
        let transaction_type =
            TransactionType::get_from_capture_method(router_data.request.capture_method, &amount);

        // Build urlDetails from router_return_url if available
        let url_details =
            router_data
                .request
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
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            user_token_id,
            client_unique_id: Some(client_unique_id),
            is_rebilling,
            related_transaction_id: card_related_transaction_id,
            payment_option,
            transaction_type,
            is_partial_approval,
            is_moto,
            dynamic_descriptor,
            items,
            amount_details,
            device_details,
            billing_address,
            shipping_address,
            url_details,
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

        // A zero-amount Auth is a card verification: once approved it is complete.
        let zero_amount =
            router_data.request.amount.amount == common_utils::types::MinorUnit::new(0);
        let status = get_nuvei_payment_status(
            zero_amount,
            response.transaction_type,
            response.transaction_status.as_ref(),
            &response.status,
        );

        // AVS / CVV results come back on approvals and declines alike.
        let connector_response = get_nuvei_connector_response(response);

        // A 2xx response can still be a rejected request, a decline or a
        // gateway error; those are returned as errors with this flow's status.
        let failure_status = if status == common_enums::AttemptStatus::AuthorizationFailed {
            common_enums::AttemptStatus::AuthorizationFailed
        } else {
            common_enums::AttemptStatus::Failure
        };
        if let Some(error_response) =
            build_nuvei_error_response(response, item.http_code, failure_status)
        {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: failure_status,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Get connector transaction ID
        let connector_transaction_id = response
            .transaction_id
            .clone()
            .or(response.order_id.clone())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("missing transaction_id and order_id in Nuvei PSync response".to_string()),
                ))
            })?;

        let redirection_data = response
            .payment_option
            .as_ref()
            .and_then(|payment_option| payment_option.redirect_url.clone())
            .and_then(|url| Url::parse(&url).ok())
            .map(|url| Box::new(RedirectForm::from((url, Method::Get))));

        // userPaymentOptionId is the stored credential Nuvei created for this
        // customer; the customer's IP is kept with it for later MITs, as
        // hyperswitch does.
        let mandate_reference = response
            .payment_option
            .as_ref()
            .and_then(|payment_option| payment_option.user_payment_option_id.as_ref())
            .map(|id| id.peek().to_string())
            .filter(|id| !id.is_empty())
            .map(|id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: router_data
                        .request
                        .browser_info
                        .as_ref()
                        .and_then(|browser_info| browser_info.ip_address)
                        .map(|ip_address| {
                            Secret::new(serde_json::Value::String(ip_address.to_string()))
                        }),
                })
            });

        let network_txn_id = response
            .external_scheme_transaction_id
            .as_ref()
            .map(|ntid| ntid.peek().to_string())
            .filter(|ntid| !ntid.is_empty());

        // Partial approval: the processed amount is what was captured (Sale)
        // or what can be captured (Auth).
        let processed_amount = response
            .partial_approval
            .as_ref()
            .and_then(|partial_approval| {
                partial_approval
                    .processed_amount
                    .clone()
                    .zip(partial_approval.processed_currency)
            })
            .map(|(amount, currency)| {
                domain_types::utils::convert_back_amount_to_minor_units(
                    &common_utils::types::StringMajorUnitForConnector,
                    amount,
                    currency,
                )
                .map(|amount| common_utils::types::Money { amount, currency })
                .change_context(
                    ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some(
                            "invalid partialApproval.processedAmount in Nuvei response".to_string(),
                        ),
                    ),
                )
            })
            .transpose()?;
        let (amount_captured, amount_capturable) = match (processed_amount, status) {
            (Some(processed_amount), common_enums::AttemptStatus::Charged) => (
                Some(processed_amount),
                router_data.resource_common_data.amount_capturable.clone(),
            ),
            (Some(processed_amount), common_enums::AttemptStatus::Authorized) => (
                router_data.resource_common_data.amount_captured.clone(),
                Some(processed_amount),
            ),
            (Some(_), _) | (None, _) => (
                router_data.resource_common_data.amount_captured.clone(),
                router_data.resource_common_data.amount_capturable.clone(),
            ),
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data,
            mandate_reference,
            connector_metadata: None,
            network_txn_id,
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|link_id| !link_id.is_empty()),
            connector_response_reference_id: response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                amount_captured,
                amount_capturable,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ---------------------------------------------------------------------------
// 3DS legs: PreAuthenticate (/initPayment.do) and Authenticate (the first
// /payment.do, with the threeD block). The charging call stays Authorize.
// ---------------------------------------------------------------------------

// threeD values hyperswitch sends on the first /payment.do: full-screen
// challenge window, challenge preferred, browser platform, and "method URL
// fingerprint not run" because the 3DS method step is skipped.
const NUVEI_CHALLENGE_WINDOW_SIZE: &str = "05";
const NUVEI_CHALLENGE_PREFERENCE: &str = "01";
const NUVEI_PLATFORM_TYPE_BROWSER: &str = "02";
const NUVEI_METHOD_COMPLETION_UNAVAILABLE: &str = "U";
// Form field the ACS expects the challenge request in.
const NUVEI_CREQ_FORM_FIELD: &str = "creq";

/// Card of the two 3DS legs. cardHolderName is optional here, as in hyperswitch.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDsCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    pub card_holder_name: Option<Secret<String>>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
    pub three_d: Option<NuveiChallengeThreeD>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeDsPaymentOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: NuveiThreeDsCard<T>,
}

/// threeD block of the first /payment.do. `version` is not sent (hyperswitch
/// never sends it).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiChallengeThreeD {
    pub method_completion_ind: String,
    pub browser_details: NuveiBrowserDetails,
    #[serde(rename = "notificationURL")]
    pub notification_url: String,
    #[serde(rename = "merchantURL")]
    pub merchant_url: String,
    pub platform_type: String,
    pub v2_additional_params: NuveiV2AdditionalParams,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiBrowserDetails {
    pub accept_header: String,
    pub ip: Secret<String, pii::IpAddress>,
    /// "TRUE" / "FALSE"
    pub java_enabled: String,
    /// "TRUE" / "FALSE"
    pub java_script_enabled: String,
    pub language: String,
    pub color_depth: u8,
    pub screen_height: u32,
    pub screen_width: u32,
    pub time_zone: i32,
    pub user_agent: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiV2AdditionalParams {
    pub challenge_window_size: String,
    pub challenge_preference: String,
}

/// /initPayment.do request. The session token authenticates it: Nuvei's input
/// table has no timeStamp and no checksum for this call.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiInitPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub user_token_id: Option<String>,
    pub payment_option: NuveiThreeDsPaymentOption<T>,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    pub url_details: Option<NuveiUrlDetails>,
}

/// First /payment.do of a 3DS payment: the Authorize body plus
/// relatedTransactionId (the /initPayment.do transaction) and the threeD block.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Secret<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub client_unique_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    pub related_transaction_id: String,
    pub payment_option: NuveiThreeDsPaymentOption<T>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    pub shipping_address: Option<NuveiShippingAddress>,
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Both legs answer with the /payment-type envelope.
pub type NuveiInitPaymentResponse = NuveiPaymentResponse;
pub type NuveiAuthenticateResponse = NuveiPaymentResponse;

/// What the final /payment.do needs from the Authenticate leg. It is returned
/// as connector_feature_data and sent back on the Authorize request.
#[derive(Debug, Serialize, Deserialize)]
pub struct NuveiThreeDsState {
    pub session_token: Option<Secret<String>>,
    /// transactionId of the first /payment.do (the Auth3D transaction).
    pub related_transaction_id: String,
}

/// transStatus of the decoded challenge response (CRes).
#[derive(Debug, Deserialize)]
pub enum NuveiChallengeTransStatus {
    #[serde(rename = "Y", alias = "y", alias = "1")]
    Success,
    #[serde(rename = "N", alias = "n", alias = "0")]
    Failed,
    #[serde(other)]
    Unknown,
}

/// The part of the decoded CRes that decides whether the challenge passed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiChallengeResponse {
    pub trans_status: Option<NuveiChallengeTransStatus>,
}

/// MissingRequiredField of a 3DS leg, saying what to send and why.
fn nuvei_three_ds_missing_field(
    field_name: &'static str,
    suggested_action: &str,
    additional_context: &str,
) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(suggested_action.to_string()),
            doc_url: None,
            additional_context: Some(additional_context.to_string()),
        },
    }
    .into()
}

/// paymentOption of a 3DS leg. Nuvei runs 3DS on card data only, so every
/// other payment method is refused before a request is built.
fn get_nuvei_three_ds_payment_option<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: Option<&PaymentMethodData<T>>,
    resource_data: &PaymentFlowData,
    three_d: Option<NuveiChallengeThreeD>,
) -> Result<NuveiThreeDsPaymentOption<T>, Report<IntegrationError>> {
    match payment_method_data {
        Some(PaymentMethodData::Card(card_data)) => Ok(NuveiThreeDsPaymentOption {
            card: NuveiThreeDsCard {
                card_number: card_data.card_number.clone(),
                card_holder_name: resource_data
                    .get_optional_billing_full_name()
                    .or_else(|| card_data.card_holder_name.clone()),
                expiration_month: card_data.card_exp_month.clone(),
                expiration_year: card_data.card_exp_year.clone(),
                cvv: card_data.card_cvc.clone(),
                three_d,
            },
        }),
        Some(PaymentMethodData::CardWithNoCvc(_))
        | Some(PaymentMethodData::CardDetailsForNetworkTransactionId(_))
        | Some(PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_))
        | Some(PaymentMethodData::CardRedirect(_))
        | Some(PaymentMethodData::Wallet(_))
        | Some(PaymentMethodData::PayLater(_))
        | Some(PaymentMethodData::BankRedirect(_))
        | Some(PaymentMethodData::BankDebit(_))
        | Some(PaymentMethodData::BankTransfer(_))
        | Some(PaymentMethodData::Crypto(_))
        | Some(PaymentMethodData::MandatePayment)
        | Some(PaymentMethodData::Reward)
        | Some(PaymentMethodData::RealTimePayment(_))
        | Some(PaymentMethodData::Upi(_))
        | Some(PaymentMethodData::Voucher(_))
        | Some(PaymentMethodData::GiftCard(_))
        | Some(PaymentMethodData::PaymentMethodToken(_))
        | Some(PaymentMethodData::OpenBanking(_))
        | Some(PaymentMethodData::NetworkToken(_))
        | Some(PaymentMethodData::MobilePayment(_)) => Err(IntegrationError::NotImplemented(
            "Nuvei 3DS legs take card data only".to_string(),
            IntegrationErrorContext {
                suggested_action: Some(
                    "Send a card as payment_method for PreAuthenticate and Authenticate"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "/initPayment.do and the 3DS /payment.do need the card number, expiry and CVV"
                        .to_string(),
                ),
            },
        )
        .into()),
        None => Err(nuvei_three_ds_missing_field(
            "payment_method",
            "Send the card as payment_method",
            "/initPayment.do and the 3DS /payment.do need the card number, expiry and CVV",
        )),
    }
}

/// The browser information of a 3DS leg and the customer's IP in it; Nuvei
/// requires deviceDetails.ipAddress on both legs.
fn get_nuvei_three_ds_browser_info(
    browser_info: Option<&BrowserInformation>,
) -> Result<(&BrowserInformation, std::net::IpAddr), Report<IntegrationError>> {
    let browser_info = browser_info.ok_or_else(|| {
        nuvei_three_ds_missing_field(
            "browser_info",
            "Send browser_info with the customer's ip_address",
            "Nuvei requires deviceDetails.ipAddress on /initPayment.do and /payment.do",
        )
    })?;
    let ip_address = browser_info.ip_address.ok_or_else(|| {
        nuvei_three_ds_missing_field(
            "browser_info.ip_address",
            "Send the customer's IP address in browser_info.ip_address",
            "Nuvei requires deviceDetails.ipAddress on /initPayment.do and /payment.do",
        )
    })?;
    Ok((browser_info, ip_address))
}

/// billingAddress of a 3DS leg; Nuvei requires its email and country.
fn get_nuvei_three_ds_billing_address(
    resource_data: &PaymentFlowData,
    fallback_email: Option<pii::Email>,
) -> Result<NuveiBillingAddress, Report<IntegrationError>> {
    get_billing_address(resource_data, fallback_email).ok_or_else(|| {
        nuvei_three_ds_missing_field(
            "billing_address (email and country required)",
            "Send the billing address country and an email (billing or customer)",
            "Nuvei requires billingAddress.email and billingAddress.country",
        )
    })
}

/// urlDetails from the return URL, as Authorize sends them.
fn get_nuvei_three_ds_url_details(return_url: Option<&Url>) -> Option<NuveiUrlDetails> {
    return_url.map(|url| NuveiUrlDetails {
        success_url: url.to_string(),
        failure_url: url.to_string(),
        pending_url: url.to_string(),
    })
}

impl TryFrom<(&BrowserInformation, std::net::IpAddr)> for NuveiBrowserDetails {
    type Error = Report<IntegrationError>;

    fn try_from(
        (browser_info, ip_address): (&BrowserInformation, std::net::IpAddr),
    ) -> Result<Self, Self::Error> {
        let missing = |field_name: &'static str| {
            nuvei_three_ds_missing_field(
                field_name,
                "Send the complete browser_info collected from the customer's browser",
                "Nuvei threeD.browserDetails is what the issuer's ACS uses to run the 3DS2 challenge",
            )
        };
        // Nuvei expects the two flags as upper-case TRUE / FALSE strings.
        let to_flag = |enabled: bool| enabled.to_string().to_uppercase();

        Ok(Self {
            accept_header: browser_info
                .accept_header
                .clone()
                .ok_or_else(|| missing("browser_info.accept_header"))?,
            ip: Secret::new(ip_address.to_string()),
            java_enabled: browser_info
                .java_enabled
                .map(to_flag)
                .ok_or_else(|| missing("browser_info.java_enabled"))?,
            java_script_enabled: browser_info
                .java_script_enabled
                .map(to_flag)
                .ok_or_else(|| missing("browser_info.java_script_enabled"))?,
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
}

// PreAuthenticate Request Transformation
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

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // clientUniqueId carries the merchant reference; Nuvei caps it at 45 characters.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // The same session token is used by /initPayment.do and both /payment.do calls.
        let session_token = get_nuvei_session_token(&router_data.resource_common_data)?;

        let payment_option = get_nuvei_three_ds_payment_option(
            router_data.request.payment_method_data.as_ref(),
            &router_data.resource_common_data,
            None,
        )?;

        let (_, ip_address) =
            get_nuvei_three_ds_browser_info(router_data.request.browser_info.as_ref())?;

        let billing_address = get_nuvei_three_ds_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        )?;

        let currency = router_data.request.amount.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(router_data.request.amount.amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the amount to Nuvei major units".to_string(),
                    ),
                },
            })?;

        Ok(Self {
            session_token: Secret::new(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            client_unique_id,
            amount,
            currency,
            user_token_id: router_data
                .resource_common_data
                .customer_id
                .as_ref()
                .map(|customer_id| customer_id.get_string_repr().to_string()),
            payment_option,
            device_details: NuveiDeviceDetails {
                ip_address: Secret::new(ip_address.to_string()),
            },
            billing_address,
            url_details: get_nuvei_three_ds_url_details(
                router_data.request.router_return_url.as_ref(),
            ),
        })
    }
}

/// threeD.version of a response as a semantic version. Anything that is not
/// three numeric parts is dropped, so the authentication data stays convertible.
fn get_nuvei_three_ds_message_version(version: Option<&String>) -> Option<SemanticVersion> {
    let parts = version?
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    match parts.as_slice() {
        [major, minor, patch] => Some(SemanticVersion::new(*major, *minor, *patch)),
        _ => None,
    }
}

/// Authentication data of a 3DS leg. Only what Nuvei returned is set; the
/// challenge-result fields belong to no Nuvei leg.
fn get_nuvei_authentication_data(
    transaction_id: Option<String>,
    three_d: Option<&NuveiResponseThreeD>,
    with_authentication_values: bool,
) -> AuthenticationData {
    let authentication_values = three_d.filter(|_| with_authentication_values);
    AuthenticationData {
        trans_status: None,
        eci: authentication_values
            .and_then(|three_d| three_d.eci.clone())
            .filter(|eci| !eci.is_empty()),
        cavv: authentication_values
            .and_then(|three_d| three_d.cavv.clone())
            .filter(|cavv| !cavv.peek().is_empty()),
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: three_d
            .and_then(|three_d| three_d.server_trans_id.clone())
            .filter(|id| !id.is_empty()),
        message_version: get_nuvei_three_ds_message_version(
            three_d.and_then(|three_d| three_d.version.as_ref()),
        ),
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id,
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }
}

fn get_nuvei_response_three_d(response: &NuveiPaymentResponse) -> Option<&NuveiResponseThreeD> {
    response
        .payment_option
        .as_ref()
        .and_then(|payment_option| payment_option.card.as_ref())
        .and_then(|card| card.three_d.as_ref())
}

/// Error of a 3DS leg that did not end in the outcome the leg exists for.
/// A rejected request, a decline or a gateway error keeps Nuvei's code and
/// message; any other outcome has none to report. threeDReason is added to
/// the reason when Nuvei returned one.
fn get_nuvei_authentication_error(
    response: &NuveiPaymentResponse,
    http_code: u16,
    unexpected_outcome: &str,
) -> domain_types::router_data::ErrorResponse {
    let mut error_response = build_nuvei_error_response(
        response,
        http_code,
        common_enums::AttemptStatus::AuthenticationFailed,
    )
    .unwrap_or_else(|| domain_types::router_data::ErrorResponse {
        code: consts::NO_ERROR_CODE.to_string(),
        message: consts::NO_ERROR_MESSAGE.to_string(),
        reason: Some(unexpected_outcome.to_string()),
        status_code: http_code,
        attempt_status: Some(FlowStatus::Payment(
            common_enums::AttemptStatus::AuthenticationFailed,
        )),
        connector_transaction_id: response.transaction_id.clone(),
        ..Default::default()
    });

    let three_d_reason = get_nuvei_response_three_d(response)
        .and_then(|three_d| three_d.three_d_reason.clone())
        .filter(|reason| !reason.is_empty());
    if let Some(three_d_reason) = three_d_reason {
        error_response.reason = Some(match error_response.reason {
            Some(reason) => format!("{reason}; {three_d_reason}"),
            None => three_d_reason,
        });
    }
    error_response
}

// PreAuthenticate Response Transformation
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

        // /initPayment.do only looks the card up (spec: ThreeDS step 1). An
        // approval lets the payment go on to the Authenticate leg; it is never
        // an authorized or captured payment. Anything else ends the payment.
        let is_approved = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => true,
            Some(NuveiTransactionStatus::Declined)
            | Some(NuveiTransactionStatus::Error)
            | Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Unknown)
            | None => false,
        };
        let is_failure = build_nuvei_error_response(
            response,
            item.http_code,
            common_enums::AttemptStatus::AuthenticationFailed,
        )
        .is_some();
        if is_failure || !is_approved {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(get_nuvei_authentication_error(
                    response,
                    item.http_code,
                    "Nuvei did not approve the 3DS initialisation (/initPayment.do)",
                )),
                ..router_data.clone()
            });
        }

        // The Authenticate leg sends this id as relatedTransactionId.
        let transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("missing transactionId in Nuvei initPayment response".to_string()),
                ))
            })?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::AuthenticationPending,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id.clone())),
                authentication_data: Some(get_nuvei_authentication_data(
                    Some(transaction_id),
                    get_nuvei_response_three_d(response),
                    false,
                )),
                redirection_data: None,
                connector_response_reference_id: response.order_id.clone(),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

// Authenticate Request Transformation
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

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // clientUniqueId carries the merchant reference; Nuvei caps it at 45 characters.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // A frictionless result is final, so the transaction type must be one
        // Nuvei has: Sale or Auth. Any other capture intent is refused.
        match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic)
            | Some(common_enums::CaptureMethod::Manual)
            | None => {}
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                return Err(IntegrationError::CaptureMethodNotSupported {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use capture_method AUTOMATIC or MANUAL for Nuvei".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei /payment.do supports transactionType Sale and Auth only"
                                .to_string(),
                        ),
                    },
                }
                .into())
            }
        }

        // The session token /initPayment.do was called with.
        let session_token = get_nuvei_session_token(&router_data.resource_common_data)?;

        // Nuvei error 1271: the 3DS payment must reference the InitAuth3D
        // transaction made by /initPayment.do.
        let related_transaction_id = router_data
            .request
            .authentication_data
            .as_ref()
            .and_then(|authentication_data| authentication_data.transaction_id.clone())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                nuvei_three_ds_missing_field(
                    "authentication_data.connector_transaction_id",
                    "Run PreAuthenticate first and pass its authentication_data.connector_transaction_id",
                    "The 3DS /payment.do sends the /initPayment.do transactionId as relatedTransactionId",
                )
            })?;

        let (browser_info, ip_address) =
            get_nuvei_three_ds_browser_info(router_data.request.browser_info.as_ref())?;
        let browser_details = NuveiBrowserDetails::try_from((browser_info, ip_address))?;

        // notificationURL is where the ACS posts the challenge result,
        // merchantURL the merchant's site; both as hyperswitch sends them.
        let notification_url = router_data
            .request
            .continue_redirection_url
            .as_ref()
            .map(Url::to_string)
            .ok_or_else(|| {
                nuvei_three_ds_missing_field(
                    "continue_redirection_url",
                    "Send the URL the ACS posts the challenge result to in continue_redirection_url",
                    "Nuvei threeD.notificationURL receives the cres after the challenge",
                )
            })?;
        let merchant_url = router_data
            .request
            .router_return_url
            .as_ref()
            .map(Url::to_string)
            .ok_or_else(|| {
                nuvei_three_ds_missing_field(
                    "return_url",
                    "Send the merchant return URL in return_url",
                    "Nuvei threeD.merchantURL is the return URL of the payment",
                )
            })?;

        let payment_option = get_nuvei_three_ds_payment_option(
            router_data.request.payment_method_data.as_ref(),
            &router_data.resource_common_data,
            Some(NuveiChallengeThreeD {
                method_completion_ind: NUVEI_METHOD_COMPLETION_UNAVAILABLE.to_string(),
                browser_details,
                notification_url,
                merchant_url,
                platform_type: NUVEI_PLATFORM_TYPE_BROWSER.to_string(),
                v2_additional_params: NuveiV2AdditionalParams {
                    challenge_window_size: NUVEI_CHALLENGE_WINDOW_SIZE.to_string(),
                    challenge_preference: NUVEI_CHALLENGE_PREFERENCE.to_string(),
                },
            }),
        )?;

        let billing_address = get_nuvei_three_ds_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        )?;

        let currency = router_data.request.amount.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(router_data.request.amount.amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the amount to Nuvei major units".to_string(),
                    ),
                },
            })?;

        // Same rule as Authorize: Auth for manual capture, Sale otherwise.
        let transaction_type =
            TransactionType::get_from_capture_method(router_data.request.capture_method, &amount);

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Same checksum as Authorize: merchantId + merchantSiteId + clientRequestId + amount + currency + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        Ok(Self {
            session_token: Secret::new(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            payment_option,
            transaction_type,
            device_details: NuveiDeviceDetails {
                ip_address: Secret::new(ip_address.to_string()),
            },
            billing_address,
            shipping_address: get_shipping_address(&router_data.resource_common_data),
            url_details: get_nuvei_three_ds_url_details(
                router_data.request.router_return_url.as_ref(),
            ),
            time_stamp,
            checksum,
        })
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
        let three_d = get_nuvei_response_three_d(response);

        let authentication_failed = |unexpected_outcome: &str| Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::AuthenticationFailed,
                connector_response: get_nuvei_connector_response(response),
                ..router_data.resource_common_data.clone()
            },
            response: Err(get_nuvei_authentication_error(
                response,
                item.http_code,
                unexpected_outcome,
            )),
            ..router_data.clone()
        };

        // A rejected request, a decline or a gateway error.
        if build_nuvei_error_response(
            response,
            item.http_code,
            common_enums::AttemptStatus::AuthenticationFailed,
        )
        .is_some()
        {
            return Ok(authentication_failed(
                "Nuvei rejected the 3DS payment (/payment.do)",
            ));
        }

        // This leg ends in one of two ways (spec: ThreeDS step 3): the ACS
        // challenge redirect, or the final result when the issuer needs no
        // challenge. Every other outcome fails the authentication, so the
        // caller never goes on to charge an unauthenticated payment.
        let (status, redirection_data, is_challenge, authentication_data) = match response
            .transaction_status
        {
            Some(NuveiTransactionStatus::Redirect) => {
                let challenge = three_d.and_then(|three_d| {
                    three_d
                        .acs_url
                        .clone()
                        .filter(|acs_url| !acs_url.is_empty())
                        .zip(
                            three_d
                                .c_req
                                .clone()
                                .filter(|c_req| !c_req.peek().is_empty()),
                        )
                });
                let Some((acs_url, c_req)) = challenge else {
                    return Ok(authentication_failed(
                        "Nuvei returned REDIRECT without threeD.acsUrl and threeD.cReq",
                    ));
                };
                (
                    common_enums::AttemptStatus::AuthenticationPending,
                    Some(Box::new(RedirectForm::Form {
                        endpoint: acs_url,
                        method: Method::Post,
                        form_fields: HashMap::from([(
                            NUVEI_CREQ_FORM_FIELD.to_string(),
                            c_req.peek().to_string(),
                        )]),
                    })),
                    true,
                    None,
                )
            }
            Some(NuveiTransactionStatus::Approved) => {
                // Frictionless: the payment is final in this call.
                let zero_amount =
                    router_data.request.amount.amount == common_utils::types::MinorUnit::new(0);
                let status = get_nuvei_payment_status(
                    zero_amount,
                    response.transaction_type,
                    response.transaction_status.as_ref(),
                    &response.status,
                );
                if !matches!(
                    status,
                    common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::Authorized
                ) {
                    return Ok(authentication_failed(
                            "Nuvei approved the 3DS payment with a transactionType that is neither Sale nor Auth",
                        ));
                }
                let authentication_data = get_nuvei_authentication_data(None, three_d, true);
                (
                    status,
                    None,
                    false,
                    (authentication_data.eci.is_some() || authentication_data.cavv.is_some())
                        .then_some(authentication_data),
                )
            }
            Some(NuveiTransactionStatus::Declined)
            | Some(NuveiTransactionStatus::Error)
            | Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Unknown)
            | None => {
                return Ok(authentication_failed(
                    "Nuvei returned neither a 3DS challenge nor a final result",
                ));
            }
        };

        let transaction_id = response
            .transaction_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("missing transactionId in Nuvei 3DS payment response".to_string()),
                ))
            })?;

        // The final /payment.do (Authorize) must use the same session and
        // reference this transaction.
        let connector_feature_data = is_challenge
            .then(|| {
                serde_json::to_value(NuveiThreeDsState {
                    session_token: response
                        .session_token
                        .clone()
                        .filter(|token| !token.peek().is_empty())
                        .or_else(|| {
                            get_nuvei_session_token(&router_data.resource_common_data)
                                .ok()
                                .map(Secret::new)
                        }),
                    related_transaction_id: transaction_id.clone(),
                })
                .change_context(
                    ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some("failed to encode the Nuvei 3DS state".to_string()),
                    ),
                )
            })
            .transpose()?;

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: get_nuvei_connector_response(response),
                ..router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(transaction_id)),
                redirection_data,
                authentication_data,
                connector_feature_data,
                connector_response_reference_id: response.order_id.clone(),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

/// Message of a challenge that did not pass, as Hyperswitch reports it.
const NUVEI_THREE_DS_AUTHENTICATION_FAILED: &str = "3ds Authentication failed";

/// The error object the ACS posts, base64-encoded, as `error` instead of a cres.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NuveiAcsErrorResponse {
    error_code: Option<String>,
    error_message: Option<String>,
    error_detail: Option<String>,
}

/// What the caller brought back from the ACS (spec: ThreeDS step 4). The ACS
/// posts either `cres`, the base64 challenge response, or `error`, a base64
/// error object.
enum NuveiChallengeResult {
    /// Neither a `cres` nor an `error` came back.
    Absent,
    /// The decoded cres has transStatus Y.
    Passed,
    /// An `error` payload, a cres that cannot be decoded, or a transStatus
    /// that is not Y.
    Failed {
        field_name: &'static str,
        additional_context: &'static str,
        acs_error: Option<NuveiAcsErrorResponse>,
    },
}

/// Decides whether the challenge passed. The PostAuthenticate leg and the
/// check on the final Authorize both go through here, so they cannot disagree.
fn classify_nuvei_challenge_result(
    redirect_response: Option<&ContinueRedirectionResponse>,
) -> NuveiChallengeResult {
    let Some(payload) = redirect_response.and_then(|redirect| redirect.payload.as_ref()) else {
        return NuveiChallengeResult::Absent;
    };
    let payload = payload.peek();

    if let Some(error) = payload.get("error") {
        return NuveiChallengeResult::Failed {
            field_name: "redirection_response.payload.error",
            additional_context:
                "3ds Authentication failed: the ACS returned an error instead of a challenge response",
            acs_error: error
                .as_str()
                .and_then(|error| safe_base64_decode(error.to_string()).ok())
                .and_then(|decoded| {
                    serde_json::from_slice::<NuveiAcsErrorResponse>(&decoded).ok()
                }),
        };
    }

    let Some(cres) = payload.get("cres") else {
        return NuveiChallengeResult::Absent;
    };
    let Some(challenge_response) = cres
        .as_str()
        .and_then(|cres| safe_base64_decode(cres.to_string()).ok())
        .and_then(|decoded| serde_json::from_slice::<NuveiChallengeResponse>(&decoded).ok())
    else {
        return NuveiChallengeResult::Failed {
            field_name: "redirection_response.payload.cres",
            additional_context:
                "3ds Authentication failed: the cres is not a base64-encoded challenge response",
            acs_error: None,
        };
    };

    match challenge_response.trans_status {
        Some(NuveiChallengeTransStatus::Success) => NuveiChallengeResult::Passed,
        Some(NuveiChallengeTransStatus::Failed)
        | Some(NuveiChallengeTransStatus::Unknown)
        | None => NuveiChallengeResult::Failed {
            field_name: "redirection_response.payload.cres",
            additional_context:
                "3ds Authentication failed: the challenge response transStatus is not Y",
            acs_error: None,
        },
    }
}

/// Refuses the final /payment.do when the caller returns from a challenge
/// that did not pass (spec: ThreeDS step 4). The failed status itself is
/// reported by the PostAuthenticate leg, or by Hyperswitch, which applies it
/// without calling this service. This refusal is the backstop that keeps a
/// failed challenge away from the charging call whoever the caller is.
fn validate_nuvei_challenge_result(
    redirect_response: Option<&ContinueRedirectionResponse>,
) -> Result<(), Report<IntegrationError>> {
    match classify_nuvei_challenge_result(redirect_response) {
        NuveiChallengeResult::Absent | NuveiChallengeResult::Passed => Ok(()),
        NuveiChallengeResult::Failed {
            field_name,
            additional_context,
            ..
        } => Err(Report::new(IntegrationError::InvalidDataFormat {
            field_name,
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Do not complete this payment: the 3DS authentication failed. Start a new payment"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(additional_context.to_string()),
            },
        })),
    }
}

/// PostAuthenticate - reports the result of the ACS challenge. Nuvei has no
/// call for this step, so nothing is sent: the cres the browser brought back
/// is decoded and the leg answers AuthenticationSuccessful for transStatus Y
/// and AuthenticationFailed for everything else, a missing result included.
/// Nuvei keeps the cavv and eci and applies them on the final /payment.do.
pub(crate) fn handle_post_authenticate_response<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >,
    _event_builder: Option<&mut common_utils::events::Event>,
    res: domain_types::router_response_types::Response,
) -> common_utils::errors::CustomResult<
    RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >,
    ConnectorError,
> {
    let authentication_failed = |reason: &str, acs_error: Option<NuveiAcsErrorResponse>| {
        let (code, message) = acs_error
            .map(|acs_error| {
                (
                    acs_error.error_code,
                    acs_error.error_detail.or(acs_error.error_message),
                )
            })
            .unwrap_or((None, None));
        domain_types::router_data::ErrorResponse {
            code: code
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: message
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| NUVEI_THREE_DS_AUTHENTICATION_FAILED.to_string()),
            reason: Some(reason.to_string()),
            status_code: res.status_code,
            attempt_status: Some(FlowStatus::Payment(
                common_enums::AttemptStatus::AuthenticationFailed,
            )),
            ..Default::default()
        }
    };

    let (status, response) =
        match classify_nuvei_challenge_result(data.request.redirect_response.as_ref()) {
            NuveiChallengeResult::Passed => (
                common_enums::AttemptStatus::AuthenticationSuccessful,
                Ok(PaymentsResponseData::PostAuthenticateResponse {
                    authentication_data: Some(AuthenticationData {
                        trans_status: Some(common_enums::TransactionStatus::Success),
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
                    }),
                    connector_response_reference_id: None,
                    status_code: res.status_code,
                }),
            ),
            NuveiChallengeResult::Failed {
                additional_context,
                acs_error,
                ..
            } => (
                common_enums::AttemptStatus::AuthenticationFailed,
                Err(authentication_failed(additional_context, acs_error)),
            ),
            NuveiChallengeResult::Absent => (
                common_enums::AttemptStatus::AuthenticationFailed,
                Err(authentication_failed(
                    "3ds Authentication failed: no challenge response (cres) was returned",
                    None,
                )),
            ),
        };

    Ok(RouterDataV2 {
        resource_common_data: PaymentFlowData {
            status,
            ..data.resource_common_data.clone()
        },
        response,
        ..data.clone()
    })
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
        // clientRequestId and clientUniqueId both carry the request reference id,
        // which has to fit Nuvei's clientUniqueId.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let client_request_id = client_unique_id.clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Send the connector_transaction_id returned by the authorization"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei /settleTransaction.do settles the Auth named by relatedTransactionId"
                                .to_string(),
                        ),
                    },
                }
                .into());
            }
        };

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.amount_to_capture.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert amount_to_capture to Nuvei major units".to_string(),
                    ),
                },
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

// PSync Response Transformation
impl TryFrom<ResponseRouterData<NuveiSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiSyncResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let transaction_details = response.transaction_details.as_ref();

        // `status` ERROR means the lookup itself failed; it says nothing about
        // the payment, so the payment status is left as it came in.
        let lookup_failed = matches!(response.status, NuveiPaymentStatus::Error);

        // The transaction is not readable yet (a lookup right after the
        // payment call): not a failure and not an error, as in hyperswitch.
        if lookup_failed && response.err_code == Some(NUVEI_TRANSACTION_NOT_FOUND_ERR_CODE) {
            return Ok(Self {
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: router_data.request.connector_transaction_id.clone(),
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

        let transaction_type = transaction_details.and_then(|details| details.transaction_type);
        // Hyperswitch reads the zero-amount verification from the capturable
        // amount of the payment, which a lookup carries only when it is known.
        let zero_amount = router_data
            .resource_common_data
            .amount_capturable
            .as_ref()
            .map(|amount_capturable| amount_capturable.amount)
            == Some(common_utils::types::MinorUnit::new(0));
        let status = get_nuvei_payment_status(
            zero_amount,
            transaction_type,
            transaction_details.and_then(|details| details.transaction_status.as_ref()),
            &response.status,
        );

        // A declined or errored transaction is returned as an error with the
        // failure status of its own transaction type; a declined Auth is a plain
        // Failure on a lookup, as in hyperswitch, where it is terminal.
        let failure_status = if matches!(
            status,
            common_enums::AttemptStatus::VoidFailed
                | common_enums::AttemptStatus::AuthenticationFailed
        ) {
            status
        } else {
            common_enums::AttemptStatus::Failure
        };
        if let Some(error_response) = build_nuvei_error_response_from_fields(
            NuveiErrorFields {
                status: &response.status,
                err_code: response.err_code,
                reason: response.reason.as_ref(),
                transaction_status: transaction_details
                    .and_then(|details| details.transaction_status.as_ref()),
                transaction_id: transaction_details
                    .and_then(|details| details.transaction_id.as_ref()),
                gw_error_code: transaction_details.and_then(|details| details.gw_error_code),
                gw_extended_error_code: transaction_details
                    .and_then(|details| details.gw_extended_error_code),
                gw_error_reason: transaction_details
                    .and_then(|details| details.gw_error_reason.as_ref()),
                merchant_advice_code: None,
                issuer_decline_code: None,
                issuer_decline_reason: None,
                payment_method_error_reason: None,
            },
            item.http_code,
            failure_status,
        ) {
            if lookup_failed {
                // An inconclusive lookup is never terminal for the payment.
                return Ok(Self {
                    response: Err(domain_types::router_data::ErrorResponse {
                        attempt_status: None,
                        ..error_response
                    }),
                    ..router_data.clone()
                });
            }
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: failure_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Extract transaction details
        let transaction_details = transaction_details.ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_details missing in Nuvei PSync response".to_string()),
            ))
        })?;

        // Get connector transaction ID from transaction_details
        let connector_transaction_id =
            transaction_details.transaction_id.clone().ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("transaction_id missing in Nuvei PSync transaction_details".to_string()),
                ))
            })?;

        // Partial approval, as hyperswitch reads it from a lookup: the
        // requested amount comes in partialApproval and the processed amount
        // in transactionDetails; it is what was captured (Sale) or what can be
        // captured (Auth).
        let processed_amount = response
            .partial_approval
            .as_ref()
            .filter(|partial_approval| {
                partial_approval.requested_amount.is_some()
                    && partial_approval.requested_currency.is_some()
            })
            .and_then(|_| {
                transaction_details
                    .processed_amount
                    .clone()
                    .zip(transaction_details.processed_currency)
            })
            .map(|(amount, currency)| {
                domain_types::utils::convert_back_amount_to_minor_units(
                    &common_utils::types::StringMajorUnitForConnector,
                    amount,
                    currency,
                )
                .map(|amount| common_utils::types::Money { amount, currency })
                .change_context(
                    ConnectorError::response_handling_failed_with_context(
                        item.http_code,
                        Some(
                            "invalid transactionDetails.processedAmount in Nuvei response"
                                .to_string(),
                        ),
                    ),
                )
            })
            .transpose()?;
        let (amount_captured, amount_capturable) = match (processed_amount, transaction_type) {
            (
                Some(processed_amount),
                Some(NuveiTransactionType::Sale) | Some(NuveiTransactionType::Auth3D),
            ) => (
                Some(processed_amount),
                router_data.resource_common_data.amount_capturable.clone(),
            ),
            (
                Some(processed_amount),
                Some(NuveiTransactionType::Auth) | Some(NuveiTransactionType::InitAuth3D),
            ) => (
                router_data.resource_common_data.amount_captured.clone(),
                Some(processed_amount),
            ),
            (
                Some(_),
                Some(NuveiTransactionType::Settle)
                | Some(NuveiTransactionType::Void)
                | Some(NuveiTransactionType::Credit)
                | Some(NuveiTransactionType::Refund)
                | Some(NuveiTransactionType::Sale3D)
                | Some(NuveiTransactionType::Unknown)
                | None,
            )
            | (None, _) => (
                router_data.resource_common_data.amount_captured.clone(),
                router_data.resource_common_data.amount_capturable.clone(),
            ),
        };

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|link_id| !link_id.is_empty()),
            connector_response_reference_id: transaction_details.client_unique_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                amount_capturable,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Capture Response Transformation
impl TryFrom<ResponseRouterData<NuveiCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // A rejected request, a declined settle and a gateway error (a settle
        // above the authorized amount is transactionStatus ERROR) are all a
        // failed capture.
        if let Some(error_response) = build_nuvei_error_response_from_fields(
            NuveiErrorFields {
                status: &response.status,
                err_code: response.err_code,
                reason: response.reason.as_ref(),
                transaction_status: response.transaction_status.as_ref(),
                transaction_id: response.transaction_id.as_ref(),
                gw_error_code: response.gw_error_code,
                gw_extended_error_code: response.gw_extended_error_code,
                gw_error_reason: response.gw_error_reason.as_ref(),
                merchant_advice_code: response.merchant_advice_code.as_ref(),
                issuer_decline_code: response.issuer_decline_code.as_ref(),
                issuer_decline_reason: response.issuer_decline_reason.as_ref(),
                payment_method_error_reason: response.payment_method_error_reason.as_ref(),
            },
            item.http_code,
            common_enums::AttemptStatus::CaptureFailed,
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Charged only when Nuvei says the approved transaction is a Settle; an
        // approval without a transaction type stays Pending until it is synced
        // (spec: Status Mappings).
        let status = get_nuvei_payment_status(
            false,
            response.transaction_type,
            response.transaction_status.as_ref(),
            &response.status,
        );
        // A failure in this flow is a failed capture, whatever the type says.
        let status = if matches!(
            status,
            common_enums::AttemptStatus::Failure
                | common_enums::AttemptStatus::AuthorizationFailed
                | common_enums::AttemptStatus::VoidFailed
                | common_enums::AttemptStatus::AuthenticationFailed
        ) {
            common_enums::AttemptStatus::CaptureFailed
        } else {
            status
        };

        // The settle response does not echo the amount: what was captured is
        // the amount this request asked to settle.
        let amount_captured = if status == common_enums::AttemptStatus::Charged {
            Some(router_data.request.amount_to_capture.clone())
        } else {
            router_data.resource_common_data.amount_captured.clone()
        };

        // Get connector transaction ID
        let connector_transaction_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei capture response".to_string()),
            ))
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
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
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                amount_captured,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
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
        // clientRequestId and clientUniqueId both carry the request reference id,
        // which has to fit Nuvei's clientUniqueId.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let client_request_id = client_unique_id.clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.refund_amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the refund amount to Nuvei major units".to_string(),
                    ),
                },
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

        let time_stamp = NuveiAuthType::get_timestamp();

        // The lookup names the refund (Credit) transaction by its own
        // transactionId, never the payment. clientUniqueId is not sent: the
        // one of this request is not the one the refund was created with, and
        // Nuvei rejects a transactionId / clientUniqueId pair that does not match.
        let transaction_id = router_data.request.connector_refund_id.clone();

        if transaction_id.is_empty() {
            return Err(IntegrationError::MissingRequiredField {
                field_name: "connector_refund_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the connector_refund_id returned by the refund call".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /getTransactionDetails.do looks a refund up by the transactionId of the refund"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        // Generate checksum for getTransactionDetails without clientUniqueId: merchantId + merchantSiteId + transactionId + timeStamp + merchantSecretKey
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

        // A rejected request (status ERROR) and a gateway error (a refund above
        // the remaining amount is transactionStatus ERROR) are an error
        // response. A DECLINED refund is not: it is a failed refund without an
        // error (spec: Refund declined by the gateway).
        let is_refund_error = matches!(response.status, NuveiPaymentStatus::Error)
            || matches!(
                response.transaction_status,
                Some(NuveiTransactionStatus::Error)
            );
        let error_response = is_refund_error
            .then(|| {
                build_nuvei_error_response_from_fields(
                    NuveiErrorFields {
                        status: &response.status,
                        err_code: response.err_code,
                        reason: response.reason.as_ref(),
                        transaction_status: response.transaction_status.as_ref(),
                        transaction_id: response.transaction_id.as_ref(),
                        gw_error_code: response.gw_error_code,
                        gw_extended_error_code: response.gw_extended_error_code,
                        gw_error_reason: response.gw_error_reason.as_ref(),
                        merchant_advice_code: response.merchant_advice_code.as_ref(),
                        issuer_decline_code: response.issuer_decline_code.as_ref(),
                        issuer_decline_reason: response.issuer_decline_reason.as_ref(),
                        payment_method_error_reason: response.payment_method_error_reason.as_ref(),
                    },
                    item.http_code,
                    common_enums::AttemptStatus::Failure,
                )
            })
            .flatten();
        if let Some(error_response) = error_response {
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    // The failure of this flow is a failed refund, not a failed payment.
                    attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                    ..error_response
                }),
                ..router_data.clone()
            });
        }

        // Refund status comes from transactionStatus alone (spec: Status
        // Mappings); transactionType is not consulted.
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Unknown) => common_enums::RefundStatus::Pending,
            // No transactionStatus: an asynchronous refund answers status PENDING
            // and settles by DMN; a SUCCESS that names no transaction result is
            // not a refund.
            None => match response.status {
                NuveiPaymentStatus::Pending
                | NuveiPaymentStatus::Processing
                | NuveiPaymentStatus::Unknown => common_enums::RefundStatus::Pending,
                NuveiPaymentStatus::Success
                | NuveiPaymentStatus::Failed
                | NuveiPaymentStatus::Error => common_enums::RefundStatus::Failure,
            },
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

// Refund Sync Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<NuveiRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;
        let transaction_details = response.transaction_details.as_ref();
        let transaction_status =
            transaction_details.and_then(|details| details.transaction_status.as_ref());

        // The sync says nothing about the refund. `RefundStatus::Unknown`
        // serializes to the proto's Unspecified, on which the caller keeps the
        // status it has stored. NOT `request.refund_status`: the sync request
        // carries no status, so that field is always Pending and would turn a
        // succeeded refund back to pending.
        let unchanged_refund = || {
            let refund_status = common_enums::RefundStatus::Unknown;
            Self {
                resource_common_data: RefundFlowData {
                    status: refund_status,
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(RefundsResponseData {
                    connector_refund_id: router_data.request.connector_refund_id.clone(),
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..router_data.clone()
            }
        };

        // `status` ERROR means the lookup itself failed; it says nothing about
        // the refund, so the refund status is left for the caller to keep.
        let lookup_failed = matches!(response.status, NuveiPaymentStatus::Error);

        // The refund is not readable yet (a lookup right after the refund
        // call): not a failure and not an error, as in hyperswitch.
        if lookup_failed && response.err_code == Some(NUVEI_TRANSACTION_NOT_FOUND_ERR_CODE) {
            return Ok(unchanged_refund());
        }

        // A failed lookup and a refund in transactionStatus ERROR are an error
        // response. A DECLINED refund is not: it is a failed refund without an
        // error, as in the refund call.
        let is_refund_error =
            lookup_failed || matches!(transaction_status, Some(NuveiTransactionStatus::Error));
        let error_response = is_refund_error
            .then(|| {
                build_nuvei_error_response_from_fields(
                    NuveiErrorFields {
                        status: &response.status,
                        err_code: response.err_code,
                        reason: response.reason.as_ref(),
                        transaction_status,
                        transaction_id: transaction_details
                            .and_then(|details| details.transaction_id.as_ref()),
                        gw_error_code: transaction_details
                            .and_then(|details| details.gw_error_code),
                        gw_extended_error_code: transaction_details
                            .and_then(|details| details.gw_extended_error_code),
                        gw_error_reason: transaction_details
                            .and_then(|details| details.gw_error_reason.as_ref()),
                        merchant_advice_code: None,
                        issuer_decline_code: None,
                        issuer_decline_reason: None,
                        payment_method_error_reason: None,
                    },
                    item.http_code,
                    common_enums::AttemptStatus::Failure,
                )
            })
            .flatten();
        if let Some(error_response) = error_response {
            if lookup_failed {
                // An inconclusive lookup is never terminal for the refund.
                return Ok(Self {
                    response: Err(domain_types::router_data::ErrorResponse {
                        attempt_status: None,
                        ..error_response
                    }),
                    ..router_data.clone()
                });
            }
            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    // The failure of this flow is a failed refund, not a failed payment.
                    attempt_status: Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
                    ..error_response
                }),
                ..router_data.clone()
            });
        }

        // Refund status comes from transactionDetails.transactionStatus alone
        // (spec: RSync); transactionType, which Nuvei spells Credit, refund or
        // Refund for a refund, is not consulted.
        let refund_status = match transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::RefundStatus::Failure
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | Some(NuveiTransactionStatus::Redirect)
            | Some(NuveiTransactionStatus::Unknown) => common_enums::RefundStatus::Pending,
            // A lookup that names no transaction result does not decide the refund.
            None => return Ok(unchanged_refund()),
        };

        // Get connector refund ID from transaction_details
        let connector_refund_id = transaction_details
            .and_then(|details| details.transaction_id.clone())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "transaction_id missing in Nuvei refund sync transaction_details"
                            .to_string(),
                    ),
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
        // clientRequestId and clientUniqueId both carry the request reference id,
        // which has to fit Nuvei's clientUniqueId.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;
        let client_request_id = client_unique_id.clone();

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Extract amount and currency from the request
        // For void, we need to send the original transaction amount and currency
        let minor_amount = router_data.request.amount.as_ref().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the amount of the original transaction with the void".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /voidTransaction.do compares amount with the original transaction"
                            .to_string(),
                    ),
                },
            }
        })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the currency of the original transaction with the void".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /voidTransaction.do compares currency with the original transaction"
                            .to_string(),
                    ),
                },
            }
                })?;

        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount.amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the void amount to Nuvei major units".to_string(),
                    ),
                },
            })?;

        // Generate checksum: merchantId + merchantSiteId + clientRequestId + clientUniqueId + amount + currency + relatedTransactionId + "" + "" + timeStamp + merchantSecretKey
        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &client_unique_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &related_transaction_id,
            "", // authCode (empty)
            "", // comment (empty)
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

        // A rejected request, a declined void and a gateway error (a second
        // void of the same transaction is transactionStatus ERROR) are all a
        // failed void.
        if let Some(error_response) = build_nuvei_error_response_from_fields(
            NuveiErrorFields {
                status: &response.status,
                err_code: response.err_code,
                reason: response.reason.as_ref(),
                transaction_status: response.transaction_status.as_ref(),
                transaction_id: response.transaction_id.as_ref(),
                gw_error_code: response.gw_error_code,
                gw_extended_error_code: response.gw_extended_error_code,
                gw_error_reason: response.gw_error_reason.as_ref(),
                merchant_advice_code: response.merchant_advice_code.as_ref(),
                issuer_decline_code: response.issuer_decline_code.as_ref(),
                issuer_decline_reason: response.issuer_decline_reason.as_ref(),
                payment_method_error_reason: response.payment_method_error_reason.as_ref(),
            },
            item.http_code,
            common_enums::AttemptStatus::VoidFailed,
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Voided only when Nuvei says the approved transaction is a Void; an
        // approval without a transaction type stays Pending until it is synced
        // (spec: Status Mappings).
        let status = get_nuvei_payment_status(
            false,
            response.transaction_type,
            response.transaction_status.as_ref(),
            &response.status,
        );
        // A failure in this flow is a failed void, whatever the type says.
        let status = if matches!(
            status,
            common_enums::AttemptStatus::Failure
                | common_enums::AttemptStatus::AuthorizationFailed
                | common_enums::AttemptStatus::CaptureFailed
                | common_enums::AttemptStatus::AuthenticationFailed
        ) {
            common_enums::AttemptStatus::VoidFailed
        } else {
            status
        };

        // Get connector transaction ID
        let connector_transaction_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei void response".to_string()),
            ))
        })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
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
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
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
    pub session_token: Option<String>,
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

        // Check if the overall request status is ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
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
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
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
                NuveiClientAuthenticationResponseDomain {
                    session_token: Secret::new(session_token),
                },
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
    pub session_token: Option<String>,
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

/// Nuvei's `threeD.v2supported` is documented as a boolean and sent as a
/// string. Both forms are read; same shape as `str_or_i64` above.
fn str_or_bool<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrBool {
        Str(String),
        Bool(bool),
    }

    Ok(
        Option::<StrOrBool>::deserialize(deserializer)?.map(|v| match v {
            StrOrBool::Str(s) => s,
            StrOrBool::Bool(b) => b.to_string(),
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
            .convert(
                router_data.request.amount.amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
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

impl TryFrom<NuveiOpenOrderResponse> for PaymentCreateOrderResponse {
    type Error = Report<ConnectorError>;

    fn try_from(response: NuveiOpenOrderResponse) -> Result<Self, Self::Error> {
        let connector_order_id = response.order_id.unwrap_or_default();
        Ok(Self {
            connector_order_id,
            session_data: None,
        })
    }
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
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

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
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
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

        let order_response = PaymentCreateOrderResponse::try_from(response.clone())?;

        // Extract order_id to store for Authorize flow
        let order_id = order_response.connector_order_id.clone();

        // Store session_token in session_token field for use by Authorize flow
        let session_token = response.session_token.clone();

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
// Authorize flow. The request shape mirrors NuveiPaymentRequest with isRebilling
// set to "0" to indicate this is the initial (customer-initiated) mandate
// payment. A successful response returns a userPaymentOptionId which is used
// as the connector_mandate_id for subsequent merchant-initiated recurring
// payments via the RepeatPayment flow.

/// SetupMandate request - same shape as NuveiPaymentRequest plus isRebilling flag.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_unique_id: Option<String>,
    /// userTokenId is required for Nuvei to register the card as a reusable
    /// payment option and return a non-empty `userPaymentOptionId` in the
    /// response - this is what downstream MIT/RepeatPayment calls reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token_id: Option<String>,
    pub payment_option: NuveiPaymentOption<T>,
    /// "0" marks the initial CIT transaction of a recurring series.
    pub is_rebilling: String,
    /// transactionId of the 3DS payment this call completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transaction_id: Option<String>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

/// SetupMandate response - the shared /payment.do envelope. Its paymentOption
/// carries the userPaymentOptionId returned by Nuvei for future MIT calls.
pub type NuveiSetupMandateResponse = NuveiPaymentResponse;

/// Minimal paymentOption view on the response - we only need userPaymentOptionId
/// which is used as the connector_mandate_id.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponsePaymentOption {
    pub user_payment_option_id: Option<String>,
}

// Build the SetupMandate request from the router data. Matches the Authorize
// transformer closely - the only deltas are isRebilling="0" and using
// SetupMandateRequestData fields (amount/currency are optional here).
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

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // clientUniqueId carries the merchant reference; Nuvei caps it at 45 characters.
        let client_unique_id = get_valid_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        )?;

        // Nuvei SetupMandate supports Card and NetworkToken payment_method_data.
        let payment_option = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = router_data
                    .resource_common_data
                    .get_optional_billing_full_name();

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        three_d: None,
                        stored_credentials: None,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            // Network token: expiry + externalToken only, no PAN/CVV/holder name.
            PaymentMethodData::NetworkToken(token_data) => NuveiPaymentOption {
                card: Some(NuveiCardPaymentOption::NetworkToken(
                    build_nuvei_network_token_card(token_data),
                )),
                alternative_payment_method: None,
                user_payment_option_id: None,
            },
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method not supported by Nuvei for SetupMandate".to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a card or a network token to set up a Nuvei mandate".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei SetupRecurring stores a card credential (userPaymentOptionId) through /payment.do"
                                .to_string(),
                        ),
                    },
                )
                .into())
            }
        };

        // State of the Authenticate leg, when this call completes a 3DS setup.
        let three_ds_state = router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .filter(|_| router_data.resource_common_data.is_three_ds())
            .and_then(|feature_data| {
                serde_json::from_value::<NuveiThreeDsState>(feature_data.peek().clone()).ok()
            })
            .filter(|state| !state.related_transaction_id.is_empty());

        // A 3DS setup is the final leg of the 3DS sequence: it needs the
        // result of the Authenticate call. Without one it is refused instead
        // of being sent as a non-3DS zero auth.
        if router_data.resource_common_data.is_three_ds() && three_ds_state.is_none() {
            return Err(IntegrationError::NotSupported {
                message: "3DS mandate setup without an Authenticate result is not supported"
                    .to_string(),
                connector: "nuvei",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Run PreAuthenticate and Authenticate first and send the Authenticate connector_feature_data on SetupRecurring, or send auth_type NO_THREE_DS"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "A Nuvei 3DS mandate setup needs the PreAuthenticate and Authenticate calls; SetupRecurring is the final /payment.do and carries their transaction id as relatedTransactionId"
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        // Billing address - Nuvei requires email and country.
        let billing_address = get_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        )
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "billing_address (email and country required)",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send the billing address country and an email (billing or customer)"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "Nuvei /payment.do requires billingAddress.email and billingAddress.country"
                        .to_string(),
                ),
            },
        })?;

        // Device details - ipAddress required by Nuvei. It is also stored with
        // the mandate for the later merchant-initiated payments.
        let ip_address = router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send browser_info.ip_address on the SetupRecurring request".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /payment.do requires deviceDetails.ipAddress".to_string(),
                    ),
                },
            })?;
        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address.to_string()),
        };

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // For SetupMandate amount is optional; default to 0 if absent so that
        // Nuvei treats this as a zero-value auth verification for the mandate.
        let minor_amount = router_data
            .request
            .amount
            .as_ref()
            .map(|money| money.amount)
            .unwrap_or(common_utils::types::MinorUnit::new(0));
        let currency = router_data.request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the SetupMandate amount to Nuvei major units"
                            .to_string(),
                    ),
                },
            })?;

        // Session token populated by ServerSessionAuthenticationToken flow. The
        // final call of a 3DS setup stays on the session of its earlier legs.
        let session_token = match three_ds_state
            .as_ref()
            .and_then(|state| state.session_token.as_ref())
            .map(|token| token.peek().to_string())
            .filter(|token| !token.is_empty())
        {
            Some(session_token) => session_token,
            None => get_nuvei_session_token(&router_data.resource_common_data)?,
        };

        // transactionId of the 3DS payment made by the Authenticate leg, which
        // this call completes. Not sent on a non-3DS setup.
        let related_transaction_id = three_ds_state.map(|state| state.related_transaction_id);

        // Always Auth for mandate setup - we don't want to capture funds.
        let transaction_type = TransactionType::Auth;

        let url_details =
            router_data
                .request
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

        // Nuvei requires a userTokenId on the initial CIT call so that it
        // binds the card to a reusable userPaymentOptionId.
        let user_token_id = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|c| c.get_string_repr().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send customer.id on the SetupRecurring request".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei binds the stored credential to userTokenId (the customer id); without it no userPaymentOptionId is returned"
                            .to_string(),
                    ),
                },
            })?;

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            client_unique_id: Some(client_unique_id),
            user_token_id: Some(user_token_id),
            payment_option,
            is_rebilling: "0".to_string(),
            related_transaction_id,
            transaction_type,
            device_details,
            billing_address,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

// Map the Nuvei SetupMandate response onto the SetupMandate RouterDataV2.
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

        // A setup without an amount is sent as a zero-amount Auth, which is a
        // card verification: once approved it is complete.
        let zero_amount = !router_data
            .request
            .amount
            .as_ref()
            .is_some_and(|money| money.amount != common_utils::types::MinorUnit::new(0));
        let status = get_nuvei_payment_status(
            zero_amount,
            response.transaction_type,
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = get_nuvei_connector_response(response);

        // A 2xx response can still be a rejected request, a decline or a
        // gateway error; those are returned as errors with this flow's status
        // and never carry a mandate reference.
        let failure_status = if status == common_enums::AttemptStatus::AuthorizationFailed {
            common_enums::AttemptStatus::AuthorizationFailed
        } else {
            common_enums::AttemptStatus::Failure
        };
        if let Some(error_response) =
            build_nuvei_error_response(response, item.http_code, failure_status)
        {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: failure_status,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        let connector_transaction_id = response
            .transaction_id
            .clone()
            .or(response.order_id.clone())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "missing transaction_id and order_id in Nuvei SetupMandate response"
                            .to_string(),
                    ),
                ))
            })?;

        // userPaymentOptionId is the connector mandate id for future MITs; the
        // customer's IP is kept with it for those MITs, as hyperswitch does.
        let mandate_reference = response
            .payment_option
            .as_ref()
            .and_then(|payment_option| payment_option.user_payment_option_id.as_ref())
            .map(|id| id.peek().to_string())
            .filter(|id| !id.is_empty())
            .map(|id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: router_data
                        .request
                        .browser_info
                        .as_ref()
                        .and_then(|browser_info| browser_info.ip_address)
                        .map(|ip_address| {
                            Secret::new(serde_json::Value::String(ip_address.to_string()))
                        }),
                })
            });

        // An approved setup without a userPaymentOptionId is unusable
        // downstream (RepeatPayment needs it), so it is reported as a failure.
        let is_approved = matches!(
            status,
            common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::Authorized
        );
        if is_approved && mandate_reference.is_none() {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: consts::NO_ERROR_CODE.to_string(),
                    message: "Nuvei SetupMandate response missing userPaymentOptionId".to_string(),
                    reason: Some(
                        "Nuvei SetupMandate response missing userPaymentOptionId".to_string(),
                    ),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: Some(connector_transaction_id),
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

        // Network transaction id (NTID) of the scheme, when Nuvei returns one.
        let network_txn_id = response
            .external_scheme_transaction_id
            .as_ref()
            .map(|ntid| ntid.peek().to_string())
            .filter(|ntid| !ntid.is_empty());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference,
            connector_metadata: None,
            network_txn_id,
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|link_id| !link_id.is_empty()),
            connector_response_reference_id: response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ===== RepeatPayment (MIT) flow =====
//
// Merchant-initiated recurring charges reuse the same /ppp/api/v1/payment.do
// endpoint as Authorize/SetupMandate with isRebilling="1" and either the stored
// userPaymentOptionId from the initial SetupMandate response, or the card (or
// network token) together with the scheme's network transaction id.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_unique_id: Option<String>,
    /// userTokenId must match the value used on the initial SetupMandate so
    /// Nuvei resolves the stored payment option correctly (the customer id).
    /// Not sent on a network-token MIT by network transaction id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token_id: Option<String>,
    pub payment_option: NuveiRepeatPaymentOptionTypes<T>,
    /// "1" marks a merchant-initiated rebilling transaction; not sent on a
    /// network-token MIT by network transaction id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rebilling: Option<String>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    /// billingAddress.email and billingAddress.country are mandatory on
    /// every MIT.
    pub billing_address: NuveiBillingAddress,
    /// Original-transaction reference (NTID + brand) for card and
    /// network-token MITs by network transaction id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_scheme_details: Option<NuveiExternalSchemeDetails>,
    /// Mastercard transaction link id of the original CIT, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_link_id: Option<String>,
    /// Return URLs, sent when the request carries one, as on Authorize.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Serialize-only untagged enum: stored-credential MIT keeps its exact previous
// wire shape; network-token MIT emits paymentOption.card with externalToken;
// card + NTID MIT emits paymentOption.card without a CVV
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiRepeatPaymentOptionTypes<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    StoredCredential(NuveiRepeatPaymentOption),
    NetworkToken(NuveiRepeatPaymentCardOption),
    NtidCard(NuveiRepeatPaymentNtidCardOption<T>),
}

/// paymentOption payload for stored-credential MIT - only userPaymentOptionId
/// is required; Nuvei reuses the stored card bound to this id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentOption {
    pub user_payment_option_id: String,
}

/// paymentOption payload for network-token MIT.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentCardOption {
    pub card: NuveiNetworkTokenCard,
}

/// paymentOption payload for a card MIT by network transaction id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentNtidCardOption<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card: NuveiNtidCard<T>,
}

/// card object of a card MIT by network transaction id: the card without a
/// CVV (mirrors hyperswitch get_ntid_card_info).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiNtidCard<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_holder_name: Option<Secret<String>>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
}

/// RepeatPayment response - the shared /payment.do envelope.
pub type NuveiRepeatPaymentResponse = NuveiPaymentResponse;

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
    > for NuveiRepeatPaymentRequest<T>
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

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // A Nuvei MIT runs on a stored card (userPaymentOptionId), on a card
        // with the network transaction id, or on a network token with it.
        match &router_data.request.payment_method_data {
            PaymentMethodData::MandatePayment
            | PaymentMethodData::Card(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_) => {}
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Payment method not supported by Nuvei for RepeatPayment".to_string(),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Charge a connector_mandate_id, or a card or network token with a network transaction id"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei merchant-initiated payments are card payments through /payment.do"
                                .to_string(),
                        ),
                    },
                )
                .into())
            }
        }

        // userTokenId must match the initial SetupMandate, which sends the
        // customer id; the connector customer id is accepted in its place.
        // A network-token MIT by network transaction id is not a rebill of a
        // stored payment option: it carries neither field, as in hyperswitch.
        let (user_token_id, is_rebilling) = match &router_data.request.mandate_reference {
            MandateReferenceId::NetworkTokenWithNTI(_) => (None, None),
            MandateReferenceId::ConnectorMandateId(_) | MandateReferenceId::NetworkMandateId(_) => {
                let user_token_id = router_data
                    .resource_common_data
                    .customer_id
                    .as_ref()
                    .map(|customer_id| customer_id.get_string_repr().to_string())
                    .or_else(|| router_data.resource_common_data.connector_customer.clone())
                    .filter(|user_token_id| !user_token_id.is_empty())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "customer_id",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Send customer.id (the one used on SetupRecurring) or connector_customer_id on the Charge request"
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Nuvei requires userTokenId on a rebilling payment (error 1144)"
                                    .to_string(),
                            ),
                        },
                    })?;
                (Some(user_token_id), Some("1".to_string()))
            }
        };

        // Stored-credential MIT uses Nuvei's own ConnectorMandateId (the
        // userPaymentOptionId returned by SetupMandate); the other two send
        // the card or the network token from payment_method plus the NTID
        // from the mandate reference (mirrors hyperswitch PR #13093).
        let (payment_option, external_scheme_details, transaction_link_id, mandate_ip_address) =
            match &router_data.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(c) => {
                    let user_payment_option_id = c
                        .get_connector_mandate_id()
                        .filter(|id| !id.is_empty())
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "mandate_reference.connector_mandate_id",
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Send the connector_mandate_id returned by SetupRecurring"
                                        .to_string(),
                                ),
                                doc_url: None,
                                additional_context: Some(
                                    "It is sent to Nuvei as paymentOption.userPaymentOptionId"
                                        .to_string(),
                                ),
                            },
                        })?;

                    // The customer's IP stored with the mandate at setup.
                    let mandate_ip_address = c
                        .get_mandate_metadata()
                        .and_then(|metadata| metadata.peek().as_str().map(str::to_string));

                    (
                        NuveiRepeatPaymentOptionTypes::StoredCredential(NuveiRepeatPaymentOption {
                            user_payment_option_id,
                        }),
                        None,
                        None,
                        mandate_ip_address,
                    )
                }
                MandateReferenceId::NetworkTokenWithNTI(nti_ref) => {
                    let token_data = match &router_data.request.payment_method_data {
                        PaymentMethodData::NetworkToken(token_data) => token_data,
                        _ => {
                            return Err(IntegrationError::NotSupported {
                                message:
                                    "Nuvei network-token MIT requires payment_method.network_token on the Charge request"
                                        .to_string(),
                                connector: "nuvei",
                                context: IntegrationErrorContext {
                                    suggested_action: Some(
                                        "Send payment_method.network_token with network_token_with_nti"
                                            .to_string(),
                                    ),
                                    doc_url: None,
                                    additional_context: Some(
                                        "The network token is sent as paymentOption.card.externalToken"
                                            .to_string(),
                                    ),
                                },
                            }
                            .into())
                        }
                    };

                    (
                        NuveiRepeatPaymentOptionTypes::NetworkToken(NuveiRepeatPaymentCardOption {
                            card: build_nuvei_network_token_card(token_data),
                        }),
                        Some(NuveiExternalSchemeDetails {
                            transaction_id: get_nuvei_network_transaction_id(
                                &nti_ref.network_transaction_id,
                            )?,
                            brand: Some(get_nuvei_card_brand(token_data)?),
                        }),
                        None,
                        None,
                    )
                }
                // Card MIT by network transaction id: the card without a CVV
                // plus the NTID and brand of the original transaction.
                MandateReferenceId::NetworkMandateId(network_mandate) => {
                    let (card, card_network) = match &router_data.request.payment_method_data {
                        PaymentMethodData::Card(card) => (
                            NuveiNtidCard {
                                card_number: card.card_number.clone(),
                                card_holder_name: card.card_holder_name.clone(),
                                expiration_month: card.card_exp_month.clone(),
                                expiration_year: card.card_exp_year.clone(),
                            },
                            card.card_network.clone(),
                        ),
                        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                            NuveiNtidCard {
                                card_number: card.card_number.clone(),
                                card_holder_name: card.card_holder_name.clone(),
                                expiration_month: card.card_exp_month.clone(),
                                expiration_year: card.card_exp_year.clone(),
                            },
                            card.card_network.clone(),
                        ),
                        _ => {
                            return Err(IntegrationError::NotSupported {
                                message:
                                    "Nuvei MIT by network transaction id requires card data on the Charge request"
                                        .to_string(),
                                connector: "nuvei",
                                context: IntegrationErrorContext {
                                    suggested_action: Some(
                                        "Send payment_method.card_details_for_network_transaction_id with network_mandate_id"
                                            .to_string(),
                                    ),
                                    doc_url: None,
                                    additional_context: Some(
                                        "The card is sent as paymentOption.card without a CVV"
                                            .to_string(),
                                    ),
                                },
                            }
                            .into())
                        }
                    };
                    // Brand: the explicit card network, else the BIN issuer.
                    let brand = match card_network {
                        Some(network) => NuveiCardType::try_from(network)?,
                        None => NuveiCardType::try_from(&domain_types::utils::get_card_issuer(
                            card.card_number.peek(),
                        )?)?,
                    };

                    (
                        NuveiRepeatPaymentOptionTypes::NtidCard(NuveiRepeatPaymentNtidCardOption {
                            card,
                        }),
                        Some(NuveiExternalSchemeDetails {
                            transaction_id: get_nuvei_network_transaction_id(
                                &network_mandate.network_transaction_id,
                            )?,
                            brand: Some(brand),
                        }),
                        network_mandate
                            .transaction_link_id
                            .clone()
                            .filter(|link_id| !link_id.is_empty()),
                        None,
                    )
                }
            };

        // deviceDetails.ipAddress is mandatory: the IP of this request, else
        // the customer's IP kept with the mandate (as hyperswitch does).
        let ip_address = router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address)
            .map(|ip_address| ip_address.to_string())
            .or(mandate_ip_address)
            .filter(|ip_address| !ip_address.is_empty())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "browser_info.ip_address",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send browser_info.ip_address, or the mandate_metadata returned by SetupRecurring"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /payment.do requires deviceDetails.ipAddress".to_string(),
                    ),
                },
            })?;
        let device_details = NuveiDeviceDetails {
            ip_address: Secret::new(ip_address),
        };

        // Billing address - email and country are mandatory on every MIT.
        let billing_address = get_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        )
        .ok_or_else(|| {
            let has_email = router_data
                .resource_common_data
                .get_optional_billing_email()
                .or(router_data.request.email.clone())
                .is_some();
            IntegrationError::MissingRequiredField {
                field_name: if has_email {
                    "billing_address.country"
                } else {
                    "billing_address.email"
                },
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send the billing address country and an email (billing or customer)"
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Nuvei /payment.do requires billingAddress.email and billingAddress.country"
                            .to_string(),
                    ),
                },
            }
        })?;

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = get_valid_client_unique_id(&client_request_id)?;

        let minor_amount = router_data.request.amount.amount;
        let currency = router_data.request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(
                        "Failed to convert the RepeatPayment amount to Nuvei major units"
                            .to_string(),
                    ),
                },
            })?;

        // Nuvei's short-lived session token is passed via state.access_token
        // on the Charge request.
        let session_token = get_nuvei_session_token(&router_data.resource_common_data)?;

        // Sale captures the MIT in one step; Auth when the caller asks for
        // manual capture. Nuvei knows no other capture mode, so any other
        // intent is refused instead of being sent as a Sale, as on Authorize.
        let transaction_type = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => TransactionType::Auth,
            Some(common_enums::CaptureMethod::Automatic)
            | Some(common_enums::CaptureMethod::SequentialAutomatic)
            | None => TransactionType::Sale,
            Some(common_enums::CaptureMethod::ManualMultiple)
            | Some(common_enums::CaptureMethod::Scheduled) => {
                return Err(IntegrationError::CaptureMethodNotSupported {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use capture_method AUTOMATIC or MANUAL for Nuvei".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Nuvei /payment.do supports transactionType Sale and Auth only"
                                .to_string(),
                        ),
                    },
                }
                .into())
            }
        };

        let checksum = auth.generate_checksum(&[
            auth.merchant_id.peek(),
            auth.merchant_site_id.peek(),
            &client_request_id,
            &amount.get_amount_as_string(),
            &currency.to_string(),
            &time_stamp.to_string(),
        ]);

        let url_details =
            router_data
                .request
                .router_return_url
                .as_ref()
                .map(|url| NuveiUrlDetails {
                    success_url: url.clone(),
                    failure_url: url.clone(),
                    pending_url: url.clone(),
                });

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            amount,
            currency,
            client_unique_id: Some(client_unique_id),
            user_token_id,
            payment_option,
            is_rebilling,
            transaction_type,
            device_details,
            billing_address,
            external_scheme_details,
            transaction_link_id,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

/// externalSchemeDetails.transactionId of an MIT: the scheme's network
/// transaction id from the mandate reference, refused when empty.
fn get_nuvei_network_transaction_id(
    network_transaction_id: &str,
) -> Result<Secret<String>, Report<IntegrationError>> {
    if network_transaction_id.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "network_transaction_id",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send the network transaction id of the original customer-initiated payment"
                        .to_string(),
                ),
                doc_url: None,
                additional_context: Some(
                    "It is sent to Nuvei as externalSchemeDetails.transactionId".to_string(),
                ),
            },
        }
        .into());
    }
    Ok(Secret::new(network_transaction_id.to_string()))
}

// Map the Nuvei RepeatPayment response onto the RepeatPayment RouterDataV2.
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

        // An MIT is never the zero-amount verification, so an approved Auth
        // (manual capture) is Authorized and an approved Sale is Charged.
        let status = get_nuvei_payment_status(
            false,
            response.transaction_type,
            response.transaction_status.as_ref(),
            &response.status,
        );

        let connector_response = get_nuvei_connector_response(response);

        // A 2xx response can still be a rejected request, a decline or a
        // gateway error; those are returned as errors with this flow's
        // status, carrying the merchant advice code when Nuvei returns one.
        let failure_status = if status == common_enums::AttemptStatus::AuthorizationFailed {
            common_enums::AttemptStatus::AuthorizationFailed
        } else {
            common_enums::AttemptStatus::Failure
        };
        if let Some(error_response) =
            build_nuvei_error_response(response, item.http_code, failure_status)
        {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: failure_status,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        let connector_transaction_id = response
            .transaction_id
            .clone()
            .or(response.order_id.clone())
            .ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some(
                        "missing transaction_id and order_id in Nuvei RepeatPayment response"
                            .to_string(),
                    ),
                ))
            })?;

        // Network transaction id (NTID) of the scheme, when Nuvei returns one.
        let network_txn_id = response
            .external_scheme_transaction_id
            .as_ref()
            .map(|ntid| ntid.peek().to_string())
            .filter(|ntid| !ntid.is_empty());

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id,
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|link_id| !link_id.is_empty()),
            connector_response_reference_id: response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// ---------------------------------------------------------------------------
// Incoming webhooks: payment DMNs and Control Panel chargeback events
// (spec: Webhook Events)
// ---------------------------------------------------------------------------

/// productId is part of the payment-DMN checksum; a DMN without one is signed
/// with this literal (hyperswitch rule).
pub(super) const NUVEI_DMN_ABSENT_PRODUCT_ID: &str = "NA";

/// A DMN whose clientRequestId starts with this prefix belongs to a payout.
const NUVEI_PAYOUT_REFERENCE_PREFIX: &str = "payout_";

/// Any notification Nuvei sends to the merchant DMN URL: a payment DMN
/// (form-encoded) or a Control Panel chargeback event (JSON).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NuveiWebhook {
    PaymentDmn(Box<NuveiPaymentDmn>),
    Chargeback(Box<NuveiChargebackDmn>),
}

/// `ppp_status` of a payment DMN.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiDmnApiStatus {
    Ok,
    Fail,
    Pending,
    #[serde(other)]
    Unknown,
}

/// `Status` of a payment DMN.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiDmnStatus {
    Approved,
    Success,
    Declined,
    Error,
    Pending,
    Update,
    #[serde(other)]
    Unknown,
}

impl NuveiDmnStatus {
    /// The upper-cased wire value as it enters the DMN checksum. `None` for a
    /// value this connector does not know: its spelling is not kept, so the
    /// checksum cannot be rebuilt and the DMN stays unverified.
    pub(super) fn checksum_value(self) -> Option<&'static str> {
        match self {
            Self::Approved => Some("APPROVED"),
            Self::Success => Some("SUCCESS"),
            Self::Declined => Some("DECLINED"),
            Self::Error => Some("ERROR"),
            Self::Pending => Some("PENDING"),
            Self::Update => Some("UPDATE"),
            Self::Unknown => None,
        }
    }
}

/// Payment DMN (spec: Webhook Payload Structure).
#[derive(Debug, Deserialize)]
pub struct NuveiPaymentDmn {
    pub ppp_status: Option<NuveiDmnApiStatus>,
    #[serde(rename = "PPP_TransactionID")]
    pub ppp_transaction_id: String,
    #[serde(rename = "TransactionID")]
    pub transaction_id: Option<String>,
    #[serde(rename = "relatedTransactionId")]
    pub related_transaction_id: Option<String>,
    #[serde(rename = "Status")]
    pub status: Option<NuveiDmnStatus>,
    #[serde(rename = "transactionType")]
    pub transaction_type: Option<NuveiTransactionType>,
    #[serde(rename = "totalAmount")]
    pub total_amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(rename = "responseTimeStamp")]
    pub response_time_stamp: String,
    #[serde(rename = "productId")]
    pub product_id: Option<String>,
    #[serde(rename = "advanceResponseChecksum")]
    pub advance_response_checksum: Option<Secret<String>>,
    #[serde(rename = "ErrCode")]
    pub err_code: Option<String>,
    #[serde(rename = "Reason")]
    pub reason: Option<String>,
    #[serde(rename = "ReasonCode")]
    pub reason_code: Option<String>,
    #[serde(rename = "merchantAdviceCode")]
    pub merchant_advice_code: Option<String>,
    #[serde(rename = "clientRequestId")]
    pub client_request_id: Option<String>,
    #[serde(rename = "clientUniqueId")]
    pub client_unique_id: Option<String>,
}

impl NuveiPaymentDmn {
    /// Payout DMNs are not handled by this connector.
    pub(super) fn is_payout(&self) -> bool {
        self.client_request_id
            .as_deref()
            .is_some_and(|id| id.starts_with(NUVEI_PAYOUT_REFERENCE_PREFIX))
    }
}

/// `EventType` of a Control Panel event DMN.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum NuveiControlPanelEventType {
    Chargeback,
    #[serde(other)]
    Unknown,
}

/// Chargeback DMN (spec: Webhook Payload Structure).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NuveiChargebackDmn {
    pub event_type: NuveiControlPanelEventType,
    pub chargeback: NuveiChargebackData,
    pub transaction_details: NuveiChargebackTransactionDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NuveiChargebackData {
    pub chargeback_status_category: Option<NuveiChargebackStatusCategory>,
    #[serde(rename = "Type")]
    pub chargeback_type: Option<NuveiChargebackType>,
    pub reported_amount: FloatMajorUnit,
    pub reported_currency: common_enums::Currency,
    pub chargeback_reason: Option<String>,
    pub chargeback_reason_category: Option<String>,
    pub dispute_id: Option<String>,
    pub dispute_unified_status_code: Option<NuveiDisputeUnifiedStatusCode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NuveiChargebackTransactionDetails {
    pub transaction_id: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum NuveiChargebackType {
    Chargeback,
    Retrieval,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum NuveiChargebackStatusCategory {
    Regular,
    #[serde(rename = "cancelled")]
    Cancelled,
    Duplicate,
    #[serde(rename = "RDR-Refund")]
    RdrRefund,
    #[serde(rename = "Soft_CB")]
    SoftCb,
    #[serde(other)]
    Unknown,
}

/// `Chargeback.DisputeUnifiedStatusCode` (spec: Webhook Payload Structure).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub enum NuveiDisputeUnifiedStatusCode {
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

/// What a payment DMN reports: the event, and the status of the payment or
/// refund it belongs to.
#[derive(Debug, Clone)]
pub(super) enum NuveiDmnEvent {
    Payment {
        event_type: EventType,
        status: common_enums::AttemptStatus,
    },
    Refund {
        event_type: EventType,
        status: common_enums::RefundStatus,
    },
}

impl NuveiDmnEvent {
    pub(super) fn event_type(&self) -> EventType {
        match self {
            Self::Payment { event_type, .. } | Self::Refund { event_type, .. } => {
                event_type.clone()
            }
        }
    }
}

/// (Status, transactionType) of a payment DMN -> event (spec: Webhook ->
/// event mapping). The refund guide's DMN samples carry transactionType
/// Refund, so it is treated like Credit. `None` is "event type not found":
/// the DMN changes no state.
pub(super) fn map_nuvei_dmn_to_event(
    status: NuveiDmnStatus,
    transaction_type: NuveiTransactionType,
) -> Option<NuveiDmnEvent> {
    let payment = |event_type, status| Some(NuveiDmnEvent::Payment { event_type, status });
    let refund = |event_type, status| Some(NuveiDmnEvent::Refund { event_type, status });
    match status {
        NuveiDmnStatus::Approved | NuveiDmnStatus::Success => match transaction_type {
            NuveiTransactionType::Auth => payment(
                EventType::PaymentIntentAuthorizationSuccess,
                common_enums::AttemptStatus::Authorized,
            ),
            NuveiTransactionType::Sale => payment(
                EventType::PaymentIntentSuccess,
                common_enums::AttemptStatus::Charged,
            ),
            NuveiTransactionType::Settle => payment(
                EventType::PaymentIntentCaptureSuccess,
                common_enums::AttemptStatus::Charged,
            ),
            NuveiTransactionType::Void => payment(
                EventType::PaymentIntentCancelled,
                common_enums::AttemptStatus::Voided,
            ),
            NuveiTransactionType::Credit | NuveiTransactionType::Refund => refund(
                EventType::RefundSuccess,
                common_enums::RefundStatus::Success,
            ),
            NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::Sale3D
            | NuveiTransactionType::Unknown => None,
        },
        NuveiDmnStatus::Declined | NuveiDmnStatus::Error => match transaction_type {
            NuveiTransactionType::Auth => payment(
                EventType::PaymentIntentAuthorizationFailure,
                common_enums::AttemptStatus::AuthorizationFailed,
            ),
            NuveiTransactionType::Sale => payment(
                EventType::PaymentIntentFailure,
                common_enums::AttemptStatus::Failure,
            ),
            NuveiTransactionType::Settle => payment(
                EventType::PaymentIntentCaptureFailure,
                common_enums::AttemptStatus::CaptureFailed,
            ),
            NuveiTransactionType::Void => payment(
                EventType::PaymentIntentCancelFailure,
                common_enums::AttemptStatus::VoidFailed,
            ),
            NuveiTransactionType::Credit | NuveiTransactionType::Refund => refund(
                EventType::RefundFailure,
                common_enums::RefundStatus::Failure,
            ),
            NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::Sale3D
            | NuveiTransactionType::Unknown => None,
        },
        NuveiDmnStatus::Pending => match transaction_type {
            NuveiTransactionType::Auth
            | NuveiTransactionType::Sale
            | NuveiTransactionType::Settle => payment(
                EventType::PaymentIntentProcessing,
                common_enums::AttemptStatus::Pending,
            ),
            NuveiTransactionType::Void
            | NuveiTransactionType::Credit
            | NuveiTransactionType::Refund
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::Sale3D
            | NuveiTransactionType::Unknown => None,
        },
        NuveiDmnStatus::Update | NuveiDmnStatus::Unknown => None,
    }
}

/// Reference of a payment DMN: TransactionID is the payment for Auth / Sale /
/// Settle / Void (and the 3DS types), the refund for Credit / Refund, whose
/// relatedTransactionId is the refunded payment.
///
/// A payment is identified by TransactionID only. clientUniqueId echoes the
/// connector request reference sent on /payment.do, which is not the id a
/// caller looks a payment up by as `merchant_transaction_id`, so returning it
/// makes the lookup miss. A refund keeps clientUniqueId as its merchant
/// refund id.
pub(super) fn get_nuvei_dmn_reference(
    dmn: NuveiPaymentDmn,
) -> Result<WebhookResourceReference, Report<WebhookError>> {
    let non_empty = |value: Option<String>| value.filter(|value| !value.is_empty());
    let transaction_id = non_empty(dmn.transaction_id);
    match dmn.transaction_type {
        Some(
            NuveiTransactionType::Auth
            | NuveiTransactionType::Sale
            | NuveiTransactionType::Settle
            | NuveiTransactionType::Void
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::Sale3D,
        ) => Ok(WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: Some(
                transaction_id.ok_or(WebhookError::WebhookReferenceIdNotFound)?,
            ),
            merchant_transaction_id: None,
        })),
        Some(NuveiTransactionType::Credit | NuveiTransactionType::Refund) => {
            Ok(WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(
                    transaction_id.ok_or(WebhookError::WebhookReferenceIdNotFound)?,
                ),
                merchant_refund_id: non_empty(dmn.client_unique_id),
                connector_transaction_id: non_empty(dmn.related_transaction_id),
                merchant_transaction_id: None,
            }))
        }
        Some(NuveiTransactionType::Unknown) | None => {
            Err(WebhookError::WebhookEventTypeNotFound.into())
        }
    }
}

/// DisputeUnifiedStatusCode -> dispute status (spec: Webhook -> event
/// mapping), with the ChargebackStatusCategory fallback for a code that maps
/// to nothing. `None` is "event type not found".
pub(super) fn map_nuvei_dispute_to_event(
    chargeback: &NuveiChargebackData,
) -> Option<common_enums::DisputeStatus> {
    let from_code = chargeback
        .dispute_unified_status_code
        .and_then(|code| match code {
            NuveiDisputeUnifiedStatusCode::FirstChargebackInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::CreditChargebackInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::McCollaborationInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedRecall
            | NuveiDisputeUnifiedStatusCode::InquiryInitiatedByIssuer => {
                Some(common_enums::DisputeStatus::DisputeOpened)
            }
            NuveiDisputeUnifiedStatusCode::CreditChargebackAcceptedAutomatically
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedAutomatically
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedAutomaticallyMcoll
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackDisputeResponseNotAllowed
            | NuveiDisputeUnifiedStatusCode::Rdr
            | NuveiDisputeUnifiedStatusCode::McCollaborationRefundedByMerchant
            | NuveiDisputeUnifiedStatusCode::McCollaborationAutomaticAccept
            | NuveiDisputeUnifiedStatusCode::InquiryAcceptedFullRefund
            | NuveiDisputeUnifiedStatusCode::PreArbitrationAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationPartiallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationAutomaticallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::RejectedPreArbAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::RejectedPreArbExpiredAutoAccepted => {
                Some(common_enums::DisputeStatus::DisputeAccepted)
            }
            NuveiDisputeUnifiedStatusCode::FirstChargebackNoResponseExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackPartiallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedCardholderFavour
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedCardholderFavour
            | NuveiDisputeUnifiedStatusCode::McCollaborationClosedCardholderFavour => {
                Some(common_enums::DisputeStatus::DisputeLost)
            }
            NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedAutomatically
            | NuveiDisputeUnifiedStatusCode::PreArbitrationInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationRejectedByIssuer
            | NuveiDisputeUnifiedStatusCode::InquiryRespondedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationRejectedByMerchant => {
                Some(common_enums::DisputeStatus::DisputeChallenged)
            }
            NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedAutomaticallyExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackPartiallyAcceptedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::McCollaborationExpired
            | NuveiDisputeUnifiedStatusCode::InquiryExpired
            | NuveiDisputeUnifiedStatusCode::PreArbitrationPartiallyAcceptedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::PreArbitrationRejectedByMerchantExpired => {
                Some(common_enums::DisputeStatus::DisputeExpired)
            }
            NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationAcceptedByIssuer
            | NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationPartiallyAcceptedByIssuer
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedMerchantFavour
            | NuveiDisputeUnifiedStatusCode::McCollaborationClosedMerchantFavour
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedMerchantFavour => {
                Some(common_enums::DisputeStatus::DisputeWon)
            }
            NuveiDisputeUnifiedStatusCode::FirstChargebackRecalledByIssuer
            | NuveiDisputeUnifiedStatusCode::InquiryCancelledAfterRefund
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedRecall
            | NuveiDisputeUnifiedStatusCode::CreditChargebackRecalledByIssuer => {
                Some(common_enums::DisputeStatus::DisputeCancelled)
            }
            NuveiDisputeUnifiedStatusCode::McCollaborationPreviouslyRefundedAuto
            | NuveiDisputeUnifiedStatusCode::McCollaborationRejectedByMerchant
            | NuveiDisputeUnifiedStatusCode::InquiryAutomaticallyRejected
            | NuveiDisputeUnifiedStatusCode::InquiryPartialAcceptedPartialRefund
            | NuveiDisputeUnifiedStatusCode::InquiryUpdated
            | NuveiDisputeUnifiedStatusCode::Unknown => None,
        });

    from_code.or_else(|| {
        chargeback
            .chargeback_status_category
            .and_then(|category| match category {
                NuveiChargebackStatusCategory::Cancelled
                | NuveiChargebackStatusCategory::Duplicate => {
                    Some(common_enums::DisputeStatus::DisputeCancelled)
                }
                NuveiChargebackStatusCategory::RdrRefund => {
                    Some(common_enums::DisputeStatus::DisputeAccepted)
                }
                NuveiChargebackStatusCategory::Regular
                | NuveiChargebackStatusCategory::SoftCb
                | NuveiChargebackStatusCategory::Unknown => None,
            })
    })
}

pub(super) fn get_nuvei_dispute_event_type(status: common_enums::DisputeStatus) -> EventType {
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

/// Dispute stage (spec: Webhook -> event mapping): by status code first, then
/// by Type, then by ChargebackStatusCategory. `DisputeStage` has no reversal
/// stage, so a recalled credit chargeback (CC-I-RCLL) and the cancelled /
/// Duplicate categories stay in the dispute stage.
pub(super) fn get_nuvei_dispute_stage(
    chargeback: &NuveiChargebackData,
) -> Option<common_enums::DisputeStage> {
    let from_code = chargeback
        .dispute_unified_status_code
        .and_then(|code| match code {
            NuveiDisputeUnifiedStatusCode::Rdr
            | NuveiDisputeUnifiedStatusCode::InquiryInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::InquiryRespondedByMerchant
            | NuveiDisputeUnifiedStatusCode::InquiryExpired
            | NuveiDisputeUnifiedStatusCode::InquiryAutomaticallyRejected
            | NuveiDisputeUnifiedStatusCode::InquiryCancelledAfterRefund
            | NuveiDisputeUnifiedStatusCode::InquiryAcceptedFullRefund
            | NuveiDisputeUnifiedStatusCode::InquiryPartialAcceptedPartialRefund
            | NuveiDisputeUnifiedStatusCode::InquiryUpdated => {
                Some(common_enums::DisputeStage::PreDispute)
            }
            NuveiDisputeUnifiedStatusCode::FirstChargebackInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::CreditChargebackInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::FirstChargebackNoResponseExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedAutomatically
            | NuveiDisputeUnifiedStatusCode::FirstChargebackAcceptedAutomaticallyMcoll
            | NuveiDisputeUnifiedStatusCode::FirstChargebackPartiallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackPartiallyAcceptedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedByMerchant
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedAutomatically
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRejectedAutomaticallyExpired
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedMerchantFavour
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedCardholderFavour
            | NuveiDisputeUnifiedStatusCode::FirstChargebackClosedRecall
            | NuveiDisputeUnifiedStatusCode::FirstChargebackRecalledByIssuer
            | NuveiDisputeUnifiedStatusCode::FirstChargebackDisputeResponseNotAllowed
            | NuveiDisputeUnifiedStatusCode::McCollaborationInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::McCollaborationPreviouslyRefundedAuto
            | NuveiDisputeUnifiedStatusCode::McCollaborationRefundedByMerchant
            | NuveiDisputeUnifiedStatusCode::McCollaborationExpired
            | NuveiDisputeUnifiedStatusCode::McCollaborationRejectedByMerchant
            | NuveiDisputeUnifiedStatusCode::McCollaborationAutomaticAccept
            | NuveiDisputeUnifiedStatusCode::McCollaborationClosedMerchantFavour
            | NuveiDisputeUnifiedStatusCode::McCollaborationClosedCardholderFavour
            | NuveiDisputeUnifiedStatusCode::CreditChargebackAcceptedAutomatically
            | NuveiDisputeUnifiedStatusCode::CreditChargebackRecalledByIssuer => {
                Some(common_enums::DisputeStage::Dispute)
            }
            NuveiDisputeUnifiedStatusCode::PreArbitrationInitiatedByIssuer
            | NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationAcceptedByIssuer
            | NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationRejectedByIssuer
            | NuveiDisputeUnifiedStatusCode::MerchantPreArbitrationPartiallyAcceptedByIssuer
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedMerchantFavour
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedCardholderFavour
            | NuveiDisputeUnifiedStatusCode::PreArbitrationAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationPartiallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationPartiallyAcceptedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::PreArbitrationRejectedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationRejectedByMerchantExpired
            | NuveiDisputeUnifiedStatusCode::PreArbitrationAutomaticallyAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::PreArbitrationClosedRecall
            | NuveiDisputeUnifiedStatusCode::RejectedPreArbAcceptedByMerchant
            | NuveiDisputeUnifiedStatusCode::RejectedPreArbExpiredAutoAccepted => {
                Some(common_enums::DisputeStage::PreArbitration)
            }
            NuveiDisputeUnifiedStatusCode::Unknown => None,
        });

    from_code
        .or(match chargeback.chargeback_type {
            Some(NuveiChargebackType::Retrieval) => Some(common_enums::DisputeStage::PreDispute),
            Some(NuveiChargebackType::Chargeback | NuveiChargebackType::Unknown) | None => None,
        })
        .or(match chargeback.chargeback_status_category {
            Some(
                NuveiChargebackStatusCategory::Cancelled
                | NuveiChargebackStatusCategory::Duplicate
                | NuveiChargebackStatusCategory::Regular,
            ) => Some(common_enums::DisputeStage::Dispute),
            Some(NuveiChargebackStatusCategory::RdrRefund) => {
                Some(common_enums::DisputeStage::PreDispute)
            }
            Some(NuveiChargebackStatusCategory::SoftCb) => {
                Some(common_enums::DisputeStage::PreArbitration)
            }
            Some(NuveiChargebackStatusCategory::Unknown) | None => None,
        })
}

/// The chargeback DMN checksum covers every JSON value of the payload,
/// concatenated in payload order (spec: Webhook Authentication & Signature
/// Verification). A null contributes nothing.
pub(super) fn concat_nuvei_json_values(value: &serde_json::Value, output: &mut String) {
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::Bool(flag) => output.push_str(&flag.to_string()),
        serde_json::Value::Number(number) => output.push_str(&number.to_string()),
        serde_json::Value::String(text) => output.push_str(text),
        serde_json::Value::Array(values) => values
            .iter()
            .for_each(|value| concat_nuvei_json_values(value, output)),
        serde_json::Value::Object(fields) => fields
            .values()
            .for_each(|value| concat_nuvei_json_values(value, output)),
    }
}
