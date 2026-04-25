use std::{str::FromStr, sync::LazyLock};

use crate::{
    connectors::redsys::{RedsysAmountConvertor, RedsysRouterData},
    types::ResponseRouterData,
    utils,
};
use base64::Engine;
use common_enums::enums;
use common_utils::{
    consts::BASE64_ENGINE,
    crypto::{self, EncodeMessage, SignMessage},
    ext_traits::Encode,
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, PSync, PreAuthenticate, RSync,
        Refund, Void,
    },
    connector_types::{
        self, ClientAuthenticationTokenData, ClientAuthenticationTokenRequestData,
        ConnectorSpecificClientAuthenticationResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RedsysClientAuthenticationResponse as RedsysClientAuthenticationResponseDomain,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, ResponseId,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub const SIGNATURE_VERSION: &str = "HMAC_SHA256_V1";
pub const DS_VERSION: &str = "0.0";
pub const XMLNS_WEB_URL: &str = "http://webservices.apl02.redsys.es";
pub const REDSYS_SOAP_ACTION: &str = "consultaOperaciones";
pub const REDSYS_ORDER_ID_METADATA_KEY: &str = "order_id";
pub const REDSYS_ORDER_ID_MAX_LENGTH: usize = 12;

static LWV_THRESHOLD: LazyLock<common_utils::types::MinorUnit> =
    LazyLock::new(|| common_utils::types::MinorUnit::new(3000)); // €30

type Error = Report<IntegrationError>;
type ResponseError = Report<ConnectorError>;

fn get_redsys_order_id_from_metadata(
    metadata: Option<&Secret<serde_json::Value>>,
) -> Option<String> {
    metadata
        .and_then(|meta| meta.peek().as_object())
        .and_then(|obj| obj.get(REDSYS_ORDER_ID_METADATA_KEY))
        .and_then(|value| value.as_str())
        .map(|s| s.to_string())
        .filter(|s| s.len() <= REDSYS_ORDER_ID_MAX_LENGTH)
}

fn get_ds_merchant_order(
    connector_request_reference_id: String,
    metadata: Option<&Secret<serde_json::Value>>,
) -> Result<String, Error> {
    if connector_request_reference_id.len() <= REDSYS_ORDER_ID_MAX_LENGTH {
        return Ok(connector_request_reference_id);
    }

    Ok(get_redsys_order_id_from_metadata(metadata).ok_or_else(|| {
        IntegrationError::MaxFieldLengthViolated {
            connector: "Redsys".to_string(),
            field_name: "ds_merchant_order".to_string(),
            max_length: REDSYS_ORDER_ID_MAX_LENGTH,
            received_length: connector_request_reference_id.len(),
            context: Default::default(),
        }
    })?)
}

// Specifies the type of transaction for XML requests
// Specifies the type of transaction for XML requests
pub mod transaction_type {
    pub const PAYMENT: &str = "0";
    pub const PREAUTHORIZATION: &str = "1";
    pub const CONFIRMATION: &str = "2";
    pub const REFUND: &str = "3";
    pub const CANCELLATION: &str = "9";
}

// Common Structs
/// Authentication credentials for Redsys connector
pub struct RedsysAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) terminal_id: Secret<String>,
    pub(super) sha256_pwd: Secret<String>,
}

/// Signed transaction envelope sent to Redsys
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedsysTransaction {
    #[serde(rename = "Ds_SignatureVersion")]
    pub ds_signature_version: String,
    #[serde(rename = "Ds_MerchantParameters")]
    pub ds_merchant_parameters: Secret<String>,
    #[serde(rename = "Ds_Signature")]
    pub ds_signature: Secret<String>,
}

// Helper functions and impls

impl requests::RedsysEmvThreeDsRequestData {
    pub fn new(three_d_s_info: requests::RedsysThreeDsInfo) -> Self {
        Self {
            three_d_s_info,
            protocol_version: None,
            browser_accept_header: None,
            browser_user_agent: None,
            browser_java_enabled: None,
            browser_javascript_enabled: None,
            browser_language: None,
            browser_color_depth: None,
            browser_screen_height: None,
            browser_screen_width: None,
            browser_t_z: None,
            browser_i_p: None,
            three_d_s_server_trans_i_d: None,
            notification_u_r_l: None,
            three_d_s_comp_ind: None,
            cres: None,
            billing_data: None,
            shipping_data: None,
        }
    }

    pub fn add_browser_data(
        mut self,
        browser_info: domain_types::router_request_types::BrowserInformation,
    ) -> Result<Self, Error> {
        self.browser_accept_header = Some(browser_info.get_accept_header()?);
        self.browser_user_agent = Some(Secret::new(browser_info.get_user_agent()?));
        self.browser_java_enabled = Some(browser_info.get_java_enabled()?);
        self.browser_javascript_enabled = browser_info.get_java_script_enabled().ok();
        self.browser_language = Some(browser_info.get_language()?);
        self.browser_color_depth = Some(browser_info.get_color_depth()?.to_string());
        self.browser_screen_height = Some(browser_info.get_screen_height()?.to_string());
        self.browser_screen_width = Some(browser_info.get_screen_width()?.to_string());
        self.browser_t_z = Some(browser_info.get_time_zone()?.to_string());
        self.browser_i_p = Some(browser_info.get_ip_address()?);
        Ok(self)
    }

    pub fn set_three_d_s_server_trans_i_d(mut self, three_d_s_server_trans_i_d: String) -> Self {
        self.three_d_s_server_trans_i_d = Some(three_d_s_server_trans_i_d);
        self
    }

