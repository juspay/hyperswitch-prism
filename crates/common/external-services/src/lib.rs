#[cfg(feature = "otel")]
pub mod otel_metrics;
pub mod http_client;
pub mod service;
pub mod shared_metrics;
pub use service::*;
