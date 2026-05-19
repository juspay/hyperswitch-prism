use axum::{
    async_trait,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub struct StripeForm<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for StripeForm<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(req.into_body(), 1024 * 1024)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    axum::Json(json!({
                        "error": {
                            "type": "invalid_request_error",
                            "message": format!("body read error: {e}")
                        }
                    })),
                )
                    .into_response()
            })?;
        // Non-strict mode so percent-encoded brackets (%5B / %5D) are treated
        // as nested-key markers — the UCS Dummy Rust connector URL-encodes
        // form keys, so `payment_method_data%5Bcard%5D%5Bnumber%5D` must parse
        // the same as `payment_method_data[card][number]`. DO NOT switch this
        // back to strict — every real connector request will silently 400.
        let parsed: T = serde_qs::Config::new(10, false)
            .deserialize_bytes(&bytes)
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    axum::Json(json!({
                        "error": {
                            "type": "invalid_request_error",
                            "code": "form_parse_error",
                            "message": e.to_string()
                        }
                    })),
                )
                    .into_response()
            })?;
        Ok(Self(parsed))
    }
}
