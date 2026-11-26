use base64::Engine;
use domain_types::{errors, router_data::ConnectorSpecificConfig};
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

const BASIC_AUTH_SCHEME: &str = "Basic";
const EMPTY_FIELD: &str = "";
const NO_ADDITIONAL_AMOUNT: i64 = 0;
const PAYMENT_NOTIFICATION_DISABLED: u8 = 0;
const PAYMENT_NOTIFICATION_EMAIL: u8 = 1;
const METADATA_VERSION: u8 = 1;
const REFUND_QUERY_ID_DOMAIN: &[u8] = b"paynamics-workflow-refund-query:";
const QUERY_ID_DOMAIN: &[u8] = b"paynamics-workflow-query:";
const QUERY_ID_PREFIX: &str = "q";
const QUERY_ID_DIGEST_LENGTH: usize = 31;

#[derive(Debug, Clone)]
pub struct PaynamicsAuthType {
    pub merchant_id: Secret<String>,
    pub merchant_key: Secret<String>,
    pub username: Secret<String>,
    pub password: Secret<String>,
}

impl PaynamicsAuthType {
    pub fn generate_authorization_header(&self) -> String {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        format!(
            "{BASIC_AUTH_SCHEME} {}",
            base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
        )
    }
}

impl TryFrom<&ConnectorSpecificConfig> for PaynamicsAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Paynamics {
                merchant_id,
                merchant_key,
                username,
                password,
                ..
            } => {
                for (field_name, value) in [
                    ("connector_config.merchant_id", merchant_id.peek()),
                    ("connector_config.merchant_key", merchant_key.peek()),
                    ("connector_config.username", username.peek()),
                    ("connector_config.password", password.peek()),
                ] {
                    if value.is_empty() {
                        return Err(domain_types::utils::missing_field_err(field_name)());
                    }
                }
                Ok(Self {
                    merchant_id: merchant_id.clone(),
                    merchant_key: merchant_key.clone(),
                    username: username.clone(),
                    password: password.clone(),
                })
            }
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

/// Authentication and transport errors may omit all payment identifiers.
#[derive(Debug, Serialize, Deserialize)]
pub struct PaynamicsErrorResponse {
    pub merchant_id: Option<Secret<String>>,
    pub request_id: Option<String>,
    pub response_id: Option<String>,
    pub response_code: Option<String>,
    pub response_message: Option<String>,
    pub response_advise: Option<String>,
}

/// Production has no default binding: an explicit HTTPS REST base is required.
pub fn validate_base_url(
    base_url: &str,
) -> Result<(), error_stack::Report<errors::IntegrationError>> {
    let parsed =
        url::Url::parse(base_url).map_err(|_| errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_config.base_url",
            context: Default::default(),
        })?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().ends_with('/')
        || parsed.path().to_ascii_lowercase().contains(".asmx")
    {
        return Err(errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_config.base_url",
            context: Default::default(),
        }
        .into());
    }
    Ok(())
}

use super::PaynamicsRouterData;
use crate::types::ResponseRouterData;
use common_enums::{AttemptStatus, CaptureMethod};
use common_utils::types::{MinorUnit, StringMajorUnit};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData, ResponseId,
        RefundFlowData, RefundSyncData, RefundsResponseData,
    },
    payment_method_data::{
        BankTransferData, PaymentMethodData, PaymentMethodDataTypes, VoucherData,
    },
    router_data::{ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_request_types::{AuthoriseIntegrityObject, PaymentSynIntegrityObject},
    router_response_types::RedirectForm,
};
use sha2::{Digest, Sha512};
use std::fmt::Debug;

type RequestError = error_stack::Report<errors::IntegrationError>;
fn invalid(field_name: &'static str) -> RequestError {
    errors::IntegrationError::InvalidDataFormat {
        field_name,
        context: Default::default(),
    }
    .into()
}

fn unsupported(message: &str) -> RequestError {
    errors::IntegrationError::NotSupported {
        message: message.to_owned(),
        connector: "paynamics",
        context: Default::default(),
    }
    .into()
}

fn signature_component(value: Option<&str>) -> &str {
    // Workflow §10.3: omitted optional signed fields contribute empty text,
    // never values copied from request context into the received signature.
    value.unwrap_or(EMPTY_FIELD)
}

/// Exact selected-channel routing. RCBC is mapped at the caller's request, but
/// Provider corrected its channel November 24; staging remains unavailable.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum PaynamicsPaymentMethod {
    #[serde(rename = "onlinebanktransfer")]
    OnlineBankTransfer,
    #[serde(rename = "bank_otc")]
    BankOtc,
    #[serde(rename = "nonbank_otc")]
    NonbankOtc,
}
impl PaynamicsPaymentMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::OnlineBankTransfer => "onlinebanktransfer",
            Self::BankOtc => "bank_otc",
            Self::NonbankOtc => "nonbank_otc",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum PaynamicsChannel {
    #[serde(rename = "bpi_online")]
    Bpi,
    #[serde(rename = "ubp_online")]
    Unionbank,
    #[serde(rename = "br_landbank_ph")]
    Landbank,
    #[serde(rename = "rcbc_digital_ph")]
    Rcbc,
    #[serde(rename = "rbank_ph")]
    Robinsons,
    #[serde(rename = "dp_ph")]
    Dragonpay,
}
impl PaynamicsChannel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bpi => "bpi_online",
            Self::Unionbank => "ubp_online",
            Self::Landbank => "br_landbank_ph",
            Self::Rcbc => "rcbc_digital_ph",
            Self::Robinsons => "rbank_ph",
            Self::Dragonpay => "dp_ph",
        }
    }
    fn method(self) -> PaynamicsMethod {
        let (pmethod, payment_action) = match self {
            Self::Bpi | Self::Unionbank | Self::Landbank | Self::Rcbc => (
                PaynamicsPaymentMethod::OnlineBankTransfer,
                PaynamicsPaymentAction::UrlLink,
            ),
            Self::Robinsons => (
                PaynamicsPaymentMethod::BankOtc,
                PaynamicsPaymentAction::DirectOtc,
            ),
            Self::Dragonpay => (
                PaynamicsPaymentMethod::NonbankOtc,
                PaynamicsPaymentAction::DirectOtc,
            ),
        };
        PaynamicsMethod {
            pmethod,
            pchannel: self,
            payment_action,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum PaynamicsPaymentAction {
    #[serde(rename = "url_link")]
    UrlLink,
    #[serde(rename = "direct_otc")]
    DirectOtc,
}
impl PaynamicsPaymentAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::UrlLink => "url_link",
            Self::DirectOtc => "direct_otc",
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
enum PaynamicsCollectionMethod {
    #[serde(rename = "single_pay")]
    SinglePay,
}
impl PaynamicsCollectionMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::SinglePay => "single_pay",
        }
    }
}

// These enums represent values chosen by this connector, not vendor statuses.
// Parse the versioned caller-retained carrier strictly so unsupported method
// values can never become serializable Unknown request values.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PaynamicsMethod {
    pub pmethod: PaynamicsPaymentMethod,
    pub pchannel: PaynamicsChannel,
    pub payment_action: PaynamicsPaymentAction,
}
impl<'de> Deserialize<'de> for PaynamicsMethod {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct CarrierMethod {
            pmethod: String,
            pchannel: String,
            payment_action: String,
        }
        let carrier = CarrierMethod::deserialize(deserializer)?;
        [
            PaynamicsChannel::Bpi,
            PaynamicsChannel::Unionbank,
            PaynamicsChannel::Landbank,
            PaynamicsChannel::Rcbc,
            PaynamicsChannel::Robinsons,
            PaynamicsChannel::Dragonpay,
        ]
        .into_iter()
        .map(PaynamicsChannel::method)
        .find(|method| {
            method.pmethod.as_str() == carrier.pmethod
                && method.pchannel.as_str() == carrier.pchannel
                && method.payment_action.as_str() == carrier.payment_action
        })
        .ok_or_else(|| serde::de::Error::custom("paynamics: unsupported carrier method"))
    }
}
pub fn payment_method<T: PaymentMethodDataTypes>(
    data: &PaymentMethodData<T>,
) -> Result<PaynamicsMethod, RequestError> {
    let channel = match data {
        PaymentMethodData::BankTransfer(bank) => match bank.as_ref() {
            BankTransferData::LocalBankTransfer { bank_code } => match bank_code.as_deref() {
                Some("bpi_online") => PaynamicsChannel::Bpi,
                Some("ubp_online") => PaynamicsChannel::Unionbank,
                Some("br_landbank_ph") => PaynamicsChannel::Landbank,
                Some("rcbc_digital_ph") => PaynamicsChannel::Rcbc,
                Some("rbank_ph") => PaynamicsChannel::Robinsons,
                None => {
                    return Err(domain_types::utils::missing_field_err(
                        "bank_transfer.local_bank_transfer.bank_code",
                    )())
                }
                Some(_) => return Err(invalid("bank_transfer.local_bank_transfer.bank_code")),
            },
            _ => return Err(unsupported("bank transfer variant is not supported")),
        },
        PaymentMethodData::Voucher(VoucherData::Dragonpay) => PaynamicsChannel::Dragonpay,
        _ => return Err(unsupported("payment method is not supported")),
    };
    Ok(channel.method())
}

