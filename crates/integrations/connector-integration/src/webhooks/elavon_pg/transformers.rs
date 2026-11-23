use domain_types::connector_types::EventType;

use crate::connectors::elavon_pg::transformers::{ElavonPgEventType, ElavonPgNotification};

impl ElavonPgNotification {
    pub fn event_type(&self) -> EventType {
        match self.transaction_event() {
            ElavonPgEventType::SaleAuthorized => EventType::PaymentIntentAuthorizationSuccess,
            ElavonPgEventType::SaleCaptured | ElavonPgEventType::SaleSettled => {
                EventType::PaymentIntentSuccess
            }
            ElavonPgEventType::SaleDeclined => EventType::PaymentIntentFailure,
            ElavonPgEventType::SaleHeldForReview => EventType::PaymentIntentProcessing,
            ElavonPgEventType::RefundAuthorized
            | ElavonPgEventType::RefundCaptured
            | ElavonPgEventType::RefundSettled => EventType::RefundSuccess,
            ElavonPgEventType::RefundDeclined => EventType::RefundFailure,
            ElavonPgEventType::Other => EventType::IncomingWebhookEventUnspecified,
        }
    }
}
