use crate::{connectors::getnet::GetnetRouterData, types::ResponseRouterData};
use common_enums::{AttemptStatus, AuthenticationType, Currency, RefundStatus};
use common_utils::{request::Method, types::MinorUnit, Email};
use domain_types::errors::{ConnectorError, IntegrationError, IntegrationErrorContext};
use domain_types::router_request_types::AuthenticationData;
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PaymentMethodToken, PostAuthenticate,
        PreAuthenticate, RSync, Refund, ServerAuthenticationToken, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ResponseId, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
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
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use time::{Duration as TimeDuration, OffsetDateTime};

const TRANSACTION_TYPE_FULL: &str = "FULL";
const DEFAULT_INSTALLMENTS: i32 = 1;

/// Document type sent on the boleto `data.customer`. Globalgetnet expects the
/// Brazilian individual-taxpayer document type ("CPF") for boleto payers.
const BOLETO_DOCUMENT_TYPE_CPF: &str = "CPF";

/// Number of days from "now" used as the boleto due date (`data.boleto.expiration_date`).
/// 30 days is the standard Brazilian boleto payment window — a boleto must carry a
/// future due date and 30 days is the conventional default merchants use when the
/// caller doesn't specify one (UCS does not currently surface a per-payment boleto
/// expiry on `PaymentsAuthorizeData`).
const BOLETO_DUE_DAYS: i64 = 30;

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
    /// Pix QR has elapsed its expiration window without payment — terminal failure.
    Expired,
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
            | GetnetPaymentStatus::Error
            | GetnetPaymentStatus::Expired => Self::Failure,
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
            | GetnetPaymentStatus::Error
            | GetnetPaymentStatus::Expired => Self::Failure,
            _ => Self::Pending,
        }
    }
}

/// Top-level Authorize request envelope.
///
/// Globalgetnet uses *different* request shapes for different payment methods:
///   * `Standard`  → posted to `/dpm/payments-gwproxy/v2/payments`
///     (card / wallet etc. — legacy shape with top-level `order_id`,
///     `data.customer_id`, and `data.payment.{transaction_type, number_installments}`).
///   * `Boleto`    → posted to `/dpm/payments-gwproxy/v2/payments/boleto`
///     (BRL voucher — different shape: no top-level `order_id`,
///     nested `data.order` object, required `data.customer` with `name`,
///     `data.boleto.expiration_date` in DD/MM/YYYY, no installments/transaction_type).
///   * `Pix`       → posted to `/dpm/payments-gwproxy/v2/payments/qrcode/pix`
///     (instant Pix QR — flat shape with `amount`, `currency`, `order_id`,
///     `customer_id`, `idempotency_key`; NO `data` wrapper, NO `request_id`).
///
/// The discriminant is the `PaymentMethodData` of the request. `#[serde(untagged)]`
/// makes serde flatten one of the shapes onto the wire without an extra wrapper.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum GetnetAuthorizeRequest<T: PaymentMethodDataTypes> {
    Standard(Box<GetnetStandardAuthorize<T>>),
    Boleto(Box<GetnetBoletoAuthorize>),
    Pix(GetnetPixAuthorize),
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
    // 3DS authentication result fields — populated inline on the `/payments`
    // payload after PreAuthenticate / Authenticate / PostAuthenticate produce
    // an `AuthenticationData`. All `Option`-wrapped with `skip_serializing_if`
    // so non-3DS card requests serialize identically to the pre-3DS shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucaf: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tdsdsxid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tdsver: Option<String>,
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

// ===== PIX-QR REQUEST (different endpoint, flat schema) =====
//
// Body posted to `/dpm/payments-gwproxy/v2/payments/qrcode/pix`. Unlike the
// `Standard` and `Boleto` shapes this is *flat* — no `data` wrapper and no
// `request_id`. `amount` serializes as a JSON integer (cents) thanks to the
// `MinorUnit` Serialize impl.
#[derive(Debug, Serialize)]
pub struct GetnetPixAuthorize {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub order_id: String,
    pub customer_id: String,
    pub idempotency_key: String,
}

