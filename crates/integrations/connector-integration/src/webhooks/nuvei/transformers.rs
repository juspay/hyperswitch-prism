use common_enums::DisputeStatus;
use domain_types::{connector_types::EventType, errors::WebhookError};
use error_stack::Report;

use crate::connectors::nuvei::transformers::{
    DmnApiTransactionStatus, DmnStatus, NuveiTransactionType, PaymentDmnNotification,
};

/// A `payout_`-prefixed clientRequestId marks a payout DMN (hyperswitch
/// `has_payout_prefix`); payouts are out of scope and refused.
pub fn is_payout_dmn(notification: &PaymentDmnNotification) -> bool {
    notification
        .client_request_id
        .as_deref()
        .is_some_and(|id| id.starts_with("payout_"))
}

/// Payment/refund DMN -> event (hyperswitch `map_notification_to_event`).
pub fn map_notification_to_event(
    status: &DmnStatus,
    transaction_type: &NuveiTransactionType,
) -> Result<EventType, Report<WebhookError>> {
    match (status, transaction_type) {
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Auth) => {
            Ok(EventType::PaymentIntentAuthorizationSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Sale) => {
            Ok(EventType::PaymentIntentSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Settle) => {
            Ok(EventType::PaymentIntentCaptureSuccess)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Void) => {
            Ok(EventType::PaymentIntentCancelled)
        }
        (DmnStatus::Success | DmnStatus::Approved, NuveiTransactionType::Credit) => {
            Ok(EventType::RefundSuccess)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Auth) => {
            Ok(EventType::PaymentIntentAuthorizationFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Sale) => {
            Ok(EventType::PaymentIntentFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Settle) => {
            Ok(EventType::PaymentIntentCaptureFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Void) => {
            Ok(EventType::PaymentIntentCancelFailure)
        }
        (DmnStatus::Error | DmnStatus::Declined, NuveiTransactionType::Credit) => {
            Ok(EventType::RefundFailure)
        }
        (
            DmnStatus::Pending,
            NuveiTransactionType::Auth | NuveiTransactionType::Sale | NuveiTransactionType::Settle,
        ) => Ok(EventType::PaymentIntentProcessing),
        (
            DmnStatus::Success | DmnStatus::Approved | DmnStatus::Error | DmnStatus::Declined,
            NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        )
        | (
            DmnStatus::Pending,
            NuveiTransactionType::Credit
            | NuveiTransactionType::Void
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        )
        | (
            DmnStatus::Unknown,
            NuveiTransactionType::Auth
            | NuveiTransactionType::Sale
            | NuveiTransactionType::Settle
            | NuveiTransactionType::Void
            | NuveiTransactionType::Credit
            | NuveiTransactionType::Auth3D
            | NuveiTransactionType::InitAuth3D
            | NuveiTransactionType::Unknown,
        ) => Err(WebhookError::WebhookEventTypeNotFound.into()),
    }
}

/// Dispute status -> webhook event.
pub fn dispute_event_from_status(status: DisputeStatus) -> EventType {
    match status {
        DisputeStatus::DisputeOpened => EventType::DisputeOpened,
        DisputeStatus::DisputeExpired => EventType::DisputeExpired,
        DisputeStatus::DisputeAccepted => EventType::DisputeAccepted,
        DisputeStatus::DisputeCancelled => EventType::DisputeCancelled,
        DisputeStatus::DisputeChallenged => EventType::DisputeChallenged,
        DisputeStatus::DisputeWon => EventType::DisputeWon,
        DisputeStatus::DisputeLost => EventType::DisputeLost,
    }
}

impl PaymentDmnNotification {
    pub fn has_conflicting_status(&self) -> bool {
        self.ppp_status == DmnApiTransactionStatus::Fail
            && matches!(self.status, Some(DmnStatus::Approved | DmnStatus::Success))
    }
}
