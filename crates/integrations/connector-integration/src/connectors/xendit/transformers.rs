use std::collections::HashMap;

use common_enums::Currency;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    pii,
    request::Method,
    types::{
        FloatMajorUnit, StringMajorUnit, StringMajorUnitForConnector, StringMinorUnitForConnector,
    },
};
use domain_types::{
    connector_flow::{Authorize, Capture, RepeatPayment, SetupMandate},
    connector_types::{
        DisputeWebhookDetailsResponse, DisputeWebhookReference, EventType, MandateReference,
        MandateReferenceId, PaymentFlowData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundWebhookReference,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    payment_method_data::{
        Card, CardDetailsForNetworkTransactionId, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::{
        AuthenticationData, AuthoriseIntegrityObject, PaymentSynIntegrityObject,
        RefundIntegrityObject, RefundSyncIntegrityObject,
    },
    router_response_types::RedirectForm,
    utils::split_full_name,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::xendit::{XenditAmountConvertor, XenditRouterData},
    types::ResponseRouterData,
    utils::get_unimplemented_payment_method_error_message,
};

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

const XENDIT_CONNECTOR_NAME: &str = "xendit";
const XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST: &str =
    "https://docs.xendit.co/apidocs/create-payment-request";
const XENDIT_DOC_URL_3DS: &str = "https://docs.xendit.co/docs/cards-authentication-3ds2";

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

// Xendit error response body (spec "## HTTP Codes and Errors" -> "### Error Response Body Format")
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct XenditErrorResponse {
    pub error_code: Option<String>,
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// v3 request types (POST /v3/payment_requests, api-version 2024-11-11)
// spec "#### 1. Create a payment request" -> "Request Parameters"
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditPaymentRequestType {
    Pay,
    PayAndSave,
    VerifyPaymentMethod,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditCaptureMethod {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditChannelCode {
    Cards,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditCardOnFileType {
    Recurring,
    MerchantUnscheduled,
    CustomerUnscheduled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditTransactionSequence {
    Initial,
    Subsequent,
}

/// The PAN sent in card_details: a raw card (Card<T>) or the plain card number of a
/// network-transaction-id MIT (CardDetailsForNetworkTransactionId). Both serialize as the
/// bare card-number string.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum XenditCardNumber<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Raw(RawCardNumber<T>),
    Network(cards::CardNumber),
}

#[derive(Debug, Serialize)]
pub struct XenditCardDetails<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub card_number: XenditCardNumber<T>,
    pub expiry_month: Secret<String>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    XenditCardDetails<T>
{
    /// Card details from a raw card. Cardholder name = card_holder_name split into
    /// first/last, else the billing first/last name; email = billing email, else the
    /// request email; phone = billing phone (E.164 with country code). All optional on v3
    /// and omitted when absent.
    pub fn from_card(
        card: &Card<T>,
        resource_common_data: &PaymentFlowData,
        request_email: Option<pii::Email>,
    ) -> Self {
        let cvn = Some(card.card_cvc.clone()).filter(|cvc| !cvc.peek().is_empty());
        Self::build(
            XenditCardNumber::Raw(card.card_number.clone()),
            card.card_exp_month.clone(),
            card.get_expiry_year_4_digit(),
            cvn,
            card.card_holder_name.clone(),
            resource_common_data,
            request_email,
        )
    }

    /// Card details for a Model B MIT (spec "#### 13. Model B — PAN MIT"): the full PAN is
    /// resent with the scheme network_transaction_id. The NTI card carries no CVC.
    pub fn from_network_transaction_card(
        card: &CardDetailsForNetworkTransactionId,
        resource_common_data: &PaymentFlowData,
        request_email: Option<pii::Email>,
    ) -> Self {
        Self::build(
            XenditCardNumber::Network(card.card_number.clone()),
            card.card_exp_month.clone(),
            card.get_expiry_year_4_digit(),
            None,
            card.card_holder_name.clone(),
            resource_common_data,
            request_email,
        )
    }

    fn build(
        card_number: XenditCardNumber<T>,
        expiry_month: Secret<String>,
        expiry_year: Secret<String>,
        cvn: Option<Secret<String>>,
        card_holder_name: Option<Secret<String>>,
        resource_common_data: &PaymentFlowData,
        request_email: Option<pii::Email>,
    ) -> Self {
        let (first_name, last_name) = match split_full_name(card_holder_name) {
            (None, None) => (
                resource_common_data.get_optional_billing_first_name(),
                resource_common_data.get_optional_billing_last_name(),
            ),
            names => names,
        };
        Self {
            card_number,
            expiry_month,
            expiry_year,
            cvn,
            cardholder_first_name: first_name,
            cardholder_last_name: last_name,
            cardholder_email: resource_common_data
                .get_optional_billing_email()
                .or(request_email),
            // Optional field: a billing phone that cannot be rendered with its country code is
            // simply not sent.
            cardholder_phone_number: resource_common_data.get_billing_phone_number().ok(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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
    pub country: Option<common_enums::CountryAlpha2>,
}

impl XenditBillingInformation {
    /// spec "### 1. Billing details on the Authorize request — SUPPORTED". Omitted entirely
    /// when the request carries no billing address.
    pub fn from_flow_data(resource_common_data: &PaymentFlowData) -> Option<Self> {
        let billing = Self {
            street_line1: resource_common_data.get_optional_billing_line1(),
            street_line2: resource_common_data.get_optional_billing_line2(),
            city: resource_common_data.get_optional_billing_city(),
            province_state: resource_common_data.get_optional_billing_state(),
            postal_code: resource_common_data.get_optional_billing_zip(),
            country: resource_common_data.get_optional_billing_country(),
        };
        let has_any = billing.street_line1.is_some()
            || billing.street_line2.is_some()
            || billing.city.is_some()
            || billing.province_state.is_some()
            || billing.postal_code.is_some()
            || billing.country.is_some();
        has_any.then_some(billing)
    }
}

#[derive(Debug, Serialize)]
pub struct XenditChannelProperties<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_details: Option<XenditCardDetails<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_information: Option<XenditBillingInformation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_three_ds: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_descriptor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_on_file_type: Option<XenditCardOnFileType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_sequence: Option<XenditTransactionSequence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_transaction_id: Option<Secret<String>>,
}

/// v3 create-payment-request body for a card charge (PAY / PAY_AND_SAVE).
/// No country, no shipping, no L2/L3 or line items (none are sent on the v3 card request),
/// no description/metadata, no idempotency field.
#[derive(Debug, Serialize)]
pub struct XenditPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub reference_id: String,
    #[serde(rename = "type")]
    pub payment_request_type: XenditPaymentRequestType,
    pub currency: Currency,
    pub request_amount: FloatMajorUnit,
    pub capture_method: XenditCaptureMethod,
    pub channel_code: XenditChannelCode,
    pub channel_properties: XenditChannelProperties<T>,
}

/// v3 create-payment-request body for a zero-amount card verification (SetupMandate,
/// spec "#### 9. Verify a payment method"). request_amount and capture_method are "ignored
/// if passed" and payment_token_id is not applicable, so none of them is sent.
#[derive(Debug, Serialize)]
pub struct XenditSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub reference_id: String,
    #[serde(rename = "type")]
    pub payment_request_type: XenditPaymentRequestType,
    pub currency: Currency,
    pub channel_code: XenditChannelCode,
    pub channel_properties: XenditChannelProperties<T>,
}

/// v3 create-payment-request body for a merchant-initiated charge (RepeatPayment).
/// Model A (spec "#### 12. Model A — token MIT"): top-level payment_token_id, no channel_code
/// and no card_details / network_transaction_id. Model B (spec "#### 13. Model B — PAN MIT"):
/// channel_code CARDS + full card_details + network_transaction_id, no payment_token_id.
#[derive(Debug, Serialize)]
pub struct XenditRepeatPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub reference_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_token_id: Option<Secret<String>>,
    #[serde(rename = "type")]
    pub payment_request_type: XenditPaymentRequestType,
    pub currency: Currency,
    pub request_amount: FloatMajorUnit,
    pub capture_method: XenditCaptureMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_code: Option<XenditChannelCode>,
    pub channel_properties: XenditChannelProperties<T>,
}

// ---------------------------------------------------------------------------
// v3 response types — spec "#### 1. Create a payment request" (Response 201) and
// "#### 3. Get the status of a payment" (payment object)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditPaymentRequestStatus {
    AcceptingPayments,
    RequiresAction,
    Authorized,
    Canceled,
    Expired,
    Succeeded,
    Failed,
    Verified,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditPaymentStatus {
    Pending,
    Authorized,
    Succeeded,
    Failed,
    Canceled,
    Expired,
    /// v2-legacy value carried by `payment.awaiting_capture` bodies (HS v2 PaymentStatus).
    AwaitingCapture,
    /// v2-legacy value (HS v2 PaymentStatus).
    Verified,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditActionType {
    RedirectCustomer,
    PresentToCustomer,
    ApiPostRequest,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditActionDescriptor {
    WebUrl,
    DeeplinkUrl,
    CapturePayment,
    PaymentCode,
    QrString,
    VirtualAccountNumber,
    ValidateOtp,
    ResendOtp,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum XenditCheckResult {
    M,
    N,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditAction {
    #[serde(rename = "type")]
    pub action_type: XenditActionType,
    pub descriptor: Option<XenditActionDescriptor>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditAuthorizationData {
    pub authorization_code: Option<String>,
    pub cvn_verification_result: Option<XenditCheckResult>,
    pub address_verification_result: Option<XenditCheckResult>,
    pub network_response_code: Option<String>,
    pub network_response_code_descriptor: Option<String>,
    pub network_transaction_id: Option<Secret<String>>,
    pub retrieval_reference_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPaymentDetails {
    pub authorization_data: Option<XenditAuthorizationData>,
    pub authentication_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditCapture {
    pub capture_id: String,
    pub capture_amount: FloatMajorUnit,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPayment {
    pub payment_id: String,
    pub payment_request_id: String,
    pub reference_id: Option<String>,
    /// pt- token of a PAY_AND_SAVE / token payment (optional `latest_payment.payment_token_id`).
    pub payment_token_id: Option<Secret<String>>,
    pub status: XenditPaymentStatus,
    pub failure_code: Option<String>,
    pub capture_method: Option<XenditCaptureMethod>,
    pub captures: Option<Vec<XenditCapture>>,
    pub payment_details: Option<XenditPaymentDetails>,
}

impl XenditPayment {
    fn authorization_data(&self) -> Option<&XenditAuthorizationData> {
        self.payment_details
            .as_ref()
            .and_then(|details| details.authorization_data.as_ref())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditResponseCardDetails {
    pub network: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditResponseChannelProperties {
    pub card_details: Option<XenditResponseCardDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditPaymentRequestResponse {
    pub payment_request_id: String,
    pub reference_id: String,
    pub status: XenditPaymentRequestStatus,
    #[serde(rename = "type")]
    pub payment_request_type: Option<XenditPaymentRequestType>,
    pub capture_method: Option<XenditCaptureMethod>,
    pub currency: Currency,
    pub request_amount: Option<FloatMajorUnit>,
    pub actions: Option<Vec<XenditAction>>,
    pub latest_payment_id: Option<String>,
    pub latest_payment: Option<XenditPayment>,
    pub payment_token_id: Option<Secret<String>>,
    pub failure_code: Option<String>,
    pub channel_properties: Option<XenditResponseChannelProperties>,
}

impl XenditPaymentRequestResponse {
    fn authorization_data(&self) -> Option<&XenditAuthorizationData> {
        self.latest_payment
            .as_ref()
            .and_then(|payment| payment.payment_details.as_ref())
            .and_then(|details| details.authorization_data.as_ref())
    }

    fn card_network(&self) -> Option<String> {
        self.channel_properties
            .as_ref()
            .and_then(|properties| properties.card_details.as_ref())
            .and_then(|card_details| card_details.network.clone())
    }

    /// The py- payment id, preferring latest_payment_id and falling back to the embedded
    /// payment object.
    fn payment_id(&self) -> Option<&String> {
        self.latest_payment_id.as_ref().or(self
            .latest_payment
            .as_ref()
            .map(|payment| &payment.payment_id))
    }

    /// The pt- mandate token: top-level payment_token_id (always present on the GET once minted,
    /// per the v3 GET schema), else latest_payment.payment_token_id (create 201 response schema).
    fn payment_token_id(&self) -> Option<&Secret<String>> {
        self.payment_token_id.as_ref().or(self
            .latest_payment
            .as_ref()
            .and_then(|payment| payment.payment_token_id.as_ref()))
    }

    /// REDIRECT_CUSTOMER / WEB_URL action, the Xendit-hosted 3DS page.
    fn redirect_url(&self) -> Option<String> {
        self.actions.as_ref().and_then(|actions| {
            actions
                .iter()
                .find(|action| {
                    action.action_type == XenditActionType::RedirectCustomer
                        && action.descriptor == Some(XenditActionDescriptor::WebUrl)
                })
                .and_then(|action| action.value.clone())
        })
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Payment-request status -> attempt status. Exhaustive, no catch-all.
pub fn map_payment_request_status(
    status: XenditPaymentRequestStatus,
) -> common_enums::AttemptStatus {
    match status {
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::AcceptingPayments => common_enums::AttemptStatus::Pending,
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::RequiresAction => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::Authorized => common_enums::AttemptStatus::Authorized,
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::Succeeded => common_enums::AttemptStatus::Charged,
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::Failed => common_enums::AttemptStatus::Failure,
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::Canceled => common_enums::AttemptStatus::Voided,
        // doc: spec "### Payment request status (`payment_request.status`)"
        XenditPaymentRequestStatus::Expired => common_enums::AttemptStatus::Failure,
        // doc: spec "### Card verification status (`payment_request.status` when `type = VERIFY_PAYMENT_METHOD`)"
        XenditPaymentRequestStatus::Verified => common_enums::AttemptStatus::Charged,
        // doc: spec "### Card verification status (`payment_request.status` when `type = VERIFY_PAYMENT_METHOD`)"
        XenditPaymentRequestStatus::Pending => common_enums::AttemptStatus::Pending,
        // doc: undocumented value — non-terminal until a read-back resolves it
        XenditPaymentRequestStatus::Unknown => common_enums::AttemptStatus::Pending,
    }
}

/// Payment (py-) status -> attempt status. Exhaustive, no catch-all.
pub fn map_payment_status(status: XenditPaymentStatus) -> common_enums::AttemptStatus {
    match status {
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Pending => common_enums::AttemptStatus::Pending,
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Authorized => common_enums::AttemptStatus::Authorized,
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Succeeded => common_enums::AttemptStatus::Charged,
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Failed => common_enums::AttemptStatus::Failure,
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Canceled => common_enums::AttemptStatus::Voided,
        // doc: spec "### Payment status (`payment.status`, `latest_payment.status`, webhook `data.status`)"
        XenditPaymentStatus::Expired => common_enums::AttemptStatus::Failure,
        // doc: v2-legacy (as in Hyperswitch's xendit connector) — AWAITING_CAPTURE is an authorization
        XenditPaymentStatus::AwaitingCapture => common_enums::AttemptStatus::Authorized,
        // doc: v2-legacy — VERIFIED is a completed payment in Hyperswitch's xendit connector
        XenditPaymentStatus::Verified => common_enums::AttemptStatus::Charged,
        // doc: undocumented value — non-terminal until a read-back resolves it
        XenditPaymentStatus::Unknown => common_enums::AttemptStatus::Pending,
    }
}

/// ErrorResponse for a 2xx body whose status is FAILED / EXPIRED (a failed attempt is never Ok).
pub fn build_xendit_failure_response(
    response: &XenditPaymentRequestResponse,
    status_code: u16,
) -> ErrorResponse {
    xendit_failure_response(
        response.failure_code.as_ref(),
        response.authorization_data(),
        &response.payment_request_id,
        status_code,
    )
}

/// Same ErrorResponse for a 2xx payment (py-) body whose status is FAILED / EXPIRED
/// (Capture / Void). connector_transaction_id stays the pr- payment request id.
pub fn build_xendit_payment_failure_response(
    response: &XenditPayment,
    status_code: u16,
) -> ErrorResponse {
    xendit_failure_response(
        response.failure_code.as_ref(),
        response.authorization_data(),
        &response.payment_request_id,
        status_code,
    )
}

fn xendit_failure_response(
    failure_code: Option<&String>,
    authorization_data: Option<&XenditAuthorizationData>,
    payment_request_id: &str,
    status_code: u16,
) -> ErrorResponse {
    let network_response_code =
        authorization_data.and_then(|data| data.network_response_code.clone());
    let network_response_code_descriptor =
        authorization_data.and_then(|data| data.network_response_code_descriptor.clone());
    ErrorResponse {
        status_code,
        code: failure_code
            .cloned()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        // Hyperswitch's xendit error shape: the GSM key is (code, message), so the
        // message is failure_code; the scheme code/descriptor stay in the network_* fields.
        message: failure_code
            .cloned()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: Some(
            failure_code
                .cloned()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        ),
        attempt_status: None,
        connector_transaction_id: Some(payment_request_id.to_string()),
        network_decline_code: network_response_code,
        // Xendit carries no merchant advice code field (spec "### 8. MAC codes").
        network_advice_code: None,
        network_error_message: network_response_code_descriptor,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

/// Carried in connector_metadata / connector_feature_data: the py- payment id that v3
/// Capture and Void act on (spec "##### ⚠️ The `pr-` / `py-` trap").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XenditConnectorMetadata {
    pub payment_id: String,
}

pub fn to_connector_metadata(
    payment_id: Option<&String>,
    http_code: u16,
) -> Result<Option<serde_json::Value>, error_stack::Report<ConnectorError>> {
    payment_id
        .map(|payment_id| {
            serde_json::to_value(XenditConnectorMetadata {
                payment_id: payment_id.clone(),
            })
            .change_context(crate::utils::response_handling_fail_for_connector(
                http_code,
                XENDIT_CONNECTOR_NAME,
            ))
        })
        .transpose()
}

/// The pt- token as the UCS mandate reference (Authorize, PSync, SetupMandate, RepeatPayment,
/// payment webhook): spec "##### The durable mandate identifier".
pub fn xendit_mandate_reference(token: &Secret<String>) -> Box<MandateReference> {
    Box::new(MandateReference {
        connector_mandate_id: Some(token.clone().expose()),
        payment_method_id: None,
        connector_mandate_request_reference_id: None,
        mandate_metadata: None,
    })
}

/// REDIRECT_CUSTOMER / WEB_URL action of a REQUIRES_ACTION payment request as a GET form. The
/// query parameters become form_fields and the endpoint drops the query, so an auto-submitted
/// GET keeps them. An unparsable URL is passed through as a bare GET form.
pub fn xendit_redirect_form(response: &XenditPaymentRequestResponse) -> Option<Box<RedirectForm>> {
    match response.status {
        XenditPaymentRequestStatus::RequiresAction => response.redirect_url().map(|endpoint| {
            Box::new(match url::Url::parse(&endpoint) {
                Ok(redirect_url) => RedirectForm::from((redirect_url, Method::Get)),
                Err(_) => RedirectForm::Form {
                    endpoint,
                    method: Method::Get,
                    form_fields: HashMap::new(),
                },
            })
        }),
        XenditPaymentRequestStatus::AcceptingPayments
        | XenditPaymentRequestStatus::Authorized
        | XenditPaymentRequestStatus::Canceled
        | XenditPaymentRequestStatus::Expired
        | XenditPaymentRequestStatus::Succeeded
        | XenditPaymentRequestStatus::Failed
        | XenditPaymentRequestStatus::Verified
        | XenditPaymentRequestStatus::Pending
        | XenditPaymentRequestStatus::Unknown => None,
    }
}

/// One TransactionResponse for every non-failed v3 payment-request response: pr- id, py- id in connector_metadata, scheme network_transaction_id and the
/// merchant reference echo.
pub fn build_xendit_transaction_response(
    response: &XenditPaymentRequestResponse,
    redirection_data: Option<Box<RedirectForm>>,
    mandate_reference: Option<Box<MandateReference>>,
    http_code: u16,
) -> Result<PaymentsResponseData, error_stack::Report<ConnectorError>> {
    Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(response.payment_request_id.clone()),
        redirection_data,
        mandate_reference,
        connector_metadata: to_connector_metadata(response.payment_id(), http_code)?,
        network_txn_id: response
            .authorization_data()
            .and_then(|data| data.network_transaction_id.clone())
            .map(|ntid| ntid.expose()),
        network_txn_link_id: None,
        connector_response_reference_id: Some(response.reference_id.clone()),
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: None,
    })
}

/// Reads the py- payment id back from connector_feature_data. Never substitutes the pr-
/// connector_transaction_id.
pub fn get_xendit_payment_id(
    connector_feature_data: Option<&pii::SecretSerdeValue>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    connector_feature_data
        .and_then(|data| {
            serde_json::from_value::<XenditConnectorMetadata>(data.peek().clone()).ok()
        })
        .map(|metadata| metadata.payment_id)
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "connector_feature_data.payment_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the connector_feature_data returned by the Xendit Authorize / PSync \
                         response; it carries the py- payment id"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                    additional_context: Some(
                        "Xendit v3 capture/cancel act on the py- payment id, not the pr- payment \
                         request id"
                            .to_owned(),
                    ),
                },
            }
            .into()
        })
}

/// AVS / CVN / auth code / card network on the Authorize response (payment_details.authorization_data).
pub fn build_xendit_connector_response(
    response: &XenditPaymentRequestResponse,
) -> Option<ConnectorResponseData> {
    xendit_card_connector_response(response.authorization_data(), response.card_network())
}

/// AVS / CVN / auth code from a payment (py-) body (Capture). The payment object carries no
/// channel_properties, so no card network (spec "#### 3. Get the status of a payment").
pub fn build_xendit_payment_connector_response(
    response: &XenditPayment,
) -> Option<ConnectorResponseData> {
    xendit_card_connector_response(response.authorization_data(), None)
}

fn xendit_card_connector_response(
    authorization_data: Option<&XenditAuthorizationData>,
    card_network: Option<String>,
) -> Option<ConnectorResponseData> {
    let check_value = |result: Option<XenditCheckResult>| match result {
        Some(XenditCheckResult::M) => Some("M"),
        Some(XenditCheckResult::N) => Some("N"),
        Some(XenditCheckResult::Unknown) | None => None,
    };
    let mut payment_checks = serde_json::Map::new();
    if let Some(avs) =
        check_value(authorization_data.and_then(|data| data.address_verification_result))
    {
        payment_checks.insert("avs_result".to_string(), serde_json::json!(avs));
    }
    if let Some(cvn) = check_value(authorization_data.and_then(|data| data.cvn_verification_result))
    {
        payment_checks.insert("card_validation_result".to_string(), serde_json::json!(cvn));
    }
    let payment_checks =
        (!payment_checks.is_empty()).then_some(serde_json::Value::Object(payment_checks));
    let auth_code = authorization_data.and_then(|data| data.authorization_code.clone());

    if payment_checks.is_none() && auth_code.is_none() && card_network.is_none() {
        return None;
    }
    Some(ConnectorResponseData::with_additional_payment_method_data(
        AdditionalPaymentMethodConnectorResponse::Card {
            authentication_data: None,
            payment_checks,
            card_network,
            domestic_network: None,
            auth_code,
        },
    ))
}

/// card_on_file_type for a merchant-side stored credential; Recurring / Installment are refused.
pub fn get_card_on_file_type(
    mit_category: Option<common_enums::MitCategory>,
) -> Result<XenditCardOnFileType, error_stack::Report<IntegrationError>> {
    match mit_category {
        None
        | Some(common_enums::MitCategory::Unscheduled)
        | Some(common_enums::MitCategory::Resubmission) => {
            Ok(XenditCardOnFileType::MerchantUnscheduled)
        }
        Some(common_enums::MitCategory::Recurring) => Err(IntegrationError::NotSupported {
            message: "Xendit RECURRING card_on_file_type requires recurring_configuration, which \
                      no UCS field carries"
                .to_owned(),
            connector: XENDIT_CONNECTOR_NAME,
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send mit_category UNSCHEDULED (or omit it) for Xendit stored credentials"
                        .to_owned(),
                ),
                doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                additional_context: None,
            },
        }
        .into()),
        Some(common_enums::MitCategory::Installment) => Err(IntegrationError::NotSupported {
            message: "Xendit has no installment card_on_file_type".to_owned(),
            connector: XENDIT_CONNECTOR_NAME,
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Send mit_category UNSCHEDULED (or omit it) for Xendit stored credentials"
                        .to_owned(),
                ),
                doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                additional_context: None,
            },
        }
        .into()),
    }
}

/// Xendit runs 3DS2 itself and accepts no external authentication fields.
pub fn refuse_external_authentication_data(
    authentication_data: Option<&AuthenticationData>,
) -> Result<(), error_stack::Report<IntegrationError>> {
    match authentication_data {
        Some(_) => Err(IntegrationError::NotSupported {
            message: "external 3DS authentication data (eci/cavv/ds_trans_id) — Xendit runs 3DS2 \
                      itself and accepts no authentication fields"
                .to_owned(),
            connector: XENDIT_CONNECTOR_NAME,
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Omit authentication_data and send auth_type THREE_DS so Xendit runs 3DS2"
                        .to_owned(),
                ),
                doc_url: Some(XENDIT_DOC_URL_3DS.to_owned()),
                additional_context: None,
            },
        }
        .into()),
        None => Ok(()),
    }
}

/// Request capture_method -> Xendit capture_method. Xendit supports AUTOMATIC | MANUAL with
/// a single capture.
fn get_xendit_capture_method(
    capture_method: Option<common_enums::CaptureMethod>,
) -> Result<XenditCaptureMethod, error_stack::Report<IntegrationError>> {
    match capture_method {
        Some(common_enums::CaptureMethod::Automatic)
        | Some(common_enums::CaptureMethod::SequentialAutomatic)
        | None => Ok(XenditCaptureMethod::Automatic),
        Some(common_enums::CaptureMethod::Manual) => Ok(XenditCaptureMethod::Manual),
        Some(common_enums::CaptureMethod::ManualMultiple)
        | Some(common_enums::CaptureMethod::Scheduled) => {
            Err(IntegrationError::CaptureMethodNotSupported {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use capture_method AUTOMATIC or MANUAL (single capture)".to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                    additional_context: Some(
                        "Xendit supports AUTOMATIC | MANUAL, single capture".to_owned(),
                    ),
                },
            }
            .into())
        }
    }
}

// Transformer for Request: RouterData -> XenditPaymentsRequest (v3)
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
        let router_data = &item.router_data;
        let request = &router_data.request;

        // External 3DS data is refused before anything else.
        refuse_external_authentication_data(request.authentication_data.as_ref())?;
        // ManualMultiple / Scheduled refused.
        let capture_method = get_xendit_capture_method(request.capture_method)?;
        // A return URL is required for the 3DS redirect.
        let return_url = request.router_return_url.clone().ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send return_url; Xendit needs success/failure return URLs for 3DS"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                    additional_context: None,
                },
            }
        })?;

        match &request.payment_method_data {
            PaymentMethodData::Card(card) => {
                // Same predicate as HS is_mandate_payment() for a CIT.
                let is_off_session_save = request.is_customer_initiated_mandate_payment();
                let (payment_request_type, card_on_file_type, transaction_sequence) =
                    if is_off_session_save {
                        (
                            XenditPaymentRequestType::PayAndSave,
                            // Recurring / Installment refused.
                            Some(get_card_on_file_type(request.mit_category.clone())?),
                            Some(XenditTransactionSequence::Initial),
                        )
                    } else {
                        (XenditPaymentRequestType::Pay, None, None)
                    };

                let request_amount = item
                    .connector
                    .amount_converter
                    .convert(request.minor_amount, request.currency)
                    .change_context(IntegrationError::AmountConversionFailed {
                        context: IntegrationErrorContext {
                            suggested_action: None,
                            doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                            additional_context: Some(
                                "Xendit request_amount is a major-unit number".to_owned(),
                            ),
                        },
                    })?;

                Ok(Self {
                    reference_id: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    payment_request_type,
                    currency: request.currency,
                    request_amount,
                    capture_method,
                    channel_code: XenditChannelCode::Cards,
                    channel_properties: XenditChannelProperties {
                        card_details: Some(XenditCardDetails::from_card(
                            card,
                            &router_data.resource_common_data,
                            request.email.clone(),
                        )),
                        billing_information: XenditBillingInformation::from_flow_data(
                            &router_data.resource_common_data,
                        ),
                        success_return_url: Some(return_url.clone()),
                        failure_return_url: Some(return_url),
                        skip_three_ds: Some(!router_data.resource_common_data.is_three_ds()),
                        // statement_descriptor_suffix has no Xendit field and is not sent.
                        statement_descriptor: request
                            .billing_descriptor
                            .as_ref()
                            .and_then(|descriptor| descriptor.statement_descriptor.clone()),
                        card_on_file_type,
                        transaction_sequence,
                        // A customer-initiated charge carries no prior scheme transaction id.
                        network_transaction_id: None,
                    },
                })
            }
            PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message(XENDIT_CONNECTOR_NAME),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Xendit is integrated for the CARDS channel only".to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                    additional_context: None,
                },
            )
            .into()),
        }
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<XenditPaymentRequestResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentRequestResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // As for SetupMandate, extended to the PAY_AND_SAVE CIT: an off-session Authorize is never
        // reported terminal without the pt- token that is the mandate; PSync (GET) completes it.
        let is_cit = router_data.request.is_customer_initiated_mandate_payment();
        let status = match (
            map_payment_request_status(response.status),
            is_cit,
            response.payment_token_id(),
        ) {
            (
                common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::Authorized,
                true,
                None,
            ) => common_enums::AttemptStatus::Pending,
            (status, _, _) => status,
        };

        let payment_response = match response.status {
            XenditPaymentRequestStatus::Failed | XenditPaymentRequestStatus::Expired => {
                Err(build_xendit_failure_response(&response, http_code))
            }
            XenditPaymentRequestStatus::AcceptingPayments
            | XenditPaymentRequestStatus::RequiresAction
            | XenditPaymentRequestStatus::Authorized
            | XenditPaymentRequestStatus::Canceled
            | XenditPaymentRequestStatus::Succeeded
            | XenditPaymentRequestStatus::Verified
            | XenditPaymentRequestStatus::Pending
            | XenditPaymentRequestStatus::Unknown => Ok(build_xendit_transaction_response(
                &response,
                xendit_redirect_form(&response),
                response.payment_token_id().map(xendit_mandate_reference),
                http_code,
            )?),
        };

        let integrity_object = response
            .request_amount
            .map(|request_amount| {
                XenditAmountConvertor::convert_back(request_amount, response.currency)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        http_code,
                        XENDIT_CONNECTOR_NAME,
                    ))
                    .map(|amount| AuthoriseIntegrityObject {
                        amount,
                        currency: response.currency,
                    })
            })
            .transpose()?;

        let connector_response = build_xendit_connector_response(&response);

        Ok(Self {
            response: payment_response,
            request: PaymentsAuthorizeData {
                integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<XenditPaymentRequestResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentRequestResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // spec "#### 2. Get the status of a payment request": v3 has distinct AUTHORIZED /
        // SUCCEEDED, so the status no longer depends on the request capture_method.
        let status = map_payment_request_status(response.status);

        let payment_response = match response.status {
            XenditPaymentRequestStatus::Failed | XenditPaymentRequestStatus::Expired => {
                Err(build_xendit_failure_response(&response, http_code))
            }
            XenditPaymentRequestStatus::AcceptingPayments
            | XenditPaymentRequestStatus::RequiresAction
            | XenditPaymentRequestStatus::Authorized
            | XenditPaymentRequestStatus::Canceled
            | XenditPaymentRequestStatus::Succeeded
            | XenditPaymentRequestStatus::Verified
            | XenditPaymentRequestStatus::Pending
            // payment_token_id is always present on the GET once minted, which completes a
            // SetupMandate / PAY_AND_SAVE whose create response lacked it.
            | XenditPaymentRequestStatus::Unknown => Ok(build_xendit_transaction_response(
                &response,
                None,
                response.payment_token_id().map(xendit_mandate_reference),
                http_code,
            )?),
        };

        let integrity_object = response
            .request_amount
            .map(|request_amount| {
                XenditAmountConvertor::convert_back(request_amount, response.currency)
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        http_code,
                        XENDIT_CONNECTOR_NAME,
                    ))
                    .map(|amount| PaymentSynIntegrityObject {
                        amount,
                        currency: response.currency,
                    })
            })
            .transpose()?;

        let connector_response = build_xendit_connector_response(&response);

        Ok(Self {
            response: payment_response,
            request: PaymentsSyncData {
                integrity_object,
                ..router_data.request
            },
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

const XENDIT_DOC_URL_CARD_VERIFICATION: &str = "https://docs.xendit.co/docs/card-verification";

// Transformer for Request: RouterData -> XenditSetupMandateRequest (v3 VERIFY_PAYMENT_METHOD)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XenditSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
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
        let request = &router_data.request;

        // External 3DS data is refused before anything else.
        refuse_external_authentication_data(request.authentication_data.as_ref())?;
        // Recurring / Installment refused.
        let card_on_file_type = get_card_on_file_type(request.mit_category.clone())?;
        let is_three_ds = router_data.resource_common_data.is_three_ds();
        // A 3DS challenge during verification needs the return URLs.
        let return_url = match (is_three_ds, request.router_return_url.clone()) {
            (true, None) => Err(IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send return_url; Xendit needs success/failure return URLs when the \
                         card verification runs 3DS"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CARD_VERIFICATION.to_owned()),
                    additional_context: None,
                },
            }),
            (_, return_url) => Ok(return_url),
        }?;

        match &request.payment_method_data {
            PaymentMethodData::Card(card) => Ok(Self {
                reference_id: router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                payment_request_type: XenditPaymentRequestType::VerifyPaymentMethod,
                currency: request.currency,
                channel_code: XenditChannelCode::Cards,
                channel_properties: XenditChannelProperties {
                    card_details: Some(XenditCardDetails::from_card(
                        card,
                        &router_data.resource_common_data,
                        request.email.clone(),
                    )),
                    // spec "#### 9. Verify a payment method" lists no billing_information.
                    billing_information: None,
                    success_return_url: return_url.clone(),
                    failure_return_url: return_url,
                    skip_three_ds: Some(!is_three_ds),
                    // No funds move on a verification, so no statement descriptor is sent.
                    statement_descriptor: None,
                    card_on_file_type: Some(card_on_file_type),
                    transaction_sequence: Some(XenditTransactionSequence::Initial),
                    // The INITIAL leg carries no prior scheme transaction id.
                    network_transaction_id: None,
                },
            }),
            PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_) => Err(IntegrationError::NotImplemented(
                get_unimplemented_payment_method_error_message(XENDIT_CONNECTOR_NAME),
                IntegrationErrorContext {
                    suggested_action: Some(
                        "Xendit card-on-file setup is supported for the CARDS channel only"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CARD_VERIFICATION.to_owned()),
                    additional_context: None,
                },
            )
            .into()),
        }
    }
}

