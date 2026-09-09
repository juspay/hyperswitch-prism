use std::collections::HashMap;

use common_enums::{CountryAlpha2, Currency};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii,
    request::Method,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{AuthoriseIntegrityObject, RefundIntegrityObject},
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::xendit::{XenditAmountConvertor, XenditRouterData},
    types::ResponseRouterData,
    utils::get_unimplemented_payment_method_error_message,
};

/// Surfaced as `IntegrationErrorContext::doc_url` on every locally-raised, user-visible error.
const XENDIT_PAYMENT_REQUEST_DOC_URL: &str =
    "https://docs.xendit.co/apidocs/create-payment-request";

// -------------------------------------------------------------------------------------------
// Payments API v3 request types
//
// Wire reference: `POST /v3/payment_requests`, header `api-version: 2024-11-11`.
// https://docs.xendit.co/apidocs/create-payment-request
// -------------------------------------------------------------------------------------------

/// `channel_code` on the payment request. Only `CARDS` is implemented by this connector; the
/// non-card channels are rejected before a request is built (see the `TryFrom` below).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditChannelCode {
    Cards,
}

/// Top-level `type`. `PAY_AND_SAVE` additionally stores the instrument as a payment token; it is
/// selected for mandate-shaped requests, mirroring the `reusability` flag this connector used on
/// the v2 wire shape.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditRequestType {
    Pay,
    PayAndSave,
}

/// Top-level `capture_method`. `AUTOMATIC` is Xendit's default.
/// https://docs.xendit.co/docs/cards-capturing-a-card-payment
///
/// Request-only: it is never deserialized, so it deliberately carries no `Unknown` variant that
/// could be serialized back onto the wire.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditCaptureMethod {
    Automatic,
    Manual,
}

/// `channel_properties.card_details` — the full-PAN card container on the v3 wire shape.
/// Every member other than the PAN and expiry is optional per the API reference.
#[derive(Serialize, Debug)]
pub struct XenditCardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: RawCardNumber<T>,
    /// `MM`
    pub expiry_month: Secret<String>,
    /// `YYYY`
    pub expiry_year: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvn: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardholder_phone_number: Option<Secret<String>>,
}

/// `channel_properties.billing_information` — Xendit's only address object, and the input to its
/// Address Verification Service. Field spelling is taken verbatim from the response echo on
/// https://docs.xendit.co/docs/pay-with-authentication ; AVS is documented as meaningfully
/// evaluated only for US/CA/UK issuers.
/// https://docs.xendit.co/docs/cards-address-verification-service
#[derive(Serialize, Debug, Default, PartialEq, Eq)]
pub struct XenditBillingInformation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_line1: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_line2: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province_state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<CountryAlpha2>,
}

impl XenditBillingInformation {
    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// `channel_properties` for `channel_code = CARDS`.
///
/// Xendit types `channel_properties` as a free-form object in its OpenAPI schema, so only the
/// members evidenced by a worked request/response example are modelled here. Notably absent —
/// and deliberately so — are any shipping object and any Level 2 / Level 3 construct: neither
/// exists anywhere in the Xendit documentation set.
#[derive(Serialize, Debug)]
pub struct XenditChannelProperties<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_details: XenditCardDetails<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_information: Option<XenditBillingInformation>,
    pub success_return_url: String,
    pub failure_return_url: String,
    /// Requires the account to be enabled for non-3DS card requests; otherwise Xendit answers
    /// HTTP 403 `SKIP_3DS_FORBIDDEN`.
    pub skip_three_ds: bool,
    /// Soft / dynamic descriptor. Xendit publishes no length or charset constraint for it, so the
    /// merchant-supplied value is passed through unmodified.
    /// https://docs.xendit.co/docs/pay-with-authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
}

/// `POST /v3/payment_requests` body.
///
/// `country` is intentionally not sent: it is optional on the v3 schema, it is constrained to
/// Xendit's eight operating markets, and the technical specification leaves the question of which
/// domain field should supply it explicitly undecided. Guessing a source here would shape the
/// payment on an unverified assumption.
#[derive(Serialize, Debug)]
pub struct XenditPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    /// Merchant reference id. Xendit requires it to be unique for the CARDS channel, so it is
    /// derived from the caller's stable request id rather than minted per attempt.
    pub reference_id: Secret<String>,
    #[serde(rename = "type")]
    pub request_type: XenditRequestType,
    pub currency: Currency,
    pub request_amount: FloatMajorUnit,
    pub capture_method: XenditCaptureMethod,
    pub channel_code: XenditChannelCode,
    pub channel_properties: XenditChannelProperties<T>,
}

// -------------------------------------------------------------------------------------------
// Payments API v3 response types
// -------------------------------------------------------------------------------------------

/// `payment_request.status`.
/// https://docs.xendit.co/apidocs/get-payment-request
#[derive(Debug, Clone, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditPaymentRequestStatus {
    AcceptingPayments,
    RequiresAction,
    Authorized,
    Canceled,
    Expired,
    Succeeded,
    Failed,
    /// Keeps the raw value so an unmodelled status neither fails the body nor is silently
    /// collapsed into a modelled one.
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

/// `payment.status` / `latest_payment.status` — a different, payment-level enum from the
/// payment-request status above.
/// https://docs.xendit.co/apidocs/get-payment
#[derive(Debug, Clone, Deserialize, Serialize, strum::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditPaymentStatus {
    Pending,
    Authorized,
    Succeeded,
    Failed,
    Canceled,
    Expired,
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

/// `actions[].type`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditActionType {
    PresentToCustomer,
    RedirectCustomer,
    ApiPostRequest,
    #[serde(untagged)]
    Unknown(String),
}

/// `actions[].descriptor`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditActionDescriptor {
    CapturePayment,
    PaymentCode,
    QrString,
    VirtualAccountNumber,
    WebUrl,
    DeeplinkUrl,
    ValidateOtp,
    ResendOtp,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditAction {
    #[serde(rename = "type")]
    pub action_type: XenditActionType,
    pub descriptor: XenditActionDescriptor,
    pub value: String,
}

