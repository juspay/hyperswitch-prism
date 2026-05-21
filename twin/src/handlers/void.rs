use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

use crate::{services, state::AppState};

pub async fn cancel(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match services::dummy::void::cancel(&state, &id) {
        Ok(pi) => Json(pi).into_response(),
        Err(resp) => resp,
    }
}