/// SetupMandate response: the v3 payment request object of a VERIFY_PAYMENT_METHOD request.
impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<XenditPaymentRequestResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentRequestResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // A verification is never reported terminal-successful without the
        // pt- payment token that is the mandate. VERIFIED without it stays Pending and PSync
        // (GET /v3/payment_requests/{id}) completes it.
        let status = match (
            map_payment_request_status(response.status),
            response.payment_token_id(),
        ) {
            (common_enums::AttemptStatus::Charged, None) => common_enums::AttemptStatus::Pending,
            (status, _) => status,
        };

        let payment_response = match response.status {
            XenditPaymentRequestStatus::Failed | XenditPaymentRequestStatus::Expired => {
                Err(build_xendit_failure_response(&response, http_code))
            }
            XenditPaymentRequestStatus::AcceptingPayments
            | XenditPaymentRequestStatus::RequiresAction
            | XenditPaymentRequestStatus::Authorized
            | XenditPaymentRequestStatus::Canceled
            | XenditPaymentRequestStatus::Succeeded
            | XenditPaymentRequestStatus::Verified
            | XenditPaymentRequestStatus::Pending
            | XenditPaymentRequestStatus::Unknown => {
                // Shared builder: spec "##### The durable mandate identifier" —
                // the pt- token via the payment_token_id() resolver; WEB_URL query kept.
                Ok(build_xendit_transaction_response(
                    &response,
                    xendit_redirect_form(&response),
                    response.payment_token_id().map(xendit_mandate_reference),
                    http_code,
                )?)
            }
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

const XENDIT_DOC_URL_MIT_TOKEN: &str =
    "https://docs.xendit.co/docs/subsequent-merchant-initiated-transaction";
const XENDIT_DOC_URL_MIT_NTID: &str =
    "https://docs.xendit.co/docs/merchant-initiated-transaction-2";

fn xendit_mit_unsupported_payment_method() -> error_stack::Report<IntegrationError> {
    IntegrationError::NotImplemented(
        get_unimplemented_payment_method_error_message(XENDIT_CONNECTOR_NAME),
        IntegrationErrorContext {
            suggested_action: Some(
                "A Xendit network-transaction-id MIT needs the raw card (card or \
                 card_details_for_network_transaction_id)"
                    .to_owned(),
            ),
            doc_url: Some(XENDIT_DOC_URL_MIT_NTID.to_owned()),
            additional_context: None,
        },
    )
    .into()
}

// Transformer for Request: RouterData -> XenditRepeatPaymentRequest (v3 MIT)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        XenditRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for XenditRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: XenditRouterData<
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
        let request = &router_data.request;

        // External 3DS data is refused before anything else.
        refuse_external_authentication_data(request.authentication_data.as_ref())?;
        // Recurring / Installment refused. A merchant-initiated
        // leg never sends CUSTOMER_UNSCHEDULED.
        let card_on_file_type = get_card_on_file_type(request.mit_category.clone())?;
        // request_amount is required and must be > 0 on a MIT.
        if request.minor_amount.get_amount_as_i64() <= 0 {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "amount",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send an amount greater than zero; a zero-amount MIT is not a documented \
                         Xendit shape"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_MIT_TOKEN.to_owned()),
                    additional_context: Some(
                        "Xendit request_amount must be > 0 whenever payment_token_id is present"
                            .to_owned(),
                    ),
                },
            }
            .into());
        }
        // ManualMultiple / Scheduled refused.
        let capture_method = get_xendit_capture_method(request.capture_method)?;

        let request_amount = item
            .connector
            .amount_converter
            .convert(request.minor_amount, request.currency)
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: Some(XENDIT_DOC_URL_CREATE_PAYMENT_REQUEST.to_owned()),
                    additional_context: Some(
                        "Xendit request_amount is a major-unit number".to_owned(),
                    ),
                },
            })?;
        let reference_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        // Every documented MIT example carries the return URLs; sent when present.
        let return_url = request.router_return_url.clone();

        match &request.mandate_reference {
            // Model A — token MIT (spec "#### 12. Model A — token MIT").
            MandateReferenceId::ConnectorMandateId(connector_mandate) => {
                // The pt- payment token is the only chargeable mandate.
                let payment_token_id =
                    connector_mandate
                        .get_connector_mandate_id()
                        .ok_or_else(|| IntegrationError::MissingConnectorMandateID {
                            context: IntegrationErrorContext {
                                suggested_action: Some(
                                    "Pass the connector_mandate_id (pt- payment token) returned \
                                     by the Xendit SetupRecurring response"
                                        .to_owned(),
                                ),
                                doc_url: Some(XENDIT_DOC_URL_MIT_TOKEN.to_owned()),
                                additional_context: None,
                            },
                        })?;
                Ok(Self {
                    reference_id,
                    payment_token_id: Some(Secret::new(payment_token_id)),
                    payment_request_type: XenditPaymentRequestType::Pay,
                    currency: request.currency,
                    request_amount,
                    capture_method,
                    // channel_code / customer are saved in the token and MUST BE OMITTED.
                    channel_code: None,
                    channel_properties: XenditChannelProperties {
                        // The PAN lives in the token: no card object on Model A.
                        card_details: None,
                        // spec "#### 12" lists no billing_information for a token MIT.
                        billing_information: None,
                        success_return_url: return_url.clone(),
                        failure_return_url: return_url,
                        // Not sent on the token path (spec "#### 12" field table).
                        skip_three_ds: None,
                        // spec "#### 12" lists no statement_descriptor for a token MIT.
                        statement_descriptor: None,
                        card_on_file_type: Some(card_on_file_type),
                        transaction_sequence: Some(XenditTransactionSequence::Subsequent),
                        // NOT SENT on Model A: it belongs to Model B only.
                        network_transaction_id: None,
                    },
                })
            }
            // Model B — PAN MIT (spec "#### 13. Model B — PAN MIT").
            MandateReferenceId::NetworkMandateId(network_mandate) => {
                // The merchant-supplied scheme transaction id is required.
                if network_mandate.network_transaction_id.trim().is_empty() {
                    return Err(IntegrationError::MissingRequiredField {
                        field_name: "network_transaction_id",
                        context: IntegrationErrorContext {
                            suggested_action: Some(
                                "Pass the scheme network_transaction_id of the initial \
                                 customer-initiated charge"
                                    .to_owned(),
                            ),
                            doc_url: Some(XENDIT_DOC_URL_MIT_NTID.to_owned()),
                            additional_context: None,
                        },
                    }
                    .into());
                }
                let card_details = match &request.payment_method_data {
                    PaymentMethodData::Card(card) => XenditCardDetails::from_card(
                        card,
                        &router_data.resource_common_data,
                        request.email.clone(),
                    ),
                    PaymentMethodData::CardDetailsForNetworkTransactionId(card) => {
                        XenditCardDetails::from_network_transaction_card(
                            card,
                            &router_data.resource_common_data,
                            request.email.clone(),
                        )
                    }
                    PaymentMethodData::CardWithNoCvc(_)
                    | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
                    | PaymentMethodData::CardRedirect(_)
                    | PaymentMethodData::Wallet(_)
                    | PaymentMethodData::PayLater(_)
                    | PaymentMethodData::BankRedirect(_)
                    | PaymentMethodData::BankDebit(_)
                    | PaymentMethodData::BankTransfer(_)
                    | PaymentMethodData::Crypto(_)
                    | PaymentMethodData::MandatePayment
                    | PaymentMethodData::Reward
                    | PaymentMethodData::RealTimePayment(_)
                    | PaymentMethodData::Upi(_)
                    | PaymentMethodData::Voucher(_)
                    | PaymentMethodData::GiftCard(_)
                    | PaymentMethodData::PaymentMethodToken(_)
                    | PaymentMethodData::OpenBanking(_)
                    | PaymentMethodData::NetworkToken(_)
                    | PaymentMethodData::MobilePayment(_) => {
                        return Err(xendit_mit_unsupported_payment_method())
                    }
                };
                Ok(Self {
                    reference_id,
                    // Model B is the no-token path: payment_token_id MUST BE OMITTED.
                    payment_token_id: None,
                    payment_request_type: XenditPaymentRequestType::Pay,
                    currency: request.currency,
                    request_amount,
                    capture_method,
                    channel_code: Some(XenditChannelCode::Cards),
                    channel_properties: XenditChannelProperties {
                        card_details: Some(card_details),
                        // spec "#### 13" lists no billing_information for a PAN MIT.
                        billing_information: None,
                        success_return_url: return_url.clone(),
                        failure_return_url: return_url,
                        // No cardholder is present on a subsequent merchant-initiated leg.
                        skip_three_ds: Some(true),
                        // spec "#### 13" lists no statement_descriptor for a PAN MIT.
                        statement_descriptor: None,
                        card_on_file_type: Some(card_on_file_type),
                        transaction_sequence: Some(XenditTransactionSequence::Subsequent),
                        network_transaction_id: Some(Secret::new(
                            network_mandate.network_transaction_id.clone(),
                        )),
                    },
                })
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => Err(IntegrationError::NotSupported {
                message: "network token MIT (network_token_with_nti) — the Xendit card API \
                          accepts no network token"
                    .to_owned(),
                connector: XENDIT_CONNECTOR_NAME,
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use connector_mandate_id (pt- token) or network_mandate_id with the raw \
                         card"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_MIT_NTID.to_owned()),
                    additional_context: None,
                },
            }
            .into()),
        }
    }
}

