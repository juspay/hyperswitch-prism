use crate::connectors::nexixpay::transformers::{
    NexixpayAdditionalData, NexixpayOperationType, NexixpayPaymentStatus,
};
use common_utils::{self, types::StringMinorUnit};
use domain_types::{connector_types::EventType, errors::WebhookError};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::Deserialize;

/// Wire shape of the notification. The fields Nexi documents as required are optional here
/// only so a missing one is refused by name (`WebhookMissingRequiredField`) in
/// [`parse_nexixpay_webhook`] rather than as an anonymous decoding failure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NexixpayWebhookPayload {
    event_id: Option<String>,
    event_time: Option<String>,
    security_token: Option<Secret<String>>,
    operation: Option<NexixpayWebhookOperationPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NexixpayWebhookOperationPayload {
    order_id: Option<String>,
    operation_id: Option<String>,
    operation_type: Option<NexixpayOperationType>,
    operation_result: Option<NexixpayPaymentStatus>,
    operation_amount: Option<StringMinorUnit>,
    operation_currency: Option<common_enums::Currency>,
    additional_data: Option<NexixpayAdditionalData>,
}

/// A validated XPay notification (spec: ## Webhook Events, Webhook Payload Structure).
#[derive(Debug, Clone)]
pub struct NexixpayWebhookBody {
    pub event_id: String,
    pub event_time: String,
    pub security_token: Secret<String>,
    pub operation: NexixpayWebhookOperation,
}

/// The notification's `operation` object (same shape as `GET /operations/{operationId}`).
#[derive(Debug, Clone)]
pub struct NexixpayWebhookOperation {
    pub order_id: String,
    pub operation_id: String,
    pub operation_type: NexixpayOperationType,
    pub operation_result: NexixpayPaymentStatus,
    pub operation_amount: Option<StringMinorUnit>,
    pub operation_currency: Option<common_enums::Currency>,
    pub additional_data: Option<NexixpayAdditionalData>,
}

fn required_webhook_field(
    value: Option<String>,
    field: &'static str,
) -> Result<String, error_stack::Report<WebhookError>> {
    value
        .filter(|v| !v.is_empty())
        .ok_or_else(|| WebhookError::WebhookMissingRequiredField { field }.into())
}

/// Parses an XPay notification and applies the documented required-field check:
/// "securityToken, eventId, operationId, orderId, eventTime" must be present before the
/// notification is processed (spec: ## Webhook Events, Webhook Authentication).
pub fn parse_nexixpay_webhook(
    body: &[u8],
) -> Result<NexixpayWebhookBody, error_stack::Report<WebhookError>> {
    let payload: NexixpayWebhookPayload =
        serde_json::from_slice(body).change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let security_token = payload
        .security_token
        .filter(|token| !token.peek().is_empty())
        .ok_or(WebhookError::WebhookMissingRequiredField {
            field: "securityToken",
        })?;
    let event_id = required_webhook_field(payload.event_id, "eventId")?;
    let event_time = required_webhook_field(payload.event_time, "eventTime")?;
    let operation = payload
        .operation
        .ok_or(WebhookError::WebhookMissingRequiredField { field: "operation" })?;
    let order_id = required_webhook_field(operation.order_id, "operation.orderId")?;
    let operation_id = required_webhook_field(operation.operation_id, "operation.operationId")?;
    let operation_type =
        operation
            .operation_type
            .ok_or(WebhookError::WebhookMissingRequiredField {
                field: "operation.operationType",
            })?;
    let operation_result =
        operation
            .operation_result
            .ok_or(WebhookError::WebhookMissingRequiredField {
                field: "operation.operationResult",
            })?;
    Ok(NexixpayWebhookBody {
        event_id,
        event_time,
        security_token,
        operation: NexixpayWebhookOperation {
            order_id,
            operation_id,
            operation_type,
            operation_result,
            operation_amount: operation.operation_amount,
            operation_currency: operation.operation_currency,
            additional_data: operation.additional_data,
        },
    })
}