/// SHA512 of UTF-8 field concatenation, not HMAC. Never log the preimage.
pub fn sign_fields(fields: &[&str], key: &Secret<String>) -> Secret<String> {
    let mut digest = Sha512::new();
    for field in fields {
        digest.update(field.as_bytes());
    }
    digest.update(key.peek().as_bytes());
    Secret::new(format!("{:x}", digest.finalize()))
}

/// Preserve exact decimal text from UCS conversion; pad only, never round.
pub fn format_f2(amount: &StringMajorUnit) -> Result<String, RequestError> {
    let value = amount.get_amount_as_string();
    // Workflow §10.1 F2: a whole-unit value has no fractional digits to pad.
    let (whole, fraction) = value.split_once('.').unwrap_or((&value, EMPTY_FIELD));
    if fraction.len() > 2
        || whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid("amount"));
    }
    Ok(format!("{whole}.{fraction:0<2}"))
}
/// Exact F2 without the shared converter's internal f64 conversion. Workflow
/// §10.3 hashes these same bytes. Reject currencies requiring greater precision.
/// Vectors: PHP 1001 => "10.01"; JPY 1001 => "1001.00"; KWD => error.
pub fn amount_f2(
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> Result<String, RequestError> {
    let minor = amount.get_amount_as_i64();
    let exponent = currency
        .number_of_digits_after_decimal_point()
        .map_err(|_| invalid("currency"))?;
    if minor < 0 || exponent > 2 {
        return Err(invalid("amount"));
    }
    let scaled = i128::from(minor) * 10i128.pow(u32::from(2 - exponent));
    let text = format!("{}.{:02}", scaled / 100, scaled % 100);
    let converted: StringMajorUnit =
        serde_json::from_value(serde_json::Value::String(text)).map_err(|_| invalid("amount"))?;
    format_f2(&converted)
}
pub fn validate_request_id(value: &str) -> Result<(), RequestError> {
    if value.is_empty() || value.chars().count() > 32 || value.chars().any(|c| c.is_control()) {
        return Err(invalid("request_id"));
    }
    Ok(())
}
pub fn sale_request_id(flow: &PaymentFlowData) -> Result<String, RequestError> {
    // Workflow §10.1 requires a merchant-generated reference; UCS supplies its
    // connector request reference when a separate merchant_request_id is absent.
    let id = flow
        .merchant_request_id
        .as_ref()
        .unwrap_or(&flow.connector_request_reference_id);
    validate_request_id(id)?;
    Ok(id.clone())
}
fn validate_url(value: &str, field: &'static str) -> Result<(), RequestError> {
    let url = url::Url::parse(value).map_err(|_| invalid(field))?;
    if value.len() > 255
        || !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(field));
    }
    Ok(())
}

