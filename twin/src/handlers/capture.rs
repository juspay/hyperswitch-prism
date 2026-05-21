use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::{form::StripeForm, handlers::common, services, state::AppState};

#[derive(Debug, Deserialize)]
pub struct CaptureReq {
    #[serde(default)]
    pub amount_to_capture: Option<i64>,
}

pub async fn capture(
    Path(id): Path<String>,
    State(state): State<AppState>,
    StripeForm(req): StripeForm<CaptureReq>,
) -> impl IntoResponse {
    match services::dummy::capture::capture(&state, &id, &req) {
        Ok(pi) => Json(pi).into_response(),
        Err(resp) => resp,
    }
}

pub async fn increment_authorization(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // v1 echo stub: return the PI unchanged regardless of state.
    match common::load_intent(&state, &id) {
        Ok(pi) => Json(pi).into_response(),
        Err(resp) => resp,
    }
}
