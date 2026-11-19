use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PayhereWebhookPayload {
    pub merchant_id: String,
    pub order_id: String,
    pub payment_id: Option<i64>,
    pub payhere_amount: String,
    pub payhere_currency: String,
    pub status_code: i32,
    pub md5sig: String,
}
