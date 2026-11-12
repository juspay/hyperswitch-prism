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
    crypto::{self, EncodeMessage, SignMessage, VerifySignature},
    ext_traits::Encode,
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        self, PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{
        ApplePayPaymentData, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes,
        WalletData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use super::{requests, responses};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};

// Kept at HMAC_SHA256_V1 for REST and SOAP on purpose: it moves live money and is accepted in
// sandbox and production. Sandbox probe (the run's sandbox terminal) accepts HMAC_SHA256_V1 and
// HMAC_SHA512_V1 and rejects HMAC_SHA512_V2 (SIS0430). The REST manual v4.0.1 Annex 2 documents
// HMAC_SHA512_V1 without a sunset date for SHA256; the SOAP Consulta manual (PSync/RSync) is
// SHA256-only.
pub const SIGNATURE_VERSION: &str = "HMAC_SHA256_V1";
pub const DS_VERSION: &str = "0.0";
pub const XMLNS_WEB_URL: &str = "http://webservices.apl02.redsys.es";
pub const REDSYS_SOAP_ACTION: &str = "consultaOperaciones";
pub const REDSYS_ORDER_ID_METADATA_KEY: &str = "order_id";
pub const REDSYS_ORDER_ID_MAX_LENGTH: usize = 12;
pub const REDSYS_ORDER_ID_MIN_LENGTH: usize = 4;
const REDSYS_MERCHANT_URL_MAX_LENGTH: usize = 250;
const REDSYS_MERCHANT_DESCRIPTOR_MAX_LENGTH: usize = 25;
/// DS_MERCHANT_IDENTIFIER value asking Redsys to generate a card-on-file reference
const REDSYS_COF_IDENTIFIER_REQUIRED: &str = "REQUIRED";

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
        .filter(|s| is_valid_redsys_order(s))
}

/// SIS0075: Ds_Merchant_Order must be 4-12 alphanumeric characters, the first 4 numeric
fn is_valid_redsys_order(order: &str) -> bool {
    (REDSYS_ORDER_ID_MIN_LENGTH..=REDSYS_ORDER_ID_MAX_LENGTH).contains(&order.len())
        && order.bytes().all(|b| b.is_ascii_alphanumeric())
        && order.bytes().take(4).all(|b| b.is_ascii_digit())
}

/// Merchant reference id -> Ds_Merchant_Order, shared by every builder that sends the merchant
/// reference as the Redsys order (Authorize card/wallet, the 3DS legs, SetupMandate, RepeatPayment).
///
/// 1. `merchant_transaction_id` already valid for Redsys (4-12 alphanumeric, first 4 digits) is sent
///    unchanged.
/// 2. else a valid `metadata.order_id` is sent unchanged.
/// 3. else a non-empty `merchant_transaction_id` is mapped deterministically to a 12-digit order:
///    the first 8 bytes of SHA-256(reference) as a big-endian u64, mod 10^12, zero-padded. The same
///    reference always yields the same order, so a retry hits the same Redsys order.
/// 4. an empty reference with no valid `metadata.order_id` is refused.
///
/// The order sent is echoed back as Ds_Order and becomes connector_transaction_id, so Capture,
/// Void, Refund, PSync and RSync keep addressing the right order.
fn get_ds_merchant_order(
    connector_request_reference_id: String,
    metadata: Option<&Secret<serde_json::Value>>,
) -> Result<String, Error> {
    if is_valid_redsys_order(&connector_request_reference_id) {
        return Ok(connector_request_reference_id);
    }
    if let Some(order_id) = get_redsys_order_id_from_metadata(metadata) {
        return Ok(order_id);
    }
    if connector_request_reference_id.is_empty() {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "ds_merchant_order",
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Send a non-empty merchant_transaction_id (or metadata.{REDSYS_ORDER_ID_METADATA_KEY})"
                )),
                doc_url: None,
                additional_context: Some(
                    "Redsys Ds_Merchant_Order is mandatory (SIS0075); no merchant reference to send or derive it from"
                        .to_string(),
                ),
            },
        }
        .into());
    }
    Ok(derive_redsys_order(&connector_request_reference_id))
}

fn derive_redsys_order(reference: &str) -> String {
    use sha2::{Digest, Sha256};
    // first 8 digest bytes as a big-endian u64
    let prefix = Sha256::digest(reference.as_bytes())
        .iter()
        .take(8)
        .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
    format!("{:012}", prefix % 1_000_000_000_000)
}

pub(super) fn redsys_error_context(
    suggested_action: &str,
    additional_context: &str,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_string()),
        doc_url: None,
        additional_context: Some(additional_context.to_string()),
    }
}

fn check_max_length(field_name: &str, value: String, max_length: usize) -> Result<String, Error> {
    let received_length = value.chars().count();
    if received_length > max_length {
        return Err(IntegrationError::MaxFieldLengthViolated {
            connector: "Redsys".to_string(),
            field_name: field_name.to_string(),
            max_length,
            received_length,
            context: IntegrationErrorContext {
                suggested_action: Some(format!(
                    "Shorten {field_name} to at most {max_length} characters"
                )),
                doc_url: None,
                additional_context: Some("Redsys parameters table length limit".to_string()),
            },
        }
        .into());
    }
    Ok(value)
}

/// webhook_url -> DS_MERCHANT_MERCHANTURL (max 250)
fn build_merchant_url(webhook_url: Option<&String>) -> Result<Option<String>, Error> {
    webhook_url
        .map(|url| check_max_length("webhook_url", url.clone(), REDSYS_MERCHANT_URL_MAX_LENGTH))
        .transpose()
}

/// billing_descriptor.statement_descriptor -> DS_MERCHANT_MERCHANTDESCRIPTOR (max 25)
fn build_merchant_descriptor(
    billing_descriptor: Option<&connector_types::BillingDescriptor>,
) -> Result<Option<String>, Error> {
    billing_descriptor
        .and_then(|descriptor| descriptor.statement_descriptor.clone())
        .map(|descriptor| {
            check_max_length(
                "statement_descriptor",
                descriptor,
                REDSYS_MERCHANT_DESCRIPTOR_MAX_LENGTH,
            )
        })
        .transpose()
}

/// mit_category -> DS_MERCHANT_COF_TYPE; absent -> C (documented default "other")
fn cof_type_from_mit_category(
    mit_category: Option<&common_enums::MitCategory>,
) -> requests::RedsysCofType {
    match mit_category {
        Some(common_enums::MitCategory::Installment) => requests::RedsysCofType::I,
        Some(common_enums::MitCategory::Recurring)
        | Some(common_enums::MitCategory::Subscription) => requests::RedsysCofType::R,
        Some(common_enums::MitCategory::Resubmission) => requests::RedsysCofType::E,
        Some(common_enums::MitCategory::Unscheduled) | None => requests::RedsysCofType::C,
    }
}

/// capture_method -> DS_MERCHANT_TRANSACTIONTYPE: 0 payment (auto capture), 1 preauthorization.
/// A preauthorization is confirmed once (type 2), so multiple/scheduled capture is refused.
fn transaction_type_from_capture_method(
    capture_method: Option<enums::CaptureMethod>,
) -> Result<requests::RedsysTransactionType, Error> {
    match capture_method {
        Some(enums::CaptureMethod::Automatic)
        | Some(enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(requests::RedsysTransactionType::Payment),
        Some(enums::CaptureMethod::Manual) => Ok(requests::RedsysTransactionType::Preauthorization),
        Some(capture_method @ enums::CaptureMethod::ManualMultiple)
        | Some(capture_method @ enums::CaptureMethod::Scheduled) => {
            Err(IntegrationError::NotSupported {
                message: format!("capture_method {capture_method}"),
                connector: "redsys",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method AUTOMATIC or MANUAL".to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Redsys confirms a preauthorization (type 2) once; multiple capture is not supported"
                            .to_string(),
                    ),
                },
            }
            .into())
        }
    }
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
                        .as_ref()
                        .and_then(|state| state.code_or_name())
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
                        .as_ref()
                        .and_then(|state| state.code_or_name())
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
            | Some(PaymentMethodData::GiftCard(..))
            | Some(PaymentMethodData::Voucher(..))
            | Some(PaymentMethodData::CardRedirect(..))
            | Some(PaymentMethodData::Reward)
            | Some(PaymentMethodData::RealTimePayment(..))
            | Some(PaymentMethodData::MobilePayment(..))
            | Some(PaymentMethodData::Upi(..))
            | Some(PaymentMethodData::OpenBanking(_))
            | Some(PaymentMethodData::PaymentMethodToken(..))
            | Some(PaymentMethodData::NetworkToken(..))
            | Some(PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_))
            | Some(PaymentMethodData::CardWithNoCvc(_))
            | None => Err(IntegrationError::NotImplemented(
                domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
                Default::default(),
            )
            .into()),
            Some(PaymentMethodData::MandatePayment)
            | Some(PaymentMethodData::CardDetailsForNetworkTransactionId(_)) => {
                Err(IntegrationError::NotImplemented(
                    domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
                    IntegrationErrorContext {
                        suggested_action: Some(
                            "Use RecurringPaymentService/Charge for merchant-initiated payments"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Redsys MIT runs through RepeatPayment".to_string(),
                        ),
                    },
                )
                .into())
            }
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

