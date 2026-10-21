//! Deterministic Elavon Payment Gateway gRPC flow coverage.
//!
//! The test starts UCS in-process and redirects connector HTTP calls to a stateful
//! mock. This exercises the public gRPC boundary and the real Elavon PG request /
//! response transformers without requiring sandbox credentials or an interactive
//! hosted page.

#![allow(clippy::expect_used)]

use std::{
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use cards::CardNumber;
use grpc_api_types::payments::{
    payment_method,
    payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient,
    payment_service_client::PaymentServiceClient, refund_service_client::RefundServiceClient,
    Address, AuthenticationType, CaptureMethod, CardDetails, Currency, Money, PaymentAddress,
    PaymentMethod, PaymentMethodAuthenticationServicePreAuthenticateRequest,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceCreateOrderRequest,
    PaymentServiceGetRequest, PaymentServiceRefundRequest, PaymentServiceVoidRequest,
    PaymentStatus, RefundServiceGetRequest, RefundStatus, TokenPaymentMethodType,
};
use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde_json::{json, Value};
use tonic::{transport::Channel, Request};
use ucs_env::configs;

mod common;

const CONNECTOR: &str = "elavon_pg";
const MERCHANT_ID: &str = "merchant_elavon_pg_grpc_test";
const AMOUNT: i64 = 1000;
const ORDER_HREF: &str = "https://mock.elavon.test/orders/order_123";
const SESSION_HREF: &str = "https://mock.elavon.test/payment-sessions/session_123";
const HOSTED_PAGE_URL: &str = "https://mock.elavon.test/hosted/session_123";

#[derive(Clone, Debug)]
struct RecordedRequest {
    method: Method,
    original_url: String,
    body: Value,
}

#[derive(Clone, Default)]
struct MockState {
    sale_count: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

async fn elavon_mock(
    State(state): State<MockState>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let original_url = headers
        .get("x-api-url")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).expect("connector request must contain valid JSON")
    };

    state
        .requests
        .lock()
        .expect("mock request log lock must not be poisoned")
        .push(RecordedRequest {
            method: method.clone(),
            original_url: original_url.clone(),
            body: body.clone(),
        });

    if method == Method::POST && original_url.ends_with("/orders") {
        return (
            StatusCode::CREATED,
            Json(json!({ "id": "order_123", "href": ORDER_HREF })),
        )
            .into_response();
    }

    if method == Method::POST && original_url.ends_with("/payment-sessions") {
        return (
            StatusCode::CREATED,
            Json(json!({
                "id": "session_123",
                "href": SESSION_HREF,
                "url": HOSTED_PAGE_URL,
                "expiresAt": "2030-01-01T00:00:00Z"
            })),
        )
            .into_response();
    }

    if method == Method::POST && original_url.ends_with("/transactions") {
        if body.get("paymentSession").is_some() {
            let sale_number = state.sale_count.fetch_add(1, Ordering::SeqCst);
            let transaction_id = if sale_number == 0 {
                "txn_capture"
            } else {
                "txn_void"
            };
            return (
                StatusCode::CREATED,
                Json(json!({
                    "id": transaction_id,
                    "type": "sale",
                    "state": "authorized",
                    "isAuthorized": true,
                    "doCapture": false,
                    "total": { "amount": "10.00", "currencyCode": "USD" },
                    "partialCapture": { "isPartiallyCapturable": false },
                    "failures": []
                })),
            )
                .into_response();
        }

        if body.get("type") == Some(&Value::String("void".to_string())) {
            return (
                StatusCode::CREATED,
                Json(json!({
                    "id": "txn_void_child",
                    "type": "void",
                    "state": "voided",
                    "isAuthorized": true,
                    "failures": []
                })),
            )
                .into_response();
        }

        if body.get("type") == Some(&Value::String("refund".to_string())) {
            return (
                StatusCode::CREATED,
                Json(json!({
                    "id": "txn_refund",
                    "type": "refund",
                    "state": "captured",
                    "isAuthorized": true,
                    "processorReference": "refund_processor_reference",
                    "failures": []
                })),
            )
                .into_response();
        }
    }

    if method == Method::POST && original_url.ends_with("/transactions/txn_capture") {
        return (
            StatusCode::OK,
            Json(json!({
                "id": "capture_123",
                "state": "captured",
                "isAuthorized": true,
                "failures": []
            })),
        )
            .into_response();
    }

    if method == Method::GET && original_url.ends_with("/transactions/txn_capture") {
        return (
            StatusCode::OK,
            Json(json!({
                "id": "txn_capture",
                "type": "sale",
                "state": "authorized",
                "isAuthorized": true,
                "doCapture": false,
                "total": { "amount": "10.00", "currencyCode": "USD" },
                "partialCapture": { "isPartiallyCapturable": false },
                "failures": []
            })),
        )
            .into_response();
    }

    if method == Method::GET && original_url.ends_with("/transactions/txn_refund") {
        return (
            StatusCode::OK,
            Json(json!({
                "id": "txn_refund",
                "type": "refund",
                "state": "captured",
                "isAuthorized": true,
                "processorReference": "refund_processor_reference",
                "failures": []
            })),
        )
            .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "status": 404,
            "failures": [{
                "code": "unexpectedMockRequest",
                "description": format!("No mock response for {method} {original_url}")
            }]
        })),
    )
        .into_response()
}

