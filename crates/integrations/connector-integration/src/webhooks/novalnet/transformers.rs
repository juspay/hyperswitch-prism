use crate::connectors::novalnet::transformers::{
    NovalnetAPIStatus, NovalnetTransactionStatus, NovalnetWebhookTransactionData, WebhookEventType,
};
use domain_types::connector_types::EventType;

/// The event class comes from the signed checksum preimage (`event.type` + `result.status`).
///
/// `transaction_status` is unsigned, so it is used under the same lower-only rule as the
/// attempt status: it may narrow a signed `PAYMENT`/`SUCCESS` to the authorization-success or
/// processing variant (which is what an ON_HOLD or PENDING transaction actually is, and what
/// this returned before the flow was reworked), never widen anything. Editing it on a validly
/// signed body can therefore only under-report an event.
pub fn get_incoming_webhook_event(
    event_type: WebhookEventType,
    result_status: NovalnetAPIStatus,
    transaction_status: Option<NovalnetTransactionStatus>,
) -> EventType {
    // Narrow PAYMENT/SUCCESS to match the status HandleEvent reports for the same body.
    if let (WebhookEventType::Payment, NovalnetAPIStatus::Success) = (&event_type, &result_status) {
        match transaction_status {
            Some(NovalnetTransactionStatus::OnHold) => {
                return EventType::PaymentIntentAuthorizationSuccess
            }
            Some(NovalnetTransactionStatus::Pending | NovalnetTransactionStatus::Progress) => {
                return EventType::PaymentIntentProcessing
            }
            _ => {}
        }
    }
    match (event_type, result_status) {
        // spec: ### Webhook Authentication & Signature Verification (undocumented result.status
        // stays non-terminal)
        (_, NovalnetAPIStatus::Unknown) => EventType::IncomingWebhookEventUnspecified,
        // spec: ## Webhook Events (PAYMENT + SUCCESS)
        (WebhookEventType::Payment, NovalnetAPIStatus::Success) => EventType::PaymentIntentSuccess,
        // spec: ## Webhook Events (PAYMENT + FAILURE)
        (WebhookEventType::Payment, NovalnetAPIStatus::Failure) => EventType::PaymentIntentFailure,
        // spec: ## Webhook Events (TRANSACTION_CAPTURE + SUCCESS)
        (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Success) => {
            EventType::PaymentIntentCaptureSuccess
        }
        // spec: ## Webhook Events (TRANSACTION_CAPTURE + FAILURE)
        (WebhookEventType::TransactionCapture, NovalnetAPIStatus::Failure) => {
            EventType::PaymentIntentCaptureFailure
        }
        // spec: ## Webhook Events (TRANSACTION_CANCEL + SUCCESS)
        (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Success) => {
            EventType::PaymentIntentCancelled
        }
        // spec: ## Webhook Events (TRANSACTION_CANCEL + FAILURE)
        (WebhookEventType::TransactionCancel, NovalnetAPIStatus::Failure) => {
            EventType::PaymentIntentCancelFailure
        }
        // spec: ## Webhook Events (TRANSACTION_REFUND + SUCCESS)
        (WebhookEventType::TransactionRefund, NovalnetAPIStatus::Success) => {
            EventType::RefundSuccess
        }
        // spec: ## Webhook Events (TRANSACTION_REFUND + FAILURE)
        (WebhookEventType::TransactionRefund, NovalnetAPIStatus::Failure) => {
            EventType::RefundFailure
        }
        // spec: ## Webhook Events (CHARGEBACK / RETURN_DEBIT / REVERSAL -> dispute opened)
        (
            WebhookEventType::Chargeback
            | WebhookEventType::ReturnDebit
            | WebhookEventType::Reversal,
            NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure,
        ) => EventType::DisputeOpened,
        // spec: ## Webhook Events (CREDIT -> DisputeWon)
        (WebhookEventType::Credit, NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure) => {
            EventType::DisputeWon
        }
        // spec: ## Webhook Events (TRANSACTION_UPDATE, INSTALMENT, ... -> EventNotSupported)
        (WebhookEventType::Unknown, NovalnetAPIStatus::Success | NovalnetAPIStatus::Failure) => {
            EventType::IncomingWebhookEventUnspecified
        }
    }
}

impl NovalnetWebhookTransactionData {
    /// The unsigned `transaction.status`, whichever variant the body decoded as. It is outside
    /// the checksum preimage, so it may only lower an outcome, never raise one.
    pub fn transaction_status(&self) -> Option<NovalnetTransactionStatus> {
        match self {
            Self::SyncTransactionData(data) => Some(data.status),
            Self::CaptureTransactionData(data) => data.status,
            Self::CancelTransactionData(data) => data.status,
            Self::RefundsTransactionData(_) => None,
        }
    }
}