/// 0000-0099 (except 0002, a decline per HS parity) are authorised payments (spec "### Ds_Response ranges")
fn is_redsys_authorised(ds_response: &responses::DsResponse) -> bool {
    ds_response.0.starts_with("00") && ds_response.0.as_str() != "0002"
}

/// Maps Ds_Response to an attempt status (spec "## Status Mappings"; HS
/// redsys/transformers.rs:1065-1106). An undocumented code returns `None` so the caller keeps the
/// current status instead of erroring (TH-07).
fn get_redsys_attempt_status(
    ds_response: &responses::DsResponse,
    capture_method: Option<enums::CaptureMethod>,
) -> Option<common_enums::AttemptStatus> {
    if is_redsys_authorised(ds_response) {
        return match capture_method {
            Some(enums::CaptureMethod::Automatic)
            | Some(enums::CaptureMethod::SequentialAutomatic)
            | None => Some(common_enums::AttemptStatus::Charged),
            Some(enums::CaptureMethod::Manual) => Some(common_enums::AttemptStatus::Authorized),
            // refused before any request is sent (transaction_type_from_capture_method)
            Some(enums::CaptureMethod::ManualMultiple) | Some(enums::CaptureMethod::Scheduled) => {
                None
            }
        };
    }
    match ds_response.0.as_str() {
        // spec: confirmation authorised
        "0900" => Some(common_enums::AttemptStatus::Charged),
        // spec: cancellation accepted / previously cancelled
        "0400" | "0481" | "0940" | "9915" => Some(common_enums::AttemptStatus::Voided),
        // HS redsys/transformers.rs:1081
        "0950" => Some(common_enums::AttemptStatus::VoidFailed),
        // spec + HS: awaiting authentication / SCA required (0195 soft decline) on every path
        "0195" | "0112" | "8210" | "8220" | "9998" | "9999" => {
            Some(common_enums::AttemptStatus::AuthenticationPending)
        }
        // HS redsys/transformers.rs:1085 (plan UD-11: 0129 kept AuthenticationFailed)
        "0129" | "0184" | "9256" | "9257" => {
            Some(common_enums::AttemptStatus::AuthenticationFailed)
        }
        // spec: contact issuer / update completed
        "0107" | "0300" => Some(common_enums::AttemptStatus::Pending),
        // spec: declines
        "0101" | "0102" | "0104" | "0106" | "0110" | "0114" | "0115" | "0116" | "0117" | "0118"
        | "0121" | "0123" | "0125" | "0126" | "0130" | "0162" | "0163" | "0171" | "0172"
        | "0173" | "0174" | "0180" | "0181" | "0182" | "0187" | "0190" | "0191" | "0193"
        | "0201" | "0202" | "0204" | "0206" | "0290" | "0881" | "0904" | "0909" | "0912"
        | "0913" | "0941" | "0944" | "0945" | "0965" | "9912" | "9064" | "9078" | "9093"
        | "9094" | "9104" | "9218" | "9253" | "9261" | "9997" | "0002" => {
            Some(common_enums::AttemptStatus::Failure)
        }
        // HS redsys/transformers.rs:1101-1106: undocumented code keeps the previous status
        _undocumented => None,
    }
}

/// Capture (type 2): a failed confirmation is CaptureFailed, not a failed payment (TH-06)
fn capture_status_from_ds_response(
    ds_response: &responses::DsResponse,
    current_status: common_enums::AttemptStatus,
) -> common_enums::AttemptStatus {
    match get_redsys_attempt_status(ds_response, Some(enums::CaptureMethod::Automatic)) {
        Some(status) if domain_types::utils::is_payment_failure(status) => {
            common_enums::AttemptStatus::CaptureFailed
        }
        Some(status) => status,
        None => current_status,
    }
}

/// Void (type 9): a declined cancellation is VoidFailed, not a failed payment (TH-06)
fn void_status_from_ds_response(
    ds_response: &responses::DsResponse,
    current_status: common_enums::AttemptStatus,
) -> common_enums::AttemptStatus {
    match get_redsys_attempt_status(ds_response, None) {
        Some(status) if domain_types::utils::is_payment_failure(status) => {
            common_enums::AttemptStatus::VoidFailed
        }
        Some(status) => status,
        None => current_status,
    }
}

/// Refund Ds_Response (spec "### Refund"); an undocumented code stays non-terminal (TH-07)
fn refund_status_from_ds_response(
    ds_response: &responses::DsResponse,
) -> common_enums::RefundStatus {
    match ds_response.0.as_str() {
        "0900" => common_enums::RefundStatus::Success,
        "9999" => common_enums::RefundStatus::Pending,
        "0950" | "0172" | "0174" => common_enums::RefundStatus::Failure,
        _undocumented => common_enums::RefundStatus::Pending,
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
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
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
    current_status: common_enums::AttemptStatus,
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
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    });

    let ds_order = redsys_payments_response.ds_order.clone();

    // Card-on-file reference and NTID, returned when DS_MERCHANT_IDENTIFIER=REQUIRED was sent
    let mandate_reference = redsys_payments_response
        .ds_merchant_identifier
        .as_ref()
        .map(|identifier| {
            Box::new(connector_types::MandateReference {
                connector_mandate_id: Some(identifier.peek().clone()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        });
    let network_txn_id = redsys_payments_response
        .ds_merchant_cof_txnid
        .as_ref()
        .map(|txn_id| txn_id.peek().clone());

    if let Some(ds_response) = redsys_payments_response.ds_response {
        let status =
            get_redsys_attempt_status(&ds_response, capture_method).unwrap_or(current_status);

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
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else if use_transaction_response {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                ),
                redirection_data: None,
                mandate_reference,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        } else {
            Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                )),
                redirection_data: None,
                authentication_data,
                connector_feature_data: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                status_code: http_code,
                mandate_reference,
                network_txn_id,
                network_txn_link_id: None,
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
                mandate_reference,
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        } else {
            Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id: Some(ResponseId::ConnectorTransactionId(
                    redsys_payments_response.ds_order.clone(),
                )),
                redirection_data: redirection_form.map(Box::new),
                authentication_data,
                connector_feature_data: None,
                connector_response_reference_id: Some(redsys_payments_response.ds_order.clone()),
                status_code: http_code,
                mandate_reference,
                network_txn_id,
                network_txn_link_id: None,
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
        let amount = RedsysAmountConvertor::convert(&router_data.request.money)?;

        let ds_merchant_emv3ds = Some(requests::RedsysEmvThreeDsRequestData::new(
            requests::RedsysThreeDsInfo::CardData,
        ));
        let ds_merchant_transactiontype =
            transaction_type_from_capture_method(router_data.request.capture_method)?;

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
            ds_merchant_cvv2: Some(card_data.cvv2),
            ds_merchant_emv3ds,
            ds_merchant_expirydate: Some(card_data.expiry_date),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: Some(
                cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: redsys_error_context(
                            "Send a valid card number",
                            "Redsys DS_MERCHANT_PAN",
                        ),
                    })
                    .attach_printable("Invalid card number")?,
            ),
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_excep_sca: None,
            ds_merchant_directpayment: None,
            ds_merchant_mpiexternal: None,
            ds_merchant_identifier: None,
            ds_merchant_cof_ini: None,
            ds_merchant_cof_type: None,
            ds_merchant_cof_txnid: None,
            ds_merchant_merchanturl: None,
            ds_merchant_merchantdescriptor: None,
            ds_xpaydata: None,
            ds_xpaydecodeddata: None,
            ds_xpaytype: None,
            ds_xpayorigen: None,
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
                        resource_id: response_ref_id
                            .clone()
                            .map(ResponseId::ConnectorTransactionId),
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
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: Some(
                        err.error_code_description
                            .clone()
                            .unwrap_or_else(|| err.error_code.clone()),
                    ),
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

        let ds_merchant_transactiontype =
            transaction_type_from_capture_method(router_data.request.capture_method)?;

        let card_data = requests::RedsysCardData::try_from(
            &item.router_data.request.payment_method_data.clone(),
        )?;

        let ds_merchant_order = get_ds_merchant_order(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            None,
        )?;

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
            ds_merchant_amount: RedsysAmountConvertor::convert(&router_data.request.money)?,
            ds_merchant_currency: router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?
                .iso_4217()
                .to_owned(),
            ds_merchant_cvv2: Some(card_data.cvv2),
            ds_merchant_emv3ds: Some(emv3ds_data),
            ds_merchant_expirydate: Some(card_data.expiry_date),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: Some(
                cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                    .change_context(IntegrationError::RequestEncodingFailed {
                        context: redsys_error_context(
                            "Send a valid card number",
                            "Redsys DS_MERCHANT_PAN",
                        ),
                    })
                    .attach_printable("Invalid card number")?,
            ),
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_excep_sca: None,
            ds_merchant_directpayment: None,
            ds_merchant_mpiexternal: None,
            ds_merchant_identifier: None,
            ds_merchant_cof_ini: None,
            ds_merchant_cof_type: None,
            ds_merchant_cof_txnid: None,
            ds_merchant_merchanturl: None,
            ds_merchant_merchantdescriptor: None,
            ds_xpaydata: None,
            ds_xpaydecodeddata: None,
            ds_xpaytype: None,
            ds_xpayorigen: None,
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
                let current_status = item.router_data.resource_common_data.status;

                let (authenticate_response, status, ds_order) = get_payments_response(
                    response_data,
                    item.router_data.request.capture_method,
                    auth_data,
                    current_status,
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
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: Some(
                        err.error_code_description
                            .clone()
                            .unwrap_or_else(|| err.error_code.clone()),
                    ),
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
            request.money.amount() <= *LWV_THRESHOLD,
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
    if request.money.amount() <= *LWV_THRESHOLD {
        Ok(requests::RedsysStrongCustomerAuthenticationException::Lwv)
    } else {
        Ok(requests::RedsysStrongCustomerAuthenticationException::Tra)
    }
}
fn missing_mpi_field(field_name: &'static str) -> IntegrationError {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Send cavv, eci, ds_trans_id, threeds_server_transaction_id and message_version for an externally authenticated payment"
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some(
                "Redsys DS_MERCHANT_MPIEXTERNAL requires every field (SIS0605-SIS0614)".to_string(),
            ),
        },
    }
}

