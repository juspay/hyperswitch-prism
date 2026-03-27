pub mod qr_code;
pub mod xml_utils;
use base64::Engine;
use common_utils::{
    consts::{
        BASE64_ENGINE, BASE64_ENGINE_STD_NO_PAD, BASE64_ENGINE_URL_SAFE,
        BASE64_ENGINE_URL_SAFE_NO_PAD,
    },
    errors::{ParsingError, ReportSwitchExt},
    ext_traits::ValueExt,
    request::MultipartData,
    types::MinorUnit,
    CustomResult,
};
use domain_types::{
    connector_types::{
        CaptureSyncResponse, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsSyncData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_response_types::Response,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};
pub use xml_utils::preprocess_xml_response_bytes;

type Error = Report<errors::ConnectorError>;
use common_enums::enums;
use serde::{Deserialize, Serialize};

pub fn build_form_from_struct<T: Serialize>(
    data: T,
) -> Result<MultipartData, errors::ParsingError> {
    let mut form = MultipartData::new();
    let serialized =
        serde_json::to_value(&data).map_err(|_| errors::ParsingError::EncodeError("json-value"))?;
    let serialized_object = serialized
        .as_object()
        .ok_or(errors::ParsingError::EncodeError("Expected object"))?;
    for (key, values) in serialized_object {
        let value = match values {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(_) | Value::Object(_) | Value::Null => {
                tracing::warn!(field = %key, "Form construction encountered a non-primitive type. Skipping field or using empty string.");
                "".to_string()
            }
        };
        form.add_text(key.clone(), value.clone());
    }
    Ok(form)
}

#[macro_export]
macro_rules! with_error_response_body {
    ($event_builder:ident, $response:ident) => {
        if let Some(body) = $event_builder {
            body.set_connector_response(&$response);
        }
    };
}

#[macro_export]
macro_rules! with_response_body {
    ($event_builder:ident, $response:ident) => {
        if let Some(body) = $event_builder {
            body.set_connector_response(&$response);
        }
    };
}

pub trait PaymentsAuthorizeRequestData {
    fn get_router_return_url(&self) -> Result<String, Error>;
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static>
    PaymentsAuthorizeRequestData for PaymentsAuthorizeData<T>
{
    fn get_router_return_url(&self) -> Result<String, Error> {
        self.router_return_url
            .clone()
            .ok_or_else(missing_field_err("return_url"))
    }
}

pub fn missing_field_err(
    message: &'static str,
) -> Box<dyn Fn() -> Report<errors::ConnectorError> + 'static> {
    Box::new(move || {
        errors::ConnectorError::MissingRequiredField {
            field_name: message,
        }
        .into()
    })
}

pub(crate) fn get_unimplemented_payment_method_error_message(connector: &str) -> String {
    format!("Selected payment method through {connector}")
}

pub(crate) fn to_connector_meta_from_secret<T>(
    connector_meta: Option<Secret<Value>>,
) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let connector_meta_secret =
        connector_meta.ok_or_else(missing_field_err("connector_meta_data"))?;

    let json_value = connector_meta_secret.expose();

    let parsed: T = match json_value {
        Value::String(json_str) => serde_json::from_str(&json_str)
            .map_err(Report::from)
            .change_context(errors::ConnectorError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
            })?,
        _ => serde_json::from_value(json_value.clone())
            .map_err(Report::from)
            .change_context(errors::ConnectorError::InvalidConnectorConfig {
                config: "merchant_connector_account.metadata",
            })?,
    };

    Ok(parsed)
}

pub(crate) fn handle_json_response_deserialization_failure(
    res: Response,
    _connector: &'static str,
) -> CustomResult<ErrorResponse, errors::ConnectorError> {
    let response_data = String::from_utf8(res.response.to_vec())
        .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

    // check for whether the response is in json format
    match serde_json::from_str::<Value>(&response_data) {
        // in case of unexpected response but in json format
        Ok(_) => Err(errors::ConnectorError::ResponseDeserializationFailed)?,
        // in case of unexpected response but in html or string format
        Err(_error_msg) => Ok(ErrorResponse {
            status_code: res.status_code,
            code: "No error code".to_string(),
            message: "Unsupported response type".to_string(),
            reason: Some(response_data),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        }),
    }
}

pub fn is_refund_failure(status: enums::RefundStatus) -> bool {
    match status {
        common_enums::RefundStatus::Failure | common_enums::RefundStatus::TransactionFailure => {
            true
        }
        common_enums::RefundStatus::ManualReview
        | common_enums::RefundStatus::Pending
        | common_enums::RefundStatus::Success => false,
    }
}

