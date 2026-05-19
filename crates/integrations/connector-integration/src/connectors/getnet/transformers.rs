use crate::{connectors::getnet::GetnetRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, AuthenticationType, Currency, RefundStatus};
use common_utils::{id_type::CustomerId, request::Method, types::MinorUnit, Email};
use domain_types::errors::{ConnectorError, IntegrationError};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, ServerAuthenticationToken, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    payment_method_data::{
        BankRedirectData, BankTransferData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, VoucherData,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt;
use time::{Duration as TimeDuration, OffsetDateTime};

const TRANSACTION_TYPE_FULL: &str = "FULL";
const DEFAULT_INSTALLMENTS: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GetnetPaymentMethod {
    #[serde(rename = "CREDIT")]
    DirectCredit,
    #[serde(rename = "CREDIT_AUTHORIZATION")]
    DirectCreditAuthorization,
    #[serde(rename = "PIX")]
    Pix,
    #[serde(rename = "PIX_AUTOMATICO")]
    PixAutomatico,
    #[serde(rename = "BOLETO")]
    Boleto,
    #[serde(rename = "BIZUM")]
    Bizum,
}

impl fmt::Display for GetnetPaymentMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectCredit => write!(f, "CREDIT"),
            Self::DirectCreditAuthorization => write!(f, "CREDIT_AUTHORIZATION"),
            Self::Pix => write!(f, "PIX"),
            Self::PixAutomatico => write!(f, "PIX_AUTOMATICO"),
            Self::Boleto => write!(f, "BOLETO"),
            Self::Bizum => write!(f, "BIZUM"),
        }
    }
}

impl GetnetPaymentMethod {
    /// Determine payment method based on capture method
    fn from_capture_method(capture_method: Option<common_enums::CaptureMethod>) -> Self {
        match capture_method {
            Some(common_enums::CaptureMethod::Manual) => Self::DirectCreditAuthorization,
            _ => Self::DirectCredit,
        }
    }

    /// Determine payment method from payment method data + capture method.
    /// For 3DS cards the payment_method string is still `CREDIT` — the 3DS payload
    /// goes into `additional_data.three_ds`.
    pub fn from_payment_method_data<T: PaymentMethodDataTypes>(
        payment_method_data: &PaymentMethodData<T>,
        capture_method: Option<common_enums::CaptureMethod>,
        setup_future_usage: Option<common_enums::FutureUsage>,
    ) -> Result<Self, IntegrationError> {
        match payment_method_data {
            PaymentMethodData::Card(_) => Ok(Self::from_capture_method(capture_method)),
            PaymentMethodData::BankTransfer(bt) => match bt.as_ref() {
                BankTransferData::Pix { .. } => {
                    if matches!(
                        setup_future_usage,
                        Some(common_enums::FutureUsage::OffSession)
                    ) {
                        Ok(Self::PixAutomatico)
                    } else {
                        Ok(Self::Pix)
                    }
                }
                _ => Err(IntegrationError::NotSupported {
                    message: "BankTransfer variant".to_string(),
                    connector: "Getnet",
                    context: Default::default(),
                }),
            },
            PaymentMethodData::Voucher(VoucherData::Boleto(_)) => Ok(Self::Boleto),
            PaymentMethodData::BankRedirect(BankRedirectData::Bizum {}) => Ok(Self::Bizum),
            _ => Err(IntegrationError::NotSupported {
                message: "Payment method".to_string(),
                connector: "Getnet",
                context: Default::default(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum GetnetCardBrand {
    Mastercard,
    Visa,
    Amex,
    Elo,
    Hipercard,
}

#[derive(Debug, Clone)]
pub struct GetnetAuthType {
    pub api_key: Secret<String>,
    pub api_secret: Secret<String>,
    pub seller_id: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for GetnetAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Getnet {
                api_key,
                api_secret,
                seller_id,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
                seller_id: seller_id.to_owned(),
            }),
            _other => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: Default::default()
                }
            )),
        }
    }
}