/// RepeatPayment response: the v3 payment request object of a merchant-initiated PAY.
impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<XenditPaymentRequestResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<XenditPaymentRequestResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // REQUIRES_ACTION on a MIT maps to AuthenticationPending like every other leg, but no
        // redirection_data is emitted: there is no cardholder to redirect, so the
        // action is surfaced, not followed.
        let status = map_payment_request_status(response.status);

        let payment_response = match response.status {
            XenditPaymentRequestStatus::Failed | XenditPaymentRequestStatus::Expired => {
                Err(build_xendit_failure_response(&response, http_code))
            }
            XenditPaymentRequestStatus::AcceptingPayments
            | XenditPaymentRequestStatus::RequiresAction
            | XenditPaymentRequestStatus::Authorized
            | XenditPaymentRequestStatus::Canceled
            | XenditPaymentRequestStatus::Succeeded
            | XenditPaymentRequestStatus::Verified
            | XenditPaymentRequestStatus::Pending
            | XenditPaymentRequestStatus::Unknown => {
                // Model A charges the pt- token, which stays the mandate; resolved through
                // payment_token_id() (top-level, else latest_payment). Model B has
                // no token.
                let mandate_reference = match &router_data.request.mandate_reference {
                    MandateReferenceId::ConnectorMandateId(_) => {
                        response.payment_token_id().map(xendit_mandate_reference)
                    }
                    MandateReferenceId::NetworkMandateId(_)
                    | MandateReferenceId::NetworkTokenWithNTI(_) => None,
                };
                // Shared builder; no redirection_data on a MIT.
                Ok(build_xendit_transaction_response(
                    &response,
                    None,
                    mandate_reference,
                    http_code,
                )?)
            }
        };

        let connector_response = build_xendit_connector_response(&response);

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

