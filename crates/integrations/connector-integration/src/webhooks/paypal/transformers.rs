use crate::connectors::paypal::transformers::{OutcomeCode, PaypalWebhookEventType};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct PaypalWebooksEventType {
    pub event_type: PaypalWebhookEventType,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct DisputeOutcome {
    pub outcome_code: OutcomeCode,
}

pub fn get_paypal_event_type(
    event: PaypalWebhookEventType,
    outcome: Option<OutcomeCode>,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType as E;
    match event {
        PaypalWebhookEventType::PaymentCaptureCompleted
        | PaypalWebhookEventType::CheckoutOrderCompleted => E::PaymentIntentSuccess,
        PaypalWebhookEventType::PaymentCapturePending
        | PaypalWebhookEventType::CheckoutOrderProcessed => E::PaymentIntentProcessing,
        PaypalWebhookEventType::PaymentCaptureDeclined => E::PaymentIntentFailure,
        PaypalWebhookEventType::PaymentCaptureRefunded => E::RefundSuccess,
        PaypalWebhookEventType::CustomerDisputeCreated => E::DisputeOpened,
        PaypalWebhookEventType::RiskDisputeCreated => E::DisputeAccepted,
        PaypalWebhookEventType::CustomerDisputeResolved => outcome
            .map(|o| match o {
                OutcomeCode::ResolvedBuyerFavour => E::DisputeLost,
                OutcomeCode::ResolvedSellerFavour => E::DisputeWon,
                OutcomeCode::CanceledByBuyer => E::DisputeCancelled,
                OutcomeCode::ACCEPTED => E::DisputeAccepted,
                OutcomeCode::DENIED | OutcomeCode::NONE => E::DisputeCancelled,
                OutcomeCode::ResolvedWithPayout => E::IncomingWebhookEventUnspecified,
            })
            .unwrap_or(E::IncomingWebhookEventUnspecified),
        _ => E::IncomingWebhookEventUnspecified,
    }
}
