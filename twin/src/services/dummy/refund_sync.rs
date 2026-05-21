//! Dummy backend — RefundSync.

use axum::response::Response;

use crate::handlers::common;
use crate::state::AppState;
use crate::types::Refund;

pub fn get(state: &AppState, refund_id: &str) -> Result<Refund, Response> {
    state
        .refunds
        .get(refund_id)
        .map(|entry| entry.value().0.clone())
        .ok_or_else(|| common::resource_missing("No such refund"))
}