fn add_elavon_metadata<T>(request: &mut Request<T>, request_id: &str) {
    let connector_config = json!({
        "config": {
            "ElavonPg": {
                "api_key": "mock_merchant_alias",
                "key1": "mock_secret_key"
            }
        }
    })
    .to_string();

    for (key, value) in [
        ("x-connector", CONNECTOR.to_string()),
        ("x-connector-config", connector_config),
        ("x-merchant-id", MERCHANT_ID.to_string()),
        ("x-request-id", request_id.to_string()),
        ("x-tenant-id", "default".to_string()),
        (
            "x-connector-request-reference-id",
            format!("elavon_pg_{request_id}"),
        ),
    ] {
        request.metadata_mut().insert(
            key,
            value.parse().expect("test metadata value should be valid"),
        );
    }
}

fn money() -> Money {
    Money {
        minor_amount: AMOUNT,
        currency: i32::from(Currency::Usd),
    }
}

fn card_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
            card_number: Some(
                CardNumber::from_str("4546341111111119").expect("test card number should be valid"),
            ),
            card_exp_month: Some(Secret::new("12".to_string())),
            card_exp_year: Some(Secret::new("30".to_string())),
            card_cvc: Some(Secret::new("123".to_string())),
            card_holder_name: Some(Secret::new("Elavon PG Test".to_string())),
            card_type: Some("credit".to_string()),
            ..Default::default()
        })),
    }
}

fn session_payment_method() -> PaymentMethod {
    PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Token(
            TokenPaymentMethodType {
                token: Some(Secret::new(SESSION_HREF.to_string())),
                token_payment_method_type: None,
            },
        )),
    }
}

async fn create_hosted_authorization(
    payment_client: &mut PaymentServiceClient<Channel>,
    authentication_client: &mut PaymentMethodAuthenticationServiceClient<Channel>,
    suffix: &str,
) -> grpc_api_types::payments::PaymentServiceAuthorizeResponse {
    let mut create_order = Request::new(PaymentServiceCreateOrderRequest {
        merchant_order_id: Some(format!("order_{suffix}")),
        amount: Some(money()),
        test_mode: Some(true),
        ..Default::default()
    });
    add_elavon_metadata(&mut create_order, &format!("create_order_{suffix}"));
    let order = payment_client
        .create_order(create_order)
        .await
        .expect("CreateOrder gRPC call should succeed")
        .into_inner();
    assert_eq!(order.status, i32::from(PaymentStatus::Pending));
    assert_eq!(order.connector_order_id.as_deref(), Some(ORDER_HREF));

    let mut pre_authenticate =
        Request::new(PaymentMethodAuthenticationServicePreAuthenticateRequest {
            merchant_order_id: Some(format!("order_{suffix}")),
            merchant_transaction_id: Some(format!("payment_{suffix}")),
            amount: Some(money()),
            payment_method: Some(card_payment_method()),
            address: Some(PaymentAddress {
                billing_address: Some(Address::default()),
                shipping_address: None,
            }),
            enrolled_for_3ds: true,
            return_url: Some("https://merchant.example/return".to_string()),
            continue_redirection_url: Some("https://router.example/complete".to_string()),
            capture_method: Some(i32::from(CaptureMethod::Manual)),
            connector_order_id: order.connector_order_id,
            test_mode: Some(true),
            ..Default::default()
        });
    add_elavon_metadata(&mut pre_authenticate, &format!("pre_authenticate_{suffix}"));
    let pre_authenticate = authentication_client
        .pre_authenticate(pre_authenticate)
        .await
        .expect("PreAuthenticate gRPC call should succeed")
        .into_inner();
    assert_eq!(
        pre_authenticate.status,
        i32::from(PaymentStatus::AuthenticationPending)
    );
    assert!(pre_authenticate.redirection_data.is_some());
    let connector_feature_data = pre_authenticate
        .connector_feature_data
        .expect("PreAuthenticate should return the hosted payment token");
    let connector_feature_data: Value = serde_json::from_str(&connector_feature_data.expose())
        .expect("hosted payment token metadata should be JSON");
    assert_eq!(
        connector_feature_data
            .get("payment_method_token")
            .and_then(Value::as_str),
        Some(SESSION_HREF)
    );

    let mut authorize = Request::new(PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some(format!("payment_{suffix}")),
        merchant_order_id: Some(format!("order_{suffix}")),
        amount: Some(money()),
        payment_method: Some(session_payment_method()),
        address: Some(PaymentAddress {
            billing_address: Some(Address::default()),
            shipping_address: None,
        }),
        capture_method: Some(i32::from(CaptureMethod::Manual)),
        auth_type: i32::from(AuthenticationType::ThreeDs),
        enrolled_for_3ds: Some(true),
        test_mode: Some(true),
        ..Default::default()
    });
    add_elavon_metadata(&mut authorize, &format!("authorize_{suffix}"));
    let authorize = Box::pin(payment_client.authorize(authorize))
        .await
        .expect("hosted-session Authorize gRPC call should succeed")
        .into_inner();
    assert_eq!(authorize.status, i32::from(PaymentStatus::Authorized));
    authorize
}

