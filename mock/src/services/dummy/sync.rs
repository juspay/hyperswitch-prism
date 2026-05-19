//! Dummy backend — PSync.

use axum::response::Response;

use crate::handlers::common;
use crate::state::AppState;
use crate::types::PaymentIntent;

pub fn get(state: &AppState, pi_id: &str) -> Result<PaymentIntent, Response> {
    common::load_intent(state, pi_id)
}