// Every sensitive request value is a Secret: request/event serialization masks
// MID, customer data, addresses, signatures and all checkout input as a unit.
#[derive(Debug, Serialize)]
pub struct PaynamicsAuthorizeRequest {
    pub transaction: Secret<serde_json::Value>,
    pub customer_info: Secret<serde_json::Value>,
    pub billing_info: Secret<serde_json::Value>,
    pub order_details: Secret<serde_json::Value>,
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaynamicsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaynamicsAuthorizeRequest
{
    type Error = RequestError;
    fn try_from(
        item: PaynamicsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let req = &data.request;
        let flow = &data.resource_common_data;
        if !matches!(req.capture_method, None | Some(CaptureMethod::Automatic)) {
            return Err(unsupported(
                "capture_method: only automatic one-time sale is supported",
            ));
        }
        if req.mandate_id.is_some()
            || req.setup_mandate_details.is_some()
            || req.off_session == Some(true)
        {
            return Err(unsupported(
                "mandates and off-session payments are not supported",
            ));
        }
        let method = payment_method(&req.payment_method_data)?;
        let auth = PaynamicsAuthType::try_from(&data.connector_config)?;
        if auth.merchant_id.peek().len() > 32 {
            return Err(invalid("connector_config.merchant_id"));
        }
        let id = sale_request_id(flow)?;
        let amount = amount_f2(req.money.amount(), req.money.currency())?;
        let currency = req.money.currency().to_string();
        let notify = req
            .webhook_url
            .as_deref()
            .ok_or_else(domain_types::utils::missing_field_err("webhook_url"))?;
        validate_url(notify, "webhook_url")?;
        let return_url = req
            .router_return_url
            .as_deref()
            .or(flow.return_url.as_deref())
            // Workflow §10.1: return/cancel URLs are optional for direct_otc.
            .unwrap_or(EMPTY_FIELD);
        if !return_url.is_empty() {
            validate_url(return_url, "return_url")?;
        }
        if method.payment_action == PaynamicsPaymentAction::UrlLink && return_url.is_empty() {
            return Err(domain_types::utils::missing_field_err("return_url")());
        }
        let fname = flow.get_billing_first_name()?;
        let lname = flow.get_billing_last_name()?;
        for name in [fname.peek(), lname.peek()] {
            if name.is_empty()
                || name.chars().count() > 50
                || !name.chars().all(|c| {
                    c.is_ascii_alphabetic()
                        || matches!(c, 'ñ' | 'Ñ')
                        || matches!(c, ' ' | '\'' | '-' | '.')
                })
            {
                return Err(invalid("customer_name"));
            }
        }
        // Workflow §10.1/§10.3: absent conditional contact contributes empty text.
        let email = req
            .email
            .as_ref()
            .map(|e| e.peek().as_str())
            .unwrap_or(EMPTY_FIELD);
        if email.len() > 100 {
            return Err(invalid("email"));
        }
        let phone = flow
            .address
            .get_payment_method_billing()
            .and_then(|b| b.phone.as_ref())
            .and_then(|p| p.number.as_ref())
            .map(|p| p.peek().as_str())
            // Workflow §10.1/§10.3: absent conditional phone contributes empty text.
            .unwrap_or(EMPTY_FIELD);
        if phone.len() > 32 {
            return Err(invalid("customer_info.phone"));
        }
        let dob = req
            .customer_date_of_birth
            .as_ref()
            .map(|date| date.peek().to_string())
            // Workflow §10.1/§10.3: optional DOB contributes empty text when absent.
            .unwrap_or_default();
        let customer_signature = sign_fields(
            &[
                fname.peek(),
                lname.peek(),
                EMPTY_FIELD,
                email,
                phone,
                EMPTY_FIELD,
                &dob,
            ],
            &auth.merchant_key,
        );
        let collection_method = PaynamicsCollectionMethod::SinglePay;
        let notification_status = PAYMENT_NOTIFICATION_DISABLED.to_string();
        let notification_channel = PAYMENT_NOTIFICATION_EMAIL.to_string();
        let signature = sign_fields(
            &[
                auth.merchant_id.peek(),
                &id,
                notify,
                return_url,
                return_url,
                method.pmethod.as_str(),
                method.payment_action.as_str(),
                EMPTY_FIELD,
                collection_method.as_str(),
                EMPTY_FIELD,
                EMPTY_FIELD,
                EMPTY_FIELD,
                &amount,
                &currency,
                EMPTY_FIELD,
                &notification_status,
                &notification_channel,
            ],
            &auth.merchant_key,
        );
        let orders = flow
            .order_details
            .as_ref()
            .filter(|x| !x.is_empty())
            .ok_or_else(domain_types::utils::missing_field_err("order_details"))?;
        let mut lines = Vec::new();
        let mut subtotal = 0i64;
        for order in orders {
            if order.product_name.is_empty() || order.quantity == 0 {
                return Err(invalid("order_details"));
            }
            let total = order
                .amount
                .get_amount_as_i64()
                .checked_mul(i64::from(order.quantity))
                .ok_or_else(|| invalid("order_details.amount"))?;
            subtotal = subtotal
                .checked_add(total)
                .ok_or_else(|| invalid("order_details.amount"))?;
            lines.push(serde_json::json!({"itemname":order.product_name,"quantity":order.quantity.to_string(),"unitprice":amount_f2(order.amount, req.money.currency())?,"totalprice":amount_f2(MinorUnit::new(total),req.money.currency())?}));
        }
        // Workflow §10.1 order_details: absent UCS shipping_cost adds no charge;
        // the checked subtotal + shipping equality below rejects hidden costs.
        let shipping = req
            .shipping_cost
            .unwrap_or(MinorUnit::new(NO_ADDITIONAL_AMOUNT));
        if subtotal.checked_add(shipping.get_amount_as_i64())
            != Some(req.money.amount().get_amount_as_i64())
        {
            return Err(invalid("order_details.totalorderamount"));
        }
        let mut billing = serde_json::Map::new();
        if let Some(address) = flow
            .address
            .get_payment_method_billing()
            .and_then(|b| b.address.as_ref())
        {
            for (key, value, limit) in [
                ("billing_address1", address.line1.as_ref(), 100),
                ("billing_address2", address.line2.as_ref(), 100),
                ("billing_city", address.city.as_ref(), 30),
                ("billing_zip", address.zip.as_ref(), 12),
            ] {
                if let Some(value) = value {
                    if value.peek().chars().count() > limit {
                        return Err(invalid("billing_info"));
                    }
                    billing.insert(key.into(), serde_json::json!(value.peek()));
                }
            }
            if let Some(state) = address.get_optional_state() {
                if state.peek().chars().count() > 30 {
                    return Err(invalid("billing_info.billing_state"));
                }
                billing.insert("billing_state".into(), serde_json::json!(state.peek()));
            }
            if let Some(country) = address.country {
                billing.insert(
                    "billing_country".into(),
                    serde_json::json!(country.to_string()),
                );
            }
        }
        Ok(Self {
            transaction: Secret::new(
                serde_json::json!({"merchant_id":auth.merchant_id.peek(),"request_id":id,"notification_url":notify,"response_url":return_url,"cancel_url":return_url,"pmethod":method.pmethod,"pchannel":method.pchannel,"payment_action":method.payment_action,"collection_method":collection_method,"amount":amount,"currency":currency,"payment_notification_status":PAYMENT_NOTIFICATION_DISABLED,"payment_notification_channel":PAYMENT_NOTIFICATION_EMAIL,"signature":signature.peek()}),
            ),
            customer_info: Secret::new(
                serde_json::json!({"fname":fname.peek(),"lname":lname.peek(),"mname":EMPTY_FIELD,"email":email,"phone":phone,"mobile":EMPTY_FIELD,"dob":dob,"signature":customer_signature.peek()}),
            ),
            billing_info: Secret::new(serde_json::Value::Object(billing)),
            order_details: Secret::new(
                serde_json::json!({"orders":lines,"subtotalprice":amount_f2(MinorUnit::new(subtotal),req.money.currency())?,"shippingprice":amount_f2(shipping,req.money.currency())?,"discountamount":amount_f2(MinorUnit::new(NO_ADDITIONAL_AMOUNT), req.money.currency())?,"totalorderamount":amount}),
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaynamicsResponseCode {
    GR001,
    GR002,
    GR033,
    GR003,
    GR038,
    GR039,
    GR152,
    GR158,
    GR150,
    GR122,
    GR178,
    GR011,
    GR007,
    GR031,
    GR036,
    GR153,
    FR030,
    FR032,
    FR033,
    FR036,
    QM001,
    #[serde(other)]
    Unknown,
}
impl From<PaynamicsResponseCode> for AttemptStatus {
    fn from(code: PaynamicsResponseCode) -> Self {
        match code {
            PaynamicsResponseCode::GR001 => Self::Charged,
            PaynamicsResponseCode::GR033
            | PaynamicsResponseCode::GR002
            | PaynamicsResponseCode::QM001
            | PaynamicsResponseCode::Unknown
            | PaynamicsResponseCode::GR122
            | PaynamicsResponseCode::GR178
            | PaynamicsResponseCode::GR011 => Self::Pending,
            PaynamicsResponseCode::GR158 => Self::Voided,
            PaynamicsResponseCode::GR003
            | PaynamicsResponseCode::GR038
            | PaynamicsResponseCode::GR039
            | PaynamicsResponseCode::GR152
            | PaynamicsResponseCode::GR150
            | PaynamicsResponseCode::GR007
            | PaynamicsResponseCode::GR031
            | PaynamicsResponseCode::GR036
            | PaynamicsResponseCode::GR153
            | PaynamicsResponseCode::FR030
            | PaynamicsResponseCode::FR032
            | PaynamicsResponseCode::FR033
            | PaynamicsResponseCode::FR036 => Self::Failure,
        }
    }
}

/// Flat Workflow response, with optional fields left absent for signing. Never
/// insert the outbound request_id: real accepted DragonPay responses omit it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynamicsPaymentResponse {
    pub merchant_id: Option<Secret<String>>,
    pub request_id: Option<String>,
    pub response_id: Option<String>,
    pub gateway_id: Option<String>,
    pub gatewayid: Option<String>,
    pub response_code: String,
    pub response_message: Option<String>,
    pub response_advise: Option<String>,
    pub timestamp: Option<String>,
    pub processor_response_id: Option<String>,
    pub processor_response_authcode: Option<Secret<String>>,
    pub pay_reference: Option<Secret<String>>,
    pub payment_action_info: Option<Secret<serde_json::Value>>,
    pub direct_otc_info: Option<Secret<serde_json::Value>>,
    pub expiry_limit: Option<String>,
    pub signature: Option<Secret<String>>,
}
impl PaynamicsPaymentResponse {
    pub fn code(&self) -> PaynamicsResponseCode {
        serde_json::from_value(serde_json::Value::String(self.response_code.clone()))
            // Appendix B is extensible: unknown business codes remain explicit
            // connector errors, never payment success or terminal failure.
            .unwrap_or(PaynamicsResponseCode::Unknown)
    }
    pub fn is_payment_record(&self) -> bool {
        matches!(
            self.code(),
            PaynamicsResponseCode::GR001
                | PaynamicsResponseCode::GR033
                | PaynamicsResponseCode::GR003
                | PaynamicsResponseCode::GR038
                | PaynamicsResponseCode::GR039
                | PaynamicsResponseCode::GR152
                | PaynamicsResponseCode::GR158
        )
    }
    pub fn verify_signature(
        &self,
        auth: &PaynamicsAuthType,
    ) -> Result<(), error_stack::Report<errors::ConnectorError>> {
        let fail = || {
            crate::utils::response_handling_fail(
                200,
                "paynamics: response identity or signature mismatch",
            )
        };
        if self
            .merchant_id
            .as_ref()
            .is_some_and(|mid| mid.peek() != auth.merchant_id.peek())
        {
            return Err(fail().into());
        }
        if let (Some(a), Some(b)) = (&self.gateway_id, &self.gatewayid) {
            if a != b {
                return Err(fail().into());
            }
        }
        if let Some(received) = &self.signature {
            let expected = sign_fields(
                &[
                    signature_component(self.merchant_id.as_ref().map(|s| s.peek().as_str())),
                    signature_component(self.request_id.as_deref()),
                    signature_component(self.response_id.as_deref()),
                    signature_component(self.gateway_id.as_deref().or(self.gatewayid.as_deref())),
                    &self.response_code,
                    signature_component(self.response_message.as_deref()),
                    signature_component(self.response_advise.as_deref()),
                    signature_component(self.timestamp.as_deref()),
                    signature_component(self.processor_response_id.as_deref()),
                    signature_component(
                        self.processor_response_authcode
                            .as_ref()
                            .map(|s| s.peek().as_str()),
                    ),
                    signature_component(self.pay_reference.as_ref().map(|s| s.peek().as_str())),
                ],
                &auth.merchant_key,
            );
            // Fixed 128-byte SHA512 hex; fold all bytes without an early comparison exit.
            let provided = received.peek().as_bytes();
            let wanted = expected.peek().as_bytes();
            let mismatch = provided.len() != wanted.len()
                || provided
                    .iter()
                    .zip(wanted)
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    != 0;
            if mismatch {
                return Err(fail().into());
            }
        }
        Ok(())
    }
    pub fn redirect(&self) -> Option<Box<RedirectForm>> {
        let dragonpay_url;
        let text = match self.payment_action_info.as_ref().and_then(|value| value.peek().as_str()) {
            Some(text) => text,
            None => {
                dragonpay_url = dragonpay_instruction_url(self.direct_otc_info.as_ref()?.peek())?;
                &dragonpay_url
            }
        };
        let url = url::Url::parse(text).ok()?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return None;
        }
        Some(Box::new(RedirectForm::Form {
            endpoint: text.to_owned(),
            method: common_utils::Method::Get,
            form_fields: Default::default(),
        }))
    }
}

// DragonPay's observed direct_otc response supplies both HTML and a standalone
// checkout URL in pay_instructions. Never execute the HTML or extract arbitrary
// href values. Only promote the known sandbox checkout route's plain-text URL;
// other OTC instructions and ambiguous destinations remain informational.
fn dragonpay_instruction_url(instructions: &serde_json::Value) -> Option<String> {
    let mut checkout: Option<String> = None;
    for entry in instructions.as_array()? {
        if entry.get("pay_channel").and_then(serde_json::Value::as_str) != Some("dp_ph") {
            continue;
        }
        let Some(text) = entry.get("pay_instructions").and_then(serde_json::Value::as_str) else {
            continue;
        };
        for candidate in text.split_whitespace() {
            let Ok(url) = url::Url::parse(candidate) else { continue };
            let Some(token) = url.path().strip_prefix("/paygate/transactions/dragonpay/status/") else {
                continue;
            };
            if url.scheme() != "https"
                || url.host_str() != Some("payin.payserv.net")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || token.is_empty()
                || !token.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                continue;
            }
            if checkout.as_deref().is_some_and(|previous| previous != candidate) {
                return None;
            }
            checkout = Some(candidate.to_owned());
        }
    }
    checkout
}

/// Caller must retain this opaque carrier as connector_feature_data. Instructions
/// may contain provider HTML: display safely as text, never execute it. Only the
/// separately validated standalone URL is an executable checkout action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaynamicsMetadata {
    pub version: u8,
    pub original_sale_request_id: String,
    pub latest_provider_id: Option<String>,
    pub original_provider_id: Option<String>,
    pub method: PaynamicsMethod,
    pub amount: String,
    pub currency: common_enums::Currency,
    pub pay_reference: Option<Secret<String>>,
    pub direct_otc_info: Option<Secret<serde_json::Value>>,
    pub payment_action_info: Option<Secret<serde_json::Value>>,
    pub expiry_limit: Option<String>,
    pub response_code: String,
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ResponseRouterData<
            PaynamicsPaymentResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    > for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            PaynamicsPaymentResponse,
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let response = item.response;
        let fail = || {
            crate::utils::response_handling_fail(
                200,
                "paynamics: invalid response or request context",
            )
        };
        let auth = PaynamicsAuthType::try_from(&data.connector_config).map_err(|_| fail())?;
        response.verify_signature(&auth)?;
        let original_id = sale_request_id(&data.resource_common_data).map_err(|_| fail())?;
        if response
            .request_id
            .as_ref()
            .is_some_and(|id| id != &original_id)
        {
            return Err(fail().into());
        }
        data.resource_common_data.raw_connector_status =
            Some(domain_types::connector_types::RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            });
        // Appendix B GR011 means the original request was already processed,
        // not that its payment failed. As with query errors, non-payment codes
        // must preserve the caller's state and be reconciled using the original
        // sale reference; never initiate a fresh sale implicitly.
        if !response.is_payment_record() {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                ..Default::default()
            });
            return Ok(data);
        }
        let status = AttemptStatus::from(response.code());
        data.resource_common_data.status = status;
        // Workflow §10.2 ordinary sale response has no comparable amount/currency.
        // The fullwiki amount at line 5265 belongs to static-reference enrollment.
        data.request.integrity_object = Some(AuthoriseIntegrityObject {
            amount: data.request.money.amount(),
            currency: data.request.money.currency(),
        });
        if domain_types::utils::is_payment_failure(status) || status == AttemptStatus::Voided {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: response.response_id,
                ..Default::default()
            });
            return Ok(data);
        }
        let metadata = PaynamicsMetadata {
            version: METADATA_VERSION,
            original_sale_request_id: original_id,
            latest_provider_id: response.response_id.clone(),
            original_provider_id: response.response_id.clone(),
            method: payment_method(&data.request.payment_method_data).map_err(|_| fail())?,
            amount: amount_f2(data.request.money.amount(), data.request.money.currency())
                .map_err(|_| fail())?,
            currency: data.request.money.currency(),
            pay_reference: response.pay_reference.clone(),
            direct_otc_info: response.direct_otc_info.clone(),
            payment_action_info: response.payment_action_info.clone(),
            expiry_limit: response.expiry_limit.clone(),
            response_code: response.response_code.clone(),
        };
        let metadata = serde_json::to_value(metadata).map_err(|_| fail())?;
        data.resource_common_data.connector_feature_data = Some(Secret::new(metadata.clone()));
        data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: response
                .response_id
                .clone()
                .map(ResponseId::ConnectorTransactionId)
                // Workflow §10.2 optional parser IDs: preserve absence rather
                // than substitute the merchant sale reference as a provider ID.
                .unwrap_or(ResponseId::NoResponseId),
            redirection_data: response.redirect(),
            connector_metadata: Some(metadata),
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: response.response_id,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        });
        Ok(data)
    }
}