    pub fn set_protocol_version(mut self, protocol_version: String) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }

    pub fn set_notification_u_r_l(mut self, notification_u_r_l: url::Url) -> Self {
        self.notification_u_r_l = Some(notification_u_r_l.to_string());
        self
    }

    pub fn set_three_d_s_comp_ind(
        mut self,
        three_d_s_comp_ind: requests::RedsysThreeDSCompInd,
    ) -> Self {
        self.three_d_s_comp_ind = Some(three_d_s_comp_ind);
        self
    }

    pub fn set_three_d_s_cres(mut self, cres: String) -> Self {
        self.cres = Some(cres);
        self
    }

    pub fn set_billing_data(
        mut self,
        address: Option<&domain_types::payment_address::Address>,
    ) -> Result<Self, Error> {
        self.billing_data = address
            .and_then(|address| {
                address.address.as_ref().map(|address_details| {
                    let state = address_details
                        .state
                        .clone()
                        .map(|state| domain_types::utils::convert_spain_state_to_code(state.peek()))
                        .transpose();

                    match state {
                        Ok(bill_addr_state) => Ok(requests::RedsysBillingData {
                            bill_addr_city: address_details.city.clone(),
                            bill_addr_country: address_details.get_optional_country().map(
                                |country| {
                                    common_enums::CountryAlpha2::from_alpha2_to_alpha3(country)
                                        .to_string()
                                },
                            ),
                            bill_addr_line1: address_details.line1.clone(),
                            bill_addr_line2: address_details.line2.clone(),
                            bill_addr_line3: address_details.line3.clone(),
                            bill_addr_postal_code: address_details.zip.clone(),
                            bill_addr_state: bill_addr_state.map(Secret::new),
                        }),
                        Err(err) => Err(err),
                    }
                })
            })
            .transpose()?;
        Ok(self)
    }

    pub fn set_shipping_data(
        mut self,
        address: Option<&domain_types::payment_address::Address>,
    ) -> Result<Self, Error> {
        self.shipping_data = address
            .and_then(|address| {
                address.address.as_ref().map(|address_details| {
                    let state = address_details
                        .state
                        .clone()
                        .map(|state| domain_types::utils::convert_spain_state_to_code(state.peek()))
                        .transpose();

                    match state {
                        Ok(ship_addr_state) => Ok(requests::RedsysShippingData {
                            ship_addr_city: address_details.city.clone(),
                            ship_addr_country: address_details.get_optional_country().map(
                                |country| {
                                    common_enums::CountryAlpha2::from_alpha2_to_alpha3(country)
                                        .to_string()
                                },
                            ),
                            ship_addr_line1: address_details.line1.clone(),
                            ship_addr_line2: address_details.line2.clone(),
                            ship_addr_line3: address_details.line3.clone(),
                            ship_addr_postal_code: address_details.zip.clone(),
                            ship_addr_state: ship_addr_state.map(Secret::new),
                        }),
                        Err(err) => Err(err),
                    }
                })
            })
            .transpose()?;
        Ok(self)
    }
}

impl<T> TryFrom<&Option<PaymentMethodData<T>>> for requests::RedsysCardData<T>
where
    T: PaymentMethodDataTypes,
{
    type Error = Error;
    fn try_from(payment_method_data: &Option<PaymentMethodData<T>>) -> Result<Self, Self::Error> {
        match payment_method_data {
            Some(PaymentMethodData::Card(card)) => {
                let year = card.get_card_expiry_year_2_digit()?.expose();
                let month = card.get_card_expiry_month_2_digit()?.expose();
                let expiry_date = Secret::new(format!("{year}{month}"));
                Ok(Self {
                    card_number: card.card_number.clone(),
                    cvv2: card.card_cvc.clone(),
                    expiry_date,
                })
            }
            Some(PaymentMethodData::Wallet(..))
            | Some(PaymentMethodData::PayLater(..))
            | Some(PaymentMethodData::BankDebit(..))
            | Some(PaymentMethodData::BankRedirect(..))
            | Some(PaymentMethodData::BankTransfer(..))
            | Some(PaymentMethodData::Crypto(..))
            | Some(PaymentMethodData::MandatePayment)
            | Some(PaymentMethodData::GiftCard(..))
            | Some(PaymentMethodData::Voucher(..))
            | Some(PaymentMethodData::CardRedirect(..))
            | Some(PaymentMethodData::Reward)
            | Some(PaymentMethodData::RealTimePayment(..))
            | Some(PaymentMethodData::MobilePayment(..))
            | Some(PaymentMethodData::Upi(..))
            | Some(PaymentMethodData::OpenBanking(_))
            | Some(PaymentMethodData::PaymentMethodToken(..))
            // TODO: Implement CardToken support for Redsys InSite SDK flow.
            // After CreateClientAuthenticationToken returns merchant_parameters/signature,
            // the InSite JS SDK tokenizes card data and returns an operationId.
            // CardToken should extract payment_method_token and pass it as Ds_Merchant_Identifier
            // in the authorize request, following the Globalpay .map() pattern.
            | Some(PaymentMethodData::CardToken(..))
            | Some(PaymentMethodData::NetworkToken(..))
            | Some(PaymentMethodData::CardDetailsForNetworkTransactionId(_))
            | Some(PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_))
            | None => Err(IntegrationError::NotImplemented(
                domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
                Default::default(),
            )
            .into()),
        }
    }
}

impl TryFrom<&ConnectorSpecificConfig> for RedsysAuthType {
    type Error = Error;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Redsys {
            merchant_id,
            terminal_id,
            sha256_pwd,
            ..
        } = auth_type
        {
            Ok(Self {
                merchant_id: merchant_id.to_owned(),
                terminal_id: terminal_id.to_owned(),
                sha256_pwd: sha256_pwd.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into())
        }
    }
}