/// AVS / CVN outcome. Every published Xendit schema exposes exactly two values, `M` and `N`;
/// the prose "partial match" and "unavailable" categories have no wire representation, so an
/// absent field is the only way "unavailable" reaches us.
/// https://docs.xendit.co/docs/cards-address-verification-service
#[derive(Debug, Clone, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
pub enum XenditVerificationResult {
    #[serde(rename = "M")]
    #[strum(to_string = "M")]
    Match,
    #[serde(rename = "N")]
    #[strum(to_string = "N")]
    NoMatch,
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

/// `payment_details.authorization_data`.
///
/// Only the members this connector actually reads are modelled. There is deliberately no
/// merchant-advice-code member: Xendit publishes the Mastercard advice-code *value table* but
/// exposes no API field carrying it, on this object or anywhere else.
/// https://docs.xendit.co/docs/retrieving-authorization-and-decline-details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditAuthorizationData {
    #[serde(default)]
    pub authorization_code: Option<String>,
    #[serde(default)]
    pub address_verification_result: Option<XenditVerificationResult>,
    #[serde(default)]
    pub cvn_verification_result: Option<XenditVerificationResult>,
    /// Raw scheme response code, e.g. `"00"` for approval.
    #[serde(default)]
    pub network_response_code: Option<String>,
    #[serde(default)]
    pub network_response_code_descriptor: Option<String>,
    #[serde(default)]
    pub network_transaction_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPaymentDetails {
    #[serde(default)]
    pub authorization_data: Option<XenditAuthorizationData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditCaptureDetail {
    pub capture_id: String,
    pub capture_amount: FloatMajorUnit,
}

/// `latest_payment` — the embedded payment object on a payment-request response. It is absent
/// while the payment request is still `REQUIRES_ACTION`, which is why the authorization detail
/// below is always optional on a 3DS authorize.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditLatestPayment {
    #[serde(default)]
    pub failure_code: Option<XenditFailureCode>,
    #[serde(default)]
    pub payment_token_id: Option<Secret<String>>,
    #[serde(default)]
    pub payment_details: Option<XenditPaymentDetails>,
}

/// `POST /v3/payment_requests` (201) and `GET /v3/payment_requests/{id}` response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPaymentResponse {
    pub payment_request_id: String,
    pub reference_id: Secret<String>,
    pub status: XenditPaymentRequestStatus,
    pub currency: Currency,
    pub request_amount: FloatMajorUnit,
    #[serde(default)]
    pub failure_code: Option<XenditFailureCode>,
    #[serde(default)]
    pub actions: Option<Vec<XenditAction>>,
    /// The payment id (`py-…`). v3 captures address the payment, not the payment request.
    #[serde(default)]
    pub latest_payment_id: Option<String>,
    #[serde(default)]
    pub latest_payment: Option<XenditLatestPayment>,
}

/// `POST /v3/payments/{payment_id}/capture` response — the full payment schema.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditCaptureResponse {
    pub payment_id: String,
    pub status: XenditPaymentStatus,
    pub currency: Currency,
    #[serde(default)]
    pub payment_request_id: Option<String>,
    #[serde(default)]
    pub reference_id: Option<Secret<String>>,
    #[serde(default)]
    pub request_amount: Option<FloatMajorUnit>,
    #[serde(default)]
    pub failure_code: Option<XenditFailureCode>,
    #[serde(default)]
    pub captures: Option<Vec<XenditCaptureDetail>>,
    #[serde(default)]
    pub payment_details: Option<XenditPaymentDetails>,
}

/// `POST /v3/payments/{payment_id}/capture` body. `capture_amount` may be omitted to capture the
/// full authorization; UCS always knows the amount it wants, so it is always sent.
#[derive(Serialize, Deserialize, Debug)]
pub struct XenditPaymentsCaptureRequest {
    pub capture_amount: FloatMajorUnit,
}

/// Persisted by Authorize/PSync into `connector_metadata` and read back by Capture.
///
/// v3 capture addresses the payment id (`py-…`) while every other Xendit flow in this connector
/// (PSync, Refund) addresses the payment-request id (`pr-…`), which is what Authorize reports as
/// its `resource_id`. Re-deriving the payment id at capture time is impossible, so it travels on
/// `connector_metadata` and comes back on `PaymentServiceCaptureRequest.connector_feature_data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XenditConnectorMetadata {
    pub latest_payment_id: Option<String>,
}

/// Transaction-level `failure_code`.
///
/// The 33 values published on https://docs.xendit.co/apidocs/get-payment-request , plus the six
/// further values that Xendit's card-declines page maps network response codes onto but which are
/// absent from the OpenAPI enum (`PARTIAL_APPROVAL`, `SUCCESS`, `INVALID_CARD`,
/// `ISSUER_SUSPECT_FRAUD`, `TRANSACTION_BLOCKED`, and the page's `PROCESS_ERROR` spelling of
/// `PROCESSOR_ERROR`). The mapping has to be total over both sets.
/// https://docs.xendit.co/docs/cards-card-declines-and-error-codes
#[derive(Debug, Clone, Deserialize, Serialize, strum::Display, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditFailureCode {
    AccountAccessBlocked,
    InvalidMerchantSettings,
    InvalidAccountDetails,
    PaymentAttemptCountsExceeded,
    UserDeviceUnreachable,
    ChannelUnavailable,
    InsufficientBalance,
    AccountNotActivated,
    InvalidToken,
    ServerError,
    PartnerTimeoutError,
    TimeoutError,
    UserDeclinedPayment,
    UserDidNotAuthorize,
    PaymentRequestExpired,
    FailureDetailsUnavailable,
    ExpiredOtp,
    InvalidOtp,
    PaymentAmountLimitsExceeded,
    OtpAttemptCountsExceeded,
    CardDeclined,
    DeclinedByIssuer,
    IssuerUnavailable,
    InvalidCvv,
    DeclinedByProcessor,
    CaptureAmountExceeded,
    AuthenticationFailed,
    ProcessorError,
    ExpiredCard,
    StolenCard,
    InactiveOrUnauthorizedCard,
    InvalidMerchantCredentials,
    SuspectedFraudulent,
    // Values used by the card-declines mapping table but absent from the OpenAPI enum.
    PartialApproval,
    Success,
    InvalidCard,
    IssuerSuspectFraud,
    TransactionBlocked,
    ProcessError,
    #[serde(untagged)]
    #[strum(default)]
    #[strum(to_string = "{0}")]
    Unknown(String),
}

