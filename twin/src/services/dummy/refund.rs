//! Dummy backend — Refund (create).

use std::time::Instant;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::handlers::common;
use crate::handlers::refund::RefundReq;
use crate::state::AppState;
use crate::types::{IntentStatus, Refund};

pub fn create(state: &AppState, req: &RefundReq) -> Result<Refund, Response> {
    let Some(entry) = state.payment_intents.get(&req.payment_intent) else {
        return Err(common::intent_not_found());
    };
    let pi = &entry.0;
    if pi.status != IntentStatus::Succeeded.as_str() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "charge_not_refundable",
                    "message": format!("Cannot refund payment in status {}", pi.status)
                }
            })),
        )
            .into_response());
    }
    let amount = req.amount.unwrap_or(pi.amount);
    let r = Refund {
        id: format!("re_{}", Uuid::new_v4().simple()),
        object: "refund",
        amount,
        currency: pi.currency.clone(),
        payment_intent: req.payment_intent.clone(),
        status: IntentStatus::Succeeded.as_str().to_string(),
        created: chrono::Utc::now().timestamp(),
        metadata: serde_json::json!({}),
    };
    state
        .refunds
        .insert(r.id.clone(), (r.clone(), Instant::now()));
    Ok(r)
}