const XENDIT_DOC_URL_CAPTURE: &str = "https://docs.xendit.co/apidocs/capture-payment";

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
        let request = &item.router_data.request;
        // Xendit allows a single (full or partial) capture per authorization
        // (spec "### 11. Auto capture vs manual capture").
        if request.multiple_capture_data.is_some() {
            return Err(IntegrationError::NotSupported {
                message: "multiple partial captures".to_owned(),
                connector: XENDIT_CONNECTOR_NAME,
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Capture once (full or partial) without multiple_capture_data".to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CAPTURE.to_owned()),
                    additional_context: Some(
                        "Currently multiple partial captures are not supported by Xendit."
                            .to_owned(),
                    ),
                },
            }
            .into());
        }
        // Always sent: the full authorized amount or a single partial amount.
        let capture_amount =
            XenditAmountConvertor::convert(request.minor_amount_to_capture, request.currency)
                .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send amount_to_capture in the minor unit of a currency Xendit supports"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_CAPTURE.to_owned()),
                    additional_context: Some(
                        "capture_amount could not be converted to a Xendit major-unit amount"
                            .to_owned(),
                    ),
                },
            })?;
        Ok(Self { capture_amount })
    }
}

/// POST /v3/payments/{payment_id}/capture body (spec "#### 4. Capture a payment").
#[derive(Serialize, Deserialize, Debug)]
pub struct XenditPaymentsCaptureRequest {
    pub capture_amount: FloatMajorUnit,
}

