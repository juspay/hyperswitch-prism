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

/// Prefix applied to every UCS metric name so the whole set can be filtered in
/// VictoriaMetrics/Prometheus via `{__name__=~"ucs_.*"}`, regardless of how the
/// OTLP resource (`service.name`) / scope (`otel_scope_name`) attributes are
/// mapped to labels by the collector.
const METRIC_PREFIX: &str = "ucs_";

static METER: LazyLock<Meter> = LazyLock::new(|| global::meter("connector_service"));

/// Total gRPC requests received, labelled by endpoint, flow, connector and status.
static GRPC_SERVER_REQUESTS_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter(format!("{METRIC_PREFIX}grpc_server_requests_total"))
        .with_description("Total number of gRPC requests received, by endpoint and gRPC status")
        .build()
});

/// gRPC request latency in seconds, labelled by endpoint, flow, connector and status class.
static GRPC_SERVER_REQUEST_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram(format!(
            "{METRIC_PREFIX}grpc_server_request_duration_seconds"
        ))
        .with_description("gRPC request latency in seconds")
        .with_boundaries(LATENCY_BUCKETS.to_vec())
        .build()
});

/// Total outbound external/connector API calls, by method, service and connector.
static EXTERNAL_SERVICE_TOTAL_API_CALLS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter(format!("{METRIC_PREFIX}external_service_total_api_calls"))
        .with_description("Total number of outbound external/connector API calls")
        .build()
});

/// Outbound external/connector API call latency in seconds.
static EXTERNAL_SERVICE_API_CALLS_LATENCY: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram(format!(
            "{METRIC_PREFIX}external_service_api_calls_latency_seconds"
        ))
        .with_description("Latency of outbound external/connector API calls, in seconds")
        .with_boundaries(LATENCY_BUCKETS.to_vec())
        .build()
});

/// Errors from outbound external/connector API calls, by method, service, connector and error.
static EXTERNAL_SERVICE_API_CALLS_ERRORS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter(format!("{METRIC_PREFIX}external_service_api_calls_errors"))
        .with_description("Number of errored outbound external/connector API calls")
        .build()
});

/// Record one outbound connector API call (count). `mode` is "primary"/"shadow".
pub fn record_external_call(method: &str, service: &str, connector: &str, mode: &str) {
    EXTERNAL_SERVICE_TOTAL_API_CALLS.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("mode", mode.to_string()),
        ],
    );
}

/// Record the latency of one outbound connector API call. `mode` is "primary"/"shadow".
pub fn record_external_latency(
    method: &str,
    service: &str,
    connector: &str,
    mode: &str,
    duration_secs: f64,
) {
    EXTERNAL_SERVICE_API_CALLS_LATENCY.record(
        duration_secs,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("mode", mode.to_string()),
        ],
    );
}

/// Record one errored outbound connector API call (`error` is typically the status code).
pub fn record_external_error(
    method: &str,
    service: &str,
    connector: &str,
    mode: &str,
    error: &str,
) {
    EXTERNAL_SERVICE_API_CALLS_ERRORS.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("mode", mode.to_string()),
            KeyValue::new("error", error.to_string()),
        ],
    );
}

/// Connector flow outcome (FlowStatus: payment/refund/dispute status), by
/// connector, flow and mode. This is the payment-outcome signal — a connector
/// decline is recorded here even when the transport/gRPC call "succeeded".
static PAYMENT_STATUS_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter(format!("{METRIC_PREFIX}payment_status_total"))
        .with_description(
            "Connector flow outcome (payment/refund/dispute status), by connector and mode",
        )
        .build()
});

/// Record one connector flow outcome. `payment_status` is a `FlowStatus` label
/// (e.g. `payment_failure`); cardinality is bounded by the finite status enums.
pub fn record_payment_status(connector: &str, flow: &str, mode: &str, payment_status: &str) {
    PAYMENT_STATUS_TOTAL.add(
        1,
        &[
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("flow", flow.to_string()),
            KeyValue::new("mode", mode.to_string()),
            KeyValue::new("payment_status", payment_status.to_string()),
        ],
    );
}

/// Classify a gRPC status code into a coarse outcome class, the gRPC way.
///
/// `success` for OK(0); `client_error` for codes attributable to the caller
/// (analogous to HTTP 4xx); `server_error` for everything else (analogous to
/// HTTP 5xx). See <https://grpc.io/docs/guides/status-codes/>.
pub fn classify_grpc_status(code: tonic::Code) -> &'static str {
    use tonic::Code;
    match code {
        Code::Ok => "success",
        Code::Cancelled
        | Code::InvalidArgument
        | Code::NotFound
        | Code::AlreadyExists
        | Code::PermissionDenied
        | Code::FailedPrecondition
        | Code::OutOfRange
        | Code::Unauthenticated => "client_error",
        _ => "server_error",
    }
}

/// Record one completed gRPC request against the OTLP-exported instruments.
///
/// `method` is the RPC/flow name (e.g. `Authorize`), `service` the gRPC service,
/// `connector` the target connector, and `grpc_status` the gRPC status code.
pub fn record_grpc_request(
    method: &str,
    service: &str,
    connector: &str,
    mode: &str,
    grpc_status: tonic::Code,
    duration_secs: f64,
) {
    let status_class = classify_grpc_status(grpc_status);
    // Human-readable gRPC code name (e.g. "Ok", "Internal", "Unavailable") for a
    // legible dashboard label. Bounded cardinality (gRPC defines <= 17 codes);
    // kept alongside the coarse `status_class`.
    let grpc_status_name = format!("{grpc_status:?}");

    // `mode` is "primary"/"shadow" (bounded) for rollout observability.
    GRPC_SERVER_REQUESTS_TOTAL.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("flow", method.to_string()),
            KeyValue::new("service", service.to_string()),
            KeyValue::new("connector", connector.to_string()),
            KeyValue::new("mode", mode.to_string()),
            KeyValue::new("grpc_status", grpc_status_name),
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
            KeyValue::new("mode", mode.to_string()),
            KeyValue::new("status_class", status_class),
        ],
    );
}