pub fn get_nexixpay_webhook_event_type(operation: &NexixpayWebhookOperation) -> Option<EventType> {
    use NexixpayPaymentStatus as R;
    match operation.operation_type {
        NexixpayOperationType::Authorization => match operation.operation_result {
            // spec: Event Types (AUTHORIZATION / AUTHORIZED "Payment authorized")
            R::Authorized => Some(EventType::PaymentIntentAuthorizationSuccess),
            // spec: Status Mappings (EXECUTED "Payment confirmed" -> Charged)
            R::Executed => Some(EventType::PaymentIntentSuccess),
            // spec: Event Types (AUTHORIZATION / PENDING "Payment ongoing")
            R::Pending => Some(EventType::PaymentIntentProcessing),
            // spec: Event Types (DECLINED, DENIED_BY_RISK, THREEDS_FAILED, FAILED -> failure)
            R::Declined | R::DeniedByRisk | R::ThreedsFailed | R::Failed => {
                Some(EventType::PaymentIntentFailure)
            }
            // spec: Event Types (AUTHORIZATION / CANCELED "Canceled by the cardholder")
            R::Canceled => Some(EventType::PaymentIntentCancelled),
            // spec: Event Types ((any) / THREEDS_VALIDATED not mapped); others undocumented
            R::ThreedsValidated | R::Voided | R::Refunded | R::Unknown => None,
        },
        NexixpayOperationType::Capture => match operation.operation_result {
            // spec: Event Types (CAPTURE / EXECUTED "Payment confirmed")
            R::Executed => Some(EventType::PaymentIntentSuccess),
            // spec: Event Types (CAPTURE / PENDING "Payment ongoing")
            R::Pending => Some(EventType::PaymentIntentProcessing),
            // spec: Event Types (DECLINED, DENIED_BY_RISK, THREEDS_FAILED, FAILED -> failure)
            R::Declined | R::DeniedByRisk | R::ThreedsFailed | R::Failed => {
                Some(EventType::PaymentIntentFailure)
            }
            // spec: Event Types ((any) / THREEDS_VALIDATED not mapped); others undocumented
            R::Authorized
            | R::ThreedsValidated
            | R::Canceled
            | R::Voided
            | R::Refunded
            | R::Unknown => None,
        },
        NexixpayOperationType::Void => match operation.operation_result {
            // spec: Event Types (VOID / VOIDED "Online reversal of the full authorized amount")
            R::Voided => Some(EventType::PaymentIntentCancelled),
            // spec: Event Types (no other VOID result is documented)
            R::Authorized
            | R::Executed
            | R::Declined
            | R::DeniedByRisk
            | R::ThreedsValidated
            | R::ThreedsFailed
            | R::Pending
            | R::Canceled
            | R::Refunded
            | R::Failed
            | R::Unknown => None,
        },
        NexixpayOperationType::Refund => match operation.operation_result {
            // spec: Event Types (REFUND / EXECUTED, VOIDED, REFUNDED -> refund success)
            R::Executed | R::Voided | R::Refunded => Some(EventType::RefundSuccess),
            // spec: Event Types (REFUND / FAILED, DECLINED -> refund failure)
            R::Failed | R::Declined => Some(EventType::RefundFailure),
            // spec: Event Types (REFUND / PENDING -> refund processing)
            R::Pending => Some(EventType::RefundProcessing),
            // spec: Event Types (no other REFUND result is documented)
            R::Authorized
            | R::DeniedByRisk
            | R::ThreedsValidated
            | R::ThreedsFailed
            | R::Canceled
            | R::Unknown => None,
        },
        // spec: Event Types (CANCEL / any "rollback of a capture, refund" -> not mapped);
        // CARD_VERIFICATION and unknown operation types move no money and are not mapped.
        NexixpayOperationType::Cancel
        | NexixpayOperationType::CardVerification
        | NexixpayOperationType::Unknown => None,
    }
}