/// The opaque sale carrier is the only source of the Workflow lookup target.
pub fn sync_metadata(request: &PaymentsSyncData) -> Result<PaynamicsMetadata, RequestError> {
    let carrier = request.connector_feature_data.as_ref().ok_or_else(
        domain_types::utils::missing_field_err("connector_feature_data"),
    )?;
    let metadata: PaynamicsMetadata = serde_json::from_value(carrier.peek().clone())
        .map_err(|_| invalid("connector_feature_data"))?;
    if metadata.version != METADATA_VERSION {
        return Err(invalid("connector_feature_data.version"));
    }
    validate_request_id(&metadata.original_sale_request_id)?;
    if metadata.method != metadata.method.pchannel.method() {
        return Err(invalid("connector_feature_data.method"));
    }
    metadata_minor_amount(&metadata)?;
    Ok(metadata)
}
fn metadata_minor_amount(metadata: &PaynamicsMetadata) -> Result<MinorUnit, RequestError> {
    let (whole, fraction) = metadata
        .amount
        .split_once('.')
        .ok_or_else(|| invalid("connector_feature_data.amount"))?;
    if whole.is_empty()
        || fraction.len() != 2
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return Err(invalid("connector_feature_data.amount"));
    }
    let exponent = metadata
        .currency
        .number_of_digits_after_decimal_point()
        .map_err(|_| invalid("connector_feature_data.currency"))?;
    if exponent > 2 {
        return Err(invalid("connector_feature_data.currency"));
    }
    let scaled = whole
        .parse::<i128>()
        .ok()
        .and_then(|n| n.checked_mul(100))
        .and_then(|n| fraction.parse::<i128>().ok().and_then(|f| n.checked_add(f)))
        .ok_or_else(|| invalid("connector_feature_data.amount"))?;
    let divisor = 10i128.pow(u32::from(2 - exponent));
    if scaled % divisor != 0 {
        return Err(invalid("connector_feature_data.amount"));
    }
    let minor =
        i64::try_from(scaled / divisor).map_err(|_| invalid("connector_feature_data.amount"))?;
    let amount = MinorUnit::new(minor);
    if amount_f2(amount, metadata.currency)? != metadata.amount {
        return Err(invalid("connector_feature_data.amount"));
    }
    Ok(amount)
}
/// Domain-separated deterministic query ID: stable for one sync call/reference,
/// distinct from the original sale. The provider requires a new query reference.
pub fn query_request_id(flow: &PaymentFlowData, original: &str) -> Result<String, RequestError> {
    let reference = sale_request_id(flow)?;
    let mut hash = Sha512::new();
    hash.update(QUERY_ID_DOMAIN);
    hash.update((reference.len() as u64).to_be_bytes());
    hash.update(reference.as_bytes());
    hash.update((original.len() as u64).to_be_bytes());
    hash.update(original.as_bytes());
    let digest = format!("{:x}", hash.finalize());
    let id = format!("{QUERY_ID_PREFIX}{}", &digest[..QUERY_ID_DIGEST_LENGTH]);
    if id == original {
        return Err(invalid("query_request_id"));
    }
    Ok(id)
}
#[derive(Debug, Serialize)]
pub struct PaynamicsSyncRequest {
    pub merchant_id: Secret<String>,
    pub request_id: String,
    pub org_trxid2: String,
    pub signature: Secret<String>,
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaynamicsRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for PaynamicsSyncRequest
{
    type Error = RequestError;
    fn try_from(
        item: PaynamicsRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data;
        let metadata = sync_metadata(&data.request)?;
        let auth = PaynamicsAuthType::try_from(&data.connector_config)?;
        if auth.merchant_id.peek().len() > 32 {
            return Err(invalid("connector_config.merchant_id"));
        }
        let request_id = query_request_id(
            &data.resource_common_data,
            &metadata.original_sale_request_id,
        )?;
        let signature = sign_fields(
            &[
                auth.merchant_id.peek(),
                &request_id,
                &metadata.original_sale_request_id,
            ],
            &auth.merchant_key,
        );
        Ok(Self {
            merchant_id: auth.merchant_id,
            request_id,
            org_trxid2: metadata.original_sale_request_id,
            signature,
        })
    }
}
impl
    TryFrom<
        ResponseRouterData<
            PaynamicsPaymentResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    > for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<
            PaynamicsPaymentResponse,
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        >,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let response = item.response;
        let fail = || {
            crate::utils::response_handling_fail(
                item.http_code,
                "paynamics: invalid sync response or sale context",
            )
        };
        let mut metadata = sync_metadata(&data.request).map_err(|_| fail())?;
        let auth = PaynamicsAuthType::try_from(&data.connector_config).map_err(|_| fail())?;
        response.verify_signature(&auth)?;
        data.resource_common_data.raw_connector_status =
            Some(domain_types::connector_types::RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            });
        // Query errors can echo the CURRENT query ID (observed GR122), not the
        // sale ID. They do not establish the payment's state or provider ID.
        if !response.is_payment_record() {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                ..Default::default()
            });
            return Ok(data);
        }
        if response
            .request_id
            .as_ref()
            .is_some_and(|id| id != &metadata.original_sale_request_id)
        {
            return Err(fail().into());
        }
        // §10.9 returns the Workflow record. Optional missing echo IDs are
        // correlated only by this HTTPS reply to this single signed query;
        // provider IDs are not assumed stable between initial/final phases.
        let status = AttemptStatus::from(response.code());
        data.resource_common_data.status = status;
        // Ordinary Workflow records omit amount/currency (§10.2/§10.9).
        // Use the persisted exact original sale values, never invent echoes.
        data.request.integrity_object = Some(PaymentSynIntegrityObject {
            amount: metadata_minor_amount(&metadata).map_err(|_| fail())?,
            currency: metadata.currency,
        });
        if let Some(id) = response.response_id.as_ref().filter(|id| !id.is_empty()) {
            metadata.latest_provider_id = Some(id.clone());
        }
        metadata.pay_reference = response.pay_reference.clone().or(metadata.pay_reference);
        metadata.direct_otc_info = response
            .direct_otc_info
            .clone()
            .or(metadata.direct_otc_info);
        metadata.payment_action_info = response
            .payment_action_info
            .clone()
            .or(metadata.payment_action_info);
        metadata.expiry_limit = response.expiry_limit.clone().or(metadata.expiry_limit);
        metadata.response_code = response.response_code.clone();
        let provider_id = metadata.latest_provider_id.clone();
        let metadata = serde_json::to_value(metadata).map_err(|_| fail())?;
        data.resource_common_data.connector_feature_data = Some(Secret::new(metadata.clone()));
        if domain_types::utils::is_payment_failure(status) || status == AttemptStatus::Voided {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: provider_id,
                ..Default::default()
            });
            return Ok(data);
        }
        data.response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: provider_id
                .clone()
                .map(ResponseId::ConnectorTransactionId)
                // Workflow §10.9 uses optional provider IDs across phases;
                // retain the caller's existing ID if no provider ID was returned.
                .unwrap_or_else(|| data.request.connector_transaction_id.clone()),
            redirection_data: response.redirect(),
            connector_metadata: Some(metadata),
            mandate_reference: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: provider_id,
            incremental_authorization_allowed: None,
            splits: None,
            status_code: item.http_code,
            payment_account_reference: None,
        });
        Ok(data)
    }
}

