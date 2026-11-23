use crate::connectors::checkout::transformers::{CheckoutWebhookBody, CheckoutWebhookEventType};

pub fn is_refund_event(event_code: &CheckoutWebhookEventType) -> bool {
    matches!(
        event_code,
        CheckoutWebhookEventType::PaymentRefunded
            | CheckoutWebhookEventType::PaymentRefundDeclined
            | CheckoutWebhookEventType::PaymentRefundPending
    )
}

pub fn is_chargeback_event(event_code: &CheckoutWebhookEventType) -> bool {
    matches!(
        event_code,
        CheckoutWebhookEventType::DisputeReceived
            | CheckoutWebhookEventType::DisputeExpired
            | CheckoutWebhookEventType::DisputeAccepted
            | CheckoutWebhookEventType::DisputeCanceled
            | CheckoutWebhookEventType::DisputeEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeEvidenceAcknowledgedByScheme
            | CheckoutWebhookEventType::DisputeEvidenceRequired
            | CheckoutWebhookEventType::DisputeArbitrationEvidenceSubmitted
            | CheckoutWebhookEventType::DisputeArbitrationSentToScheme
            | CheckoutWebhookEventType::DisputeArbitrationLost
            | CheckoutWebhookEventType::DisputeArbitrationWon
            | CheckoutWebhookEventType::DisputeWon
            | CheckoutWebhookEventType::DisputeLost
    )
}

pub fn is_payout_webhook(body: &CheckoutWebhookBody) -> bool {
    match &body.transaction_type {
        CheckoutWebhookEventType::PayoutPaid => true,
        CheckoutWebhookEventType::PaymentApproved | CheckoutWebhookEventType::PaymentDeclined => {
            body.data.destination.is_some()
        }
        _ => false,
    }
}
