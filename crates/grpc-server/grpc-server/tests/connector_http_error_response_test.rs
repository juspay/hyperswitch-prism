#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, routing::any, Json, Router};
use grpc_api_types::payments::{
    payment_service_client::PaymentServiceClient, Currency, PaymentServiceGetRequest,
};
use grpc_server::app;
use serde_json::json;
use tonic::{transport::Channel, Request};
use ucs_env::configs;

mod common;

async fn stripe_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "resource_missing",
                "message": "No such payment_intent: pi_missing",
                "type": "invalid_request_error",
                "param": "id",
                "payment_intent": { "id": "pi_missing" }
            }
        })),
    )
}

fn add_test_metadata<T>(request: &mut Request<T>) {
    for (key, value) in [
        ("x-connector", "stripe"),
        ("x-auth", "header-key"),
        ("x-api-key", "sk_test_mock"),
        ("x-merchant-id", "merchant_e2e"),
        ("x-request-id", "request_connector_404"),
        ("x-tenant-id", "default"),
        ("x-connector-request-reference-id", "payment_e2e_404"),
    ] {
        request
            .metadata_mut()
            .insert(key, value.parse().expect("test metadata should be valid"));
    }
}

#[tokio::test]
async fn connector_404_is_returned_as_grpc_ok_with_error_details() {
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let mock_address = mock_listener
        .local_addr()
        .expect("mock listener should have an address");
    let mock_server = tokio::spawn(async move {
        axum::serve(mock_listener, Router::new().fallback(any(stripe_not_found)))
            .await
            .expect("mock server should run");
    });

    let mut config = configs::Config::new().expect("test config should load");
    config.test.enabled = true;
    config.test.mock_server_url = Some(format!("http://{mock_address}/mockGateway"));
    let config = Arc::new(config);

    let service = app::Service::new(config.clone()).await;
    let (server, mut client) =
        common::server_and_client_stub::<PaymentServiceClient<Channel>>(service, config)
            .await
            .expect("gRPC test server should start");

    let request = async move {
        let mut request = Request::new(PaymentServiceGetRequest {
            connector_transaction_id: "pi_missing".to_string(),
            amount: Some(grpc_api_types::payments::Money {
                minor_amount: 1000,
                currency: Currency::Usd as i32,
            }),
            test_mode: Some(true),
            ..Default::default()
        });
        add_test_metadata(&mut request);

        let response = client
            .get(request)
            .await
            .expect("a parsed connector 404 must use gRPC OK")
            .into_inner();

        assert_eq!(response.status_code, 404);
        let connector_error = response
            .error
            .and_then(|error| error.connector_details)
            .expect("connector error details should be present");
        assert_eq!(connector_error.code.as_deref(), Some("resource_missing"));
        assert_eq!(
            connector_error.message.as_deref(),
            Some("No such payment_intent: pi_missing")
        );
    };

    tokio::select! {
        _ = server => panic!("gRPC server stopped before the assertion completed"),
        _ = request => {}
    }

    mock_server.abort();
}
