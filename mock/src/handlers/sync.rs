use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::{services, state::AppState};

pub async fn payment_intent(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match services::dummy::sync::get(&state, &id) {
        Ok(pi) => Json(pi).into_response(),
        Err(resp) => resp,
    }
}

pub async fn setup_intent(Path(id): Path<String>) -> impl IntoResponse {
    Json(json!({
        "id": id,
        "object": "setup_intent",
        "status": "succeeded"
    }))
}
