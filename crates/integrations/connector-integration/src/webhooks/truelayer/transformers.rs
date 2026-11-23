use crate::connectors::truelayer::transformers::TruelayerWebhookEventType;

pub fn get_webhook_event(
    event: TruelayerWebhookEventType,
) -> domain_types::connector_types::EventType {
    match event {
        TruelayerWebhookEventType::PaymentExecuted
        | TruelayerWebhookEventType::PaymentAuthorized
        | TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentFundsReceived
        | TruelayerWebhookEventType::PaymentSettlementStalled => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        TruelayerWebhookEventType::PaymentSettled => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        TruelayerWebhookEventType::PaymentFailed => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        TruelayerWebhookEventType::RefundExecuted => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        TruelayerWebhookEventType::RefundFailed => {
            domain_types::connector_types::EventType::RefundFailure
        }
        TruelayerWebhookEventType::PaymentReversed => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        TruelayerWebhookEventType::PayoutExecuted => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        TruelayerWebhookEventType::PayoutFailed => {
            domain_types::connector_types::EventType::PayoutFailure
        }
        TruelayerWebhookEventType::PaymentDisputed | TruelayerWebhookEventType::Unknown => {
            domain_types::connector_types::EventType::IncomingWebhookEventUnspecified
        }
    }
}