/// Card block sent on the `/payments` standard-shape Authorize request.
///
/// Two mutually-exclusive ways to identify the PAN:
///   * `number` — raw PAN, sent when the caller has not pre-tokenized
///     the card via the Cofre `PaymentMethodToken` flow.
///   * `number_token` — opaque 128-char hex token returned by Cofre
///     (`POST /dpm/cofre-gw-proxy/v1/tokens/card`). Used when UCS stashed
///     the token in `PaymentFlowData.session_token` on a previous leg.
///
/// Both are `Option`-wrapped with `skip_serializing_if = "Option::is_none"` so
/// the on-wire JSON contains *exactly one* of the two fields and the legacy
/// no-token shape is byte-identical to before.
#[derive(Debug, Serialize)]
pub struct GetnetCard<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<RawCardNumber<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_token: Option<Secret<String>>,
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
    pub ip_address: Secret<String>,
    pub device_id: Secret<String>,
    pub finger_print: Secret<String>,
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

/// Format a `Date` as Globalgetnet's required Brazilian-locale `DD/MM/YYYY`. The boleto
/// endpoint rejects ISO-8601 / `YYYY-MM-DD`.
fn format_boleto_date(date: time::Date) -> String {
    format!(
        "{:02}/{:02}/{:04}",
        date.day(),
        u8::from(date.month()),
        date.year()
    )
}

/// Return a `DD/MM/YYYY` boleto due date for `now() + days_from_now`. Used as the
/// fallback when the caller didn't supply an explicit due date.
fn boleto_expiration_date(days_from_now: i64) -> String {
    let target = OffsetDateTime::now_utc() + TimeDuration::days(days_from_now);
    format_boleto_date(target.date())
}

