use domain_types::connector_types::EventType;

use crate::connectors::glomopay::transformers::{
    GlomopayRefundWebhookEventType, GlomopayRefundWebhookPayload, GlomopayWebhookEventType,
    GlomopayWebhookPayload,
};

impl GlomopayWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayWebhookEventType::Success | GlomopayWebhookEventType::FundsAvailable => {
                EventType::PaymentIntentSuccess
            }
            GlomopayWebhookEventType::Failed => EventType::PaymentIntentFailure,
            GlomopayWebhookEventType::InProgress => EventType::PaymentIntentProcessing,
            GlomopayWebhookEventType::ActionRequired => EventType::PaymentActionRequired,
        }
    }
}

impl GlomopayRefundWebhookPayload {
    pub fn get_event_type(&self) -> EventType {
        match self.event_type {
            GlomopayRefundWebhookEventType::Success => EventType::RefundSuccess,
            GlomopayRefundWebhookEventType::Failed => EventType::RefundFailure,
            GlomopayRefundWebhookEventType::ActionRequired => EventType::RefundProcessing,
        }
    }
}
