use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;

pub async fn bearer_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let token = auth
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let unauthorized = || {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "authentication_required",
                    "message": "Missing or invalid Authorization header. Expected 'Bearer <token>'."
                }
            })),
        )
            .into_response()
    };

    let Some(tok) = token else {
        return unauthorized();
    };

    if tok.len() >= 8 && tok[..8].eq_ignore_ascii_case("sk_live_") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "live_keys_not_supported",
                    "message": "Live-mode API keys are rejected by the mock."
                }
            })),
        )
            .into_response();
    }

    if let Some(required) = state.require_token.as_deref() {
        if tok != required {
            return unauthorized();
        }
    }

    let prefix: String = tok.chars().take(10).collect();
    tracing::debug!(token_prefix = %prefix, "auth ok");
    next.run(request).await
}
