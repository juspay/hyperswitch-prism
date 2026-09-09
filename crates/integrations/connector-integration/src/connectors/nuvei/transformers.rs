use common_utils::{consts, pii, request::Method, types::StringMajorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, CreateOrder, PSync, RSync, Refund,
        RepeatPayment, SetupMandate, Void,
    },
    connector_types::{
        ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, MandateReference, MandateReferenceId,
        NuveiClientAuthenticationResponse as NuveiClientAuthenticationResponseDomain,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{
        BankDebitData, BankRedirectData, BankTransferData, NetworkTokenData, PaymentMethodData,
        PaymentMethodDataTypes, RawCardNumber,
    },
    router_data::{ConnectorSpecificConfig, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use super::NuveiRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::{ConnectorError, IntegrationError};

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
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
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
    pub payment_option: NuveiPaymentOption<T>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<NuveiShippingAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_descriptor: Option<NuveiDynamicDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
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
    pub card_holder_name: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    #[serde(rename = "CVV")]
    pub cvv: Secret<String>,
    /// Only populated for merchant-supplied (external MPI) 3DS. Nuvei rejects a
    /// `threeD` object on the post-challenge final `/payment.do`, so this stays
    /// `None` for every non-external-MPI path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_d: Option<NuveiThreeD>,
}

/// `paymentOption.card.threeD`. Only the `externalMpi` member is populated by
/// UCS today - the browser-challenge 3DS members belong to `/initPayment.do`,
/// which this connector does not drive.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiThreeD {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_mpi: Option<NuveiExternalMpi>,
}

/// `paymentOption.card.threeD.externalMpi` - merchant-supplied 3DS values.
///
/// Per the Nuvei External-MPI reference this object has exactly five members.
/// `threeDSVersion` and `xid` are **not** members: the 3DS message version is
/// the sibling `threeD.version`, and XID is a 3DS-1 concept Nuvei does not
/// accept here.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiExternalMpi {
    pub eci: Option<String>,
    pub cavv: Secret<String>,
    /// JSON key is `dsTransID` - capital `ID`.
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
    /// `ExemptionRequest` or `NoPreference`. Mandatory whenever external MPI
    /// values are sent.
    pub challenge_preference: Option<String>,
    /// Mandatory when `challengePreference == "ExemptionRequest"`.
    pub exemption_request_reason: Option<String>,
}

impl NuveiExternalMpi {
    /// Build the external-MPI block from the domain 3DS authentication data.
    /// Returns `None` when the merchant did not supply a CAVV, which is the
    /// one member Nuvei treats as non-optional.
    fn from_authentication_data(
        auth_data: &domain_types::router_request_types::AuthenticationData,
    ) -> Option<Self> {
        let cavv = auth_data.cavv.clone()?;
        // Nuvei accepts only these four exemption reasons; anything else is
        // sent as a plain `NoPreference` rather than an invalid literal.
        let exemption_request_reason =
            auth_data
                .exemption_indicator
                .as_ref()
                .and_then(|indicator| match indicator {
                    common_enums::ExemptionIndicator::LowValue => Some("LowValuePayment"),
                    common_enums::ExemptionIndicator::TransactionRiskAssessment => {
                        Some("TransactionRiskAnalysis")
                    }
                    _ => None,
                });
        let challenge_preference = Some(
            if exemption_request_reason.is_some() {
                "ExemptionRequest"
            } else {
                "NoPreference"
            }
            .to_string(),
        );
        Some(Self {
            eci: auth_data.eci.clone(),
            cavv,
            ds_trans_id: auth_data.ds_trans_id.clone(),
            challenge_preference,
            exemption_request_reason: exemption_request_reason.map(String::from),
        })
    }
}

/// Root-level `dynamicDescriptor` - the text and phone the cardholder sees on
/// their statement. Does not participate in the checksum.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiDynamicDescriptor {
    pub merchant_name: Option<Secret<String>>,
    pub merchant_phone: Option<Secret<String>>,
}

/// Nuvei field length limits for `dynamicDescriptor` (API reference).
const NUVEI_MERCHANT_NAME_MAX_LENGTH: usize = 25;
const NUVEI_MERCHANT_PHONE_MAX_LENGTH: usize = 13;
/// Nuvei rejects a `clientUniqueId` longer than 45 characters.
const NUVEI_CLIENT_UNIQUE_ID_MAX_LENGTH: usize = 45;

impl NuveiDynamicDescriptor {
    /// Build `dynamicDescriptor` from the domain billing descriptor. Returns
    /// `None` when neither member is populated - Nuvei rejects an empty object
    /// on some accounts.
    fn from_billing_descriptor(
        descriptor: &domain_types::connector_types::BillingDescriptor,
    ) -> Result<Option<Self>, Report<IntegrationError>> {
        let merchant_name = descriptor
            .name
            .as_ref()
            .map(|name| name.peek().trim().to_string())
            .or_else(|| {
                descriptor
                    .statement_descriptor
                    .as_ref()
                    .map(|descriptor| descriptor.trim().to_string())
            })
            .filter(|name| !name.is_empty())
            .map(|name| {
                Secret::new(
                    name.chars()
                        .take(NUVEI_MERCHANT_NAME_MAX_LENGTH)
                        .collect::<String>(),
                )
            });

        let merchant_phone = descriptor.phone.clone();
        if let Some(phone) = merchant_phone.as_ref() {
            if phone.peek().len() > NUVEI_MERCHANT_PHONE_MAX_LENGTH {
                return Err(IntegrationError::InvalidDataFormat {
                    field_name: "billing_descriptor.phone",
                    context: Default::default(),
                }
                .into());
            }
        }

        if merchant_name.is_none() && merchant_phone.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            merchant_name,
            merchant_phone,
        }))
    }
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
    // Nuvei's JSON keys for the extra street lines are `address2` / `address3`,
    // NOT the camelCased `addressLine2` / `addressLine3`.
    #[serde(rename = "address2", skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<Secret<String>>,
    #[serde(rename = "address3", skip_serializing_if = "Option::is_none")]
    pub address_line3: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
}

/// Root-level `shippingAddress`, a sibling of `billingAddress` on
/// `/payment.do`. Every member is optional. Note that the county key differs
/// from billing's (`shippingCounty` vs `county`) - neither is populated today
/// because the UCS `AddressDetails` carries no county field.
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiShippingAddress {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    #[serde(rename = "address2")]
    pub address_line2: Option<Secret<String>>,
    #[serde(rename = "address3")]
    pub address_line3: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub state: Option<Secret<String>>,
    pub zip: Option<Secret<String>>,
    pub country: Option<String>,
    pub email: Option<pii::Email>,
    pub phone: Option<Secret<String>>,
}

