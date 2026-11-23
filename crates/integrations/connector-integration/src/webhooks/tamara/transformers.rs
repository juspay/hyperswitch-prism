use common_enums::{AttemptStatus, RefundStatus};
use domain_types::connector_types::EventType;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TamaraWebhookEvent {
    OrderApproved,
    OrderAuthorised,
    OrderCanceled,
    OrderCaptured,
    OrderRefunded,
    OrderUpdated,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookData {
    pub capture_id: Option<String>,
    pub refund_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TamaraWebhookDataOrArray {
    Data(TamaraWebhookData),
    // Tamara sometimes sends `data` as an array; contents are intentionally ignored
    #[allow(dead_code)]
    Array(Vec<serde::de::IgnoredAny>),
}

fn deserialize_tamara_webhook_data<'de, D>(
    deserializer: D,
) -> Result<Option<TamaraWebhookData>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<TamaraWebhookDataOrArray>::deserialize(deserializer)? {
        None => Ok(None),
        Some(TamaraWebhookDataOrArray::Data(data)) => Ok(Some(data)),
        Some(TamaraWebhookDataOrArray::Array(_)) => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookEventType {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: TamaraWebhookEvent,
    #[serde(default, deserialize_with = "deserialize_tamara_webhook_data")]
    pub data: Option<TamaraWebhookData>,
}

impl From<TamaraWebhookEvent> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::EventNotSupported,
        }
    }
}

impl From<TamaraWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEventType) -> Self {
        Self::from(event.event_type)
    }
}

impl From<TamaraWebhookEvent> for AttemptStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::Authorized
            }
            TamaraWebhookEvent::OrderCaptured | TamaraWebhookEvent::OrderRefunded => Self::Charged,
            TamaraWebhookEvent::OrderCanceled => Self::Voided,
            TamaraWebhookEvent::OrderUpdated | TamaraWebhookEvent::Unknown => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for RefundStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderRefunded => Self::Success,
            _ => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for EventType {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}
