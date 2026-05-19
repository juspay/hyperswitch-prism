use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;

use crate::{form::StripeForm, services, state::AppState};

#[derive(Debug, Deserialize)]
pub struct RefundReq {
    pub payment_intent: String,
    #[serde(default)]
    pub amount: Option<i64>,
}

pub async fn create(
    State(state): State<AppState>,
    StripeForm(req): StripeForm<RefundReq>,
) -> impl IntoResponse {
    match services::dummy::refund::create(&state, &req) {
        Ok(refund) => Json(refund).into_response(),
        Err(resp) => resp,
    }
}