/// External 3DS result -> DS_MERCHANT_MPIEXTERNAL; every field is mandatory
fn build_mpi_external(
    auth_data: &domain_types::router_request_types::AuthenticationData,
) -> Result<requests::RedsysMpiExternal, Error> {
    Ok(requests::RedsysMpiExternal {
        three_d_s_server_trans_i_d: auth_data.threeds_server_transaction_id.clone().ok_or(
            missing_mpi_field("authentication_data.threeds_server_transaction_id"),
        )?,
        ds_trans_i_d: auth_data
            .ds_trans_id
            .clone()
            .ok_or(missing_mpi_field("authentication_data.ds_trans_id"))?,
        authentication_value: auth_data
            .cavv
            .clone()
            .ok_or(missing_mpi_field("authentication_data.cavv"))?,
        protocol_version: auth_data
            .message_version
            .as_ref()
            .map(|version| version.to_string())
            .ok_or(missing_mpi_field("authentication_data.message_version"))?,
        eci: auth_data
            .eci
            .clone()
            .ok_or(missing_mpi_field("authentication_data.eci"))?,
    })
}

/// Redsys-hosted 3DS continuation: ChallengeResponse (cres) or AuthenticationData after the
/// 3DS method
fn build_authorize_emv3ds<T: PaymentMethodDataTypes>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    auth_data: &domain_types::router_request_types::AuthenticationData,
    billing_data: Option<&domain_types::payment_address::Address>,
    shipping_data: Option<&domain_types::payment_address::Address>,
) -> Result<requests::RedsysEmvThreeDsRequestData, Error> {
    let three_d_s_server_trans_i_d = auth_data.threeds_server_transaction_id.clone().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "authentication_data.threeds_server_transaction_id",
            context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
        },
    )?;
    let message_version = auth_data
        .message_version
        .as_ref()
        .map(|v| v.to_string())
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.message_version",
            context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
        })?;

    // Determine if this is invoke case based on threeds_completion_indicator:
    // - Success/Failure means 3DS method was invoked (invoke case)
    // - NotAvailable means no 3DS method URL was present (exempt case)
    let threeds_completion_indicator = router_data.request.threeds_method_comp_ind.clone();

    let redirect_response = router_data.request.redirect_response.as_ref().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "redirect_response",
            context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
        },
    )?;

    let redirect_payload_value: Option<responses::RedsysThreedsChallengeResponse> =
        redirect_response.payload.as_ref().and_then(|secret| {
            serde_json::from_value::<responses::RedsysThreedsChallengeResponse>(
                secret.peek().clone(),
            )
            .ok()
        });

    match redirect_payload_value {
        Some(payload) => requests::RedsysEmvThreeDsRequestData::new(
            requests::RedsysThreeDsInfo::ChallengeResponse,
        )
        .set_protocol_version(message_version)
        .set_three_d_s_cres(payload.cres)
        .set_billing_data(billing_data)?
        .set_shipping_data(shipping_data),
        None => match threeds_completion_indicator {
            Some(comp_ind) => {
                let three_d_s_comp_ind = requests::RedsysThreeDSCompInd::from(comp_ind);
                let browser_info = router_data.request.browser_info.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "browser_info",
                        context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
                    },
                )?;
                let continue_redirection_url = router_data
                    .request
                    .continue_redirection_url
                    .as_ref()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "continue_redirection_url",
                        context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
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
                .set_shipping_data(shipping_data)
            }
            None => Err(IntegrationError::MissingRequiredField {
                field_name: "threeds_completion_indicator",
                context: redsys_error_context("Complete the Redsys 3DS method/challenge leg before Authorize, or send the external 3DS result", "Redsys 3DS Authorize continuation"),
            }
            .into()),
        },
    }
}

/// Wallet (XPAY) part of an Authorize request; DS_XPAYDATA and DS_XPAYDECODEDDATA are
/// mutually exclusive (spec "Authorize/Wallet": "alternative to DS_XPAYDATA (not both)")
struct RedsysWalletFields {
    xpay_data: Option<Secret<String>>,
    xpay_decoded_data: Option<Secret<String>>,
    xpay_type: requests::RedsysXpayType,
    /// Google Pay CARD (PAN_ONLY) only: AuthenticationData without protocolVersion /
    /// threeDSServerTransID
    emv3ds: Option<requests::RedsysEmvThreeDsRequestData>,
}

fn wallet_error_context(additional_context: &str) -> IntegrationErrorContext {
    redsys_error_context(
        "Send the full Apple Pay / Google Pay payment data",
        additional_context,
    )
}

/// G-Authorize__Wallet-04: decrypted wallet expiry -> XPAYDECODEDDATA expirationDate (YYMM)
fn wallet_expiry_yymm(
    year: &Secret<String>,
    month: &Secret<String>,
    year_field: &'static str,
    month_field: &'static str,
) -> Result<Secret<String>, Error> {
    let year = year.peek().trim();
    let month = month.peek().trim();
    // Apple gives YYMMDD-derived years as 2 or 4 digits; anything shorter is unusable
    let year_2_digit = year
        .len()
        .checked_sub(2)
        .and_then(|start| year.get(start..))
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: year_field,
            context: wallet_error_context("Redsys DS_XPAYDECODEDDATA expirationDate (AAMM)"),
        })?;
    if month.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name: month_field,
            context: wallet_error_context("Redsys DS_XPAYDECODEDDATA expirationDate (AAMM)"),
        }
        .into());
    }
    Ok(Secret::new(format!("{year_2_digit}{month:0>2}")))
}

fn encode_xpay_decoded_data(
    decoded: &requests::RedsysXpayDecodedData,
) -> Result<Secret<String>, Error> {
    // UD-09: DS_XPAYDECODEDDATA is a JSON-encoded string, as in every published example
    serde_json::to_string(decoded)
        .map(Secret::new)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: wallet_error_context("Redsys DS_XPAYDECODEDDATA"),
        })
}

