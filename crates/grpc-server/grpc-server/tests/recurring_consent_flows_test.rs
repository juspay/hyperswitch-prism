//! Contract coverage for the connector-neutral recurring consent RPCs.
//!
//! These tests exercise the real gRPC handlers and request conversions. Existing
//! connectors must reject the new flows with a structured integration error,
//! rather than an unregistered gRPC method or an accidental outbound request.

#![allow(clippy::expect_used)]

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{extract::State, http::StatusCode, routing::any, Json, Router};
use grpc_api_types::payments::{
    pix_automatico_payer_account::DocumentType,
    recurring_payment_service_client::RecurringPaymentServiceClient, IntegrationError,
    PixAutomaticoPayerAccount, RecurringPaymentServiceGetQrRequest,
    RecurringPaymentServiceRequestConfirmationRequest,
};
use grpc_server::app;
use hyperswitch_masking::Secret;
use prost::Message;
use serde_json::json;
use tonic::{transport::Channel, Code, Request, Status};
use ucs_env::configs;

mod common;

const RECURRENCE_ID: &str = "RR1234567820261123abcdefghijk";

fn add_metadata<T>(payload: T) -> Request<T> {
    let mut request = Request::new(payload);
    for (key, value) in [
        ("x-connector", "stripe"),
        ("x-auth", "header-key"),
        ("x-api-key", "sk_test_consent_contract"),
        ("x-merchant-id", "merchant_consent_contract"),
        ("x-request-id", "request_consent_contract"),
        ("x-tenant-id", "default"),
        ("x-connector-request-reference-id", "consent_contract"),
    ] {
        request
            .metadata_mut()
            .insert(key, value.parse().expect("test metadata should be valid"));
    }
    request
}

fn confirmation() -> RecurringPaymentServiceRequestConfirmationRequest {
    RecurringPaymentServiceRequestConfirmationRequest {
        merchant_request_id: Some("confirmation_contract".to_string()),
        connector_recurring_payment_id: RECURRENCE_ID.to_string(),
        payer: Some(PixAutomaticoPayerAccount {
            document_number: Some(Secret::new("12345678901".to_string())),
            document_type: DocumentType::Cpf.into(),
            account_number: Some(Secret::new("123456".to_string())),
            bank_ispb: "12345678".to_string(),
            branch: Some(Secret::new("1234".to_string())),
        }),
        expires_at: "2030-12-01T10:00:00Z".to_string(),
        test_mode: Some(true),
        ..Default::default()
    }
}

fn qr() -> RecurringPaymentServiceGetQrRequest {
    RecurringPaymentServiceGetQrRequest {
        merchant_request_id: Some("qr_contract".to_string()),
        connector_recurring_payment_id: RECURRENCE_ID.to_string(),
        test_mode: Some(true),
        ..Default::default()
    }
}

fn integration_error(status: Status, expected_code: Code) -> IntegrationError {
    assert_eq!(
        status.code(),
        expected_code,
        "unexpected gRPC status: {status}"
    );
    assert!(
        !status.details().is_empty(),
        "the handler must return typed error details, not a bare unknown-method status"
    );
    let error = IntegrationError::decode(status.details())
        .expect("gRPC details should contain an IntegrationError");
    assert_eq!(error.error_message, status.message());
    error
}

fn assert_unsupported(status: Status, flow: &str) {
    let error = integration_error(status, Code::Unimplemented);
    assert!(
        matches!(
            error.error_code.as_str(),
            "FLOW_NOT_SUPPORTED" | "NOT_SUPPORTED" | "NOT_IMPLEMENTED"
        ),
        "expected a connector capability error, got {error:?}"
    );
    let normalized: String = error
        .error_message
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    assert!(
        normalized.contains("stripe"),
        "missing connector context: {error:?}"
    );
    assert!(normalized.contains(flow), "missing flow context: {error:?}");
}

