use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{form::StripeForm, services, state::AppState};

#[derive(Debug, Deserialize)]
pub struct AuthorizeReq {
    pub amount: i64,
    pub currency: String,
    #[serde(default)]
    pub capture_method: Option<String>,
    pub payment_method_data: Option<PaymentMethodData>,
    #[serde(default)]
    pub return_url: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct PaymentMethodData {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub card: Option<CardData>,
    pub upi: Option<UpiData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Card body schema mirrors Stripe; only `number` is read for scenario dispatch.
pub struct CardData {
    pub number: String,
    #[serde(default)]
    pub exp_month: Option<String>,
    #[serde(default)]
    pub exp_year: Option<String>,
    #[serde(default)]
    pub cvc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpiData {
    #[serde(alias = "vpa_id", alias = "vpa")]
    pub vpa: String,
}

pub async fn create(
    State(state): State<AppState>,
    StripeForm(req): StripeForm<AuthorizeReq>,
) -> impl IntoResponse {
    let pi = services::dummy::authorize::create(&state, &req);
    (StatusCode::OK, Json(pi)).into_response()
}

// v1-compatibility stubs — UCS connector calls these but mock-dummy doesn't
// model them in any depth. Returning generic-success JSON keeps the harness
// happy without growing the scenarios surface.

pub async fn create_setup() -> impl IntoResponse {
    let id = format!("set_{}", Uuid::new_v4().simple());
    Json(serde_json::json!({
        "id": id,
        "object": "setup_intent",
        "status": "succeeded",
        "client_secret": format!("{id}_secret_x"),
        "created": chrono::Utc::now().timestamp()
    }))
}

pub async fn create_payment_method() -> impl IntoResponse {
    Json(serde_json::json!({
        "id": format!("pm_{}", Uuid::new_v4().simple()),
        "object": "payment_method"
    }))
}

pub async fn create_customer() -> impl IntoResponse {
    Json(serde_json::json!({
        "id": format!("cus_{}", Uuid::new_v4().simple()),
        "object": "customer"
    }))
}