/// Build a Nuvei `shippingAddress` from `PaymentFlowData`. Returns `None` when
/// the caller supplied no shipping address at all, so the object is omitted
/// rather than sent empty.
fn get_shipping_address(resource_data: &PaymentFlowData) -> Option<NuveiShippingAddress> {
    resource_data.get_optional_shipping()?;
    let shipping = NuveiShippingAddress {
        first_name: resource_data.get_optional_shipping_first_name(),
        last_name: resource_data.get_optional_shipping_last_name(),
        address: resource_data.get_optional_shipping_line1(),
        address_line2: resource_data.get_optional_shipping_line2(),
        address_line3: resource_data.get_optional_shipping_line3(),
        city: resource_data.get_optional_shipping_city(),
        state: resource_data.get_optional_shipping_state(),
        zip: resource_data.get_optional_shipping_zip(),
        country: resource_data
            .get_optional_shipping_country()
            .map(|country| country.to_string()),
        email: resource_data.get_optional_shipping_email(),
        // Nuvei's `phone` is a plain String(18); the `_plain` helper is the one
        // that does not require a separate dialling code, matching how the
        // billing phone is sourced.
        phone: resource_data.get_optional_shipping_phone_number_plain(),
    };
    // Nuvei rejects an empty object on some accounts.
    let is_empty = shipping.first_name.is_none()
        && shipping.last_name.is_none()
        && shipping.address.is_none()
        && shipping.address_line2.is_none()
        && shipping.address_line3.is_none()
        && shipping.city.is_none()
        && shipping.state.is_none()
        && shipping.zip.is_none()
        && shipping.country.is_none()
        && shipping.email.is_none()
        && shipping.phone.is_none();
    (!is_empty).then_some(shipping)
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
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    /// Filter / risk-rejection detail. Meaningful when `gwErrorCode == -1100`.
    #[serde(rename = "gwExtendedErrorCode")]
    pub gw_extended_error_code: Option<i64>,
    /// Mastercard Merchant Advice Code - the network advice code.
    pub merchant_advice_code: Option<String>,
    /// The issuer's own decline code / text, where the acquirer forwards it.
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    /// Stage-3 (APM provider) failure pair. Not applicable to raw card.
    pub payment_method_error_code: Option<i64>,
    pub payment_method_error_reason: Option<String>,
    /// Network Transaction ID (NTID), to be stored for later MITs.
    pub external_scheme_transaction_id: Option<String>,
    /// Mastercard Transaction Link ID.
    pub transaction_link_id: Option<String>,
    pub auth_code: Option<String>,
    pub session_token: Option<String>,
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
    pub card: Option<NuveiResponseCard>,
    pub user_payment_option_id: Option<String>,
}

/// `paymentOption.card` on a `/payment.do` or `/getTransactionDetails.do`
/// response. Carries the issuer's AVS and CVV2 verdicts.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiResponseCard {
    /// AVS result: `X`,`Y`,`A`,`W`,`Z`,`N`,`U`,`S`,`R`,`B`. Empty unless the
    /// request carried `billingAddress.address` / `billingAddress.zip`.
    pub avs_code: Option<String>,
    /// CVV2 result: `M`,`N`,`P`,`U`,`S`.
    pub cvv2_reply: Option<String>,
    pub card_brand: Option<String>,
    /// Older responses spell the brand `brand` rather than `cardBrand`.
    pub brand: Option<String>,
    pub card_type: Option<String>,
    pub bin: Option<String>,
    pub last4_digits: Option<String>,
    pub issuer_bank_name: Option<String>,
    pub issuer_country: Option<String>,
}

impl NuveiResponseCard {
    fn card_network(&self) -> Option<String> {
        self.card_brand.clone().or_else(|| self.brand.clone())
    }
}

/// AVS code -> human description (Nuvei AVS reference).
fn get_avs_response_description(code: &str) -> Option<&'static str> {
    match code {
        "X" => Some("Exact match of both the 9-digit ZIP code and the street address."),
        "Y" => Some("Postal code and the street address match."),
        "A" => Some("The street address matches, the ZIP code does not."),
        "W" => Some("Postal code matches, the street address does not."),
        "Z" => Some("Postal code matches, the street address does not."),
        "N" => Some("Both the street address and postal code do not match."),
        "U" => Some("Issuer is unavailable."),
        "S" => Some("AVS not supported by issuer."),
        "R" => Some("Retry."),
        "B" => Some("Not authorized (declined)."),
        _ => None,
    }
}

/// cvv2Reply code -> human description (Nuvei CVV reference).
fn get_cvv2_response_description(code: &str) -> Option<&'static str> {
    match code {
        "M" => Some("CVV2 Match"),
        "N" => Some("CVV2 No Match"),
        "P" => Some("Not Processed. For EU card-on-file and e-commerce network-token transactions Visa strips the CVV and returns P."),
        "U" => Some("Issuer is not certified and/or has not supplied Visa the encryption keys."),
        "S" => Some("CVV2 processor is unavailable."),
        _ => None,
    }
}

/// Surface the issuer's AVS and CVV2 verdicts (plus the processor card brand)
/// as a structured `payment_checks` blob on `connector_response`, rather than
/// folding them into the error message.
fn build_nuvei_connector_response(
    payment_option: Option<&PaymentOption>,
    auth_code: Option<String>,
) -> Option<domain_types::router_data::ConnectorResponseData> {
    let card = payment_option?.card.as_ref()?;
    let avs_code = card.avs_code.as_deref().filter(|code| !code.is_empty());
    let cvv2_code = card.cvv2_reply.as_deref().filter(|code| !code.is_empty());
    let card_network = card.card_network().filter(|brand| !brand.is_empty());
    // Nuvei sends `authCode: ""` on a decline; an empty string is not an auth code.
    let auth_code = auth_code.filter(|code| !code.is_empty());

    if avs_code.is_none() && cvv2_code.is_none() && card_network.is_none() && auth_code.is_none() {
        return None;
    }

    let mut payment_checks = serde_json::Map::new();
    if let Some(code) = avs_code {
        payment_checks.insert("avs_result".to_string(), serde_json::json!(code));
        payment_checks.insert(
            "avs_description".to_string(),
            serde_json::json!(get_avs_response_description(code)),
        );
    }
    if let Some(code) = cvv2_code {
        payment_checks.insert(
            "card_validation_result".to_string(),
            serde_json::json!(code),
        );
        payment_checks.insert(
            "card_validation_description".to_string(),
            serde_json::json!(get_cvv2_response_description(code)),
        );
    }

    Some(
        domain_types::router_data::ConnectorResponseData::with_additional_payment_method_data(
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: (!payment_checks.is_empty())
                    .then(|| serde_json::Value::Object(payment_checks)),
                card_network,
                domestic_network: None,
                auth_code,
            },
        ),
    )
}