fn assert_invalid(status: Status, field: &str) {
    let error = integration_error(status, Code::InvalidArgument);
    assert!(
        matches!(
            error.error_code.as_str(),
            "MISSING_REQUIRED_FIELD" | "INVALID_DATA_FORMAT" | "MISSING_REQUIRED_FIELDS"
        ),
        "expected a request validation error, got {error:?}"
    );
    assert!(
        error.error_message.contains(field),
        "missing invalid-field context {field}: {error:?}"
    );
}

async fn unexpected_connector_request(
    State(request_count): State<Arc<AtomicUsize>>,
) -> (StatusCode, Json<serde_json::Value>) {
    request_count.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "unsupported consent flow must not call a connector"})),
    )
}

#[tokio::test]
async fn consent_rpcs_validate_and_dispatch_without_requiring_payment_amounts() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let request_count = Arc::new(AtomicUsize::new(0));
    let mock_state = Arc::clone(&request_count);
    let mock_server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(any(unexpected_connector_request))
                .with_state(mock_state),
        )
        .await
        .expect("mock server should run");
    });

    let mut config = configs::Config::new().expect("test config should load");
    config.test.enabled = true;
    config.test.mock_server_url = Some(format!("http://{address}/consent-contract"));
    let config = Arc::new(config);
    let service = app::Service::new(Arc::clone(&config)).await;
    let (server, mut client) =
        common::server_and_client_stub::<RecurringPaymentServiceClient<Channel>>(service, config)
            .await
            .expect("isolated gRPC server should start");

    let requests = async move {
        let error = client
            .request_confirmation(add_metadata(confirmation()))
            .await
            .expect_err("an existing connector should explicitly reject push confirmation");
        assert_unsupported(error, "requestrecurringconfirmation");

        let error = client
            .get_qr(add_metadata(qr()))
            .await
            .expect_err("an existing connector should explicitly reject recurrence QR retrieval");
        assert_unsupported(error, "getrecurringqr");

        let mut request = qr();
        request.initial_transaction_id = Some("a12345678901234567890123456".to_string());
        let error = client
            .get_qr(add_metadata(request))
            .await
            .expect_err("a valid initial-payment reference should reach connector dispatch");
        assert_unsupported(error, "getrecurringqr");

        let mut request = confirmation();
        request.connector_recurring_payment_id.clear();
        let error = client
            .request_confirmation(add_metadata(request))
            .await
            .expect_err("confirmation must require a recurrence reference");
        assert_invalid(error, "connector_recurring_payment_id");

        let mut request = confirmation();
        request.payer = None;
        let error = client
            .request_confirmation(add_metadata(request))
            .await
            .expect_err("confirmation must require payer details");
        assert_invalid(error, "payer");

        let mut request = confirmation();
        request.expires_at = "not-an-rfc3339-timestamp".to_string();
        let error = client
            .request_confirmation(add_metadata(request))
            .await
            .expect_err("confirmation must reject malformed expiry timestamps");
        assert_invalid(error, "expires_at");

        let mut request = confirmation();
        request
            .payer
            .as_mut()
            .expect("valid payer")
            .bank_ispb
            .clear();
        let error = client
            .request_confirmation(add_metadata(request))
            .await
            .expect_err("confirmation must require the payer bank ISPB");
        assert_invalid(error, "bank_ispb");

        let mut request = qr();
        request.connector_recurring_payment_id = "   ".to_string();
        let error = client
            .get_qr(add_metadata(request))
            .await
            .expect_err("QR retrieval must reject a blank recurrence reference");
        assert_invalid(error, "connector_recurring_payment_id");

        let mut request = qr();
        request.initial_transaction_id = Some("invalid/path?txid=value".to_string());
        let error = client
            .get_qr(add_metadata(request))
            .await
            .expect_err("QR retrieval must reject unsafe initial-payment references");
        assert_invalid(error, "initial_transaction_id");
    };

    tokio::select! {
        _ = server => panic!("gRPC server stopped before the contract assertions completed"),
        _ = requests => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            panic!("recurring consent contract assertions timed out");
        }
    }

    mock_server.abort();
    assert_eq!(
        request_count.load(Ordering::SeqCst),
        0,
        "unsupported or invalid consent requests must not issue connector HTTP calls"
    );
}
