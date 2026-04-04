use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use base64::Engine;
use common_enums::{CurrencyUnit, PaymentMethodType};
use common_utils::{consts, metadata::MaskedMetadata, AmountConvertor, CustomResult, MinorUnit};
use error_stack::{report, Result, ResultExt};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use time::PrimitiveDateTime;

use crate::{
    errors::{
        self, ConnectorResponseTransformationError, IntegrationError, IntegrationErrorContext,
        ParsingError,
    },
    payment_method_data::{Card, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ErrorResponse,
    router_response_types::Response,
    types::PaymentMethodDataType,
};

pub type Error = error_stack::Report<errors::IntegrationError>;

/// Trait for converting from one foreign type to another
pub trait ForeignTryFrom<F>: Sized {
    /// Custom error for conversion failure
    type Error;

    /// Convert from a foreign type to the current type and return an error if the conversion fails
    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

pub trait ForeignFrom<F>: Sized {
    /// Convert from a foreign type to the current type and return an error if the conversion fails
    fn foreign_from(from: F) -> Self;
}

pub trait ValueExt {
    /// Convert `serde_json::Value` into type `<T>` by using `serde::Deserialize`
    fn parse_value<T>(self, type_name: &'static str) -> Result<T, ParsingError>
    where
        T: serde::de::DeserializeOwned;
}

impl ValueExt for Value {
    fn parse_value<T>(self, type_name: &'static str) -> Result<T, ParsingError>
    where
        T: serde::de::DeserializeOwned,
    {
        let debug = format!(
            "Unable to parse {type_name} from serde_json::Value: {:?}",
            &self
        );
        serde_json::from_value::<T>(self)
            .change_context(ParsingError::StructParseFailure(type_name))
            .attach_printable_lazy(|| debug)
    }
}

pub trait Encode<'e>
where
    Self: 'e + std::fmt::Debug,
{
    fn encode_to_value(&'e self) -> Result<Value, ParsingError>
    where
        Self: Serialize;
}

impl<'e, A> Encode<'e> for A
where
    Self: 'e + std::fmt::Debug,
{
    fn encode_to_value(&'e self) -> Result<Value, ParsingError>
    where
        Self: Serialize,
    {
        serde_json::to_value(self)
            .change_context(ParsingError::EncodeError("json-value"))
            .attach_printable_lazy(|| format!("Unable to convert {self:?} to a value"))
    }
}

pub fn handle_json_response_deserialization_failure(
    res: Response,
    _: &'static str,
) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
    let status = res.status_code;
    let response_data = String::from_utf8(res.response.to_vec())
        .change_context(ConnectorResponseTransformationError::response_handling_failed(status))?;

    // check for whether the response is in json format
    match serde_json::from_str::<Value>(&response_data) {
        // in case of unexpected response but in json format
        Ok(_) => Err(ConnectorResponseTransformationError::response_handling_failed(status))?,
        // in case of unexpected response but in html or string format
        Err(_) => Ok(ErrorResponse {
            status_code: res.status_code,
            code: consts::NO_ERROR_CODE.to_string(),
            message: consts::UNSUPPORTED_ERROR_MESSAGE.to_string(),
            reason: Some(response_data.clone()),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        }),
    }
}

pub fn generate_random_bytes(length: usize) -> Vec<u8> {
    // returns random bytes of length n
    let mut rng = rand::thread_rng();
    (0..length).map(|_| rand::Rng::gen(&mut rng)).collect()
}

pub fn missing_field_err(
    message: &'static str,
) -> Box<dyn Fn() -> error_stack::Report<errors::IntegrationError> + 'static> {
    Box::new(move || {
        errors::IntegrationError::MissingRequiredField {
            field_name: message,
            context: Default::default(),
        }
        .into()
    })
}

pub fn construct_not_supported_error_report(
    capture_method: common_enums::CaptureMethod,
    connector_name: &'static str,
) -> error_stack::Report<errors::IntegrationError> {
    errors::IntegrationError::NotSupported {
        message: capture_method.to_string(),
        connector: connector_name,
        context: Default::default(),
    }
    .into()
}

pub fn to_currency_base_unit_with_zero_decimal_check(
    amount: i64,
    currency: common_enums::Currency,
) -> core::result::Result<String, error_stack::Report<IntegrationError>> {
    currency
        .to_currency_base_unit_with_zero_decimal_check(amount)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })
}

pub fn get_timestamp_in_milliseconds(datetime: &PrimitiveDateTime) -> i64 {
    let utc_datetime = datetime.assume_utc();
    utc_datetime.unix_timestamp() * 1000
}

pub fn get_amount_as_string(
    currency_unit: &CurrencyUnit,
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> core::result::Result<String, error_stack::Report<IntegrationError>> {
    let amount = match currency_unit {
        CurrencyUnit::Minor => amount.get_amount_as_i64().to_string(),
        CurrencyUnit::Base => to_currency_base_unit(amount, currency)?,
    };
    Ok(amount)
}

pub fn base64_decode(
    data: String,
) -> core::result::Result<Vec<u8>, error_stack::Report<ConnectorResponseTransformationError>> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .change_context(
            ConnectorResponseTransformationError::response_handling_failed_http_status_unknown(),
        )
}

pub fn to_currency_base_unit(
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> core::result::Result<String, error_stack::Report<IntegrationError>> {
    currency
        .to_currency_base_unit(amount.get_amount_as_i64())
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "amount",
            context: Default::default(),
        })
}

pub const SELECTED_PAYMENT_METHOD: &str = "Selected payment method";

pub fn get_unimplemented_payment_method_error_message(connector: &str) -> String {
    format!("{SELECTED_PAYMENT_METHOD} through {connector}")
}

pub fn get_header_key_value<'a>(
    key: &str,
    headers: &'a actix_web::http::header::HeaderMap,
) -> CustomResult<&'a str, errors::IntegrationError> {
    get_header_field(headers.get(key))
}