/// The three error stages Nuvei reports, gathered from one response so the
/// precedence rule can be applied in one place.
pub(crate) struct NuveiErrorFields {
    pub status: NuveiPaymentStatus,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub gw_error_code: Option<i32>,
    pub gw_error_reason: Option<String>,
    pub gw_extended_error_code: Option<i64>,
    pub merchant_advice_code: Option<String>,
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub payment_method_error_code: Option<i64>,
    pub payment_method_error_reason: Option<String>,
    pub transaction_id: Option<String>,
}

/// Apply Nuvei's three-stage error model.
///
/// * stage 1 - `status == ERROR`: the request was rejected before the gateway;
///   the code/message are `errCode` / `reason`.
/// * stage 2 - `status == SUCCESS` but `transactionStatus` is `DECLINED` /
///   `ERROR`: a declined card still returns a top-level `SUCCESS`, so the
///   decline text has to come from `gwErrorCode` / `gwErrorReason`.
/// * stage 3 - the APM pair, preferred over the gateway pair when populated.
///
/// Returns `None` when the response is not an error at all.
fn build_nuvei_error_response(
    fields: NuveiErrorFields,
    http_code: u16,
    flow_status: FlowStatus,
) -> Option<domain_types::router_data::ErrorResponse> {
    let is_gateway_failure = matches!(
        fields.transaction_status,
        Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error)
    ) || fields
        .gw_error_reason
        .as_deref()
        .is_some_and(|reason| reason == "Missing argument");

    let (code, message) = match fields.status {
        NuveiPaymentStatus::Error => (
            fields.err_code.map(|code| code.to_string()),
            fields.reason.clone(),
        ),
        _ if is_gateway_failure => {
            // Stage 3 (APM provider) detail wins over the gateway pair when
            // the APM reported its own failure.
            match (
                fields.payment_method_error_code,
                fields.payment_method_error_reason.clone(),
            ) {
                (None, None) => (
                    fields.gw_error_code.map(|code| code.to_string()),
                    fields.gw_error_reason.clone(),
                ),
                (code, reason) => (
                    code.map(|code| code.to_string())
                        .or_else(|| fields.gw_error_code.map(|code| code.to_string())),
                    reason.or_else(|| fields.gw_error_reason.clone()),
                ),
            }
        }
        _ => return None,
    };

    // `gwExtendedErrorCode` is the Nuvei risk-filter code when
    // `gwErrorCode == -1100`; keep it alongside the gateway code so GSM can
    // key on the specific filter (1104 invalid CVV2, 1119/1120 AVS, ...).
    let network_decline_code = fields
        .issuer_decline_code
        .clone()
        .filter(|code| !code.is_empty())
        .or_else(
            || match (fields.gw_error_code, fields.gw_extended_error_code) {
                (Some(gw), Some(extended)) if extended != 0 => Some(format!("{gw}:{extended}")),
                (Some(gw), _) => Some(gw.to_string()),
                (None, Some(extended)) => Some(extended.to_string()),
                (None, None) => None,
            },
        );

    let network_error_message = fields
        .issuer_decline_reason
        .clone()
        .filter(|reason| !reason.is_empty())
        .or_else(|| fields.gw_error_reason.clone())
        .filter(|reason| !reason.is_empty());

    let message = message
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string());

    Some(domain_types::router_data::ErrorResponse {
        code: code
            .filter(|code| !code.is_empty())
            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message: message.clone(),
        reason: Some(message),
        status_code: http_code,
        attempt_status: Some(flow_status),
        connector_transaction_id: fields.transaction_id.clone(),
        network_advice_code: fields
            .merchant_advice_code
            .clone()
            .filter(|code| !code.is_empty()),
        network_decline_code,
        network_error_message,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    })
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NuveiPaymentStatus {
    Success,
    Failed,
    Error,
    #[default]
    Processing,
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
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    pub internal_request_id: Option<i64>,
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
    pub version: Option<String>,
    pub transaction_details: Option<NuveiTransactionDetails>,
    /// `/getTransactionDetails.do` echoes the same `paymentOption.card` block
    /// as `/payment.do`, carrying the AVS and CVV2 verdicts.
    #[serde(rename = "paymentOption")]
    pub payment_option: Option<PaymentOption>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiTransactionDetails {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub auth_code: Option<String>,
    pub client_unique_id: Option<String>,
    pub date: Option<String>,
    pub original_transaction_date: Option<String>,
    pub credited: Option<String>,
    pub acquiring_bank_name: Option<String>,
    pub transaction_type: Option<String>,
    pub processed_amount: Option<String>,
    pub processed_currency: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    #[serde(rename = "gwExtendedErrorCode")]
    pub gw_extended_error_code: Option<i64>,
}

// ---------------------------------------------------------------------------
// Level 2 / Level 3 addendums
//
// Nuvei accepts interchange-optimisation data ONLY as `addendums.l23processingData`
// on `/settleTransaction.do` - never on `/payment.do`, and only on the
// Auth->Settle path (an auto-capture `Sale` cannot carry it). It must not be
// confused with the root-level `items` / `amountDetails` basket that
// `/payment.do` accepts for risk scoring, which is a different object with
// different field names.
//
// `addendums` does not participate in the settle checksum (row 5 of the
// checksum table), so adding it does not change the signed string.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiAddendums {
    #[serde(rename = "l23processingData")]
    pub l23_processing_data: NuveiL23ProcessingData,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiL23ProcessingData {
    /// `0` tax not included, `1` state/provincial tax included, `2` not subject to tax.
    pub tax_indicator: Option<String>,
    pub customer_code: Option<String>,
    #[serde(rename = "merchantVATRegNum")]
    pub merchant_vat_reg_num: Option<Secret<String>>,
    #[serde(rename = "customerVATRegNum")]
    pub customer_vat_reg_num: Option<Secret<String>>,
    pub destination_zip: Option<Secret<String>>,
    pub ship_from_zip: Option<Secret<String>>,
    pub destination_country_code: Option<String>,
    /// `YYMMDD`.
    pub order_date: Option<String>,
    pub line_item_count: Option<String>,
    pub items: Option<Vec<NuveiL23Item>>,
    pub amount_details: Option<NuveiL23AmountDetails>,
}

impl NuveiL23ProcessingData {
    fn is_empty(&self) -> bool {
        self.tax_indicator.is_none()
            && self.customer_code.is_none()
            && self.merchant_vat_reg_num.is_none()
            && self.customer_vat_reg_num.is_none()
            && self.destination_zip.is_none()
            && self.ship_from_zip.is_none()
            && self.destination_country_code.is_none()
            && self.order_date.is_none()
            && self.line_item_count.is_none()
            && self.items.is_none()
            && self.amount_details.is_none()
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiL23Item {
    pub commodity_code: Option<String>,
    pub description: Option<String>,
    pub product_code: Option<String>,
    pub quantity: Option<String>,
    pub unit_measure: Option<String>,
    pub price: Option<StringMajorUnit>,
    #[serde(rename = "vatOrTaxAmount")]
    pub vat_or_tax_amount: Option<StringMajorUnit>,
    #[serde(rename = "vatOrTaxRate")]
    pub vat_or_tax_rate: Option<String>,
    pub total_amount: Option<StringMajorUnit>,
    pub discount_rate: Option<String>,
    pub discount: Option<StringMajorUnit>,
    pub tax_type: Option<String>,
    /// `D` debit or `C` credit. Every UCS line item is a purchase.
    pub credit_indicator: Option<String>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiL23AmountDetails {
    pub total_discount: Option<StringMajorUnit>,
    pub total_shipping: Option<StringMajorUnit>,
    pub duty_amount: Option<StringMajorUnit>,
    #[serde(rename = "vatOrTaxAmount")]
    pub vat_or_tax_amount: Option<StringMajorUnit>,
    pub tax_amount: Option<StringMajorUnit>,
}

impl NuveiL23AmountDetails {
    fn is_empty(&self) -> bool {
        self.total_discount.is_none()
            && self.total_shipping.is_none()
            && self.duty_amount.is_none()
            && self.vat_or_tax_amount.is_none()
            && self.tax_amount.is_none()
    }
}

/// Nuvei field length caps for the Level 2/3 addendum (Level 2&3 reference).
const NUVEI_L23_CUSTOMER_CODE_MAX_LENGTH: usize = 25;
const NUVEI_L23_MERCHANT_VAT_MAX_LENGTH: usize = 20;
const NUVEI_L23_CUSTOMER_VAT_MAX_LENGTH: usize = 13;
const NUVEI_L23_DESCRIPTION_MAX_LENGTH: usize = 35;
const NUVEI_L23_PRODUCT_CODE_MAX_LENGTH: usize = 12;
const NUVEI_L23_COMMODITY_CODE_MAX_LENGTH: usize = 12;

fn truncate_to(value: String, max_length: usize) -> Option<String> {
    let value: String = value.chars().take(max_length).collect();
    (!value.is_empty()).then_some(value)
}

/// Build `addendums.l23processingData` from the domain `L2L3Data`.
///
/// Only fields the UCS domain request can actually supply are populated -
/// nothing is hardcoded or invented. Returns `None` when the caller supplied
/// no Level 2/3 data, so the object stays off the wire entirely.
///
/// NOTE: today this always returns `None` on the gRPC path, because
/// `grpc_api_types::payments::PaymentServiceCaptureRequest` has no
/// `l2_l3_data` field and `PaymentFlowData::foreign_try_from` for that request
/// hardcodes `l2_l3_data: None` / `order_details: None`. The mapping below
/// reads only real `L2L3Data` accessors, so it starts emitting the addendum
/// the moment the capture request carries the data - and until then the settle
/// request (and therefore its checksum) is byte-for-byte unchanged.
fn build_nuvei_addendums(
    router_data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    amount_converter: &(dyn common_utils::types::AmountConvertor<Output = StringMajorUnit> + Sync),
) -> Result<Option<NuveiAddendums>, Report<IntegrationError>> {
    let Some(l2_l3) = router_data.resource_common_data.l2_l3_data.as_deref() else {
        return Ok(None);
    };
    let currency = router_data.request.currency;
    let convert = |amount: common_utils::types::MinorUnit| {
        amount_converter.convert(amount, currency).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )
    };

    // `YYMMDD` per the Level 2&3 reference (note: NOT the `YYYYMMDDHHmmss`
    // form used by the request `timeStamp`).
    let order_date = l2_l3
        .get_order_date()
        .map(|date| {
            date.format(&time::macros::format_description!(
                "[year repr:last_two][month][day]"
            ))
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
        })
        .transpose()?;

    let order_tax_amount = l2_l3.get_order_tax_amount();
    // `0` tax not included, `1` state/provincial tax included, `2` not subject to tax.
    let tax_indicator = match l2_l3.get_tax_status() {
        Some(common_enums::TaxStatus::Exempt) => Some("2".to_string()),
        Some(common_enums::TaxStatus::Taxable) => Some(
            if order_tax_amount.is_some_and(|amount| amount.get_amount_as_i64() > 0) {
                "1"
            } else {
                "0"
            }
            .to_string(),
        ),
        None => None,
    };

    let order_details = l2_l3
        .get_order_details()
        .or_else(|| router_data.resource_common_data.order_details.clone())
        .unwrap_or_default();

    let items = order_details
        .iter()
        .map(|detail| {
            Ok(NuveiL23Item {
                commodity_code: detail
                    .commodity_code
                    .clone()
                    .and_then(|code| truncate_to(code, NUVEI_L23_COMMODITY_CODE_MAX_LENGTH)),
                description: truncate_to(
                    detail
                        .description
                        .clone()
                        .unwrap_or_else(|| detail.product_name.clone()),
                    NUVEI_L23_DESCRIPTION_MAX_LENGTH,
                ),
                product_code: detail
                    .product_id
                    .clone()
                    .or_else(|| detail.sku.clone())
                    .and_then(|code| truncate_to(code, NUVEI_L23_PRODUCT_CODE_MAX_LENGTH)),
                quantity: Some(detail.quantity.to_string()),
                unit_measure: detail.unit_of_measure.clone(),
                price: Some(convert(detail.amount)?),
                vat_or_tax_amount: detail.total_tax_amount.map(convert).transpose()?,
                vat_or_tax_rate: detail.tax_rate.map(|rate| rate.to_string()),
                total_amount: detail.total_amount.map(convert).transpose()?,
                discount_rate: detail.discount_percentage.map(|rate| rate.to_string()),
                discount: detail.unit_discount_amount.map(convert).transpose()?,
                tax_type: detail.product_tax_code.clone(),
                credit_indicator: Some("D".to_string()),
            })
        })
        .collect::<Result<Vec<_>, Report<IntegrationError>>>()?;

    let amount_details = NuveiL23AmountDetails {
        total_discount: l2_l3.get_discount_amount().map(convert).transpose()?,
        total_shipping: l2_l3.get_shipping_cost().map(convert).transpose()?,
        duty_amount: l2_l3.get_duty_amount().map(convert).transpose()?,
        vat_or_tax_amount: l2_l3.get_shipping_amount_tax().map(convert).transpose()?,
        tax_amount: order_tax_amount.map(convert).transpose()?,
    };

    let l23_processing_data = NuveiL23ProcessingData {
        tax_indicator,
        customer_code: l2_l3.get_customer_id().and_then(|id| {
            truncate_to(
                id.get_string_repr().to_string(),
                NUVEI_L23_CUSTOMER_CODE_MAX_LENGTH,
            )
        }),
        merchant_vat_reg_num: l2_l3.get_merchant_tax_registration_id().and_then(|id| {
            truncate_to(id.peek().to_string(), NUVEI_L23_MERCHANT_VAT_MAX_LENGTH).map(Secret::new)
        }),
        customer_vat_reg_num: l2_l3.get_customer_tax_registration_id().and_then(|id| {
            truncate_to(id.peek().to_string(), NUVEI_L23_CUSTOMER_VAT_MAX_LENGTH).map(Secret::new)
        }),
        destination_zip: l2_l3.get_shipping_zip(),
        ship_from_zip: l2_l3.get_shipping_origin_zip(),
        destination_country_code: l2_l3
            .get_shipping_country()
            .map(|country| country.to_string()),
        order_date,
        line_item_count: (!items.is_empty()).then(|| items.len().to_string()),
        items: (!items.is_empty()).then_some(items),
        amount_details: (!amount_details.is_empty()).then_some(amount_details),
    };

    Ok((!l23_processing_data.is_empty()).then_some(NuveiAddendums {
        l23_processing_data,
    }))
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
    /// Level 2 / Level 3 interchange-optimisation data. `/settleTransaction.do`
    /// is the ONLY endpoint that accepts it. Does not participate in the checksum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addendums: Option<NuveiAddendums>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Capture Response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiCaptureResponse {
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub transaction_id: Option<String>,
    pub status: NuveiPaymentStatus,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    #[serde(rename = "gwExtendedErrorCode")]
    pub gw_extended_error_code: Option<i64>,
    pub merchant_advice_code: Option<String>,
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
    pub auth_code: Option<String>,
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
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}

// Refund Sync Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_unique_id: String,
    pub transaction_id: String,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Refund Sync Response (separate type to avoid macro conflicts)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRefundSyncResponse {
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
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
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    #[serde(rename = "gwExtendedErrorCode")]
    pub gw_extended_error_code: Option<i64>,
    pub merchant_advice_code: Option<String>,
    pub issuer_decline_code: Option<String>,
    pub issuer_decline_reason: Option<String>,
}

// Error Response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiErrorResponse {
    pub reason: Option<String>,
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
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
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

        // Extract payment method data
        let payment_option = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .or(router_data.request.customer_name.clone().map(Secret::new))
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address.first_name and billing_address.last_name or customer_name",
                context: Default::default()
                    })?;

                // External MPI (merchant-supplied 3DS): when the caller already
                // ran its own MPI we skip /initPayment.do entirely and inline
                // the authentication values on this single /payment.do.
                let three_d = router_data
                    .request
                    .authentication_data
                    .as_ref()
                    .and_then(NuveiExternalMpi::from_authentication_data)
                    .map(|external_mpi| NuveiThreeD {
                        external_mpi: Some(external_mpi),
                    });

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

        let dynamic_descriptor = router_data
            .request
            .billing_descriptor
            .as_ref()
            .map(NuveiDynamicDescriptor::from_billing_descriptor)
            .transpose()?
            .flatten();

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
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let currency = router_data.request.currency;

        // Extract session token from PaymentFlowData
        // The ServerSessionAuthenticationToken flow runs before Authorize and populates this field
        let session_token = router_data
            .resource_common_data
            .session_token
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: Default::default(),
            })?;

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
            client_unique_id: Some(truncate_client_unique_id(
                &router_data
                    .resource_common_data
                    .connector_request_reference_id,
            )),
            payment_option,
            transaction_type,
            device_details,
            billing_address,
            shipping_address,
            dynamic_descriptor,
            url_details,
            time_stamp,
            checksum,
        })
    }
}