/// Workflow query returns the same flat response contract (§10.9).
pub type PaynamicsSyncResponse = PaynamicsPaymentResponse;

/// Only the documented flat Workflow JSON object is accepted. Detect duplicate
/// keys before constructing a JSON map (which would otherwise silently overwrite).
pub fn parse_callback(
    body: &[u8],
) -> Result<PaynamicsPaymentResponse, error_stack::Report<errors::WebhookError>> {
    struct UniqueObject;
    impl<'de> serde::de::Visitor<'de> for UniqueObject {
        type Value = serde_json::Map<String, serde_json::Value>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a flat Workflow JSON object with unique fields")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut access: M,
        ) -> Result<Self::Value, M::Error> {
            let mut fields = serde_json::Map::new();
            while let Some((key, value)) = access.next_entry::<String, serde_json::Value>()? {
                if fields.contains_key(&key) {
                    return Err(serde::de::Error::custom("duplicate callback field"));
                }
                fields.insert(key, value);
            }
            if fields.contains_key("gateway_id") && fields.contains_key("gatewayid") {
                return Err(serde::de::Error::custom(
                    "duplicate gateway identifier aliases",
                ));
            }
            Ok(fields)
        }
    }
    let mut decoder = serde_json::Deserializer::from_slice(body);
    let fields = serde::de::Deserializer::deserialize_map(&mut decoder, UniqueObject)
        .map_err(|_| errors::WebhookError::WebhookBodyDecodingFailed)?;
    decoder
        .end()
        .map_err(|_| errors::WebhookError::WebhookBodyDecodingFailed)?;
    let response: PaynamicsPaymentResponse =
        serde_json::from_value(serde_json::Value::Object(fields))
            .map_err(|_| errors::WebhookError::WebhookBodyDecodingFailed)?;
    for id in [&response.request_id, &response.response_id]
        .into_iter()
        .flatten()
    {
        if id.is_empty() || id.chars().any(char::is_control) {
            return Err(errors::WebhookError::WebhookReferenceIdNotFound.into());
        }
    }
    Ok(response)
}

/// Source verification is informational in the shared dispatcher. Both verification
/// and content processing must call this gate. Only canonical signed fields may
/// influence actionable callback results; instructions/amounts are not signed.
pub fn verified_callback(
    body: &[u8],
    config: Option<&ConnectorSpecificConfig>,
) -> Result<PaynamicsPaymentResponse, error_stack::Report<errors::WebhookError>> {
    let response = parse_callback(body)?;
    let auth = PaynamicsAuthType::try_from(
        config.ok_or(errors::WebhookError::WebhookVerificationSecretNotFound)?,
    )
    .map_err(|_| errors::WebhookError::WebhookVerificationSecretInvalid)?;
    if response.merchant_id.as_ref().map(|mid| mid.peek()) != Some(auth.merchant_id.peek()) {
        return Err(errors::WebhookError::WebhookSourceVerificationFailed.into());
    }
    let signature = response
        .signature
        .as_ref()
        .ok_or(errors::WebhookError::WebhookSignatureNotFound)?;
    let text = signature.peek();
    if text.len() != 128 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(errors::WebhookError::WebhookSourceVerificationFailed.into());
    }
    // Hexadecimal case is representation only; signed payload text is untouched.
    let mut normalized = response.clone();
    normalized.signature = Some(Secret::new(text.to_ascii_lowercase()));
    normalized
        .verify_signature(&auth)
        .map_err(|_| errors::WebhookError::WebhookSourceVerificationFailed)?;
    if response.request_id.is_none() && response.response_id.is_none() {
        return Err(errors::WebhookError::WebhookReferenceIdNotFound.into());
    }
    Ok(response)
}