pub fn get_http_header<'a>(
    key: &str,
    headers: &'a http::HeaderMap,
) -> CustomResult<&'a str, errors::IntegrationError> {
    get_header_field(headers.get(key))
}

fn get_header_field(
    field: Option<&http::HeaderValue>,
) -> CustomResult<&str, errors::IntegrationError> {
    field
        .map(|header_value| {
            header_value
                .to_str()
                .change_context(errors::IntegrationError::InvalidDataFormat {
                    field_name: "header",
                    context: Default::default(),
                })
        })
        .ok_or(report!(errors::IntegrationError::MissingRequiredField {
            field_name: "header",
            context: Default::default()
        }))?
}

pub fn is_payment_failure(status: common_enums::AttemptStatus) -> bool {
    match status {
        common_enums::AttemptStatus::AuthenticationFailed
        | common_enums::AttemptStatus::AuthorizationFailed
        | common_enums::AttemptStatus::CaptureFailed
        | common_enums::AttemptStatus::VoidFailed
        | common_enums::AttemptStatus::Expired
        | common_enums::AttemptStatus::Failure => true,
        common_enums::AttemptStatus::Started
        | common_enums::AttemptStatus::RouterDeclined
        | common_enums::AttemptStatus::AuthenticationPending
        | common_enums::AttemptStatus::AuthenticationSuccessful
        | common_enums::AttemptStatus::Authorized
        | common_enums::AttemptStatus::Charged
        | common_enums::AttemptStatus::Authorizing
        | common_enums::AttemptStatus::CodInitiated
        | common_enums::AttemptStatus::Voided
        | common_enums::AttemptStatus::VoidedPostCapture
        | common_enums::AttemptStatus::VoidInitiated
        | common_enums::AttemptStatus::VoidPostCaptureInitiated
        | common_enums::AttemptStatus::PartiallyAuthorized
        | common_enums::AttemptStatus::CaptureInitiated
        | common_enums::AttemptStatus::AutoRefunded
        | common_enums::AttemptStatus::PartialCharged
        | common_enums::AttemptStatus::PartialChargedAndChargeable
        | common_enums::AttemptStatus::Unresolved
        | common_enums::AttemptStatus::Unspecified
        | common_enums::AttemptStatus::Pending
        | common_enums::AttemptStatus::PaymentMethodAwaited
        | common_enums::AttemptStatus::ConfirmationAwaited
        | common_enums::AttemptStatus::DeviceDataCollectionPending
        | common_enums::AttemptStatus::IntegrityFailure
        | common_enums::AttemptStatus::Unknown => false,
    }
}

