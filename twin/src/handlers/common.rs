//! Shared helpers used by HTTP handlers + service functions.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;
use crate::types::PaymentIntent;

pub fn intent_not_found() -> Response {
    resource_missing("No such payment_intent")
}

pub fn resource_missing(message: &'static str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "type": "invalid_request_error",
                "code": "resource_missing",
                "message": message
            }
        })),
    )
        .into_response()
}

pub fn load_intent(state: &AppState, id: &str) -> Result<PaymentIntent, Response> {
    state
        .payment_intents
        .get(id)
        .map(|entry| entry.value().0.clone())
        .ok_or_else(intent_not_found)
}