pub(crate) fn safe_base64_decode(base64_data: String) -> Result<Vec<u8>, Error> {
    let mut error_stack = Vec::new();
    [
        &BASE64_ENGINE,
        &BASE64_ENGINE_STD_NO_PAD,
        &BASE64_ENGINE_URL_SAFE,
        &BASE64_ENGINE_URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| {
        engine
            .decode(&base64_data)
            .map_err(|e| error_stack.push(e))
            .ok()
    })
    .ok_or(errors::ConnectorError::ResponseDeserializationFailed)
    .attach_printable(format!(
        "Base64 decoding failed for all engines. Errors: {:?}",
        error_stack
    ))
}

pub fn deserialize_zero_minor_amount_as_none<'de, D>(
    deserializer: D,
) -> Result<Option<MinorUnit>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let amount = Option::<MinorUnit>::deserialize(deserializer)?;
    match amount {
        Some(value) if value.get_amount_as_i64() == 0 => Ok(None),
        _ => Ok(amount),
    }
}

pub fn convert_uppercase<'de, D, T>(v: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Debug + std::fmt::Display + std::error::Error,
{
    use serde::de::Error;
    let output = <&str>::deserialize(v)?;
    output.to_uppercase().parse::<T>().map_err(D::Error::custom)
}

pub trait SplitPaymentData {
    fn get_split_payment_data(&self)
        -> Option<domain_types::connector_types::SplitPaymentsRequest>;
}

impl SplitPaymentData for PaymentsCaptureData {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        None
    }
}

impl<T: PaymentMethodDataTypes> SplitPaymentData for PaymentsAuthorizeData<T> {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        self.split_payments.clone()
    }
}

impl<T: PaymentMethodDataTypes> SplitPaymentData for RepeatPaymentData<T> {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        self.split_payments.clone()
    }
}

impl SplitPaymentData for PaymentsSyncData {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        self.split_payments.clone()
    }
}

impl SplitPaymentData for PaymentVoidData {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        None
    }
}

impl<T: PaymentMethodDataTypes> SplitPaymentData for SetupMandateRequestData<T> {
    fn get_split_payment_data(
        &self,
    ) -> Option<domain_types::connector_types::SplitPaymentsRequest> {
        None
    }
}

pub fn serialize_to_xml_string_with_root<T: Serialize>(
    root_name: &str,
    data: &T,
) -> Result<String, Error> {
    let xml_content = quick_xml::se::to_string_with_root(root_name, data)
        .change_context(errors::ConnectorError::RequestEncodingFailed)
        .attach_printable("Failed to serialize XML with root")?;

    let full_xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>{xml_content}");
    Ok(full_xml)
}

pub fn get_error_code_error_message_based_on_priority(
    connector: impl ConnectorErrorTypeMapping,
    error_list: Vec<ErrorCodeAndMessage>,
) -> Option<ErrorCodeAndMessage> {
    let error_type_list = error_list
        .iter()
        .map(|error| {
            connector
                .get_connector_error_type(error.error_code.clone(), error.error_message.clone())
        })
        .collect::<Vec<ConnectorErrorType>>();
    let mut error_zip_list = error_list
        .iter()
        .zip(error_type_list.iter())
        .collect::<Vec<(&ErrorCodeAndMessage, &ConnectorErrorType)>>();
    error_zip_list.sort_by_key(|&(_, error_type)| error_type);
    error_zip_list
        .first()
        .map(|&(error_code_message, _)| error_code_message)
        .cloned()
}