#[tokio::test]
async fn hosted_page_and_lifecycle_flows_cross_the_grpc_boundary() {
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let mock_address = mock_listener
        .local_addr()
        .expect("mock listener should have an address");
    let mock_state = MockState::default();
    let request_log = Arc::clone(&mock_state.requests);
    let mock_server = tokio::spawn(async move {
        axum::serve(
            mock_listener,
            Router::new()
                .fallback(any(elavon_mock))
                .with_state(mock_state),
        )
        .await
        .expect("mock server should run");
    });

    let mut config = configs::Config::new().expect("test config should load");
    config.test.enabled = true;
    config.test.mock_server_url = Some(format!("http://{mock_address}/mock-elavon"));
    let config = Arc::new(config);
    let service = app::Service::new(Arc::clone(&config)).await;
    let (server, channel) = common::server_and_channel_stub(service, config)
        .await
        .expect("gRPC test server should start");

    let test = async move {
        let mut payment_client = PaymentServiceClient::new(channel.clone());
        let mut authentication_client =
            PaymentMethodAuthenticationServiceClient::new(channel.clone());
        let mut refund_client = RefundServiceClient::new(channel);

        let capture_authorization = Box::pin(create_hosted_authorization(
            &mut payment_client,
            &mut authentication_client,
            "capture",
        ))
        .await;
        assert_eq!(
            capture_authorization.connector_transaction_id.as_deref(),
            Some("txn_capture")
        );

        let mut sync = Request::new(PaymentServiceGetRequest {
            connector_transaction_id: "txn_capture".to_string(),
            amount: Some(money()),
            capture_method: Some(i32::from(CaptureMethod::Manual)),
            test_mode: Some(true),
            ..Default::default()
        });
        add_elavon_metadata(&mut sync, "payment_sync");
        let sync = payment_client
            .get(sync)
            .await
            .expect("PSync gRPC call should succeed")
            .into_inner();
        assert_eq!(sync.status, i32::from(PaymentStatus::Authorized));

        let mut capture = Request::new(PaymentServiceCaptureRequest {
            merchant_capture_id: Some("capture_merchant_123".to_string()),
            connector_transaction_id: "txn_capture".to_string(),
            amount_to_capture: Some(money()),
            connector_feature_data: capture_authorization.connector_feature_data,
            capture_method: Some(i32::from(CaptureMethod::Manual)),
            test_mode: Some(true),
            ..Default::default()
        });
        add_elavon_metadata(&mut capture, "capture");
        let capture = payment_client
            .capture(capture)
            .await
            .expect("Capture gRPC call should succeed")
            .into_inner();
        assert_eq!(capture.status, i32::from(PaymentStatus::Charged));
        assert_eq!(capture.connector_transaction_id, "txn_capture");

        let mut refund = Request::new(PaymentServiceRefundRequest {
            merchant_refund_id: Some("refund_merchant_123".to_string()),
            connector_transaction_id: "txn_capture".to_string(),
            payment_amount: AMOUNT,
            refund_amount: Some(money()),
            test_mode: Some(true),
            ..Default::default()
        });
        add_elavon_metadata(&mut refund, "refund");
        let refund = payment_client
            .refund(refund)
            .await
            .expect("Refund gRPC call should succeed")
            .into_inner();
        assert_eq!(refund.status, i32::from(RefundStatus::RefundSuccess));
        assert_eq!(refund.connector_refund_id, "txn_refund");

        let mut refund_sync = Request::new(RefundServiceGetRequest {
            merchant_refund_id: Some("refund_merchant_123".to_string()),
            connector_transaction_id: "txn_capture".to_string(),
            refund_id: "txn_refund".to_string(),
            connector_refund_id: "txn_refund".to_string(),
            test_mode: Some(true),
            ..Default::default()
        });
        add_elavon_metadata(&mut refund_sync, "refund_sync");
        let refund_sync = refund_client
            .get(refund_sync)
            .await
            .expect("RSync gRPC call should succeed")
            .into_inner();
        assert_eq!(refund_sync.status, i32::from(RefundStatus::RefundSuccess));

        let void_authorization = Box::pin(create_hosted_authorization(
            &mut payment_client,
            &mut authentication_client,
            "void",
        ))
        .await;
        assert_eq!(
            void_authorization.connector_transaction_id.as_deref(),
            Some("txn_void")
        );

        let mut void = Request::new(PaymentServiceVoidRequest {
            merchant_void_id: Some("void_merchant_123".to_string()),
            connector_transaction_id: "txn_void".to_string(),
            amount: Some(money()),
            test_mode: Some(true),
            ..Default::default()
        });
        add_elavon_metadata(&mut void, "void");
        let void = payment_client
            .void(void)
            .await
            .expect("Void gRPC call should succeed")
            .into_inner();
        assert_eq!(void.status, i32::from(PaymentStatus::Voided));
        assert_eq!(void.connector_transaction_id, "txn_void");
    };

    tokio::select! {
        _ = server => panic!("gRPC server stopped before the assertions completed"),
        _ = test => {}
    }

    mock_server.abort();

    let requests = request_log
        .lock()
        .expect("mock request log lock must not be poisoned");
    let hosted_sales = requests
        .iter()
        .filter(|request| request.body.get("paymentSession").is_some())
        .collect::<Vec<_>>();
    assert_eq!(hosted_sales.len(), 2);
    for request in hosted_sales {
        assert_eq!(request.method, Method::POST);
        assert!(request.original_url.ends_with("/transactions"));
        assert_eq!(
            request.body,
            json!({
                "paymentSession": SESSION_HREF,
                "total": { "amount": "10.00", "currencyCode": "USD" }
            })
        );
    }

    let pre_authenticate = requests
        .iter()
        .find(|request| request.original_url.ends_with("/payment-sessions"))
        .expect("PreAuthenticate connector request should be recorded");
    assert_eq!(
        pre_authenticate.body.get("order").and_then(Value::as_str),
        Some(ORDER_HREF)
    );
    assert_eq!(
        pre_authenticate
            .body
            .get("doThreeDSecure")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        pre_authenticate
            .body
            .get("doCreateTransaction")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        pre_authenticate
            .body
            .get("doCapture")
            .and_then(Value::as_bool),
        Some(false)
    );

    let capture = requests
        .iter()
        .find(|request| {
            request.method == Method::POST
                && request.original_url.ends_with("/transactions/txn_capture")
        })
        .expect("Capture connector request should be recorded");
    assert_eq!(capture.body, json!({ "doCapture": true }));

    let void = requests
        .iter()
        .find(|request| request.body.get("type") == Some(&json!("void")))
        .expect("Void connector request should be recorded");
    assert_eq!(
        void.body.get("parentTransaction").and_then(Value::as_str),
        Some("txn_void")
    );

    let refund = requests
        .iter()
        .find(|request| request.body.get("type") == Some(&json!("refund")))
        .expect("Refund connector request should be recorded");
    assert_eq!(
        refund.body.get("parentTransaction").and_then(Value::as_str),
        Some("txn_capture")
    );
    assert_eq!(
        refund.body.get("total"),
        Some(&json!({ "amount": "10.00", "currencyCode": "USD" }))
    );
}