fn des_encrypt(message: &str, key: &str) -> Result<Vec<u8>, Report<IntegrationError>> {
    let iv_array = [0u8; crypto::TripleDesEde3CBC::TRIPLE_DES_IV_LENGTH];
    let iv = iv_array.to_vec();
    let key_bytes = BASE64_ENGINE
        .decode(key)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Base64 decoding failed")?;
    let triple_des =
        crypto::TripleDesEde3CBC::new(Some(common_enums::CryptoPadding::ZeroPadding), iv)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable("Triple DES encryption failed")?;
    let encrypted = triple_des
        .encode_message(&key_bytes, message.as_bytes())
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Triple DES encryption failed")?;
    let expected_len = encrypted.len() - crypto::TripleDesEde3CBC::TRIPLE_DES_IV_LENGTH;
    let encrypted_trimmed = encrypted
        .get(..expected_len)
        .ok_or(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
        .attach_printable("Failed to trim encrypted data to the expected length")?;
    Ok(encrypted_trimmed.to_vec())
}

/// Generates HMAC-SHA256 signature for Redsys API requests
fn get_signature(
    order_id: &str,
    params: &str,
    key: &str,
) -> Result<String, Report<IntegrationError>> {
    let secret_ko = des_encrypt(order_id, key)?;
    let result =
        crypto::HmacSha256::sign_message(&crypto::HmacSha256, &secret_ko, params.as_bytes())
            .map_err(|_| IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
    let encoded = BASE64_ENGINE.encode(result);
    Ok(encoded)
}

/// Trait for types that can be used to calculate Redsys signatures
pub trait SignatureCalculationData {
    fn get_merchant_parameters(&self) -> Result<String, Error>;
    fn get_order_id(&self) -> String;
}

impl SignatureCalculationData for requests::RedsysPaymentRequest {
    fn get_merchant_parameters(&self) -> Result<String, Error> {
        self.encode_to_string_of_json()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
    }

    fn get_order_id(&self) -> String {
        self.ds_merchant_order.clone()
    }
}

impl SignatureCalculationData for requests::RedsysOperationRequest {
    fn get_merchant_parameters(&self) -> Result<String, Error> {
        self.encode_to_string_of_json()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
    }

    fn get_order_id(&self) -> String {
        self.ds_merchant_order.clone()
    }
}

impl<T> TryFrom<(&T, &RedsysAuthType)> for RedsysTransaction
where
    T: SignatureCalculationData,
{
    type Error = Error;
    fn try_from(data: (&T, &RedsysAuthType)) -> Result<Self, Self::Error> {
        let (request_data, auth) = data;
        let merchant_parameters = request_data.get_merchant_parameters()?;
        let ds_merchant_parameters = BASE64_ENGINE.encode(&merchant_parameters);
        let sha256_pwd = auth.sha256_pwd.clone().expose();
        let ds_merchant_order = request_data.get_order_id();
        let signature = get_signature(&ds_merchant_order, &ds_merchant_parameters, &sha256_pwd)?;
        Ok(Self {
            ds_signature_version: SIGNATURE_VERSION.to_string(),
            ds_merchant_parameters: Secret::new(ds_merchant_parameters),
            ds_signature: Secret::new(signature),
        })
    }
}

fn get_redsys_attempt_status(
    ds_response: responses::DsResponse,
    capture_method: Option<enums::CaptureMethod>,
    is_three_ds: bool,
    http_status: u16,
) -> Result<common_enums::AttemptStatus, ResponseError> {
    // Redsys consistently provides a 4-digit response code, where numbers ranging from 0000 to 0099 indicate successful transactions

    if ds_response.0.starts_with("00") && ds_response.0.as_str() != "0002" {
        match capture_method {
            Some(enums::CaptureMethod::Automatic) | None => {
                Ok(common_enums::AttemptStatus::Charged)
            }
            Some(enums::CaptureMethod::Manual) => Ok(common_enums::AttemptStatus::Authorized),
            _ => Err(Report::new(
                ConnectorError::response_handling_failed_with_context(
                    http_status,
                    Some("capture method not supported".to_string()),
                ),
            )),
        }
    } else {
        match ds_response.0.as_str() {
            "0900" => Ok(common_enums::AttemptStatus::Charged),
            "0400" | "0481" | "0940" | "9915" => Ok(common_enums::AttemptStatus::Voided),
            "0950" => Ok(common_enums::AttemptStatus::VoidFailed),
            "0195" => {
                // 0195 = Soft decline (issuer requests authentication)
                // If 3DS was requested → pending (issuer wants auth, flow continues)
                // If no 3DS was requested → failed (issuer rejected because they want 3DS)
                if is_three_ds {
                    Ok(common_enums::AttemptStatus::AuthenticationPending)
                } else {
                    Ok(common_enums::AttemptStatus::AuthenticationFailed)
                }
            }
            "0112" | "8210" | "8220" | "9998" | "9999" => {
                Ok(common_enums::AttemptStatus::AuthenticationPending)
            }
            "0129" | "0184" | "9256" | "9257" => {
                Ok(common_enums::AttemptStatus::AuthenticationFailed)
            }
            "0107" | "0300" => Ok(common_enums::AttemptStatus::Pending),
            "0101" | "0102" | "0104" | "0106" | "0110" | "0114" | "0115" | "0116" | "0117"
            | "0118" | "0121" | "0123" | "0125" | "0126" | "0130" | "0162" | "0163" | "0171"
            | "0172" | "0173" | "0174" | "0180" | "0181" | "0182" | "0187" | "0190" | "0191"
            | "0193" | "0201" | "0202" | "0204" | "0206" | "0290" | "0881" | "0904" | "0909"
            | "0912" | "0913" | "0941" | "0944" | "0945" | "0965" | "9912" | "9064" | "9078"
            | "9093" | "9094" | "9104" | "9218" | "9253" | "9261" | "9997" | "0002" => {
                Ok(common_enums::AttemptStatus::Failure)
            }
            error => Err(Report::from(utils::response_handling_fail_for_connector(
                http_status,
                "redsys",
            ))
            .attach_printable(format!("Received Unknown Status:{error}"))),
        }
    }
}

fn refund_status_from_ds_response(
    ds_response: responses::DsResponse,
    http_status: u16,
) -> Result<common_enums::RefundStatus, ResponseError> {
    match ds_response.0.as_str() {
        "0900" => Ok(common_enums::RefundStatus::Success),
        "9999" => Ok(common_enums::RefundStatus::Pending),
        "0950" | "0172" | "174" => Ok(common_enums::RefundStatus::Failure),
        unknown_status => Err(Report::from(utils::response_handling_fail_for_connector(
            http_status,
            "redsys",
        ))
        .attach_printable(format!("Received unknown refund status:{unknown_status}"))),
    }
}

fn to_connector_response_data<T>(
    connector_response: &str,
    http_status: u16,
) -> Result<T, ResponseError>
where
    T: serde::de::DeserializeOwned,
{
    let decoded_bytes = utils::safe_base64_decode(connector_response.to_string())
        .change_context(
            utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
        )
        .attach_printable("Failed to decode Base64")?;

    let response_data: T = serde_json::from_slice(&decoded_bytes).change_context(
        utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
    )?;

    Ok(response_data)
}

fn build_threeds_form(
    ds_emv3ds: &responses::RedsysEmv3DSResponseData,
    http_status: u16,
) -> Result<router_response_types::RedirectForm, ResponseError> {
    let creq = ds_emv3ds.creq.clone().ok_or(
        utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
    )?;

    let endpoint = ds_emv3ds.acs_u_r_l.clone().ok_or(
        utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
    )?;

    let mut form_fields = std::collections::HashMap::new();
    form_fields.insert("creq".to_string(), creq);

    Ok(router_response_types::RedirectForm::Form {
        endpoint,
        method: common_utils::request::Method::Post,
        form_fields,
    })
}

fn get_preauthenticate_response(
    response_data: &responses::RedsysPaymentsResponse,
    continue_redirection_url: Option<&url::Url>,
    existing_connector_meta: Option<Secret<serde_json::Value>>,
    http_status: u16,
) -> Result<responses::PreAuthenticateResponseData, ResponseError> {
    let emv3ds = match &response_data.ds_emv3ds {
        Some(emv3ds) => emv3ds,
        None => {
            return Ok(responses::PreAuthenticateResponseData {
                redirection_data: None,
                connector_meta_data: existing_connector_meta,
                response_ref_id: Some(response_data.ds_order.clone()),
                authentication_data: None,
            });
        }
    };

    let three_d_s_server_trans_i_d = emv3ds.three_d_s_server_trans_i_d.clone().ok_or(
        utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
    )?;

    let message_version = &emv3ds.protocol_version;
    let semantic_version = common_utils::types::SemanticVersion::from_str(message_version)
        .change_context(
            utils::response_deserialization_fail(http_status, "redsys: response body did not match the expected format; confirm API version and connector documentation."),
        )
        .attach_printable("Failed to parse message_version as SemanticVersion")?;

    let authentication_data = Some(domain_types::router_request_types::AuthenticationData {
        threeds_server_transaction_id: Some(three_d_s_server_trans_i_d.clone()),
        message_version: Some(semantic_version.clone()),
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id: None,
        exemption_indicator: None,
        network_params: None,
    });

    match &emv3ds.three_d_s_method_u_r_l {
        Some(three_ds_method_url) => build_threeds_invoke_response(
            response_data,
            &three_d_s_server_trans_i_d,
            three_ds_method_url,
            continue_redirection_url,
            semantic_version,
            http_status,
        ),
        None => build_threeds_exempt_response(response_data, authentication_data),
    }
}

fn build_threeds_invoke_response(
    response_data: &responses::RedsysPaymentsResponse,
    three_d_s_server_trans_i_d: &str,
    three_ds_method_url: &str,
    continue_redirection_url: Option<&url::Url>,
    protocol_version: common_utils::types::SemanticVersion,
    http_status: u16,
) -> Result<responses::PreAuthenticateResponseData, ResponseError> {
    let notification_url = continue_redirection_url
        .map(|url| url.to_string())
        .ok_or_else(|| {
            Report::new(ConnectorError::response_handling_failed_with_context(
                http_status,
                Some("continue_redirection_url missing for 3DS method URL".to_string()),
            ))
        })?;

    let threeds_invoke_request = requests::RedsysThreedsInvokeRequest {
        three_d_s_method_notification_u_r_l: notification_url,
        three_d_s_server_trans_i_d: three_d_s_server_trans_i_d.to_string(),
    };

    let three_ds_data_string = threeds_invoke_request
        .encode_to_string_of_json()
        .change_context(utils::response_handling_fail_for_connector(
            http_status,
            "redsys",
        ))?;

    let three_ds_method_data = BASE64_ENGINE.encode(&three_ds_data_string);

    let three_ds_invoke_data = requests::RedsysThreeDsInvokeData {
        message_version: protocol_version,
        three_ds_method_data: three_ds_method_data.clone(),
        three_ds_method_data_submission: true.to_string(),
        three_ds_method_url: three_ds_method_url.to_string(),
        three_d_s_server_trans_i_d: three_d_s_server_trans_i_d.to_string(),
    };

    // Serialize to JSON, then deserialize to HashMap<String, String>
    let json = serde_json::to_value(&three_ds_invoke_data).change_context(
        utils::response_handling_fail_for_connector(http_status, "redsys"),
    )?;
    let form_fields: std::collections::HashMap<String, String> =
        serde_json::from_value(json).change_context(
            utils::response_handling_fail_for_connector(http_status, "redsys"),
        )?;

    let redirect_form = Some(Box::new(router_response_types::RedirectForm::Form {
        endpoint: three_ds_method_url.to_string(),
        method: common_utils::request::Method::Post,
        form_fields,
    }));

    Ok(responses::PreAuthenticateResponseData {
        redirection_data: redirect_form,
        connector_meta_data: None,
        response_ref_id: Some(response_data.ds_order.clone()),
        authentication_data: None,
    })
}

fn build_threeds_exempt_response(
    response_data: &responses::RedsysPaymentsResponse,
    authentication_data: Option<domain_types::router_request_types::AuthenticationData>,
) -> Result<responses::PreAuthenticateResponseData, ResponseError> {
    Ok(responses::PreAuthenticateResponseData {
        redirection_data: None,
        connector_meta_data: None,
        response_ref_id: Some(response_data.ds_order.clone()),
        authentication_data,
    })
}

fn get_payments_response(
    redsys_payments_response: responses::RedsysPaymentsResponse,
    capture_method: Option<enums::CaptureMethod>,
    authentication_data: Option<domain_types::router_request_types::AuthenticationData>,
    is_three_ds: bool,
    http_code: u16,
    use_transaction_response: bool,
) -> Result<
    (
        Result<PaymentsResponseData, domain_types::router_data::ErrorResponse>,
        common_enums::AttemptStatus,
        String,
    ),
    ResponseError,
> {
    let authentication_data = Some(domain_types::router_request_types::AuthenticationData {
        threeds_server_transaction_id: authentication_data
            .clone()
            .as_ref()
            .and_then(|auth_data| auth_data.threeds_server_transaction_id.clone()),
        message_version: authentication_data
            .clone()
            .as_ref()
            .and_then(|auth_data| auth_data.message_version.clone()),
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id: None,
        exemption_indicator: None,
        network_params: None,
    });

    let ds_order = redsys_payments_response.ds_order.clone();

    if let Some(ds_response) = redsys_payments_response.ds_response {
        let status =
            get_redsys_attempt_status(ds_response.clone(), capture_method, is_three_ds, http_code)?;

        let response = if domain_types::utils::is_payment_failure(status) {
            let error_message = redsys_payments_response
                .ds_response_description
                .clone()
                .unwrap_or_else(|| ds_response.0.clone());

            Err(domain_types::router_data::ErrorResponse {
                code: ds_response.0.clone(),
                message: error_message.clone(),
                reason: Some(error_message),
                status_code: http_code,
                attempt_status: None,
                connector_transaction_id: Some(redsys_payments_response.ds_order.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            })
        } else if use_transaction_response {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        } else {
            Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                )),
                redirection_data: None,
                authentication_data,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                status_code: http_code,
            })
        };

        Ok((response, status, ds_order))
    } else {
        let redirection_form = redsys_payments_response
            .ds_emv3ds
            .map(|ds_emv3ds| build_threeds_form(&ds_emv3ds, http_code))
            .transpose()?;

        let response = if use_transaction_response {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                ),
                redirection_data: redirection_form.map(Box::new),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
            })
        } else {
            Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                )),
                redirection_data: redirection_form.map(Box::new),
                authentication_data,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                status_code: http_code,
            })
        };

        Ok((
            response,
            common_enums::AttemptStatus::AuthenticationPending,
            ds_order,
        ))
    }
}

