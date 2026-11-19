use common_utils::{ext_traits::ByteSliceExt, pii::Email};
use domain_types::errors::WebhookError;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::connectors::razorpay::transformers::{
    AcquirerData, RazorpayEntity, RazorpayPaymentStatus, RazorpayRefundStatus,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayWebhook {
    pub account_id: String,
    pub contains: Vec<String>,
    pub entity: String,
    pub event: String,
    pub payload: Payload,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Payload {
    pub payment: Option<PaymentWrapper>,
    pub refund: Option<RefundWrapper>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PaymentWrapper {
    pub entity: PaymentEntity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RefundWrapper {
    pub entity: RefundEntity,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PaymentEntity {
    pub id: String,
    pub entity: RazorpayEntity,
    pub amount: i64,
    pub currency: String,
    pub status: RazorpayPaymentStatus,
    pub order_id: String,
    pub invoice_id: Option<String>,
    pub international: bool,
    pub method: String,
    pub amount_refunded: i64,
    pub refund_status: Option<String>,
    pub captured: bool,
    pub description: Option<String>,
    pub card_id: Option<String>,
    pub bank: Option<String>,
    pub wallet: Option<String>,
    pub vpa: Option<Secret<String>>,
    pub email: Option<Email>,
    pub contact: Option<Secret<String>>,
    pub notes: Vec<String>,
    pub fee: Option<i64>,
    pub tax: Option<i64>,
    pub error_code: Option<String>,
    pub error_description: Option<String>,
    pub error_reason: Option<String>,
    pub error_source: Option<String>,
    pub error_step: Option<String>,
    pub acquirer_data: Option<AcquirerData>,
    pub card: Option<RazorpayWebhookCard>,
    pub token_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RefundEntity {
    pub id: String,
    pub entity: RazorpayEntity,
    pub amount: i64,
    pub currency: String,
    pub payment_id: String,
    pub status: RazorpayRefundStatus,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RazorpayWebhookCard {
    pub id: String,
    pub entity: String,
    pub name: String,
    pub last4: String,
    pub network: String,
    #[serde(rename = "type")]
    pub card_type: String,
    pub sub_type: String,
    pub issuer: Option<String>,
    pub international: bool,
    pub iin: String,
    pub emi: bool,
}

pub fn get_webhook_object_from_body(
    body: Vec<u8>,
) -> Result<Payload, error_stack::Report<WebhookError>> {
    let webhook: RazorpayWebhook = body
        .parse_struct("RazorpayWebhook")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    Ok(webhook.payload)
}
