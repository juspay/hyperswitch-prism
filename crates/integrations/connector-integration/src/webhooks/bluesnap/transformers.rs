use common_utils::errors::CustomResult;
use domain_types::errors::WebhookError;
use error_stack::ResultExt;

use crate::connectors::bluesnap::transformers::{BluesnapChargebackStatus, BluesnapWebhookEvent};

pub fn map_chargeback_status_to_event_type(
    cb_status: &str,
) -> CustomResult<domain_types::connector_types::EventType, WebhookError> {
    use domain_types::connector_types::EventType;

    let status: BluesnapChargebackStatus =
        serde_json::from_value(serde_json::Value::String(cb_status.to_string()))
            .change_context(WebhookError::WebhookEventTypeNotFound)?;

    Ok(match status {
        BluesnapChargebackStatus::New | BluesnapChargebackStatus::Working => {
            EventType::DisputeOpened
        }
        BluesnapChargebackStatus::Closed => EventType::DisputeExpired,
        BluesnapChargebackStatus::CompletedLost => EventType::DisputeLost,
        BluesnapChargebackStatus::CompletedPending => EventType::DisputeChallenged,
        BluesnapChargebackStatus::CompletedWon => EventType::DisputeWon,
    })
}

pub fn map_webhook_event_to_incoming_webhook_event(
    webhook_event: &BluesnapWebhookEvent,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;

    match webhook_event {
        BluesnapWebhookEvent::Decline | BluesnapWebhookEvent::CcChargeFailed => {
            EventType::PaymentIntentFailure
        }
        BluesnapWebhookEvent::Charge => EventType::PaymentIntentSuccess,
        BluesnapWebhookEvent::Refund => EventType::RefundSuccess,
        BluesnapWebhookEvent::Chargeback | BluesnapWebhookEvent::ChargebackStatusChanged => {
            EventType::DisputeOpened
        }
        BluesnapWebhookEvent::Unknown => EventType::PaymentIntentFailure,
    }
}
