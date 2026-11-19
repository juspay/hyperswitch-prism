use crate::connectors::grabpay::transformers::{GrabpayPaymentStatus, GrabpayRefundStatus};
use common_enums::{AttemptStatus, Currency, RefundStatus};
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RefundWebhookReference, WebhookResourceReference,
    },
    errors,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayWebhookBody {
    #[serde(rename = "txType")]
    pub tx_type: Option<String>,
    #[serde(rename = "txCategory")]
    pub tx_category: Option<String>,
    #[serde(rename = "txStatus")]
    pub tx_status: Option<String>,
    #[serde(rename = "partnerTxID")]
    pub partner_tx_id: Option<String>,
    #[serde(rename = "txID")]
    pub tx_id: Option<String>,
    #[serde(rename = "origTxID")]
    pub orig_tx_id: Option<String>,
    pub amount: Option<serde_json::Value>,
    pub currency: Option<Currency>,
    pub status: Option<String>,
    pub reason: Option<String>,
    pub payload: Option<GrabpayWebhookPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabpayWebhookPayload {
    #[serde(rename = "partnerGroupTxID")]
    pub partner_group_tx_id: Option<String>,
    #[serde(rename = "newStatus")]
    pub new_status: Option<String>,
    pub reason: Option<String>,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    pub echo: Option<String>,
}

impl GrabpayWebhookBody {
    pub fn is_refund_event(&self) -> bool {
        let tx_type_is_refund = self
            .tx_type
            .as_deref()
            .map(|tx_type| tx_type.to_ascii_lowercase().contains("refund"))
            .unwrap_or(false);
        let tx_category_is_refund = self
            .tx_category
            .as_deref()
            .map(|tx_category| tx_category.to_ascii_lowercase().contains("refund"))
            .unwrap_or(false);

        tx_type_is_refund || tx_category_is_refund
    }

    pub fn effective_status(&self) -> Option<&str> {
        self.payload
            .as_ref()
            .and_then(|payload| payload.new_status.as_deref())
            .or(self.tx_status.as_deref())
            .or(self.status.as_deref())
    }

    pub fn effective_reason(&self) -> Option<String> {
        self.payload
            .as_ref()
            .and_then(|payload| payload.reason.clone())
            .or_else(|| self.reason.clone())
    }

    pub fn webhook_reference(&self) -> WebhookResourceReference {
        if self.is_refund_event() {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: self.tx_id.clone(),
                merchant_refund_id: self.partner_tx_id.clone(),
                connector_transaction_id: self.orig_tx_id.clone(),
                merchant_transaction_id: self
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.partner_group_tx_id.clone()),
            })
        } else {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: self.tx_id.clone(),
                merchant_transaction_id: self.partner_tx_id.clone(),
                connector_preprocessing_id: None,
            })
        }
    }
}

pub fn grabpay_webhook_event_type(
    webhook_body: &GrabpayWebhookBody,
) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
    if webhook_body.is_refund_event() {
        let status = parse_webhook_status::<GrabpayRefundStatus>(webhook_body)?;
        Ok(match status {
            GrabpayRefundStatus::Success => EventType::RefundSuccess,
            GrabpayRefundStatus::Failed
            | GrabpayRefundStatus::Cancelled
            | GrabpayRefundStatus::AuthorisationDeclined => EventType::RefundFailure,
            GrabpayRefundStatus::Processing | GrabpayRefundStatus::TransactionAlreadyExist => {
                EventType::RefundProcessing
            }
            GrabpayRefundStatus::Unknown(_) => EventType::IncomingWebhookEventUnspecified,
        })
    } else {
        let status = parse_webhook_status::<GrabpayPaymentStatus>(webhook_body)?;
        Ok(match status {
            GrabpayPaymentStatus::Success => EventType::PaymentIntentSuccess,
            GrabpayPaymentStatus::Failed
            | GrabpayPaymentStatus::Cancelled
            | GrabpayPaymentStatus::AuthorisationDeclined => EventType::PaymentIntentFailure,
            GrabpayPaymentStatus::Processing | GrabpayPaymentStatus::TransactionAlreadyExist => {
                EventType::PaymentIntentProcessing
            }
            GrabpayPaymentStatus::Authorised => EventType::PaymentIntentAuthorizationSuccess,
            GrabpayPaymentStatus::Unknown(_) => EventType::IncomingWebhookEventUnspecified,
        })
    }
}

pub fn grabpay_webhook_attempt_status(
    webhook_body: &GrabpayWebhookBody,
) -> Result<AttemptStatus, error_stack::Report<errors::WebhookError>> {
    let status = parse_webhook_status::<GrabpayPaymentStatus>(webhook_body)?;
    Ok(AttemptStatus::from(status))
}

pub fn grabpay_webhook_refund_status(
    webhook_body: &GrabpayWebhookBody,
) -> Result<RefundStatus, error_stack::Report<errors::WebhookError>> {
    let status = parse_webhook_status::<GrabpayRefundStatus>(webhook_body)?;
    Ok(RefundStatus::from(status))
}

fn parse_webhook_status<T>(
    webhook_body: &GrabpayWebhookBody,
) -> Result<T, error_stack::Report<errors::WebhookError>>
where
    T: std::str::FromStr,
{
    let status = webhook_body
        .effective_status()
        .ok_or_else(|| error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed))?;
    status
        .trim()
        .parse::<T>()
        .map_err(|_| error_stack::report!(errors::WebhookError::WebhookBodyDecodingFailed))
}
