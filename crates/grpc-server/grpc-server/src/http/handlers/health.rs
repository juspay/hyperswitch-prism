use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::http::state::AppState;

pub async fn health(State(_state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "connector-service"
    })))
}