pub fn callback_event(
    response: &PaynamicsPaymentResponse,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;
    match response.code() {
        PaynamicsResponseCode::GR001 => EventType::PaymentIntentSuccess,
        PaynamicsResponseCode::GR033 => EventType::PaymentIntentProcessing,
        PaynamicsResponseCode::GR003
        | PaynamicsResponseCode::GR038
        | PaynamicsResponseCode::GR039
        | PaynamicsResponseCode::GR152 => EventType::PaymentIntentFailure,
        // Cancellation/admin/query/card codes are outside selected notifications.
        PaynamicsResponseCode::GR002
        | PaynamicsResponseCode::GR158
        | PaynamicsResponseCode::GR150
        | PaynamicsResponseCode::GR122
        | PaynamicsResponseCode::GR178
        | PaynamicsResponseCode::GR011
        | PaynamicsResponseCode::GR007
        | PaynamicsResponseCode::GR031
        | PaynamicsResponseCode::GR036
        | PaynamicsResponseCode::GR153
        | PaynamicsResponseCode::FR030
        | PaynamicsResponseCode::FR032
        | PaynamicsResponseCode::FR033
        | PaynamicsResponseCode::FR036
        | PaynamicsResponseCode::QM001
        | PaynamicsResponseCode::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}

/// Merchant lifecycle inputs, passed as refund_connector_metadata for Refund.
/// Merchant host IP is configuration input, never the customer's browser IP.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaynamicsLifecycleInput {
    pub merchant_host_ip: Secret<String>,
    pub response_url: Option<String>,
}
impl PaynamicsLifecycleInput {
    pub fn validate(&self) -> Result<(), RequestError> {
        let ip = self.merchant_host_ip.peek();
        if ip.len() > 20 || ip.parse::<std::net::IpAddr>().is_err() {
            return Err(invalid("lifecycle_metadata.merchant_host_ip"));
        }
        if let Some(url) = &self.response_url {
            validate_url(url, "lifecycle_metadata.response_url")?;
        }
        Ok(())
    }
}

/// Parse the unchanged sale carrier for refund/reversal correlation.
pub fn lifecycle_sale_metadata(
    value: Option<&Secret<serde_json::Value>>,
) -> Result<PaynamicsMetadata, RequestError> {
    let value = value.ok_or_else(domain_types::utils::missing_field_err(
        "connector_feature_data",
    ))?;
    let metadata: PaynamicsMetadata = serde_json::from_value(value.peek().clone())
        .map_err(|_| invalid("connector_feature_data"))?;
    if metadata.version != METADATA_VERSION {
        return Err(invalid("connector_feature_data.version"));
    }
    validate_request_id(&metadata.original_sale_request_id)?;
    metadata_minor_amount(&metadata)?;
    Ok(metadata)
}

/// Refund eligibility comes from the provider's supplied processing manual,
/// not acceptance of a generic endpoint. Existing-refund lookup is independent.
fn validate_refund_eligibility(channel: PaynamicsChannel) -> Result<(), RequestError> {
    let message = match channel {
        PaynamicsChannel::Bpi | PaynamicsChannel::Unionbank =>
            "API refunds are unavailable for BPI/UnionBank Online; request a refund through PTI Merchant Support",
        PaynamicsChannel::Landbank | PaynamicsChannel::Dragonpay =>
            "API refunds are unavailable for Brankas/Landbank and DragonPay; the merchant must process refunds manually",
        PaynamicsChannel::Robinsons =>
            "API refund eligibility for Robinsons OTC is unconfirmed; contact PTI Merchant Support for manual refund instructions",
        PaynamicsChannel::Rcbc =>
            "API refund eligibility for RCBC Digital is unconfirmed; contact PTI Merchant Support before requesting a refund",
    };
    Err(unsupported(message))
}

#[derive(Debug, Serialize)]
pub struct PaynamicsRefundRequest {
    pub merchant_id: Secret<String>,
    pub request_id: String,
    pub org_trxid: String,
    pub ip_address: Secret<String>,
    pub notification_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_url: Option<String>,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub signature: Secret<String>,
}
pub type PaynamicsRefundResponse = PaynamicsPaymentResponse;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaynamicsRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                domain_types::connector_types::RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    > for PaynamicsRefundRequest
{
    type Error = RequestError;
    fn try_from(
        item: PaynamicsRouterData<
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                domain_types::connector_types::RefundsData,
                RefundsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = item.router_data;
        let req = &data.request;
        let auth = PaynamicsAuthType::try_from(&data.connector_config)?;
        let sale = lifecycle_sale_metadata(req.connector_feature_data.as_ref())?;
        validate_refund_eligibility(sale.method.pchannel)?;
        let input = req.refund_connector_metadata.as_ref().ok_or_else(
            domain_types::utils::missing_field_err("refund_connector_metadata.merchant_host_ip"),
        )?;
        let input: PaynamicsLifecycleInput = serde_json::from_value(input.peek().clone())
            .map_err(|_| invalid("refund_connector_metadata"))?;
        input.validate()?;
        validate_request_id(&req.refund_id)?;
        if req.refund_id == sale.original_sale_request_id {
            return Err(invalid("refund_id"));
        }
        if req.split_refunds.is_some() || req.split_settlement_refund.is_some() {
            return Err(unsupported(
                "Split-pay refunds are outside the Paynamics Workflow integration",
            ));
        }
        if req.refund_money.currency() != sale.currency
            || req.payment_money.currency() != sale.currency
            || req.currency != sale.currency
            || req.payment_money.amount() != metadata_minor_amount(&sale)?
            || req.refund_money.amount().get_amount_as_i64() <= 0
            || req.refund_money.amount() > req.payment_money.amount()
        {
            return Err(invalid("refund_money"));
        }
        if !req.connector_transaction_id.is_empty()
            && req.connector_transaction_id != sale.original_sale_request_id
            && ![
                sale.original_provider_id.as_ref(),
                sale.latest_provider_id.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|id| id == &req.connector_transaction_id)
        {
            return Err(invalid("connector_transaction_id"));
        }
        // The observed ordinary Workflow refund endpoint rejects requests without
        // notification_url (FR030). Forward the caller's dedicated refund receiver;
        // this does not add lifecycle ingestion to the payment-only webhook handler.
        let notification_url = req
            .webhook_url
            .as_deref()
            .ok_or_else(domain_types::utils::missing_field_err("webhook_url"))?;
        if notification_url.chars().any(char::is_whitespace) {
            return Err(invalid("webhook_url"));
        }
        validate_url(notification_url, "webhook_url")?;
        if req
            .reason
            .as_ref()
            .is_some_and(|reason| reason.chars().count() > 100)
        {
            return Err(invalid("reason"));
        }
        if auth.merchant_id.peek().chars().count() > 32 {
            return Err(invalid("connector_config.merchant_id"));
        }
        let amount = amount_f2(req.refund_money.amount(), sale.currency)?;
        let signature = sign_fields(
            &[
                auth.merchant_id.peek(),
                &req.refund_id,
                &sale.original_sale_request_id,
                input.merchant_host_ip.peek(),
                notification_url,
                signature_component(input.response_url.as_deref()),
                &amount,
                signature_component(req.reason.as_deref()),
            ],
            &auth.merchant_key,
        );
        Ok(Self {
            merchant_id: auth.merchant_id,
            request_id: req.refund_id.clone(),
            org_trxid: sale.original_sale_request_id,
            ip_address: input.merchant_host_ip,
            notification_url: notification_url.to_owned(),
            response_url: input.response_url,
            amount,
            reason: req.reason.clone(),
            signature,
        })
    }
}