impl From<connector_types::ThreeDsCompletionIndicator> for requests::RedsysThreeDSCompInd {
    fn from(value: connector_types::ThreeDsCompletionIndicator) -> Self {
        match value {
            connector_types::ThreeDsCompletionIndicator::Success => Self::Y,
            connector_types::ThreeDsCompletionIndicator::Failure => Self::N,
            connector_types::ThreeDsCompletionIndicator::NotAvailable => Self::U,
        }
    }
}

// PreAuthenticate

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysPreAuthenticateRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    T::Inner: Clone,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
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

        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;
        let card_data =
            requests::RedsysCardData::try_from(&router_data.request.payment_method_data.clone())?;
        let is_auto_capture = router_data.request.is_auto_capture()?;
        let amount = RedsysAmountConvertor::convert(
            router_data.request.amount,
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?,
        )?;

        let ds_merchant_emv3ds = Some(requests::RedsysEmvThreeDsRequestData::new(
            requests::RedsysThreeDsInfo::CardData,
        ));
        let ds_merchant_transactiontype = if is_auto_capture {
            requests::RedsysTransactionType::Payment
        } else {
            requests::RedsysTransactionType::Preauthorization
        };

        let connector_request_reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let ds_merchant_order = get_ds_merchant_order(connector_request_reference_id, None)?;

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: amount,
            ds_merchant_currency: router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?
                .iso_4217()
                .to_owned(),
            ds_merchant_cvv2: card_data.cvv2,
            ds_merchant_emv3ds,
            ds_merchant_expirydate: card_data.expiry_date,
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })
                .attach_printable("Invalid card number")?,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_excep_sca: None,
            ds_merchant_directpayment: None,
        };

        let transaction = Self::try_from((&payment_request, &auth))?;
        Ok(transaction)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysPaymentsResponse = to_connector_response_data(
                    &transaction.ds_merchant_parameters.clone().expose(),
                    item.http_code,
                )?;

                let responses::PreAuthenticateResponseData {
                    redirection_data,
                    connector_meta_data,
                    response_ref_id,
                    authentication_data,
                } = get_preauthenticate_response(
                    &response_data,
                    item.router_data.request.continue_redirection_url.as_ref(),
                    item.router_data
                        .resource_common_data
                        .connector_feature_data
                        .clone(),
                    item.http_code,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationPending,
                        connector_feature_data: connector_meta_data,
                        reference_id: response_ref_id.clone(),
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                        redirection_data,
                        connector_response_reference_id: response_ref_id,
                        status_code: item.http_code,
                        authentication_data,
                    }),
                    ..item.router_data
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

