use serde::{Deserialize, Serialize};

use crate::connectors::cryptopay::transformers::CryptopayPaymentResponseData;

#[derive(Debug, Serialize, Deserialize)]
pub struct CryptopayWebhookDetails {
    #[serde(rename = "type")]
    pub service_type: String,
    pub event: WebhookEvent,
    pub data: CryptopayPaymentResponseData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    TransactionCreated,
    TransactionConfirmed,
    StatusChanged,
}