/// Who declined, for phrasing the merchant-facing message. Xendit publishes no per-`failure_code`
/// description, so this only separates an issuer/card decline from a merchant-configuration
/// problem, a transient outage and an abandoned customer action — enough that a 403 credential
/// rejection or a channel outage does not surface as "card declined". The code itself always
/// travels verbatim in `ErrorResponse::code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XenditFailureCategory {
    IssuerDecline,
    MerchantConfiguration,
    TemporarilyUnavailable,
    CustomerAction,
    Unknown,
}

impl XenditFailureCode {
    /// Every variant is matched explicitly: a catch-all here would silently file a newly
    /// documented failure code under whichever category happened to be first.
    pub fn category(&self) -> XenditFailureCategory {
        match self {
            Self::CardDeclined
            | Self::DeclinedByIssuer
            | Self::DeclinedByProcessor
            | Self::InvalidCvv
            | Self::ExpiredCard
            | Self::StolenCard
            | Self::InactiveOrUnauthorizedCard
            | Self::SuspectedFraudulent
            | Self::IssuerSuspectFraud
            | Self::TransactionBlocked
            | Self::InsufficientBalance
            | Self::PaymentAmountLimitsExceeded
            | Self::InvalidAccountDetails
            | Self::InvalidCard
            | Self::AccountNotActivated
            | Self::AccountAccessBlocked
            | Self::PartialApproval => XenditFailureCategory::IssuerDecline,
            Self::InvalidMerchantSettings
            | Self::InvalidMerchantCredentials
            | Self::InvalidToken
            | Self::CaptureAmountExceeded => XenditFailureCategory::MerchantConfiguration,
            Self::ChannelUnavailable
            | Self::IssuerUnavailable
            | Self::ServerError
            | Self::ProcessorError
            | Self::ProcessError
            | Self::PartnerTimeoutError
            | Self::TimeoutError
            | Self::UserDeviceUnreachable => XenditFailureCategory::TemporarilyUnavailable,
            Self::UserDeclinedPayment
            | Self::UserDidNotAuthorize
            | Self::AuthenticationFailed
            | Self::ExpiredOtp
            | Self::InvalidOtp
            | Self::OtpAttemptCountsExceeded
            | Self::PaymentAttemptCountsExceeded
            | Self::PaymentRequestExpired => XenditFailureCategory::CustomerAction,
            Self::FailureDetailsUnavailable | Self::Success | Self::Unknown(_) => {
                XenditFailureCategory::Unknown
            }
        }
    }

    fn merchant_message(&self) -> &'static str {
        match self.category() {
            XenditFailureCategory::IssuerDecline => {
                "The card issuer declined the transaction at Xendit"
            }
            XenditFailureCategory::MerchantConfiguration => {
                "Xendit rejected the transaction because of the merchant account configuration"
            }
            XenditFailureCategory::TemporarilyUnavailable => {
                "Xendit or the issuer was temporarily unable to process the transaction"
            }
            XenditFailureCategory::CustomerAction => {
                "The customer did not complete the payment at Xendit"
            }
            XenditFailureCategory::Unknown => "Xendit reported the transaction as failed",
        }
    }
}

pub struct XenditAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for XenditAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Xendit { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum XenditResponse {
    /// Boxed: the payment-request payload is an order of magnitude larger than the webhook one,
    /// and this enum is cloned on every sync.
    Payment(Box<XenditPaymentResponse>),
    Webhook(XenditWebhookEvent),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct XenditWebhookEvent {
    pub event: XenditEventType,
    pub data: EventDetails,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum XenditEventType {
    #[serde(rename = "payment.succeeded")]
    PaymentSucceeded,
    #[serde(rename = "payment.awaiting_capture")]
    PaymentAwaitingCapture,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "capture.succeeded")]
    CaptureSucceeded,
    #[serde(rename = "capture.failed")]
    CaptureFailed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventDetails {
    pub id: String,
    pub payment_request_id: Option<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
}

/// HTTP-level (4xx/5xx) error body. Distinct from the transaction-level `failure_code` carried
/// inside a 2xx body. https://docs.xendit.co/apidocs/create-payment-request
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct XenditErrorResponse {
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

// -------------------------------------------------------------------------------------------
// Request-building helpers
// -------------------------------------------------------------------------------------------

fn missing_field(field_name: &'static str, suggested_action: &'static str) -> IntegrationError {
    IntegrationError::MissingRequiredField {
        field_name,
        context: IntegrationErrorContext {
            suggested_action: Some(suggested_action.to_owned()),
            doc_url: Some(XENDIT_PAYMENT_REQUEST_DOC_URL.to_owned()),
            additional_context: None,
        },
    }
}

/// Maps the requested capture mode onto Xendit's two-valued `capture_method`.
///
/// `crate::utils::is_manual_capture` folds `ManualMultiple` in with `Manual`; Xendit documents
/// that "multiple partial captures are not supported", so accepting `ManualMultiple` would
/// promise a capture mode Xendit cannot honour. Every other `CaptureMethod` is therefore rejected
/// locally instead of being sent.
/// https://docs.xendit.co/docs/cards-capturing-a-card-payment
fn get_capture_method<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    data: &PaymentsAuthorizeData<T>,
) -> Result<XenditCaptureMethod, error_stack::Report<IntegrationError>> {
    match data.capture_method {
        Some(common_enums::CaptureMethod::Automatic) | None => Ok(XenditCaptureMethod::Automatic),
        Some(common_enums::CaptureMethod::Manual) => Ok(XenditCaptureMethod::Manual),
        Some(_) => Err(IntegrationError::CaptureMethodNotSupported {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Use AUTOMATIC or MANUAL capture; Xendit does not support multiple partial captures."
                        .to_owned(),
                ),
                doc_url: Some(
                    "https://docs.xendit.co/docs/cards-capturing-a-card-payment".to_owned(),
                ),
                additional_context: Some(format!(
                    "unsupported capture_method: {:?}",
                    data.capture_method
                )),
            },
        }
        .into()),
    }
}