// Authenticate

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysAuthenticateRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    T::Inner: Clone,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
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
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        let is_auto_capture = router_data.request.is_auto_capture()?;

        let ds_merchant_transactiontype = if is_auto_capture {
            requests::RedsysTransactionType::Payment
        } else {
            requests::RedsysTransactionType::Preauthorization
        };

        let card_data = requests::RedsysCardData::try_from(
            &item.router_data.request.payment_method_data.clone(),
        )?;

        let ds_merchant_order = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let auth_data = router_data.request.authentication_data.as_ref();

        let three_d_s_server_trans_i_d = auth_data
            .and_then(|auth| auth.threeds_server_transaction_id.clone())
            .or_else(|| router_data.resource_common_data.reference_id.clone())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.threeds_server_transaction_id",
                context: Default::default(),
            })?;

        let message_version = auth_data
            .and_then(|auth| auth.message_version.as_ref().map(|v| v.to_string()))
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.message_version",
                context: Default::default(),
            })?;

        let emv3ds_data = requests::RedsysEmvThreeDsRequestData::new(
            requests::RedsysThreeDsInfo::AuthenticationData,
        )
        .set_three_d_s_server_trans_i_d(three_d_s_server_trans_i_d)
        .set_protocol_version(message_version)
        .set_notification_u_r_l(router_data.request.get_continue_redirection_url()?)
        .add_browser_data(router_data.request.get_browser_info()?)?
        .set_three_d_s_comp_ind(requests::RedsysThreeDSCompInd::N)
        .set_billing_data(router_data.resource_common_data.get_optional_billing())?
        .set_shipping_data(router_data.resource_common_data.get_optional_shipping())?;

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(
                router_data.request.amount,
                router_data
                    .request
                    .currency
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "currency",
                        context: Default::default(),
                    })?,
            )?,
            ds_merchant_currency: router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?
                .iso_4217()
                .to_owned(),
            ds_merchant_cvv2: card_data.cvv2,
            ds_merchant_emv3ds: Some(emv3ds_data),
            ds_merchant_expirydate: card_data.expiry_date,
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })
                .attach_printable("Invalid card number")?,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_excep_sca: None,
            ds_merchant_directpayment: None,
        };

        let transaction = Self::try_from((&payment_request, &auth))?;
        Ok(transaction)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysPaymentsResponse = to_connector_response_data(
                    &transaction.ds_merchant_parameters.clone().expose(),
                    item.http_code,
                )?;

                let auth_data = item.router_data.request.authentication_data.clone();
                let is_three_ds = item.router_data.resource_common_data.is_three_ds();

                let (authenticate_response, status, ds_order) = get_payments_response(
                    response_data,
                    item.router_data.request.capture_method,
                    auth_data,
                    is_three_ds,
                    item.http_code,
                    false,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_feature_data: item
                            .router_data
                            .resource_common_data
                            .connector_feature_data,
                        reference_id: Some(ds_order),

                        ..item.router_data.resource_common_data
                    },
                    response: authenticate_response,
                    ..item.router_data
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

// Authorize

