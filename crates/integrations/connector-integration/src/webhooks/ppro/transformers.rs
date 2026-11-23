use domain_types::{connector_types::EventType, errors::WebhookError};
use serde::{Deserialize, Serialize};

use crate::connectors::ppro::transformers::{PproAgreementStatus, PproFailure, PproPaymentStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PproWebhookType {
    PaymentChargeCreated,
    PaymentChargeAuthenticationPending,
    PaymentChargeProviderConfirmationPending,
    PaymentChargeAuthorizationSucceeded,
    PaymentChargeSuccess,
    PaymentChargeAuthorizationFailed,
    PaymentChargeFailed,
    PaymentChargeDiscarded,
    PaymentChargeCaptureSucceeded,
    PaymentChargeCaptureFailed,
    PaymentChargeVoidSucceeded,
    PaymentChargeVoidFailed,
    PaymentChargeRefundPending,
    PaymentChargeRefundSucceeded,
    PaymentChargeRefundFailed,
    PaymentAgreementActive,
    PaymentAgreementFailed,
    PaymentAgreementRevokedByConsumer,
    PaymentAgreementRevokedByMerchant,
    PaymentAgreementRevokedByProvider,
}

impl TryFrom<PproWebhookType> for EventType {
    type Error = error_stack::Report<WebhookError>;

    fn try_from(event_type: PproWebhookType) -> Result<Self, Self::Error> {
        match event_type {
            PproWebhookType::PaymentChargeCreated
            | PproWebhookType::PaymentChargeAuthenticationPending
            | PproWebhookType::PaymentChargeProviderConfirmationPending => {
                Ok(Self::PaymentIntentProcessing)
            }
            PproWebhookType::PaymentChargeCaptureSucceeded => Ok(Self::PaymentIntentCaptureSuccess),
            PproWebhookType::PaymentChargeFailed
            | PproWebhookType::PaymentChargeAuthorizationFailed
            | PproWebhookType::PaymentChargeDiscarded => Ok(Self::PaymentIntentFailure),
            PproWebhookType::PaymentChargeAuthorizationSucceeded
            | PproWebhookType::PaymentChargeSuccess => Ok(Self::PaymentIntentAuthorizationSuccess),
            PproWebhookType::PaymentChargeRefundPending => Ok(Self::RefundProcessing),
            PproWebhookType::PaymentChargeRefundSucceeded => Ok(Self::RefundSuccess),
            PproWebhookType::PaymentChargeRefundFailed => Ok(Self::RefundFailure),
            PproWebhookType::PaymentChargeVoidSucceeded => Ok(Self::PaymentIntentCancelled),
            PproWebhookType::PaymentChargeVoidFailed => Ok(Self::PaymentIntentCancelFailure),
            PproWebhookType::PaymentChargeCaptureFailed => Ok(Self::PaymentIntentCaptureFailure),
            PproWebhookType::PaymentAgreementActive => Ok(Self::MandateActive),
            PproWebhookType::PaymentAgreementFailed => Ok(Self::MandateFailed),
            PproWebhookType::PaymentAgreementRevokedByConsumer
            | PproWebhookType::PaymentAgreementRevokedByMerchant
            | PproWebhookType::PaymentAgreementRevokedByProvider => Ok(Self::MandateRevoked),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PproWebhookEvent {
    pub specversion: String,
    pub r#type: PproWebhookType,
    pub source: String,
    pub id: String,
    pub time: String,
    pub data: PproWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PproWebhookChargeData {
    pub payment_charge_id: String,
    pub payment_charge_status: PproPaymentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_payment_charge_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PproFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_refund_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PproWebhookAgreementData {
    pub payment_agreement_id: String,
    pub payment_agreement_status: PproAgreementStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<PproFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PproWebhookData {
    Charge(PproWebhookChargeData),
    Agreement(PproWebhookAgreementData),
}
