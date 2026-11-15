use domain_types::{connector_types::EventType, errors::WebhookError};

use crate::common::adyen::{is_success_scenario, DisputeStatus, WebhookEventCode};

pub(crate) fn get_adyen_webhook_event_type(
    code: WebhookEventCode,
    is_success: String,
    dispute_status: Option<DisputeStatus>,
) -> Result<EventType, WebhookError> {
    match code {
        // Adyen sends the same AUTHORISATION eventCode for both success and
        // failure, distinguished only by `success` -- mirrors
        // get_adyen_payment_webhook_event's Authorized/Failure split below.
        // Most Adyen integrations auto-capture, so a successful AUTHORISATION
        // is the final settlement signal, not just an intermediate auth step.
        WebhookEventCode::Authorisation | WebhookEventCode::RecurringContract => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentSuccess)
            } else {
                Ok(EventType::PaymentIntentFailure)
            }
        }
        WebhookEventCode::AuthorisationAdjustment => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentExtendAuthorizationSuccess)
            } else {
                Ok(EventType::PaymentIntentExtendAuthorizationFailure)
            }
        }
        WebhookEventCode::Cancellation => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentCancelled)
            } else {
                Ok(EventType::PaymentIntentCancelFailure)
            }
        }
        WebhookEventCode::Capture => {
            if is_success_scenario(&is_success) {
                Ok(EventType::PaymentIntentCaptureSuccess)
            } else {
                Ok(EventType::PaymentIntentCaptureFailure)
            }
        }
        WebhookEventCode::CaptureFailed => Ok(EventType::PaymentIntentCaptureFailure),
        WebhookEventCode::OfferClosed => Ok(EventType::PaymentIntentExpired),
        WebhookEventCode::Refund | WebhookEventCode::CancelOrRefund => {
            if is_success_scenario(&is_success) {
                Ok(EventType::RefundSuccess)
            } else {
                Ok(EventType::RefundFailure)
            }
        }
        WebhookEventCode::RefundFailed => Ok(EventType::RefundFailure),
        WebhookEventCode::RefundReversed => Ok(EventType::RefundReview),
        WebhookEventCode::NotificationOfChargeback => Ok(EventType::DisputeOpened),
        WebhookEventCode::Chargeback => match dispute_status {
            Some(DisputeStatus::Won) => Ok(EventType::DisputeWon),
            Some(DisputeStatus::Lost) | None => Ok(EventType::DisputeLost),
            Some(DisputeStatus::Accepted) => Ok(EventType::DisputeAccepted),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::RequestForInformation => match dispute_status {
            Some(DisputeStatus::Expired) => Ok(EventType::DisputeExpired),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::InformationSupplied => match dispute_status {
            Some(DisputeStatus::Responded) => Ok(EventType::DisputeChallenged),
            _ => Ok(EventType::DisputeOpened),
        },
        WebhookEventCode::ChargebackReversed => match dispute_status {
            Some(DisputeStatus::Pending) => Ok(EventType::DisputeChallenged),
            _ => Ok(EventType::DisputeWon),
        },
        WebhookEventCode::PrearbitrationWon => Ok(EventType::DisputeWon),
        WebhookEventCode::SecondChargeback | WebhookEventCode::PrearbitrationLost => {
            Ok(EventType::DisputeLost)
        }
        WebhookEventCode::PrearbitrationOpen | WebhookEventCode::SchemeArbitration => {
            Ok(EventType::DisputeOpened)
        }
        WebhookEventCode::PrearbitrationAccepted => Ok(EventType::DisputeAccepted),
        WebhookEventCode::PrearbitrationDeclined => Ok(EventType::DisputeChallenged),
        WebhookEventCode::PrearbitrationIssuerWithdrawn
        | WebhookEventCode::SchemeArbitrationWon
        | WebhookEventCode::IssuerResponseTimeframeExpired => Ok(EventType::DisputeWon),
        WebhookEventCode::SchemeArbitrationLost => Ok(EventType::DisputeLost),
        WebhookEventCode::DisputeDefensePeriodEnded => match dispute_status {
            Some(DisputeStatus::Accepted) => Ok(EventType::DisputeAccepted),
            _ => Ok(EventType::DisputeLost),
        },
        WebhookEventCode::NotificationOfFraud | WebhookEventCode::IssuerComments => {
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
        WebhookEventCode::Unknown => {
            tracing::warn!(
                "Received unknown Adyen webhook event code; acknowledging without processing"
            );
            Ok(EventType::IncomingWebhookEventUnspecified)
        }
    }
}
