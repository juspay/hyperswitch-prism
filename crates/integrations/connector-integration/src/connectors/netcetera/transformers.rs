// Netcetera 3DS connector transformers.
//
// Implements the EMVCo 3DS authentication flows (PreAuthenticate /
// Authenticate / PostAuthenticate). Ported (pragmatically) from the router's
// `crates/hyperswitch_connectors/src/connectors/netcetera/transformers.rs`.
//
// Notable UCS-specific differences vs. the router port:
//   * The PAN uses `RawCardNumber<T>` (no Luhn) so VGS aliases pass through
//     (see `netcetera_types::CardholderAccount::acct_number`).
//   * The UCS `Payments{PreAuthenticate,Authenticate,PostAuthenticate}Data`
//     types are leaner than the router's authentication request data — they do
//     NOT carry the 3DS-state fields the router threaded between flows
//     (`threeds_server_transaction_id`, `device_channel`, `acquirer_bin`,
//     `connector_meta_data`, `webhook_url`, `message_version`, ...). Fields that
//     have no UCS source are defaulted with `// TODO(emvco)` markers.
//   * Auth (mTLS certificate) is handled by `ConnectorCommon`/the framework, not
//     here.

use common_utils::types::SemanticVersion;
use domain_types::{
    connector_flow::{Authenticate, Authorize, PostAuthenticate, PreAuthenticate},
    connector_types::{
        PaymentFlowData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{netcetera_types, NetceteraRouterData};
use crate::types::ResponseRouterData;

// ---------------------------------------------------------------------------
// Error response
// ---------------------------------------------------------------------------

/// Minimal error response envelope used by `build_error_response`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetceteraErrorResponse {
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
    #[serde(default)]
    pub error_detail: Option<String>,
}

/// Rich EMVCo error detail object (used inside flow failure responses).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraErrorDetails {
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: Option<String>,
    #[serde(rename = "acsTransID")]
    pub acs_trans_id: Option<String>,
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
    pub error_code: String,
    pub error_component: Option<String>,
    pub error_description: String,
    pub error_detail: Option<String>,
    #[serde(rename = "sdkTransID")]
    pub sdk_trans_id: Option<String>,
    pub error_message_type: Option<String>,
}

/// Build a UCS `ErrorResponse` from an EMVCo error detail object.
fn error_response_from_details(
    error_details: &NetceteraErrorDetails,
    http_code: u16,
) -> domain_types::router_data::ErrorResponse {
    domain_types::router_data::ErrorResponse {
        code: error_details.error_code.clone(),
        message: error_details.error_description.clone(),
        reason: error_details.error_detail.clone(),
        status_code: http_code,
        attempt_status: None,
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// ===========================================================================
// PreAuthenticate (3DS version / method call)
// ===========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraPreAuthenticateRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    /// PAN (or VGS alias) used to resolve the supported 3DS version / card range.
    /// `RawCardNumber<T>` so VGS aliases (non-Luhn) pass through.
    cardholder_account_number: domain_types::payment_method_data::RawCardNumber<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme_id: Option<netcetera_types::SchemeId>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NetceteraRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NetceteraPreAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NetceteraRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let card = get_card(&item.router_data.request.payment_method_data)?;
        Ok(Self {
            scheme_id: resolve_scheme_id(&card)?,
            cardholder_account_number: card.card_number,
        })
    }
}

