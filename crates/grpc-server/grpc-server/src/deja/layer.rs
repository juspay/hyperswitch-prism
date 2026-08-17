//! The gRPC ingress record/replay boundary: a tower layer that captures each unary
//! request/response as one `grpc_incoming` event.
//!
//! **Inert until a boot hook is installed.** When no hook is active
//! ([`process_is_active`](super::process_is_active) is false) `call` is a pure
//! passthrough: no buffering, no allocation, the streaming body untouched — so the
//! server behaves exactly as it does without the feature.
//!
//! Server semantics (unlike the client egress boundary): the handler **always runs**,
//! in record and replay alike. We record the request and response; the orchestrator
//! drives replay. This is the record-only `EventBuilder` / `LazyEventFinalizer` shape,
//! not the substitute-the-call shape.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use http::{Request, Response};
use http_body_util::BodyExt as _;
use tonic::body::Body;
use tower::{Layer, Service};
use tracing::Instrument as _;

use super::sampler::{RecordingDecisionGuard, RequestRecordingFacts, RequestRecordingSampler};

/// Tower layer installing the gRPC ingress boundary. Cheap to clone.
#[derive(Clone)]
pub struct DejaIngressLayer {
    sampler: Option<Arc<dyn RequestRecordingSampler>>,
}

impl DejaIngressLayer {
    pub fn new(sampler: Option<Arc<dyn RequestRecordingSampler>>) -> Self {
        Self { sampler }
    }
}

impl<S> Layer<S> for DejaIngressLayer {
    type Service = DejaIngressMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DejaIngressMiddleware {
            inner,
            sampler: self.sampler.clone(),
        }
    }
}

#[derive(Clone)]
pub struct DejaIngressMiddleware<S> {
    inner: S,
    sampler: Option<Arc<dyn RequestRecordingSampler>>,
}

