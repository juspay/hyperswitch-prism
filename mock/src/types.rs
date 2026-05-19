use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub object: &'static str,
    pub status: String,
    pub amount: i64,
    pub currency: String,
    pub capture_method: String,
    pub client_secret: Option<String>,
    pub next_action: Option<NextAction>,
    pub latest_charge: Option<Charge>,
    pub last_payment_error: Option<StripeErrorObject>,
    pub created: i64,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Charge {
    pub id: String,
    pub object: &'static str,
    pub amount: i64,
    pub amount_captured: i64,
    pub status: String,
    pub paid: bool,
    pub captured: bool,
    pub payment_intent: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NextAction {
    #[serde(rename = "type")]
    pub kind: String,
    pub redirect_to_url: RedirectToUrl,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedirectToUrl {
    pub url: String,
    pub return_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refund {
    pub id: String,
    pub object: &'static str,
    pub amount: i64,
    pub currency: String,
    pub payment_intent: String,
    pub status: String,
    pub created: i64,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StripeErrorObject {
    #[serde(rename = "type")]
    pub kind: String,
    pub code: String,
    pub message: String,
    pub decline_code: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentStatus {
    Succeeded,
    RequiresCapture,
    RequiresAction,
    Failed,
    Canceled,
}

impl IntentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::RequiresCapture => "requires_capture",
            Self::RequiresAction => "requires_action",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }
}
