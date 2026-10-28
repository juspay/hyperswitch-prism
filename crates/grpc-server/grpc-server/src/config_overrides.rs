use common_utils::{consts::X_OUTGOING_API_OVERHEAD, request_metrics::ConnectorLatencyTracker};
use http::{Request, Response};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tonic::body::Body;
use tower::{Layer, Service};
use ucs_env::{configs::Config, error::IntoGrpcStatus};
// Simple middleware layer for Tonic
#[derive(Clone)]
pub struct RequestExtensionsLayer {
    base_config: Arc<Config>,
}

#[allow(clippy::new_without_default)]
impl RequestExtensionsLayer {
    pub fn new(base_config: Arc<Config>) -> Self {
        Self { base_config }
    }
}

impl<S> Layer<S> for RequestExtensionsLayer {
    type Service = TonicRequestExtensionsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TonicRequestExtensionsMiddleware {
            inner,
            base_config: self.base_config.clone(),
        }
    }
}
#[derive(Clone)]
pub struct TonicRequestExtensionsMiddleware<S> {
    inner: S,
    base_config: Arc<Config>,
}

impl<S> Service<Request<Body>> for TonicRequestExtensionsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = tonic::Status> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = tonic::Status;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| e)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let config_override = req
            .headers()
            .get("x-config-override")
            .and_then(|h| h.to_str().ok());

        match ucs_interface_common::middleware::extract_and_merge_config(
            config_override,
            &self.base_config,
        ) {
            Ok(cfg) => {
                req.extensions_mut().insert(cfg);
            }
            Err(e) => {
                let err = e.into_grpc_status();
                let fut = async move { Err(err) };
                return Box::pin(fut);
            }
        }

        let future = self.inner.call(req);
        Box::pin(async move {
            let response = future.await?;
            Ok(response)
        })
    }
}

#[derive(Clone, Copy, Default)]
pub struct CompositeLatencyLayer;

impl<S> Layer<S> for CompositeLatencyLayer {
    type Service = CompositeLatencyService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CompositeLatencyService { inner }
    }
}

#[derive(Clone)]
pub struct CompositeLatencyService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for CompositeLatencyService<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = tonic::Status> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let is_composite = request
            .uri()
            .path()
            .split('/')
            .nth(1)
            .and_then(|service| service.rsplit('.').next())
            .is_some_and(|service| service.starts_with("Composite"));

        if !is_composite {
            let future = self.inner.call(request);
            return Box::pin(future);
        }

        let tracker = ConnectorLatencyTracker::default();
        request.extensions_mut().insert(tracker.clone());
        let future = self.inner.call(request);

        Box::pin(async move {
            let result = future.await;
            let outgoing_latency_ms = tracker.connector_time().as_millis().to_string();
            match result {
                Ok(mut response) => {
                    if let Ok(value) = outgoing_latency_ms.parse() {
                        response
                            .headers_mut()
                            .insert(X_OUTGOING_API_OVERHEAD, value);
                    }
                    Ok(response)
                }
                Err(mut status) => {
                    if let Ok(value) = outgoing_latency_ms.parse() {
                        status.metadata_mut().insert(X_OUTGOING_API_OVERHEAD, value);
                    }
                    Err(status)
                }
            }
        })
    }
}
