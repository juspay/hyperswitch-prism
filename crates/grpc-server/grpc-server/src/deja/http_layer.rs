//! The HTTP-mode ingress record boundary: an axum/tower layer that captures each
//! request/response as one `http_incoming` event — **byte-compatible with hyperswitch's
//! shape**, so déjà's existing HTTP replay driver (`deja-kernel`) can reconstruct and
//! drive prism tapes unchanged.
//!
//! Inert until a boot hook is installed; passthrough for anything that isn't a business
//! request (only `POST`s are recordable; `GET /health` and friends stream through
//! untouched). The layer is infallible by construction (axum routers require
//! `Error = Infallible`): a capture problem degrades, it never fails the request.
//!
//! Wire-contract notes (the parts the kernel reads — do not change casually):
//! - `boundary = "http_incoming"`, `request.{method,path,query,request_id,headers,
//!   content_type,content_length,request_body}`, headers as the `deja::http::headers`
//!   multimap, bodies as `deja::http::body` capture objects;
//! - the response partial carries `status`; the finalizer injects `response_body`;
//! - `EventBuilder::finish` auto-duplicates `args`→`request` and `result`→`response`,
//!   which is exactly where the kernel looks.

use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::body::Body;
use http::{Request, Response};
use tower::{Layer, Service};
use tracing::Instrument as _;

use super::sampler::{RecordingDecisionGuard, RequestRecordingFacts, RequestRecordingSampler};

/// Request bodies larger than this are recorded lossily (event skipped) rather than
/// buffered without bound. Matches axum's default extractor limit, which every business
/// route here enforces downstream anyway — so anything we skip would have been rejected
/// by the handler regardless.
const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Tower layer installing the HTTP ingress boundary. Cheap to clone.
#[derive(Clone)]
pub struct DejaHttpIngressLayer {
    sampler: Option<Arc<dyn RequestRecordingSampler>>,
}

impl DejaHttpIngressLayer {
    pub fn new(sampler: Option<Arc<dyn RequestRecordingSampler>>) -> Self {
        Self { sampler }
    }
}

impl<S> Layer<S> for DejaHttpIngressLayer {
    type Service = DejaHttpIngressMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DejaHttpIngressMiddleware {
            inner,
            sampler: self.sampler.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DejaHttpIngressMiddleware<S> {
    inner: S,
    sampler: Option<Arc<dyn RequestRecordingSampler>>,
}

impl<S> Service<Request<Body>> for DejaHttpIngressMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, Infallible>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // PASSTHROUGH — no hook installed, or not a recordable business request.
        if !super::process_is_active() || !is_recordable_http(req.method(), req.uri().path()) {
            return Box::pin(self.inner.call(req));
        }

        let caller = std::panic::Location::caller();
        let sampler = self.sampler.clone();
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let method = parts.method.as_str().to_owned();
            let path = parts.uri.path().to_owned();
            let query = parts.uri.query().unwrap_or("").to_owned();
            // Guaranteed present: this layer sits inside SetRequestIdLayer.
            let request_id = parts
                .headers
                .get(common_utils::consts::X_REQUEST_ID)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();

            // 1. Sampler decision — record mode only, pushed BEFORE any buffering; RAII-cleared.
            let mut _decision_guard: Option<RecordingDecisionGuard> = None;
            let should_record = if super::process_is_record_mode() {
                let decision = match &sampler {
                    Some(sampler) => {
                        sampler
                            .should_record(RequestRecordingFacts {
                                request_id: request_id.clone(),
                                rpc: path.clone(),
                            })
                            .await
                    }
                    None => true,
                };
                deja::set_recording_decision(request_id.clone(), decision);
                _decision_guard = Some(RecordingDecisionGuard(request_id.clone()));
                decision
            } else {
                false
            };

            // Sampled-out (and replay-mode) requests skip capture entirely — no buffering.
            if !should_record {
                let req = Request::from_parts(parts, body);
                let span = tracing::info_span!(
                    "deja::http_incoming",
                    request_id = %request_id, method = %method, path = %path
                );
                return inner.call(req).instrument(span).await;
            }

            // 2. Buffer the request body (bounded). A failure degrades to an uncaptured
            //    passthrough of nothing-left-to-send — the honest response is a 500 here,
            //    but over-limit bodies would be rejected by the handler's own extractor
            //    anyway; record nothing and let the handler produce the error.
            let headers = parts.headers.clone();
            let (request_bytes, body_capture_failed) =
                match axum::body::to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
                    Ok(bytes) => (bytes, false),
                    Err(_) => (bytes::Bytes::new(), true),
                };
            let rebuilt = Request::from_parts(parts, Body::from(request_bytes.clone()));

