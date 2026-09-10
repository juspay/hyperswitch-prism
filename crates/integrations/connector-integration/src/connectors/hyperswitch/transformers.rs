use common_enums::Currency;
use common_utils::{ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::RepeatPayment,
    connector_types::{
        EventType, MandateReferenceId, PaymentFlowData, PaymentWebhookReference,
        PaymentsResponseData, PaymentsSyncData, RepeatPaymentData, ResponseId,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::hyperswitch::HyperswitchRouterData, types::ResponseRouterData};

// =============================================================================
// AUTH
// =============================================================================
pub struct HyperswitchAuthType {
    pub(super) api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for HyperswitchAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Hyperswitch { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// =============================================================================
// ENUMS
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperswitchIntentStatus {
    Succeeded,
    Failed,
    Cancelled,
    CancelledPostCapture,
    Processing,
    RequiresCustomerAction,
    RequiresMerchantAction,
    RequiresPaymentMethod,
    RequiresConfirmation,
    RequiresCapture,
    PartiallyCaptured,
    PartiallyCapturedAndCapturable,
    PartiallyAuthorizedAndRequiresCapture,
    PartiallyCapturedAndProcessing,
    Conflicted,
    Expired,
    #[serde(other)]
    Unknown,
}

// =============================================================================
// REQUEST: REPEAT PAYMENT (MIT recurring charge)  (POST /payments + recurring_details)
// =============================================================================
// RepeatPayment charges off-session against a payment method vaulted by a prior
// SetupMandate. Hyperswitch references its own vaulted payment method by id, so
// the request uses `recurring_details { type: "payment_method_id", data: "pm_..." }`
// together with the owning `customer_id` — NOT the `processor_payment_token`
// object (that form is for raw processor tokens and leaves Hyperswitch with no
// card data, which the underlying gateway rejects). See tech spec
// "RepeatPayment — Merchant-Initiated Recurring Charge (MIT)".
#[derive(Debug, Serialize)]
pub struct HyperswitchPaymentMethodIdRecurringDetails {
    #[serde(rename = "type")]
    pub recurring_type: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct HyperswitchRepeatPaymentRequest {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub confirm: bool,
    pub off_session: bool,
    pub customer_id: String,
    pub recurring_details: HyperswitchPaymentMethodIdRecurringDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        HyperswitchRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for HyperswitchRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: HyperswitchRouterData<
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

        let amount = item
            .connector
            .amount_converter
            .convert(
                router_data.request.minor_amount,
                router_data.request.currency,
            )
            .change_context(IntegrationError::AmountConversionFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "hyperswitch repeat payment: failed to convert amount to minor units"
                            .to_owned(),
                    ),
                    ..Default::default()
                },
            })?;

        // The vaulted payment-method id comes from the mandate reference
        // established by a prior SetupMandate (the connector maps Hyperswitch's
        // `payment_method_id` into `connector_mandate_id`). Only a connector
        // mandate id is supported here.
        let payment_method_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotImplemented(
                    "hyperswitch repeat payment only supports connector mandate id references"
                        .to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        // Hyperswitch requires the owning `customer_id` to charge a vaulted
        // payment method off-session. Mirror SetupMandate's resolution.
        let customer_id = router_data
            .resource_common_data
            .customer_id
            .as_ref()
            .map(|id| id.get_string_repr().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer_id",
                context: Default::default(),
            })?;

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            confirm: true,
            off_session: true,
            customer_id,
            recurring_details: HyperswitchPaymentMethodIdRecurringDetails {
                recurring_type: "payment_method_id".to_string(),
                data: payment_method_id,
            },
        })
    }
}