/// Capture response: the full payment (py-) object (spec "#### 4. Capture a payment",
/// Response 200 = Get Payment schema).
impl<F> TryFrom<ResponseRouterData<XenditPayment, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<XenditPayment, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let status = map_payment_status(response.status);

        let payment_response = match response.status {
            XenditPaymentStatus::Failed | XenditPaymentStatus::Expired => {
                Err(build_xendit_payment_failure_response(&response, http_code))
            }
            XenditPaymentStatus::Pending
            | XenditPaymentStatus::Authorized
            | XenditPaymentStatus::Succeeded
            | XenditPaymentStatus::Canceled
            | XenditPaymentStatus::AwaitingCapture
            | XenditPaymentStatus::Verified
            | XenditPaymentStatus::Unknown => Ok(PaymentsResponseData::TransactionResponse {
                // The pr- payment request id stays the connector transaction id: a later
                // Refund / PSync addresses it, so Capture must not blank it.
                resource_id: ResponseId::ConnectorTransactionId(
                    response.payment_request_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: to_connector_metadata(Some(&response.payment_id), http_code)?,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.reference_id.clone(),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

        let connector_response = build_xendit_payment_connector_response(&response);

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                connector_response: connector_response
                    .or(router_data.resource_common_data.connector_response.clone()),
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

/// Void response: the full payment (py-) object with status CANCELED
/// (spec "#### 5. Cancel a payment (this is Void)", Success response — HTTP 200).
impl<F> TryFrom<ResponseRouterData<XenditPayment, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<XenditPayment, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // CANCELED -> Voided is the only Void success (spec "### Payment status");
        // any other non-failure status is reported as mapped, never as Voided.
        let status = map_payment_status(response.status);

        let payment_response = match response.status {
            XenditPaymentStatus::Failed | XenditPaymentStatus::Expired => {
                Err(build_xendit_payment_failure_response(&response, http_code))
            }
            XenditPaymentStatus::Pending
            | XenditPaymentStatus::Authorized
            | XenditPaymentStatus::Succeeded
            | XenditPaymentStatus::Canceled
            | XenditPaymentStatus::AwaitingCapture
            | XenditPaymentStatus::Verified
            | XenditPaymentStatus::Unknown => Ok(PaymentsResponseData::TransactionResponse {
                // The pr- payment request id stays the connector transaction id.
                resource_id: ResponseId::ConnectorTransactionId(
                    response.payment_request_id.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: response.reference_id.clone(),
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            }),
        };

        Ok(Self {
            response: payment_response,
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            ..router_data
        })
    }
}

const XENDIT_DOC_URL_REFUND: &str = "https://docs.xendit.co/apidocs/refund-payment-request";

/// Refund `reason` (spec "#### 8. Create a refund (Refund)": a required 5-value enum).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditRefundReason {
    Fraudulent,
    Duplicate,
    RequestedByCustomer,
    Cancellation,
    Others,
}

impl XenditRefundReason {
    /// RefundsData.reason when it case-insensitively names one of the five documented
    /// values, otherwise REQUESTED_BY_CUSTOMER (the constant Hyperswitch sends).
    fn from_refund_reason(reason: Option<&str>) -> Self {
        match reason
            .map(|reason| reason.trim().to_ascii_uppercase())
            .as_deref()
        {
            Some("FRAUDULENT") => Self::Fraudulent,
            Some("DUPLICATE") => Self::Duplicate,
            Some("CANCELLATION") => Self::Cancellation,
            Some("OTHERS") => Self::Others,
            // REQUESTED_BY_CUSTOMER, any other text, or no reason.
            Some(_) | None => Self::RequestedByCustomer,
        }
    }
}

// POST /refunds (unversioned; spec "#### 8. Create a refund (Refund)").
#[derive(Debug, Serialize)]
pub struct XenditRefundRequest {
    pub reference_id: String,
    pub payment_request_id: String,
    pub currency: Currency,
    pub amount: FloatMajorUnit,
    pub reason: XenditRefundReason,
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
        let request = &item.router_data.request;
        // Xendit's refund amount has exclusiveMinimum 0; refuse before conversion.
        if request.minor_refund_amount.get_amount_as_i64() <= 0 {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "refund_amount",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a refund_amount greater than zero (full or partial)".to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_REFUND.to_owned()),
                    additional_context: Some(
                        "Xendit refund amount has exclusiveMinimum 0".to_owned(),
                    ),
                },
            }
            .into());
        }
        // Full refund = the captured amount, partial = less; Xendit enforces the remaining total.
        let amount = XenditAmountConvertor::convert(request.minor_refund_amount, request.currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send refund_amount in the minor unit of a currency Xendit supports"
                            .to_owned(),
                    ),
                    doc_url: Some(XENDIT_DOC_URL_REFUND.to_owned()),
                    additional_context: Some(
                        "refund amount could not be converted to a Xendit major-unit amount"
                            .to_owned(),
                    ),
                },
            })?;
        Ok(Self {
            reference_id: request.refund_id.clone(),
            payment_request_id: request.connector_transaction_id.clone(),
            currency: request.currency,
            amount,
            reason: XenditRefundReason::from_refund_reason(request.reason.as_deref()),
        })
    }
}

// Refund object (spec "Payments_API_RefundSchema"), shared by Refund and RSync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XenditRefundResponse {
    pub id: String,
    pub status: XenditRefundStatus,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub failure_code: Option<String>,
    pub payment_request_id: Option<String>,
    pub reference_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditRefundStatus {
    Succeeded,
    Failed,
    Pending,
    Cancelled,
    RequiresAction,
    #[serde(other)]
    Unknown,
}

pub fn map_refund_status(status: &XenditRefundStatus) -> common_enums::RefundStatus {
    match status {
        // doc: spec "#### 8. Create a refund (Refund)" -> Expected result / "### Refund status"
        XenditRefundStatus::Succeeded => common_enums::RefundStatus::Success,
        // doc: spec "### Refund status" (FAILED carries failure_code)
        XenditRefundStatus::Failed => common_enums::RefundStatus::Failure,
        // doc: spec "### Refund status" (CANCELLED is terminal)
        XenditRefundStatus::Cancelled => common_enums::RefundStatus::Failure,
        // doc: spec "#### 8. Create a refund (Refund)" -> PENDING, final state via webhook / RSync
        XenditRefundStatus::Pending => common_enums::RefundStatus::Pending,
        // doc: Hyperswitch-tolerated value, not in the spec enum -> non-terminal
        XenditRefundStatus::RequiresAction => common_enums::RefundStatus::Pending,
        // doc: undocumented value -> non-terminal
        XenditRefundStatus::Unknown => common_enums::RefundStatus::Pending,
    }
}

/// One refund-object reader for Refund and RSync: a 2xx FAILED / CANCELLED refund is a terminal
/// failure (`Err`, attempt_status Refund(Failure)); every other status is `Ok` with the mapped status.
fn refund_result_from_response(
    response: XenditRefundResponse,
    http_code: u16,
) -> Result<RefundsResponseData, ErrorResponse> {
    let refund_status = map_refund_status(&response.status);
    match response.status {
        // A 2xx body whose refund FAILED / was CANCELLED is a terminal refund failure.
        XenditRefundStatus::Failed | XenditRefundStatus::Cancelled => Err(ErrorResponse {
            code: response
                .failure_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .failure_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.failure_code.clone(),
            status_code: http_code,
            attempt_status: Some(FlowStatus::Refund(refund_status)),
            connector_transaction_id: Some(response.id.clone()),
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        }),
        XenditRefundStatus::Succeeded
        | XenditRefundStatus::Pending
        | XenditRefundStatus::RequiresAction
        | XenditRefundStatus::Unknown => Ok(RefundsResponseData {
            connector_refund_id: response.id,
            refund_status,
            status_code: http_code,
            acquirer_reference_number: None,
        }),
    }
}

impl<F> TryFrom<ResponseRouterData<XenditRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<XenditRefundResponse, Self>) -> Result<Self, Self::Error> {
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

        let response_integrity_object = Some(RefundIntegrityObject {
            refund_amount: response_amount,
            currency: response.currency,
        });

        let refund_response = refund_result_from_response(response, http_code);

        Ok(Self {
            response: refund_response,
            request: RefundsData {
                integrity_object: response_integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<XenditRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<XenditRefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        // Refund-sync integrity as in Hyperswitch's xendit connector, on the ids UCS's object carries.
        let integrity_object = Some(RefundSyncIntegrityObject {
            connector_transaction_id: response
                .payment_request_id
                .clone()
                .unwrap_or_else(|| router_data.request.connector_transaction_id.clone()),
            connector_refund_id: response.id.clone(),
        });
        // Same reader as the Refund flow: Refund and RSync read a terminal failure identically.
        let refund_response = refund_result_from_response(response, http_code);
        Ok(Self {
            response: refund_response,
            request: RefundSyncData {
                integrity_object,
                ..router_data.request
            },
            ..router_data
        })
    }
}

// ---------------------------------------------------------------------------
// Incoming webhooks — spec "## Webhook Events"
// Payment / refund events: `{event, business_id, created, data, api_version}` envelope.
// Dispute events: flat body with `event` at the top level and no `data`.
// ---------------------------------------------------------------------------

/// Header carrying the account's static callback token
/// (spec "### Webhook Authentication & Signature Verification").
pub const XENDIT_CALLBACK_TOKEN_HEADER: &str = "x-callback-token";

/// Spec "### Event Types" plus the legacy v2 names Hyperswitch recognises.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum XenditWebhookEventType {
    #[serde(rename = "payment.capture")]
    PaymentCapture,
    /// Legacy v2 alias of `payment.capture`.
    #[serde(rename = "payment.succeeded")]
    PaymentSucceeded,
    #[serde(rename = "payment.authorization")]
    PaymentAuthorization,
    /// Legacy v2 name (Hyperswitch).
    #[serde(rename = "payment.awaiting_capture")]
    PaymentAwaitingCapture,
    /// Legacy v2 name (Hyperswitch).
    #[serde(rename = "capture.succeeded")]
    CaptureSucceeded,
    #[serde(rename = "payment.failure")]
    PaymentFailure,
    /// Legacy v2 alias of `payment.failure`.
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    /// Legacy v2 name (Hyperswitch).
    #[serde(rename = "capture.failed")]
    CaptureFailed,
    #[serde(rename = "payment.expiry")]
    PaymentExpiry,
    #[serde(rename = "payment_request.expiry")]
    PaymentRequestExpiry,
    #[serde(rename = "refund.succeeded")]
    RefundSucceeded,
    #[serde(rename = "refund.failed")]
    RefundFailed,
    #[serde(rename = "dispute.action_required")]
    DisputeActionRequired,
    #[serde(rename = "dispute.under_review")]
    DisputeUnderReview,
    #[serde(rename = "dispute.won")]
    DisputeWon,
    #[serde(rename = "dispute.lost")]
    DisputeLost,
    #[serde(rename = "dispute.due_date_approaching")]
    DisputeDueDateApproaching,
    /// Card verification (VERIFY_PAYMENT_METHOD) completed; `data` is the pr- object keyed `id`.
    #[serde(rename = "payment.verified")]
    PaymentVerified,
    #[serde(rename = "payment_token.activation")]
    PaymentTokenActivation,
    #[serde(rename = "payment_token.failure")]
    PaymentTokenFailure,
    #[serde(rename = "payment_token.expiry")]
    PaymentTokenExpiry,
    #[serde(rename = "payment_session.completed")]
    PaymentSessionCompleted,
    /// Anything undocumented.
    #[serde(other)]
    Unknown,
}

/// Which object a webhook event carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XenditWebhookObject {
    /// `data` is a payment (py-) object keyed by `payment_request_id`.
    Payment,
    /// `data` is a payment-request object (schema not published).
    PaymentRequest,
    /// `data` is the refund object.
    Refund,
    /// The whole body is the dispute object.
    Dispute,
    /// Recognised, no UCS state change (`payment_token.*`, `payment_session.completed`).
    Acknowledged,
    Unsupported,
}

impl XenditWebhookEventType {
    fn object(self) -> XenditWebhookObject {
        match self {
            Self::PaymentCapture
            | Self::PaymentSucceeded
            | Self::PaymentAuthorization
            | Self::PaymentAwaitingCapture
            | Self::CaptureSucceeded
            | Self::PaymentFailure
            | Self::PaymentFailed
            | Self::CaptureFailed
            | Self::PaymentExpiry => XenditWebhookObject::Payment,
            Self::PaymentRequestExpiry | Self::PaymentVerified => {
                XenditWebhookObject::PaymentRequest
            }
            Self::RefundSucceeded | Self::RefundFailed => XenditWebhookObject::Refund,
            Self::DisputeActionRequired
            | Self::DisputeUnderReview
            | Self::DisputeWon
            | Self::DisputeLost
            | Self::DisputeDueDateApproaching => XenditWebhookObject::Dispute,
            Self::PaymentTokenActivation
            | Self::PaymentTokenFailure
            | Self::PaymentTokenExpiry
            | Self::PaymentSessionCompleted => XenditWebhookObject::Acknowledged,
            Self::Unknown => XenditWebhookObject::Unsupported,
        }
    }
}