pub trait ConnectorErrorTypeMapping {
    fn get_connector_error_type(
        &self,
        _error_code: String,
        _error_message: String,
    ) -> ConnectorErrorType {
        ConnectorErrorType::UnknownError
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorCodeAndMessage {
    pub error_code: String,
    pub error_message: String,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
//Priority of connector_error_type
pub enum ConnectorErrorType {
    UserError = 2,
    BusinessError = 3,
    TechnicalError = 4,
    UnknownError = 1,
}

pub(crate) fn to_connector_meta<T>(connector_meta: Option<Value>) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    let json = connector_meta.ok_or_else(missing_field_err("connector_meta_data"))?;
    json.parse_value(std::any::type_name::<T>()).switch()
}

pub trait MultipleCaptureSyncResponse {
    fn get_connector_capture_id(&self) -> String;
    fn get_capture_attempt_status(&self) -> common_enums::AttemptStatus;
    fn is_capture_response(&self) -> bool;
    fn get_connector_reference_id(&self) -> Option<String> {
        None
    }
    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, Report<ParsingError>>;
}

pub(crate) fn construct_captures_response_hashmap<T>(
    capture_sync_response_list: Vec<T>,
) -> CustomResult<HashMap<String, CaptureSyncResponse>, errors::ConnectorError>
where
    T: MultipleCaptureSyncResponse,
{
    let mut hashmap = HashMap::new();
    for capture_sync_response in capture_sync_response_list {
        let connector_capture_id = capture_sync_response.get_connector_capture_id();
        if capture_sync_response.is_capture_response() {
            hashmap.insert(
                connector_capture_id.clone(),
                CaptureSyncResponse::Success {
                    resource_id: ResponseId::ConnectorTransactionId(connector_capture_id),
                    status: capture_sync_response.get_capture_attempt_status(),
                    connector_response_reference_id: capture_sync_response
                        .get_connector_reference_id(),
                    amount: capture_sync_response
                        .get_amount_captured()
                        .change_context(errors::ConnectorError::AmountConversionFailed)
                        .attach_printable(
                            "failed to convert back captured response amount to minor unit",
                        )?,
                },
            );
        }
    }

    Ok(hashmap)
}

pub(crate) fn is_manual_capture(capture_method: Option<enums::CaptureMethod>) -> bool {
    capture_method == Some(enums::CaptureMethod::Manual)
        || capture_method == Some(enums::CaptureMethod::ManualMultiple)
}

pub fn get_token_expiry_month_year_2_digit_with_delimiter(
    month: Secret<String>,
    year: Secret<String>,
) -> Secret<String> {
    let year_2_digit = if year.peek().len() == 4 {
        Secret::new(year.peek().chars().skip(2).collect::<String>())
    } else {
        year
    };
    Secret::new(format!("{}/{}", month.peek(), year_2_digit.peek()))
}

/// Common merchant-defined information structure for Cybersource-based connectors
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantDefinedInformation {
    pub key: u8,
    pub value: String,
}

/// Converts metadata JSON to merchant-defined information format
///
/// Used by Cybersource-based connectors (Barclaycard, Cybersource) to send custom merchant metadata.
/// The silent failure (unwrap_or_default) is intentional:
/// - Metadata is optional and non-critical for payment processing
/// - Input is already valid JSON (serde_json::Value), so parsing rarely fails
/// - Better to continue payment without metadata than to fail the entire payment
pub fn convert_metadata_to_merchant_defined_info(
    metadata: Value,
) -> Vec<MerchantDefinedInformation> {
    serde_json::from_str::<std::collections::BTreeMap<String, Value>>(&metadata.to_string())
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(index, (key, value))| {
            u8::try_from(index + 1)
                .ok()
                .map(|key_num| MerchantDefinedInformation {
                    key: key_num,
                    value: format!("{key}={value}"),
                })
        })
        .collect()
}

/// Convert state/province to 2-letter code based on country
/// Returns None if the state is already 2 letters or if conversion is not needed
/// Returns Some(code) if successfully converted from full name to abbreviation
pub fn get_state_code_for_country(
    state: &Secret<String>,
    country: Option<common_enums::CountryAlpha2>,
) -> Option<Secret<String>> {
    let state_str = state.peek();

    // If already 2 letters, return as-is (already a code)
    if state_str.len() == 2 {
        Some(state.clone())
    } else if state_str.is_empty() {
        // If empty, return None
        None
    } else {
        // Convert based on country
        match country {
            Some(common_enums::CountryAlpha2::US) => {
                // Try to convert US state name to abbreviation
                common_enums::UsStatesAbbreviation::from_state_name(state_str)
                    .map(|abbr| Secret::new(abbr.to_string()))
            }
            Some(common_enums::CountryAlpha2::CA) => {
                // Try to convert Canada province name to abbreviation
                common_enums::CanadaStatesAbbreviation::from_province_name(state_str)
                    .map(|abbr| Secret::new(abbr.to_string()))
            }
            _ => {
                // For other countries, return the state as-is if it's not empty
                Some(state.clone())
            }
        }
    }
}

/// Utility function for collecting and sorting values from JSON for webhook signature verification.
///
/// Recursively collects all values from a JSON structure, excluding a specific signature field,
/// sorts them alphabetically, and returns them joined with "/" separator.
///
/// # Arguments
/// * `value` - The JSON value to process
/// * `signature` - The signature value to exclude from collection
///
/// # Returns
/// Sorted vector of all values (excluding the signature)
pub fn collect_and_sort_values_by_removing_signature(
    value: &Value,
    signature: &str,
) -> Vec<String> {
    let mut values = collect_values_by_removing_signature(value, signature);
    values.sort();
    values
}

fn collect_values_by_removing_signature(value: &Value, signature: &str) -> Vec<String> {
    match value {
        Value::Null => vec!["null".to_owned()],
        Value::Bool(b) => vec![b.to_string()],
        Value::Number(n) => match n.as_f64() {
            Some(f) => vec![format!("{f:.2}")],
            None => vec![n.to_string()],
        },
        Value::String(s) => {
            if signature == s {
                vec![]
            } else {
                vec![s.clone()]
            }
        }
        Value::Array(arr) => arr
            .iter()
            .flat_map(|v| collect_values_by_removing_signature(v, signature))
            .collect(),
        Value::Object(obj) => obj
            .values()
            .flat_map(|v| collect_values_by_removing_signature(v, signature))
            .collect(),
    }
}
