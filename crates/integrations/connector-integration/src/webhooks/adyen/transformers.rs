use common_utils::{ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_types::EventType,
    errors::{IntegrationError, WebhookError},
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, strum::Display, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEventCode {
    Authorisation,
    AuthorisationAdjustment,
    Cancellation,
    Capture,
    CaptureFailed,
    Refund,
    RefundFailed,
    RefundReversed,
    CancelOrRefund,
    NotificationOfChargeback,
    Chargeback,
    ChargebackReversed,
    SecondChargeback,
    PrearbitrationWon,
    PrearbitrationLost,
    RequestForInformation,
    NotificationOfFraud,
    InformationSupplied,
    PrearbitrationOpen,
    PrearbitrationAccepted,
    PrearbitrationDeclined,
    PrearbitrationIssuerWithdrawn,
    SchemeArbitration,
    SchemeArbitrationWon,
    SchemeArbitrationLost,
    DisputeDefensePeriodEnded,
    IssuerResponseTimeframeExpired,
    IssuerComments,
    OfferClosed,
    RecurringContract,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub enum DisputeStatus {
    Undefended,
    Pending,
    Lost,
    Accepted,
    Won,
    Responded,
    Expired,
    Unresponded,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenAdditionalDataWH {
    #[serde(rename = "hmacSignature")]
    pub hmac_signature: Option<String>,
    pub dispute_status: Option<DisputeStatus>,
    pub chargeback_reason_code: Option<String>,
    /// Enable recurring details in Adyen dashboard to receive this ID.
    #[serde(rename = "recurring.recurringDetailReference")]
    pub recurring_detail_reference: Option<Secret<String>>,
    #[serde(rename = "recurring.shopperReference")]
    pub recurring_shopper_reference: Option<String>,
    pub network_tx_reference: Option<Secret<String>>,
    pub refusal_reason_raw: Option<String>,
    pub refusal_code_raw: Option<String>,
    pub shopper_email: Option<Secret<String>>,
    pub shopper_reference: Option<String>,
    pub expiry_date: Option<Secret<String>>,
    pub card_summary: Option<Secret<String>>,
    pub card_issuing_country: Option<String>,
    pub card_issuing_bank: Option<String>,
    pub payment_method_variant: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub defense_period_ends_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenNotificationRequestItemWH {
    pub original_reference: Option<String>,
    pub psp_reference: String,
    pub amount: AdyenAmountWH,
    pub event_code: WebhookEventCode,
    pub merchant_account_code: String,
    pub merchant_reference: String,
    pub success: String,
    pub reason: Option<String>,
    pub additional_data: AdyenAdditionalDataWH,
}

#[derive(Debug, Deserialize)]
pub struct AdyenAmountWH {
    pub value: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AdyenItemObjectWH {
    pub notification_request_item: AdyenNotificationRequestItemWH,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenIncomingWebhook {
    pub notification_items: Vec<AdyenItemObjectWH>,
}

pub fn get_webhook_object_from_body(
    body: Vec<u8>,
) -> Result<AdyenNotificationRequestItemWH, error_stack::Report<IntegrationError>> {
    let mut webhook: AdyenIncomingWebhook = body
        .parse_struct("AdyenIncomingWebhook")
        .change_context(IntegrationError::NotImplemented(
            "webhook body decoding failed".to_string(),
            Default::default(),
        ))?;

    let item_object =
        webhook
            .notification_items
            .drain(..)
            .next()
            .ok_or(IntegrationError::NotImplemented(
                "webhook body decoding failed".to_string(),
                Default::default(),
            ))?;

    Ok(item_object.notification_request_item)
}

fn is_success_scenario(is_success: &str) -> bool {
    is_success == "true"
}

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