/// Envelope shared by every Xendit webhook; dispute bodies also carry a top-level `event`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditWebhookEnvelope {
    pub event: XenditWebhookEventType,
    pub business_id: Option<String>,
    pub created: Option<String>,
    pub data: Option<serde_json::Value>,
    pub api_version: Option<String>,
}

/// Ids read from `data` for ParseEvent (payment, payment-request and refund objects).
#[derive(Debug, Clone, Deserialize)]
struct XenditWebhookDataIds {
    id: Option<String>,
    payment_request_id: Option<String>,
}

/// Payment object inside a payment webhook (spec "### Webhook Payload Structure").
/// Only the fields the webhook result needs; v2 legacy bodies share them.
#[derive(Debug, Clone, Deserialize)]
pub struct XenditWebhookPayment {
    pub payment_request_id: String,
    pub reference_id: Option<String>,
    pub status: XenditPaymentStatus,
    pub failure_code: Option<String>,
    pub payment_details: Option<XenditPaymentDetails>,
    /// pt- token of a PAY_AND_SAVE / token payment (spec Payment schema `payment_token_id`).
    pub payment_token_id: Option<Secret<String>>,
    /// Payment schema `type` (PAY | PAY_AND_SAVE | REUSABLE_PAYMENT_CODE).
    #[serde(rename = "type")]
    pub payment_type: Option<XenditWebhookPaymentType>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditWebhookPaymentType {
    Pay,
    PayAndSave,
    ReusablePaymentCode,
    #[serde(other)]
    Unknown,
}

/// Payment-request object inside `payment_request.expiry` (schema not published).
#[derive(Debug, Clone, Deserialize)]
pub struct XenditWebhookPaymentRequest {
    pub payment_request_id: Option<String>,
    pub id: Option<String>,
    pub reference_id: Option<String>,
    pub status: Option<XenditPaymentRequestStatus>,
    pub failure_code: Option<String>,
    /// Optional in the payment-request schema; absent from the payment.verified example (UNDECIDED #6).
    pub payment_token_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XenditDisputeStatus {
    ActionRequired,
    UnderReview,
    Won,
    Lost,
    #[serde(other)]
    Unknown,
}

/// Dispute amounts are strings in major units (spec "### Dispute webhook payload").
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditDisputeAmount {
    pub initial: StringMajorUnit,
    pub terminal: Option<StringMajorUnit>,
}

/// Flat dispute webhook body (spec "### Dispute webhook payload (`dispute.*`)").
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XenditDisputeWebhook {
    pub event: XenditWebhookEventType,
    pub id: String,
    pub payment_id: Option<String>,
    pub reference_id: Option<String>,
    pub status: XenditDisputeStatus,
    pub status_reason: Option<String>,
    pub currency: Currency,
    pub amount: XenditDisputeAmount,
    pub note: Option<String>,
    pub due_date: Option<String>,
}

/// Case-insensitive read of the `x-callback-token` header; an empty value counts as absent.
pub fn get_xendit_callback_token(headers: &HashMap<String, String>) -> Option<&str> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(XENDIT_CALLBACK_TOKEN_HEADER))
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

pub fn parse_xendit_webhook_envelope(
    body: &[u8],
) -> Result<XenditWebhookEnvelope, error_stack::Report<WebhookError>> {
    serde_json::from_slice::<XenditWebhookEnvelope>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("xendit webhook body is not a Xendit event envelope")
}

fn parse_xendit_webhook_data<D: serde::de::DeserializeOwned>(
    data: Option<serde_json::Value>,
) -> Result<D, error_stack::Report<WebhookError>> {
    let data =
        data.ok_or_else(|| error_stack::report!(WebhookError::WebhookResourceObjectNotFound))?;
    serde_json::from_value::<D>(data).change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Spec "#### UCS event mapping (`get_event_type`)". Exhaustive, no catch-all.
pub fn get_xendit_webhook_event_type(
    event: XenditWebhookEventType,
) -> Result<EventType, error_stack::Report<WebhookError>> {
    match event {
        // doc: spec "#### UCS event mapping" — payment.capture -> PaymentIntentSuccess
        XenditWebhookEventType::PaymentCapture => Ok(EventType::PaymentIntentSuccess),
        // doc: spec "### Event Types" legacy note — payment.succeeded is the v2 name of payment.capture
        XenditWebhookEventType::PaymentSucceeded => Ok(EventType::PaymentIntentSuccess),
        // doc: spec "#### UCS event mapping" (Hyperswitch) — capture.succeeded -> PaymentIntentCaptureSuccess
        XenditWebhookEventType::CaptureSucceeded => Ok(EventType::PaymentIntentCaptureSuccess),
        // doc: spec "#### UCS event mapping" — payment.authorization -> PaymentIntentAuthorizationSuccess
        XenditWebhookEventType::PaymentAuthorization => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        // doc: spec "#### UCS event mapping" (Hyperswitch) — payment.awaiting_capture -> PaymentIntentAuthorizationSuccess
        XenditWebhookEventType::PaymentAwaitingCapture => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        // doc: spec "#### UCS event mapping" — payment.failure -> PaymentIntentFailure
        XenditWebhookEventType::PaymentFailure => Ok(EventType::PaymentIntentFailure),
        // doc: spec "### Event Types" legacy note — payment.failed is the v2 name of payment.failure
        XenditWebhookEventType::PaymentFailed => Ok(EventType::PaymentIntentFailure),
        // doc: spec "#### UCS event mapping" (Hyperswitch) — capture.failed -> PaymentIntentFailure
        XenditWebhookEventType::CaptureFailed => Ok(EventType::PaymentIntentFailure),
        // doc: spec "#### UCS event mapping" — payment.expiry -> PaymentIntentExpired
        XenditWebhookEventType::PaymentExpiry => Ok(EventType::PaymentIntentExpired),
        // doc: spec "#### UCS event mapping" — payment_request.expiry -> PaymentIntentExpired
        XenditWebhookEventType::PaymentRequestExpiry => Ok(EventType::PaymentIntentExpired),
        // doc: spec "#### UCS event mapping" — refund.succeeded -> RefundSuccess
        XenditWebhookEventType::RefundSucceeded => Ok(EventType::RefundSuccess),
        // doc: spec "#### UCS event mapping" — refund.failed -> RefundFailure
        XenditWebhookEventType::RefundFailed => Ok(EventType::RefundFailure),
        // doc: spec "#### UCS event mapping" — dispute.action_required -> DisputeOpened
        XenditWebhookEventType::DisputeActionRequired => Ok(EventType::DisputeOpened),
        // doc: spec "#### UCS event mapping" — dispute.due_date_approaching -> DisputeOpened
        XenditWebhookEventType::DisputeDueDateApproaching => Ok(EventType::DisputeOpened),
        // doc: spec "#### UCS event mapping" — dispute.under_review -> DisputeChallenged
        XenditWebhookEventType::DisputeUnderReview => Ok(EventType::DisputeChallenged),
        // doc: spec "#### UCS event mapping" — dispute.won -> DisputeWon
        XenditWebhookEventType::DisputeWon => Ok(EventType::DisputeWon),
        // doc: spec "#### UCS event mapping" — dispute.lost -> DisputeLost
        XenditWebhookEventType::DisputeLost => Ok(EventType::DisputeLost),
        // doc: spec "#### UCS event mapping" — payment.verified (VERIFY_PAYMENT_METHOD done) -> PaymentIntentSuccess
        XenditWebhookEventType::PaymentVerified => Ok(EventType::PaymentIntentSuccess),
        // doc: spec "#### UCS event mapping" — token / session lifecycle: recognised, no payment state change;
        // Unspecified = HS EventNotSupported -> 200 ack so Xendit stops retrying
        XenditWebhookEventType::PaymentTokenActivation
        | XenditWebhookEventType::PaymentTokenFailure
        | XenditWebhookEventType::PaymentTokenExpiry
        | XenditWebhookEventType::PaymentSessionCompleted => {
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
        // doc: undocumented event
        XenditWebhookEventType::Unknown => {
            Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
        }
    }
}

/// ParseEvent reference (spec "#### UCS event mapping", reference-id column).
pub fn get_xendit_webhook_reference(
    body: &[u8],
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    let envelope = parse_xendit_webhook_envelope(body)?;
    match envelope.event.object() {
        XenditWebhookObject::Payment => {
            let ids: XenditWebhookDataIds = parse_xendit_webhook_data(envelope.data)?;
            // pr- payment request id is the connector transaction id (never py-).
            let payment_request_id = ids
                .payment_request_id
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;
            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(payment_request_id),
                    merchant_transaction_id: None,
                },
            )))
        }
        XenditWebhookObject::PaymentRequest => {
            let ids: XenditWebhookDataIds = parse_xendit_webhook_data(envelope.data)?;
            // payment.verified: data.id (the pr- object id), else data.payment_request_id.
            // payment_request.expiry: data.payment_request_id, else data.id.
            let payment_request_id = if envelope.event == XenditWebhookEventType::PaymentVerified {
                ids.id.or(ids.payment_request_id)
            } else {
                ids.payment_request_id.or(ids.id)
            }
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;
            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(payment_request_id),
                    merchant_transaction_id: None,
                },
            )))
        }
        XenditWebhookObject::Refund => {
            let ids: XenditWebhookDataIds = parse_xendit_webhook_data(envelope.data)?;
            let refund_id = ids
                .id
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;
            Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: Some(refund_id),
                    merchant_refund_id: None,
                    // Optional in the refund schema; the docs example omits it.
                    connector_transaction_id: ids.payment_request_id,
                    merchant_transaction_id: None,
                },
            )))
        }
        XenditWebhookObject::Dispute => {
            let dispute = parse_xendit_dispute_webhook(body)?;
            Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: Some(dispute.id),
                    // The dispute names the py- payment id, not the pr- id (spec).
                    connector_transaction_id: dispute.payment_id,
                },
            )))
        }
        // Recognised token / session events carry no payment or refund to look up.
        XenditWebhookObject::Acknowledged => Ok(None),
        XenditWebhookObject::Unsupported => {
            Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
        }
    }
}

