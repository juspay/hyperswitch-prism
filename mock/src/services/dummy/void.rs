//! Dummy backend — Void.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::handlers::common;
use crate::state::AppState;
use crate::types::{IntentStatus, PaymentIntent};

pub fn cancel(state: &AppState, pi_id: &str) -> Result<PaymentIntent, Response> {
    let Some(mut entry) = state.payment_intents.get_mut(pi_id) else {
        return Err(common::intent_not_found());
    };
    let pi = &mut entry.0;
    let cancellable = pi.status == IntentStatus::RequiresCapture.as_str()
        || pi.status == IntentStatus::RequiresAction.as_str();
    if !cancellable {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "payment_intent_unexpected_state",
                    "message": format!("Cannot cancel in status {}", pi.status)
                }
            })),
        )
            .into_response());
    }
    pi.status = IntentStatus::Canceled.as_str().to_string();
    Ok(pi.clone())
}
