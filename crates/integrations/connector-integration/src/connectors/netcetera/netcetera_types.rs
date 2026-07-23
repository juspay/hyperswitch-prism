// Netcetera EMVCo 3DS types.
//
// Ported (pragmatically) from the router's
// `crates/hyperswitch_connectors/src/connectors/netcetera/netcetera_types.rs`.
//
// The key UCS-specific change vs. the router: the PAN
// (`CardholderAccount::acct_number` / `NetceteraPreAuthenticationRequest::
// cardholder_account_number`) uses `RawCardNumber<T>` (no Luhn enforcement) so
// VGS card aliases pass through. Everywhere the router used
// `cards::CardNumber`, UCS uses `RawCardNumber<T>`.
//
// Only the subset of EMVCo types needed by the three implemented auth flows
// (PreAuthenticate / Authenticate / PostAuthenticate) is ported here. Large
// optional EMVCo sub-objects (full MerchantRiskIndicator, SDK/app-channel data,
// seller info, prior-auth info, etc.) are deliberately omitted or simplified;
// see the `// TODO(emvco)` markers.

use std::fmt::Debug;

use common_utils::types::SemanticVersion;
use domain_types::{
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodDataTypes, RawCardNumber},
};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SingleOrListElement (EMVCo 2.3.1 changed several scalar fields to arrays)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum SingleOrListElement<T> {
    Single(T),
    List(Vec<T>),
}

impl<T> SingleOrListElement<T> {
    /// For EMVCo >= 2.3.0 these fields became arrays; below that they are scalars.
    pub fn get_version_checked(message_version: &SemanticVersion, value: T) -> Self {
        if message_version.get_major() >= 2 && message_version.get_minor() >= 3 {
            Self::List(vec![value])
        } else {
            Self::Single(value)
        }
    }
}

// ---------------------------------------------------------------------------
// Device channel / message category / 3DS method completion
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum NetceteraDeviceChannel {
    #[serde(rename = "01")]
    AppBased,
    #[serde(rename = "02")]
    Browser,
    #[serde(rename = "03")]
    ThreeDsRequestorInitiated,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum NetceteraMessageCategory {
    #[serde(rename = "01")]
    PaymentAuthentication,
    #[serde(rename = "02")]
    NonPaymentAuthentication,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ThreeDSMethodCompletionIndicator {
    /// Successfully completed
    Y,
    /// Did not successfully complete
    N,
    /// Unavailable - 3DS Method URL was not present in the PRes message data
    U,
}

// ---------------------------------------------------------------------------
// Scheme id
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, strum::Display)]
pub enum SchemeId {
    Visa,
    Mastercard,
    #[serde(rename = "JCB")]
    Jcb,
    #[serde(rename = "American Express")]
    AmericanExpress,
    Diners,
    // For Cartes Bancaires and UnionPay, it is recommended to send the scheme ID
    #[serde(rename = "CB")]
    CartesBancaires,
    UnionPay,
}

impl TryFrom<common_enums::CardNetwork> for SchemeId {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(network: common_enums::CardNetwork) -> Result<Self, Self::Error> {
        match network {
            common_enums::CardNetwork::Visa => Ok(Self::Visa),
            common_enums::CardNetwork::Mastercard => Ok(Self::Mastercard),
            common_enums::CardNetwork::JCB => Ok(Self::Jcb),
            common_enums::CardNetwork::AmericanExpress => Ok(Self::AmericanExpress),
            common_enums::CardNetwork::DinersClub => Ok(Self::Diners),
            common_enums::CardNetwork::CartesBancaires => Ok(Self::CartesBancaires),
            common_enums::CardNetwork::UnionPay => Ok(Self::UnionPay),
            _ => Err(IntegrationError::NotSupported {
                message: "Invalid card network".to_string(),
                connector: "netcetera",
                context: Default::default(),
            })?,
        }
    }
}

