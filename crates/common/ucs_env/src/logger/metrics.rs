//! OpenTelemetry (OTLP push) metrics pipeline.
//!
//! Mirrors the hyperswitch app's `setup_metrics_pipeline`: builds an OTLP/gRPC
//! metric exporter, wires it to a `PeriodicReader`, and installs a global
//! `MeterProvider`. Once installed, metric instruments created via
//! `opentelemetry::global::meter(..)` anywhere in the process are pushed to the
//! configured collector. This is gated behind the `otel` feature and is
//! additive to the Prometheus `/metrics` scrape endpoint.

use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    metrics::{periodic_reader_with_async_runtime::PeriodicReader, SdkMeterProvider, Temporality},
    runtime, Resource,
};

use crate::configs::OtelMetricsConfig;

/// Default interval between metric pushes, in seconds.
const DEFAULT_COLLECTION_INTERVAL_SECS: u64 = 3;
/// Default per-export timeout, in seconds.
const DEFAULT_EXPORT_TIMEOUT_SECS: u64 = 10;

/// Set up the OTLP metrics push pipeline if enabled in config.
///
/// No-op when `config.enabled` is false. On exporter build failure the error is
/// logged and the process continues without metrics push (never fatal).
pub fn setup_metrics_pipeline(config: &OtelMetricsConfig, service_name: &str) {
    if !config.enabled {
        return;
    }

    let mut exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_temporality(Temporality::Cumulative)
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(Duration::from_millis(
            config.otel_exporter_otlp_timeout_ms.unwrap_or(10_000),
        ));
    if let Some(endpoint) = &config.otel_exporter_otlp_endpoint {
        exporter = exporter.with_endpoint(endpoint);
    }

    let exporter = match exporter.build() {
        Ok(exporter) => exporter,
        Err(error) => {
            tracing::warn!(
                ?error,
                "failed to build OTLP metric exporter; metrics push disabled"
            );
            return;
        }
    };

    let reader = PeriodicReader::builder(exporter, runtime::TokioCurrentThread)
        .with_interval(Duration::from_secs(
            config
                .collection_interval_secs
                .unwrap_or(DEFAULT_COLLECTION_INTERVAL_SECS),
        ))
        .with_timeout(Duration::from_secs(
            config
                .export_timeout_secs
                .unwrap_or(DEFAULT_EXPORT_TIMEOUT_SECS),
        ))
        .build();

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(
            Resource::builder_empty()
                .with_attributes([
                    KeyValue::new("service.name", service_name.to_string()),
                    KeyValue::new(
                        "pod",
                        std::env::var("POD_NAME")
                            .unwrap_or_else(|_| "connector-service-default".to_string()),
                    ),
                ])
                .build(),
        )
        .build();

    opentelemetry::global::set_meter_provider(provider);

    tracing::info!(
        endpoint = ?config.otel_exporter_otlp_endpoint,
        "OTLP metrics push pipeline initialised"
    );
}
