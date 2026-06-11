//! OpenTelemetry gRPC server metrics (OTLP push).
//!
//! Defines OTel meter instruments mirroring the Prometheus counters in
//! [`crate::shared_metrics`], recorded from the same gRPC middleware. These are
//! exported via the global `MeterProvider` installed by `ucs_env` (OTLP push),
//! and are gated behind the `otel` feature. The Prometheus `/metrics` scrape
//! endpoint continues to work independently.

use std::sync::LazyLock;

use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter},
    KeyValue,
};

/// Latency histogram buckets (seconds) — kept in sync with the Prometheus layer.
const LATENCY_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

static METER: LazyLock<Meter> = LazyLock::new(|| global::meter("connector_service"));

/// Total gRPC requests received, labelled by endpoint, flow, connector and status.
static GRPC_SERVER_REQUESTS_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("grpc_server_requests_total")
        .with_description("Total number of gRPC requests received, by endpoint and gRPC status")
        .build()
});

/// gRPC request latency in seconds, labelled by endpoint, flow, connector and status class.
static GRPC_SERVER_REQUEST_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("grpc_server_request_duration_seconds")
        .with_description("gRPC request latency in seconds")
        .with_boundaries(LATENCY_BUCKETS.to_vec())
        .build()
});

/// Classify a gRPC status code into a coarse outcome class, the gRPC way.
///
/// `success` for OK(0); `client_error` for codes attributable to the caller
/// (analogous to HTTP 4xx); `server_error` for everything else (analogous to
/// HTTP 5xx). See <https://grpc.io/docs/guides/status-codes/>.
pub fn classify_grpc_status(code: i32) -> &'static str {
    match code {
        0 => "success",
        // CANCELLED, INVALID_ARGUMENT, NOT_FOUND, ALREADY_EXISTS, PERMISSION_DENIED,
        // FAILED_PRECONDITION, OUT_OF_RANGE, UNAUTHENTICATED
        1 | 3 | 5 | 6 | 7 | 9 | 11 | 16 => "client_error",
        // UNKNOWN, DEADLINE_EXCEEDED, RESOURCE_EXHAUSTED, ABORTED, UNIMPLEMENTED,
        // INTERNAL, UNAVAILABLE, DATA_LOSS
        _ => "server_error",
    }
}

/// Record one completed gRPC request against the OTLP-exported instruments.
///
/// `method` is the RPC/flow name (e.g. `Authorize`), `service` the gRPC service,
/// `connector` the target connector, and `grpc_status` the numeric gRPC status code.
pub fn record_grpc_request(
    method: &str,
    service: &str,
    connector: &str,
    grpc_status: i32,
    duration_secs: f64,
) {
    let status_class = classify_grpc_status(grpc_status);

    GRPC_SERVER_REQUESTS_TOTAL.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("flow", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("grpc_status", i64::from(grpc_status)),
            KeyValue::new("status_class", status_class),
        ],
    );

    GRPC_SERVER_REQUEST_DURATION.record(
        duration_secs,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("flow", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("status_class", status_class),
        ],
    );
}