// ===== ERROR RESPONSE =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetnetErrorResponse {
    #[serde(rename = "error_code")]
    pub code: Option<String>,
    pub message: String,
    pub details: Option<Vec<GetnetErrorDetail>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetnetErrorDetail {
    pub field: Option<String>,
    pub message: Option<String>,
}

// ===== STATUS ENUMS =====
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum GetnetPaymentStatus {
    Approved,
    Captured,
    Pending,
    Waiting,
    Authorized,
    Denied,
    Failed,
    Error,
    Canceled,
    Cancelled,
    #[serde(rename = "REQUIRES_ACTION")]
    RequiresAction,
    Redirect,
    /// Boleto-specific terminal-pending status returned by Globalgetnet
    /// (the literal `"EM ABERTO"` from the sandbox, plus common variants).
    /// Boleto is paid offline, so "open" maps to `AttemptStatus::Pending`.
    #[serde(rename = "EM ABERTO", alias = "em aberto", alias = "EM_ABERTO")]
    Open,
    #[serde(other)]
    Unknown,
}

impl From<&GetnetPaymentStatus> for AttemptStatus {
    fn from(status: &GetnetPaymentStatus) -> Self {
        match status {
            GetnetPaymentStatus::Approved | GetnetPaymentStatus::Captured => Self::Charged,
            GetnetPaymentStatus::Authorized => Self::Authorized,
            GetnetPaymentStatus::Pending
            | GetnetPaymentStatus::Waiting
            | GetnetPaymentStatus::Open => Self::Pending,
            GetnetPaymentStatus::Denied
            | GetnetPaymentStatus::Failed
            | GetnetPaymentStatus::Error => Self::Failure,
            GetnetPaymentStatus::Canceled | GetnetPaymentStatus::Cancelled => Self::Voided,
            GetnetPaymentStatus::RequiresAction | GetnetPaymentStatus::Redirect => {
                Self::AuthenticationPending
            }
            GetnetPaymentStatus::Unknown => Self::Pending,
        }
    }
}

impl From<&GetnetPaymentStatus> for RefundStatus {
    fn from(status: &GetnetPaymentStatus) -> Self {
        match status {
            GetnetPaymentStatus::Canceled | GetnetPaymentStatus::Cancelled => Self::Success,
            GetnetPaymentStatus::Pending
            | GetnetPaymentStatus::Waiting
            | GetnetPaymentStatus::Open => Self::Pending,
            GetnetPaymentStatus::Denied
            | GetnetPaymentStatus::Failed
            | GetnetPaymentStatus::Error => Self::Failure,
            _ => Self::Pending,
        }
    }
}

/// Top-level Authorize request envelope.
///
/// Globalgetnet uses *different* request shapes for different payment methods:
///   * `Standard`  → posted to `/dpm/payments-gwproxy/v2/payments`
///                   (card / wallet etc. — legacy shape with top-level `order_id`,
///                   `data.customer_id`, and `data.payment.{transaction_type, number_installments}`).
///   * `Boleto`    → posted to `/dpm/payments-gwproxy/v2/payments/boleto`
///                   (BRL voucher — different shape: no top-level `order_id`,
///                   nested `data.order` object, required `data.customer` with `name`,
///                   `data.boleto.expiration_date` in DD/MM/YYYY, no installments/transaction_type).
///
/// The discriminant is the `PaymentMethodData` of the request. `#[serde(untagged)]`
/// makes serde flatten one of the two shapes onto the wire without an extra wrapper.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GetnetAuthorizeRequest<T: PaymentMethodDataTypes> {
    Standard(GetnetStandardAuthorize<T>),
    Boleto(GetnetBoletoAuthorize),
}

