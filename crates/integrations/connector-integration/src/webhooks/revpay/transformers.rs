use domain_types::connector_types::EventType;

use crate::connectors::revpay::transformers::{RevpayResponseCode, RevpayWebhookBody};

impl RevpayWebhookBody {
    /// Event interpretation: "00" — the sale settled → success;
    /// "09"/"99" — still in flight at the bank / pending authorization →
    /// processing; every other code → failure.
    pub fn event_type(&self) -> EventType {
        match &self.response_code {
            RevpayResponseCode::Success => EventType::PaymentIntentSuccess,
            RevpayResponseCode::RedirectRequired | RevpayResponseCode::PendingAuthorization => {
                EventType::PaymentIntentProcessing
            }
            RevpayResponseCode::TransactionNotFound | RevpayResponseCode::Unknown(_) => {
                EventType::PaymentIntentFailure
            }
        }
    }
}