fn build_wallet_fields<T: PaymentMethodDataTypes>(
    wallet_data: &WalletData,
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<RedsysWalletFields, Error> {
    match wallet_data {
        WalletData::ApplePay(apple_pay) => match &apple_pay.payment_data {
            // Mode A (Redsys decrypts): DS_XPAYDATA = hex of the paymentData JSON (UD-13)
            ApplePayPaymentData::Encrypted(_) => {
                let payment_data = apple_pay.get_applepay_decoded_payment_data()?;
                Ok(RedsysWalletFields {
                    xpay_data: Some(Secret::new(hex::encode(payment_data.peek()))),
                    xpay_decoded_data: None,
                    xpay_type: requests::RedsysXpayType::Apple,
                    emv3ds: None,
                })
            }
            // Mode B (decrypted before Redsys): DS_XPAYDECODEDDATA
            ApplePayPaymentData::Decrypted(decrypted) => {
                let decoded = requests::RedsysXpayDecodedData {
                    token: decrypted.application_primary_account_number.clone(),
                    cryptogram: Some(decrypted.payment_data.online_payment_cryptogram.clone()),
                    eci_ind: decrypted.payment_data.eci_indicator.clone(),
                    expiration_date: wallet_expiry_yymm(
                        &decrypted.application_expiration_year,
                        &decrypted.application_expiration_month,
                        "application_expiration_year",
                        "application_expiration_month",
                    )?,
                    payment_method: None,
                };
                Ok(RedsysWalletFields {
                    xpay_data: None,
                    xpay_decoded_data: Some(encode_xpay_decoded_data(&decoded)?),
                    xpay_type: requests::RedsysXpayType::Apple,
                    emv3ds: None,
                })
            }
        },
        WalletData::GooglePay(google_pay) => match &google_pay.tokenization_data {
            // Mode A: DS_XPAYDATA = base64 of the whole paymentMethodData (Google Pay guide v1.7+)
            GpayTokenizationData::Encrypted(encrypted) => {
                let payment_method_data = requests::RedsysGooglePayPaymentMethodData {
                    pm_type: google_pay.pm_type.clone(),
                    description: google_pay.description.clone(),
                    info: requests::RedsysGooglePayInfo {
                        card_network: google_pay.info.card_network.clone(),
                        card_details: google_pay.info.card_details.clone(),
                        assurance_details: google_pay.info.assurance_details.as_ref().map(
                            |details| requests::RedsysGooglePayAssuranceDetails {
                                card_holder_authenticated: details.card_holder_authenticated,
                                account_verified: details.account_verified,
                            },
                        ),
                    },
                    tokenization_data: requests::RedsysGooglePayTokenizationData {
                        token_type: encrypted.token_type.clone(),
                        token: Secret::new(encrypted.token.clone()),
                    },
                };
                let json = serde_json::to_vec(&payment_method_data).change_context(
                    IntegrationError::RequestEncodingFailed {
                        context: wallet_error_context("Redsys DS_XPAYDATA (Google Pay)"),
                    },
                )?;
                Ok(RedsysWalletFields {
                    xpay_data: Some(Secret::new(BASE64_ENGINE.encode(json))),
                    xpay_decoded_data: None,
                    xpay_type: requests::RedsysXpayType::Google,
                    emv3ds: None,
                })
            }
            GpayTokenizationData::Decrypted(decrypted) => {
                let cryptogram = decrypted
                    .cryptogram
                    .clone()
                    .filter(|cryptogram| !cryptogram.peek().is_empty());
                // TOKENIZED_CARD (CRYPTOGRAM_3DS) needs no EMV3DS; CARD (PAN_ONLY) must carry
                // DS_MERCHANT_EMV3DS AuthenticationData (SIS0754/9998 otherwise)
                let (payment_method, emv3ds) = match &cryptogram {
                    Some(_) => (requests::RedsysGooglePayMethod::TokenizedCard, None),
                    None => {
                        // G-Authorize__Wallet-01
                        let browser_info = router_data.request.browser_info.clone().ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "browser_info",
                                context: redsys_error_context(
                                    "Send browser_info for a Google Pay PAN_ONLY (no cryptogram) payment",
                                    "Redsys Google Pay CARD requires DS_MERCHANT_EMV3DS AuthenticationData (SIS0754/9998)",
                                ),
                            },
                        )?;
                        let mut emv3ds = requests::RedsysEmvThreeDsRequestData::new(
                            requests::RedsysThreeDsInfo::AuthenticationData,
                        )
                        .add_browser_data(browser_info)?
                        .set_billing_data(router_data.resource_common_data.get_optional_billing())?
                        .set_shipping_data(
                            router_data.resource_common_data.get_optional_shipping(),
                        )?;
                        if let Some(url) = router_data.request.continue_redirection_url.clone() {
                            emv3ds = emv3ds.set_notification_u_r_l(url);
                        }
                        (requests::RedsysGooglePayMethod::Card, Some(emv3ds))
                    }
                };
                let decoded = requests::RedsysXpayDecodedData {
                    token: decrypted.application_primary_account_number.clone(),
                    cryptogram,
                    eci_ind: decrypted.eci_indicator.clone(),
                    expiration_date: wallet_expiry_yymm(
                        &decrypted.card_exp_year,
                        &decrypted.card_exp_month,
                        "card_exp_year",
                        "card_exp_month",
                    )?,
                    payment_method: Some(payment_method),
                };
                Ok(RedsysWalletFields {
                    xpay_data: None,
                    xpay_decoded_data: Some(encode_xpay_decoded_data(&decoded)?),
                    xpay_type: requests::RedsysXpayType::Google,
                    emv3ds,
                })
            }
        },
        WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::BluecodeRedirect {}
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::DanaRedirect {}
        | WalletData::GrabpayRedirect {}
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_)
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::SatispayIntent(_)
        | WalletData::SatispayQr(_)
        | WalletData::Wero(_)
        | WalletData::LazyPayRedirect(_)
        | WalletData::PhonePeRedirect(_)
        | WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_)
        | WalletData::PaymayaRedirect(_)
        | WalletData::PayhereRedirect {}
        | WalletData::QwikcilverWalletDirect(_)
        | WalletData::Skrill(_)
        | WalletData::Neteller(_) => Err(IntegrationError::NotImplemented(
            domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
            redsys_error_context(
                "Use Apple Pay or Google Pay",
                "Redsys supports only Apple Pay and Google Pay wallets (DS_XPAYTYPE Apple|Google)",
            ),
        )
        .into()),
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

        // Wallet arms carry XPAY fields instead of PAN/expiry/CVV
        let mut wallet = match &router_data.request.payment_method_data {
            PaymentMethodData::Wallet(wallet_data) => {
                Some(build_wallet_fields(wallet_data, router_data)?)
            }
            _ => None,
        };
        let card_data = match wallet {
            Some(_) => None,
            None => Some(requests::RedsysCardData::try_from(&Some(
                router_data.request.payment_method_data.clone(),
            ))?),
        };
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        let billing_data = router_data.resource_common_data.get_optional_billing();
        let shipping_data = router_data.resource_common_data.get_optional_shipping();

        // Redsys does not really support a no-3DS flow. Exemptions requested without EMV3DS data are
        // marked in the authorization; if the issuer does not accept them it answers Ds_Response
        // 0195 ("soft decline", SCA required) and the merchant must start a new 3DS request.
        let (
            ds_merchant_excep_sca,
            ds_merchant_directpayment,
            ds_merchant_emv3ds,
            ds_merchant_mpiexternal,
        ) = if let Some(wallet) = wallet.as_mut() {
            // Wallet payments are authenticated by the wallet (cryptogram) or, for Google Pay
            // PAN_ONLY, by the EMV3DS AuthenticationData built with the wallet fields; the
            // published wallet requests carry no EXCEP_SCA / DIRECTPAYMENT / MPIEXTERNAL
            (None, None, wallet.emv3ds.take(), None)
        } else if !item.router_data.resource_common_data.is_three_ds() {
            let exemption = determine_exemption(router_data)?;
            (Some(exemption), Some(true), None, None)
        } else {
            let auth_data = router_data
                .request
                .authentication_data
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                field_name: "authentication_data",
                context: redsys_error_context(
                    "Send authentication_data from the Authenticate leg or an external 3DS result",
                    "Redsys THREE_DS Authorize",
                ),
            })?;

            // External MPI: authentication was done outside Redsys (a CAVV is supplied and this
            // is not the continuation of a Redsys 3DS method/challenge) -> DS_MERCHANT_MPIEXTERNAL
            // threeds_method_comp_ind is not consulted: the gRPC->domain conversion always sets it
            // on Authorize (UNSPECIFIED -> NotAvailable), and Redsys-hosted legs never emit a cavv
            let is_external_mpi =
                auth_data.cavv.is_some() && router_data.request.redirect_response.is_none();

            if is_external_mpi {
                (None, None, None, Some(build_mpi_external(auth_data)?))
            } else {
                let emv3ds_data =
                    build_authorize_emv3ds(router_data, auth_data, billing_data, shipping_data)?;
                (None, None, Some(emv3ds_data), None)
            }
        };

        let ds_merchant_transactiontype =
            transaction_type_from_capture_method(router_data.request.capture_method)?;

        // INV-27 / SIS0663: a Redsys-hosted 3DS continuation addresses the order PreAuthenticate and
        // Authenticate used; those legs derive it without metadata (PaymentsAuthenticateData has none)
        let order_metadata = if ds_merchant_emv3ds.is_some() && wallet.is_none() {
            None
        } else {
            router_data.request.metadata.as_ref()
        };
        let ds_merchant_order = get_ds_merchant_order(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            order_metadata,
        )?;

        // CIT: ask Redsys for a card-on-file reference (Ds_Merchant_Identifier) and NTID
        let (ds_merchant_identifier, ds_merchant_cof_ini, ds_merchant_cof_type) = if router_data
            .request
            .setup_future_usage
            == Some(common_enums::FutureUsage::OffSession)
        {
            (
                Some(Secret::new(REDSYS_COF_IDENTIFIER_REQUIRED.to_string())),
                Some(requests::RedsysCofIni::S),
                Some(cof_type_from_mit_category(
                    router_data.request.mit_category.as_ref(),
                )),
            )
        } else {
            (None, None, None)
        };

        let (ds_merchant_pan, ds_merchant_cvv2, ds_merchant_expirydate) = match card_data {
            Some(card_data) => (
                Some(
                    cards::CardNumber::try_from(card_data.card_number.peek().to_string())
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: redsys_error_context(
                                "Send a valid card number",
                                "Redsys DS_MERCHANT_PAN",
                            ),
                        })
                        .attach_printable("Invalid card number")?,
                ),
                Some(card_data.cvv2),
                Some(card_data.expiry_date),
            ),
            None => (None, None, None),
        };

        // UD-04: browser_info present -> web checkout, absent -> in-app
        let ds_xpayorigen = wallet.as_ref().map(|_| {
            if router_data.request.browser_info.is_some() {
                requests::RedsysXpayOrigen::Web
            } else {
                requests::RedsysXpayOrigen::InApp
            }
        });
        let (ds_xpaydata, ds_xpaydecodeddata, ds_xpaytype) = match wallet {
            Some(wallet) => (
                wallet.xpay_data,
                wallet.xpay_decoded_data,
                Some(wallet.xpay_type),
            ),
            None => (None, None, None),
        };

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(&router_data.request.money)?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_cvv2,
            ds_merchant_emv3ds,
            ds_merchant_expirydate,
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_directpayment,
            ds_merchant_excep_sca,
            ds_merchant_mpiexternal,
            ds_merchant_identifier,
            ds_merchant_cof_ini,
            ds_merchant_cof_type,
            ds_merchant_cof_txnid: None,
            ds_merchant_merchanturl: build_merchant_url(router_data.request.webhook_url.as_ref())?,
            ds_merchant_merchantdescriptor: build_merchant_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            )?,
            ds_xpaydata,
            ds_xpaydecodeddata,
            ds_xpaytype,
            ds_xpayorigen,
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
                let current_status = item.router_data.resource_common_data.status;

                let (authenticate_response, status, ds_order) = get_payments_response(
                    response_data,
                    item.router_data.request.capture_method,
                    auth_data,
                    current_status,
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
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
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
            }),
        }
    }
}