/// Resolve the boleto due date: prefer the merchant-supplied `expiration_date`, else
/// fall back to the connector default ([`BOLETO_DUE_DAYS`] from now). Mirrors how
/// connectors source the Pix expiry from the request and only default when absent.
fn boleto_due_date(boleto_data: &domain_types::payment_method_data::BoletoVoucherData) -> String {
    match boleto_data.expiration_date {
        Some(dt) => format_boleto_date(dt.date()),
        None => boleto_expiration_date(BOLETO_DUE_DAYS),
    }
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
/// Globalgetnet enforces every one of these on the `/payments/boleto` endpoint, so
/// they are treated as required: when the caller omits any of them we surface a
/// `MissingRequiredField` error (with the exact field path) rather than silently
/// injecting placeholder KYC that the gateway — or the Brazilian tax authority —
/// would reject downstream.
fn build_boleto_customer<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    boleto_data: &domain_types::payment_method_data::BoletoVoucherData,
) -> Result<GetnetBoletoCustomer, error_stack::Report<IntegrationError>> {
    let first_name = item.resource_common_data.get_billing_first_name()?;
    let last_name = item.resource_common_data.get_billing_last_name()?;
    let full_name = Secret::new(format!("{} {}", first_name.peek(), last_name.peek()));
    let email = item
        .request
        .email
        .clone()
        .or_else(|| item.resource_common_data.get_optional_billing_email());

    let phone_number = item
        .resource_common_data
        .get_optional_billing_phone_number()
        .map(|p| Secret::new(digits_only(p.peek())));

    // CPF — mandatory for boleto in Brazil; a placeholder document number would be
    // rejected by the gateway and is meaningless for reconciliation.
    let document_number = boleto_data
        .social_security_number
        .as_ref()
        .map(|s| Secret::new(digits_only(s.peek())))
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data.voucher.boleto.social_security_number",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide the payer's CPF — Globalgetnet requires it on every boleto."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;

    // Globalgetnet's boleto schema rejects a partial address — every field is required,
    // so use the error-propagating getters and surface a MissingRequiredField on absence.
    let street = item.resource_common_data.get_billing_line1()?;
    let number = item.resource_common_data.get_billing_line2()?;
    let district = item.resource_common_data.get_billing_line3()?;
    let city = item.resource_common_data.get_billing_city()?;
    let state = item.resource_common_data.get_billing_state()?;
    let country = item.resource_common_data.get_billing_country()?.to_string();
    // Brazilian CEP — digits only, max 8 chars (Globalgetnet enforces ≤ 8).
    let zip = item.resource_common_data.get_billing_zip()?;
    let postal_code = Secret::new(
        zip.peek()
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(8)
            .collect::<String>(),
    );

    let billing_address = GetnetBoletoAddress {
        street,
        number,
        district,
        city,
        state,
        country,
        postal_code,
    };

    Ok(GetnetBoletoCustomer {
        first_name,
        last_name,
        name: full_name,
        email,
        document_type: BOLETO_DOCUMENT_TYPE_CPF.to_string(),
        document_number,
        phone_number,
        billing_address,
    })
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
                let customer = build_boleto_customer(item, boleto_data)?;
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
                        expiration_date: boleto_due_date(boleto_data),
                    },
                    payment: GetnetBoletoPayment {
                        payment_method: GetnetPaymentMethod::Boleto.to_string(),
                        payment_id,
                    },
                };
                return Ok(Self::Boleto(Box::new(GetnetBoletoAuthorize {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    idempotency_key: item.resource_common_data.get_merchant_request_id()?,
                    data,
                })));
            }
            PaymentMethodData::BankTransfer(bt) => {
                if matches!(bt.as_ref(), BankTransferData::Pix { .. }) {
                    // Pix Automatico (recurring / off-session) is NOT exposed via the
                    // Globalgetnet Regional API — the Subscriptions API (`/rpy/*`)
                    // accepts only credit/debit cards, the `/payments/qrcode/<sub>`
                    // family contains only the instant `pix` variant, and the
                    // `Combined Payments` docs explicitly state APMs like PIX are not
                    // supported. Surface a precise runtime error so callers know to
                    // use the merchant-direct integration instead of debugging seller
                    // configuration.
                    if matches!(
                        item.request.setup_future_usage,
                        Some(common_enums::FutureUsage::OffSession)
                    ) {
                        return Err(IntegrationError::NotSupported {
                            message: "Pix Automatico is not exposed via the Globalgetnet Regional API. Only Pix QR (instant) is supported. Use the merchant-direct integration if Pix Automatico is required.".to_string(),
                            connector: "Getnet",
                            context: Default::default(),
                        }
                        .into());
                    }
                    // Pix QR (instant) — flat body posted to
                    // `/dpm/payments-gwproxy/v2/payments/qrcode/pix`. Globalgetnet
                    // requires `customer_id` on this endpoint, so propagate a clear
                    // error when it's absent rather than sending an empty default.
                    let customer_id = item
                        .resource_common_data
                        .get_customer_id()?
                        .get_string_repr()
                        .to_string();
                    return Ok(Self::Pix(GetnetPixAuthorize {
                        amount: item.request.minor_amount,
                        currency: item.request.currency,
                        order_id: request_ref_id,
                        customer_id,
                        idempotency_key: item.resource_common_data.get_merchant_request_id()?,
                    }));
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
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Provide the cardholder name, or a billing first/last name to derive it from."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: None,
                        },
                    })?;
                // Prefer the Cofre token when present. UCS stashes the token
                // returned by `PaymentMethodToken` on `session_token`; when set,
                // we send `number_token` instead of the raw PAN. Globalgetnet
                // rejects bodies that include `number` and `number_token` (or
                // `bin`) together, so this is strictly one-or-the-other.
                let (number, number_token) =
                    match item.resource_common_data.get_session_token().ok() {
                        Some(token) => (None, Some(Secret::new(token))),
                        None => (Some(card_data.card_number.clone()), None),
                    };
                Some(GetnetCard {
                    number,
                    number_token,
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

        // When this Authorize is the final leg of a 3DS trio
        // (PreAuthenticate -> Authenticate [-> PostAuthenticate] -> Authorize),
        // UCS forwards the resulting `AuthenticationData` here. Globalgetnet's
        // `/payments` endpoint rejects `data.payment.three_ds` as a sub-object
        // but accepts inline fields: xid / eci / ucaf / tdsdsxid / tdsver. We
        // populate them only on the Card branch when 3DS is active AND we have
        // an authentication result; otherwise the fields stay None and
        // `skip_serializing_if` keeps the wire format identical to the no-3DS
        // case (preserving the gateway-driven frictionless path).
        let (xid_field, eci_field, ucaf_field, tdsdsxid_field, tdsver_field) = if matches!(
            item.resource_common_data.auth_type,
            AuthenticationType::ThreeDs
        ) {
            match item.request.authentication_data.as_ref() {
                Some(ad) => (
                    ad.transaction_id.clone(),
                    ad.eci.clone(),
                    ad.cavv.clone(),
                    ad.ds_trans_id.clone(),
                    ad.message_version.as_ref().map(|v| v.to_string()),
                ),
                None => (None, None, None, None, None),
            }
        } else {
            (None, None, None, None, None)
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
            xid: xid_field,
            eci: eci_field,
            ucaf: ucaf_field,
            tdsdsxid: tdsdsxid_field,
            tdsver: tdsver_field,
        };

        // The /dpm/payments-gwproxy/v2/payments schema rejects `data.customer` and
        // `data.additional_data.three_ds`, so both are omitted. 3DS challenge is
        // initiated by Globalgetnet itself based on seller / BIN policy; when
        // required the response carries `next_step.redirect_url`.
        // `customer_id` is the only customer reference Globalgetnet's `/payments`
        // endpoint accepts, so propagate a clear error when it's missing instead of
        // sending an empty default the gateway would reject.
        let customer_id = item
            .resource_common_data
            .get_customer_id()?
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

        // `request_id` is a fresh per-attempt UUID. `idempotency_key` reuses the caller's
        // `merchant_request_id` so a retried request de-duplicates; it is required.
        Ok(Self::Standard(Box::new(GetnetStandardAuthorize {
            request_id: uuid::Uuid::new_v4().to_string(),
            idempotency_key: item.resource_common_data.get_merchant_request_id()?,
            order_id: request_ref_id,
            data,
        })))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetAuthorizeResponse {
    pub payment_id: String,
    #[serde(default)]
    pub order_id: Option<String>,
    #[serde(default)]
    pub amount: Option<serde_json::Value>,
    #[serde(default)]
    pub currency: Option<Currency>,
    pub status: GetnetPaymentStatus,
    #[serde(default)]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub received_at: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub authorization_code: Option<String>,
    #[serde(default)]
    pub brand: Option<GetnetCardBrand>,
    // Pix QR
    #[serde(default)]
    pub qr_code_value: Option<Secret<String>>,
    #[serde(default)]
    pub qr_code_url: Option<String>,
    // Bizum / 3DS redirect
    #[serde(default)]
    pub redirect_url: Option<String>,
    // Legacy single-field boleto fields (older schemas / planned future endpoints).
    #[serde(default)]
    pub barcode: Option<Secret<String>>,
    #[serde(default)]
    pub digitable_line: Option<Secret<String>>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Boleto-endpoint nested response: present when the request was routed to
    /// `/dpm/payments-gwproxy/v2/payments/boleto`. We surface the full object as
    /// `connector_metadata` so downstream callers can render the barcode / PDF link.
    #[serde(default)]
    pub boleto: Option<GetnetBoletoResponseDetails>,
    // Nested next-step for redirect challenges
    #[serde(default)]
    pub next_step: Option<GetnetNextStep>,
    // Optional nested payment object that may carry method-specific response
    #[serde(default)]
    pub payment: Option<serde_json::Value>,
    /// Pix-QR endpoint nested response: present when the request was routed to
    /// `/dpm/payments-gwproxy/v2/payments/qrcode/pix`. Carries the EMV BR Code
    /// `qr_code` ("Copia e Cola"), creation/expiration timestamps, and PSP code.
    #[serde(default)]
    pub additional_data: Option<GetnetPixAdditionalData>,
    /// Pix-QR endpoint accompaniment fields. Present so deserialization doesn't
    /// fail on unknown-shape responses.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_split: Option<bool>,
}

/// Pix-QR `additional_data` block. The `qr_code` field is the EMV BR Code
/// "Copia e Cola" payload (a self-describing payment string, NOT a URL); the
/// SDK is expected to either render it as a QR code or display it for the
/// customer to copy/paste into their banking app.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GetnetPixAdditionalData {
    pub transaction_id: Option<String>,
    pub qr_code: Option<String>,
    pub creation_date_qrcode: Option<String>,
    pub expiration_date_qrcode: Option<String>,
    pub psp_code: Option<String>,
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
    pub our_number: Option<Secret<String>>,
    pub document_number: Option<Secret<String>>,
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
        let redirect_url_str = item.response.redirect_url.clone().or_else(|| {
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

        // Surface method-specific response data via `connector_metadata` so the
        // SDK can render barcode / digitable line / Pix QR string. Precedence:
        //   1. Boleto: dedicated nested `boleto` object → serialize as-is.
        //   2. Pix QR: dedicated `additional_data` block (carries the EMV BR
        //      Code `qr_code` string + expiration) → serialize as-is.
        //   3. Generic fallback for legacy / other shapes.
        let connector_metadata = if let Some(boleto_details) = &item.response.boleto {
            serde_json::to_value(boleto_details).ok()
        } else if let Some(pix_details) = &item.response.additional_data {
            // The presence of `additional_data.qr_code` is the strongest signal
            // that this response came from `/payments/qrcode/pix`. Pix QR is not
            // a redirect flow, so we do not set redirection_data — the SDK is
            // expected to present the EMV BR Code to the customer.
            if pix_details.qr_code.is_some() {
                serde_json::to_value(pix_details).ok()
            } else {
                serde_json::to_value(&item.response).ok()
            }
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
    #[serde(default)]
    pub order_id: Option<String>,
    pub status: GetnetPaymentStatus,
    #[serde(default)]
    pub payment: Option<GetnetSyncPaymentDetails>,
    #[serde(default)]
    pub records: Option<Vec<GetnetSyncRecord>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetnetSyncPaymentDetails {
    #[serde(default)]
    pub payment_method: String,
    #[serde(default)]
    pub transaction_type: String,
    #[serde(default)]
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
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Provide the amount to void — Globalgetnet's cancel endpoint requires it."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: None,
                },
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

// =====================================================================
// 3DS FLOW SURFACE: PreAuthenticate / Authenticate / PostAuthenticate
// =====================================================================
//
// Globalgetnet exposes three companion endpoints under the
// `/dpm/security-gwproxy/v2/` prefix:
//
//   * POST `/enrolments-initial`  -> PreAuthenticate
//   * POST `/enrolments-continue` -> Authenticate
//   * POST `/validations`         -> PostAuthenticate (CRES validation after ACS)
//
// All three share an identical *response* schema (`GetnetThreeDsResponse`)
// even though some fields are only populated on certain legs. Sandbox
// behavior: `status` strings are lowercased ("pending enrollment continue")
// so deserialization uses `alias =` to accept both canonical and
// sandbox-shaped variants, plus an English / British spelling pair for the
// enrolment/enrollment word.

#[derive(Debug, Serialize)]
pub struct GetnetPreAuthenticateRequest {
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub extra_fields: GetnetThreeDsExtraFields,
}

#[derive(Debug, Serialize)]
pub struct GetnetThreeDsExtraFields {
    pub billing_address: GetnetThreeDsAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<GetnetThreeDsAddress>,
}

#[derive(Debug, Serialize)]
pub struct GetnetThreeDsAddress {
    pub street: Secret<String>,
    pub number: Secret<String>,
    /// ISO 3166-1 alpha-2 country code (e.g. `"BR"`).
    pub country: String,
    pub postal_code: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct GetnetAuthenticateRequest {
    pub transaction_id: String,
    pub xid: String,
}

#[derive(Debug, Serialize)]
pub struct GetnetPostAuthenticateRequest {
    /// CRES received at `term_url` after the ACS challenge completes.
    pub token: String,
}

/// Shared response schema across the three 3DS endpoints. Field semantics:
///   * `transaction_id` / `xid` — opaque correlators. `transaction_id` is
///     the long string set by `enrolments-initial`; `xid` is the short
///     Base64-ish token also set on initial. After `enrolments-continue`
///     the server may rotate `md` to a new opaque token but `transaction_id`
///     and `xid` remain stable.
///   * `protocol` — e.g. `"3DS2.3.1"` or `"3DS2.2.0"`. The `tdsver` field
///     on `/payments` wants the version *without* the `"3DS"` prefix
///     (see `protocol_to_msg_version`).
///   * `tds_method_content` — HTML fragment for the DDC iframe form
///     (empty when DDC is not required).
///   * `redirect_html_template` — auto-POSTing HTML form to the ACS for the
///     challenge (empty when frictionless).
///   * `acs_redirect_form` — structured representation of the same
///     auto-POST form. Preferred over `redirect_html_template` when both
///     are populated because it lets the caller render the form natively
///     instead of injecting HTML.
///   * `eci` / `cavv` / `ds_trans_id` — final authentication artifacts
///     produced on the frictionless `enrolments-continue` exit or after
///     `validations` for the challenge path. These flow back to
///     `data.payment.{xid,eci,ucaf,tdsdsxid,tdsver}` on `/payments`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetnetThreeDsResponse {
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub xid: Option<String>,
    #[serde(default)]
    pub tx_id: Option<i64>,
    #[serde(default)]
    pub md: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub status: Option<GetnetThreeDsStatus>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub tds_method_content: Option<String>,
    #[serde(default)]
    pub redirect_html_template: Option<String>,
    #[serde(default)]
    pub acs_redirect_form: Option<GetnetAcsRedirectForm>,
    #[serde(default)]
    pub eci: Option<String>,
    #[serde(default)]
    pub cavv: Option<Secret<String>>,
    #[serde(default)]
    pub ds_trans_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetnetAcsRedirectForm {
    pub action_url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub creq: Option<Secret<String>>,
    #[serde(rename = "threeDSSessionData", default)]
    pub three_ds_session_data: Option<Secret<String>>,
}

/// `StatusTransaction` enum from the Globalgetnet API. Sandbox lowercases
/// the values so all variants carry case-insensitive aliases. The
/// `Enrolment`/`Enrollment` doublet matches both UK and US spellings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum GetnetThreeDsStatus {
    #[serde(rename = "Authenticated", alias = "authenticated")]
    Authenticated,
    #[serde(rename = "Denied", alias = "denied")]
    Denied,
    #[serde(rename = "Pending", alias = "pending")]
    Pending,
    #[serde(rename = "Pending Challenge", alias = "pending challenge")]
    PendingChallenge,
    #[serde(
        rename = "Pending Enrolment Continue",
        alias = "pending enrolment continue",
        alias = "Pending Enrollment Continue",
        alias = "pending enrollment continue"
    )]
    PendingEnrolmentContinue,
    #[serde(other)]
    Unknown,
}

pub type GetnetPreAuthenticateResponse = GetnetThreeDsResponse;
pub type GetnetAuthenticateResponse = GetnetThreeDsResponse;
pub type GetnetPostAuthenticateResponse = GetnetThreeDsResponse;

/// Strip the leading `"3DS"` from a protocol string. `"3DS2.3.1"` → `"2.3.1"`.
/// Returns `None` if input is `None` or doesn't start with `"3DS"`.
fn protocol_to_msg_version(protocol: Option<&str>) -> Option<String> {
    protocol.and_then(|p| p.strip_prefix("3DS").map(|s| s.to_string()))
}

/// Build the address block required by `enrolments-initial.extra_fields.billing_address`.
/// Globalgetnet rejects 3DS enrolment (HTTP 500) when this object is absent, so the
/// billing address is mandatory: each missing field surfaces a `MissingRequiredField`
/// error naming the exact path rather than a placeholder the ACS would choke on.
fn build_threeds_address<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >,
) -> Result<GetnetThreeDsAddress, error_stack::Report<IntegrationError>> {
    // The 3DS enrolment billing address is mandatory; use the error-propagating getters.
    let street = item.resource_common_data.get_billing_line1()?;
    let number = item.resource_common_data.get_billing_line2()?;
    let country = item.resource_common_data.get_billing_country()?.to_string();
    let postal_code = item.resource_common_data.get_billing_zip()?;

    Ok(GetnetThreeDsAddress {
        street,
        number,
        country,
        postal_code,
    })
}

// ===== PreAuthenticate request: `POST /dpm/security-gwproxy/v2/enrolments-initial` =====

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GetnetPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;

        // TODO: Globalgetnet supports both "CREDIT" and "DEBIT" operations, but
        // for cards (the only PM supported on this leg today) the 3DS enrolment
        // ride uses "CREDIT" regardless of underlying card type. Revisit if/when
        // a debit-card 3DS scenario surfaces from QA.
        let operation = "CREDIT".to_string();

        let billing_address = build_threeds_address(item)?;

        Ok(Self {
            operation,
            currency: item.request.currency,
            amount: Some(item.request.amount),
            term_url: item
                .request
                .router_return_url
                .as_ref()
                .map(|u| u.to_string()),
            md: Some(
                item.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            description: None,
            extra_fields: GetnetThreeDsExtraFields {
                billing_address,
                shipping_address: None,
            },
        })
    }
}

// ===== Authenticate request: `POST /dpm/security-gwproxy/v2/enrolments-continue` =====
//
// Field-mapping convention used across this connector:
//   * `AuthenticationData.threeds_server_transaction_id` carries Globalgetnet's
//     `transaction_id` (the long opaque string from `enrolments-initial`).
//   * `AuthenticationData.transaction_id` carries Globalgetnet's `xid` (the
//     short Base64-ish token from `enrolments-initial`).
// The two flow inputs are echoed back here so the gateway can match the
// continue call to the original enrolment record.
impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GetnetAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        let auth_data = item.request.authentication_data.as_ref().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "request.authentication_data",
                context: IntegrationErrorContext {
                    suggested_action: Some("Run PreAuthenticate before Authenticate — the 3DS enrolment result must be threaded into this step.".to_string()),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;

        let transaction_id = auth_data.threeds_server_transaction_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.threeds_server_transaction_id",
                context: IntegrationErrorContext {
                    suggested_action: Some("PreAuthenticate must return a threeds_server_transaction_id before Authenticate can run.".to_string()),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;
        let xid = auth_data.transaction_id.clone().ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.transaction_id",
                context: IntegrationErrorContext {
                    suggested_action: Some("PreAuthenticate must return a transaction_id (xid) before Authenticate can run.".to_string()),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;

        Ok(Self {
            transaction_id,
            xid,
        })
    }
}

// ===== PostAuthenticate request: `POST /dpm/security-gwproxy/v2/validations` =====
//
// The CRES token is the Base64-encoded ACS response posted to the merchant's
// `term_url` after the customer completes the browser challenge. UCS forwards
// it via `request.redirect_response.payload` — a `SecretSerdeValue` carrying
// the form fields keyed by name (typically `cres` or `CRES`).
impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for GetnetPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // `get_redirect_response_payload()` already surfaces
        // `MissingRequiredField { field_name: "request.redirect_response.payload" }`,
        // so propagate it directly.
        let payload = wrapper
            .router_data
            .request
            .get_redirect_response_payload()?;
        let payload_json = payload.expose();
        // Accept either `cres` (canonical) or `CRES` (some browser-driven
        // posts uppercase form-field names).
        let token = payload_json
            .get("cres")
            .or_else(|| payload_json.get("CRES"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "request.redirect_response.payload.cres",
                context: IntegrationErrorContext {
                    suggested_action: Some("The posted-back challenge payload must carry a `cres` (or `CRES`) field.".to_string()),
                    doc_url: None,
                    additional_context: None,
                },
            })
        })?;

        Ok(Self { token })
    }
}

