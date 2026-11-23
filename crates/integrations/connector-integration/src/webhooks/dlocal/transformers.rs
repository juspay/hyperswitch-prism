use serde::{Deserialize, Serialize};

use crate::connectors::dlocal::transformers::{DlocalEnrollment, DlocalPaymentStatus};

/// dLocal IPN (Instant Payment Notification) webhook body.
///
/// The IPN payload is the dLocal Payment or Enrollment object as JSON. We only
/// deserialize the fields needed to build the PSync-shaped status response and
/// persist ACTIVE enrollment ids; every other field is ignored.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DlocalWebhookBody {
    /// Connector resource id (payment `F-...` or enrollment `E-...`).
    pub id: String,
    pub status: DlocalPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<serde_json::Value>,
    /// Merchant-assigned reference echoed back by dLocal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    /// Enrollment API merchant-assigned reference echoed back by dLocal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrollment: Option<DlocalEnrollment>,
}

impl DlocalWebhookBody {
    pub fn is_enrollment_webhook(&self) -> bool {
        self.order_id.is_none() && self.external_id.is_some()
    }

    pub fn enrollment_id_from_successful_payment(&self) -> Option<String> {
        (!self.is_enrollment_webhook() && matches!(&self.status, DlocalPaymentStatus::Paid))
            .then(|| {
                self.enrollment
                    .as_ref()
                    .and_then(|enrollment| enrollment.id.clone())
            })
            .flatten()
    }
}

/// Maps a parsed dLocal IPN status to the UCS webhook `EventType`.
///
/// Mirrors the `DlocalPaymentStatus -> AttemptStatus` mapping used by the sync
/// flows, then projects each terminal/intermediate status onto the payment-intent
/// webhook event the EventService expects.
impl From<&DlocalWebhookBody> for domain_types::connector_types::EventType {
    fn from(body: &DlocalWebhookBody) -> Self {
        match &body.status {
            DlocalPaymentStatus::Active if body.is_enrollment_webhook() => {
                Self::PaymentIntentSuccess
            }
            // PAID(200) / VERIFIED(700) -> success.
            DlocalPaymentStatus::Paid | DlocalPaymentStatus::Verified => Self::PaymentIntentSuccess,
            // AUTHORIZED -> funds held, capture pending
            DlocalPaymentStatus::Authorized => Self::PaymentIntentAuthorizationSuccess,
            // PENDING(100) -> still processing
            DlocalPaymentStatus::Pending | DlocalPaymentStatus::Active => {
                Self::PaymentIntentProcessing
            }
            // CANCELLED -> voided
            DlocalPaymentStatus::Cancelled => Self::PaymentIntentCancelled,
            // REJECTED(300) -> failure
            DlocalPaymentStatus::Rejected => Self::PaymentIntentFailure,
        }
    }
}