fn parse_xendit_dispute_webhook(
    body: &[u8],
) -> Result<XenditDisputeWebhook, error_stack::Report<WebhookError>> {
    serde_json::from_slice::<XenditDisputeWebhook>(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("xendit dispute webhook body did not match the dispute schema")
}

/// HandleEvent payment content: `data.status` through the shared payment-status map, and the
/// shared 2xx-failure error fields when the status is a failure.
pub fn build_xendit_payment_webhook_response(
    body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let envelope = parse_xendit_webhook_envelope(body)?;
    let is_payment_verified = envelope.event == XenditWebhookEventType::PaymentVerified;
    let (
        payment_request_id,
        reference_id,
        status,
        failure_code,
        authorization_data,
        payment_token_id,
    ) = match envelope.event.object() {
        XenditWebhookObject::Payment => {
            let payment: XenditWebhookPayment = parse_xendit_webhook_data(envelope.data)?;
            let authorization_data = payment
                .payment_details
                .and_then(|details| details.authorization_data);
            // On the webhook path too, a PAY_AND_SAVE payment is never reported
            // terminal without the pt- token that is the mandate; PSync GET completes it.
            let status = match (
                map_payment_status(payment.status),
                payment.payment_type,
                payment.payment_token_id.as_ref(),
            ) {
                (
                    common_enums::AttemptStatus::Charged | common_enums::AttemptStatus::Authorized,
                    Some(XenditWebhookPaymentType::PayAndSave),
                    None,
                ) => common_enums::AttemptStatus::Pending,
                (status, _, _) => status,
            };
            (
                payment.payment_request_id,
                payment.reference_id,
                status,
                payment.failure_code,
                authorization_data,
                payment.payment_token_id,
            )
        }
        XenditWebhookObject::PaymentRequest => {
            let payment_request: XenditWebhookPaymentRequest =
                parse_xendit_webhook_data(envelope.data)?;
            // payment.verified: data.id (pr-), else data.payment_request_id;
            // payment_request.expiry: data.payment_request_id, else data.id.
            let payment_request_id = if is_payment_verified {
                payment_request.id.or(payment_request.payment_request_id)
            } else {
                payment_request.payment_request_id.or(payment_request.id)
            }
            .ok_or_else(|| error_stack::report!(WebhookError::WebhookReferenceIdNotFound))?;
            // payment.verified: absent status reads as VERIFIED (spec "#### `payment.verified`").
            // payment_request.expiry: the event itself is "payment request expired"
            // (spec "### Event Types"); its schema is unpublished, so absent reads as EXPIRED.
            let status = payment_request.status.unwrap_or(if is_payment_verified {
                XenditPaymentRequestStatus::Verified
            } else {
                XenditPaymentRequestStatus::Expired
            });
            // As for SetupMandate, a verification without the pt- token stays Pending.
            let status = match (
                map_payment_request_status(status),
                payment_request.payment_token_id.as_ref(),
            ) {
                (common_enums::AttemptStatus::Charged, None) if is_payment_verified => {
                    common_enums::AttemptStatus::Pending
                }
                (status, _) => status,
            };
            (
                payment_request_id,
                payment_request.reference_id,
                status,
                payment_request.failure_code,
                None,
                payment_request.payment_token_id,
            )
        }
        XenditWebhookObject::Refund
        | XenditWebhookObject::Dispute
        | XenditWebhookObject::Acknowledged
        | XenditWebhookObject::Unsupported => {
            return Err(error_stack::report!(
                WebhookError::WebhookResourceObjectNotFound
            ))
            .attach_printable("xendit: expected a payment webhook");
        }
    };

    let network_txn_id = authorization_data
        .as_ref()
        .and_then(|data| data.network_transaction_id.clone())
        .map(|network_transaction_id| network_transaction_id.expose());

    // The pt- card token (payment.* PAY_AND_SAVE / payment.verified), when Xendit sends it;
    // shared mapper with Authorize / PSync / SetupMandate / RepeatPayment.
    let mandate_reference = payment_token_id.as_ref().map(xendit_mandate_reference);

    let (error_code, error_message, error_reason) =
        if status == common_enums::AttemptStatus::Failure {
            // Same code / message / reason as the Authorize / PSync 2xx-failure path.
            let failure = xendit_failure_response(
                failure_code.as_ref(),
                authorization_data.as_ref(),
                &payment_request_id,
                200,
            );
            (Some(failure.code), Some(failure.message), failure.reason)
        } else {
            (None, None, None)
        };

    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(payment_request_id)),
        status,
        connector_response_reference_id: reference_id.clone(),
        // Xendit echoes our reference_id (= connector_request_reference_id).
        connector_request_reference_id: reference_id,
        mandate_reference,
        error_code,
        error_message,
        error_reason,
        raw_connector_response: Some(String::from_utf8_lossy(body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
    })
}

