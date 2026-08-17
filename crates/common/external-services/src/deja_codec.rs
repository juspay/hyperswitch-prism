//! Déjà codec + args for the connector-egress HTTP boundary (`call_connector_api`).
//!
//! `Response` cannot derive serde (`http::HeaderMap` is not serde), so a `TapeResponse`
//! mirror carries status + sorted headers + base64 body. The codec captures/reconstructs
//! the boundary's full return type `CustomResult<Result<Response, Response>, ApiClientError>`.
//!
//! **Always-substitute policy:** `reconstruct` returns `Some` for every recorded arm — the
//! success body, the connector-error body, and the client error alike — so replay never
//! re-issues a live connector call. This is deliberately stricter than hyperswitch's
//! Ok-only HTTP policy (a connector regression rig must exercise declines/errors from tape).

use base64::Engine as _;
use common_enums::ApiClientError;
use common_utils::request::Request;
use domain_types::router_response_types::Response;
use error_stack::report;

use crate::service::CustomResult;

/// Serde-able mirror of `Response` (whose `http::HeaderMap` is not serde).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TapeResponse {
    pub status_code: u16,
    /// Sorted (name, value) header pairs. Non-UTF-8 header values are dropped (they never
    /// occur on the connector paths and the existing masking helpers already drop them).
    pub headers: Vec<(String, String)>,
    /// Response body, base64 so arbitrary bytes round-trip losslessly.
    pub body_b64: String,
}

impl TapeResponse {
    fn from_response(response: &Response) -> Self {
        let mut headers: Vec<(String, String)> = response
            .headers
            .as_ref()
            .map(|map| {
                map.iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.as_str().to_owned(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        headers.sort();
        Self {
            status_code: response.status_code,
            headers,
            body_b64: base64::engine::general_purpose::STANDARD.encode(&response.response),
        }
    }

    fn into_response(self) -> Response {
        // `Response.headers` uses reqwest's (http 0.2) HeaderMap, not the http 1.x in scope.
        let mut header_map = reqwest::header::HeaderMap::new();
        for (name, value) in &self.headers {
            if let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::from_bytes(name.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                header_map.insert(name, value);
            }
        }
        let body = base64::engine::general_purpose::STANDARD
            .decode(&self.body_b64)
            .unwrap_or_default();
        Response {
            headers: (!header_map.is_empty()).then_some(header_map),
            response: bytes::Bytes::from(body),
            status_code: self.status_code,
        }
    }
}

/// Tagged tape envelope for the three outcomes of `call_connector_api`.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HttpTapeOutcome {
    /// Outer `Ok`, inner `Ok`: connector returned a success status.
    Ok(TapeResponse),
    /// Outer `Ok`, inner `Err`: connector returned an error status (with a body).
    HttpErr(TapeResponse),
    /// Outer `Err`: the client itself failed (timeout, connection, decode…).
    ClientErr(ApiClientError),
}

/// Codec for `call_connector_api`'s return type. Always-substitute (see module docs).
pub struct HttpOutcomeCodec;

impl deja::codec::ReplayCodec for HttpOutcomeCodec {
    type Value = CustomResult<Result<Response, Response>, ApiClientError>;

    fn capture(value: &Self::Value) -> (serde_json::Value, bool) {
        let (outcome, is_error) = match value {
            Ok(Ok(response)) => (HttpTapeOutcome::Ok(TapeResponse::from_response(response)), false),
            Ok(Err(response)) => (
                HttpTapeOutcome::HttpErr(TapeResponse::from_response(response)),
                false,
            ),
            Err(report) => (
                HttpTapeOutcome::ClientErr(report.current_context().clone()),
                true,
            ),
        };
        (
            serde_json::to_value(&outcome).unwrap_or(serde_json::Value::Null),
            is_error,
        )
    }

    fn reconstruct(recorded: serde_json::Value) -> Option<Self::Value> {
        // Always-substitute: every arm reconstructs; never `None` (which would go live).
        match serde_json::from_value::<HttpTapeOutcome>(recorded).ok()? {
            HttpTapeOutcome::Ok(tape) => Some(Ok(Ok(tape.into_response()))),
            HttpTapeOutcome::HttpErr(tape) => Some(Ok(Err(tape.into_response()))),
            HttpTapeOutcome::ClientErr(error) => Some(Err(report!(error))),
        }
    }
}

/// Build the boundary args (identity) for a connector request: method, url, sorted
/// headers, and a body projection. mTLS material is never captured.
pub fn http_args(request: &Request) -> serde_json::Value {
    use hyperswitch_masking::PeekInterface as _;

    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .map(|(name, value)| {
            let value = match value {
                hyperswitch_masking::Maskable::Masked(secret) => secret.peek().clone(),
                hyperswitch_masking::Maskable::Normal(value) => value.clone(),
            };
            (name.clone(), value)
        })
        .collect();
    headers.sort();

    let body = request
        .body
        .as_ref()
        .map(|body| serde_json::to_value(body).unwrap_or(serde_json::Value::Null));

    serde_json::json!({
        "method": request.method.to_string(),
        "url": request.url,
        "headers": headers,
        "body": body,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use deja::codec::ReplayCodec;

    fn response(status: u16, body: &[u8]) -> Response {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        Response {
            headers: Some(headers),
            response: bytes::Bytes::copy_from_slice(body),
            status_code: status,
        }
    }

    /// Every arm round-trips through capture -> reconstruct (always-substitute), including
    /// the connector-error body and the client error — none reconstruct to `None`.
    #[test]
    fn all_three_arms_round_trip() {
        // Ok(Ok): success body (non-UTF-8 bytes included).
        let ok: <HttpOutcomeCodec as ReplayCodec>::Value = Ok(Ok(response(200, &[0, 159, 146, 150])));
        let (json, is_err) = HttpOutcomeCodec::capture(&ok);
        assert!(!is_err);
        let back = HttpOutcomeCodec::reconstruct(json).expect("ok reconstructs");
        let inner = back.expect("outer ok").expect("inner ok");
        assert_eq!(inner.status_code, 200);
        assert_eq!(inner.response.as_ref(), &[0, 159, 146, 150]);

        // Ok(Err): connector returned an error status with a body.
        let http_err: <HttpOutcomeCodec as ReplayCodec>::Value = Ok(Err(response(503, b"down")));
        let (json, is_err) = HttpOutcomeCodec::capture(&http_err);
        assert!(!is_err);
        let back = HttpOutcomeCodec::reconstruct(json).expect("http_err reconstructs");
        let inner = back.expect("outer ok").expect_err("inner err");
        assert_eq!(inner.status_code, 503);

        // Err: the client itself failed — reconstructs to the same typed context.
        let client_err: <HttpOutcomeCodec as ReplayCodec>::Value =
            Err(report!(ApiClientError::RequestTimeoutReceived));
        let (json, is_err) = HttpOutcomeCodec::capture(&client_err);
        assert!(is_err);
        let back = HttpOutcomeCodec::reconstruct(json).expect("client_err reconstructs");
        assert_eq!(
            *back.expect_err("outer err").current_context(),
            ApiClientError::RequestTimeoutReceived
        );
    }
}