/// `clientUniqueId` is capped at 45 characters by Nuvei; a longer value is
/// rejected. The value is deliberately NOT the same as `clientRequestId`
/// semantically: it identifies the transaction (echoed on the DMN and usable
/// as a `/getTransactionDetails.do` lookup key), whereas `clientRequestId`
/// identifies one API call.
fn truncate_client_unique_id(reference_id: &str) -> String {
    reference_id
        .chars()
        .take(NUVEI_CLIENT_UNIQUE_ID_MAX_LENGTH)
        .collect()
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

        // AVS / CVV verdicts are surfaced as structured payment checks on
        // `connector_response`, never folded into the error message - they are
        // reported on approvals as well as declines.
        let connector_response = build_nuvei_connector_response(
            response.payment_option.as_ref(),
            response.auth_code.clone(),
        )
        .or_else(|| router_data.resource_common_data.connector_response.clone());

        // Three-stage error model. A DECLINED card still returns a top-level
        // `status: "SUCCESS"`, so the decline is detected from
        // `transactionStatus` and described by `gwErrorCode` / `gwErrorReason`.
        if let Some(error_response) = build_nuvei_error_response(
            NuveiErrorFields {
                status: response.status.clone(),
                transaction_status: response.transaction_status.clone(),
                err_code: response.err_code,
                reason: response.reason.clone(),
                gw_error_code: response.gw_error_code,
                gw_error_reason: response.gw_error_reason.clone(),
                gw_extended_error_code: response.gw_extended_error_code,
                merchant_advice_code: response.merchant_advice_code.clone(),
                issuer_decline_code: response.issuer_decline_code.clone(),
                issuer_decline_reason: response.issuer_decline_reason.clone(),
                payment_method_error_code: response.payment_method_error_code,
                payment_method_error_reason: response.payment_method_error_reason.clone(),
                transaction_id: response.transaction_id.clone(),
            },
            item.http_code,
            FlowStatus::Payment(common_enums::AttemptStatus::Failure),
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => {
                if router_data.request.is_auto_capture() {
                    common_enums::AttemptStatus::Charged
                } else {
                    common_enums::AttemptStatus::Authorized
                }
            }
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Pending
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

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

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data,
            mandate_reference: None,
            connector_metadata: None,
            // externalSchemeTransactionId is the Network Transaction ID (NTID),
            // needed to key later merchant-initiated transactions.
            network_txn_id: response
                .external_scheme_transaction_id
                .clone()
                .filter(|id| !id.is_empty()),
            network_txn_link_id: response
                .transaction_link_id
                .clone()
                .filter(|id| !id.is_empty()),
            connector_response_reference_id: response.client_request_id.clone(),
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
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => id.clone(),
            ResponseId::EncodedData(id) => id.clone(),
            ResponseId::NoResponseId => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
                .into());
            }
        };

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                router_data.request.minor_amount_to_capture,
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
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

        // Level 2/3 data rides on the settle, never on /payment.do, and is not
        // part of the checksum concatenation.
        let addendums =
            build_nuvei_addendums(router_data, item.connector.amount_converter_webhooks)?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id,
            client_unique_id,
            amount,
            currency,
            related_transaction_id,
            addendums,
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

        // AVS / CVV verdicts are echoed on getTransactionDetails too.
        let connector_response =
            build_nuvei_connector_response(response.payment_option.as_ref(), None)
                .or_else(|| router_data.resource_common_data.connector_response.clone());

        // Three-stage error model: stage-1 rejections carry errCode/reason,
        // stage-2 declines keep a top-level SUCCESS and describe themselves
        // through the gwError* fields inside transactionDetails.
        let details = response.transaction_details.as_ref();
        if let Some(error_response) = build_nuvei_error_response(
            NuveiErrorFields {
                status: response.status.clone(),
                transaction_status: details.and_then(|td| td.transaction_status.clone()),
                err_code: response.err_code,
                reason: response.reason.clone(),
                gw_error_code: details.and_then(|td| td.gw_error_code),
                gw_error_reason: details.and_then(|td| td.gw_error_reason.clone()),
                gw_extended_error_code: details.and_then(|td| td.gw_extended_error_code),
                merchant_advice_code: None,
                issuer_decline_code: None,
                issuer_decline_reason: None,
                payment_method_error_code: None,
                payment_method_error_reason: None,
                transaction_id: details.and_then(|td| td.transaction_id.clone()),
            },
            item.http_code,
            FlowStatus::Payment(common_enums::AttemptStatus::Failure),
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    connector_response,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Extract transaction details
        let transaction_details = response.transaction_details.as_ref().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_details missing in Nuvei PSync response".to_string()),
            ))
        })?;

        // Map transaction status to attempt status
        let status = match transaction_details.transaction_status {
            Some(NuveiTransactionStatus::Approved) => {
                // For PSync, we need to determine if it was authorized or captured
                // Check transaction_type: "Auth" means authorized only, "Sale" means captured
                match transaction_details.transaction_type.as_deref() {
                    Some("Auth") => common_enums::AttemptStatus::Authorized,
                    Some("Sale") | Some("Settle") => common_enums::AttemptStatus::Charged,
                    _ => common_enums::AttemptStatus::Charged, // Default to Charged for unknown types
                }
            }
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Pending
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

        // Get connector transaction ID from transaction_details
        let connector_transaction_id =
            transaction_details.transaction_id.clone().ok_or_else(|| {
                Report::new(ConnectorError::response_handling_failed_with_context(
                    item.http_code,
                    Some("transaction_id missing in Nuvei PSync transaction_details".to_string()),
                ))
            })?;

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: transaction_details.client_unique_id.clone(),
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

// Capture Response Transformation
impl TryFrom<ResponseRouterData<NuveiCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiCaptureResponse, Self>) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Three-stage error model: a DECLINED settle/void still returns a
        // top-level `status: "SUCCESS"`, so the decline is read from
        // `transactionStatus` and described by the gwError* pair.
        if let Some(error_response) = build_nuvei_error_response(
            NuveiErrorFields {
                status: response.status.clone(),
                transaction_status: response.transaction_status.clone(),
                err_code: response.err_code,
                reason: response.reason.clone(),
                gw_error_code: response.gw_error_code,
                gw_error_reason: response.gw_error_reason.clone(),
                gw_extended_error_code: response.gw_extended_error_code,
                merchant_advice_code: response.merchant_advice_code.clone(),
                issuer_decline_code: response.issuer_decline_code.clone(),
                issuer_decline_reason: response.issuer_decline_reason.clone(),
                payment_method_error_code: None,
                payment_method_error_reason: None,
                transaction_id: response.transaction_id.clone(),
            },
            item.http_code,
            FlowStatus::Payment(common_enums::AttemptStatus::Failure),
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(error_response),
                ..router_data.clone()
            });
        }

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Charged
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Charged
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
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
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(
                common_utils::types::MinorUnit::new(router_data.request.refund_amount),
                router_data.request.currency,
            )
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
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

        // Per Hyperswitch pattern: ALWAYS send both transaction_id AND client_unique_id
        // NOTE: For RSync to work correctly, we need the ORIGINAL clientUniqueId from refund creation
        // Using current connector_request_reference_id may not match the original
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );
        let transaction_id = router_data.request.connector_transaction_id.clone();

        if transaction_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }
            .into());
        }

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

