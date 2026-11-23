use crate::connectors::givepayments::transformers::{
    GivepaymentsPaymentProcessingState, GivepaymentsPaymentResponseData,
    GivepaymentsRefundProcessingState, GivepaymentsRefundResponseData,
};
use common_enums::{self, AttemptStatus, RefundStatus};
use domain_types::connector_types::EventType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GivepaymentsIncomingWebhookData {
    id: String,
    #[serde(rename = "type")]
    pub event_type: GivepaymentsWebhookEventType,
    pub data: GivepaymentsWebhookData,
    merchant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GivepaymentsWebhookEventType {
    #[serde(rename = "payment.created")]
    PaymentCreated,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "payment.voided")]
    PaymentVoided,
    #[serde(rename = "payment.declined")]
    PaymentDeclined,
    #[serde(rename = "payment.authorized")]
    PaymentAuthorized,
    #[serde(rename = "payment.captured")]
    PaymentCaptured,
    #[serde(rename = "payment.settled")]
    PaymentSettled,
    #[serde(rename = "refund.created")]
    RefundCreated,
    #[serde(rename = "refund.pending")]
    RefundPending,
    #[serde(rename = "refund.failed")]
    RefundFailed,
    #[serde(rename = "refund.declined")]
    RefundDeclined,
    #[serde(rename = "refund.canceled")]
    RefundCanceled,
    #[serde(rename = "refund.approved")]
    RefundApproved,
    #[serde(rename = "refund.settled")]
    RefundSettled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "object", rename_all = "snake_case")]
pub enum GivepaymentsWebhookData {
    Payment(GivepaymentsPaymentResponseData),
    Refund(GivepaymentsRefundResponseData),
}

impl From<GivepaymentsPaymentProcessingState> for AttemptStatus {
    fn from(item: GivepaymentsPaymentProcessingState) -> Self {
        match item {
            GivepaymentsPaymentProcessingState::Created
            | GivepaymentsPaymentProcessingState::Authorized => Self::Pending,

            GivepaymentsPaymentProcessingState::Voided => Self::Voided,

            GivepaymentsPaymentProcessingState::Captured
            | GivepaymentsPaymentProcessingState::Settled => Self::Charged,

            GivepaymentsPaymentProcessingState::Failed
            | GivepaymentsPaymentProcessingState::Declined => Self::Failure,
        }
    }
}

impl From<GivepaymentsRefundProcessingState> for RefundStatus {
    fn from(status: GivepaymentsRefundProcessingState) -> Self {
        match status {
            GivepaymentsRefundProcessingState::Created
            | GivepaymentsRefundProcessingState::Pending => Self::Pending,

            GivepaymentsRefundProcessingState::Approved
            | GivepaymentsRefundProcessingState::Settled => Self::Success,

            GivepaymentsRefundProcessingState::Failed
            | GivepaymentsRefundProcessingState::Declined
            | GivepaymentsRefundProcessingState::Canceled => Self::Failure,
        }
    }
}

impl From<GivepaymentsWebhookEventType> for EventType {
    fn from(event_type: GivepaymentsWebhookEventType) -> Self {
        match event_type {
            GivepaymentsWebhookEventType::PaymentCreated
            | GivepaymentsWebhookEventType::PaymentAuthorized => Self::PaymentIntentProcessing,

            GivepaymentsWebhookEventType::PaymentFailed
            | GivepaymentsWebhookEventType::PaymentDeclined => Self::PaymentIntentCaptureFailure,

            GivepaymentsWebhookEventType::PaymentVoided => Self::PaymentIntentCancelled,

            GivepaymentsWebhookEventType::PaymentCaptured
            | GivepaymentsWebhookEventType::PaymentSettled => Self::PaymentIntentCaptureSuccess,

            GivepaymentsWebhookEventType::RefundCreated
            | GivepaymentsWebhookEventType::RefundPending => Self::RefundProcessing,

            GivepaymentsWebhookEventType::RefundFailed
            | GivepaymentsWebhookEventType::RefundDeclined
            | GivepaymentsWebhookEventType::RefundCanceled => Self::RefundFailure,

            GivepaymentsWebhookEventType::RefundApproved
            | GivepaymentsWebhookEventType::RefundSettled => Self::RefundSuccess,

            GivepaymentsWebhookEventType::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}