fn determine_exemption<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<requests::RedsysStrongCustomerAuthenticationException, Error> {
    let request = &router_data.request;
    // 1. Explicit exemption requested
    if let Some(indicator) = request
        .authentication_data
        .as_ref()
        .and_then(|auth| auth.exemption_indicator.as_ref())
    {
        return Ok(map_exemption_indicator(
            indicator,
            request.amount <= *LWV_THRESHOLD,
        ));
    }
    // 2. Auto-detect: MIT for stored credential payments
    let is_connector_mandate = request.connector_mandate_id().is_some();
    let is_off_session = request.off_session.unwrap_or(false);
    let is_setup_future = request.setup_future_usage == Some(common_enums::FutureUsage::OffSession);
    if is_connector_mandate || (is_off_session && !is_setup_future) {
        return Ok(requests::RedsysStrongCustomerAuthenticationException::Mit);
    }
    // 3. First payment in recurring series
    if is_setup_future {
        return Ok(requests::RedsysStrongCustomerAuthenticationException::Tra);
    }
    // 4. Default: amount-based
    // For Redsys, both LWV and TRA are capped at €30
    if request.amount <= *LWV_THRESHOLD {
        Ok(requests::RedsysStrongCustomerAuthenticationException::Lwv)
    } else {
        Ok(requests::RedsysStrongCustomerAuthenticationException::Tra)
    }
}
fn map_exemption_indicator(
    indicator: &common_enums::ExemptionIndicator,
    is_low_value: bool,
) -> requests::RedsysStrongCustomerAuthenticationException {
    match indicator {
        common_enums::ExemptionIndicator::LowValue => {
            requests::RedsysStrongCustomerAuthenticationException::Lwv
        }
        common_enums::ExemptionIndicator::SecureCorporatePayment => {
            requests::RedsysStrongCustomerAuthenticationException::Cor
        }
        common_enums::ExemptionIndicator::ScaDelegation => {
            requests::RedsysStrongCustomerAuthenticationException::Atd
        }
        common_enums::ExemptionIndicator::TransactionRiskAssessment => {
            requests::RedsysStrongCustomerAuthenticationException::Tra
        }
        common_enums::ExemptionIndicator::RecurringOperation => {
            requests::RedsysStrongCustomerAuthenticationException::Mit
        }
        // Unmapped: fall back to amount-based
        _ if is_low_value => requests::RedsysStrongCustomerAuthenticationException::Lwv,
        _ => requests::RedsysStrongCustomerAuthenticationException::Tra,
    }
}

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysAuthorizeRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
    T::Inner: Clone,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
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

        let card_data = requests::RedsysCardData::try_from(&Some(
            item.router_data.request.payment_method_data.clone(),
        ))?;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        let billing_data = router_data.resource_common_data.get_optional_billing();
        let shipping_data = router_data.resource_common_data.get_optional_shipping();

        let (ds_merchant_excep_sca, ds_merchant_directpayment, ds_merchant_emv3ds) =
                    // Redsys does not really support no 3ds flow. The exemptions requested in the requests in which the
                    // EMV3DS data have not been reported, will be marked in the authorization. If this exemption is not
                    // accepted by the issuer, a denial will be made with Ds_Response = 0195 ("soft-decline" requires SCA).
                    // In this case, the merchant can decide to start the operation again with EMV3DS data (3DS transaction),
                    // but a new request must be sent.
                    if !item.router_data.resource_common_data.is_three_ds() {
                        let exemption = determine_exemption(router_data)?;

                        (Some(exemption), Some(true), None)
                    } else {
                        // Get authentication data from the request
                        let auth_data = router_data.request.authentication_data.as_ref().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "authentication_data",
                                context: Default::default(),
                            },
                        )?;

                        let three_d_s_server_trans_i_d = auth_data
                            .threeds_server_transaction_id
                            .clone()
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "authentication_data.threeds_server_transaction_id",
                                context: Default::default(),
                            })?;
                        let message_version = auth_data
                                            .message_version
                                            .as_ref()
                                            .map(|v| v.to_string())
                                            .ok_or(IntegrationError::MissingRequiredField {
                                                field_name: "authentication_data.message_version",
                                                context: Default::default(),
                                            })?;

                                        // Determine if this is invoke case based on threeds_completion_indicator:
                                        // - Success/Failure means 3DS method was invoked (invoke case)
                                        // - NotAvailable means no 3DS method URL was present (exempt case)
                                        let threeds_completion_indicator =
                                            router_data.request.threeds_method_comp_ind.clone();

                                        let redirect_response = router_data.request.redirect_response.as_ref().ok_or(
                                            IntegrationError::MissingRequiredField {
                                                field_name: "redirect_response",
                                                context: Default::default(),
                                            },
                                        )?;

                                        let redirect_payload_value: Option<responses::RedsysThreedsChallengeResponse> =
                                            redirect_response.payload.as_ref().and_then(|secret| {
                                                let payload_data = secret.peek();
                                                serde_json::from_value::<responses::RedsysThreedsChallengeResponse>(
                                                                            payload_data.clone(),
                                                                        )
                                                                        .ok()
                                                                    });

                                                                let emv3ds_data = match redirect_payload_value {
                                                                    Some(payload) => requests::RedsysEmvThreeDsRequestData::new(
                                                                        requests::RedsysThreeDsInfo::ChallengeResponse,
                                                                    )
                                                                    .set_protocol_version(message_version)
                                                                    .set_three_d_s_cres(payload.cres)
                                                                    .set_billing_data(billing_data)?
                                                                    .set_shipping_data(shipping_data)?,
                                                                    None => match threeds_completion_indicator {
                                                                        Some(comp_ind) => {
                                                                                                    let three_d_s_comp_ind = requests::RedsysThreeDSCompInd::from(comp_ind);
                                                                                                    let browser_info = router_data.request.browser_info.clone().ok_or(
                                                                                                        IntegrationError::MissingRequiredField {
                                                                                                            field_name: "browser_info",
                                                                                                            context: Default::default(),
                                                                                                        },
                                                                                                    )?;
                                                                                                    let continue_redirection_url = router_data
                                                                                                        .request
                                                                                                        .continue_redirection_url
                                                                                                        .as_ref()
                                                                                                        .ok_or(IntegrationError::MissingRequiredField {
                                                                                                            field_name: "continue_redirection_url",
                                                                                                            context: Default::default(),
                                                                                                        })?;

                                                                                                    requests::RedsysEmvThreeDsRequestData::new(
                                                                                                        requests::RedsysThreeDsInfo::AuthenticationData,
                                                                                                    )
                                                                                                    .set_three_d_s_server_trans_i_d(three_d_s_server_trans_i_d)
                                                                                                    .set_protocol_version(message_version)
                                                                                                    .set_three_d_s_comp_ind(three_d_s_comp_ind)
                                                                                                    .add_browser_data(browser_info)?
                                                                                                    .set_notification_u_r_l(continue_redirection_url.clone())
                                                                                                    .set_billing_data(billing_data)?
                                                                                                    .set_shipping_data(shipping_data)?
                                                                                                }
                                                                                                None => {
                                                                                                    return Err(IntegrationError::MissingRequiredField {
                                                                                                        field_name: "threeds_completion_indicator",
                                                                                                        context: Default::default(),
                                                                                                    })?;
                                                                                                }
                                                                                            },
                                                                                        };

                                                                                        (None, None, Some(emv3ds_data))
                                                                                    };

        let is_auto_capture = router_data.request.is_auto_capture();
        let ds_merchant_transactiontype = if is_auto_capture {
            requests::RedsysTransactionType::Payment
        } else {
            requests::RedsysTransactionType::Preauthorization
        };

        let ds_merchant_order = get_ds_merchant_order(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            router_data.request.metadata.as_ref(),
        )?;

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(
                router_data.request.amount,
                router_data.request.currency,
            )?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_cvv2: card_data.cvv2,
            ds_merchant_emv3ds,
            ds_merchant_expirydate: card_data.expiry_date,
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })
                .attach_printable("Invalid card number")?,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_directpayment,
            ds_merchant_excep_sca,
        };

        let transaction = Self::try_from((&payment_request, &auth))?;
        Ok(transaction)
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysPaymentsResponse = to_connector_response_data(
                    &transaction.ds_merchant_parameters.clone().expose(),
                    item.http_code,
                )?;

                let auth_data = item.router_data.request.authentication_data.clone();
                let is_three_ds = item.router_data.resource_common_data.is_three_ds();

                let (authenticate_response, status, ds_order) = get_payments_response(
                    response_data,
                    item.router_data.request.capture_method,
                    auth_data,
                    is_three_ds,
                    item.http_code,
                    true,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_feature_data: item
                            .router_data
                            .resource_common_data
                            .connector_feature_data,
                        reference_id: Some(ds_order),

                        ..item.router_data.resource_common_data
                    },
                    response: authenticate_response,
                    ..item.router_data
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