// ===== 3DS Response shared helpers =====

/// Build a `RedirectForm` from whichever 3DS-response field is populated.
/// Precedence: structured `acs_redirect_form` first (the most parser-friendly
/// shape and the one returned on challenge flows), then either of the HTML
/// fragments (`redirect_html_template` for the full ACS auto-POST page,
/// `tds_method_content` for the DDC iframe). Returns `None` when none of
/// these are present (frictionless / no DDC).
fn build_threeds_redirection(response: &GetnetThreeDsResponse) -> Option<RedirectForm> {
    if let Some(form) = response.acs_redirect_form.as_ref() {
        let method = match form.method.as_deref() {
            Some(m) if m.eq_ignore_ascii_case("GET") => Method::Get,
            _ => Method::Post,
        };
        let mut form_fields = std::collections::HashMap::new();
        if let Some(creq) = form.creq.as_ref() {
            form_fields.insert("creq".to_string(), creq.peek().clone());
        }
        if let Some(sd) = form.three_ds_session_data.as_ref() {
            form_fields.insert("threeDSSessionData".to_string(), sd.peek().clone());
        }
        return Some(RedirectForm::Form {
            endpoint: form.action_url.clone(),
            method,
            form_fields,
        });
    }
    if let Some(html) = response
        .redirect_html_template
        .as_ref()
        .filter(|s| !s.is_empty())
    {
        return Some(RedirectForm::Html {
            html_data: html.clone(),
        });
    }
    if let Some(html) = response
        .tds_method_content
        .as_ref()
        .filter(|s| !s.is_empty())
    {
        return Some(RedirectForm::Html {
            html_data: html.clone(),
        });
    }
    None
}