pub fn get_card_details<T>(
    payment_method_data: PaymentMethodData<T>,
    connector_name: &'static str,
) -> Result<Card<T>, errors::IntegrationError>
where
    T: PaymentMethodDataTypes,
{
    match payment_method_data {
        PaymentMethodData::Card(details) => Ok(details),
        _ => Err(errors::IntegrationError::NotSupported {
            message: SELECTED_PAYMENT_METHOD.to_string(),
            connector: connector_name,
            context: Default::default(),
        })?,
    }
}

pub fn is_mandate_supported<T>(
    selected_pmd: PaymentMethodData<T>,
    payment_method_type: Option<PaymentMethodType>,
    mandate_implemented_pmds: HashSet<PaymentMethodDataType>,
    connector: &'static str,
) -> core::result::Result<(), Error>
where
    T: PaymentMethodDataTypes,
{
    if mandate_implemented_pmds.contains(&PaymentMethodDataType::from(selected_pmd.clone())) {
        Ok(())
    } else {
        match payment_method_type {
            Some(pm_type) => Err(errors::IntegrationError::NotSupported {
                message: format!("{pm_type} mandate payment"),
                connector,
                context: Default::default(),
            }
            .into()),
            None => Err(errors::IntegrationError::NotSupported {
                message: " mandate payment".to_string(),
                connector,
                context: Default::default(),
            }
            .into()),
        }
    }
}

pub fn convert_amount<T>(
    amount_convertor: &dyn AmountConvertor<Output = T>,
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> core::result::Result<T, error_stack::Report<errors::IntegrationError>> {
    amount_convertor.convert(amount, currency).change_context(
        errors::IntegrationError::AmountConversionFailed {
            context: Default::default(),
        },
    )
}

pub fn convert_amount_for_webhook<T>(
    amount_convertor: &dyn AmountConvertor<Output = T>,
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> core::result::Result<T, error_stack::Report<errors::WebhookError>> {
    amount_convertor.convert(amount, currency).map_err(|_| {
        error_stack::report!(errors::WebhookError::WebhookAmountConversionFailed {
            reason: format!(
                "Failed to convert amount from minor units: amount={}, currency={}",
                amount.get_amount_as_i64(),
                currency
            ),
        })
    })
}

pub fn convert_back_amount_to_minor_units_for_webhook<T>(
    amount_convertor: &dyn AmountConvertor<Output = T>,
    amount: T,
    currency: common_enums::Currency,
) -> core::result::Result<MinorUnit, error_stack::Report<errors::WebhookError>> {
    amount_convertor
        .convert_back(amount, currency)
        .map_err(|_| {
            error_stack::report!(errors::WebhookError::WebhookAmountConversionFailed {
                reason: format!(
                    "Failed to convert amount to minor units: currency={}",
                    currency
                ),
            })
        })
}

pub fn convert_back_amount_to_minor_units<T>(
    amount_convertor: &dyn AmountConvertor<Output = T>,
    amount: T,
    currency: common_enums::Currency,
) -> core::result::Result<MinorUnit, error_stack::Report<common_utils::errors::ParsingError>> {
    amount_convertor.convert_back(amount, currency)
}

#[derive(Debug, Copy, Clone, strum::Display, Eq, Hash, PartialEq)]
pub enum CardIssuer {
    AmericanExpress,
    Master,
    Maestro,
    Visa,
    Discover,
    DinersClub,
    JCB,
    CarteBlanche,
    CartesBancaires,
    UnionPay,
}

// Helper function for extracting connector request reference ID
pub(crate) fn extract_connector_request_reference_id(identifier: &Option<String>) -> String {
    identifier.clone().unwrap_or_default()
}

#[track_caller]
pub fn get_card_issuer(
    card_number: &str,
) -> core::result::Result<CardIssuer, error_stack::Report<IntegrationError>> {
    for (k, v) in CARD_REGEX.iter() {
        let regex: Regex = v
            .clone()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        if regex.is_match(card_number) {
            return Ok(*k);
        }
    }
    Err(error_stack::Report::new(IntegrationError::not_implemented(
        "Card Type",
    )))
}

static CARD_REGEX: LazyLock<HashMap<CardIssuer, core::result::Result<Regex, regex::Error>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        // Reference: https://gist.github.com/michaelkeevildown/9096cd3aac9029c4e6e05588448a8841
        // [#379]: Determine card issuer from card BIN number
        map.insert(CardIssuer::Master, Regex::new(r"^5[1-5][0-9]{14}$"));
        map.insert(CardIssuer::AmericanExpress, Regex::new(r"^3[47][0-9]{13}$"));
        map.insert(CardIssuer::Visa, Regex::new(r"^4[0-9]{12}(?:[0-9]{3})?$"));
        map.insert(CardIssuer::Discover, Regex::new(r"^65[4-9][0-9]{13}|64[4-9][0-9]{13}|6011[0-9]{12}|(622(?:12[6-9]|1[3-9][0-9]|[2-8][0-9][0-9]|9[01][0-9]|92[0-5])[0-9]{10})$"));
        map.insert(
            CardIssuer::Maestro,
            Regex::new(r"^(5018|5020|5038|5893|6304|6759|6761|6762|6763)[0-9]{8,15}$"),
        );
        map.insert(
            CardIssuer::DinersClub,
            Regex::new(r"^3(?:0[0-5]|[68][0-9])[0-9]{11}$"),
        );
        map.insert(
            CardIssuer::JCB,
            Regex::new(r"^(3(?:088|096|112|158|337|5(?:2[89]|[3-8][0-9]))\d{12})$"),
        );
        map.insert(CardIssuer::CarteBlanche, Regex::new(r"^389[0-9]{11}$"));
        map
    });