            // 3. Open the record-only event in hyperswitch's exact http_incoming shape.
            let finalizer = if body_capture_failed {
                None // lossy: the body could not be captured faithfully; skip the event.
            } else {
                super::hook().map(|hook| {
                    let args = http_incoming_args(
                        &method,
                        &path,
                        &query,
                        &request_id,
                        &headers,
                        &request_bytes,
                    );
                    let builder = deja::EventBuilder::start_with_correlation_id(
                        hook.as_ref(),
                        "http_incoming",
                        "DejaHttpIngressLayer",
                        "call",
                        caller,
                        Some(request_id.clone()),
                        args,
                    )
                    .with_semantics(deja::BoundarySemantics {
                        replay_strategy: deja::ReplayStrategy::Substitute,
                        kind: None,
                        declaration: Some(deja::BoundaryDeclaration::default().reply_canon(
                            deja::CanonRef::new("project:!created_at,!last_synced,!modified_at"),
                        )),
                    });
                    (hook.clone(), builder)
                })
            };

            // 4. Run the handler inside the ingress span (ambient correlation for egress
            //    boundaries fired downstream). Handlers are infallible; 4xx/5xx are
            //    ordinary responses and flow the success path with their status.
            let span = tracing::info_span!(
                "deja::http_incoming",
                request_id = %request_id, method = %method, path = %path
            );
            let response = inner.call(rebuilt).instrument(span).await?;

            // 5. Wrap the response body; the finalizer injects `response_body` and emits
            //    the event at end-of-stream. The partial MUST carry `status` — the replay
            //    driver's baseline reads `response.status`.
            Ok(match finalizer {
                Some((hook, builder)) => {
                    let partial = serde_json::json!({
                        "method": method,
                        "path": path,
                        "query": query,
                        "request_id": request_id,
                        "status": response.status().as_u16(),
                    });
                    let hook_dyn: Arc<dyn deja::DejaHook> = hook;
                    let fin = deja::LazyEventFinalizer::new(builder, hook_dyn, partial, false);
                    response.map(|body| Body::new(HttpRecordingBody::new(body, fin)))
                }
                None => response,
            })
        })
    }
}

/// Only business requests are captured: every recordable route in the HTTP router is a
/// unary `POST`; `GET /health` (and any other non-POST) streams through untouched.
fn is_recordable_http(method: &http::Method, path: &str) -> bool {
    method == http::Method::POST && path != "/health"
}

/// Build the `http_incoming` args in hyperswitch's `IncomingHttpRecord` shape — the
/// contract deja-kernel reconstructs (`method/path/query/request_id/headers/content_type/
/// content_length/request_body`, multimap headers, `deja::http::body` capture object).
fn http_incoming_args(
    method: &str,
    path: &str,
    query: &str,
    request_id: &str,
    headers: &http::HeaderMap,
    body: &[u8],
) -> serde_json::Value {
    let header_pairs = headers.iter().map(|(name, value)| {
        (
            name.as_str().to_owned(),
            value
                .to_str()
                .map(str::to_owned)
                .unwrap_or_else(|_| format!("{value:?}")),
        )
    });
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let content_length = headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    serde_json::json!({
        "method": method,
        "path": path,
        "query": query,
        "request_id": request_id,
        "headers": deja::http::headers(header_pairs),
        "content_type": content_type,
        "content_length": content_length,
        "request_body": deja::http::body(body),
    })
}

