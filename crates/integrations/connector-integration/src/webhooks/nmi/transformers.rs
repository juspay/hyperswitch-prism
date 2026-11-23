use crate::connectors::nmi::transformers::NmiActionBody;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct NmiWebhookObjectReference {
    pub event_body: NmiReferenceBody,
}

#[derive(Debug, Deserialize)]
pub struct NmiReferenceBody {
    pub order_id: String,
    pub action: NmiActionBody,
}

#[derive(Debug, Deserialize)]
pub struct NmiWebhookEventBody {
    pub event_type: NmiWebhookEventType,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NmiWebhookEventType {
    #[serde(rename = "transaction.sale.success")]
    SaleSuccess,
    #[serde(rename = "transaction.sale.failure")]
    SaleFailure,
    #[serde(rename = "transaction.sale.unknown")]
    SaleUnknown,
    #[serde(rename = "transaction.auth.success")]
    AuthSuccess,
    #[serde(rename = "transaction.auth.failure")]
    AuthFailure,
    #[serde(rename = "transaction.auth.unknown")]
    AuthUnknown,
    #[serde(rename = "transaction.refund.success")]
    RefundSuccess,
    #[serde(rename = "transaction.refund.failure")]
    RefundFailure,
    #[serde(rename = "transaction.refund.unknown")]
    RefundUnknown,
    #[serde(rename = "transaction.void.success")]
    VoidSuccess,
    #[serde(rename = "transaction.void.failure")]
    VoidFailure,
    #[serde(rename = "transaction.void.unknown")]
    VoidUnknown,
    #[serde(rename = "transaction.capture.success")]
    CaptureSuccess,
    #[serde(rename = "transaction.capture.failure")]
    CaptureFailure,
    #[serde(rename = "transaction.capture.unknown")]
    CaptureUnknown,
}

/// Maps the NMI webhook `event_type` to the prism webhook event type.
/// Ports HS `get_nmi_webhook_event` 1:1. The `*.unknown` events map to
/// `IncomingWebhookEventUnspecified` (proto `UNSPECIFIED`), which hyperswitch
/// converts back to `IncomingWebhookEvent::EventNotSupported` — matching the
/// Direct gateway's `EventNotSupported`.
pub(crate) fn get_nmi_webhook_event(
    status: NmiWebhookEventType,
) -> domain_types::connector_types::EventType {
    use domain_types::connector_types::EventType;
    match status {
        NmiWebhookEventType::SaleSuccess => EventType::PaymentIntentSuccess,
        NmiWebhookEventType::SaleFailure => EventType::PaymentIntentFailure,
        NmiWebhookEventType::RefundSuccess => EventType::RefundSuccess,
        NmiWebhookEventType::RefundFailure => EventType::RefundFailure,
        NmiWebhookEventType::VoidSuccess => EventType::PaymentIntentCancelled,
        NmiWebhookEventType::AuthSuccess => EventType::PaymentIntentAuthorizationSuccess,
        NmiWebhookEventType::CaptureSuccess => EventType::PaymentIntentCaptureSuccess,
        NmiWebhookEventType::AuthFailure => EventType::PaymentIntentAuthorizationFailure,
        NmiWebhookEventType::CaptureFailure => EventType::PaymentIntentCaptureFailure,
        NmiWebhookEventType::VoidFailure => EventType::PaymentIntentCancelFailure,
        NmiWebhookEventType::SaleUnknown
        | NmiWebhookEventType::RefundUnknown
        | NmiWebhookEventType::AuthUnknown
        | NmiWebhookEventType::VoidUnknown
        | NmiWebhookEventType::CaptureUnknown => EventType::IncomingWebhookEventUnspecified,
    }
}