// =============================================================================
// RESPONSE: PAYMENTS (PSync / RepeatPayment)
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchPaymentsResponse {
    pub payment_id: String,
    pub status: HyperswitchIntentStatus,
    pub connector_transaction_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

// Distinct identifiers per flow so the macro generates unique `*Templating`
// marker types; all resolve to the same concrete response struct, so the
// `TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, _>>` impls apply.
pub type HyperswitchPSyncResponse = HyperswitchPaymentsResponse;
pub type HyperswitchRepeatPaymentResponse = HyperswitchPaymentsResponse;

fn map_intent_status(
    status: &HyperswitchIntentStatus,
    _is_auto_capture: bool,
) -> common_enums::AttemptStatus {
    match status {
        // Hyperswitch returns `requires_capture` (not `succeeded`) for an
        // authorized-but-uncaptured manual-capture payment, so `succeeded`
        // always means a charge has completed.
        HyperswitchIntentStatus::Succeeded => common_enums::AttemptStatus::Charged,
        HyperswitchIntentStatus::Failed
        | HyperswitchIntentStatus::Conflicted
        | HyperswitchIntentStatus::Expired => common_enums::AttemptStatus::Failure,
        HyperswitchIntentStatus::Cancelled | HyperswitchIntentStatus::CancelledPostCapture => {
            common_enums::AttemptStatus::Voided
        }
        HyperswitchIntentStatus::Processing
        | HyperswitchIntentStatus::PartiallyCapturedAndProcessing
        | HyperswitchIntentStatus::RequiresMerchantAction => common_enums::AttemptStatus::Pending,
        HyperswitchIntentStatus::RequiresCapture
        | HyperswitchIntentStatus::PartiallyAuthorizedAndRequiresCapture => {
            common_enums::AttemptStatus::Authorized
        }
        HyperswitchIntentStatus::PartiallyCaptured => common_enums::AttemptStatus::PartialCharged,
        HyperswitchIntentStatus::PartiallyCapturedAndCapturable => {
            common_enums::AttemptStatus::PartialChargedAndChargeable
        }
        HyperswitchIntentStatus::RequiresPaymentMethod => {
            common_enums::AttemptStatus::PaymentMethodAwaited
        }
        HyperswitchIntentStatus::RequiresConfirmation => {
            common_enums::AttemptStatus::ConfirmationAwaited
        }
        HyperswitchIntentStatus::RequiresCustomerAction => {
            common_enums::AttemptStatus::AuthenticationPending
        }
        HyperswitchIntentStatus::Unknown => common_enums::AttemptStatus::Unknown,
    }
}

fn build_payments_response(
    response: HyperswitchPaymentsResponse,
    http_code: u16,
    status: common_enums::AttemptStatus,
) -> Result<PaymentsResponseData, ErrorResponse> {
    if status == common_enums::AttemptStatus::Failure {
        Err(ErrorResponse {
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string()),
            message: response
                .error_message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.error_message.clone(),
            attempt_status: None,
            connector_transaction_id: Some(response.payment_id.clone()),
            status_code: http_code,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    } else {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.payment_id.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: response.connector_transaction_id.clone(),
            network_txn_link_id: None,
            connector_response_reference_id: Some(response.payment_id.clone()),
            incremental_authorization_allowed: None,
            status_code: http_code,
            splits: None,
            payment_account_reference: None,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = router_data.request.is_auto_capture();
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
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

impl<F> TryFrom<ResponseRouterData<HyperswitchPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<HyperswitchPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let is_auto_capture = !matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Manual)
        );
        let status = map_intent_status(&response.status, is_auto_capture);
        let payment_response = build_payments_response(response, http_code, status);
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

// =============================================================================
// ERROR RESPONSE
// =============================================================================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorResponse {
    pub error: HyperswitchErrorDetail,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchErrorDetail {
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    pub message: Option<String>,
    // Hyperswitch always returns an error `code` in its error envelope; keeping
    // it mandatory ensures this struct doesn't match arbitrary JSON objects.
    pub code: String,
}

// =============================================================================
// INCOMING WEBHOOKS
// =============================================================================
// Hyperswitch (the orchestrator) sends outgoing webhooks shaped as:
//   { "merchant_id": "...", "event_id": "...", "event_type": "payment_succeeded",
//     "content": { "type": "payment_details", "object": { ...PaymentsResponse... } } }
// `content` is adjacently tagged on `type` + `object`. We model `object` as a raw
// JSON value and parse it lazily per content type, so unknown content/event types
// never fail deserialization of the envelope.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookBody {
    pub merchant_id: Option<String>,
    pub event_id: Option<String>,
    pub event_type: String,
    pub content: HyperswitchWebhookContent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub object: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperswitchWebhookPayment {
    pub payment_id: String,
    pub status: HyperswitchIntentStatus,
    pub connector_transaction_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

const CONTENT_TYPE_PAYMENT: &str = "payment_details";

/// Parse the webhook envelope from the raw body.
pub fn get_webhook_object_from_body(
    body: &[u8],
) -> Result<HyperswitchWebhookBody, error_stack::Report<WebhookError>> {
    body.parse_struct("HyperswitchWebhookBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

/// Map a Hyperswitch `event_type` string to the UCS `EventType`.
/// Only payment events are mapped; everything else (including refunds, which this
/// connector does not handle) is left Unspecified rather than force-mapped to a
/// semantically different variant.
pub fn map_webhook_event_type(event_type: &str) -> EventType {
    match event_type {
        "payment_succeeded" => EventType::PaymentIntentSuccess,
        "payment_failed" => EventType::PaymentIntentFailure,
        "payment_processing" => EventType::PaymentIntentProcessing,
        "payment_cancelled" => EventType::PaymentIntentCancelled,
        "payment_authorized" => EventType::PaymentIntentAuthorizationSuccess,
        "payment_captured" => EventType::PaymentIntentCaptureSuccess,
        "payment_expired" => EventType::PaymentIntentExpired,
        "action_required" => EventType::PaymentActionRequired,
        _ => EventType::IncomingWebhookEventUnspecified,
    }
}

fn parse_webhook_payment(
    content: &HyperswitchWebhookContent,
) -> Result<HyperswitchWebhookPayment, error_stack::Report<WebhookError>> {
    if content.content_type != CONTENT_TYPE_PAYMENT {
        return Err(report!(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("hyperswitch webhook content is not payment_details"));
    }
    serde_json::from_value(content.object.clone())
        .change_context(WebhookError::WebhookResourceObjectNotFound)
        .attach_printable("failed to parse hyperswitch payment webhook object")
}

/// Build the typed resource reference (payment) for the ParseEvent phase.
pub fn get_webhook_reference(
    body: &HyperswitchWebhookBody,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    match body.content.content_type.as_str() {
        CONTENT_TYPE_PAYMENT => {
            let payment = parse_webhook_payment(&body.content)?;
            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: payment.connector_transaction_id,
                    merchant_transaction_id: Some(payment.payment_id),
                },
            )))
        }
        _ => Ok(None),
    }
}

/// Build the payment webhook response (HandleEvent → process_payment_webhook).
pub fn build_webhook_payment_response(
    body: &HyperswitchWebhookBody,
    raw_body: &[u8],
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
    let payment = parse_webhook_payment(&body.content)?;
    // Webhooks are out-of-band terminal/async notifications; treat as auto-capture
    // for status mapping (succeeded -> Charged, requires_capture -> Authorized).
    let status = map_intent_status(&payment.status, true);
    Ok(WebhookDetailsResponse {
        resource_id: Some(ResponseId::ConnectorTransactionId(
            payment.payment_id.clone(),
        )),
        status,
        connector_response_reference_id: Some(payment.payment_id.clone()),
        connector_request_reference_id: Some(payment.payment_id),
        mandate_reference: None,
        error_code: payment.error_code,
        error_message: payment.error_message.clone(),
        error_reason: payment.error_message,
        raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
        status_code: 200,
        response_headers: None,
        amount_captured: None,
        minor_amount_captured: None,
        network_txn_id: payment.connector_transaction_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}
