use http::{Request, Response};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tonic::body::Body;
use tower::{Layer, Service};
use ucs_env::configs::Config;
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
                let err = tonic::Status::internal(format!(
                    "Failed to merge config with override config: {e:?}"
                ));
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
