use crate::connectors::noon::transformers::{NoonPaymentStatus, NoonWebhookEventTypes};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoonWebhookEvent {
    pub order_status: NoonPaymentStatus,
    pub event_type: NoonWebhookEventTypes,
}

/// Maps a signed (`eventType`, `orderStatus`) pair to an event (spec ## Webhook Events).
///
/// Only the pairs the spec documents are mapped; every other pair (3DS, Authenticate, Cancel,
/// Review, Lock, an unknown event, or an undocumented status) is
/// `IncomingWebhookEventUnspecified`, so it can never move a payment or refund to a terminal state.
pub(crate) fn get_noon_webhook_event_type(
    event_type: &NoonWebhookEventTypes,
    order_status: &NoonPaymentStatus,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;
    use NoonPaymentStatus as Status;
    use NoonWebhookEventTypes as Event;

    match event_type {
        Event::Sale => match order_status {
            Status::Captured => EventType::PaymentIntentSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Capture => match order_status {
            Status::Captured => EventType::PaymentIntentSuccess,
            Status::PartiallyCaptured => EventType::PaymentIntentCaptureSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Authorize => match order_status {
            Status::Authorized => EventType::PaymentIntentAuthorizationSuccess,
            Status::Failed => EventType::PaymentIntentAuthorizationFailure,
            Status::Pending | Status::MarkedForReview => EventType::PaymentIntentProcessing,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        // Fail carries the order's resulting status: only a failed order is a payment failure.
        // A failed later operation (e.g. a refund on a CAPTURED order) must not be terminal.
        Event::Fail => match order_status {
            Status::Failed | Status::Rejected | Status::Cancelled | Status::Expired => {
                EventType::PaymentIntentFailure
            }
            Status::Initiated
            | Status::Authorized
            | Status::Captured
            | Status::PartiallyCaptured
            | Status::PartiallyRefunded
            | Status::PaymentInfoAdded
            | Status::ThreeDsEnrollInitiated
            | Status::ThreeDsEnrollChecked
            | Status::ThreeDsResultVerified
            | Status::MarkedForReview
            | Status::Authenticated
            | Status::PartiallyReversed
            | Status::Pending
            | Status::Refunded
            | Status::Reversed
            | Status::Locked
            | Status::Unknown => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Reject => match order_status {
            Status::Rejected => EventType::PaymentIntentFailure,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Reverse => match order_status {
            Status::Reversed => EventType::PaymentIntentCancelled,
            // AUTHORIZED on a Reverse event means the reversal failed (spec ## Webhook Events).
            Status::Authorized => EventType::PaymentIntentCancelFailure,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::Refund => match order_status {
            Status::Refunded | Status::PartiallyRefunded => EventType::RefundSuccess,
            Status::Pending => EventType::RefundProcessing,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        Event::ThreeDs
        | Event::Authenticate
        | Event::Cancel
        | Event::Review
        | Event::Lock
        | Event::Unknown => EventType::IncomingWebhookEventUnspecified,
    }
}
