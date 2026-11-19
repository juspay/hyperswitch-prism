use common_enums::AttemptStatus;
use domain_types::connector_types::EventType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct BetterpaymentWebhookBody {
    pub transaction_id: String,
    pub status: BetterpaymentWebhookStatus,
    pub order_id: String,
    pub message: Option<String>,
    pub reason_code: Option<String>,
    // Refund-only fields; absent on payment postbacks.
    pub refund_id: Option<String>,
    #[serde(default)]
    pub refund_status_code: Option<BetterpaymentRefundWebhookStatus>,
    pub refund_reason_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BetterpaymentWebhookStatus {
    Started,
    Pending,
    Completed,
    Declined,
    Canceled,
    Error,
    #[serde(other)]
    Unknown,
}

impl From<BetterpaymentWebhookStatus> for EventType {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::PaymentIntentSuccess,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::PaymentIntentFailure
            }
            BetterpaymentWebhookStatus::Started | BetterpaymentWebhookStatus::Pending => {
                Self::PaymentIntentProcessing
            }
            BetterpaymentWebhookStatus::Canceled => Self::PaymentIntentFailure,
            BetterpaymentWebhookStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl From<BetterpaymentWebhookStatus> for AttemptStatus {
    fn from(status: BetterpaymentWebhookStatus) -> Self {
        match status {
            BetterpaymentWebhookStatus::Completed => Self::Charged,
            BetterpaymentWebhookStatus::Declined | BetterpaymentWebhookStatus::Error => {
                Self::Failure
            }
            BetterpaymentWebhookStatus::Started => Self::AuthenticationPending,
            BetterpaymentWebhookStatus::Pending => Self::Pending,
            BetterpaymentWebhookStatus::Canceled => Self::Failure,
            BetterpaymentWebhookStatus::Unknown => Self::Pending,
        }
    }
}

/// Derives the UCS EventType from a parsed webhook body.
/// Refund postbacks are identified by the presence of `refund_id`.
pub fn get_webhook_event_type(body: &BetterpaymentWebhookBody) -> EventType {
    if body.refund_id.is_some() {
        return get_refund_webhook_event_type(body.refund_status_code);
    }
    EventType::from(body.status.clone())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(from = "u64")]
pub enum BetterpaymentRefundWebhookStatus {
    Started,
    Success,
    Error,
    Local,
    Reversed,
    Declined,
    Unknown,
}

impl From<u64> for BetterpaymentRefundWebhookStatus {
    fn from(code: u64) -> Self {
        match code {
            0 => Self::Started,
            1 => Self::Success,
            2 => Self::Error,
            3 => Self::Local,
            4 => Self::Reversed,
            5 => Self::Declined,
            _ => Self::Unknown,
        }
    }
}

pub fn get_refund_webhook_event_type(
    status: Option<BetterpaymentRefundWebhookStatus>,
) -> EventType {
    match status
        .map(common_enums::RefundStatus::from)
        .unwrap_or_default()
    {
        common_enums::RefundStatus::Success => EventType::RefundSuccess,
        common_enums::RefundStatus::Failure => EventType::RefundFailure,
        _ => EventType::RefundProcessing,
    }
}

impl From<BetterpaymentRefundWebhookStatus> for common_enums::RefundStatus {
    fn from(status: BetterpaymentRefundWebhookStatus) -> Self {
        match status {
            BetterpaymentRefundWebhookStatus::Success => Self::Success,
            BetterpaymentRefundWebhookStatus::Error
            | BetterpaymentRefundWebhookStatus::Reversed
            | BetterpaymentRefundWebhookStatus::Declined => Self::Failure,
            BetterpaymentRefundWebhookStatus::Started
            | BetterpaymentRefundWebhookStatus::Local
            | BetterpaymentRefundWebhookStatus::Unknown => Self::Pending,
        }
    }
}
