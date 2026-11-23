use crate::connectors::fiuu::transformers::{FiuuPaymentWebhookStatus, FiuuRefundsWebhookStatus};
use domain_types::connector_types::EventType;

impl From<FiuuPaymentWebhookStatus> for EventType {
    fn from(value: FiuuPaymentWebhookStatus) -> Self {
        match value {
            FiuuPaymentWebhookStatus::Success => Self::PaymentIntentSuccess,
            FiuuPaymentWebhookStatus::Failure => Self::PaymentIntentFailure,
            FiuuPaymentWebhookStatus::Pending => Self::PaymentIntentProcessing,
            FiuuPaymentWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu payment webhook status received; acknowledging without processing"
                );
                Self::IncomingWebhookEventUnspecified
            }
        }
    }
}

impl From<FiuuRefundsWebhookStatus> for EventType {
    fn from(value: FiuuRefundsWebhookStatus) -> Self {
        match value {
            FiuuRefundsWebhookStatus::RefundSuccess => Self::RefundSuccess,
            FiuuRefundsWebhookStatus::RefundFailure => Self::RefundFailure,
            FiuuRefundsWebhookStatus::RefundPending => Self::IncomingWebhookEventUnspecified,
            FiuuRefundsWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu refund webhook status received; acknowledging without processing"
                );
                Self::IncomingWebhookEventUnspecified
            }
        }
    }
}