// Capture

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::RedsysCaptureRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = match &router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(id) => Ok(id.clone()),
            _ => Err(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }),
        }?;

        let amount_to_capture =
            common_utils::types::MinorUnit::new(router_data.request.amount_to_capture);

        let capture_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(
                amount_to_capture,
                router_data.request.currency,
            )?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order: connector_transaction_id,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype: requests::RedsysTransactionType::Confirmation,
        };

        let transaction = Self::try_from((&capture_request, &auth))?;
        Ok(transaction)
    }
}

impl TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysOperationsResponse =
                    to_connector_response_data(
                        &transaction.ds_merchant_parameters.clone().expose(),
                        item.http_code,
                    )?;

                let attempt_status = get_redsys_attempt_status(
                    response_data.ds_response.clone(),
                    item.router_data.request.capture_method,
                    false,
                    item.http_code,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response_data.ds_order.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(response_data.ds_order),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

// Void

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::RedsysVoidRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();
        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        let amount = router_data
            .request
            .amount
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: Default::default(),
            })?;

        let void_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(amount, currency)?,
            ds_merchant_currency: currency.iso_4217().to_owned(),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order: connector_transaction_id,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype: requests::RedsysTransactionType::Cancellation,
        };

        let transaction = Self::try_from((&void_request, &auth))?;
        Ok(transaction)
    }
}

impl TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysOperationsResponse =
                    to_connector_response_data(
                        &transaction.ds_merchant_parameters.clone().expose(),
                        item.http_code,
                    )?;

                let attempt_status = get_redsys_attempt_status(
                    response_data.ds_response.clone(),
                    None,
                    false,
                    item.http_code,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: attempt_status,
                        ..item.router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::TransactionResponse {
                        resource_id: ResponseId::ConnectorTransactionId(
                            response_data.ds_order.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(response_data.ds_order),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                    }),
                    ..item.router_data
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}

// PSync

fn find_latest_response(
    responses: Vec<responses::RedsysSyncResponseData>,
) -> Option<responses::RedsysSyncResponseData> {
    responses
        .into_iter()
        .filter(|response_data| response_data.ds_date.is_some() && response_data.ds_hour.is_some())
        .max_by(|current_response_data, next_response_data| {
            match current_response_data
                .ds_date
                .cmp(&next_response_data.ds_date)
            {
                std::cmp::Ordering::Equal => match current_response_data
                    .ds_hour
                    .cmp(&next_response_data.ds_hour)
                {
                    std::cmp::Ordering::Equal => {
                        // Higher transaction type number wins i.e., later operations (like refunds) override earlier ones
                        current_response_data
                            .ds_transactiontype
                            .cmp(&next_response_data.ds_transactiontype)
                    }
                    other => other,
                },
                other => other,
            }
        })
}

pub fn construct_sync_request(
    order_id: String,
    transaction_type: Option<String>,
    auth: RedsysAuthType,
) -> Result<Vec<u8>, Error> {
    let sync_message = if transaction_type.is_some() {
        requests::Message {
            content: requests::MessageContent::Transaction(requests::RedsysTransactionRequest {
                ds_merchant_code: auth.merchant_id,
                ds_terminal: auth.terminal_id,
                ds_order: order_id.clone(),
                ds_transaction_type: transaction_type.ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "transaction_type",
                        context: Default::default(),
                    },
                )?,
            }),
        }
    } else {
        requests::Message {
            content: requests::MessageContent::Monitor(requests::RedsysMonitorRequest {
                ds_merchant_code: auth.merchant_id,
                ds_terminal: auth.terminal_id,
                ds_order: order_id.clone(),
            }),
        }
    };

    let version = requests::RedsysVersionData {
        ds_version: DS_VERSION.to_owned(),
        message: sync_message,
    };

    let version_data = quick_xml::se::to_string(&version).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

    let signature = get_signature(&order_id, &version_data, auth.sha256_pwd.peek())?;

    let messages = requests::Messages {
        version,
        signature,
        signature_version: SIGNATURE_VERSION.to_owned(),
    };

    let cdata = quick_xml::se::to_string(&messages).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?;

    let body = format!(
        r#"<soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" xmlns:web="{}"><soapenv:Header/><soapenv:Body><web:consultaOperaciones><cadenaXML><![CDATA[{}]]></cadenaXML></web:consultaOperaciones></soapenv:Body></soapenv:Envelope>"#,
        XMLNS_WEB_URL, cdata
    );

    Ok(body.as_bytes().to_vec())
}

impl TryFrom<ResponseRouterData<responses::RedsysSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let message_data = item
            .response
            .body
            .consultaoperacionesresponse
            .consultaoperacionesreturn
            .messages
            .version
            .message;

        let (status, response) = match (message_data.response, message_data.errormsg) {
            (Some(responses), None) => {
                if let Some(latest_response) = find_latest_response(responses) {
                    if let Some(ds_response) = latest_response.ds_response {
                        let attempt_status = get_redsys_attempt_status(
                            ds_response.clone(),
                            item.router_data.request.capture_method,
                            false,
                            item.http_code,
                        )?;
                        let payment_response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                latest_response.ds_order.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(latest_response.ds_order.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        });
                        (attempt_status, payment_response)
                    } else {
                        // No ds_response - check Ds_State for status mapping
                        let status = match latest_response.ds_state {
                            Some(responses::DsState::A) => {
                                // Authenticating - customer needs to complete 3DS
                                common_enums::AttemptStatus::AuthenticationPending
                            }
                            Some(responses::DsState::P) => common_enums::AttemptStatus::Pending, // Authorizing - payment in progress
                            Some(responses::DsState::S) => common_enums::AttemptStatus::Pending, // Requested - initial state
                            Some(responses::DsState::F) => {
                                // Completed - check capture method for final status
                                match item.router_data.request.capture_method {
                                    Some(enums::CaptureMethod::Automatic) | None => {
                                        common_enums::AttemptStatus::Charged
                                    }
                                    Some(enums::CaptureMethod::Manual) => {
                                        common_enums::AttemptStatus::Authorized
                                    }
                                    _ => common_enums::AttemptStatus::Pending,
                                }
                            }
                            _ => item.router_data.resource_common_data.status, // Fallback to existing status if Ds_State is unknown/missing
                        };

                        let payment_response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                latest_response.ds_order.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(latest_response.ds_order.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                        });
                        (status, payment_response)
                    }
                } else {
                    // NEW: No valid responses found
                    let error_response = Err(domain_types::router_data::ErrorResponse {
                        code: "NO_VALID_RESPONSES".to_string(),
                        message: "No valid responses found in Monitor query".to_string(),
                        reason: Some(
                            "Monitor query returned no responses with valid date/hour".to_string(),
                        ),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    });
                    (item.router_data.resource_common_data.status, error_response)
                }
            }
            (None, Some(errormsg)) => {
                let error_code = errormsg.ds_errorcode.clone();
                let response = Err(domain_types::router_data::ErrorResponse {
                    code: error_code.clone(),
                    message: error_code.clone(),
                    reason: Some(error_code),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                });
                (item.router_data.resource_common_data.status, response)
            }
            (Some(_), Some(_)) | (None, None) => Err(utils::response_handling_fail_for_connector(
                item.http_code,
                "redsys",
            ))?,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// Refund