// SetupMandate: TRANSACTIONTYPE 7 card validation (UD-01) with DS_MERCHANT_IDENTIFIER=REQUIRED,
// COF_INI=S and DIRECTPAYMENT=true; sandbox probe answers 0000 with Ds_Merchant_Identifier and
// Ds_Merchant_Cof_Txnid (run:test/evidence/baseline/probe_SetupMandate.txt).

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysSetupMandateRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
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

        // G-SetupMandate-01: the Redsys 3DS legs are wired for Authorize only
        if router_data.resource_common_data.is_three_ds() {
            return Err(IntegrationError::NotSupported {
                message: "3DS SetupMandate".to_string(),
                connector: "redsys",
                context: redsys_error_context(
                    "Use Authorize with setup_future_usage OFF_SESSION for a 3DS card-on-file setup",
                    "Redsys SetupMandate is a TRANSACTIONTYPE 7 validation with DIRECTPAYMENT=true",
                ),
            }
            .into());
        }

        // Wallet CIT rides Authorize setup_future_usage: XPAY fields apply to TRANSACTIONTYPE 0/1 only
        if let PaymentMethodData::Wallet(WalletData::ApplePay(_) | WalletData::GooglePay(_)) =
            &router_data.request.payment_method_data
        {
            return Err(IntegrationError::NotSupported {
                message: "wallet SetupMandate".to_string(),
                connector: "redsys",
                context: redsys_error_context(
                    "Use Authorize with setup_future_usage OFF_SESSION for a wallet card-on-file setup",
                    "Redsys DS_XPAY* fields apply to TRANSACTIONTYPE 0/1 only",
                ),
            }
            .into());
        }

        // Card arm; every other payment method is refused as NotImplemented here
        let card_data = requests::RedsysCardData::try_from(&Some(
            router_data.request.payment_method_data.clone(),
        ))?;
        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        // G-SetupMandate-02: type 7 must carry an amount (UD-02); never defaulted
        let money =
            router_data
                .request
                .money
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: redsys_error_context(
                        "Send amount 0 on SetupRecurring",
                        "Redsys TRANSACTIONTYPE 7 requires DS_MERCHANT_AMOUNT",
                    ),
                })?;
        // G-SetupMandate-02 / UD-02: a type-7 validation moves no money (issuer sees 0), so a
        // non-zero amount would be reported Charged for a charge that never happened
        if money.amount() > common_utils::types::MinorUnit::zero() {
            return Err(IntegrationError::NotSupported {
                message: "Non-zero amount on SetupMandate".to_string(),
                connector: "redsys",
                context: redsys_error_context(
                    "Send amount 0 on SetupRecurring, or use Authorize with setup_future_usage OFF_SESSION for a paid CIT",
                    "Redsys TRANSACTIONTYPE 7 validation: amount sent to the issuer is zero",
                ),
            }
            .into());
        }

        // G-SetupMandate-03: Ds_Merchant_Order (SIS0075) via the shared helper: a valid reference is
        // sent as-is, any other non-empty one is derived to 12 digits; only an empty one is refused
        let ds_merchant_order = get_ds_merchant_order(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            router_data.request.metadata.as_ref(),
        )?;

        let ds_merchant_pan = cards::CardNumber::try_from(card_data.card_number.peek().to_string())
            .change_context(IntegrationError::RequestEncodingFailed {
                context: redsys_error_context("Send a valid card number", "Redsys DS_MERCHANT_PAN"),
            })
            .attach_printable("Invalid card number")?;

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(money)?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_cvv2: Some(card_data.cvv2),
            ds_merchant_emv3ds: None,
            ds_merchant_expirydate: Some(card_data.expiry_date),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: Some(ds_merchant_pan),
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype: requests::RedsysTransactionType::Validation,
            ds_merchant_excep_sca: None,
            ds_merchant_directpayment: Some(true),
            ds_merchant_mpiexternal: None,
            ds_merchant_identifier: Some(Secret::new(REDSYS_COF_IDENTIFIER_REQUIRED.to_string())),
            ds_merchant_cof_ini: Some(requests::RedsysCofIni::S),
            ds_merchant_cof_type: Some(cof_type_from_mit_category(
                router_data.request.mit_category.as_ref(),
            )),
            ds_merchant_cof_txnid: None,
            ds_merchant_merchanturl: build_merchant_url(router_data.request.webhook_url.as_ref())?,
            ds_merchant_merchantdescriptor: build_merchant_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            )?,
            ds_xpaydata: None,
            ds_xpaydecodeddata: None,
            ds_xpaytype: None,
            ds_xpayorigen: None,
        };

        Self::try_from((&payment_request, &auth))
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
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

                let current_status = item.router_data.resource_common_data.status;

                // UD-12: an authorised type-7 validation has nothing to capture -> Charged
                // (capture_method None maps 0000-0099 to Charged). An authorised validation must
                // return the card-on-file reference (spec "## API Call Sequences").
                let is_authorised = response_data
                    .ds_response
                    .as_ref()
                    .and_then(|ds_response| get_redsys_attempt_status(ds_response, None))
                    == Some(common_enums::AttemptStatus::Charged);
                if is_authorised && response_data.ds_merchant_identifier.is_none() {
                    return Err(utils::response_handling_fail(
                        item.http_code,
                        "redsys: authorised SetupMandate returned no Ds_Merchant_Identifier",
                    )
                    .into());
                }

                let (response, status, ds_order) = get_payments_response(
                    response_data,
                    None,
                    None,
                    current_status,
                    item.http_code,
                    true,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: Some(ds_order),
                        ..item.router_data.resource_common_data
                    },
                    response,
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
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                        common_enums::AttemptStatus::Failure,
                    )),
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
            }),
        }
    }
}

