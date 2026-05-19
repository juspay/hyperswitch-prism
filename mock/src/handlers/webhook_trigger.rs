use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TriggerReq {
    pub target_url: String,
    pub payment_intent_id: String,
    pub event_type: String,
}

pub async fn trigger(
    State(state): State<AppState>,
    Json(req): Json<TriggerReq>,
) -> impl IntoResponse {
    let pi = match state.payment_intents.get(&req.payment_intent_id) {
        Some(entry) => entry.value().0.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "type": "invalid_request_error",
                        "code": "resource_missing",
                        "message": "No such payment_intent"
                    }
                })),
            )
                .into_response();
        }
    };

    let event_id = format!("evt_{}", Uuid::new_v4().simple());
    let event = json!({
        "id": event_id,
        "object": "event",
        "type": req.event_type,
        "created": chrono::Utc::now().timestamp(),
        "data": { "object": pi },
        "livemode": false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    match client.post(&req.target_url).json(&event).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            (
                StatusCode::OK,
                Json(json!({
                    "delivered_to": req.target_url,
                    "status": status,
                    "event_id": event_id
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": {
                    "type": "api_error",
                    "code": "webhook_delivery_failed",
                    "message": e.to_string()
                }
            })),
        )
            .into_response(),
    }
}
