use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

use crate::{services, state::AppState};

pub async fn get(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match services::dummy::refund_sync::get(&state, &id) {
        Ok(refund) => Json(refund).into_response(),
        Err(resp) => resp,
    }
}
