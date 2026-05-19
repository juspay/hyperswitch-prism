//! Dummy backend — Capture.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::handlers::capture::CaptureReq;
use crate::handlers::common;
use crate::state::AppState;
use crate::types::{IntentStatus, PaymentIntent};

pub fn capture(
    state: &AppState,
    pi_id: &str,
    req: &CaptureReq,
) -> Result<PaymentIntent, Response> {
    let Some(mut entry) = state.payment_intents.get_mut(pi_id) else {
        return Err(common::intent_not_found());
    };
    let pi = &mut entry.0;
    if pi.status != IntentStatus::RequiresCapture.as_str() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "payment_intent_unexpected_state",
                    "message": format!("Cannot capture in status {}", pi.status)
                }
            })),
        )
            .into_response());
    }
    pi.status = IntentStatus::Succeeded.as_str().to_string();
    let captured = req.amount_to_capture.unwrap_or(pi.amount);
    if let Some(ch) = pi.latest_charge.as_mut() {
        ch.status = IntentStatus::Succeeded.as_str().to_string();
        ch.captured = true;
        ch.paid = true;
        ch.amount_captured = captured;
    }
    Ok(pi.clone())
}