// Refund Response Transformation
impl TryFrom<ResponseRouterData<NuveiRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<NuveiRefundResponse, Self>) -> Result<Self, Self::Error> {
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
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
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

        // Map transaction status to refund status
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::RefundStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Success
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::RefundStatus::Success
                } else {
                    common_enums::RefundStatus::Failure
                }
            }
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

        // Check if the overall request status is SUCCESS or ERROR
        if matches!(response.status, NuveiPaymentStatus::Error) {
            let error_code = response.err_code.map(|c| c.to_string()).unwrap_or_default();
            let error_message = response
                .reason
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());

            return Ok(Self {
                resource_common_data: RefundFlowData {
                    status: common_enums::RefundStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: response.transaction_id.clone(),
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

        // Map transaction status to refund status
        let refund_status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::RefundStatus::Success,
            Some(NuveiTransactionStatus::Declined) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Error) => common_enums::RefundStatus::Failure,
            Some(NuveiTransactionStatus::Pending) => common_enums::RefundStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Success
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::RefundStatus::Success
                } else {
                    common_enums::RefundStatus::Failure
                }
            }
        };

        // Get connector refund ID
        let connector_refund_id = response.transaction_id.clone().ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in Nuvei refund sync response".to_string()),
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
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        // Extract relatedTransactionId from connector_transaction_id
        let related_transaction_id = router_data.request.connector_transaction_id.clone();

        // Extract amount and currency from the request
        // For void, we need to send the original transaction amount and currency
        let minor_amount =
            router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;

        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
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

        // Three-stage error model. A failed void is non-terminal for the
        // payment itself, so the attempt status stays `VoidFailed`.
        if let Some(error_response) = build_nuvei_error_response(
            NuveiErrorFields {
                status: response.status.clone(),
                transaction_status: response.transaction_status.clone(),
                err_code: response.err_code,
                reason: response.reason.clone(),
                gw_error_code: response.gw_error_code,
                gw_error_reason: response.gw_error_reason.clone(),
                gw_extended_error_code: response.gw_extended_error_code,
                merchant_advice_code: response.merchant_advice_code.clone(),
                issuer_decline_code: response.issuer_decline_code.clone(),
                issuer_decline_reason: response.issuer_decline_reason.clone(),
                payment_method_error_code: None,
                payment_method_error_reason: None,
                transaction_id: response.transaction_id.clone(),
            },
            item.http_code,
            FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed),
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

        // Map transaction status to attempt status
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Voided,
            Some(NuveiTransactionStatus::Declined) => common_enums::AttemptStatus::VoidFailed,
            Some(NuveiTransactionStatus::Error) => common_enums::AttemptStatus::VoidFailed,
            Some(NuveiTransactionStatus::Pending) => common_enums::AttemptStatus::Pending,
            _ => {
                // If transaction_status is not present but status is SUCCESS, default to Voided
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Voided
                } else {
                    common_enums::AttemptStatus::VoidFailed
                }
            }
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
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
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
    pub merchant_id: Option<String>,
    pub merchant_site_id: Option<String>,
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
        let client_unique_id = truncate_client_unique_id(
            &router_data
                .resource_common_data
                .connector_request_reference_id,
        );

        // Convert amount using the connector's amount converter
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(router_data.request.amount, router_data.request.currency)
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
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    pub billing_address: NuveiBillingAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_details: Option<NuveiUrlDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