/// Splits the cardholder name into Xendit's `cardholder_first_name` / `cardholder_last_name`.
///
/// The last whitespace-separated token becomes the last name, mirroring
/// `checkout::split_account_holder_name`. A single-token name yields a first name only; both
/// fields are optional on the Xendit wire, so an absent cardholder name yields neither. The
/// billing name is deliberately not used as a substitute — it can legitimately differ from the
/// name embossed on the card.
fn split_cardholder_name(
    card_holder_name: Option<Secret<String>>,
) -> (Option<Secret<String>>, Option<Secret<String>>) {
    let name = card_holder_name.map(|name| name.expose().trim().to_string());
    match name {
        Some(name) if !name.is_empty() => match name.rsplit_once(' ') {
            Some((first, last)) => (
                Some(Secret::new(first.trim().to_string())),
                Some(Secret::new(last.to_string())),
            ),
            None => (Some(Secret::new(name)), None),
        },
        _ => (None, None),
    }
}

/// Builds `channel_properties.billing_information` from the billing address on the router data.
/// Returns `None` when the router data carries no billing address at all, so the object is
/// omitted rather than sent empty.
fn build_billing_information(
    resource_common_data: &PaymentFlowData,
) -> Option<XenditBillingInformation> {
    let billing_information = XenditBillingInformation {
        street_line1: resource_common_data.get_optional_billing_line1(),
        street_line2: resource_common_data.get_optional_billing_line2(),
        city: resource_common_data.get_optional_billing_city(),
        province_state: resource_common_data.get_optional_billing_state(),
        postal_code: resource_common_data.get_optional_billing_zip(),
        country: resource_common_data.get_optional_billing_country(),
    };

    (!billing_information.is_empty()).then_some(billing_information)
}

/// Rejects an Authorize that carries externally generated 3DS data.
///
/// No Xendit endpoint accepts a CAVV, ECI, XID or DS transaction id — its 3DS2 is performed by
/// Xendit's own 3DS server and the authentication fields exist on responses only. Silently
/// dropping merchant-supplied authentication would downgrade an authenticated transaction to an
/// unauthenticated one and move liability, so it fails here instead.
/// https://docs.xendit.co/docs/cards-authentication-3ds2
fn reject_external_authentication_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    request: &PaymentsAuthorizeData<T>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let carries_authentication = request.authentication_data.as_ref().is_some_and(|data| {
        data.cavv.is_some()
            || data.eci.is_some()
            || data.ds_trans_id.is_some()
            || data.threeds_server_transaction_id.is_some()
    });

    if carries_authentication {
        return Err(IntegrationError::NotSupported {
            message: "External 3DS authentication data".to_owned(),
            connector: "xendit",
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Route externally authenticated payments to a connector that accepts 3DS passthrough, or let Xendit perform the authentication."
                        .to_owned(),
                ),
                doc_url: Some("https://docs.xendit.co/docs/cards-authentication-3ds2".to_owned()),
                additional_context: Some(
                    "Xendit accepts no CAVV/ECI/XID/ds_trans_id on any request field".to_owned(),
                ),
            },
        }
        .into());
    }
    Ok(())
}

/// Reads the payment id (`py-…`) that the v3 capture endpoint addresses.
///
/// Preference order: the `latest_payment_id` Authorize persisted into `connector_metadata` and
/// the caller echoed back on `connector_feature_data`; then the connector transaction id itself,
/// but only when it is already a payment id, since Authorize normally reports the payment-request
/// id (`pr-…`) there and the capture endpoint would 404 on it.
pub fn get_capture_payment_id(
    request: &PaymentsCaptureData,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let from_metadata = request
        .connector_feature_data
        .clone()
        .and_then(|metadata| {
            serde_json::from_value::<XenditConnectorMetadata>(metadata.expose()).ok()
        })
        .and_then(|metadata| metadata.latest_payment_id);

    if let Some(payment_id) = from_metadata {
        return Ok(payment_id);
    }

    let connector_transaction_id = request.get_connector_transaction_id()?;
    if connector_transaction_id.starts_with("py-") {
        return Ok(connector_transaction_id);
    }

    Err(missing_field(
        "connector_feature_data.latest_payment_id",
        "Echo the connector_metadata returned by Authorize back on the capture request: Xendit's v3 capture endpoint addresses the payment id (py-...), not the payment request id (pr-...).",
    )
    .into())
}

// -------------------------------------------------------------------------------------------
// Response-mapping helpers
// -------------------------------------------------------------------------------------------

impl From<&XenditPaymentRequestStatus> for common_enums::AttemptStatus {
    fn from(status: &XenditPaymentRequestStatus) -> Self {
        match status {
            // A reusable payment code is live but nothing has been paid against it yet.
            XenditPaymentRequestStatus::AcceptingPayments => Self::Pending,
            XenditPaymentRequestStatus::RequiresAction => Self::AuthenticationPending,
            XenditPaymentRequestStatus::Authorized => Self::Authorized,
            XenditPaymentRequestStatus::Succeeded => Self::Charged,
            XenditPaymentRequestStatus::Canceled => Self::Voided,
            XenditPaymentRequestStatus::Failed | XenditPaymentRequestStatus::Expired => {
                Self::Failure
            }
            // Leave the caller holding whatever status it already had rather than inventing one.
            XenditPaymentRequestStatus::Unknown(_) => Self::Unspecified,
        }
    }
}