/// Response-body wrapper: tees data frames into the finalizer and emits the event at
/// end-of-stream (or early, once the sized length is fully captured; or via the
/// finalizer's own `Drop` on client disconnect). `is_end_stream`/`size_hint` delegate —
/// h2/hyper rely on them for END_STREAM placement.
pub struct HttpRecordingBody {
    inner: Body,
    finalizer: Option<deja::LazyEventFinalizer>,
    expected_bytes: Option<u64>,
    captured_bytes: u64,
}

impl HttpRecordingBody {
    fn new(inner: Body, finalizer: deja::LazyEventFinalizer) -> Self {
        use http_body::Body as _;
        if inner.is_end_stream() {
            // Bodyless response: emit the event now (empty capture) and wrap plain.
            let _ = finalizer.finalize();
            return Self {
                inner,
                finalizer: None,
                expected_bytes: Some(0),
                captured_bytes: 0,
            };
        }
        let expected_bytes = inner.size_hint().exact();
        Self {
            inner,
            finalizer: Some(finalizer),
            expected_bytes,
            captured_bytes: 0,
        }
    }
}

impl http_body::Body for HttpRecordingBody {
    type Data = bytes::Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<bytes::Bytes>, axum::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    if let Some(finalizer) = this.finalizer.as_mut() {
                        finalizer.capture_chunk(data);
                    }
                    this.captured_bytes = this
                        .captured_bytes
                        .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                    // Sized bodies may never yield an explicit end-of-stream poll on a
                    // keep-alive connection; finalize as soon as the full length landed.
                    if let Some(expected) = this.expected_bytes {
                        if this.captured_bytes >= expected {
                            if let Some(finalizer) = this.finalizer.take() {
                                let _ = finalizer.finalize();
                            }
                        }
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(None) => {
                if let Some(finalizer) = this.finalizer.take() {
                    let _ = finalizer.finalize();
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        http_body::Body::is_end_stream(&self.inner)
    }

    fn size_hint(&self) -> http_body::SizeHint {
        http_body::Body::size_hint(&self.inner)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn only_business_posts_are_recordable() {
        assert!(is_recordable_http(
            &http::Method::POST,
            "/payments/authorize"
        ));
        assert!(is_recordable_http(&http::Method::POST, "/composite/payments/authorize"));
        assert!(!is_recordable_http(&http::Method::GET, "/health"));
        assert!(!is_recordable_http(&http::Method::POST, "/health"));
        assert!(!is_recordable_http(&http::Method::GET, "/payments/authorize"));
    }

    /// The kernel contract: `request.{method,path,query,request_id,headers,request_body}`
    /// with multimap headers and a fidelity-ordered body-capture object. This pins the
    /// shape deja-kernel's `reconstruct_driver_request` reads.
    #[test]
    fn args_match_the_kernel_reconstruct_shape() {
        let mut headers = http::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.append("x-multi", "a".parse().unwrap());
        headers.append("x-multi", "b".parse().unwrap());
        let args = http_incoming_args(
            "POST",
            "/payments/authorize",
            "expand=true",
            "req-1",
            &headers,
            br#"{"amount":100}"#,
        );

        assert_eq!(args["method"], "POST");
        assert_eq!(args["path"], "/payments/authorize");
        assert_eq!(args["query"], "expand=true");
        assert_eq!(args["request_id"], "req-1");
        // Multimap headers: name -> array of values, duplicates preserved in order.
        assert_eq!(args["headers"]["content-type"][0], "application/json");
        assert_eq!(args["headers"]["x-multi"][0], "a");
        assert_eq!(args["headers"]["x-multi"][1], "b");
        assert_eq!(args["content_type"], "application/json");
        // Body capture object: fidelity fields the kernel recovers bytes from.
        assert_eq!(args["request_body"]["captured"], true);
        assert_eq!(args["request_body"]["bytes_len"], 14);
        assert_eq!(args["request_body"]["utf8"], true);
        assert_eq!(args["request_body"]["json"]["amount"], 100);
        assert_eq!(args["request_body"]["raw_bytes"][0], 123); // '{'
    }
}