/// Distinct operation outcomes; sale-status conversion must not decide refunds.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum PaynamicsRefundCode {
    GR001,
    GR033,
    GR003,
    GR038,
    #[serde(other)]
    Unknown,
}
impl From<PaynamicsRefundCode> for common_enums::RefundStatus {
    fn from(code: PaynamicsRefundCode) -> Self {
        match code {
            PaynamicsRefundCode::GR001 => Self::Success,
            PaynamicsRefundCode::GR033 | PaynamicsRefundCode::Unknown => Self::Pending,
            PaynamicsRefundCode::GR003 | PaynamicsRefundCode::GR038 => Self::Failure,
        }
    }
}
impl
    TryFrom<
        ResponseRouterData<
            PaynamicsRefundResponse,
            RouterDataV2<
                domain_types::connector_flow::Refund,
                RefundFlowData,
                domain_types::connector_types::RefundsData,
                RefundsResponseData,
            >,
        >,
    >
    for RouterDataV2<
        domain_types::connector_flow::Refund,
        RefundFlowData,
        domain_types::connector_types::RefundsData,
        RefundsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaynamicsRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let response = item.response;
        let fail = || {
            crate::utils::response_handling_fail(
                item.http_code,
                "paynamics: invalid refund response context",
            )
        };
        let auth = PaynamicsAuthType::try_from(&data.connector_config).map_err(|_| fail())?;
        response.verify_signature(&auth)?;
        if response
            .request_id
            .as_ref()
            .is_some_and(|id| id != &data.request.refund_id)
        {
            return Err(fail().into());
        }
        data.resource_common_data.raw_connector_status =
            Some(domain_types::connector_types::RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            });
        let code: PaynamicsRefundCode =
            serde_json::from_value(serde_json::Value::String(response.response_code.clone()))
                .map_err(|_| fail())?;
        let status = common_enums::RefundStatus::from(code);
        if matches!(
            code,
            PaynamicsRefundCode::Unknown | PaynamicsRefundCode::GR003 | PaynamicsRefundCode::GR038
        ) {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                attempt_status: (!matches!(code, PaynamicsRefundCode::Unknown))
                    .then_some(FlowStatus::Refund(status)),
                connector_transaction_id: Some(data.request.connector_transaction_id.clone()),
                ..Default::default()
            });
            return Ok(data);
        }
        // A successful-operation record must identify this refund; no fallback
        // to the original sale/provider ID or to an unrelated diagnostic ID.
        if response.merchant_id.is_none()
            || response.request_id.is_none()
            || response.timestamp.is_none()
        {
            return Err(fail().into());
        }
        let refund_id = response
            .response_id
            .filter(|id| !id.is_empty() && id.len() <= 32)
            .ok_or_else(fail)?;
        // §10.21 does not echo refund amount/currency. Retain the exact
        // validated request money for the framework integrity comparison.
        data.request.integrity_object =
            Some(domain_types::router_request_types::RefundIntegrityObject {
                refund_amount: data.request.refund_money.amount(),
                currency: data.request.refund_money.currency(),
            });
        data.resource_common_data.status = status;
        data.response = Ok(RefundsResponseData {
            connector_refund_id: refund_id,
            refund_status: status,
            status_code: item.http_code,
            // §10.21 does not provide an acquirer reference number.
            acquirer_reference_number: None,
        });
        Ok(data)
    }
}

/// Reverse has no dedicated lifecycle metadata slot in its domain request.
/// The caller retains the full sale carrier and supplies this per-operation
/// object at connector_feature_data.reversal. Never use browser/client IP.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaynamicsReversalInput {
    merchant_host_ip: Secret<String>,
    response_url: Option<String>,
    notification_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaynamicsVoidPCRequest {
    merchant_id: Secret<String>,
    request_id: String,
    org_trxid: String,
    ip_address: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    signature: Secret<String>,
}
pub type PaynamicsVoidPCResponse = PaynamicsPaymentResponse;

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaynamicsRouterData<
            RouterDataV2<
                domain_types::connector_flow::VoidPC,
                PaymentFlowData,
                domain_types::connector_types::PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaynamicsVoidPCRequest
{
    type Error = RequestError;
    fn try_from(
        item: PaynamicsRouterData<
            RouterDataV2<
                domain_types::connector_flow::VoidPC,
                PaymentFlowData,
                domain_types::connector_types::PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = item.router_data;
        let req = &data.request;
        let flow = &data.resource_common_data;
        let auth = PaynamicsAuthType::try_from(&data.connector_config)?;
        let sale = lifecycle_sale_metadata(flow.connector_feature_data.as_ref())?;
        let input = flow
            .connector_feature_data
            .as_ref()
            .and_then(|value| value.peek().get("reversal"))
            .ok_or_else(domain_types::utils::missing_field_err(
                "connector_feature_data.reversal",
            ))?;
        let input: PaynamicsReversalInput = serde_json::from_value(input.clone())
            .map_err(|_| invalid("connector_feature_data.reversal"))?;
        PaynamicsLifecycleInput {
            merchant_host_ip: input.merchant_host_ip.clone(),
            response_url: input.response_url.clone(),
        }
        .validate()?;
        let request_id = sale_request_id(flow)?;
        if request_id == sale.original_sale_request_id {
            return Err(invalid("merchant_request_id"));
        }
        if !matches!(req.capture_method, None | Some(CaptureMethod::Automatic)) {
            return Err(unsupported(
                "Paynamics noncard reversal requires an automatic sale",
            ));
        }
        if req.connector_transaction_id.is_empty()
            || (req.connector_transaction_id != sale.original_sale_request_id
                && ![
                    sale.original_provider_id.as_ref(),
                    sale.latest_provider_id.as_ref(),
                ]
                .into_iter()
                .flatten()
                .any(|id| id == &req.connector_transaction_id))
        {
            return Err(invalid("connector_transaction_id"));
        }
        // §10.20 makes amount conditional for reversal. If supplied, only the
        // full original sale is supported: partial-reversal eligibility unknown.
        let amount = req
            .money
            .as_ref()
            .map(|money| {
                if money.currency() != sale.currency
                    || money.amount() != metadata_minor_amount(&sale)?
                    || money.amount().get_amount_as_i64() <= 0
                {
                    return Err(invalid("amount"));
                }
                amount_f2(money.amount(), money.currency())
            })
            .transpose()?;
        if input.notification_url.is_some() {
            return Err(unsupported(
                "Paynamics reversal callbacks are unsupported: omit reversal.notification_url; lifecycle callbacks lack an operation discriminator",
            ));
        }
        if req
            .cancellation_reason
            .as_ref()
            .is_some_and(|reason| reason.chars().count() > 100)
        {
            return Err(invalid("cancellation_reason"));
        }
        if auth.merchant_id.peek().chars().count() > 32 {
            return Err(invalid("connector_config.merchant_id"));
        }
        let signature = sign_fields(
            &[
                auth.merchant_id.peek(),
                &request_id,
                &sale.original_sale_request_id,
                input.merchant_host_ip.peek(),
                // Lifecycle callback ingestion is unsupported: sign the omitted field as empty.
                EMPTY_FIELD,
                signature_component(input.response_url.as_deref()),
                signature_component(amount.as_deref()),
                signature_component(req.cancellation_reason.as_deref()),
            ],
            &auth.merchant_key,
        );
        Ok(Self {
            merchant_id: auth.merchant_id,
            request_id,
            org_trxid: sale.original_sale_request_id,
            ip_address: input.merchant_host_ip,
            response_url: input.response_url,
            amount,
            reason: req.cancellation_reason.clone(),
            signature,
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum PaynamicsReversalCode {
    GR001,
    GR091,
    GR033,
    GR003,
    GR038,
    #[serde(other)]
    Unknown,
}
impl From<PaynamicsReversalCode> for common_enums::PostCaptureVoidStatus {
    fn from(code: PaynamicsReversalCode) -> Self {
        match code {
            PaynamicsReversalCode::GR001 | PaynamicsReversalCode::GR091 => Self::Succeeded,
            PaynamicsReversalCode::GR033 | PaynamicsReversalCode::Unknown => Self::Pending,
            PaynamicsReversalCode::GR003 | PaynamicsReversalCode::GR038 => Self::Failed,
        }
    }
}
impl TryFrom<ResponseRouterData<PaynamicsVoidPCResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::VoidPC,
        PaymentFlowData,
        domain_types::connector_types::PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaynamicsVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let response = item.response;
        let fail = || {
            crate::utils::response_handling_fail(
                item.http_code,
                "paynamics: invalid reversal response context",
            )
        };
        let auth = PaynamicsAuthType::try_from(&data.connector_config).map_err(|_| fail())?;
        response.verify_signature(&auth)?;
        let request_id = sale_request_id(&data.resource_common_data).map_err(|_| fail())?;
        if response
            .request_id
            .as_ref()
            .is_some_and(|id| id != &request_id)
        {
            return Err(fail().into());
        }
        data.resource_common_data.raw_connector_status =
            Some(domain_types::connector_types::RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            });
        let code: PaynamicsReversalCode =
            serde_json::from_value(serde_json::Value::String(response.response_code.clone()))
                .map_err(|_| fail())?;
        let status = common_enums::PostCaptureVoidStatus::from(code);
        if matches!(
            code,
            PaynamicsReversalCode::Unknown
                | PaynamicsReversalCode::GR003
                | PaynamicsReversalCode::GR038
        ) {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response
                    .response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                // A failed reverse does not mean the original captured sale failed.
                connector_transaction_id: Some(data.request.connector_transaction_id.clone()),
                ..Default::default()
            });
            return Ok(data);
        }
        if response.merchant_id.is_none()
            || response.request_id.is_none()
            || response.timestamp.is_none()
        {
            return Err(fail().into());
        }
        let operation_id = response
            .response_id
            .filter(|id| !id.is_empty() && id.len() <= 32)
            .ok_or_else(fail)?;
        // §10.21 echoes only this operation's ID, not the original transaction.
        // Retain the original ID validated against the persisted sale carrier.
        data.request.integrity_object = Some(
            domain_types::router_request_types::PaymentVoidPostCaptureIntegrityObject {
                connector_transaction_id: data.request.connector_transaction_id.clone(),
            },
        );
        data.response = Ok(PaymentsResponseData::PostCaptureVoidResponse {
            post_capture_void_status: status,
            connector_reference_id: Some(operation_id),
            status_code: item.http_code,
        });
        Ok(data)
    }
}

