#![allow(clippy::unwrap_used)]
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

use error_stack::ResultExt;
use http_body::Body as HttpBody;
use lazy_static::lazy_static;
use prometheus::{
    self, register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};
use tower::{Layer, Service};
// Define latency buckets for histograms
const LATENCY_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

lazy_static! {
    pub static ref GRPC_SERVER_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "GRPC_SERVER_REQUESTS_TOTAL",
        "Total number of gRPC requests received",
        &["method", "service", "connector"]
    )
    .unwrap();
    pub static ref GRPC_SERVER_REQUESTS_SUCCESSFUL: IntCounterVec = register_int_counter_vec!(
        "GRPC_SERVER_REQUESTS_SUCCESSFUL",
        "Total number of gRPC requests successful",
        &["method", "service", "connector"]
    )
    .unwrap();
    pub static ref GRPC_SERVER_REQUEST_LATENCY: HistogramVec = register_histogram_vec!(
        "GRPC_SERVER_REQUEST_LATENCY",
        "Request latency in seconds",
        &["method", "service", "connector"],
        LATENCY_BUCKETS.to_vec()
    )
    .unwrap();
    pub static ref EXTERNAL_SERVICE_API_CALLS_LATENCY: HistogramVec = register_histogram_vec!(
        "EXTERNAL_SERVICE_API_CALLS_LATENCY_SECONDS",
        "Latency of external service API calls",
        &["method", "service", "connector"],
        LATENCY_BUCKETS.to_vec()
    )
    .unwrap();
    pub static ref EXTERNAL_SERVICE_TOTAL_API_CALLS: IntCounterVec = register_int_counter_vec!(
        "EXTERNAL_SERVICE_TOTAL_API_CALLS",
        "Total number of external service API calls",
        &["method", "service", "connector"]
    )
    .unwrap();
    pub static ref EXTERNAL_SERVICE_API_CALLS_ERRORS: IntCounterVec = register_int_counter_vec!(
        "EXTERNAL_SERVICE_API_CALLS_ERRORS",
        "Total number of errors in external service API calls",
        &["method", "service", "connector", "error"]
    )
    .unwrap();
}

// Middleware Layer that automatically handles all gRPC methods
#[derive(Clone)]
pub struct GrpcMetricsLayer;

#[allow(clippy::new_without_default)]
impl GrpcMetricsLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for GrpcMetricsLayer {
    type Service = GrpcMetricsService<S>;

    fn layer(&self, service: S) -> Self::Service {
        GrpcMetricsService::new(service)
    }
}

// Middleware Service that intercepts all gRPC calls
#[derive(Clone)]
pub struct GrpcMetricsService<S> {
    inner: S,
}

impl<S> GrpcMetricsService<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, B> Service<hyper::Request<B>> for GrpcMetricsService<S>
where
    S: Service<hyper::Request<B>, Response = hyper::Response<B>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Debug,
    B: HttpBody + Send + 'static,
{
    type Response = hyper::Response<B>;
    type Error = tonic::Status;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| {
            tonic::Status::internal(format!("GrpcMetricsService inner poll_ready error: {e:?}"))
        })
    }

    fn call(&mut self, mut req: hyper::Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        let start_time = Instant::now();

        // Extract method name from gRPC path
        let method_name = extract_grpc_method_name(&req);

        let service_name = extract_grpc_service_name(&req);

        // Extract connector from request headers/metadata
        let connector = extract_connector_from_request(&req);

        // Increment total requests counter
        GRPC_SERVER_REQUESTS_TOTAL
            .with_label_values(&[&method_name, &service_name, &connector])
            .inc();
        req.extensions_mut().insert(service_name.clone());
        Box::pin(async move {
            let result = inner.call(req).await;

            // Record metrics based on response
            match &result {
                Ok(response) => {
                    // Check gRPC status from response
                    if is_grpc_success(response) {
                        GRPC_SERVER_REQUESTS_SUCCESSFUL
                            .with_label_values(&[&method_name, &service_name, &connector])
                            .inc();
                    }
                }
                Err(_) => {
                    // Network/transport level error
                }
            }

            // Record latency
            let duration = start_time.elapsed().as_secs_f64();
            GRPC_SERVER_REQUEST_LATENCY
                .with_label_values(&[&method_name, &service_name, &connector])
                .observe(duration);

            result.map_err(|e| {
                tonic::Status::internal(format!("GrpcMetricsService inner call error: {e:?}"))
            })
        })
    }
}

// Extract gRPC method name from HTTP request
fn extract_grpc_method_name<B>(req: &hyper::Request<B>) -> String {
    let path = req.uri().path();
    if let Some(method) = path.rfind('/') {
        let method_name = &path[method + 1..];
        if !method_name.is_empty() {
            return method_name.to_string();
        }
    }
    "unknown_method".to_string()
}

fn extract_grpc_service_name<B>(req: &hyper::Request<B>) -> String {
    let path = req.uri().path();

    if let Some(pos) = path.rfind('/') {
        let full_service = &path[1..pos];
        if let Some(service_name) = full_service.rsplit('.').next() {
            return service_name.to_string();
        }
    }

    "unknown_service".to_string()
}

// Extract connector information from request
fn extract_connector_from_request<B>(req: &hyper::Request<B>) -> String {
    if let Some(connector) = req.headers().get("x-connector") {
        if let Ok(connector_str) = connector.to_str() {
            return connector_str.to_string();
        }
    }

    "unknown".to_string()
}

// Check if gRPC response indicates success
fn is_grpc_success<B>(response: &hyper::Response<B>) -> bool {
    // gRPC success is based on grpc-status header, not HTTP status
    if let Some(grpc_status) = response.headers().get("grpc-status") {
        if let Ok(status_str) = grpc_status.to_str() {
            if let Ok(status_code) = status_str.parse::<i32>() {
                if status_code == 0 {
                    return true; // gRPC OK
                } else {
                    return false;
                }
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

// Metrics handler
pub async fn metrics_handler() -> error_stack::Result<String, MetricsError> {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder
        .encode(&metric_families, &mut buffer)
        .change_context(MetricsError::EncodingError)?;
    String::from_utf8(buffer).change_context(MetricsError::Utf8Error)
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Error encoding metrics")]
    EncodingError,
    #[error("Error converting metrics to utf8")]
    Utf8Error,
}
