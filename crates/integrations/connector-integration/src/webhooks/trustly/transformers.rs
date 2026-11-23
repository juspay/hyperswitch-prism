use crate::connectors::trustly::transformers::TrustlyWebhookMethod;

pub fn get_webhook_event(
    event: TrustlyWebhookMethod,
    message_id: String,
) -> domain_types::connector_types::EventType {
    match (event, !message_id.as_str().starts_with("payout_")) {
        (TrustlyWebhookMethod::Credit, true) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (TrustlyWebhookMethod::Credit, false) => {
            domain_types::connector_types::EventType::PayoutReversed
        }
        (TrustlyWebhookMethod::Debit, _) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        (TrustlyWebhookMethod::Cancel, true) => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        (TrustlyWebhookMethod::Cancel, false) => {
            domain_types::connector_types::EventType::PayoutCancelled
        }
        (TrustlyWebhookMethod::Account, _) => {
            domain_types::connector_types::EventType::PaymentAssociatedDataUpdate
        }
        (TrustlyWebhookMethod::Pending, _) => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        (TrustlyWebhookMethod::PayoutConfirmation, true) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, true) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        (TrustlyWebhookMethod::PayoutConfirmation, false) => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, false) => {
            domain_types::connector_types::EventType::PayoutFailure
        }
    }
}
