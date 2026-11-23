use crate::connectors::hyperswitch::transformers::{map_intent_status, HyperswitchIntentStatus};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, ResponseId, WebhookDetailsResponse,
        WebhookResourceReference,
    },
    errors::WebhookError,
};
use error_stack::{report, ResultExt};
use serde::{Deserialize, Serialize};

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
                    connector_preprocessing_id: None,
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
        raw_connector_status: None,
        amount: None,
        connector_returned_payment_method_details: None,
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
        captured_money: None,
        network_txn_id: payment.connector_transaction_id,
        payment_method_update: None,
        sender_payment_instrument_id: None,
    })
}
