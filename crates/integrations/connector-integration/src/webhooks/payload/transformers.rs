use crate::connectors::payload::transformers::PayloadWebhooksTrigger;

// Webhook event transformation
pub fn get_event_type_from_trigger(
    trigger: PayloadWebhooksTrigger,
) -> domain_types::connector_types::EventType {
    match trigger {
        // Payment Success Events
        PayloadWebhooksTrigger::Processed => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        PayloadWebhooksTrigger::Authorized => {
            domain_types::connector_types::EventType::PaymentIntentAuthorizationSuccess
        }
        // Payment Processing Events
        PayloadWebhooksTrigger::Payment | PayloadWebhooksTrigger::AutomaticPayment => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        // Payment Failure Events
        PayloadWebhooksTrigger::Decline
        | PayloadWebhooksTrigger::Reject
        | PayloadWebhooksTrigger::BankAccountReject => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        PayloadWebhooksTrigger::Void | PayloadWebhooksTrigger::Reversal => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        // Refund Events
        PayloadWebhooksTrigger::Refund => domain_types::connector_types::EventType::RefundSuccess,
        // Dispute Events
        PayloadWebhooksTrigger::Chargeback => {
            domain_types::connector_types::EventType::DisputeOpened
        }
        PayloadWebhooksTrigger::ChargebackReversal => {
            domain_types::connector_types::EventType::DisputeWon
        }
        // Other payment-related events - treat as generic payment processing
        PayloadWebhooksTrigger::PaymentActivationStatus
        | PayloadWebhooksTrigger::Credit
        | PayloadWebhooksTrigger::Deposit
        | PayloadWebhooksTrigger::PaymentLinkStatus
        | PayloadWebhooksTrigger::ProcessingStatus
        | PayloadWebhooksTrigger::TransactionOperation
        | PayloadWebhooksTrigger::TransactionOperationClear => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
    }
}