impl<T>
    TryFrom<
        RedsysRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for requests::RedsysRefundRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;
        let refund_amount = common_utils::types::MinorUnit::new(router_data.request.refund_amount);

        let refund_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(
                refund_amount,
                router_data.request.currency,
            )?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order: router_data.request.connector_transaction_id.clone(),
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype: requests::RedsysTransactionType::Refund,
        };

        let transaction = Self::try_from((&refund_request, &auth))?;
        Ok(transaction)
    }
}

impl TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let response_data: responses::RedsysOperationsResponse =
                    to_connector_response_data(
                        &transaction.ds_merchant_parameters.clone().expose(),
                        item.http_code,
                    )?;

                let refund_status = refund_status_from_ds_response(
                    response_data.ds_response.clone(),
                    item.http_code,
                )?;

                Ok(RefundsResponseData {
                    connector_refund_id: response_data.ds_order,
                    refund_status,
                    status_code: item.http_code,
                })
            }
            responses::RedsysResponse::RedsysErrorResponse(ref err) => {
                Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err.error_code_description.clone(),
                    reason: Some(err.error_code_description.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// RSync

impl TryFrom<ResponseRouterData<responses::RedsysSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let message_data = item
            .response
            .body
            .consultaoperacionesresponse
            .consultaoperacionesreturn
            .messages
            .version
            .message;

        let response = match (message_data.response, message_data.errormsg) {
            (Some(responses), None) => {
                // NEW: Use latest response for consistency (even for RSync)
                if let Some(latest_response) = find_latest_response(responses) {
                    if let Some(ds_response) = latest_response.ds_response {
                        let refund_status =
                            refund_status_from_ds_response(ds_response.clone(), item.http_code)?;
                        Ok(RefundsResponseData {
                            connector_refund_id: latest_response.ds_order,
                            refund_status,
                            status_code: item.http_code,
                        })
                    } else {
                        Ok(RefundsResponseData {
                            connector_refund_id: latest_response.ds_order,
                            refund_status: common_enums::RefundStatus::Pending,
                            status_code: item.http_code,
                        })
                    }
                } else {
                    Err(domain_types::router_data::ErrorResponse {
                        code: "NO_VALID_RESPONSES".to_string(),
                        message: "No valid responses found in Monitor/Transaction query"
                            .to_string(),
                        reason: Some(
                            "Query returned no responses with valid date/hour".to_string(),
                        ),
                        status_code: item.http_code,
                        attempt_status: None,
                        connector_transaction_id: None,
                        network_decline_code: None,
                        network_advice_code: None,
                        network_error_message: None,
                    })
                }
            }
            (None, Some(errormsg)) => {
                let error_code = errormsg.ds_errorcode.clone();
                Err(domain_types::router_data::ErrorResponse {
                    code: error_code.clone(),
                    message: error_code.clone(),
                    reason: Some(error_code),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            (Some(_), Some(_)) | (None, None) => Err(utils::response_handling_fail_for_connector(
                item.http_code,
                "redsys",
            ))?,
        };

        Ok(Self {
            response,
            ..item.router_data
        })
    }
}

// ---- ClientAuthenticationToken flow types ----

/// Request transformer for ClientAuthenticationToken flow.
/// Builds a RedsysTransaction containing signed merchant parameters
/// for client-side InSite SDK initialization.
/// Uses RedsysOperationRequest (no card data needed) since the
/// client SDK will collect card details directly.
impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysClientAuthRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
            RouterDataV2<
                ClientAuthenticationToken,
                PaymentFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        let amount = RedsysAmountConvertor::convert(
            router_data.request.amount,
            router_data.request.currency,
        )?;

        let connector_request_reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let ds_merchant_order = if connector_request_reference_id.len() <= 12 {
            Ok(connector_request_reference_id)
        } else {
            Err(IntegrationError::MaxFieldLengthViolated {
                connector: "Redsys".to_string(),
                field_name: "ds_merchant_order".to_string(),
                max_length: 12,
                received_length: connector_request_reference_id.len(),
                context: Default::default(),
            })
        }?;

        let operation_request = requests::RedsysOperationRequest {
            ds_merchant_amount: amount,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype: requests::RedsysTransactionType::Payment,
        };

        let transaction = Self::try_from((&operation_request, &auth))?;
        Ok(transaction)
    }
}

/// Response transformer for ClientAuthenticationToken flow.
/// Extracts the signed merchant parameters from the Redsys response
/// and returns them as SDK initialization data for the InSite JS SDK.
impl TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    >
{
    type Error = ResponseError;

    fn try_from(
        item: ResponseRouterData<responses::RedsysResponse, Self>,
    ) -> Result<Self, Self::Error> {
        match item.response {
            responses::RedsysResponse::RedsysResponse(ref transaction) => {
                let session_data = ClientAuthenticationTokenData::ConnectorSpecific(Box::new(
                    ConnectorSpecificClientAuthenticationResponse::Redsys(
                        RedsysClientAuthenticationResponseDomain {
                            merchant_parameters: transaction.ds_merchant_parameters.clone(),
                            signature: transaction.ds_signature.clone(),
                            signature_version: transaction.ds_signature_version.clone(),
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
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                response: Err(domain_types::router_data::ErrorResponse {
                    status_code: item.http_code,
                    code: err.error_code.clone(),
                    message: err.error_code.clone(),
                    reason: Some(err.error_code_description.clone()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            }),
        }
    }
}
