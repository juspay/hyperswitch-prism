//! Exercise real config loading, payment handlers, connector HTTP calls and JSON logging.
//! No connector credentials, Kafka or external services are needed.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
    sync::Arc,
};

use axum::{http::StatusCode, routing::any, Json, Router};
use grpc_api_types::payments::{
    payment_service_server::PaymentService, Currency, Money, PaymentServiceGetRequest,
    PaymentServiceGetResponse,
};
use grpc_server::server::payments::{Customer, MerchantAuthentication, Payments};
use log_utils::{
    AdditionalFieldsPlacement, JsonFormattingLayer, JsonFormattingLayerConfig, SpanStorageLayer,
};
use serde_json::{json, Value};
use tonic::Request;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::layer::SubscriberExt;
use ucs_env::configs::Config;
use ucs_interface_common::middleware::extract_and_merge_config;

#[derive(Clone, Default)]
struct LogCapture(Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for LogCapture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log capture lock poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl LogCapture {
    fn records(&self) -> Vec<Value> {
        let bytes = self.0.lock().expect("log capture lock");
        std::str::from_utf8(&bytes)
            .expect("UTF-8 log output")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each log line must be valid JSON"))
            .collect()
    }
}

fn load_config(enabled: bool) -> Config {
    // Use the startup loader, rather than manually compiling log fields: dropping
    // log config during deserialization or forgetting post-processing must fail here.
    let mut raw: Value = config::Config::builder()
        .add_source(config::File::from_str(
            include_str!("../../../../config/development.toml"),
            config::FileFormat::Toml,
        ))
        .add_source(config::File::from_str(
            include_str!("fixtures/log_transformation.toml"),
            config::FileFormat::Toml,
        ))
        .build()
        .expect("merge TOML fixtures")
        .try_deserialize()
        .expect("serialize configuration fixture");
    raw["log"]["fields"]["enabled"] = json!(enabled);
    raw["connector_response_masking"] = json!({
        "enabled": enabled,
        // Public values are allowlisted. Secret fields must stay masked even
        // when accidentally included in the operator's allowlist.
        "connector_keys": {"stripe": "id,status,metadata.public_field,client_secret"},
    });
    let file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("temporary config");
    serde_json::to_writer(&file, &raw).expect("write test configuration");
    Config::new_with_config_path(Some(file.path().to_path_buf())).expect("load startup config")
}

struct MockConnector {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl MockConnector {
    async fn start(status: StatusCode, body: Value) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock connector");
        let url = format!("http://{}/mockGateway", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().fallback(any(move || {
                    let body = body.clone();
                    // Avoid wall-clock Date headers making response comparisons flaky.
                    async move {
                        (
                            status,
                            [("date", "Thu, 19 Nov 2026 00:00:00 GMT")],
                            Json(body),
                        )
                    }
                })),
            )
            .await
            .expect("serve mock connector");
        });
        Self { url, task }
    }
}

impl Drop for MockConnector {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn payment_sync(
    mock: &MockConnector,
    enabled: bool,
    config_override: Option<&str>,
) -> (PaymentServiceGetResponse, Vec<Value>) {
    let mut config = load_config(enabled);
    config.test.enabled = true;
    config.test.mock_server_url = Some(mock.url.clone());
    let config = extract_and_merge_config(config_override, &Arc::new(config))
        .expect("valid override must not reject payment");

    let mut request = Request::new(PaymentServiceGetRequest {
        connector_transaction_id: "pi_log_regression".to_string(),
        amount: Some(Money {
            minor_amount: 1000,
            currency: i32::from(Currency::Usd),
        }),
        test_mode: Some(true),
        ..Default::default()
    });
    for (key, value) in [
        ("x-connector", "stripe"),
        ("x-auth", "header-key"),
        ("x-api-key", "sk_test_mock"),
        ("x-merchant-id", "merchant_log_regression"),
        ("x-request-id", "request_log_regression"),
        ("x-tenant-id", "default"),
        ("x-connector-request-reference-id", "payment_log_regression"),
    ] {
        request.metadata_mut().insert(key, value.parse().unwrap());
    }
    request.extensions_mut().insert(config);

    let capture = LogCapture::default();
    // These are the same storage and JSON formatter used by logger::setup.
    // A local subscriber keeps parallel tests isolated without a global logger.
    let formatter = JsonFormattingLayer::new(
        JsonFormattingLayerConfig {
            static_top_level_fields: HashMap::new(),
            top_level_keys: HashSet::new(),
            log_span_lifecycles: false,
            additional_fields_placement: AdditionalFieldsPlacement::TopLevel,
        },
        capture.clone(),
        serde_json::ser::CompactFormatter,
    )
    .expect("JSON formatter");
    let subscriber = tracing_subscriber::registry()
        .with(SpanStorageLayer::new([]))
        .with(formatter);
    let payments = Payments {
        merchant_authentication_service: MerchantAuthentication,
        customer_service: Customer,
    };
    let response = payments
        .get(request)
        .with_subscriber(subscriber)
        .await
        .expect("logging must not fail a transaction request")
        .into_inner();
    (response, capture.records())
}

fn golden_log<'a>(records: &'a [Value], category: &str, message: &str) -> &'a Value {
    let matching: Vec<_> = records
        .iter()
        .filter(|record| {
            record["category"] == category
                && record["message"]
                    .as_str()
                    .is_some_and(|value| value.ends_with(message))
        })
        .collect();
    assert_eq!(matching.len(), 1, "expected one {category} golden log");
    matching.first().unwrap()
}

