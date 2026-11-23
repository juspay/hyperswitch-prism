use domain_types::{connector_types::EventType, errors};

use crate::connectors::imerchantsolutions::transformers::{
    ForeignTryFrom, ImerchantsolutionsWebhookEventType, ImerchantsolutionsWebhookStatus,
};

impl
    ForeignTryFrom<(
        ImerchantsolutionsWebhookEventType,
        ImerchantsolutionsWebhookStatus,
    )> for EventType
{
    type Error = error_stack::Report<errors::WebhookError>;

    fn foreign_try_from(
        (event_type, status): (
            ImerchantsolutionsWebhookEventType,
            ImerchantsolutionsWebhookStatus,
        ),
    ) -> Result<Self, Self::Error> {
        match event_type {
            ImerchantsolutionsWebhookEventType::PaymentCompleted => match status {
                ImerchantsolutionsWebhookStatus::Authorized => {
                    Ok(Self::PaymentIntentAuthorizationSuccess)
                }
                ImerchantsolutionsWebhookStatus::PartiallyCaptured => {
                    Ok(Self::PaymentIntentPartiallyFunded)
                }
                ImerchantsolutionsWebhookStatus::Captured => Ok(Self::PaymentIntentCaptureSuccess),
                ImerchantsolutionsWebhookStatus::Cancelled
                | ImerchantsolutionsWebhookStatus::Failed
                | ImerchantsolutionsWebhookStatus::Refused
                | ImerchantsolutionsWebhookStatus::PartiallyRefunded
                | ImerchantsolutionsWebhookStatus::Refunded => {
                    Err(errors::WebhookError::WebhookBodyDecodingFailed.into())
                }
            },
            ImerchantsolutionsWebhookEventType::PaymentCancelled => {
                Ok(Self::PaymentIntentCancelled)
            }
            ImerchantsolutionsWebhookEventType::PaymentFailed => Ok(Self::PaymentIntentFailure),
            ImerchantsolutionsWebhookEventType::PaymentRefunded => Ok(Self::RefundSuccess),
        }
    }
}