/// SetupMandate response - reuses NuveiPaymentResponse fields plus paymentOption
/// (which carries the userPaymentOptionId returned by Nuvei for future MIT calls).
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSetupMandateResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub auth_code: Option<String>,
    pub session_token: Option<String>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
}

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

        // Nuvei SetupMandate supports Card payment_method_data.
        let payment_option = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let card_holder_name = router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_address.first_name and billing_address.last_name",
                        context: Default::default(),
                    })?;

                NuveiPaymentOption {
                    card: Some(NuveiCardPaymentOption::Raw(NuveiCard {
                        card_number: card_data.card_number.clone(),
                        card_holder_name,
                        expiration_month: card_data.card_exp_month.clone(),
                        expiration_year: card_data.card_exp_year.clone(),
                        cvv: card_data.card_cvc.clone(),
                        // Zero-amount verification never carries merchant-supplied 3DS.
                        three_d: None,
                    })),
                    alternative_payment_method: None,
                    user_payment_option_id: None,
                }
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method not supported for SetupMandate".to_string(),
                    connector: "nuvei",
                    context: Default::default(),
                }
                .into())
            }
        };

        // Billing address - Nuvei requires email and country.
        let billing_address = get_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        )
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "billing_address (email and country required)",
            context: Default::default(),
        })?;

        // Device details - ipAddress required by Nuvei.
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

        // For SetupMandate amount is optional; default to 0 if absent so that
        // Nuvei treats this as a zero-value auth verification for the mandate.
        let minor_amount = router_data
            .request
            .minor_amount
            .unwrap_or(common_utils::types::MinorUnit::new(0));
        let currency = router_data.request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Session token populated by ServerSessionAuthenticationToken flow.
        let session_token = router_data
            .resource_common_data
            .session_token
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: Default::default(),
            })?;

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
                field_name: "customer_id (maps to Nuvei userTokenId)",
                context: Default::default(),
            })?;

        Ok(Self {
            session_token: Some(session_token),
            merchant_id: auth.merchant_id,
            merchant_site_id: auth.merchant_site_id,
            client_request_id: client_request_id.clone(),
            amount,
            currency,
            client_unique_id: Some(client_request_id),
            user_token_id: Some(user_token_id),
            payment_option,
            is_rebilling: "0".to_string(),
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

        // Hard failure at the API layer.
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
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: response.transaction_id.clone(),
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

        // Transaction-level status - for SetupMandate an Approved Auth is the
        // success path (status Charged indicates the mandate was registered
        // successfully from the caller's perspective).
        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::AttemptStatus::Failure
            }
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | None => {
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

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

        // Surface userPaymentOptionId as the connector mandate id for future MIT.
        // A successful SetupMandate response without a userPaymentOptionId is
        // unusable downstream (RepeatPayment needs it), so treat it as a failure.
        let mandate_reference = response
            .payment_option
            .as_ref()
            .and_then(|po| po.user_payment_option_id.clone())
            .map(|id| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(id),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            });

        if mandate_reference.is_none() {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
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

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.client_request_id.clone(),
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

// ===== RepeatPayment (MIT) flow =====
//
// Merchant-initiated recurring charges reuse the same /ppp/api/v1/payment.do
// endpoint as Authorize/SetupMandate with isRebilling="1" and the stored
// userPaymentOptionId from the initial SetupMandate response. Card data is not
// sent - Nuvei re-uses the payment option linked to the userTokenId.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentRequest {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub amount: StringMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_unique_id: Option<String>,
    /// userTokenId must match the value used on the initial SetupMandate so
    /// Nuvei resolves the stored payment option correctly. Not sent for
    /// network-token (NTID) MITs, which carry the token itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_token_id: Option<String>,
    pub payment_option: NuveiRepeatPaymentOptionTypes,
    /// "1" marks a merchant-initiated rebilling transaction (stored-credential
    /// MIT only; the NTID itself marks a network-token MIT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rebilling: Option<String>,
    pub transaction_type: TransactionType,
    pub device_details: NuveiDeviceDetails,
    /// Optional on MIT since the stored userPaymentOptionId already carries
    /// the billing info captured at CIT. Forwarded when the caller supplies
    /// it so Nuvei can run AVS / risk checks if the address has changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<NuveiBillingAddress>,
    /// Original-transaction reference (NTID + brand) for network-token MITs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_scheme_details: Option<NuveiExternalSchemeDetails>,
    pub time_stamp: common_utils::date_time::DateTime<common_utils::date_time::YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Serialize-only untagged enum: stored-credential MIT keeps its exact previous
// wire shape; network-token MIT emits paymentOption.card with externalToken
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum NuveiRepeatPaymentOptionTypes {
    StoredCredential(NuveiRepeatPaymentOption),
    NetworkToken(NuveiRepeatPaymentCardOption),
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiRepeatPaymentResponse {
    pub order_id: Option<String>,
    pub transaction_id: Option<String>,
    pub transaction_status: Option<NuveiTransactionStatus>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
    #[serde(rename = "gwErrorCode")]
    pub gw_error_code: Option<i32>,
    #[serde(rename = "gwErrorReason")]
    pub gw_error_reason: Option<String>,
    pub auth_code: Option<String>,
    pub session_token: Option<String>,
    pub client_unique_id: Option<String>,
    pub client_request_id: Option<String>,
    pub internal_request_id: Option<i64>,
    pub payment_option: Option<NuveiResponsePaymentOption>,
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

        let auth = NuveiAuthType::try_from(&router_data.connector_config)?;

        // Stored-credential MIT uses Nuvei's own ConnectorMandateId (the
        // userPaymentOptionId returned by SetupMandate); network-token MIT
        // uses the token from payment_method plus the NTID from the mandate
        // reference (mirrors hyperswitch PR #13093).
        let (payment_option, external_scheme_details, user_token_id, is_rebilling) =
            match &router_data.request.mandate_reference {
                MandateReferenceId::ConnectorMandateId(c) => {
                    let user_payment_option_id = c.get_connector_mandate_id().ok_or(
                        IntegrationError::MissingRequiredField {
                            field_name: "mandate_reference.connector_mandate_id",
                            context: Default::default(),
                        },
                    )?;

                    // userTokenId must match the initial SetupMandate - caller
                    // passes the same value via connector_customer_id on the
                    // Charge request.
                    let user_token_id = router_data
                        .resource_common_data
                        .connector_customer
                        .clone()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "connector_customer_id (maps to Nuvei userTokenId)",
                            context: Default::default(),
                        })?;

                    (
                        NuveiRepeatPaymentOptionTypes::StoredCredential(NuveiRepeatPaymentOption {
                            user_payment_option_id,
                        }),
                        None,
                        Some(user_token_id),
                        Some("1".to_string()),
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
                                context: Default::default(),
                            }
                            .into())
                        }
                    };

                    (
                        NuveiRepeatPaymentOptionTypes::NetworkToken(NuveiRepeatPaymentCardOption {
                            card: build_nuvei_network_token_card(token_data),
                        }),
                        Some(NuveiExternalSchemeDetails {
                            transaction_id: Secret::new(nti_ref.network_transaction_id.clone()),
                            brand: Some(get_nuvei_card_brand(token_data)?),
                        }),
                        None,
                        None,
                    )
                }
                MandateReferenceId::NetworkMandateId(_) => {
                    return Err(IntegrationError::NotSupported {
                        message:
                            "Nuvei RepeatPayment supports connector_mandate_id or a network token with NTID"
                                .to_string(),
                        connector: "nuvei",
                        context: Default::default(),
                    }
                    .into())
                }
            };

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

        // Billing address is optional for MIT: Nuvei resolves AVS from the
        // stored userPaymentOptionId by default. Forward a block only when
        // the caller supplies enough data for a valid Nuvei payload.
        let billing_address = get_billing_address(
            &router_data.resource_common_data,
            router_data.request.email.clone(),
        );

        let time_stamp = NuveiAuthType::get_timestamp();
        let client_request_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let minor_amount = router_data.request.minor_amount;
        let currency = router_data.request.currency;
        let amount = item
            .connector
            .amount_converter_webhooks
            .convert(minor_amount, currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;

        // Nuvei's short-lived session token is passed via state.access_token
        // on the Charge request.
        let session_token = router_data
            .resource_common_data
            .access_token
            .as_ref()
            .map(|at| at.access_token.peek().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "state.access_token",
                context: Default::default(),
            })?;

        // Default to Sale so funds capture in one step for MIT; fall back to
        // Auth only if the caller explicitly asks for manual capture.
        let transaction_type = match router_data.request.capture_method {
            Some(common_enums::CaptureMethod::Manual) => TransactionType::Auth,
            _ => TransactionType::Sale,
        };

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
            client_request_id: client_request_id.clone(),
            amount,
            currency,
            client_unique_id: Some(client_request_id),
            user_token_id,
            payment_option,
            is_rebilling,
            transaction_type,
            device_details,
            billing_address,
            external_scheme_details,
            time_stamp,
            checksum,
        })
    }
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
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: error_code,
                    message: error_message.clone(),
                    reason: Some(error_message),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: response.transaction_id.clone(),
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

        let status = match response.transaction_status {
            Some(NuveiTransactionStatus::Approved) => common_enums::AttemptStatus::Charged,
            Some(NuveiTransactionStatus::Declined) | Some(NuveiTransactionStatus::Error) => {
                common_enums::AttemptStatus::Failure
            }
            Some(NuveiTransactionStatus::Redirect) => {
                common_enums::AttemptStatus::AuthenticationPending
            }
            Some(NuveiTransactionStatus::Pending)
            | Some(NuveiTransactionStatus::Processing)
            | None => {
                if matches!(response.status, NuveiPaymentStatus::Success) {
                    common_enums::AttemptStatus::Pending
                } else {
                    common_enums::AttemptStatus::Failure
                }
            }
        };

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

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(connector_transaction_id),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.client_request_id.clone(),
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
