use common_enums::AttemptStatus;
use domain_types::{
    connector_types::{ResponseId, WebhookDetailsResponse},
    errors::WebhookError,
    utils::is_payment_failure,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AbsaSanlamWebhookEvent {
    Payment(AbsaSanlamPaymentWebhookEvent),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamPaymentWebhookEvent {
    pub event_type: AbsaSanlamWebhookEventType,
    pub payment: AbsaSanlamWebhookPayment,
    pub error: Option<AbsaSanlamWebhookError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamWebhookError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum AbsaSanlamWebhookEventType {
    #[serde(rename = "payment.succeeded")]
    PaymentSucceeded,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "dispute.opened")]
    DisputeOpened,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AbsaSanlamWebhookPayment {
    pub user_reference: String,
    pub status: AbsaSanlamPaymentStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsaSanlamPaymentStatus {
    Success,
    Failure,
    Dispute,
}

impl TryFrom<AbsaSanlamWebhookEvent> for WebhookDetailsResponse {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: AbsaSanlamWebhookEvent) -> Result<Self, Self::Error> {
        match item {
            AbsaSanlamWebhookEvent::Payment(payment_event) => {
                let status = AttemptStatus::try_from(&payment_event.payment.status)?;
                if is_payment_failure(status) {
                    Ok(Self {
                        raw_connector_status: None,
                        amount: None,
                        connector_returned_payment_method_details: None,
                        status,
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            payment_event.payment.user_reference.clone(),
                        )),
                        error_code: payment_event.error.as_ref().and_then(|e| e.code.clone()),
                        error_message: payment_event.error.as_ref().and_then(|e| e.message.clone()),
                        error_reason: payment_event.error.as_ref().and_then(|e| e.reason.clone()),
                        connector_response_reference_id: Some(
                            payment_event.payment.user_reference.clone(),
                        ),
                        connector_request_reference_id: Some(payment_event.payment.user_reference),
                        mandate_reference: None,
                        network_txn_id: None,
                        raw_connector_response: None,
                        response_headers: None,
                        captured_money: None,
                        payment_method_update: None,
                        status_code: 200,
                        sender_payment_instrument_id: None,
                    })
                } else {
                    Ok(Self {
                        raw_connector_status: None,
                        amount: None,
                        connector_returned_payment_method_details: None,
                        status,
                        resource_id: Some(ResponseId::ConnectorTransactionId(
                            payment_event.payment.user_reference.clone(),
                        )),
                        mandate_reference: None,
                        network_txn_id: None,
                        connector_response_reference_id: Some(
                            payment_event.payment.user_reference.clone(),
                        ),
                        connector_request_reference_id: Some(payment_event.payment.user_reference),
                        raw_connector_response: None,
                        response_headers: None,
                        captured_money: None,
                        payment_method_update: None,
                        error_code: None,
                        error_message: None,
                        error_reason: None,
                        status_code: 200,
                        sender_payment_instrument_id: None,
                    })
                }
            }
        }
    }
}

impl TryFrom<&AbsaSanlamPaymentStatus> for AttemptStatus {
    type Error = error_stack::Report<WebhookError>;
    fn try_from(item: &AbsaSanlamPaymentStatus) -> Result<Self, Self::Error> {
        match item {
            AbsaSanlamPaymentStatus::Success => Ok(Self::Charged),
            AbsaSanlamPaymentStatus::Failure => Ok(Self::Failure),
            AbsaSanlamPaymentStatus::Dispute => Err(WebhookError::WebhookResponseEncodingFailed)?,
        }
    }
}
