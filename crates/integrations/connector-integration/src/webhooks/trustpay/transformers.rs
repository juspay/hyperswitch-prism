use serde::{Deserialize, Serialize};

use crate::connectors::trustpay::transformers::{
    CreditDebitIndicator, WebhookPaymentInformation, WebhookStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TrustpayWebhookResponse {
    pub payment_information: WebhookPaymentInformation,
    pub signature: String,
}

pub fn get_event_type_from_webhook(
    indicator: &CreditDebitIndicator,
    status: &WebhookStatus,
) -> domain_types::connector_types::EventType {
    match (indicator, status) {
        // Credit (Crdt) = Payment events
        (CreditDebitIndicator::Crdt, WebhookStatus::Paid) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (CreditDebitIndicator::Crdt, WebhookStatus::Rejected) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        // Debit (Dbit) = Refund events
        (CreditDebitIndicator::Dbit, WebhookStatus::Paid)
        | (CreditDebitIndicator::Dbit, WebhookStatus::Refunded) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (CreditDebitIndicator::Dbit, WebhookStatus::Rejected) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        // Chargeback = Dispute event
        (CreditDebitIndicator::Dbit, WebhookStatus::Chargebacked) => {
            domain_types::connector_types::EventType::DisputeLost
        }
        _ => domain_types::connector_types::EventType::IncomingWebhookEventUnspecified,
    }
}
