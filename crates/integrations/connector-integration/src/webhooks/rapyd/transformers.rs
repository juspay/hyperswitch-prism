use common_utils::FloatMajorUnit;
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::rapyd::transformers::{
    NextAction, RapydPaymentStatus, RapydResponsePaymentMethodData, RefundResponseData,
};

#[derive(Debug, Deserialize)]
pub struct RapydIncomingWebhook {
    #[serde(rename = "type")]
    pub webhook_type: RapydWebhookEventType,
    #[serde(default)]
    pub data: common_utils::pii::SecretSerdeValue,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RapydWebhookEventType {
    PaymentSucceeded,
    PaymentCompleted,
    PaymentCaptured,
    PaymentFailed,
    PaymentCanceled,
    PaymentExpired,
    RefundCompleted,
    PaymentRefundRejected,
    PaymentRefundFailed,
    PaymentDisputeCreated,
    PaymentDisputeUpdated,
    #[serde(other)]
    Unknown,
}

impl TryFrom<RapydIncomingWebhook> for domain_types::connector_types::EventType {
    type Error = error_stack::Report<domain_types::errors::WebhookError>;

    fn try_from(event: RapydIncomingWebhook) -> Result<Self, Self::Error> {
        Ok(match event.webhook_type {
            RapydWebhookEventType::PaymentSucceeded => {
                let data = event.payment_data()?;
                match (data.status, data.next_action) {
                    (RapydPaymentStatus::Closed, _) => Self::PaymentIntentSuccess,
                    (
                        RapydPaymentStatus::Active,
                        NextAction::PendingCapture | NextAction::NotApplicable,
                    ) => Self::PaymentIntentAuthorizationSuccess,
                    (
                        RapydPaymentStatus::Active,
                        NextAction::ThreedsVerification | NextAction::PendingConfirmation,
                    )
                    | (RapydPaymentStatus::New, _) => Self::PaymentIntentProcessing,
                    (
                        RapydPaymentStatus::CanceledByClientOrBank
                        | RapydPaymentStatus::ReversedByRapyd,
                        _,
                    ) => Self::PaymentIntentCancelled,
                    (RapydPaymentStatus::Expired, _) => Self::PaymentIntentExpired,
                    (RapydPaymentStatus::Error, _) => Self::PaymentIntentFailure,
                }
            }
            RapydWebhookEventType::PaymentCompleted | RapydWebhookEventType::PaymentCaptured => {
                Self::PaymentIntentSuccess
            }
            RapydWebhookEventType::PaymentFailed => Self::PaymentIntentFailure,
            RapydWebhookEventType::PaymentCanceled => Self::PaymentIntentCancelled,
            RapydWebhookEventType::PaymentExpired => Self::PaymentIntentExpired,
            RapydWebhookEventType::RefundCompleted => Self::RefundSuccess,
            RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundFailed => Self::RefundFailure,
            RapydWebhookEventType::PaymentDisputeCreated => Self::DisputeOpened,
            RapydWebhookEventType::PaymentDisputeUpdated => event.dispute_data()?.status.into(),
            RapydWebhookEventType::Unknown => Self::IncomingWebhookEventUnspecified,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RapydWebhookPaymentData {
    pub id: String,
    pub amount: FloatMajorUnit,
    pub currency_code: Option<common_enums::Currency>,
    pub status: RapydPaymentStatus,
    pub next_action: NextAction,
    pub merchant_reference_id: Option<String>,
    pub captured: Option<bool>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub payment_method: Option<Secret<String>>,
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
}

#[derive(Debug, Deserialize)]
pub struct RapydWebhookDisputeData {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub token: String,
    pub dispute_reason_description: Option<String>,
    pub status: RapydWebhookDisputeStatus,
    pub original_transaction_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum RapydWebhookDisputeStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "RVW")]
    Review,
    #[serde(rename = "LOS")]
    Lose,
    #[serde(rename = "WIN")]
    Win,
    #[serde(other)]
    Unknown,
}

impl From<RapydWebhookDisputeStatus> for domain_types::connector_types::EventType {
    fn from(status: RapydWebhookDisputeStatus) -> Self {
        match status {
            RapydWebhookDisputeStatus::Active => Self::DisputeOpened,
            RapydWebhookDisputeStatus::Review => Self::DisputeChallenged,
            RapydWebhookDisputeStatus::Lose => Self::DisputeLost,
            RapydWebhookDisputeStatus::Win => Self::DisputeWon,
            RapydWebhookDisputeStatus::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

impl TryFrom<RapydWebhookDisputeStatus> for common_enums::DisputeStatus {
    type Error = error_stack::Report<domain_types::errors::WebhookError>;

    fn try_from(status: RapydWebhookDisputeStatus) -> Result<Self, Self::Error> {
        Ok(match status {
            RapydWebhookDisputeStatus::Active => Self::DisputeOpened,
            RapydWebhookDisputeStatus::Review => Self::DisputeChallenged,
            RapydWebhookDisputeStatus::Lose => Self::DisputeLost,
            RapydWebhookDisputeStatus::Win => Self::DisputeWon,
            RapydWebhookDisputeStatus::Unknown => {
                return Err(domain_types::errors::WebhookError::WebhookEventTypeNotFound.into());
            }
        })
    }
}

impl RapydIncomingWebhook {
    pub fn dispute_data(
        self,
    ) -> Result<RapydWebhookDisputeData, error_stack::Report<domain_types::errors::WebhookError>>
    {
        use domain_types::errors::WebhookError;
        match self.webhook_type {
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                serde_json::from_value(self.data.expose())
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
            }
            _ => Err(WebhookError::WebhookEventTypeNotFound.into()),
        }
    }

    pub fn refund_data(
        self,
    ) -> Result<RefundResponseData, error_stack::Report<domain_types::errors::WebhookError>> {
        use domain_types::errors::WebhookError;
        match self.webhook_type {
            RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundFailed => {
                serde_json::from_value(self.data.expose())
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
            }
            _ => Err(WebhookError::WebhookEventTypeNotFound.into()),
        }
    }

    pub fn payment_data(
        self,
    ) -> Result<RapydWebhookPaymentData, error_stack::Report<domain_types::errors::WebhookError>>
    {
        use domain_types::errors::WebhookError;
        match self.webhook_type {
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentFailed => serde_json::from_value(self.data.expose())
                .change_context(WebhookError::WebhookBodyDecodingFailed),
            _ => Err(WebhookError::WebhookEventTypeNotFound.into()),
        }
    }
}