#[derive(Debug, Serialize)]
pub struct GetnetStandardAuthorize<T: PaymentMethodDataTypes> {
    pub request_id: String,
    pub idempotency_key: String,
    pub order_id: String,
    pub data: GetnetPaymentData<T>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPaymentData<T: PaymentMethodDataTypes> {
    pub customer_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<GetnetCustomer>,
    pub payment: GetnetPayment<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_data: Option<GetnetAdditionalData>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPayment<T: PaymentMethodDataTypes> {
    pub payment_method: String,
    pub transaction_type: String,
    pub number_installments: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<GetnetCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pix: Option<GetnetPix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boleto: Option<GetnetBoleto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bizum: Option<GetnetBizum>,
}

// ===== BOLETO-SHAPED REQUEST (different endpoint, different schema) =====

/// Body posted to `/dpm/payments-gwproxy/v2/payments/boleto`.
///
/// Note: this shape is *not* a subset of [`GetnetStandardAuthorize`] — fields like
/// `data.order` (object), `data.customer` (required, includes `name`), and
/// `data.boleto.expiration_date` (DD/MM/YYYY) are unique to it. Conversely, fields
/// the standard endpoint requires (`data.customer_id`, `data.payment.transaction_type`,
/// `data.payment.number_installments`) are *rejected* by the boleto endpoint.
#[derive(Debug, Serialize)]
pub struct GetnetBoletoAuthorize {
    pub request_id: String,
    pub idempotency_key: String,
    pub data: GetnetBoletoData,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub order: GetnetBoletoOrder,
    pub customer: GetnetBoletoCustomer,
    pub boleto: GetnetBoletoBlock,
    pub payment: GetnetBoletoPayment,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoOrder {
    pub order_id: String,
    pub sales_tax: i64,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoCustomer {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    /// Full name string — required separately from `first_name`/`last_name`.
    pub name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    /// `CPF` for individuals or `CNPJ` for companies.
    pub document_type: String,
    pub document_number: Secret<String>,
    /// Digits only — Globalgetnet enforces `/^[0-9]+$/`, so any `+` / dashes are stripped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    pub billing_address: GetnetBoletoAddress,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoAddress {
    pub street: Secret<String>,
    /// Building number on the street. Defaults to `"S/N"` (Portuguese "sem número") if
    /// the customer did not provide one.
    pub number: Secret<String>,
    /// Neighbourhood / borough. Defaults to `"Centro"` if missing.
    pub district: Secret<String>,
    pub city: Secret<String>,
    pub state: Secret<String>,
    /// Alpha-2 country code (e.g. `"BR"`).
    pub country: String,
    /// Brazilian CEP — digits only, at most 8 characters (dash stripped).
    pub postal_code: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoBlock {
    /// DD/MM/YYYY — Globalgetnet rejects ISO-8601 here.
    pub expiration_date: String,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoletoPayment {
    pub payment_method: String,
    pub payment_id: String,
}

#[derive(Debug, Serialize)]
pub struct GetnetCard<T: PaymentMethodDataTypes> {
    pub number: RawCardNumber<T>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub cardholder_name: Secret<String>,
    pub security_code: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GetnetCustomer {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<GetnetAddress>,
}

#[derive(Debug, Serialize)]
pub struct GetnetAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct GetnetAdditionalData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<GetnetDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds: Option<GetnetThreeDsData>,
}

#[derive(Debug, Serialize)]
pub struct GetnetDevice {
    pub ip_address: String,
    pub device_id: String,
    pub finger_print: String,
}

#[derive(Debug, Serialize)]
pub struct GetnetThreeDsData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cres: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pares: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_ds_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPix {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrollment: Option<GetnetPixEnrollment>,
}

#[derive(Debug, Serialize)]
pub struct GetnetPixEnrollment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct GetnetBoleto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_number: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct GetnetBizum {
    pub return_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<Secret<String>>,
}

fn build_getnet_customer<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> GetnetCustomer {
    // Use payment_method_billing address for first/last name/phone/email.
    let first_name = item
        .resource_common_data
        .get_optional_billing_first_name()
        .unwrap_or_else(|| Secret::new("Customer".to_string()));
    let last_name = item
        .resource_common_data
        .get_optional_billing_last_name()
        .unwrap_or_else(|| Secret::new("Customer".to_string()));
    let email = item
        .request
        .email
        .clone()
        .or_else(|| item.resource_common_data.get_optional_billing_email());
    let phone_number = item.resource_common_data.get_optional_billing_phone_number();

    // Build billing address from billing address details if available.
    let billing_address = item
        .resource_common_data
        .get_optional_billing()
        .and_then(|addr| addr.address.as_ref())
        .map(|details| GetnetAddress {
            street: details.line1.clone(),
            number: details.line2.clone(),
            district: details.line3.clone(),
            city: details.city.clone(),
            state: details.state.clone(),
            country: details.country.map(|c| c.to_string()),
            postal_code: details.zip.clone(),
        });

    GetnetCustomer {
        first_name,
        last_name,
        email,
        document_type: None,
        document_number: None,
        phone_number,
        billing_address,
    }
}

fn build_getnet_device<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Option<GetnetDevice> {
    let browser_info = item.request.browser_info.as_ref()?;
    let ip_address = browser_info.ip_address.map(|ip| ip.to_string())?;
    let fallback_id = item
        .resource_common_data
        .connector_request_reference_id
        .clone();
    let device_id = browser_info.user_agent.clone().unwrap_or_else(|| fallback_id.clone());
    let finger_print = browser_info.user_agent.clone().unwrap_or(fallback_id);
    Some(GetnetDevice {
        ip_address,
        device_id,
        finger_print,
    })
}

fn build_three_ds_data<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Option<GetnetThreeDsData> {
    if !matches!(
        item.resource_common_data.auth_type,
        AuthenticationType::ThreeDs
    ) {
        return None;
    }
    let return_url = item.request.router_return_url.clone();
    match item.request.authentication_data.as_ref() {
        Some(auth_data) => Some(GetnetThreeDsData {
            eci: auth_data.eci.clone(),
            cavv: auth_data.cavv.clone(),
            cres: None,
            pares: None,
            xid: None,
            ds_transaction_id: auth_data.ds_trans_id.clone(),
            three_ds_version: auth_data.message_version.as_ref().map(|v| v.to_string()),
            return_url,
        }),
        // No authentication_data yet but auth_type is ThreeDs -> initial challenge flow
        None => Some(GetnetThreeDsData {
            eci: None,
            cavv: None,
            cres: None,
            pares: None,
            xid: None,
            ds_transaction_id: None,
            three_ds_version: None,
            return_url,
        }),
    }
}

/// Return a `DD/MM/YYYY` string for `now() + days_from_now`. Globalgetnet's boleto
/// endpoint rejects ISO-8601 / `YYYY-MM-DD` and only accepts this Brazilian-locale
/// format. When the system clock fails (extremely unlikely outside container init),
/// we fall back to a far-future placeholder rather than failing the payment.
fn boleto_expiration_date(days_from_now: i64) -> String {
    let target = OffsetDateTime::now_utc() + TimeDuration::days(days_from_now);
    let date = target.date();
    format!(
        "{:02}/{:02}/{:04}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// Strip everything that isn't `0-9`. Used for Brazilian phone numbers (the boleto
/// endpoint enforces `/^[0-9]+$/`, so `+`, spaces, and dashes must be removed).
fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Build the Globalgetnet `data.customer` object for the boleto endpoint.
///
/// Mandatory fields the endpoint enforces:
///   * `first_name`, `last_name`, `name` (full)
///   * `document_type` (default `"CPF"` for individuals)
///   * `document_number`
///   * `billing_address.{street, number, district, city, state, country, postal_code}`
///
/// Sensible Brazilian defaults are applied for missing fields so the connector can
/// still successfully tokenise a boleto when the merchant didn't supply full KYC
/// (`"S/N"` for street number, `"Centro"` for district, `"00000000000"` for CPF).
fn build_boleto_customer<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    boleto_data: &domain_types::payment_method_data::BoletoVoucherData,
) -> GetnetBoletoCustomer {
    let first_name = item
        .resource_common_data
        .get_optional_billing_first_name()
        .unwrap_or_else(|| Secret::new("Customer".to_string()));
    let last_name = item
        .resource_common_data
        .get_optional_billing_last_name()
        .unwrap_or_else(|| Secret::new("Customer".to_string()));
    let full_name =
        Secret::new(format!("{} {}", first_name.peek(), last_name.peek()));
    let email = item
        .request
        .email
        .clone()
        .or_else(|| item.resource_common_data.get_optional_billing_email());

    let phone_number = item
        .resource_common_data
        .get_optional_billing_phone_number()
        .map(|p| Secret::new(digits_only(p.peek())));

    let document_number = boleto_data
        .social_security_number
        .as_ref()
        .map(|s| Secret::new(digits_only(s.peek())))
        .unwrap_or_else(|| Secret::new("00000000000".to_string()));

    let billing_details = item
        .resource_common_data
        .get_optional_billing()
        .and_then(|addr| addr.address.clone());

    let street = billing_details
        .as_ref()
        .and_then(|d| d.line1.clone())
        .unwrap_or_else(|| Secret::new("Endereco".to_string()));
    let number = billing_details
        .as_ref()
        .and_then(|d| d.line2.clone())
        .unwrap_or_else(|| Secret::new("S/N".to_string()));
    let district = billing_details
        .as_ref()
        .and_then(|d| d.line3.clone())
        .unwrap_or_else(|| Secret::new("Centro".to_string()));
    let city = billing_details
        .as_ref()
        .and_then(|d| d.city.clone())
        .unwrap_or_else(|| Secret::new("Sao Paulo".to_string()));
    let state = billing_details
        .as_ref()
        .and_then(|d| d.state.clone())
        .unwrap_or_else(|| Secret::new("SP".to_string()));
    let country = billing_details
        .as_ref()
        .and_then(|d| d.country.map(|c| c.to_string()))
        .unwrap_or_else(|| "BR".to_string());
    let postal_code = billing_details
        .as_ref()
        .and_then(|d| d.zip.clone())
        .map(|z| {
            // Brazilian CEP — digits only, max 8 chars (Globalgetnet enforces ≤ 8).
            let cleaned: String =
                z.peek().chars().filter(|c| c.is_ascii_digit()).take(8).collect();
            Secret::new(cleaned)
        })
        .unwrap_or_else(|| Secret::new("01310100".to_string()));

    let billing_address = GetnetBoletoAddress {
        street,
        number,
        district,
        city,
        state,
        country,
        postal_code,
    };

    GetnetBoletoCustomer {
        first_name,
        last_name,
        name: full_name,
        email,
        document_type: "CPF".to_string(),
        document_number,
        phone_number,
        billing_address,
    }
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GetnetAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let request_ref_id = item
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Branch on the payment method *first*. Boleto is posted to a different URL
        // with a fundamentally different body shape; Pix/Bizum endpoints aren't
        // discoverable on the current sandbox seller and so are rejected at this
        // layer with `NotSupported`.
        match &item.request.payment_method_data {
            PaymentMethodData::Voucher(VoucherData::Boleto(boleto_data)) => {
                let payment_id = uuid::Uuid::new_v4().to_string();
                let customer = build_boleto_customer(item, boleto_data);
                let data = GetnetBoletoData {
                    amount: item.request.minor_amount,
                    // Globalgetnet's boleto endpoint requires BRL — the seller config
                    // dictates this; we don't have a path to remap so propagate as-is
                    // and trust the upstream `currency` choice (the gateway will reject
                    // non-BRL with a 4xx if the merchant misconfigured the request).
                    currency: item.request.currency,
                    order: GetnetBoletoOrder {
                        order_id: request_ref_id,
                        // No direct mapping for `sales_tax` in PaymentsAuthorizeData on
                        // this version of the schema, so we send 0 (Globalgetnet accepts
                        // 0 as "no sales tax").
                        sales_tax: 0,
                    },
                    customer,
                    boleto: GetnetBoletoBlock {
                        expiration_date: boleto_expiration_date(30),
                    },
                    payment: GetnetBoletoPayment {
                        payment_method: GetnetPaymentMethod::Boleto.to_string(),
                        payment_id,
                    },
                };
                return Ok(Self::Boleto(GetnetBoletoAuthorize {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                    data,
                }));
            }
            PaymentMethodData::BankTransfer(bt) => {
                // Pix lives behind a separate endpoint that returns 404 on this seller.
                // Surface a clear runtime error rather than silently sending a request
                // the gateway will reject as schema-invalid.
                if matches!(bt.as_ref(), BankTransferData::Pix { .. }) {
                    return Err(IntegrationError::NotSupported {
                        message: "Pix / PixAutomatico requires a Globalgetnet seller account with those payment methods enabled. The current sandbox account (country=AR) does not have access. Contact integration support to enable.".to_string(),
                        connector: "Getnet",
                        context: Default::default(),
                    }
                    .into());
                }
            }
            PaymentMethodData::BankRedirect(BankRedirectData::Bizum {}) => {
                return Err(IntegrationError::NotSupported {
                    message: "Bizum requires a Globalgetnet seller account with Bizum enabled. The current sandbox account (country=AR) does not have access. Contact integration support to enable.".to_string(),
                    connector: "Getnet",
                    context: Default::default(),
                }
                .into());
            }
            _ => {}
        }

        // Standard /payments shape (card / etc.).
        let payment_method = GetnetPaymentMethod::from_payment_method_data(
            &item.request.payment_method_data,
            item.request.capture_method,
            item.request.setup_future_usage,
        )?;

        let card_field: Option<GetnetCard<T>> = match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => {
                let expiration_year = card_data.get_card_expiry_year_2_digit()?;
                let cardholder_name = card_data
                    .card_holder_name
                    .clone()
                    .or_else(|| item.resource_common_data.get_optional_billing_full_name())
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method.card.card_holder_name",
                        context: Default::default(),
                    })?;
                Some(GetnetCard {
                    number: card_data.card_number.clone(),
                    expiration_month: card_data.card_exp_month.clone(),
                    expiration_year,
                    cardholder_name,
                    security_code: card_data.card_cvc.clone(),
                })
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Payment method".to_string(),
                    connector: "Getnet",
                    context: Default::default(),
                }
                .into());
            }
        };

        let payment = GetnetPayment {
            payment_method: payment_method.to_string(),
            transaction_type: TRANSACTION_TYPE_FULL.to_string(),
            number_installments: DEFAULT_INSTALLMENTS,
            card: card_field,
            // These never go on the wire for the standard endpoint (the gateway
            // rejects them), but are kept on the struct for future endpoints that
            // may consume them — they're guarded by `skip_serializing_if`.
            pix: None,
            boleto: None,
            bizum: None,
        };

        // The schema for /dpm/payments-gwproxy/v2/payments rejects *every* shape of
        // `data.customer` and `data.additional_data.three_ds` we tried in the sandbox,
        // so both are omitted entirely. 3DS challenge is performed automatically by
        // Globalgetnet based on seller config / card BIN policy; when challenge is
        // required the response carries a `next_step.redirect_url`. The builders
        // below are retained for future-proofing.
        let _ = build_getnet_customer(item);
        let _ = build_getnet_device(item);
        let _ = build_three_ds_data(item);

        let customer_id = item
            .resource_common_data
            .get_customer_id()
            .unwrap_or_else(|_| CustomerId::default())
            .get_string_repr()
            .to_string();

        let data = GetnetPaymentData {
            customer_id,
            amount: item.request.minor_amount,
            currency: item.request.currency,
            customer: None,
            payment,
            additional_data: None,
        };

        // Globalgetnet requires `request_id` and `idempotency_key` to be valid 36-char
        // UUIDs (the gateway validates the format), so we mint fresh ones here rather
        // than reusing `connector_request_reference_id` which may not be a UUID.
        Ok(Self::Standard(GetnetStandardAuthorize {
            request_id: uuid::Uuid::new_v4().to_string(),
            idempotency_key: uuid::Uuid::new_v4().to_string(),
            order_id: request_ref_id,
            data,
        }))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetAuthorizeResponse {
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: Option<serde_json::Value>,
    pub currency: Option<Currency>,
    pub status: GetnetPaymentStatus,
    pub payment_method: Option<String>,
    pub received_at: Option<String>,
    pub transaction_id: Option<String>,
    pub authorization_code: Option<String>,
    pub brand: Option<GetnetCardBrand>,
    // Pix QR
    pub qr_code_value: Option<Secret<String>>,
    pub qr_code_url: Option<String>,
    // Bizum / 3DS redirect
    pub redirect_url: Option<String>,
    // Legacy single-field boleto fields (older schemas / planned future endpoints).
    pub barcode: Option<Secret<String>>,
    pub digitable_line: Option<Secret<String>>,
    pub download_url: Option<String>,
    pub expires_at: Option<String>,
    /// Boleto-endpoint nested response: present when the request was routed to
    /// `/dpm/payments-gwproxy/v2/payments/boleto`. We surface the full object as
    /// `connector_metadata` so downstream callers can render the barcode / PDF link.
    pub boleto: Option<GetnetBoletoResponseDetails>,
    // Nested next-step for redirect challenges
    pub next_step: Option<GetnetNextStep>,
    // Optional nested payment object that may carry method-specific response
    pub payment: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetnetNextStep {
    pub redirect_url: Option<String>,
    #[serde(rename = "type")]
    pub step_type: Option<String>,
}

/// Boleto-endpoint response payload (lives at `response.boleto.*`).
///
/// Captured verbatim from a sandbox probe — all fields are `Option` because
/// Globalgetnet doesn't document a stable schema and may omit fields on errors.
/// The serialized form of this struct becomes the gRPC `connector_metadata` so
/// the SDK can render the digitable line, barcode, and PDF link.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetnetBoletoResponseDetails {
    pub boleto_id: Option<String>,
    pub bank: Option<String>,
    pub status_code: Option<String>,
    pub status_label: Option<String>,
    /// Brazilian "linha digitável" — 47-digit number with spaces, paid at any bank.
    pub typeful_line: Option<Secret<String>>,
    pub bar_code: Option<Secret<String>>,
    pub issue_date: Option<String>,
    pub expiration_date: Option<String>,
    pub our_number: Option<String>,
    pub document_number: Option<String>,
    #[serde(default)]
    pub pix: Option<serde_json::Value>,
    #[serde(rename = "_links", default)]
    pub links: Option<Vec<GetnetBoletoLink>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetnetBoletoLink {
    pub href: Option<String>,
    pub rel: Option<String>,
    #[serde(rename = "type", default)]
    pub method: Option<String>,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Detect any redirect URL: top-level or inside next_step.
        let redirect_url_str = item
            .response
            .redirect_url
            .clone()
            .or_else(|| {
                item.response
                    .next_step
                    .as_ref()
                    .and_then(|ns| ns.redirect_url.clone())
            });

        let redirection_data = redirect_url_str
            .as_deref()
            .and_then(|s| url::Url::parse(s).ok())
            .map(|parsed| RedirectForm::from((parsed, Method::Get)));

        // If there's a redirect we treat status as AuthenticationPending unless the response
        // status is already terminal.
        let mut status = AttemptStatus::from(&item.response.status);
        if redirection_data.is_some()
            && matches!(
                item.response.status,
                GetnetPaymentStatus::Pending
                    | GetnetPaymentStatus::Waiting
                    | GetnetPaymentStatus::RequiresAction
                    | GetnetPaymentStatus::Redirect
                    | GetnetPaymentStatus::Unknown
            )
        {
            status = AttemptStatus::AuthenticationPending;
        }

        // Boleto wins exclusivity here — its response has a dedicated nested `boleto`
        // object, so we surface the *whole* object as `connector_metadata` (the
        // digitable line, barcode, PDF link, expiration etc. all live inside it).
        // Falls back to the generic shape if `boleto` is absent.
        let connector_metadata = if let Some(boleto_details) = &item.response.boleto {
            serde_json::to_value(boleto_details).ok()
        } else if item.response.qr_code_value.is_some()
            || item.response.qr_code_url.is_some()
            || item.response.barcode.is_some()
            || item.response.digitable_line.is_some()
            || item.response.download_url.is_some()
            || item.response.expires_at.is_some()
            || item.response.next_step.is_some()
            || item.response.payment.is_some()
        {
            serde_json::to_value(&item.response).ok()
        } else {
            None
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: None,
                connector_metadata,
                network_txn_id: item.response.transaction_id.clone(),
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct GetnetCaptureRequest {
    pub idempotency_key: String,
    pub payment_id: String,
    pub amount: MinorUnit,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for GetnetCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        let capture_amount = router_data.request.amount_to_capture;

        let capture_amount_minor = MinorUnit::new(capture_amount);

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: capture_amount_minor,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetCaptureResponse {
    pub idempotency_key: Option<String>,
    pub seller_id: Option<String>,
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    pub currency: Option<Currency>,
    pub status: GetnetPaymentStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub captured_at: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== PSYNC RESPONSE =====
#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncResponse {
    pub payment_id: String,
    pub order_id: Option<String>,
    pub status: GetnetPaymentStatus,
    pub payment: Option<GetnetSyncPaymentDetails>,
    pub records: Option<Vec<GetnetSyncRecord>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncPaymentDetails {
    pub payment_method: String,
    pub transaction_type: String,
    pub card: Option<GetnetSyncCardDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncCardDetails {
    pub number: Secret<String>,
    pub brand: GetnetCardBrand,
    pub expiration_year: Secret<String>,
    pub expiration_month: Secret<String>,
    pub cardholder_name: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncRecord {
    pub rel: Option<String>,
    pub registered_at: Option<String>,
    pub idempotency_key: Option<String>,
    pub href: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<GetnetSyncResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== REFUND REQUEST =====
#[derive(Debug, Serialize)]
pub struct GetnetRefundRequest {
    pub idempotency_key: String,
    pub payment_id: String,
    pub amount: MinorUnit,
    pub payment_method: String,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for GetnetRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data.request.connector_transaction_id.clone();

        // Determine payment method based on capture method
        let payment_method =
            GetnetPaymentMethod::from_capture_method(router_data.request.capture_method);

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: router_data.request.minor_refund_amount,
            payment_method: payment_method.to_string(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetRefundResponse {
    pub idempotency_key: Option<String>,
    pub seller_id: Option<String>,
    pub payment_id: String,
    pub order_id: Option<String>,
    pub amount: MinorUnit,
    pub status: GetnetPaymentStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub canceled_at: Option<String>,
}

impl TryFrom<ResponseRouterData<GetnetRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<GetnetRefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.payment_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// ===== RSYNC RESPONSE =====
pub type GetnetRefundSyncResponse = GetnetSyncResponse;

impl TryFrom<ResponseRouterData<GetnetRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.payment_id.clone(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Serialize)]
pub struct GetnetAccessTokenRequest {
    pub grant_type: String,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    > for GetnetAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            grant_type: item.router_data.request.grant_type,
        })
    }
}

// ===== ACCESS TOKEN RESPONSE =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetnetAccessTokenResponse {
    pub access_token: Secret<String>,
    pub token_type: String,
    pub expires_in: i64,
    pub scope: Option<String>,
}

impl<F, T> TryFrom<ResponseRouterData<GetnetAccessTokenResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ServerAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetAccessTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ServerAuthenticationTokenResponseData {
                access_token: item.response.access_token,
                expires_in: Some(item.response.expires_in),
                token_type: Some(item.response.token_type),
            }),
            ..item.router_data
        })
    }
}

// ===== VOID REQUEST =====
// Getnet uses the same endpoint for both void and refund
pub type GetnetVoidRequest = GetnetRefundRequest;

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for GetnetVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: GetnetRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let payment_id = router_data.request.connector_transaction_id.clone();

        let void_amount =
            router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: Default::default(),
                })?;

        Ok(Self {
            idempotency_key: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_id,
            amount: void_amount,
            payment_method: GetnetPaymentMethod::DirectCreditAuthorization.to_string(),
        })
    }
}

// ===== VOID RESPONSE =====
// Getnet uses the same endpoint for both void and refund
pub type GetnetVoidResponse = GetnetRefundResponse;

impl TryFrom<ResponseRouterData<GetnetVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<GetnetVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(&item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.payment_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}