// RepeatPayment: MIT TRANSACTIONTYPE 0/1 with COF_INI=N, EXCEP_SCA=MIT and DIRECTPAYMENT=true
// (techspec "### RepeatPayment"). Option 1: stored DS_MERCHANT_IDENTIFIER; Option 2 (NTID): PAN or
// wallet DPAN + EXPIRYDATE + DS_MERCHANT_COF_TXNID. Sandbox answers 0000 for both
// (run:test/evidence/baseline/probe_RepeatPayment.txt).

/// Card-on-file data a Redsys MIT carries, per payment-method arm
struct RedsysMitReference {
    identifier: Option<Secret<String>>,
    pan: Option<cards::CardNumber>,
    expiry_date: Option<Secret<String>>,
    cof_txnid: Option<Secret<String>>,
}

fn repeat_payment_missing_field(field_name: &'static str, additional_context: &str) -> Error {
    IntegrationError::MissingRequiredField {
        field_name,
        context: redsys_error_context(
            "Send the mandate reference that matches the payment method (connector_mandate_id with a mandate payment, network_transaction_id with card or decrypted wallet data)",
            additional_context,
        ),
    }
    .into()
}

fn repeat_payment_network_token_refused() -> Error {
    IntegrationError::NotSupported {
        message: "network token RepeatPayment".to_string(),
        connector: "redsys",
        context: redsys_error_context(
            "Use a connector_mandate_id or a network_transaction_id with card data",
            "Redsys has no network-token MIT parameter",
        ),
    }
    .into()
}

fn mit_pan(card_number: String) -> Result<cards::CardNumber, Error> {
    cards::CardNumber::try_from(card_number)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: redsys_error_context("Send a valid card number", "Redsys DS_MERCHANT_PAN"),
        })
        .attach_printable("Invalid card number")
}

/// G-RepeatPayment-03: Option 2 needs a non-empty network_transaction_id (SIS0617)
fn mit_network_transaction_id(
    mandate_reference: &connector_types::MandateReferenceId,
) -> Result<Secret<String>, Error> {
    match mandate_reference {
        connector_types::MandateReferenceId::NetworkMandateId(network_mandate)
            if !network_mandate.network_transaction_id.is_empty() =>
        {
            Ok(Secret::new(network_mandate.network_transaction_id.clone()))
        }
        connector_types::MandateReferenceId::NetworkMandateId(_)
        | connector_types::MandateReferenceId::ConnectorMandateId(_) => {
            Err(repeat_payment_missing_field(
                "network_transaction_id",
                "Redsys MIT by PAN requires DS_MERCHANT_COF_TXNID (SIS0617)",
            ))
        }
        connector_types::MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(repeat_payment_network_token_refused())
        }
    }
}

fn build_mit_reference<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
    mandate_reference: &connector_types::MandateReferenceId,
) -> Result<RedsysMitReference, Error> {
    match payment_method_data {
        // Option 1: stored reference from SetupMandate Ds_Merchant_Identifier. A PaymentMethodToken
        // with a ConnectorMandateId is the same stored-reference MIT: Redsys has no PSP token
        // parameter, so the token value is not sent (b1-02/B020).
        PaymentMethodData::MandatePayment | PaymentMethodData::PaymentMethodToken(_) => {
            match mandate_reference {
                connector_types::MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                    // G-RepeatPayment-03
                    let identifier = connector_mandate
                        .get_connector_mandate_id()
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| {
                            repeat_payment_missing_field(
                                "connector_mandate_id",
                                "Redsys MIT by stored reference requires DS_MERCHANT_IDENTIFIER (SIS0617)",
                            )
                        })?;
                    Ok(RedsysMitReference {
                        identifier: Some(Secret::new(identifier)),
                        pan: None,
                        expiry_date: None,
                        // techspec Option 1 example sends the CIT Ds_Merchant_Cof_Txnid alongside
                        cof_txnid: connector_mandate
                            .get_network_transaction_id()
                            .filter(|txn_id| !txn_id.is_empty())
                            .map(Secret::new),
                    })
                }
                // G-RepeatPayment-03: an NTID MIT needs the card or DPAN it was issued for
                connector_types::MandateReferenceId::NetworkMandateId(_) => {
                    Err(repeat_payment_missing_field(
                        "payment_method_data.card",
                        "Redsys MIT by network_transaction_id requires DS_MERCHANT_PAN and DS_MERCHANT_EXPIRYDATE",
                    ))
                }
                connector_types::MandateReferenceId::NetworkTokenWithNTI(_) => {
                    Err(repeat_payment_network_token_refused())
                }
            }
        }
        // Option 2: PAN + EXPIRYDATE + COF_TXNID
        PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
            let cof_txnid = mit_network_transaction_id(mandate_reference)?;
            Ok(RedsysMitReference {
                identifier: None,
                pan: Some(mit_pan(card.card_number.peek().to_string())?),
                expiry_date: Some(card.get_expiry_date_as_yymm()?),
                cof_txnid: Some(cof_txnid),
            })
        }
        // UD-06: wallet MIT sends the decrypted DPAN as PAN + EXPIRYDATE + COF_TXNID
        PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(wallet_token) => {
            let cof_txnid = mit_network_transaction_id(mandate_reference)?;
            Ok(RedsysMitReference {
                identifier: None,
                pan: Some(mit_pan(wallet_token.decrypted_token.peek().to_string())?),
                expiry_date: Some(wallet_token.get_expiry_date_as_yymm()?),
                cof_txnid: Some(cof_txnid),
            })
        }
        PaymentMethodData::NetworkToken(_) => Err(repeat_payment_network_token_refused()),
        PaymentMethodData::Card(_)
        | PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::Wallet(_)
        | PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::CardWithNoCvc(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
            domain_types::utils::get_unimplemented_payment_method_error_message("redsys"),
            redsys_error_context(
                "Use a mandate payment or payment method token with a connector_mandate_id, card details or decrypted wallet data with a network_transaction_id",
                "Redsys RepeatPayment supports stored-reference and NTID MITs only",
            ),
        )
        .into()),
    }
}