impl From<&XenditPaymentStatus> for common_enums::AttemptStatus {
    fn from(status: &XenditPaymentStatus) -> Self {
        match status {
            XenditPaymentStatus::Pending => Self::Pending,
            XenditPaymentStatus::Authorized => Self::Authorized,
            XenditPaymentStatus::Succeeded => Self::Charged,
            XenditPaymentStatus::Canceled => Self::Voided,
            XenditPaymentStatus::Failed | XenditPaymentStatus::Expired => Self::Failure,
            XenditPaymentStatus::Unknown(_) => Self::Unspecified,
        }
    }
}

/// Picks the browser redirect out of `actions[]`.
///
/// For a card 3DS authorization Xendit emits a single actionable entry, `REDIRECT_CUSTOMER` /
/// `WEB_URL`. The other descriptors (QR strings, virtual account numbers, OTP prompts) belong to
/// non-card channels; turning one of those into a redirect would send the customer to a value
/// that is not a URL, so they are ignored. The value is used whole — its query string carries the
/// public key Xendit's authentication page needs.
fn get_redirection_data(actions: Option<&Vec<XenditAction>>) -> Option<Box<RedirectForm>> {
    actions?
        .iter()
        .find(|action| {
            action.action_type == XenditActionType::RedirectCustomer
                && action.descriptor == XenditActionDescriptor::WebUrl
        })
        .map(|action| {
            Box::new(RedirectForm::Form {
                endpoint: action.value.clone(),
                method: Method::Get,
                form_fields: HashMap::new(),
            })
        })
}

/// Surfaces Xendit's AVS and CVN outcomes, the raw scheme response code and the scheme
/// authorization code on the payment's connector response.
///
/// Returns `None` when Xendit sent no `authorization_data` at all — which is the normal case for
/// a 3DS authorize that comes back `REQUIRES_ACTION`, since no payment exists yet. The values
/// then arrive on the subsequent PSync.
fn build_connector_response(
    authorization_data: Option<&XenditAuthorizationData>,
) -> Option<ConnectorResponseData> {
    authorization_data.map(|data| {
        let payment_checks = serde_json::json!({
            "avs_result": data.address_verification_result.as_ref().map(ToString::to_string),
            "card_validation_result": data.cvn_verification_result.as_ref().map(ToString::to_string),
            "network_response_code": data.network_response_code,
            "network_response_code_descriptor": data.network_response_code_descriptor,
        });

        ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: Some(payment_checks),
                card_network: None,
                domestic_network: None,
                auth_code: data.authorization_code.clone(),
            },
        )
    })
}

/// Stores the payment id so Capture can address the v3 capture endpoint. Returns `None` before a
/// payment exists (3DS `REQUIRES_ACTION`), in which case PSync supplies it later.
fn build_connector_metadata(latest_payment_id: Option<&String>) -> Option<serde_json::Value> {
    latest_payment_id.map(|payment_id| {
        serde_json::json!(XenditConnectorMetadata {
            latest_payment_id: Some(payment_id.clone()),
        })
    })
}

/// Builds the `ErrorResponse` for a 2xx body that reports a failed transaction.
///
/// The code is Xendit's own `failure_code`, verbatim. When Xendit also surfaced the raw scheme
/// response, its code and descriptor travel on the dedicated network fields and the descriptor —
/// Xendit's own wording — becomes the message. `network_advice_code` is always `None`: Xendit
/// documents the Mastercard merchant-advice-code value table but exposes no API field carrying
/// one, so there is nothing to read.
fn build_failure_error_response(
    failure_code: Option<&XenditFailureCode>,
    authorization_data: Option<&XenditAuthorizationData>,
    connector_transaction_id: Option<String>,
    attempt_status: common_enums::AttemptStatus,
    status_code: u16,
) -> ErrorResponse {
    let network_response_code =
        authorization_data.and_then(|data| data.network_response_code.clone());
    let network_response_descriptor =
        authorization_data.and_then(|data| data.network_response_code_descriptor.clone());

    let code = failure_code
        .map(ToString::to_string)
        .unwrap_or_else(|| NO_ERROR_CODE.to_string());

    let message = network_response_descriptor
        .clone()
        .or_else(|| failure_code.map(|failure_code| failure_code.merchant_message().to_owned()))
        .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

    let reason = match (&network_response_code, &network_response_descriptor) {
        (Some(network_code), Some(descriptor)) => Some(format!(
            "{code} (network response code {network_code}: {descriptor})"
        )),
        (Some(network_code), None) => {
            Some(format!("{code} (network response code {network_code})"))
        }
        _ => {
            failure_code.map(|failure_code| format!("{code}: {}", failure_code.merchant_message()))
        }
    };

    ErrorResponse {
        code,
        message,
        reason,
        attempt_status: Some(FlowStatus::Payment(attempt_status)),
        connector_transaction_id,
        status_code,
        network_advice_code: None,
        network_decline_code: network_response_code,
        network_error_message: network_response_descriptor,
        ..Default::default()
    }
}