/// HandleEvent refund content: `data` is the refund object (spec "### Refund webhook payload").
pub fn build_xendit_refund_webhook_response(
    body: &[u8],
) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let envelope = parse_xendit_webhook_envelope(body)?;
    if envelope.event.object() != XenditWebhookObject::Refund {
        return Err(error_stack::report!(
            WebhookError::WebhookResourceObjectNotFound
        ))
        .attach_printable("xendit: expected a refund webhook");
    }
    let refund: XenditRefundResponse = parse_xendit_webhook_data(envelope.data)?;
    Ok(RefundWebhookDetailsResponse {
        status: map_refund_status(&refund.status),
        connector_refund_id: Some(refund.id),
        merchant_transaction_id: None,
        connector_response_reference_id: refund.reference_id,
        // Same code / message source as Refund and RSync (refund_result_from_response).
        error_code: refund.failure_code.clone(),
        error_message: refund.failure_code,
        raw_connector_response: Some(String::from_utf8_lossy(body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

/// Spec "### Dispute status (`dispute.status`)". Exhaustive, no catch-all.
fn map_xendit_dispute_status(
    status: XenditDisputeStatus,
) -> Result<common_enums::DisputeStatus, error_stack::Report<WebhookError>> {
    match status {
        // doc: spec "### Dispute status" — ACTION_REQUIRED -> DisputeOpened
        XenditDisputeStatus::ActionRequired => Ok(common_enums::DisputeStatus::DisputeOpened),
        // doc: spec "### Dispute status" — UNDER_REVIEW -> DisputeChallenged
        XenditDisputeStatus::UnderReview => Ok(common_enums::DisputeStatus::DisputeChallenged),
        // doc: spec "### Dispute status" — WON -> DisputeWon
        XenditDisputeStatus::Won => Ok(common_enums::DisputeStatus::DisputeWon),
        // doc: spec "### Dispute status" — LOST -> DisputeLost
        XenditDisputeStatus::Lost => Ok(common_enums::DisputeStatus::DisputeLost),
        // doc: undocumented value — no dispute status is asserted
        XenditDisputeStatus::Unknown => {
            Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound))
        }
    }
}

/// HandleEvent dispute content (flat body). `amount.initial` is a major-unit string, converted
/// to minor units and re-emitted as the StringMinorUnit the dispute response carries.
pub fn build_xendit_dispute_webhook_response(
    body: &[u8],
) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let dispute = parse_xendit_dispute_webhook(body)?;
    if dispute.event.object() != XenditWebhookObject::Dispute {
        return Err(error_stack::report!(
            WebhookError::WebhookResourceObjectNotFound
        ))
        .attach_printable("xendit: expected a dispute webhook");
    }
    let minor_amount = domain_types::utils::convert_back_amount_to_minor_units_for_webhook(
        &StringMajorUnitForConnector,
        dispute.amount.initial.clone(),
        dispute.currency,
    )?;
    let amount = domain_types::utils::convert_amount_for_webhook(
        &StringMinorUnitForConnector,
        minor_amount,
        dispute.currency,
    )?;
    Ok(DisputeWebhookDetailsResponse {
        amount,
        currency: dispute.currency,
        dispute_id: dispute.id,
        status: map_xendit_dispute_status(dispute.status)?,
        stage: common_enums::DisputeStage::Dispute,
        connector_response_reference_id: dispute.reference_id,
        dispute_message: dispute.note,
        connector_reason_code: dispute.status_reason,
        additional_details: None,
        raw_connector_response: Some(String::from_utf8_lossy(body).to_string()),
        status_code: 200,
        response_headers: None,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! Request-body serialization and error-mapping tests (operator requirement; this is the
    //! connector's only test module). Assertions are on
    //! serde_json::to_value of the built body — the exact RequestContent::Json payload — and on
    //! ErrorResponse fields, with literal expected values.
    use std::{marker::PhantomData, str::FromStr};

    use common_utils::types::MinorUnit;
    use domain_types::{
        connector_types::ConnectorMandateReferenceId,
        mandates::{AcceptanceType, CustomerAcceptance},
        payment_address::PaymentAddress,
        payment_method_data::DefaultPCIHolder,
        types::Connectors,
    };

    use super::*;
    use crate::connectors::xendit::Xendit;

    const RETURN_URL: &str = "https://example.com/payment/return";

    fn card() -> PaymentMethodData<DefaultPCIHolder> {
        PaymentMethodData::Card(Card {
            card_number: RawCardNumber(cards::CardNumber::from_str("4000000000001091").unwrap()),
            card_exp_month: Secret::new("12".to_string()),
            card_exp_year: Secret::new("2030".to_string()),
            card_cvc: Secret::new("123".to_string()),
            ..Default::default()
        })
    }

    fn flow_data(auth_type: common_enums::AuthenticationType) -> PaymentFlowData {
        PaymentFlowData {
            merchant_id: common_utils::id_type::MerchantId::default(),
            customer_id: None,
            connector_customer: None,
            payment_id: "pay_xendit_test".to_string(),
            attempt_id: "attempt_xendit_test".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: common_enums::PaymentMethod::Card,
            payment_method_type: None,
            description: None,
            return_url: Some(RETURN_URL.to_string()),
            address: PaymentAddress::new(None, None, None, None),
            auth_type,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            connector_request_reference_id: "ref_xendit_test".to_string(),
            test_mode: None,
            connector_http_status_code: None,
            connector_response_headers: None,
            external_latency: None,
            connectors: Connectors::default().into(),
            raw_connector_response: None,
            typed_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
            merchant_request_id: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
            settlement_status: None,
            raw_connector_status: None,
        }
    }

    fn connector_config() -> ConnectorSpecificConfig {
        ConnectorSpecificConfig::Xendit {
            api_key: Secret::new("xnd_test_key".to_string()),
            base_url: None,
        }
    }

    fn authorize_body(
        setup_future_usage: Option<common_enums::FutureUsage>,
        customer_acceptance: Option<CustomerAcceptance>,
        auth_type: common_enums::AuthenticationType,
    ) -> serde_json::Value {
        let router_data: RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = RouterDataV2 {
            flow: PhantomData,
            resource_common_data: flow_data(auth_type),
            connector_config: connector_config(),
            request: PaymentsAuthorizeData {
                payment_method_data: card(),
                amount: MinorUnit::new(1500000),
                order_tax_amount: None,
                surcharge_amount: None,
                email: None,
                customer_document_details: None,
                customer_date_of_birth: None,
                customer_name: None,
                currency: Currency::IDR,
                confirm: true,
                billing_descriptor: None,
                capture_method: Some(common_enums::CaptureMethod::Automatic),
                router_return_url: Some(RETURN_URL.to_string()),
                webhook_url: None,
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage,
                off_session: None,
                browser_info: None,
                order_category: None,
                session_token: None,
                access_token: None,
                customer_acceptance,
                enrolled_for_3ds: None,
                related_transaction_id: None,
                payment_experience: None,
                payment_method_type: None,
                customer_id: None,
                request_incremental_authorization: None,
                metadata: None,
                authentication_data: None,
                split_payments: None,
                split_settlement: None,
                minor_amount: MinorUnit::new(1500000),
                merchant_order_id: None,
                shipping_cost: None,
                merchant_account_id: None,
                integrity_object: None,
                merchant_config_currency: None,
                all_keys_required: None,
                request_extended_authorization: None,
                enable_overcapture: None,
                setup_mandate_details: None,
                connector_feature_data: None,
                connector_testing_data: None,
                payment_channel: None,
                enable_partial_authorization: None,
                locale: None,
                redirect_response: None,
                threeds_method_comp_ind: None,
                continue_redirection_url: None,
                tokenization: None,
                mit_category: None,
                domain_data: None,
                currency_conversion_data: None,
                is_account_funding_transaction: None,
                recipient_details: None,
                business_country: None,
                additional_connector_details: None,
                customer: None,
                partner_merchant_identifier_details: None,
            },
            response: Err(ErrorResponse::default()),
        };
        let request = XenditPaymentsRequest::try_from(XenditRouterData {
            connector: Xendit::<DefaultPCIHolder>::new().to_owned(),
            router_data,
        })
        .expect("authorize request");
        serde_json::to_value(&request).expect("serialize authorize body")
    }

    fn repeat_payment_body(
        mandate_reference: MandateReferenceId,
        minor_amount: i64,
        currency: Currency,
    ) -> serde_json::Value {
        let router_data: RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = RouterDataV2 {
            flow: PhantomData,
            resource_common_data: flow_data(common_enums::AuthenticationType::NoThreeDs),
            connector_config: connector_config(),
            request: RepeatPaymentData {
                mandate_reference,
                amount: minor_amount,
                minor_amount: MinorUnit::new(minor_amount),
                currency,
                merchant_order_id: None,
                metadata: None,
                webhook_url: None,
                integrity_object: None,
                capture_method: Some(common_enums::CaptureMethod::Automatic),
                browser_info: None,
                email: None,
                customer_document_details: None,
                payment_method_type: None,
                connector_feature_data: None,
                off_session: Some(true),
                router_return_url: None,
                complete_authorize_url: None,
                split_payments: None,
                split_settlement: None,
                recurring_mandate_payment_data: None,
                shipping_cost: None,
                payment_channel: None,
                mit_category: None,
                enable_partial_authorization: None,
                billing_descriptor: None,
                payment_method_data: card(),
                authentication_data: None,
                locale: None,
                connector_testing_data: None,
                merchant_account_id: None,
                merchant_configured_currency: None,
                additional_payment_data: None,
                partner_merchant_identifier_details: None,
                is_account_funding_transaction: None,
                recipient_details: None,
                additional_connector_details: None,
                customer: None,
            },
            response: Err(ErrorResponse::default()),
        };
        let request = XenditRepeatPaymentRequest::try_from(XenditRouterData {
            connector: Xendit::<DefaultPCIHolder>::new().to_owned(),
            router_data,
        })
        .expect("repeat payment request");
        serde_json::to_value(&request).expect("serialize repeat payment body")
    }

    fn failed_payment_request(latest_payment: serde_json::Value) -> XenditPaymentRequestResponse {
        serde_json::from_value(serde_json::json!({
            "payment_request_id": "pr-1f2e3d4c-0000-4000-8000-000000000001",
            "reference_id": "ref_xendit_test",
            "status": "FAILED",
            "currency": "IDR",
            "request_amount": 10051,
            "latest_payment_id": "py-1f2e3d4c-0000-4000-8000-000000000002",
            "latest_payment": latest_payment,
            "failure_code": latest_payment.get("failure_code").cloned(),
        }))
        .expect("deserialize failed payment request")
    }

    fn latest_payment(
        failure_code: Option<&str>,
        network_code: &str,
        descriptor: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "payment_id": "py-1f2e3d4c-0000-4000-8000-000000000002",
            "payment_request_id": "pr-1f2e3d4c-0000-4000-8000-000000000001",
            "status": "FAILED",
            "failure_code": failure_code,
            "payment_details": {
                "authorization_data": {
                    "network_response_code": network_code,
                    "network_response_code_descriptor": descriptor
                }
            }
        })
    }

    #[test]
    fn authorize_pay_body() {
        let body = authorize_body(None, None, common_enums::AuthenticationType::ThreeDs);
        assert_eq!(body["type"], "PAY");
        assert_eq!(body["channel_code"], "CARDS");
        assert_eq!(body["currency"], "IDR");
        assert_eq!(body["capture_method"], "AUTOMATIC");
        assert_eq!(body["reference_id"], "ref_xendit_test");
        assert_eq!(body["request_amount"].as_f64(), Some(15000.0));
        let properties = &body["channel_properties"];
        assert_eq!(properties["skip_three_ds"], false);
        assert_eq!(properties["success_return_url"], RETURN_URL);
        assert_eq!(properties["failure_return_url"], RETURN_URL);
        assert_eq!(
            properties["card_details"]["card_number"],
            "4000000000001091"
        );
        assert_eq!(properties["card_details"]["expiry_year"], "2030");
        assert!(properties.get("card_on_file_type").is_none());
        assert!(properties.get("transaction_sequence").is_none());
        assert!(body.get("payment_token_id").is_none());
        assert!(body.get("country").is_none());
    }

    #[test]
    fn authorize_pay_and_save_body_requires_customer_acceptance() {
        let acceptance = CustomerAcceptance {
            acceptance_type: AcceptanceType::Offline,
            accepted_at: None,
            online: None,
        };
        let saved = authorize_body(
            Some(common_enums::FutureUsage::OffSession),
            Some(acceptance),
            common_enums::AuthenticationType::NoThreeDs,
        );
        assert_eq!(saved["type"], "PAY_AND_SAVE");
        assert_eq!(
            saved["channel_properties"]["card_on_file_type"],
            "MERCHANT_UNSCHEDULED"
        );
        assert_eq!(
            saved["channel_properties"]["transaction_sequence"],
            "INITIAL"
        );
        assert_eq!(saved["channel_properties"]["skip_three_ds"], true);

        // OFF_SESSION without customer_acceptance is a one-time PAY (HS is_mandate_payment).
        let one_time = authorize_body(
            Some(common_enums::FutureUsage::OffSession),
            None,
            common_enums::AuthenticationType::NoThreeDs,
        );
        assert_eq!(one_time["type"], "PAY");
        assert!(one_time["channel_properties"]
            .get("card_on_file_type")
            .is_none());
        assert!(one_time["channel_properties"]
            .get("transaction_sequence")
            .is_none());
    }

    #[test]
    fn capture_body() {
        let capture_amount =
            XenditAmountConvertor::convert(MinorUnit::new(1500000), Currency::IDR).unwrap();
        let body = serde_json::to_value(XenditPaymentsCaptureRequest { capture_amount }).unwrap();
        assert_eq!(body["capture_amount"].as_f64(), Some(15000.0));
        assert_eq!(body.as_object().map(|fields| fields.len()), Some(1));
    }

    #[test]
    fn refund_body() {
        let body = serde_json::to_value(XenditRefundRequest {
            reference_id: "ref_refund_test".to_string(),
            payment_request_id: "pr-1f2e3d4c-0000-4000-8000-000000000001".to_string(),
            currency: Currency::IDR,
            amount: XenditAmountConvertor::convert(MinorUnit::new(1500000), Currency::IDR).unwrap(),
            reason: XenditRefundReason::from_refund_reason(Some("duplicate")),
        })
        .unwrap();
        assert_eq!(
            body["payment_request_id"],
            "pr-1f2e3d4c-0000-4000-8000-000000000001"
        );
        assert_eq!(body["amount"].as_f64(), Some(15000.0));
        assert_eq!(body["currency"], "IDR");
        assert_eq!(body["reason"], "DUPLICATE");
        assert_eq!(
            serde_json::to_value(XenditRefundReason::from_refund_reason(Some(
                "not a documented reason"
            )))
            .unwrap(),
            "REQUESTED_BY_CUSTOMER"
        );
    }

    #[test]
    fn repeat_payment_model_a_token_body() {
        let body = repeat_payment_body(
            MandateReferenceId::ConnectorMandateId(ConnectorMandateReferenceId::new(
                Some("pt-5a6b7c8d-0000-4000-8000-000000000003".to_string()),
                None,
                None,
                None,
                None,
            )),
            6000,
            Currency::USD,
        );
        assert_eq!(
            body["payment_token_id"],
            "pt-5a6b7c8d-0000-4000-8000-000000000003"
        );
        assert_eq!(body["type"], "PAY");
        assert_eq!(body["request_amount"].as_f64(), Some(60.0));
        assert!(body.get("channel_code").is_none());
        let properties = &body["channel_properties"];
        assert!(properties.get("card_details").is_none());
        assert!(properties.get("network_transaction_id").is_none());
        assert_eq!(properties["transaction_sequence"], "SUBSEQUENT");
        assert_eq!(properties["card_on_file_type"], "MERCHANT_UNSCHEDULED");
    }

    #[test]
    fn repeat_payment_model_b_ntid_body() {
        let body = repeat_payment_body(
            MandateReferenceId::NetworkMandateId(
                domain_types::connector_types::NetworkMandateIdRef {
                    network_transaction_id: "12123456789012".to_string(),
                    transaction_link_id: None,
                },
            ),
            6000,
            Currency::USD,
        );
        assert!(body.get("payment_token_id").is_none());
        assert_eq!(body["channel_code"], "CARDS");
        let properties = &body["channel_properties"];
        assert_eq!(
            properties["card_details"]["card_number"],
            "4000000000001091"
        );
        assert_eq!(properties["network_transaction_id"], "12123456789012");
        assert_eq!(properties["skip_three_ds"], true);
        assert_eq!(properties["transaction_sequence"], "SUBSEQUENT");
    }

    #[test]
    fn failure_2xx_maps_failure_code() {
        let response =
            failed_payment_request(latest_payment(Some("EXPIRED_CARD"), "54", "Expired card"));
        let error = build_xendit_failure_response(&response, 201);
        assert_eq!(error.code, "EXPIRED_CARD");
        assert_eq!(error.message, "EXPIRED_CARD");
        assert_eq!(error.reason.as_deref(), Some("EXPIRED_CARD"));
        assert_eq!(error.network_decline_code.as_deref(), Some("54"));
        assert_eq!(error.network_error_message.as_deref(), Some("Expired card"));
        assert_eq!(
            error.connector_transaction_id.as_deref(),
            Some("pr-1f2e3d4c-0000-4000-8000-000000000001")
        );
        assert_eq!(error.status_code, 201);
    }

    #[test]
    fn failure_2xx_with_network_code_00() {
        let response = failed_payment_request(latest_payment(
            Some("CARD_DECLINED"),
            "00",
            "Approved and completed successfully",
        ));
        let error = build_xendit_failure_response(&response, 201);
        assert_eq!(error.code, "CARD_DECLINED");
        assert_eq!(error.message, "CARD_DECLINED");
        assert_eq!(error.reason.as_deref(), Some("CARD_DECLINED"));
    }

    #[test]
    fn failure_2xx_without_failure_code() {
        let response = failed_payment_request(latest_payment(None, "05", "Do not honor"));
        let error = build_xendit_failure_response(&response, 201);
        assert_eq!(error.code, NO_ERROR_CODE);
        assert_eq!(error.message, NO_ERROR_MESSAGE);
        assert_eq!(error.reason.as_deref(), Some(NO_ERROR_MESSAGE));
        assert_eq!(error.network_decline_code.as_deref(), Some("05"));
    }

    fn pay_and_save_capture_webhook(payment_token_id: Option<&str>) -> Vec<u8> {
        let mut data = serde_json::json!({
            "payment_id": "py-rca5-0001",
            "payment_request_id": "pr-rca5-0001",
            "reference_id": "rca5_pas_0001",
            "type": "PAY_AND_SAVE",
            "country": "ID",
            "currency": "IDR",
            "request_amount": 15000,
            "capture_method": "AUTOMATIC",
            "channel_code": "CARDS",
            "status": "SUCCEEDED",
            "failure_code": null
        });
        if let Some(token) = payment_token_id {
            data["payment_token_id"] = serde_json::Value::String(token.to_string());
        }
        serde_json::to_vec(&serde_json::json!({
            "event": "payment.capture",
            "business_id": "66a1b2c3d4e5f60718293a4b",
            "created": "2026-10-02T23:01:25.560Z",
            "api_version": "v3",
            "data": data
        }))
        .unwrap()
    }

    #[test]
    fn payment_webhook_carries_pt_token() {
        let with_token = build_xendit_payment_webhook_response(&pay_and_save_capture_webhook(
            Some("pt-rca5-0001"),
        ))
        .unwrap();
        assert_eq!(with_token.status, common_enums::AttemptStatus::Charged);
        assert_eq!(
            with_token
                .mandate_reference
                .as_ref()
                .and_then(|mandate| mandate.connector_mandate_id.as_deref()),
            Some("pt-rca5-0001")
        );
        assert!(matches!(
            with_token.resource_id,
            Some(ResponseId::ConnectorTransactionId(ref id)) if id == "pr-rca5-0001"
        ));

        let without_token =
            build_xendit_payment_webhook_response(&pay_and_save_capture_webhook(None)).unwrap();
        assert_eq!(without_token.status, common_enums::AttemptStatus::Pending);
        assert!(without_token.mandate_reference.is_none());
    }

    #[test]
    fn redirect_form_keeps_web_url_query() {
        let response: XenditPaymentRequestResponse = serde_json::from_value(serde_json::json!({
            "payment_request_id": "pr-1f2e3d4c-0000-4000-8000-000000000001",
            "reference_id": "ref_xendit_test",
            "status": "REQUIRES_ACTION",
            "currency": "IDR",
            "actions": [{
                "type": "REDIRECT_CUSTOMER",
                "descriptor": "WEB_URL",
                "value": "https://x.example/render?api_key=k"
            }]
        }))
        .unwrap();
        match xendit_redirect_form(&response).map(|form| *form) {
            Some(RedirectForm::Form {
                endpoint,
                method,
                form_fields,
            }) => {
                assert_eq!(endpoint, "https://x.example/render");
                assert_eq!(method, Method::Get);
                assert_eq!(form_fields.get("api_key").map(String::as_str), Some("k"));
            }
            other => panic!("expected a GET redirect form, got {other:?}"),
        }
    }
}