/// Build the `AuthenticationData` payload that travels between flows.
/// Always populated with whichever fields the response carried.
fn build_threeds_authentication_data(response: &GetnetThreeDsResponse) -> AuthenticationData {
    let message_version = protocol_to_msg_version(response.protocol.as_deref())
        .and_then(|v| common_utils::types::SemanticVersion::from_str(&v).ok());
    AuthenticationData {
        trans_status: None,
        eci: response.eci.clone(),
        cavv: response.cavv.clone(),
        ucaf_collection_indicator: None,
        // Map Globalgetnet `transaction_id` -> AuthenticationData.threeds_server_transaction_id.
        threeds_server_transaction_id: response.transaction_id.clone(),
        message_version,
        ds_trans_id: response.ds_trans_id.clone(),
        acs_transaction_id: None,
        // Map Globalgetnet `xid` -> AuthenticationData.transaction_id.
        transaction_id: response.xid.clone(),
        network_params: None,
        exemption_indicator: None,
    }
}

/// Map the three-state Globalgetnet status to a UCS `AttemptStatus`.
fn threeds_status_to_attempt(status: Option<&GetnetThreeDsStatus>) -> AttemptStatus {
    match status {
        Some(GetnetThreeDsStatus::Authenticated) => AttemptStatus::AuthenticationSuccessful,
        Some(GetnetThreeDsStatus::Denied) => AttemptStatus::AuthenticationFailed,
        Some(GetnetThreeDsStatus::PendingChallenge)
        | Some(GetnetThreeDsStatus::PendingEnrolmentContinue)
        | Some(GetnetThreeDsStatus::Pending) => AttemptStatus::AuthenticationPending,
        Some(GetnetThreeDsStatus::Unknown) | None => AttemptStatus::AuthenticationPending,
    }
}