/// Version/method ("PRes") response. Untagged so a success body or an error
/// envelope deserializes into the appropriate variant.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NetceteraPreAuthenticateResponse {
    Success(Box<NetceteraPreAuthenticationResponseData>),
    Failure(Box<NetceteraPreAuthFailureResponse>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraPreAuthFailureResponse {
    pub error_details: NetceteraErrorDetails,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraPreAuthenticationResponseData {
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: String,
    #[serde(default)]
    pub card_ranges: Vec<CardRange>,
}

impl NetceteraPreAuthenticationResponseData {
    /// Pick the card range advertising the highest supported 3DS version.
    pub fn get_card_range_if_available(&self) -> Option<CardRange> {
        self.card_ranges
            .iter()
            .max_by_key(|card_range| card_range.highest_common_supported_version.clone())
            .cloned()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardRange {
    pub scheme_id: netcetera_types::SchemeId,
    pub directory_server_id: Option<String>,
    #[serde(default)]
    pub acs_protocol_versions: Vec<AcsProtocolVersion>,
    #[serde(rename = "threeDSMethodDataForm")]
    pub three_ds_method_data_form: Option<ThreeDSMethodDataForm>,
    pub highest_common_supported_version: SemanticVersion,
}

impl CardRange {
    pub fn get_three_ds_method_url(&self) -> Option<String> {
        self.acs_protocol_versions
            .iter()
            .find(|v| v.version == self.highest_common_supported_version)
            .and_then(|v| v.three_ds_method_url.clone())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThreeDSMethodDataForm {
    /// base64 encoded value for 3ds method data collection
    #[serde(rename = "threeDSMethodData")]
    pub three_ds_method_data: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AcsProtocolVersion {
    pub version: SemanticVersion,
    #[serde(rename = "threeDSMethodURL")]
    pub three_ds_method_url: Option<String>,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NetceteraPreAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NetceteraPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            NetceteraPreAuthenticateResponse::Success(pre_authn_response) => {
                let card_range = pre_authn_response.get_card_range_if_available();
                // Version "0.0.0" is < "2.0.0", treating a card with no range as
                // not eligible for 3DS authentication.
                let maximum_supported_3ds_version = card_range
                    .as_ref()
                    .map(|range| range.highest_common_supported_version.clone())
                    .unwrap_or_else(|| SemanticVersion::new(0, 0, 0));

                // 3DS method (device data collection) form -> redirection data.
                let three_ds_method_data = card_range.as_ref().and_then(|range| {
                    range
                        .three_ds_method_data_form
                        .as_ref()
                        .map(|data| data.three_ds_method_data.clone())
                });
                let three_ds_method_url = card_range
                    .as_ref()
                    .and_then(|range| range.get_three_ds_method_url());

                // 3DS method (device data collection) form -> `RedirectForm::Form` so it
                // serializes across the gRPC boundary. The `DeutschebankThreeDSChallengeFlow`
                // variant is NOT representable in the gRPC `RedirectForm` proto (the response
                // builder rejects it with "Invalid response type received from connector"),
                // whereas `Form` maps cleanly and carries the DDC fields the router uses to
                // build the SDK `ThreeDsInvoke` next_action.
                let redirection_data = match (three_ds_method_url, three_ds_method_data) {
                    (Some(acs_url), Some(creq)) => {
                        let mut form_fields = std::collections::HashMap::new();
                        form_fields.insert("threeDsMethodData".to_string(), creq);
                        form_fields.insert("threeDsMethodUrl".to_string(), acs_url.clone());
                        form_fields.insert(
                            "threeDSServerTransID".to_string(),
                            pre_authn_response.three_ds_server_trans_id.clone(),
                        );
                        Some(Box::new(RedirectForm::Form {
                            endpoint: acs_url,
                            method: common_utils::Method::Post,
                            form_fields,
                        }))
                    }
                    _ => None,
                };

                let authentication_data = AuthenticationData {
                    trans_status: None,
                    eci: None,
                    cavv: None,
                    ucaf_collection_indicator: None,
                    threeds_server_transaction_id: Some(
                        pre_authn_response.three_ds_server_trans_id.clone(),
                    ),
                    message_version: Some(maximum_supported_3ds_version),
                    ds_trans_id: card_range
                        .as_ref()
                        .and_then(|range| range.directory_server_id.clone()),
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
                };

                Ok(Self {
                    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                        resource_id: None,
                        authentication_data: Some(authentication_data),
                        redirection_data,
                        connector_response_reference_id: Some(
                            pre_authn_response.three_ds_server_trans_id,
                        ),
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            NetceteraPreAuthenticateResponse::Failure(error_response) => Ok(Self {
                response: Err(error_response_from_details(
                    &error_response.error_details,
                    item.http_code,
                )),
                ..item.router_data
            }),
        }
    }
}

// ===========================================================================
// Authenticate (AReq / ARes)
// ===========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct NetceteraAuthenticateRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub preferred_protocol_version: Option<SemanticVersion>,
    pub enforce_preferred_protocol_version: Option<bool>,
    pub device_channel: netcetera_types::NetceteraDeviceChannel,
    pub message_category: netcetera_types::NetceteraMessageCategory,
    #[serde(rename = "threeDSCompInd")]
    pub three_ds_comp_ind: Option<netcetera_types::ThreeDSMethodCompletionIndicator>,
    #[serde(rename = "threeDSRequestor")]
    pub three_ds_requestor: Option<netcetera_types::ThreeDSRequestor>,
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: Option<String>,
    #[serde(rename = "threeDSRequestorURL")]
    pub three_ds_requestor_url: Option<url::Url>,
    pub cardholder_account: netcetera_types::CardholderAccount<T>,
    pub cardholder: Option<netcetera_types::Cardholder>,
    pub purchase: Option<netcetera_types::Purchase>,
    pub acquirer: Option<netcetera_types::AcquirerData>,
    pub merchant: Option<netcetera_types::MerchantData>,
    pub browser_information: Option<netcetera_types::Browser>,
    pub sdk_information: Option<netcetera_types::Sdk>,
    pub device_render_options: Option<netcetera_types::DeviceRenderingOptionsSupported>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NetceteraRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NetceteraAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NetceteraRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let common_data = &item.router_data.resource_common_data;
        let request = &item.router_data.request;

        // 3DS state threaded from the PreAuthenticate step (if present).
        let prior_auth_data = request.authentication_data.as_ref();
        let message_version = prior_auth_data.and_then(|d| d.message_version.clone());
        let three_ds_server_trans_id =
            prior_auth_data.and_then(|d| d.threeds_server_transaction_id.clone());

        let ip_address = request
            .browser_info
            .as_ref()
            .and_then(|browser| browser.ip_address);

        // Per-merchant NON-auth config (acquirer / merchant objects) sourced from the merchant's
        // Netcetera account, riding the request on `connector_feature_data` and deserialized into
        // `NetceteraMeta` (same mechanism axisbank / nexinets use). Absent => objects omitted and
        // the 3DS Server falls back to its stored merchant config. See `NetceteraMeta` for the
        // router-side contract.
        let netcetera_meta: Option<netcetera_types::NetceteraMeta> =
            match common_data.connector_feature_data {
                Some(_) => Some(crate::utils::to_connector_meta_from_secret(
                    common_data.connector_feature_data.clone(),
                )?),
                None => None,
            };

        // Account-level identity from the typed connector config, falling back to the
        // deprecated `NetceteraMeta` blob.
        let (cfg_merchant_configuration_id, cfg_three_ds_requestor_id, cfg_three_ds_requestor_name) =
            match &item.router_data.connector_config {
                domain_types::router_data::ConnectorSpecificConfig::Netcetera {
                    merchant_configuration_id,
                    three_ds_requestor_id,
                    three_ds_requestor_name,
                    ..
                } => (
                    merchant_configuration_id.clone(),
                    three_ds_requestor_id.clone(),
                    three_ds_requestor_name.clone(),
                ),
                _ => (None, None, None),
            };

        // Requestor challenge preference: typed field first; deprecated fallback maps the
        // legacy `force_3ds_challenge = true` to EMVCo 04 (challenge mandated).
        let challenge_indicator = request
            .three_ds_requestor_challenge_indicator
            .map(netcetera_types::ThreeDSRequestorChallengeIndicator::from)
            .or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.force_3ds_challenge)
                    .filter(|force| *force)
                    .map(|_| netcetera_types::ThreeDSRequestorChallengeIndicator::ChallengeRequestedMandate)
            });
        let authentication_indicator = request
            .three_ds_requestor_authentication_indicator
            .map(netcetera_types::ThreeDSRequestorAuthenticationIndicator::from)
            .unwrap_or(netcetera_types::ThreeDSRequestorAuthenticationIndicator::Payment);

        let three_ds_requestor = netcetera_types::ThreeDSRequestor::new(
            ip_address,
            challenge_indicator,
            authentication_indicator,
            message_version
                .as_ref()
                .unwrap_or(&SemanticVersion::new(2, 1, 0)),
        );

        let card = get_card(&request.payment_method_data)?;
        let cardholder_account = netcetera_types::CardholderAccount {
            card_expiry_date: Some(card.get_expiry_date_as_yymm()?),
            scheme_id: resolve_scheme_id(&card)?,
            acct_number: card.card_number,
            card_security_code: Some(card.card_cvc),
        };

        let purchase = request.currency.map(|currency| {
            // EMVCo exponent: number of minor-unit digits. Falls back to 2 for
            // currencies UCS cannot classify.
            let purchase_exponent = currency.number_of_digits_after_decimal_point().unwrap_or(2);
            let purchase_date = common_utils::date_time::format_date(
                common_utils::date_time::now(),
                common_utils::date_time::DateFormat::YYYYMMDDHHmmss,
            )
            .ok();
            netcetera_types::Purchase {
                purchase_amount: Some(request.amount),
                purchase_currency: currency.iso_4217().to_string(),
                purchase_exponent,
                purchase_date,
                // 01 -> Goods and Services (only use case served for now).
                trans_type: Some("01".to_string()),
            }
        });

        let is_app = matches!(
            request.device_channel,
            Some(domain_types::connector_types::DeviceChannel::App)
        );

        let browser_information = if is_app {
            None
        } else {
            request
                .browser_info
                .clone()
                .map(netcetera_types::Browser::from)
        };

        let sdk_information = if is_app {
            request
                .sdk_information
                .clone()
                .map(netcetera_types::Sdk::from)
        } else {
            None
        };

        let device_render_options = if is_app {
            Some(netcetera_types::DeviceRenderingOptionsSupported {
                sdk_interface: netcetera_types::SdkInterface::Both,
                sdk_ui_type: vec![
                    netcetera_types::SdkUiType::Text,
                    netcetera_types::SdkUiType::SingleSelect,
                    netcetera_types::SdkUiType::MultiSelect,
                    netcetera_types::SdkUiType::Oob,
                    netcetera_types::SdkUiType::HtmlOther,
                ],
            })
        } else {
            None
        };

        let cardholder = netcetera_types::Cardholder::try_from((
            common_data.address.get_payment_billing().cloned(),
            common_data.address.get_shipping().cloned(),
        ))
        .map_err(|_| IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;

        // Acquirer object: typed `acquirer_details` first, deprecated blob fallback.
        let acquirer = request
            .acquirer_details
            .as_ref()
            .map(netcetera_types::AcquirerData::from)
            .or_else(|| {
                netcetera_meta
                    .as_ref()
                    .map(netcetera_types::NetceteraMeta::to_acquirer_data)
            });

        // Browser CRes return URL (EMVCo notificationURL): the request's return_url is the
        // single source of truth; the deprecated blob `notification_url` is the fallback.
        let return_url = common_data
            .return_url
            .as_ref()
            .and_then(|u| url::Url::parse(u).ok())
            .or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.notification_url.clone())
            });
        // RReq push target (resultsResponseNotificationUrl): the request's webhook_url, with the
        // deprecated blob value as fallback.
        let results_response_notification_url = request
            .webhook_url
            .as_ref()
            .and_then(|u| url::Url::parse(u).ok())
            .or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.results_response_notification_url.clone())
            });

        // Merchant object: typed `merchant_details` + connector config first, blob fallback.
        let merchant_details = request.merchant_details.as_ref();
        let merchant_data = netcetera_types::MerchantData {
            merchant_configuration_id: cfg_merchant_configuration_id.or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.merchant_configuration_id.clone())
            }),
            mcc: merchant_details
                .and_then(|m| m.merchant_category_code)
                .map(|mcc| format!("{mcc:04}"))
                .or_else(|| netcetera_meta.as_ref().and_then(|m| m.mcc.clone())),
            // EMVCo merchantCountryCode is ISO 3166-1 numeric.
            merchant_country_code: merchant_details
                .and_then(|m| m.merchant_country_code)
                .map(|country| format!("{:03}", common_enums::CountryAlpha2::to_numeric(country)))
                .or_else(|| {
                    netcetera_meta
                        .as_ref()
                        .and_then(|m| m.merchant_country_code.clone())
                }),
            merchant_name: merchant_details
                .and_then(|m| m.merchant_name.clone())
                .or_else(|| {
                    netcetera_meta
                        .as_ref()
                        .and_then(|m| m.merchant_name.clone())
                }),
            notification_url: return_url.clone(),
            three_ds_requestor_id: cfg_three_ds_requestor_id.or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.three_ds_requestor_id.clone())
            }),
            three_ds_requestor_name: cfg_three_ds_requestor_name.or_else(|| {
                netcetera_meta
                    .as_ref()
                    .and_then(|m| m.three_ds_requestor_name.clone())
            }),
            results_response_notification_url,
        };
        // Omit the object entirely when nothing is set so the 3DS Server falls back to its
        // stored merchant configuration, as before.
        let merchant = (merchant_data.merchant_configuration_id.is_some()
            || merchant_data.mcc.is_some()
            || merchant_data.merchant_country_code.is_some()
            || merchant_data.merchant_name.is_some()
            || merchant_data.notification_url.is_some()
            || merchant_data.three_ds_requestor_id.is_some()
            || merchant_data.three_ds_requestor_name.is_some()
            || merchant_data.results_response_notification_url.is_some())
        .then_some(merchant_data);

        Ok(Self {
            preferred_protocol_version: message_version,
            enforce_preferred_protocol_version: Some(is_app),
            device_channel: request
                .device_channel
                .map(netcetera_types::NetceteraDeviceChannel::from)
                .unwrap_or(netcetera_types::NetceteraDeviceChannel::Browser),
            message_category: request
                .message_category
                .map(netcetera_types::NetceteraMessageCategory::from)
                .unwrap_or(netcetera_types::NetceteraMessageCategory::PaymentAuthentication),
            // threeDSCompInd from the typed field; "U" (3DS Method not available) when the
            // caller does not report a DDC outcome.
            three_ds_comp_ind: Some(
                request
                    .threeds_completion_indicator
                    .clone()
                    .map(netcetera_types::ThreeDSMethodCompletionIndicator::from)
                    .unwrap_or(netcetera_types::ThreeDSMethodCompletionIndicator::U),
            ),
            three_ds_requestor: Some(three_ds_requestor),
            three_ds_server_trans_id,
            three_ds_requestor_url: return_url,
            cardholder_account,
            cardholder: Some(cardholder),
            purchase,
            acquirer,
            merchant,
            browser_information,
            sdk_information,
            device_render_options,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NetceteraAuthenticateResponse {
    Error(Box<NetceteraAuthenticationFailureResponse>),
    Success(Box<NetceteraAuthenticationSuccessResponse>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraAuthenticationSuccessResponse {
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: String,
    pub trans_status: common_enums::TransactionStatus,
    pub authentication_value: Option<Secret<String>>,
    pub eci: Option<String>,
    pub acs_challenge_mandated: Option<ACSChallengeMandatedIndicator>,
    pub authentication_response: AuthenticationResponse,
    #[serde(rename = "base64EncodedChallengeRequest")]
    pub encoded_challenge_request: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraAuthenticationFailureResponse {
    pub error_details: NetceteraErrorDetails,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    #[serde(rename = "acsURL")]
    pub acs_url: Option<url::Url>,
    pub acs_reference_number: Option<String>,
    #[serde(rename = "acsTransID")]
    pub acs_trans_id: Option<String>,
    #[serde(rename = "dsTransID")]
    pub ds_trans_id: Option<String>,
    pub acs_signed_content: Option<String>,
    pub trans_status_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NetceteraChallengeFeatureData {
    pub acs_signed_content: Option<String>,
    pub acs_reference_number: Option<String>,
    pub acs_trans_id: Option<String>,
    pub three_ds_server_trans_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ACSChallengeMandatedIndicator {
    /// Challenge is mandated
    Y,
    /// Challenge is not mandated
    N,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NetceteraAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NetceteraAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            NetceteraAuthenticateResponse::Success(response) => {
                // Mirrors hyperswitch's own Netcetera connector (see PR #13403): checking only
                // `acs_challenge_mandated == Y` misses the case where the ACS returns
                // `trans_status: ChallengeRequired` (or the decoupled variant) without
                // explicitly mandating it — that payment would incorrectly be treated as
                // frictionless here, diverging from the direct-connector path for the same
                // response.
                let is_challenge = matches!(
                    response.trans_status,
                    common_enums::TransactionStatus::ChallengeRequired
                        | common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication
                );

                // Challenge flow -> redirection (ACS) form. Frictionless flow ->
                // authentication data carrying the cryptogram / ECI / status.
                // Challenge (ACS) form -> `RedirectForm::Form { endpoint = acs_url,
                // form_fields = { creq } }`. This is the shape the router's
                // `/3ds/authentication` handler expects (it reads the `creq` form field),
                // and unlike `DeutschebankThreeDSChallengeFlow` it is representable in the
                // gRPC `RedirectForm` proto (the Deutschebank variant is rejected by the
                // authenticate response builder with "Invalid response type received").
                // `RedirectForm` carries only what a browser literally submits (`endpoint` +
                // `creq`) — acs_reference_number/acs_trans_id/acs_signed_content have no such
                // role and go exclusively through `connector_feature_data` below, not
                // duplicated into `form_fields` too.
                let redirection_data = if is_challenge {
                    match (
                        response.authentication_response.acs_url.clone(),
                        response.encoded_challenge_request.clone(),
                    ) {
                        (Some(acs_url), Some(creq)) => {
                            let mut form_fields = std::collections::HashMap::new();
                            form_fields.insert("creq".to_string(), creq);
                            form_fields.insert(
                                "threeDSServerTransID".to_string(),
                                response.three_ds_server_trans_id.clone(),
                            );
                            Some(Box::new(RedirectForm::Form {
                                endpoint: acs_url.to_string(),
                                method: common_utils::Method::Post,
                                form_fields,
                            }))
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                let connector_feature_data = is_challenge
                    .then(|| {
                        serde_json::to_value(NetceteraChallengeFeatureData {
                            acs_signed_content: response
                                .authentication_response
                                .acs_signed_content
                                .clone(),
                            acs_reference_number: response
                                .authentication_response
                                .acs_reference_number
                                .clone(),
                            acs_trans_id: response.authentication_response.acs_trans_id.clone(),
                            three_ds_server_trans_id: response.three_ds_server_trans_id.clone(),
                        })
                    })
                    .transpose()
                    .change_context(ConnectorError::ResponseHandlingFailed {
                        context: Default::default(),
                    })?;

                let authentication_data = AuthenticationData {
                    trans_status: Some(response.trans_status),
                    eci: response.eci.clone(),
                    cavv: response.authentication_value.clone(),
                    ucaf_collection_indicator: None,
                    threeds_server_transaction_id: Some(response.three_ds_server_trans_id.clone()),
                    message_version: None,
                    ds_trans_id: response.authentication_response.ds_trans_id.clone(),
                    acs_transaction_id: response.authentication_response.acs_trans_id.clone(),
                    transaction_id: None,
                    network_params: None,
                    exemption_indicator: None,
                    created_at: None,
                    challenge_code: None,
                    challenge_cancel: None,
                    challenge_code_reason: None,
                    message_extension: None,
                    authentication_type: None,
                };

                Ok(Self {
                    response: Ok(PaymentsResponseData::AuthenticateResponse {
                        resource_id: None,
                        redirection_data,
                        // Mirrors hyperswitch's own Netcetera connector: `trans_status` (and the
                        // rest of `authentication_data`) is always carried on the ARes, challenge
                        // or frictionless alike — cavv/eci are simply absent until a challenge
                        // resolves via the RReq webhook, same as the direct integration.
                        authentication_data: Some(authentication_data),
                        connector_feature_data,
                        connector_response_reference_id: Some(response.three_ds_server_trans_id),
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            NetceteraAuthenticateResponse::Error(error_response) => Ok(Self {
                response: Err(error_response_from_details(
                    &error_response.error_details,
                    item.http_code,
                )),
                ..item.router_data
            }),
        }
    }
}

// ===========================================================================
// PostAuthenticate (results fetch / RReq -> RRes)
// ===========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde_with::skip_serializing_none]
pub struct NetceteraPostAuthenticateRequest {
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: Option<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NetceteraRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NetceteraPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: NetceteraRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // The challenge result is correlated by the 3DS Server transaction id,
        // which arrives in the redirect response params for the results fetch.
        let three_ds_server_trans_id = item
            .router_data
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect| redirect.params.as_ref())
            .map(|params| params.clone().expose());
        Ok(Self {
            three_ds_server_trans_id,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetceteraPostAuthenticateResponse {
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: Option<String>,
    pub trans_status: Option<common_enums::TransactionStatus>,
    pub authentication_value: Option<Secret<String>>,
    pub eci: Option<String>,
    pub error_details: Option<NetceteraErrorDetails>,
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NetceteraPostAuthenticateResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<NetceteraPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        if let Some(error_details) = item.response.error_details.as_ref() {
            return Ok(Self {
                response: Err(error_response_from_details(error_details, item.http_code)),
                ..item.router_data
            });
        }

        let authentication_data = AuthenticationData {
            trans_status: item.response.trans_status,
            eci: item.response.eci.clone(),
            cavv: item.response.authentication_value.clone(),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: item.response.three_ds_server_trans_id.clone(),
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
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data: Some(authentication_data),
                connector_response_reference_id: item.response.three_ds_server_trans_id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===========================================================================
// Authorize (NOT SUPPORTED for an auth-only 3DS connector)
// ===========================================================================

/// STUB request body for the Authorize flow. Netcetera is an authentication-only
/// (3DS) connector and does not perform payment authorization.
#[derive(Debug, Clone, Serialize)]
pub struct NetceteraAuthorizeRequest;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetceteraAuthorizeResponse;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        NetceteraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for NetceteraAuthorizeRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: NetceteraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Err(IntegrationError::not_implemented(
            "Authorize flow is not supported by the Netcetera (3DS authentication-only) connector",
            Default::default(),
        ))?
    }
}

impl<F, T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<NetceteraAuthorizeResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        _item: ResponseRouterData<NetceteraAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Unreachable: the Authorize request body / URL builders already fail
        // with `NotImplemented` before any HTTP call is made.
        Err(ConnectorError::UnexpectedResponseError {
            context: Default::default(),
        })?
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Extract the (raw) card from the UCS payment method data, erroring on any
/// non-card method (Netcetera 3DS only authenticates cards).
fn get_card<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
    payment_method_data: &Option<PaymentMethodData<T>>,
) -> Result<domain_types::payment_method_data::Card<T>, error_stack::Report<IntegrationError>> {
    match payment_method_data {
        Some(PaymentMethodData::Card(card)) => Ok(card.clone()),
        Some(_) => Err(IntegrationError::NotSupported {
            message: "Only card payment method is supported".to_string(),
            connector: "netcetera",
            context: Default::default(),
        })?,
        None => Err(IntegrationError::MissingRequiredField {
            field_name: "payment_method_data",
            context: Default::default(),
        })?,
    }
}

/// Resolve the EMVCo scheme id from the card network, only when the card is
/// cobadged (mirrors the router behaviour).
fn resolve_scheme_id<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
) -> Result<Option<netcetera_types::SchemeId>, error_stack::Report<IntegrationError>> {
    let is_cobadged = card.card_number.is_cobadged_card().map_err(|_| {
        error_stack::report!(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
    })?;
    match (is_cobadged, card.card_network.clone()) {
        (true, Some(card_network)) => Ok(Some(netcetera_types::SchemeId::try_from(card_network)?)),
        _ => Ok(None),
    }
}