/// Helper function for extracting merchant ID from metadata.
///
/// Uses the shared `merchant_id_or_default` fallback: if the `x-merchant-id`
/// header is missing, a default ID is auto-generated.
pub fn extract_merchant_id_from_metadata(
    metadata: &MaskedMetadata,
) -> Result<common_utils::id_type::MerchantId, IntegrationError> {
    let merchant_id_str = common_utils::metadata::merchant_id_or_default(
        metadata.get_raw(consts::X_MERCHANT_ID).as_deref(),
    );
    Ok(merchant_id_str
        .parse::<common_utils::id_type::MerchantId>()
        .map_err(|e| IntegrationError::InvalidDataFormat {
            field_name: "merchant_id",
            context: IntegrationErrorContext {
                additional_context: Some(format!("Failed to parse merchant ID from header: {e}")),
                ..Default::default()
            },
        })?)
}

/// Convert US state names to their 2-letter abbreviations
pub fn convert_us_state_to_code(state: &str) -> String {
    // If already 2 characters, assume it's already an abbreviation
    if state.len() == 2 {
        return state.to_uppercase();
    }

    // Convert full state names to abbreviations (case-insensitive)
    match state.to_lowercase().trim() {
        "alabama" => "AL".to_string(),
        "alaska" => "AK".to_string(),
        "american samoa" => "AS".to_string(),
        "arizona" => "AZ".to_string(),
        "arkansas" => "AR".to_string(),
        "california" => "CA".to_string(),
        "colorado" => "CO".to_string(),
        "connecticut" => "CT".to_string(),
        "delaware" => "DE".to_string(),
        "district of columbia" | "columbia" => "DC".to_string(),
        "federated states of micronesia" | "micronesia" => "FM".to_string(),
        "florida" => "FL".to_string(),
        "georgia" => "GA".to_string(),
        "guam" => "GU".to_string(),
        "hawaii" => "HI".to_string(),
        "idaho" => "ID".to_string(),
        "illinois" => "IL".to_string(),
        "indiana" => "IN".to_string(),
        "iowa" => "IA".to_string(),
        "kansas" => "KS".to_string(),
        "kentucky" => "KY".to_string(),
        "louisiana" => "LA".to_string(),
        "maine" => "ME".to_string(),
        "marshall islands" => "MH".to_string(),
        "maryland" => "MD".to_string(),
        "massachusetts" => "MA".to_string(),
        "michigan" => "MI".to_string(),
        "minnesota" => "MN".to_string(),
        "mississippi" => "MS".to_string(),
        "missouri" => "MO".to_string(),
        "montana" => "MT".to_string(),
        "nebraska" => "NE".to_string(),
        "nevada" => "NV".to_string(),
        "new hampshire" => "NH".to_string(),
        "new jersey" => "NJ".to_string(),
        "new mexico" => "NM".to_string(),
        "new york" => "NY".to_string(),
        "north carolina" => "NC".to_string(),
        "north dakota" => "ND".to_string(),
        "northern mariana islands" => "MP".to_string(),
        "ohio" => "OH".to_string(),
        "oklahoma" => "OK".to_string(),
        "oregon" => "OR".to_string(),
        "palau" => "PW".to_string(),
        "pennsylvania" => "PA".to_string(),
        "puerto rico" => "PR".to_string(),
        "rhode island" => "RI".to_string(),
        "south carolina" => "SC".to_string(),
        "south dakota" => "SD".to_string(),
        "tennessee" => "TN".to_string(),
        "texas" => "TX".to_string(),
        "utah" => "UT".to_string(),
        "vermont" => "VT".to_string(),
        "virgin islands" => "VI".to_string(),
        "virginia" => "VA".to_string(),
        "washington" => "WA".to_string(),
        "west virginia" => "WV".to_string(),
        "wisconsin" => "WI".to_string(),
        "wyoming" => "WY".to_string(),
        // If no match found, return original (might be international or invalid)
        _ => state.to_string(),
    }
}