// -------------------------------------------------------------------------------------------
// Authorize
// -------------------------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XenditPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let request = &item.router_data.request;
        let resource_common_data = &item.router_data.resource_common_data;

        reject_external_authentication_data(request)?;

        let card_data = match &request.payment_method_data {
            PaymentMethodData::Card(card_data) => card_data.clone(),
            _ => {
                return Err(IntegrationError::NotImplemented(
                    get_unimplemented_payment_method_error_message("xendit"),
                    Default::default(),
                )
                .into())
            }
        };

        let request_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })
            .attach_printable("Failed to convert amount to required type")?;

        // Xendit requires `reference_id` to be unique for the CARDS channel, so it doubles as the
        // duplicate guard. `merchant_request_id` is the caller's stable per-request id and stays
        // constant across retries; `connector_request_reference_id` is the fallback when the
        // caller did not supply one.
        let reference_id = resource_common_data
            .get_merchant_request_id()
            .unwrap_or_else(|_| resource_common_data.connector_request_reference_id.clone());

        let return_url = request.get_router_return_url().change_context(missing_field(
            "router_return_url",
            "Xendit performs 3DS in a browser redirect and requires both a success and a failure return URL.",
        ))?;

        let (cardholder_first_name, cardholder_last_name) =
            split_cardholder_name(card_data.get_optional_cardholder_name());

        let card_details = XenditCardDetails {
            card_number: card_data.card_number.clone(),
            expiry_month: card_data.get_card_expiry_month_2_digit()?,
            expiry_year: card_data.get_expiry_year_4_digit(),
            cvn: (!card_data.card_cvc.peek().is_empty()).then(|| card_data.card_cvc.clone()),
            cardholder_first_name,
            cardholder_last_name,
            cardholder_email: resource_common_data
                .get_optional_billing_email()
                .or_else(|| request.get_optional_email()),
            cardholder_phone_number: resource_common_data.get_optional_billing_phone_number(),
        };

        Ok(Self {
            reference_id: Secret::new(reference_id),
            request_type: if request.is_mandate_payment() {
                XenditRequestType::PayAndSave
            } else {
                XenditRequestType::Pay
            },
            currency: request.currency,
            request_amount,
            capture_method: get_capture_method(request)?,
            channel_code: XenditChannelCode::Cards,
            channel_properties: XenditChannelProperties {
                card_details,
                billing_information: build_billing_information(resource_common_data),
                success_return_url: return_url.clone(),
                failure_return_url: return_url,
                skip_three_ds: !resource_common_data.is_three_ds(),
                statement_descriptor: request
                    .billing_descriptor
                    .as_ref()
                    .and_then(|descriptor| descriptor.statement_descriptor.clone()),
            },
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<XenditPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let status = common_enums::AttemptStatus::from(&response.status);

        let authorization_data = response
            .latest_payment
            .as_ref()
            .and_then(|payment| payment.payment_details.as_ref())
            .and_then(|details| details.authorization_data.as_ref());

        let payment_response = if status == common_enums::AttemptStatus::Failure {
            // A 2xx body reporting FAILED/EXPIRED is an error, not a success. `failure_code` is
            // read from the payment request first and from the embedded payment as a fallback,
            // because Xendit populates it on whichever of the two actually failed.
            let failure_code = response.failure_code.as_ref().or_else(|| {
                response
                    .latest_payment
                    .as_ref()
                    .and_then(|payment| payment.failure_code.as_ref())
            });

            Err(build_failure_error_response(
                failure_code,
                authorization_data,
                Some(response.payment_request_id.clone()),
                status,
                http_code,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    response.payment_request_id.clone(),
                ),
                redirection_data: get_redirection_data(response.actions.as_ref()),
                mandate_reference: response
                    .latest_payment
                    .as_ref()
                    .and_then(|payment| payment.payment_token_id.clone())
                    .map(|payment_token_id| {
                        Box::new(MandateReference {
                            connector_mandate_id: Some(payment_token_id.expose()),
                            payment_method_id: None,
                            connector_mandate_request_reference_id: None,
                            mandate_metadata: None,
                        })
                    }),
                connector_metadata: build_connector_metadata(response.latest_payment_id.as_ref()),
                network_txn_id: authorization_data
                    .and_then(|data| data.network_transaction_id.clone()),
                network_txn_link_id: None,
                connector_response_reference_id: Some(response.reference_id.peek().to_string()),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        let response_amount =
            XenditAmountConvertor::convert_back(response.request_amount, response.currency)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    http_code, "xendit",
                ))?;

        let response_integrity_object = Some(AuthoriseIntegrityObject {
            amount: response_amount,
            currency: response.currency,
        });

        Ok(Self {
            response: payment_response,
            request: PaymentsAuthorizeData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                connector_response: build_connector_response(authorization_data),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

// -------------------------------------------------------------------------------------------
// PSync
// -------------------------------------------------------------------------------------------

impl<F> TryFrom<ResponseRouterData<XenditResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<XenditResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        match response {
            XenditResponse::Payment(payment_response) => {
                let status = common_enums::AttemptStatus::from(&payment_response.status);

                let authorization_data = payment_response
                    .latest_payment
                    .as_ref()
                    .and_then(|payment| payment.payment_details.as_ref())
                    .and_then(|details| details.authorization_data.as_ref());

                let response = if status == common_enums::AttemptStatus::Failure {
                    let failure_code = payment_response.failure_code.as_ref().or_else(|| {
                        payment_response
                            .latest_payment
                            .as_ref()
                            .and_then(|payment| payment.failure_code.as_ref())
                    });

                    Err(build_failure_error_response(
                        failure_code,
                        authorization_data,
                        Some(payment_response.payment_request_id.clone()),
                        status,
                        http_code,
                    ))
                } else {
                    Ok(PaymentsResponseData::TransactionResponse {
                        // Same reference the Authorize response reported, so both calls describe
                        // the same payment with the same id.
                        resource_id: ResponseId::ConnectorTransactionId(
                            payment_response.payment_request_id.clone(),
                        ),
                        redirection_data: None,
                        mandate_reference: None,
                        connector_metadata: build_connector_metadata(
                            payment_response.latest_payment_id.as_ref(),
                        ),
                        network_txn_id: authorization_data
                            .and_then(|data| data.network_transaction_id.clone()),
                        network_txn_link_id: None,
                        connector_response_reference_id: Some(
                            payment_response.reference_id.peek().to_string(),
                        ),
                        incremental_authorization_allowed: None,
                        status_code: http_code,
                        splits: None,
                        payment_account_reference: None,
                    })
                };

                Ok(Self {
                    response,
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response: build_connector_response(authorization_data),
                        ..router_data.resource_common_data
                    },
                    ..router_data
                })
            }
            XenditResponse::Webhook(webhook_event) => {
                let status = match webhook_event.event {
                    XenditEventType::PaymentSucceeded | XenditEventType::CaptureSucceeded => {
                        common_enums::AttemptStatus::Charged
                    }
                    XenditEventType::PaymentAwaitingCapture => {
                        common_enums::AttemptStatus::Authorized
                    }
                    XenditEventType::PaymentFailed | XenditEventType::CaptureFailed => {
                        common_enums::AttemptStatus::Failure
                    }
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data
                    },
                    ..router_data
                })
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// Capture
// -------------------------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for XenditPaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = XenditAmountConvertor::convert(
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
        Ok(Self {
            capture_amount: amount,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<XenditCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let mut status = common_enums::AttemptStatus::from(&response.status);

        // Xendit allows a single partial capture. When the captured total is short of the
        // authorized amount the payment is partially charged, not charged; if the response did
        // not carry both figures we cannot prove a full capture, so we report the conservative
        // `PartialCharged` rather than claiming the whole amount settled.
        if status == common_enums::AttemptStatus::Charged {
            let captured_total = response.captures.as_ref().map(|captures| {
                captures
                    .iter()
                    .map(|capture| {
                        XenditAmountConvertor::convert_back(
                            capture.capture_amount,
                            response.currency,
                        )
                        .map(|amount| amount.get_amount_as_i64())
                        .unwrap_or_default()
                    })
                    .sum::<i64>()
            });
            let authorized_total = response
                .request_amount
                .map(|amount| {
                    XenditAmountConvertor::convert_back(amount, response.currency)
                        .map(|amount| amount.get_amount_as_i64())
                })
                .transpose()
                .change_context(crate::utils::response_handling_fail_for_connector(
                    http_code, "xendit",
                ))?;

            status = match (captured_total, authorized_total) {
                (Some(captured), Some(authorized)) if captured >= authorized => {
                    common_enums::AttemptStatus::Charged
                }
                _ => common_enums::AttemptStatus::PartialCharged,
            };
        }

        let authorization_data = response
            .payment_details
            .as_ref()
            .and_then(|details| details.authorization_data.as_ref());

        // Every Xendit flow reports the payment-request id, so a capture describes the same
        // payment with the same reference Authorize and PSync used.
        let resource_id = response
            .payment_request_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        let response_body = if status == common_enums::AttemptStatus::Failure {
            Err(build_failure_error_response(
                response.failure_code.as_ref(),
                authorization_data,
                response.payment_request_id.clone(),
                // A declined capture leaves the authorization intact and capturable, so the
                // caller must see `CaptureFailed`, not a failed payment.
                common_enums::AttemptStatus::CaptureFailed,
                http_code,
            ))
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: build_connector_metadata(Some(&response.payment_id)),
                network_txn_id: authorization_data
                    .and_then(|data| data.network_transaction_id.clone()),
                network_txn_link_id: None,
                connector_response_reference_id: response
                    .reference_id
                    .as_ref()
                    .map(|reference_id| reference_id.peek().to_string()),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        };

        let status = if status == common_enums::AttemptStatus::Failure {
            common_enums::AttemptStatus::CaptureFailed
        } else {
            status
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response: build_connector_response(authorization_data),
                ..router_data.resource_common_data
            },
            response: response_body,
            ..router_data
        })
    }
}

// -------------------------------------------------------------------------------------------
// Refund / RSync (unchanged — the `/refunds` endpoint is not versioned)
// -------------------------------------------------------------------------------------------

#[derive(Default, Debug, Serialize)]
pub struct XenditRefundRequest {
    pub amount: FloatMajorUnit,
    pub payment_request_id: String,
    pub reason: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<XenditRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for XenditRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = XenditAmountConvertor::convert(
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        })?;
        Ok(Self {
            amount: amount.to_owned(),
            payment_request_id: item.router_data.request.connector_transaction_id.clone(),
            reason: "REQUESTED_BY_CUSTOMER".to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: RefundStatus,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefundStatus {
    RequiresAction,
    Succeeded,
    Failed,
    Pending,
    Cancelled,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let response_amount =
            XenditAmountConvertor::convert_back(response.amount, response.currency)
                .change_context(crate::utils::response_handling_fail_for_connector(
                    http_code, "xendit",
                ))?;

        let response_integrity_object = {
            Some(RefundIntegrityObject {
                refund_amount: response_amount,
                currency: response.currency,
            })
        };

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
                acquirer_reference_number: None,
            }),
            request: RefundsData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed | RefundStatus::Cancelled => Self::Failure,
            RefundStatus::Pending | RefundStatus::RequiresAction => Self::Pending,
        }
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: response.id,
                refund_status: common_enums::RefundStatus::from(response.status),
                status_code: http_code,
                acquirer_reference_number: None,
            }),
            ..router_data
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn split_cardholder_name_splits_on_the_last_space() {
        let (first, last) = split_cardholder_name(Some(Secret::new("Ada Lovelace".to_string())));
        assert_eq!(first.map(|n| n.expose()), Some("Ada".to_string()));
        assert_eq!(last.map(|n| n.expose()), Some("Lovelace".to_string()));

        let (first, last) =
            split_cardholder_name(Some(Secret::new("Maria del Carmen Ruiz".to_string())));
        assert_eq!(
            first.map(|n| n.expose()),
            Some("Maria del Carmen".to_string())
        );
        assert_eq!(last.map(|n| n.expose()), Some("Ruiz".to_string()));
    }

    #[test]
    fn split_cardholder_name_handles_single_token_and_absent_names() {
        let (first, last) = split_cardholder_name(Some(Secret::new("  Prince  ".to_string())));
        assert_eq!(first.map(|n| n.expose()), Some("Prince".to_string()));
        assert!(last.is_none());

        let (first, last) = split_cardholder_name(Some(Secret::new("   ".to_string())));
        assert!(first.is_none() && last.is_none());

        let (first, last) = split_cardholder_name(None);
        assert!(first.is_none() && last.is_none());
    }

    #[test]
    fn failure_code_round_trips_and_keeps_unmodelled_values() {
        let known: XenditFailureCode = serde_json::from_str("\"DECLINED_BY_ISSUER\"").unwrap();
        assert_eq!(known, XenditFailureCode::DeclinedByIssuer);
        assert_eq!(known.to_string(), "DECLINED_BY_ISSUER");
        assert_eq!(known.category(), XenditFailureCategory::IssuerDecline);

        // Present on the card-declines page but absent from the OpenAPI enum.
        let declines_page_only: XenditFailureCode =
            serde_json::from_str("\"ISSUER_SUSPECT_FRAUD\"").unwrap();
        assert_eq!(declines_page_only.to_string(), "ISSUER_SUSPECT_FRAUD");

        let unmodelled: XenditFailureCode = serde_json::from_str("\"BRAND_NEW_CODE\"").unwrap();
        assert_eq!(
            unmodelled,
            XenditFailureCode::Unknown("BRAND_NEW_CODE".to_string())
        );
        assert_eq!(unmodelled.to_string(), "BRAND_NEW_CODE");
        assert_eq!(unmodelled.category(), XenditFailureCategory::Unknown);

        assert_eq!(
            XenditFailureCode::InvalidMerchantCredentials.category(),
            XenditFailureCategory::MerchantConfiguration
        );
        assert_eq!(
            XenditFailureCode::IssuerUnavailable.category(),
            XenditFailureCategory::TemporarilyUnavailable
        );
        assert_eq!(
            XenditFailureCode::UserDidNotAuthorize.category(),
            XenditFailureCategory::CustomerAction
        );
    }

    #[test]
    fn verification_results_parse_the_two_documented_values() {
        let matched: XenditVerificationResult = serde_json::from_str("\"M\"").unwrap();
        assert_eq!(matched, XenditVerificationResult::Match);
        assert_eq!(matched.to_string(), "M");

        let no_match: XenditVerificationResult = serde_json::from_str("\"N\"").unwrap();
        assert_eq!(no_match, XenditVerificationResult::NoMatch);
        assert_eq!(no_match.to_string(), "N");

        let unexpected: XenditVerificationResult = serde_json::from_str("\"U\"").unwrap();
        assert_eq!(
            unexpected,
            XenditVerificationResult::Unknown("U".to_string())
        );
    }

    #[test]
    fn payment_request_status_maps_terminal_and_unmodelled_states() {
        use common_enums::AttemptStatus;

        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::RequiresAction),
            AttemptStatus::AuthenticationPending
        );
        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::Authorized),
            AttemptStatus::Authorized
        );
        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::Succeeded),
            AttemptStatus::Charged
        );
        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::Canceled),
            AttemptStatus::Voided
        );
        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::Expired),
            AttemptStatus::Failure
        );
        assert_eq!(
            AttemptStatus::from(&XenditPaymentRequestStatus::Unknown("NEW".to_string())),
            AttemptStatus::Unspecified
        );
    }

    #[test]
    fn redirect_is_taken_only_from_the_web_url_redirect_action() {
        let actions = vec![
            XenditAction {
                action_type: XenditActionType::PresentToCustomer,
                descriptor: XenditActionDescriptor::QrString,
                value: "00020101".to_string(),
            },
            XenditAction {
                action_type: XenditActionType::RedirectCustomer,
                descriptor: XenditActionDescriptor::WebUrl,
                value: "https://redirect.xendit.co/authentications/abc/render?api_key=pk"
                    .to_string(),
            },
        ];

        match get_redirection_data(Some(&actions)).as_deref() {
            Some(RedirectForm::Form {
                endpoint, method, ..
            }) => {
                assert_eq!(
                    endpoint,
                    "https://redirect.xendit.co/authentications/abc/render?api_key=pk"
                );
                assert_eq!(*method, Method::Get);
            }
            other => panic!("expected a redirect form, got {other:?}"),
        }

        let non_redirect = vec![XenditAction {
            action_type: XenditActionType::PresentToCustomer,
            descriptor: XenditActionDescriptor::PaymentCode,
            value: "8808".to_string(),
        }];
        assert!(get_redirection_data(Some(&non_redirect)).is_none());
        assert!(get_redirection_data(None).is_none());
    }

    #[test]
    fn authorization_data_deserialises_from_the_documented_example() {
        // From https://docs.xendit.co/docs/cards-capturing-a-card-payment
        // (the upstream example misspells "successfully"; normalised here for the
        // spell checker — the value itself is not asserted on).
        let raw = r#"{
            "reconciliation_id": "7345007929096981703954",
            "authorization_code": "831000",
            "acquirer_merchant_id": "xendit_ctv_agg",
            "network_response_code": "00",
            "network_transaction_id": "016153570198200",
            "cvn_verification_result": "M",
            "retrieval_reference_number": "435205253972",
            "address_verification_result": "M",
            "network_response_code_descriptor": "Approved and completed successfully"
        }"#;

        let parsed: XenditAuthorizationData = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed.address_verification_result,
            Some(XenditVerificationResult::Match)
        );
        assert_eq!(
            parsed.cvn_verification_result,
            Some(XenditVerificationResult::Match)
        );
        assert_eq!(parsed.network_response_code.as_deref(), Some("00"));
        assert_eq!(parsed.authorization_code.as_deref(), Some("831000"));
        assert_eq!(
            parsed.network_transaction_id.as_deref(),
            Some("016153570198200")
        );
    }

    #[test]
    fn failure_error_response_carries_the_network_codes_and_no_advice_code() {
        let authorization_data = XenditAuthorizationData {
            authorization_code: None,
            address_verification_result: None,
            cvn_verification_result: None,
            network_response_code: Some("05".to_string()),
            network_response_code_descriptor: Some("Do not honor".to_string()),
            network_transaction_id: None,
        };

        let error = build_failure_error_response(
            Some(&XenditFailureCode::DeclinedByIssuer),
            Some(&authorization_data),
            Some("pr-123".to_string()),
            common_enums::AttemptStatus::Failure,
            200,
        );

        assert_eq!(error.code, "DECLINED_BY_ISSUER");
        assert_eq!(error.message, "Do not honor");
        assert_eq!(error.network_decline_code.as_deref(), Some("05"));
        assert_eq!(error.network_error_message.as_deref(), Some("Do not honor"));
        // Xendit exposes no API field carrying a merchant advice code.
        assert!(error.network_advice_code.is_none());
    }

    #[test]
    fn billing_information_is_omitted_when_no_address_is_present() {
        assert!(XenditBillingInformation::default().is_empty());
        assert!(!XenditBillingInformation {
            city: Some(Secret::new("Jakarta".to_string())),
            ..Default::default()
        }
        .is_empty());
    }
}