// ---------------------------------------------------------------------------
// ThreeDSRequestor
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSRequestor {
    #[serde(rename = "threeDSRequestorAuthenticationInd")]
    pub three_ds_requestor_authentication_ind: ThreeDSRequestorAuthenticationIndicator,
    #[serde(rename = "threeDSRequestorChallengeInd")]
    pub three_ds_requestor_challenge_ind:
        Option<SingleOrListElement<ThreeDSRequestorChallengeIndicator>>,
    /// External IP address used by the 3DS Requestor App (deviceChannel = 01 / APP).
    pub app_ip: Option<std::net::IpAddr>,
}

impl ThreeDSRequestor {
    // TODO(emvco): the router additionally derived the challenge indicator from
    // `psd2_sca_exemption_type` (TransactionRiskAnalysis -> No challenge / TRA).
    // UCS `PaymentsAuthenticateData` does not currently expose an SCA exemption
    // type, so that branch is not modeled here.
    pub fn new(
        app_ip: Option<std::net::IpAddr>,
        force_3ds_challenge: bool,
        message_version: &SemanticVersion,
    ) -> Self {
        let three_ds_requestor_challenge_ind = if force_3ds_challenge {
            Some(SingleOrListElement::get_version_checked(
                message_version,
                ThreeDSRequestorChallengeIndicator::ChallengeRequestedMandate,
            ))
        } else {
            None
        };

        Self {
            three_ds_requestor_authentication_ind: ThreeDSRequestorAuthenticationIndicator::Payment,
            three_ds_requestor_challenge_ind,
            app_ip,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ThreeDSRequestorAuthenticationIndicator {
    #[serde(rename = "01")]
    Payment,
    #[serde(rename = "02")]
    Recurring,
    #[serde(rename = "03")]
    Installment,
    #[serde(rename = "04")]
    AddCard,
    #[serde(rename = "05")]
    MaintainCard,
    #[serde(rename = "06")]
    CardholderVerification,
    #[serde(rename = "07")]
    BillingAgreement,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ThreeDSRequestorChallengeIndicator {
    #[serde(rename = "01")]
    NoPreference,
    #[serde(rename = "02")]
    NoChallengeRequested,
    #[serde(rename = "03")]
    ChallengeRequested3DSRequestorPreference,
    #[serde(rename = "04")]
    ChallengeRequestedMandate,
    #[serde(rename = "05")]
    NoChallengeRequestedTransactionalRiskAnalysis,
    #[serde(rename = "06")]
    NoChallengeRequestedDataShareOnly,
    #[serde(rename = "07")]
    NoChallengeRequestedStrongConsumerAuthentication,
    #[serde(rename = "08")]
    NoChallengeRequestedWhitelistExemption,
    #[serde(rename = "09")]
    ChallengeRequestedWhitelistPrompt,
}

// ---------------------------------------------------------------------------
// CardholderAccount (PAN carrier -> RawCardNumber<T>)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardholderAccount<T: PaymentMethodDataTypes> {
    /// Expiry date of the PAN supplied to the 3DS Requestor. Format `YYMM`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_expiry_date: Option<Secret<String>>,
    /// Account number used in the authorization request. UCS uses `RawCardNumber<T>`
    /// (no Luhn) so VGS aliases pass through.
    pub acct_number: RawCardNumber<T>,
    /// Scheme id of the account, recommended when the card is cobadged.
    pub scheme_id: Option<SchemeId>,
    /// Three or four-digit security code printed on the card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_security_code: Option<Secret<String>>,
}

// ---------------------------------------------------------------------------
// Cardholder (billing / shipping address)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct Cardholder {
    pub bill_addr_city: Option<Secret<String>>,
    pub bill_addr_country: Option<String>,
    pub bill_addr_line1: Option<Secret<String>>,
    pub bill_addr_line2: Option<Secret<String>>,
    pub bill_addr_line3: Option<Secret<String>>,
    pub bill_addr_post_code: Option<Secret<String>>,
    pub bill_addr_state: Option<Secret<String>>,
    pub email: Option<common_utils::Email>,
    pub cardholder_name: Option<Secret<String>>,
    pub ship_addr_city: Option<Secret<String>>,
    pub ship_addr_country: Option<String>,
    pub ship_addr_line1: Option<Secret<String>>,
    pub ship_addr_line2: Option<Secret<String>>,
    pub ship_addr_line3: Option<Secret<String>>,
    pub ship_addr_post_code: Option<Secret<String>>,
    pub ship_addr_state: Option<Secret<String>>,
}

impl
    TryFrom<(
        Option<domain_types::payment_address::Address>,
        Option<domain_types::payment_address::Address>,
    )> for Cardholder
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        (billing_address, shipping_address): (
            Option<domain_types::payment_address::Address>,
            Option<domain_types::payment_address::Address>,
        ),
    ) -> Result<Self, Self::Error> {
        let billing = billing_address.as_ref().and_then(|a| a.address.as_ref());
        let shipping = shipping_address.as_ref().and_then(|a| a.address.as_ref());
        // TODO(emvco): EMVCo billAddrCountry/shipAddrCountry must be the ISO 3166-1
        // numeric country code. UCS does not expose alpha2 -> numeric conversion in
        // domain_types, so country is omitted for now (the router sent it).
        Ok(Self {
            bill_addr_city: billing.and_then(|add| add.city.clone()),
            bill_addr_country: None,
            bill_addr_line1: billing.and_then(|add| add.line1.clone()),
            bill_addr_line2: billing.and_then(|add| add.line2.clone()),
            bill_addr_line3: billing.and_then(|add| add.line3.clone()),
            bill_addr_post_code: billing.and_then(|add| add.zip.clone()),
            // EMVCo billAddrState must be an ISO 3166-2 subdivision code (max 3 chars). Drop full
            // state names (e.g. "California") so Netcetera doesn't reject the AReq with
            // "billAddrState: string is too long. Expected maximum 3 characters".
            bill_addr_state: billing
                .and_then(|add| add.state.clone())
                .filter(|s| s.peek().chars().count() <= 3),
            email: billing_address.as_ref().and_then(|a| a.email.clone()),
            cardholder_name: billing.and_then(get_optional_full_name),
            ship_addr_city: shipping.and_then(|add| add.city.clone()),
            ship_addr_country: None,
            ship_addr_line1: shipping.and_then(|add| add.line1.clone()),
            ship_addr_line2: shipping.and_then(|add| add.line2.clone()),
            ship_addr_line3: shipping.and_then(|add| add.line3.clone()),
            ship_addr_post_code: shipping.and_then(|add| add.zip.clone()),
            ship_addr_state: shipping
                .and_then(|add| add.state.clone())
                .filter(|s| s.peek().chars().count() <= 3),
        })
    }
}