pub type PaynamicsRefundSyncResponse = PaynamicsPaymentResponse;

/// The merchant refund request ID is the lookup target. The provider refund ID
/// and original sale carrier independently prevent accepting a sale as a refund.
pub fn refund_sync_context(
    flow: &RefundFlowData,
    request: &RefundSyncData,
) -> Result<String, RequestError> {
    let target = flow.refund_id.as_deref().ok_or_else(
        domain_types::utils::missing_field_err("merchant_refund_id"),
    )?;
    validate_request_id(target).map_err(|_| invalid("merchant_refund_id"))?;
    validate_request_id(&request.connector_refund_id)
        .map_err(|_| invalid("connector_refund_id"))?;
    let sale = lifecycle_sale_metadata(request.connector_feature_data.as_ref())?;
    if target == sale.original_sale_request_id
        || target == request.connector_refund_id
        || sale.original_provider_id.as_deref() == Some(target)
        || sale.latest_provider_id.as_deref() == Some(target)
    {
        return Err(invalid("merchant_refund_id"));
    }
    if request.connector_refund_id == sale.original_sale_request_id
        || sale.original_provider_id.as_ref() == Some(&request.connector_refund_id)
        || sale.latest_provider_id.as_ref() == Some(&request.connector_refund_id)
    {
        return Err(invalid("connector_refund_id"));
    }
    if request.connector_transaction_id.is_empty()
        || ![
            Some(sale.original_sale_request_id.as_str()),
            sale.original_provider_id.as_deref(),
            sale.latest_provider_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|id| id == request.connector_transaction_id)
    {
        return Err(invalid("connector_transaction_id"));
    }
    Ok(target.to_owned())
}

pub fn refund_query_request_id(
    flow: &RefundFlowData,
    refund_target: &str,
) -> Result<String, RequestError> {
    let reference = flow.merchant_request_id.as_deref().ok_or_else(
        domain_types::utils::missing_field_err("merchant_request_id"),
    )?;
    validate_request_id(reference).map_err(|_| invalid("merchant_request_id"))?;
    if reference == refund_target {
        return Err(invalid("merchant_request_id"));
    }
    let mut hash = Sha512::new();
    hash.update(REFUND_QUERY_ID_DOMAIN);
    hash.update((reference.len() as u64).to_be_bytes());
    hash.update(reference.as_bytes());
    hash.update((refund_target.len() as u64).to_be_bytes());
    hash.update(refund_target.as_bytes());
    let digest = format!("{:x}", hash.finalize());
    let id = format!("{QUERY_ID_PREFIX}{}", &digest[..QUERY_ID_DIGEST_LENGTH]);
    if id == refund_target {
        return Err(invalid("query_request_id"));
    }
    Ok(id)
}

#[derive(Debug, Serialize)]
pub struct PaynamicsRefundSyncRequest {
    pub merchant_id: Secret<String>,
    pub request_id: String,
    pub org_trxid2: String,
    pub signature: Secret<String>,
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaynamicsRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for PaynamicsRefundSyncRequest
{
    type Error = RequestError;
    fn try_from(
        item: PaynamicsRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = item.router_data;
        let target = refund_sync_context(&data.resource_common_data, &data.request)?;
        let auth = PaynamicsAuthType::try_from(&data.connector_config)?;
        validate_request_id(auth.merchant_id.peek())
            .map_err(|_| invalid("connector_config.merchant_id"))?;
        let request_id = refund_query_request_id(&data.resource_common_data, &target)?;
        let signature = sign_fields(
            &[auth.merchant_id.peek(), &request_id, &target],
            &auth.merchant_key,
        );
        Ok(Self {
            merchant_id: auth.merchant_id,
            request_id,
            org_trxid2: target,
            signature,
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            PaynamicsRefundSyncResponse,
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        >,
    > for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaynamicsRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        let response = item.response;
        let fail = || {
            crate::utils::response_handling_fail(
                item.http_code,
                "paynamics: invalid refund query identity or signature",
            )
        };
        let auth = PaynamicsAuthType::try_from(&data.connector_config).map_err(|_| fail())?;
        response.verify_signature(&auth)?;
        data.resource_common_data.raw_connector_status =
            Some(domain_types::connector_types::RawConnectorStatus {
                code: Some(response.response_code.clone()),
                message: response.response_message.clone(),
                reason: response.response_advise.clone(),
            });
        let code: PaynamicsRefundCode =
            serde_json::from_value(serde_json::Value::String(response.response_code.clone()))
                .map_err(|_| fail())?;
        // A query error may echo the current poll ID and carry a diagnostic ID.
        // Handle it before refund-record correlation, preserving prior status.
        if matches!(code, PaynamicsRefundCode::Unknown) {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response.response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                ..Default::default()
            });
            return Ok(data);
        }
        let target = refund_sync_context(&data.resource_common_data, &data.request)
            .map_err(|_| fail())?;
        if response.signature.is_none()
            || response.merchant_id.is_none()
            || response.request_id.as_deref() != Some(target.as_str())
            || response.response_id.as_deref() != Some(data.request.connector_refund_id.as_str())
            || response.timestamp.as_deref().filter(|value| !value.is_empty()).is_none()
        {
            return Err(fail().into());
        }
        data.request.integrity_object =
            Some(domain_types::router_request_types::RefundSyncIntegrityObject {
                connector_transaction_id: data.request.connector_transaction_id.clone(),
                connector_refund_id: data.request.connector_refund_id.clone(),
            });
        let status = common_enums::RefundStatus::from(code);
        data.resource_common_data.status = status;
        if matches!(code, PaynamicsRefundCode::GR003 | PaynamicsRefundCode::GR038) {
            data.response = Err(ErrorResponse {
                status_code: item.http_code,
                code: response.response_code,
                message: response.response_message
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: response.response_advise,
                attempt_status: Some(FlowStatus::Refund(status)),
                connector_transaction_id: Some(data.request.connector_transaction_id.clone()),
                ..Default::default()
            });
            return Ok(data);
        }
        data.response = Ok(RefundsResponseData {
            connector_refund_id: data.request.connector_refund_id.clone(),
            refund_status: status,
            status_code: item.http_code,
            // Ordinary Workflow query does not supply a refund ARN or money.
            acquirer_reference_number: None,
        });
        Ok(data)
    }
}
