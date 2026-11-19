use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct RevolutWebhookBody {
    pub event: RevolutWebhookEvent,
    pub order_id: String,
    pub merchant_order_ext_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RevolutWebhookEvent {
    OrderCompleted,
    OrderAuthorised,
    OrderCancelled,
    OrderFailed,
    OrderPaymentAuthenticated,
    OrderPaymentDeclined,
    OrderPaymentFailed,
    PayoutInitiated,
    PayoutCompleted,
    PayoutFailed,
    DisputeActionRequired,
    DisputeUnderReview,
    DisputeWon,
    DisputeLost,
}