fn get_optional_full_name(
    address: &domain_types::payment_address::AddressDetails,
) -> Option<Secret<String>> {
    use hyperswitch_masking::PeekInterface;
    match (address.first_name.as_ref(), address.last_name.as_ref()) {
        (Some(first), Some(last)) => Some(Secret::new(format!("{} {}", first.peek(), last.peek()))),
        (Some(name), None) | (None, Some(name)) => Some(name.clone()),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// Purchase
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct Purchase {
    /// Purchase amount in minor units of currency.
    pub purchase_amount: Option<common_utils::types::MinorUnit>,
    /// ISO 4217 three-digit currency code.
    pub purchase_currency: String,
    /// ISO 4217 currency exponent (number of minor-unit digits).
    pub purchase_exponent: u8,
    /// Date and time of the purchase converted to UTC, format `YYYYMMDDHHMMSS`.
    pub purchase_date: Option<String>,
    /// Type of transaction being authenticated. `01` -> Goods / Service purchase.
    pub trans_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Acquirer / Merchant data
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct AcquirerData {
    pub acquirer_bin: Option<String>,
    pub acquirer_merchant_id: Option<String>,
    pub acquirer_country_code: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct MerchantData {
    pub merchant_configuration_id: Option<String>,
    pub mcc: Option<String>,
    pub merchant_country_code: Option<String>,
    pub merchant_name: Option<String>,
    #[serde(rename = "notificationURL")]
    pub notification_url: Option<url::Url>,
    #[serde(rename = "threeDSRequestorId")]
    pub three_ds_requestor_id: Option<String>,
    #[serde(rename = "threeDSRequestorName")]
    pub three_ds_requestor_name: Option<String>,
    /// Dynamic RRes (Results Response) notification URL — where Netcetera pushes the final challenge
    /// result when the pull mechanism is disabled. Serializes as `resultsResponseNotificationUrl`.
    pub results_response_notification_url: Option<url::Url>,
}

// ---------------------------------------------------------------------------
// Per-merchant Netcetera config (NON-auth)
// ---------------------------------------------------------------------------
//
// The acquirer / merchant 3DS fields below are NOT secrets and NOT auth
// material; they originate from the merchant's Netcetera account configuration
// (acquirer BIN, acquirer merchant id, MCC, 3DS requestor id/name, ...). They
// ride the request on `PaymentFlowData.connector_feature_data` (a
// `SecretSerdeValue`) and are deserialized into this struct by the Authenticate
// transformer via `utils::to_connector_meta_from_secret`. This mirrors how
// other UCS connectors read per-merchant non-auth config (e.g. axisbank /
// nexinets reading `connector_feature_data`).
//
// ROUTER-SIDE CONTRACT (the router must populate `connector_feature_data` with
// a JSON object of this exact shape for a real AReq; all fields optional):
// ```json
// {
//   "acquirer_bin":           "<acquirer BIN>",
//   "acquirer_merchant_id":   "<acquirer-assigned merchant id>",
//   "acquirer_country_code":  "<ISO 3166-1 numeric, e.g. \"840\">",
//   "merchant_configuration_id": "<Netcetera merchant configuration id>",
//   "mcc":                    "<merchant category code>",
//   "merchant_country_code":  "<ISO 3166-1 numeric>",
//   "merchant_name":          "<merchant name>",
//   "three_ds_requestor_id":  "<3DS requestor id>",
//   "three_ds_requestor_name":"<3DS requestor name>"
// }
// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct NetceteraMeta {
    pub acquirer_bin: Option<String>,
    pub acquirer_merchant_id: Option<String>,
    pub acquirer_country_code: Option<String>,
    pub merchant_configuration_id: Option<String>,
    pub mcc: Option<String>,
    pub merchant_country_code: Option<String>,
    pub merchant_name: Option<String>,
    pub three_ds_requestor_id: Option<String>,
    pub three_ds_requestor_name: Option<String>,
    /// Per-merchant subdomain that replaces the `{{merchant_endpoint_prefix}}` template segment
    /// in the configured Netcetera 3DS Server host (e.g. `flowbird` ->
    /// `https://flowbird.3ds-server.<env>.netcetera-cloud-payment.ch`). Sourced from the
    /// merchant's Netcetera MCA metadata (`endpoint_prefix`) and forwarded by the router on
    /// `PaymentFlowData.connector_feature_data`.
    pub endpoint_prefix: Option<String>,
    pub pull_mechanism_for_external_3ds_enabled: Option<bool>,
    /// Dynamic Results Response (RRes) notification URL. When present it is forwarded in the AReq
    /// merchant object as `resultsResponseNotificationUrl`, so Netcetera pushes the final challenge
    /// result to this webhook (used when the merchant has the pull mechanism disabled). Sourced from
    /// the netcetera MCA metadata; requires the Netcetera license to allow dynamic RRes URLs.
    pub results_response_notification_url: Option<url::Url>,
    /// Browser CRes return URL (AReq `notificationURL` / `threeDSRequestorURL`) — where the ACS
    /// returns the browser after the challenge. Sourced from the netcetera MCA metadata; when
    /// absent the caller's request return_url is used instead.
    pub notification_url: Option<url::Url>,
    /// Merchant-level 3DS challenge preference (EMVCo threeDSRequestorChallengeInd = 04 when true).
    /// Sourced from the netcetera MCA metadata.
    pub force_3ds_challenge: Option<bool>,
}

impl NetceteraMeta {
    /// Build the EMVCo `acquirer` object from the per-merchant config.
    pub fn to_acquirer_data(&self) -> AcquirerData {
        AcquirerData {
            acquirer_bin: self.acquirer_bin.clone(),
            acquirer_merchant_id: self.acquirer_merchant_id.clone(),
            acquirer_country_code: self.acquirer_country_code.clone(),
        }
    }

    /// Build the EMVCo `merchant` object from the per-merchant config.
    /// `notification_url` is sourced from the request (return/webhook URL), not
    /// from the merchant config, so it is threaded in by the caller.
    pub fn to_merchant_data(&self, notification_url: Option<url::Url>) -> MerchantData {
        MerchantData {
            merchant_configuration_id: self.merchant_configuration_id.clone(),
            mcc: self.mcc.clone(),
            merchant_country_code: self.merchant_country_code.clone(),
            merchant_name: self.merchant_name.clone(),
            // Browser CRes return URL: prefer the netcetera MCA metadata value, else the
            // caller's request return_url.
            notification_url: self.notification_url.clone().or(notification_url),
            three_ds_requestor_id: self.three_ds_requestor_id.clone(),
            three_ds_requestor_name: self.three_ds_requestor_name.clone(),
            // Server-side RRes push target (pull mechanism disabled), sourced from the
            // merchant's Netcetera MCA metadata (`results_response_notification_url`).
            results_response_notification_url: self.results_response_notification_url.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Browser information (deviceChannel = 02 / BRW)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct Browser {
    browser_accept_header: Option<String>,
    #[serde(rename = "browserIP")]
    browser_ip: Option<Secret<String, common_utils::pii::IpAddress>>,
    browser_java_enabled: Option<bool>,
    browser_language: Option<String>,
    browser_color_depth: Option<String>,
    browser_screen_height: Option<u32>,
    browser_screen_width: Option<u32>,
    #[serde(rename = "browserTZ")]
    browser_tz: Option<i32>,
    browser_user_agent: Option<String>,
    challenge_window_size: Option<ChallengeWindowSizeEnum>,
    browser_javascript_enabled: Option<bool>,
    accept_language: Option<Vec<String>>,
}

impl From<domain_types::router_request_types::BrowserInformation> for Browser {
    fn from(value: domain_types::router_request_types::BrowserInformation) -> Self {
        Self {
            browser_accept_header: value.accept_header,
            browser_ip: value.ip_address.map(|ip| Secret::new(ip.to_string())),
            browser_java_enabled: value.java_enabled,
            browser_language: value.language,
            browser_color_depth: value.color_depth.map(|cd| cd.to_string()),
            browser_screen_height: value.screen_height,
            browser_screen_width: value.screen_width,
            browser_tz: value.time_zone,
            browser_user_agent: value.user_agent,
            challenge_window_size: Some(ChallengeWindowSizeEnum::FullScreen),
            browser_javascript_enabled: value.java_script_enabled,
            accept_language: value
                .accept_language
                .map(get_list_of_accept_languages)
                .or(Some(vec!["en".to_string()])),
        }
    }
}

/// Split an `Accept-Language` header into its ordered list of language tags.
pub fn get_list_of_accept_languages(accept_language: String) -> Vec<String> {
    accept_language
        .split(',')
        .map(|lang| lang.split(';').next().unwrap_or(lang).trim().to_string())
        .collect()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChallengeWindowSizeEnum {
    #[serde(rename = "01")]
    Size250x400,
    #[serde(rename = "02")]
    Size390x400,
    #[serde(rename = "03")]
    Size500x600,
    #[serde(rename = "04")]
    Size600x400,
    #[serde(rename = "05")]
    FullScreen,
}

// ---------------------------------------------------------------------------
// App-channel (device_channel = 01 / APP) SDK data
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct Sdk {
    #[serde(rename = "sdkAppID")]
    sdk_app_id: Option<String>,
    sdk_enc_data: Option<String>,
    sdk_ephem_pub_key: Option<std::collections::HashMap<String, String>>,
    sdk_max_timeout: Option<u8>,
    sdk_reference_number: Option<String>,
    #[serde(rename = "sdkTransID")]
    sdk_trans_id: Option<String>,
    sdk_server_signed_content: Option<String>,
    sdk_type: Option<SdkType>,
    default_sdk_type: Option<DefaultSdkType>,
    split_sdk_type: Option<SplitSdkType>,
}

impl From<domain_types::connector_types::SdkInformation> for Sdk {
    fn from(sdk_info: domain_types::connector_types::SdkInformation) -> Self {
        Self {
            sdk_app_id: Some(sdk_info.sdk_app_id),
            sdk_enc_data: Some(sdk_info.sdk_enc_data),
            sdk_ephem_pub_key: Some(sdk_info.sdk_ephem_pub_key),
            sdk_max_timeout: Some(sdk_info.sdk_max_timeout),
            sdk_reference_number: Some(sdk_info.sdk_reference_number),
            sdk_trans_id: Some(sdk_info.sdk_trans_id),
            sdk_server_signed_content: None,
            sdk_type: sdk_info
                .sdk_type
                .map(SdkType::from)
                .or(Some(SdkType::DefaultSdk)),
            default_sdk_type: Some(DefaultSdkType {
                sdk_variant: "01".to_string(),
                wrapped_ind: None,
            }),
            split_sdk_type: None,
        }
    }
}

impl From<domain_types::connector_types::SdkType> for SdkType {
    fn from(sdk_type: domain_types::connector_types::SdkType) -> Self {
        match sdk_type {
            domain_types::connector_types::SdkType::DefaultSdk => Self::DefaultSdk,
            domain_types::connector_types::SdkType::SplitSdk => Self::SplitSdk,
            domain_types::connector_types::SdkType::LimitedSdk => Self::LimitedSdk,
            domain_types::connector_types::SdkType::BrowserSdk => Self::BrowserSdk,
            domain_types::connector_types::SdkType::ShellSdk => Self::ShellSdk,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SdkType {
    #[serde(rename = "01")]
    DefaultSdk,
    #[serde(rename = "02")]
    SplitSdk,
    #[serde(rename = "03")]
    LimitedSdk,
    #[serde(rename = "04")]
    BrowserSdk,
    #[serde(rename = "05")]
    ShellSdk,
    #[serde(untagged)]
    PsSpecific(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSdkType {
    sdk_variant: String,
    wrapped_ind: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SplitSdkType {
    sdk_variant: String,
    limited_ind: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRenderingOptionsSupported {
    pub sdk_interface: SdkInterface,
    pub sdk_ui_type: Vec<SdkUiType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SdkInterface {
    #[serde(rename = "01")]
    Native,
    #[serde(rename = "02")]
    Html,
    #[serde(rename = "03")]
    Both,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SdkUiType {
    #[serde(rename = "01")]
    Text,
    #[serde(rename = "02")]
    SingleSelect,
    #[serde(rename = "03")]
    MultiSelect,
    #[serde(rename = "04")]
    Oob,
    #[serde(rename = "05")]
    HtmlOther,
}