impl<T>
    TryFrom<
        RedsysRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::RedsysRepeatPaymentRequest
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = Error;

    fn try_from(
        item: RedsysRouterData<
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

        // G-RepeatPayment-01: ManualMultiple/Scheduled refused before any request is built
        let ds_merchant_transactiontype =
            transaction_type_from_capture_method(router_data.request.capture_method)?;

        // G-RepeatPayment-03 (+ payment-method arm refusals)
        let mit_reference = build_mit_reference(
            &router_data.request.payment_method_data,
            &router_data.request.mandate_reference,
        )?;

        // G-RepeatPayment-02: merchant reference passed through when Redsys-valid, else derived to a
        // deterministic 12-digit order; only an empty reference is refused (SIS0075, P-foundation-09)
        let ds_merchant_order = get_ds_merchant_order(
            router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            router_data.request.metadata.as_ref(),
        )?;

        let auth = RedsysAuthType::try_from(&router_data.connector_config)?;

        let payment_request = requests::RedsysPaymentRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(&router_data.request.money)?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_cvv2: None,
            ds_merchant_emv3ds: None,
            ds_merchant_expirydate: mit_reference.expiry_date,
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order,
            ds_merchant_pan: mit_reference.pan,
            ds_merchant_terminal: auth.terminal_id.clone(),
            ds_merchant_transactiontype,
            ds_merchant_excep_sca: Some(requests::RedsysStrongCustomerAuthenticationException::Mit),
            ds_merchant_directpayment: Some(true),
            ds_merchant_mpiexternal: None,
            ds_merchant_identifier: mit_reference.identifier,
            ds_merchant_cof_ini: Some(requests::RedsysCofIni::N),
            ds_merchant_cof_type: Some(cof_type_from_mit_category(
                router_data.request.mit_category.as_ref(),
            )),
            ds_merchant_cof_txnid: mit_reference.cof_txnid,
            ds_merchant_merchanturl: build_merchant_url(router_data.request.webhook_url.as_ref())?,
            ds_merchant_merchantdescriptor: build_merchant_descriptor(
                router_data.request.billing_descriptor.as_ref(),
            )?,
            ds_xpaydata: None,
            ds_xpaydecodeddata: None,
            ds_xpaytype: None,
            ds_xpayorigen: None,
        };

        Self::try_from((&payment_request, &auth))
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<responses::RedsysResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
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

                // type 0 -> Charged, type 1 -> Authorized (spec "## Status Mappings")
                let (response, status, ds_order) = get_payments_response(
                    response_data,
                    item.router_data.request.capture_method,
                    None,
                    item.router_data.resource_common_data.status,
                    item.http_code,
                    true,
                )?;

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        reference_id: Some(ds_order),
                        ..item.router_data.resource_common_data
                    },
                    response,
                    ..item.router_data
                })
            }
            // spec "## HTTP Codes and Errors": a 2xx errorCode body (e.g. SIS0617) fails the MIT
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::Failure,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                        common_enums::AttemptStatus::Failure,
                    )),
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
            // G-Capture-01: confirmation (type 2) is keyed by the Ds_Order of the preauth
            _ => Err(IntegrationError::MissingConnectorTransactionID {
                context: redsys_error_context(
                    "Send the connector_transaction_id (Ds_Order) returned by the manual-capture Authorize",
                    "Redsys confirmation (DS_MERCHANT_TRANSACTIONTYPE 2) requires the preauth Ds_Order",
                ),
            }),
        }?;

        let capture_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(&router_data.request.capture_money)?,
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

                let attempt_status = capture_status_from_ds_response(
                    &response_data.ds_response,
                    item.router_data.resource_common_data.status,
                );

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
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(response_data.ds_order),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            // spec "## HTTP Codes and Errors": SIS0059/SIS0060/SIS0062/... reject the confirmation,
            // not the payment (TH-06)
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                        common_enums::AttemptStatus::CaptureFailed,
                    )),
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
                // G-Void-01: cancellation (type 9) must carry the preauth amount + currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: redsys_error_context(
                        "Send the currency of the authorization being voided",
                        "Redsys cancellation (DS_MERCHANT_TRANSACTIONTYPE 9) requires DS_MERCHANT_CURRENCY",
                    ),
                })?;
        let amount = router_data
            .request
            .money
            .as_ref()
            .ok_or(IntegrationError::MissingRequiredField {
            field_name: "amount",
            context: redsys_error_context(
                "Send the amount of the authorization being voided",
                "Redsys cancellation (DS_MERCHANT_TRANSACTIONTYPE 9) requires DS_MERCHANT_AMOUNT",
            ),
        })?;

        let void_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(amount)?,
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

                let attempt_status = void_status_from_ds_response(
                    &response_data.ds_response,
                    item.router_data.resource_common_data.status,
                );

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
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(response_data.ds_order),
                        incremental_authorization_allowed: None,
                        status_code: item.http_code,
                        splits: None,
                        payment_account_reference: None,
                    }),
                    ..item.router_data
                })
            }
            // spec "## HTTP Codes and Errors": SIS0072/SIS0222/SIS0225/SIS0625 reject the
            // cancellation, not the payment (TH-06)
            responses::RedsysResponse::RedsysErrorResponse(ref err) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                response: Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(domain_types::router_data::FlowStatus::Payment(
                        common_enums::AttemptStatus::VoidFailed,
                    )),
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
                            &ds_response,
                            item.router_data.request.capture_method,
                        )
                        .unwrap_or(item.router_data.resource_common_data.status);
                        let payment_response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                latest_response.ds_order.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(latest_response.ds_order.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
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
                                // Completed - capture method decides (spec "## Status Mappings"; HS :2290-2315)
                                match item.router_data.request.capture_method {
                                    Some(enums::CaptureMethod::Automatic)
                                    | Some(enums::CaptureMethod::SequentialAutomatic)
                                    | None => common_enums::AttemptStatus::Charged,
                                    Some(enums::CaptureMethod::Manual) => {
                                        common_enums::AttemptStatus::Authorized
                                    }
                                    // refused before any request is sent (transaction_type_from_capture_method)
                                    Some(enums::CaptureMethod::ManualMultiple)
                                    | Some(enums::CaptureMethod::Scheduled) => {
                                        item.router_data.resource_common_data.status
                                    }
                                }
                            }
                            // spec: T/E/D/L/W/O and undocumented states are inconclusive - keep current status (TH-07)
                            Some(responses::DsState::T)
                            | Some(responses::DsState::E)
                            | Some(responses::DsState::D)
                            | Some(responses::DsState::L)
                            | Some(responses::DsState::W)
                            | Some(responses::DsState::O)
                            | Some(responses::DsState::Unknown)
                            | None => item.router_data.resource_common_data.status,
                        };

                        let payment_response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                latest_response.ds_order.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(latest_response.ds_order.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
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
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
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
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
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
        let connector_transaction_id = &router_data.request.connector_transaction_id;
        // G-Refund-01: refund (type 3) is keyed by the Ds_Order of the settled payment;
        // an empty order is rejected by Redsys (SIS0054) - refuse before HTTP
        if connector_transaction_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: redsys_error_context(
                    "Send the connector_transaction_id (Ds_Order) of the payment being refunded",
                    "Redsys refund (DS_MERCHANT_TRANSACTIONTYPE 3) requires the payment's Ds_Order",
                ),
            }
            .into());
        }

        let refund_request = requests::RedsysOperationRequest {
            ds_merchant_amount: RedsysAmountConvertor::convert(&router_data.request.refund_money)?,
            ds_merchant_currency: router_data.request.currency.iso_4217().to_owned(),
            ds_merchant_merchantcode: auth.merchant_id.clone(),
            ds_merchant_order: connector_transaction_id.clone(),
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

                let refund_status = refund_status_from_ds_response(&response_data.ds_response);

                Ok(RefundsResponseData {
                    connector_refund_id: response_data.ds_order,
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                })
            }
            // spec "## HTTP Codes and Errors": SIS0054/SIS0056/SIS0057/SIS0626 reject the
            // refund, not the payment (TH-06)
            responses::RedsysResponse::RedsysErrorResponse(ref err) => {
                Err(domain_types::router_data::ErrorResponse {
                    code: err.error_code.clone(),
                    message: err
                        .error_code_description
                        .clone()
                        .unwrap_or_else(|| err.error_code.clone()),
                    reason: err.error_code_description.clone(),
                    status_code: item.http_code,
                    attempt_status: Some(domain_types::router_data::FlowStatus::Refund(
                        common_enums::RefundStatus::Failure,
                    )),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
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
                // P-RSync-02: only refund rows (Ds_TransactionType 3) reconcile a refund (TH-20);
                // the latest of them by date/hour wins (spec "## API Call Sequences").
                let refund_responses = responses
                    .into_iter()
                    .filter(|response_data| response_data.ds_transactiontype == "3")
                    .collect();
                if let Some(latest_response) = find_latest_response(refund_responses) {
                    if let Some(ds_response) = latest_response.ds_response {
                        let refund_status = refund_status_from_ds_response(&ds_response);
                        Ok(RefundsResponseData {
                            connector_refund_id: latest_response.ds_order,
                            refund_status,
                            status_code: item.http_code,
                            acquirer_reference_number: None,
                        })
                    } else {
                        Ok(RefundsResponseData {
                            connector_refund_id: latest_response.ds_order,
                            refund_status: common_enums::RefundStatus::Pending,
                            status_code: item.http_code,
                            acquirer_reference_number: None,
                        })
                    }
                } else {
                    Err(domain_types::router_data::ErrorResponse {
                        code: "NO_VALID_RESPONSES".to_string(),
                        message: "No valid responses found in Monitor/Transaction query"
                            .to_string(),
                        reason: Some(
                            "Query returned no refund (Ds_TransactionType 3) responses with valid date/hour".to_string(),
                        ),
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
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
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

// ---------------------------------------------------------------------------------------------
// IncomingWebhook (spec "## Webhook Events", "### Webhook Authentication & Signature Verification")
// ---------------------------------------------------------------------------------------------

/// `Ds_SignatureVersion` echoed by a notification; it is signed with the algorithm of the
/// originating request (spec: REST manual Annex 1 §2), so the verifier branches on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedsysSignatureVersion {
    #[serde(rename = "HMAC_SHA256_V1")]
    HmacSha256V1,
    #[serde(rename = "HMAC_SHA512_V1")]
    HmacSha512V1,
    #[serde(rename = "HMAC_SHA512_V2")]
    HmacSha512V2,
    #[serde(other)]
    Unknown,
}

/// The three POST parameters of an online notification (spec "### Webhook Payload Structure")
#[derive(Debug, Deserialize)]
pub struct RedsysWebhookBody {
    #[serde(rename = "Ds_SignatureVersion")]
    pub ds_signature_version: RedsysSignatureVersion,
    #[serde(rename = "Ds_MerchantParameters")]
    pub ds_merchant_parameters: Secret<String>,
    #[serde(rename = "Ds_Signature")]
    pub ds_signature: Secret<String>,
}

/// `Ds_TransactionType` of a notification (spec "### Event Types"; s19 §1.4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedsysWebhookTransactionType {
    #[serde(rename = "0")]
    Payment,
    #[serde(rename = "1")]
    Preauthorization,
    #[serde(rename = "2")]
    Confirmation,
    #[serde(rename = "3")]
    Refund,
    #[serde(rename = "7")]
    Validation,
    #[serde(rename = "9")]
    Cancellation,
    #[serde(rename = "45")]
    SeparateCancellation,
    #[serde(rename = "48")]
    SeparateConfirmation,
    #[serde(other)]
    Unknown,
}

/// Decoded `Ds_MerchantParameters` of a notification. Every field is read from the signed
/// parameters only (TH-17).
#[derive(Debug, Serialize, Deserialize)]
pub struct RedsysWebhookParameters {
    #[serde(rename = "Ds_Order")]
    pub ds_order: String,
    #[serde(rename = "Ds_TransactionType")]
    pub ds_transaction_type: RedsysWebhookTransactionType,
    #[serde(rename = "Ds_Response")]
    pub ds_response: responses::DsResponse,
}

/// What a notification means: the event plus the status it carries (plan status_map)
pub enum RedsysWebhookOutcome {
    Payment(connector_types::EventType, common_enums::AttemptStatus),
    Refund(connector_types::EventType, common_enums::RefundStatus),
}

/// Parses the notification body: JSON when it starts with `{`, else
/// `application/x-www-form-urlencoded` (plan UD-08; the content type is not documented).
pub fn parse_redsys_webhook_body(body: &[u8]) -> Result<RedsysWebhookBody, Report<WebhookError>> {
    if body.trim_ascii_start().starts_with(b"{") {
        serde_json::from_slice(body).change_context(WebhookError::WebhookBodyDecodingFailed)
    } else {
        serde_urlencoded::from_bytes(body).change_context(WebhookError::WebhookBodyDecodingFailed)
    }
}

/// Base64 decode tolerant of both alphabets and of a form-decoded `+` that arrived as a space
/// (plan UD-14).
fn decode_redsys_base64(value: &str) -> Result<Vec<u8>, Report<WebhookError>> {
    utils::safe_base64_decode(value.trim().replace(' ', "+"))
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

pub fn decode_redsys_webhook_parameters(
    body: &RedsysWebhookBody,
) -> Result<RedsysWebhookParameters, Report<WebhookError>> {
    let decoded = decode_redsys_base64(body.ds_merchant_parameters.peek())?;
    serde_json::from_slice(&decoded).change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// HMAC_SHA512_V1/V2 per-order key (spec "#### HMAC_SHA512_V1"): AES-128-CBC, zero IV, PKCS#7,
/// of `Ds_Order` keyed with the first 16 characters of the terminal key (right-padded with '0');
/// the HMAC key is the ASCII of the Base64 ciphertext. Reproduces the REST manual v4.0.1 §3.3
/// vector (order `1234567890` -> `RWt3/IPTzYRMXsQtkiGRKg==`) and the sandbox probe
/// (run test/evidence/baseline/probe_Authorize__Card.txt).
fn redsys_sha512_order_key(order: &str, key: &str) -> Result<String, Report<WebhookError>> {
    use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
    const AES_BLOCK: usize = 16;
    let aes_key: Vec<u8> = key
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(AES_BLOCK)
        .collect();
    let mut buffer = order.as_bytes().to_vec();
    buffer.resize(order.len() + AES_BLOCK, 0);
    let encrypted_len = cbc::Encryptor::<aes::Aes128>::new_from_slices(&aes_key, &[0u8; AES_BLOCK])
        .change_context(WebhookError::WebhookSourceVerificationFailed)?
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, order.len())
        .map_err(|_| Report::new(WebhookError::WebhookSourceVerificationFailed))?
        .len();
    buffer.truncate(encrypted_len);
    Ok(BASE64_ENGINE.encode(buffer))
}

/// Verifies a notification signature with the terminal key, branching on `Ds_SignatureVersion`
/// (spec "#### Verifying responses and notifications"): derive the per-order key, HMAC the
/// received `Ds_MerchantParameters` string as-is, compare in constant time (ring `hmac::verify`).
/// SHA512_V1 and V2 differ only in the signature's Base64 alphabet/padding, which decoding
/// normalises. An unknown version is never trusted.
pub fn verify_redsys_signature(
    body: &RedsysWebhookBody,
    order: &str,
    key: &str,
) -> Result<bool, Report<WebhookError>> {
    let Ok(signature) = decode_redsys_base64(body.ds_signature.peek()) else {
        return Ok(false);
    };
    let message = body.ds_merchant_parameters.peek().as_bytes();
    match body.ds_signature_version {
        RedsysSignatureVersion::HmacSha256V1 => {
            let order_key = des_encrypt(order, key)
                .change_context(WebhookError::WebhookSourceVerificationFailed)?;
            crypto::HmacSha256
                .verify_signature(&order_key, &signature, message)
                .change_context(WebhookError::WebhookSourceVerificationFailed)
        }
        RedsysSignatureVersion::HmacSha512V1 | RedsysSignatureVersion::HmacSha512V2 => {
            let order_key = redsys_sha512_order_key(order, key)?;
            crypto::HmacSha512
                .verify_signature(order_key.as_bytes(), &signature, message)
                .change_context(WebhookError::WebhookSourceVerificationFailed)
        }
        RedsysSignatureVersion::Unknown => Ok(false),
    }
}

/// Event + status of a notification. The status is the same Ds_Response mapping the sync flows
/// use (`get_redsys_attempt_status` / `capture_status_from_ds_response` /
/// `void_status_from_ds_response`, spec "## Status Mappings"); `Ds_TransactionType` only picks the
/// capture/void/refund context, and the event is derived from the status. A refund uses
/// `refund_status_from_ds_response` so an undocumented code stays non-terminal (TH-07).
pub fn get_redsys_webhook_outcome(
    params: &RedsysWebhookParameters,
) -> Result<RedsysWebhookOutcome, Report<WebhookError>> {
    use common_enums::AttemptStatus;
    use connector_types::EventType;
    use RedsysWebhookTransactionType as Tx;

    if params.ds_transaction_type == Tx::Refund {
        let status = refund_status_from_ds_response(&params.ds_response);
        let event = match status {
            common_enums::RefundStatus::Success => EventType::RefundSuccess,
            common_enums::RefundStatus::Failure
            | common_enums::RefundStatus::TransactionFailure => EventType::RefundFailure,
            common_enums::RefundStatus::Pending
            | common_enums::RefundStatus::ManualReview
            | common_enums::RefundStatus::Unknown => EventType::RefundProcessing,
        };
        return Ok(RedsysWebhookOutcome::Refund(event, status));
    }
    // One Ds_Response mapping with Authorize/PSync/Capture/Void (spec "## Status Mappings"); the
    // transaction type only picks the capture context. An undocumented code stays non-terminal
    // (TH-07): sync keeps the current status, a notification has none, so Pending.
    let ds_response = &params.ds_response;
    let status = match params.ds_transaction_type {
        Tx::Payment => {
            get_redsys_attempt_status(ds_response, Some(enums::CaptureMethod::Automatic))
        }
        Tx::Preauthorization => {
            get_redsys_attempt_status(ds_response, Some(enums::CaptureMethod::Manual))
        }
        // UD-12: an authorised type-7 validation is Charged, as in the SetupMandate response
        Tx::Validation => get_redsys_attempt_status(ds_response, None),
        Tx::Confirmation | Tx::SeparateConfirmation => Some(capture_status_from_ds_response(
            ds_response,
            AttemptStatus::Pending,
        )),
        Tx::Cancellation | Tx::SeparateCancellation => Some(void_status_from_ds_response(
            ds_response,
            AttemptStatus::Pending,
        )),
        Tx::Refund => return Err(Report::new(WebhookError::WebhookEventTypeNotFound)),
        // spec: undocumented Ds_TransactionType
        Tx::Unknown => {
            return Err(Report::new(WebhookError::WebhookEventTypeNotFound)).attach_printable(
                "redsys: notification Ds_TransactionType is not a documented operation",
            )
        }
    }
    .unwrap_or(AttemptStatus::Pending);
    let is_confirmation = matches!(
        params.ds_transaction_type,
        Tx::Confirmation | Tx::SeparateConfirmation
    );
    let event = match status {
        AttemptStatus::Charged if is_confirmation => EventType::PaymentIntentCaptureSuccess,
        AttemptStatus::Charged => EventType::PaymentIntentSuccess,
        AttemptStatus::Authorized => EventType::PaymentIntentAuthorizationSuccess,
        AttemptStatus::Voided => EventType::PaymentIntentCancelled,
        AttemptStatus::VoidFailed => EventType::PaymentIntentCancelFailure,
        AttemptStatus::CaptureFailed => EventType::PaymentIntentCaptureFailure,
        AttemptStatus::AuthenticationPending => EventType::PaymentActionRequired,
        AttemptStatus::Pending => EventType::PaymentIntentProcessing,
        _ => EventType::PaymentIntentFailure,
    };
    Ok(RedsysWebhookOutcome::Payment(event, status))
}