/// Convert Canadian province/territory names to their 2-letter abbreviations
pub fn convert_canada_state_to_code(state: &str) -> String {
    // If already 2 characters, assume it's already an abbreviation
    if state.len() == 2 {
        return state.to_uppercase();
    }

    // Convert full province/territory names to abbreviations (case-insensitive)
    match state.to_lowercase().trim() {
        "alberta" => "AB".to_string(),
        "british columbia" => "BC".to_string(),
        "manitoba" => "MB".to_string(),
        "new brunswick" => "NB".to_string(),
        "newfoundland and labrador" | "newfoundland" => "NL".to_string(),
        "northwest territories" => "NT".to_string(),
        "nova scotia" => "NS".to_string(),
        "nunavut" => "NU".to_string(),
        "ontario" => "ON".to_string(),
        "prince edward island" => "PE".to_string(),
        "quebec" | "québec" => "QC".to_string(),
        "saskatchewan" => "SK".to_string(),
        "yukon" => "YT".to_string(),
        // If no match found, return original
        _ => state.to_string(),
    }
}

/// Convert Spanish autonomous community/province names to their 2-letter ISO 3166-2:ES codes
///
/// # Arguments
/// * `state` - The state/province name or code to convert
///
/// # Returns
/// * `Ok(String)` - The 2-letter state code
/// * `Err(IntegrationError)` - If the state cannot be mapped
pub fn convert_spain_state_to_code(state: &str) -> Result<String, crate::errors::IntegrationError> {
    // If already 2 characters, assume it's already an abbreviation
    if state.len() == 2 {
        return Ok(state.to_uppercase());
    }

    match state.to_lowercase().trim() {
        "acoruna" | "lacoruna" | "esc" => Ok("C".to_string()),
        "alacant" | "esa" | "alicante" => Ok("A".to_string()),
        "albacete" | "esab" => Ok("AB".to_string()),
        "almeria" | "esal" => Ok("AL".to_string()),
        "andalucia" | "esan" => Ok("AN".to_string()),
        "araba" | "esvi" => Ok("VI".to_string()),
        "aragon" | "esar" => Ok("AR".to_string()),
        "asturias" | "eso" => Ok("O".to_string()),
        "asturiasprincipadode" | "principadodeasturias" | "esas" => Ok("AS".to_string()),
        "badajoz" | "esba" => Ok("BA".to_string()),
        "barcelona" | "esb" => Ok("B".to_string()),
        "bizkaia" | "esbi" => Ok("BI".to_string()),
        "burgos" | "esbu" => Ok("BU".to_string()),
        "canarias" | "escn" => Ok("CN".to_string()),
        "cantabria" | "ess" => Ok("S".to_string()),
        "castello" | "escs" => Ok("CS".to_string()),
        "castellon" => Ok("C".to_string()),
        "castillayleon" | "escl" => Ok("CL".to_string()),
        "castillalamancha" | "escm" => Ok("CM".to_string()),
        "cataluna" | "catalunya" | "esct" => Ok("CT".to_string()),
        "ceuta" | "esce" => Ok("CE".to_string()),
        "ciudadreal" | "escr" | "ciudad" => Ok("CR".to_string()),
        "cuenca" | "escu" => Ok("CU".to_string()),
        "caceres" | "escc" => Ok("CC".to_string()),
        "cadiz" | "esca" => Ok("CA".to_string()),
        "cordoba" | "esco" => Ok("CO".to_string()),
        "euskalherria" | "espv" => Ok("PV".to_string()),
        "extremadura" | "esex" => Ok("EX".to_string()),
        "galicia" | "esga" => Ok("GA".to_string()),
        "gipuzkoa" | "esss" => Ok("SS".to_string()),
        "girona" | "esgi" | "gerona" => Ok("GI".to_string()),
        "granada" | "esgr" => Ok("GR".to_string()),
        "guadalajara" | "esgu" => Ok("GU".to_string()),
        "huelva" | "esh" => Ok("H".to_string()),
        "huesca" | "eshu" => Ok("HU".to_string()),
        "illesbalears" | "islasbaleares" | "espm" => Ok("PM".to_string()),
        "esib" => Ok("IB".to_string()),
        "jaen" | "esj" => Ok("J".to_string()),
        "larioja" | "eslo" => Ok("LO".to_string()),
        "esri" => Ok("RI".to_string()),
        "laspalmas" | "palmas" | "esgc" => Ok("GC".to_string()),
        "leon" => Ok("LE".to_string()),
        "lleida" | "lerida" | "esl" => Ok("L".to_string()),
        "lugo" | "eslu" => Ok("LU".to_string()),
        "madrid" | "esm" => Ok("M".to_string()),
        "comunidaddemadrid" | "madridcomunidadde" | "esmd" => Ok("MD".to_string()),
        "melilla" | "esml" => Ok("ML".to_string()),
        "murcia" | "esmu" => Ok("MU".to_string()),
        "murciaregionde" | "regiondemurcia" | "esmc" => Ok("MC".to_string()),
        "malaga" | "esma" => Ok("MA".to_string()),
        "nafarroa" | "esnc" => Ok("NC".to_string()),
        "nafarroakoforukomunitatea" | "esna" => Ok("NA".to_string()),
        "navarra" => Ok("NA".to_string()),
        "navarracomunidadforalde" | "comunidadforaldenavarra" => Ok("NC".to_string()),
        "ourense" | "orense" | "esor" => Ok("OR".to_string()),
        "palencia" | "esp" => Ok("P".to_string()),
        "paisvasco" => Ok("PV".to_string()),
        "pontevedra" | "espo" => Ok("PO".to_string()),
        "salamanca" | "essa" => Ok("SA".to_string()),
        "santacruzdetenerife" | "estf" => Ok("TF".to_string()),
        "segovia" | "essg" => Ok("SG".to_string()),
        "sevilla" | "esse" => Ok("SE".to_string()),
        "soria" | "esso" => Ok("SO".to_string()),
        "tarragona" | "est" => Ok("T".to_string()),
        "teruel" | "este" => Ok("TE".to_string()),
        "toledo" | "esto" => Ok("TO".to_string()),
        "valencia" | "esv" => Ok("V".to_string()),
        "valencianacomunidad" | "esvc" => Ok("VC".to_string()),
        "valencianacomunitat" => Ok("V".to_string()),
        "valladolid" | "esva" => Ok("VA".to_string()),
        "zamora" | "esza" => Ok("ZA".to_string()),
        "zaragoza" | "esz" => Ok("Z".to_string()),
        "alava" => Ok("VI".to_string()),
        "avila" | "esav" => Ok("AV".to_string()),
        _ => Err(errors::IntegrationError::InvalidDataFormat {
            field_name: "address.state",
            context: Default::default(),
        })?,
    }
}