// ===== PreAuthenticate response → RouterDataV2 =====

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetPreAuthenticateResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetPreAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let redirection_data = build_threeds_redirection(&item.response).map(Box::new);
        let authentication_data = Some(build_threeds_authentication_data(&item.response));
        let status = threeds_status_to_attempt(item.response.status.as_ref());

        Ok(Self {
            response: Ok(PaymentsResponseData::PreAuthenticateResponse {
                authentication_data,
                redirection_data,
                connector_response_reference_id: item.response.transaction_id.clone(),
                status_code: item.http_code,
                resource_id: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// ===== Authenticate response → RouterDataV2 =====

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = threeds_status_to_attempt(item.response.status.as_ref());

        // Per the API spec:
        //   * `Authenticated` (frictionless) -> redirection_data: None,
        //     authentication_data carries the final eci/cavv/ds_trans_id/xid.
        //   * `Pending Challenge` -> redirection_data carries the ACS auto-POST
        //     form (preferred via `acs_redirect_form`, fall back to HTML).
        //     authentication_data is a partial passthrough so the next leg
        //     (PostAuthenticate) can re-correlate the session.
        //   * `Denied` -> neither.
        let (redirection_data, authentication_data) = match item.response.status {
            Some(GetnetThreeDsStatus::Authenticated) => (
                None,
                Some(build_threeds_authentication_data(&item.response)),
            ),
            Some(GetnetThreeDsStatus::PendingChallenge) => (
                build_threeds_redirection(&item.response).map(Box::new),
                Some(build_threeds_authentication_data(&item.response)),
            ),
            Some(GetnetThreeDsStatus::Denied) => (None, None),
            _ => (
                build_threeds_redirection(&item.response).map(Box::new),
                Some(build_threeds_authentication_data(&item.response)),
            ),
        };

        let resource_id = item
            .response
            .transaction_id
            .clone()
            .map(ResponseId::ConnectorTransactionId);

        Ok(Self {
            response: Ok(PaymentsResponseData::AuthenticateResponse {
                resource_id,
                redirection_data,
                authentication_data,
                connector_response_reference_id: item.response.transaction_id.clone(),
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

// ===== PostAuthenticate response → RouterDataV2 =====

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetPostAuthenticateResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetPostAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = threeds_status_to_attempt(item.response.status.as_ref());
        let authentication_data = match item.response.status {
            Some(GetnetThreeDsStatus::Denied) => None,
            _ => Some(build_threeds_authentication_data(&item.response)),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                authentication_data,
                connector_response_reference_id: item.response.transaction_id.clone(),
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

// =====================================================================
// PAYMENT METHOD TOKEN (Cofre tokenization)
// =====================================================================
//
// Globalgetnet's Cofre service exchanges a raw PAN for an opaque,
// reusable token usable on subsequent `/payments` and `/security-gwproxy`
// calls. Endpoint: `POST /dpm/cofre-gw-proxy/v1/tokens/card`.
//
// The wire request is intentionally tiny — *only* the PAN — and the
// response is `{ "number_token": "<128 hex chars>" }`. The token then
// rides back to the merchant via `PaymentFlowData.session_token`, which
// the Authorize TryFrom reads to switch the `data.payment.card` block
// from `number` to `number_token` (see `GetnetCard<T>`).

#[derive(Debug, Serialize)]
pub struct GetnetTokenizeRequest<T: PaymentMethodDataTypes> {
    /// Raw PAN. Serializes with the same convention as every other
    /// `card_number` field in this transformer (`RawCardNumber<T>`).
    pub card_number: RawCardNumber<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetnetTokenizeResponse {
    /// 128-char hex token returned by Cofre. Reused by Authorize as
    /// `data.payment.card.number_token`.
    pub number_token: Secret<String>,
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        GetnetRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for GetnetTokenizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: GetnetRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let item = &wrapper.router_data;
        match &item.request.payment_method_data {
            PaymentMethodData::Card(card_data) => Ok(Self {
                card_number: card_data.card_number.clone(),
            }),
            _ => Err(IntegrationError::NotSupported {
                message: "Only card tokenization supported".to_string(),
                connector: "Getnet",
                context: Default::default(),
            }
            .into()),
        }
    }
}

impl<T: PaymentMethodDataTypes + fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<GetnetTokenizeResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<GetnetTokenizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // PaymentMethodToken doesn't drive an AttemptStatus transition;
        // leave `resource_common_data.status` untouched.
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.number_token.expose(),
            }),
            ..item.router_data
        })
    }
}