impl<S> Service<Request<Body>> for DejaIngressMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = tonic::Status>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = tonic::Status;
    type Future = Pin<Box<dyn Future<Output = Result<Response<Body>, tonic::Status>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // PASSTHROUGH — no hook installed, or not a UCS rpc. Zero cost, streaming
        // untouched. The path check is load-bearing beyond noise reduction: server
        // reflection is a bidi-streaming rpc, and buffering its request body would
        // deadlock the stream (the client waits for responses while collect() waits
        // for end-of-stream).
        if !super::process_is_active() || !is_recordable_path(req.uri().path()) {
            return Box::pin(self.inner.call(req));
        }

        // Coarse, stable call-site identity (correlation disambiguates concurrent calls).
        let caller = std::panic::Location::caller();
        let sampler = self.sampler.clone();
        // Clone-dance: we must buffer the request body before calling `inner`, so `inner`
        // is moved into the async block. Drive the poll_ready'd instance; keep the clone.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let rpc = parts.uri.path().to_owned();
            // `http::request::Parts` is not Clone (it holds Extensions), so capture what the
            // event needs before `from_parts` consumes it.
            let authority = parts.uri.authority().map(|authority| authority.as_str().to_owned());
            let headers = parts.headers.clone();
            let request_id = headers
                .get(common_utils::consts::X_REQUEST_ID)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();

            // 1. Sampler decision (record mode only), cleared on drop.
            let mut _decision_guard: Option<RecordingDecisionGuard> = None;
            let should_record = if super::process_is_record_mode() {
                let decision = match &sampler {
                    Some(sampler) => {
                        sampler
                            .should_record(RequestRecordingFacts {
                                request_id: request_id.clone(),
                                rpc: rpc.clone(),
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

            // 2. Buffer the unary request body (needed to decode args and to rebuild the
            //    body `inner` reads). Capture failures never fail the request.
            let request_bytes = body
                .collect()
                .await
                .map_err(|error| tonic::Status::internal(error.to_string()))?
                .to_bytes();
            let rebuilt = Request::from_parts(
                parts,
                Body::new(http_body_util::Full::new(request_bytes.clone())),
            );

            // 3. Open the record-only event (explicit correlation id — no ambient dependency).
            //    `should_record` is only true in record mode, where a hook is installed;
            //    if there is somehow no hook we simply record nothing (never panic).
            let finalizer = if should_record {
                super::hook().map(|hook| {
                    let decoded = super::descriptors::decode_unary_request(&rpc, &request_bytes);
                    let args = super::grpc_incoming_args(
                        &rpc,
                        authority.as_deref(),
                        &headers,
                        &request_bytes,
                        decoded,
                    );
                    let builder = deja::EventBuilder::start_with_correlation_id(
                        hook.as_ref(),
                        "grpc_incoming",
                        "GrpcServer",
                        "call",
                        caller,
                        Some(request_id.clone()),
                        args,
                    )
                    .with_semantics(deja::BoundarySemantics {
                        replay_strategy: deja::ReplayStrategy::Substitute,
                        kind: Some("grpc_incoming".to_owned()),
                        declaration: Some(
                            deja::BoundaryDeclaration::default()
                                .operation(deja::OperationKind::ExternalCall),
                        ),
                    });
                    let hook_dyn: Arc<dyn deja::DejaHook> = hook.clone();
                    deja::LazyEventFinalizer::new(builder, hook_dyn, serde_json::json!({}), false)
                })
            } else {
                None
            };

            // 4. Run the handler inside the ingress span (stamps ambient correlation for any
            //    boundary the handler fires). The handler ALWAYS runs.
            let span = tracing::info_span!("deja::grpc_incoming", request_id = %request_id, rpc = %rpc);
            let result = inner.call(rebuilt).instrument(span).await;

            // 5. Wrap the response so the event finalizes at body end-of-stream.
            match result {
                Ok(response) => Ok(match finalizer {
                    Some(finalizer) => {
                        response.map(|body| Body::new(RecordingBody::new(body, Some(finalizer))))
                    }
                    None => response,
                }),
                Err(status) => {
                    if let Some(finalizer) = finalizer {
                        // Error path: finalize immediately (no response body to stream).
                        let _ = finalizer.finalize();
                    }
                    Err(status)
                }
            }
        })
    }
}

/// Whether this rpc path belongs to a UCS service (proto package `types`). Health
/// (`grpc.health.v1`), reflection (`grpc.reflection.*` — bidi streaming), and any other
/// system service are never captured.
fn is_recordable_path(path: &str) -> bool {
    path.starts_with("/types.")
}

/// Response-body wrapper that tees each data frame into the event finalizer and finalizes
/// at end-of-stream. Passthrough for the frames themselves — the client sees the exact
/// same bytes.
pub struct RecordingBody {
    inner: Body,
    finalizer: Option<deja::LazyEventFinalizer>,
}

impl RecordingBody {
    fn new(inner: Body, finalizer: Option<deja::LazyEventFinalizer>) -> Self {
        Self { inner, finalizer }
    }
}

impl http_body::Body for RecordingBody {
    type Data = bytes::Bytes;
    type Error = tonic::Status;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<bytes::Bytes>, tonic::Status>>> {
        // `Body` is Unpin, so projecting out of the Pin is sound.
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let (Some(finalizer), Some(data)) = (this.finalizer.as_mut(), frame.data_ref()) {
                    finalizer.capture_chunk(data);
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

    // Both delegations are load-bearing: h2 decides whether END_STREAM rides the headers
    // frame from `is_end_stream` — a hardcoded `false` on a trailers-only (error) response
    // makes the stream end without grpc-status trailers, which clients reject. (If the
    // body is already ended at wrap time, tonic's `Body::new` short-circuits to an empty
    // body and drops the wrapper; the finalizer then finalizes via its own Drop impl.)
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    #[test]
    fn only_ucs_service_paths_are_recordable() {
        assert!(super::is_recordable_path("/types.PaymentService/Authorize"));
        assert!(super::is_recordable_path("/types.RefundService/Get"));
        // System services pass through — reflection is bidi streaming and MUST not be
        // buffered (deadlock), health is probe noise.
        assert!(!super::is_recordable_path("/grpc.health.v1.Health/Check"));
        assert!(!super::is_recordable_path(
            "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo"
        ));
        assert!(!super::is_recordable_path(
            "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo"
        ));
    }
}