fn assert_transformed_logs(records: &[Value], connector_status: u16) {
    let incoming = golden_log(
        records,
        "INCOMING_API",
        "Golden Log Line (incoming - response)",
    );
    assert_eq!(incoming["provider"], "stripe");
    assert_eq!(incoming["api_details"]["status_code"], 200);
    assert_eq!(
        incoming["api_details"]["request"]["connector_transaction_id"],
        "pi_log_regression",
    );
    assert!(incoming["api_details"]["response"].is_object());
    assert!(incoming["api_details"].get("missing").is_none());
    let timestamp = incoming["timestamp"].as_str().expect("mapped timestamp");
    time::OffsetDateTime::parse(
        timestamp,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .expect("timestamp must retain its ISO format");

    let outgoing = golden_log(records, "OUTGOING_API", "Outgoing Request completed");
    assert_eq!(outgoing["api_details"]["status_code"], connector_status);
    assert!(outgoing["api_details"]["headers"].is_object());
    assert!(!outgoing["api_details"]["headers"]
        .to_string()
        .contains("sk_test_mock"));
    assert!(outgoing["api_details"]["url"]
        .as_str()
        .expect("mapped connector URL")
        .ends_with("/mockGateway"));
    assert!(outgoing["api_details"].get("missing").is_none());
}

fn comparable_response(response: &PaymentServiceGetResponse) -> Value {
    let mut value = serde_json::to_value(response).unwrap();
    // Raw envelopes contain JSON strings with HashMap-backed header ordering.
    // Compare every field semantically, including the raw request and response.
    for key in [
        "raw_connector_request",
        "raw_connector_response",
        "typed_connector_response",
    ] {
        if let Some(raw) = value.get(key).and_then(Value::as_str) {
            value[key] = serde_json::from_str(raw).expect("JSON connector envelope");
        }
    }
    value
}

#[tokio::test]
async fn log_transformation_preserves_success_response_and_emits_masked_json() {
    let mock = MockConnector::start(
        StatusCode::OK,
        json!({
            "id": "pi_log_regression", "object": "payment_intent",
            "amount": 1000, "currency": "usd", "status": "succeeded",
            "client_secret": "private_client_secret",
            "metadata": {"merchant_reference": "private_reference", "public_field": "visible"}
        }),
    )
    .await;
    let (baseline, baseline_logs) = payment_sync(&mock, false, None).await;
    assert!(baseline_logs
        .iter()
        .all(|record| record.get("category").is_none()));

    for config_override in [
        None,
        Some(r#"{"proxy":{"idle_pool_connection_timeout":25}}"#),
    ] {
        let (response, records) = payment_sync(&mock, true, config_override).await;
        assert_eq!(
            comparable_response(&response),
            comparable_response(&baseline)
        );
        assert!(response.error.is_none());
        assert_transformed_logs(&records, 200);
        let outgoing = golden_log(&records, "OUTGOING_API", "Outgoing Request completed");
        assert_eq!(
            outgoing["api_details"]["response"]["id"],
            "pi_log_regression"
        );
        let masked = &outgoing["response.masked_body"];
        assert_eq!(masked["id"], "pi_log_regression");
        assert_eq!(masked["client_secret"], "***");
        assert_eq!(masked["metadata"]["merchant_reference"], "***");
        assert_eq!(masked["metadata"]["public_field"], "visible");
        let transformed = json!({"mapped": outgoing["api_details"], "masked": masked}).to_string();
        assert!(!transformed.contains("private_client_secret"));
        assert!(!transformed.contains("private_reference"));
    }
}

#[tokio::test]
async fn log_transformation_preserves_connector_error_response() {
    let mock = MockConnector::start(
        StatusCode::NOT_FOUND,
        json!({"error": {
            "code": "resource_missing", "type": "invalid_request_error",
            "message": "No such payment_intent: pi_log_regression", "param": "id",
            "payment_intent": {"id": "pi_log_regression"}
        }}),
    )
    .await;
    let (baseline, _) = payment_sync(&mock, false, None).await;
    let (response, records) = payment_sync(&mock, true, None).await;
    assert_eq!(
        comparable_response(&response),
        comparable_response(&baseline)
    );
    assert_eq!(response.status_code, 404);
    let details = response.error.unwrap().connector_details.unwrap();
    assert_eq!(details.code.as_deref(), Some("resource_missing"));
    assert_transformed_logs(&records, 404);
}

#[test]
fn log_transformation_config_remains_operator_controlled() {
    let config = Arc::new(load_config(true));
    for patch in [
        r#"{"log":{"fields":{"enabled":false}}}"#,
        r#"{"log_fields":{}}"#,
        r#"{"unknown_field":true}"#,
    ] {
        assert!(extract_and_merge_config(Some(patch), &config).is_err());
    }
    assert!(config.log_fields.enabled);
    assert!(!config.log_fields.incoming.is_empty());
    assert!(!config.log_fields.outgoing.is_empty());
}
