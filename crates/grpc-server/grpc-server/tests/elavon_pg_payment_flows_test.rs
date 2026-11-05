//! Deterministic Elavon Payment Gateway gRPC flow coverage.
//!
//! The test starts UCS in-process and redirects connector HTTP calls to a stateful
//! mock. This exercises the public gRPC boundary and the real Elavon PG request /
//! response transformers without requiring sandbox credentials or an interactive
//! hosted page.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

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
    customer_service_client::CustomerServiceClient, payment_method,
    payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient,
    payment_method_service_client::PaymentMethodServiceClient,
    payment_service_client::PaymentServiceClient,
    recurring_payment_service_client::RecurringPaymentServiceClient,
    refund_service_client::RefundServiceClient, Address, AuthenticationType, CaptureMethod,
    CardDetails, Currency, CustomerServiceCreateRequest, Money, PaymentAddress, PaymentMethod,
    PaymentMethodAuthenticationServicePreAuthenticateRequest, PaymentMethodServiceTokenizeRequest,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceCreateOrderRequest,
    PaymentServiceGetRequest, PaymentServiceRefundRequest, PaymentServiceSetupRecurringRequest,
    PaymentServiceVoidRequest, PaymentStatus, RecurringPaymentServiceChargeRequest,
    RefundServiceGetRequest, RefundStatus, TokenPaymentMethodType,
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
/// A `credentialOnFileData` value, from a mandate saved before stored cards.
const COF_DATA: &str = "A7562943766729115153A:357115:";
const SHOPPER_HREF: &str = "https://mock.elavon.test/shoppers/shopper_123";
const STORED_CARD_HREF: &str = "https://mock.elavon.test/stored-cards/stored_card_123";
/// EPG already holds this card for the shopper, so it refuses to store it again.
const DUPLICATE_CARD: &str = "5555555555554444";
/// EPG's `schemeReference` -- the scheme network transaction id, deliberately a different
/// value from [`COF_DATA`] so the two cannot be confused in the assertions below.
const SCHEME_REF: &str = "019876543210987";

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

    if method == Method::POST && original_url.ends_with("/shoppers") {
        return (
            StatusCode::CREATED,
            Json(json!({ "id": "shopper_123", "href": SHOPPER_HREF })),
        )
            .into_response();
    }

    if method == Method::POST && original_url.ends_with("/stored-cards") {
        if body["card"]["number"] == json!(DUPLICATE_CARD) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "status": 400, "failures": [
                    {
                        "code": "badRequest",
                        "description": "The request is invalid; correct all issues before resending"
                    },
                    {
                        "code": "fieldValidationFailure",
                        "description": "Stored card already exists for this card number",
                        "field": "card.number"
                    }
                ]})),
            )
                .into_response();
        }
        return (
            StatusCode::CREATED,
            Json(json!({
                "id": "stored_card_123",
                "href": STORED_CARD_HREF,
                "shopper": SHOPPER_HREF,
                "doVerify": true,
                "failures": null
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

        // A stored card sale echoes the stored card.
        if body.get("storedCard").is_some() {
            return (
                StatusCode::CREATED,
                Json(json!({
                    "id": "txn_stored_card",
                    "type": "sale",
                    "state": "captured",
                    "isAuthorized": true,
                    "doCapture": true,
                    "total": body.get("total").cloned().unwrap_or(Value::Null),
                    "storedCard": body.get("storedCard").cloned().unwrap_or(Value::Null),
                    "schemeReference": null,
                    "failures": []
                })),
            )
                .into_response();
        }

        // Direct sale bodies: a one-time card or pre-decrypted wallet sale.
        if body.get("card").is_some() {
            return (
                StatusCode::CREATED,
                Json(json!({
                    "id": "txn_sale",
                    "type": "sale",
                    "state": "captured",
                    "isAuthorized": true,
                    "doCapture": true,
                    "total": body.get("total").cloned().unwrap_or(Value::Null),
                    "schemeReference": json!(SCHEME_REF),
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

fn authorize_request(payment_method: Value, setup_future_usage: Option<&str>) -> Value {
    let mut request = json!({
        "merchant_transaction_id": "merchant_txn_recurring",
        "amount": { "minor_amount": AMOUNT, "currency": "USD" },
        "payment_method": { "payment_method": payment_method },
        "capture_method": "AUTOMATIC",
        "auth_type": "NO_THREE_DS",
        "order_details": [],
        "address": {},
        "test_mode": true
    });
    if let Some(setup_future_usage) = setup_future_usage {
        request["setup_future_usage"] = json!(setup_future_usage);
    }
    request
}

fn card_json() -> Value {
    json!({ "card": {
        "card_number": "4111111111111111",
        "card_exp_month": "12",
        "card_exp_year": "30",
        "card_cvc": "123",
        "card_holder_name": "Elavon PG Test",
        "card_type": "credit"
    }})
}

fn google_pay_json() -> Value {
    json!({ "google_pay_sdk": {
        "type": "CARD",
        "description": "Visa 1111",
        "info": { "card_network": "VISA", "card_details": "1111" },
        "tokenization_data": { "tokenization_data": { "decrypted_data": {
            "card_exp_month": "12",
            "card_exp_year": "2030",
            "application_primary_account_number": "4111111111111111",
            "cryptogram": "AgAAAAAAAIR8CQrXcIhbQAAAAAA=",
            "eci_indicator": "07"
        }}}
    }})
}

/// Google Pay Mastercard `CRYPTOGRAM_3DS`: Google documents `eciIndicator` as empty for
/// these, and the gRPC layer also turns an omitted proto string into `Some("")`, so the
/// connector must read it as "no indicator" and send the default rather than refuse the sale.
fn mastercard_google_pay_json() -> Value {
    json!({ "google_pay_sdk": {
        "type": "CARD",
        "description": "Mastercard 4444",
        "info": { "card_network": "MASTERCARD", "card_details": "4444" },
        "tokenization_data": { "tokenization_data": { "decrypted_data": {
            "card_exp_month": "12",
            "card_exp_year": "2030",
            "application_primary_account_number": "5555555555554444",
            "cryptogram": "AgAAAAAAAIR8CQrXcIhbQAAAAAA=",
            "eci_indicator": ""
        }}}
    }})
}

fn apple_pay_json() -> Value {
    json!({ "apple_pay_sdk": {
        "payment_data": { "payment_data": { "decrypted_data": {
            "application_primary_account_number": "4111111111111111",
            "application_expiration_month": "12",
            "application_expiration_year": "2030",
            "payment_data": {
                "online_payment_cryptogram": "AgAAAAAAAIR8CQrXcIhbQAAAAAA=",
                "eci_indicator": "07"
            }
        }}},
        "payment_method": { "display_name": "Visa 1111", "network": "Visa", "type": "debit" },
        "transaction_identifier": "3130323334353637383930313233343536"
    }})
}

#[tokio::test]
async fn wallets_are_one_time_and_old_agreements_are_refused() {
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
        let mut recurring_client = RecurringPaymentServiceClient::new(channel);

        // A card sale that was meant to save the card is refused rather than reporting success
        // for a payment that stored nothing: an agreement is only ever a stored card.
        let mut raw_cit = Request::new(
            serde_json::from_value::<PaymentServiceAuthorizeRequest>(authorize_request(
                card_json(),
                Some("OFF_SESSION"),
            ))
            .expect("authorize request should deserialize"),
        );
        add_elavon_metadata(&mut raw_cit, "raw_card_cit");
        let rejection = Box::pin(payment_client.authorize(raw_cit))
            .await
            .expect_err("a recurring card sale must be refused");
        assert!(
            rejection.message().contains("without a stored card"),
            "{rejection}"
        );

        // So is a repeat on anything but a stored card, such as a mandate saved before stored
        // cards, or a network transaction id, which EPG has no request field for.
        for (suffix, recurring_id, expected) in [
            (
                "credential",
                json!({ "ConnectorMandateId": { "connector_mandate_id": COF_DATA } }),
                "without a stored card",
            ),
            (
                "ntid",
                json!({ "NetworkMandateId": { "network_transaction_id": SCHEME_REF } }),
                "network transaction id",
            ),
        ] {
            let mut charge = Request::new(
                serde_json::from_value::<RecurringPaymentServiceChargeRequest>(json!({
                    "merchant_charge_id": format!("charge_{suffix}"),
                    "connector_recurring_payment_id": { "mandate_id_type": recurring_id },
                    "amount": { "minor_amount": 3500, "currency": "USD" },
                    "payment_method": { "payment_method": card_json() },
                    "capture_method": "AUTOMATIC",
                    "off_session": true,
                    "test_mode": true
                }))
                .expect("charge request should deserialize"),
            );
            add_elavon_metadata(&mut charge, &format!("recurring_{suffix}"));
            let rejection = Box::pin(recurring_client.charge(charge))
                .await
                .expect_err("this repeat must be refused");
            assert!(rejection.message().contains(expected), "{rejection}");
        }

        // Pre-decrypted wallets are one-time sales.
        for (suffix, wallet) in [
            ("google_pay", google_pay_json()),
            ("apple_pay", apple_pay_json()),
            ("mastercard_google_pay", mastercard_google_pay_json()),
        ] {
            let mut wallet_request = Request::new(
                serde_json::from_value::<PaymentServiceAuthorizeRequest>(authorize_request(
                    wallet, None,
                ))
                .expect("wallet request should deserialize"),
            );
            add_elavon_metadata(&mut wallet_request, suffix);
            let wallet_response = Box::pin(payment_client.authorize(wallet_request))
                .await
                .expect("wallet authorize should succeed")
                .into_inner();
            assert_eq!(
                wallet_response.status,
                i32::from(PaymentStatus::Charged),
                "{suffix}"
            );
            assert!(
                wallet_response.mandate_reference_details.is_none(),
                "{suffix}"
            );
        }

        // A wallet cannot be stored, and EPG lists wallets as not allowed for credential on file.
        let mut wallet_agreement = Request::new(
            serde_json::from_value::<PaymentServiceAuthorizeRequest>(authorize_request(
                google_pay_json(),
                Some("OFF_SESSION"),
            ))
            .expect("wallet request should deserialize"),
        );
        add_elavon_metadata(&mut wallet_agreement, "wallet_recurring");
        let rejection = Box::pin(payment_client.authorize(wallet_agreement))
            .await
            .expect_err("a recurring wallet sale must be refused");
        assert!(
            rejection.message().contains("without a stored card"),
            "{rejection}"
        );

        let setup_request = |merchant_recurring_payment_id: &str, minor_amount: i64| {
            serde_json::from_value::<PaymentServiceSetupRecurringRequest>(json!({
                "merchant_recurring_payment_id": merchant_recurring_payment_id,
                "amount": { "minor_amount": minor_amount, "currency": "USD" },
                "payment_method": { "payment_method": card_json() },
                "customer": { "connector_customer_id": SHOPPER_HREF },
                "address": {},
                "auth_type": "NO_THREE_DS",
                "enrolled_for_3ds": false,
                "request_incremental_authorization": false,
                "test_mode": true
            }))
            .expect("setup request should deserialize")
        };

        // A stored card has to belong to a shopper.
        let mut no_shopper = setup_request("setup_no_shopper", 0);
        no_shopper.customer = None;
        let mut no_shopper = Request::new(no_shopper);
        add_elavon_metadata(&mut no_shopper, "setup_no_shopper");
        let rejection = Box::pin(payment_client.setup_recurring(no_shopper))
            .await
            .expect_err("a setup with no shopper must be refused");
        assert!(
            rejection.message().contains("connector_customer_id"),
            "{rejection}"
        );

        // A non-zero setup is refused: `/stored-cards` carries no total, so it would report a
        // charge EPG never made.
        let mut paid_setup = Request::new(setup_request("setup_paid", 1000));
        add_elavon_metadata(&mut paid_setup, "setup_paid");
        let paid_rejection = Box::pin(payment_client.setup_recurring(paid_setup))
            .await
            .expect_err("a non-zero setup mandate must be refused");
        assert!(
            paid_rejection.message().contains("Non-zero amount"),
            "{paid_rejection}"
        );

        let requests = request_log
            .lock()
            .expect("mock request log lock must not be poisoned")
            .clone();
        let sales: Vec<&Value> = requests
            .iter()
            .filter(|request| request.original_url.ends_with("/transactions"))
            .map(|request| &request.body)
            .collect();
        // The 3 one-time wallets; the refused requests never reached EPG.
        assert_eq!(sales.len(), 3, "{sales:?}");
        assert!(requests
            .iter()
            .all(|request| !request.original_url.ends_with("/stored-cards")));

        for (wallet, origin) in sales[..2].iter().zip(["Google Pay", "Apple Pay"]) {
            assert_eq!(wallet["paymentMethodOrigin"], json!(origin));
            assert_eq!(
                wallet["cryptogramSecurity"],
                json!({
                    "cryptogram": "AgAAAAAAAIR8CQrXcIhbQAAAAAA=",
                    "electronicCommerceIndicator": "7"
                })
            );
            assert_eq!(wallet["card"]["number"], json!("4111111111111111"));
            assert_eq!(wallet["card"]["expirationYear"], json!(2030));
            assert!(wallet["card"].get("securityCode").is_none());
            assert!(wallet.get("credentialOnFileType").is_none());
        }

        // The Mastercard token carried an empty eciIndicator: it reaches EPG as the
        // unauthenticated default rather than failing the sale or being sent through blank.
        let mastercard = sales[2];
        assert_eq!(mastercard["paymentMethodOrigin"], json!("Google Pay"));
        assert_eq!(mastercard["card"]["number"], json!("5555555555554444"));
        assert_eq!(
            mastercard["cryptogramSecurity"],
            json!({
                "cryptogram": "AgAAAAAAAIR8CQrXcIhbQAAAAAA=",
                "electronicCommerceIndicator": "7"
            })
        );
    };

    tokio::select! {
        _ = server => panic!("gRPC server stopped before the assertions completed"),
        _ = test => {}
    }

    mock_server.abort();
}

#[tokio::test]
async fn stored_cards_cross_the_grpc_boundary() {
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
        let mut customer_client = CustomerServiceClient::new(channel.clone());
        let mut token_client = PaymentMethodServiceClient::new(channel.clone());
        let mut payment_client = PaymentServiceClient::new(channel.clone());
        let mut recurring_client = RecurringPaymentServiceClient::new(channel);

        // A customer with no name still needs a `fullName` on the shopper.
        let mut shopper = Request::new(
            serde_json::from_value::<CustomerServiceCreateRequest>(json!({
                "merchant_customer_id": "cust_123",
                "test_mode": true
            }))
            .expect("customer request should deserialize"),
        );
        add_elavon_metadata(&mut shopper, "stored_card_shopper");
        let shopper = Box::pin(customer_client.create(shopper))
            .await
            .expect("shopper should be created")
            .into_inner();
        assert_eq!(shopper.connector_customer_id, SHOPPER_HREF);

        let tokenize_request = |card: Value| {
            serde_json::from_value::<PaymentMethodServiceTokenizeRequest>(json!({
                "merchant_payment_method_id": "pm_123",
                "amount": { "minor_amount": AMOUNT, "currency": "USD" },
                "payment_method": { "payment_method": card },
                "customer": { "connector_customer_id": SHOPPER_HREF },
                "address": {},
                "test_mode": true
            }))
            .expect("tokenize request should deserialize")
        };
        let mut token = Request::new(tokenize_request(card_json()));
        add_elavon_metadata(&mut token, "stored_card_token");
        let token = Box::pin(token_client.tokenize(token))
            .await
            .expect("stored card should be created")
            .into_inner();
        assert_eq!(token.payment_method_token, STORED_CARD_HREF);

        // EPG refuses a card the shopper already has. This reaches the caller as an error and
        // no token.
        let mut duplicate_card = card_json();
        duplicate_card["card"]["card_number"] = json!(DUPLICATE_CARD);
        let mut duplicate = Request::new(tokenize_request(duplicate_card));
        add_elavon_metadata(&mut duplicate, "stored_card_duplicate");
        let duplicate = Box::pin(token_client.tokenize(duplicate))
            .await
            .expect("an EPG 400 comes back in the response")
            .into_inner();
        assert!(duplicate.payment_method_token.is_empty());
        assert!(format!("{:?}", duplicate.error).contains("already exists"));

        // A 3DS CIT on the stored card: the href stands in for the card and is the mandate.
        let mut cit_request = authorize_request(
            json!({ "token": { "token": STORED_CARD_HREF } }),
            Some("OFF_SESSION"),
        );
        cit_request["auth_type"] = json!("THREE_DS");
        cit_request["mit_category"] = json!("UNSCHEDULED_MIT");
        cit_request["authentication_data"] = json!({
            "eci": "05",
            "cavv": "AAABBBCCCDDDEEEFFFGGGHHHIII=",
            "message_version": "2.2.0",
            "ds_transaction_id": "ds_123",
            "trans_status": "TRANSACTION_STATUS_SUCCESS"
        });
        let mut cit = Request::new(
            serde_json::from_value::<PaymentServiceAuthorizeRequest>(cit_request)
                .expect("authorize request should deserialize"),
        );
        add_elavon_metadata(&mut cit, "stored_card_cit");
        let cit = Box::pin(payment_client.authorize(cit))
            .await
            .expect("stored card CIT should succeed")
            .into_inner();
        assert_eq!(cit.status, i32::from(PaymentStatus::Charged));
        let cit_mandate = cit
            .mandate_reference_details
            .expect("stored card CIT should return a mandate");
        assert_eq!(
            cit_mandate.connector_mandate_id.as_deref(),
            Some(STORED_CARD_HREF)
        );
        assert!(cit_mandate.mandate_metadata.is_none());

        // A repeat whose mandate is a stored card, once with each category.
        for (index, mit_category) in [
            None,
            Some("RECURRING_MIT"),
            Some("SUBSCRIPTION_MIT"),
            Some("UNSCHEDULED_MIT"),
            Some("INSTALLMENT_MIT"),
            Some("RESUBMISSION_MIT"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut charge_request = json!({
                "merchant_charge_id": format!("charge_stored_card_{index}"),
                "connector_recurring_payment_id": { "mandate_id_type": {
                    "ConnectorMandateId": { "connector_mandate_id": STORED_CARD_HREF }
                }},
                "amount": { "minor_amount": 3500, "currency": "USD" },
                "capture_method": "AUTOMATIC",
                "off_session": true,
                "test_mode": true
            });
            if let Some(mit_category) = mit_category {
                charge_request["mit_category"] = json!(mit_category);
            }
            let mut charge = Request::new(
                serde_json::from_value::<RecurringPaymentServiceChargeRequest>(charge_request)
                    .expect("charge request should deserialize"),
            );
            add_elavon_metadata(&mut charge, &format!("stored_card_mit_{index}"));
            let charge = Box::pin(recurring_client.charge(charge))
                .await
                .expect("stored card repeat should succeed")
                .into_inner();
            assert_eq!(charge.status, i32::from(PaymentStatus::Charged));
        }

        // A zero-amount agreement is a stored card too, and takes the type at creation.
        let mut setup = Request::new(
            serde_json::from_value::<PaymentServiceSetupRecurringRequest>(json!({
                "merchant_recurring_payment_id": "setup_stored_card",
                "mit_category": "SUBSCRIPTION_MIT",
                "amount": { "minor_amount": 0, "currency": "USD" },
                "payment_method": { "payment_method": card_json() },
                "customer": { "connector_customer_id": SHOPPER_HREF },
                "address": {},
                "auth_type": "NO_THREE_DS",
                "enrolled_for_3ds": false,
                "request_incremental_authorization": false,
                "test_mode": true
            }))
            .expect("setup request should deserialize"),
        );
        add_elavon_metadata(&mut setup, "stored_card_setup");
        let setup = Box::pin(payment_client.setup_recurring(setup))
            .await
            .expect("stored card setup should succeed")
            .into_inner();
        assert_eq!(setup.status, i32::from(PaymentStatus::Charged));
        assert_eq!(
            setup
                .mandate_reference_details
                .expect("stored card setup should return a mandate")
                .connector_mandate_id
                .as_deref(),
            Some(STORED_CARD_HREF)
        );

        let requests = request_log
            .lock()
            .expect("mock request log lock must not be poisoned")
            .clone();
        let bodies = |path: &str| -> Vec<Value> {
            requests
                .iter()
                .filter(|request| request.original_url.ends_with(path))
                .map(|request| request.body.clone())
                .collect()
        };

        let shoppers = bodies("/shoppers");
        assert_eq!(shoppers.len(), 1, "{shoppers:?}");
        assert_eq!(shoppers[0]["fullName"], json!("cust_123"));

        // Tokenize, its duplicate, and the zero-amount setup.
        let stored_cards = bodies("/stored-cards");
        assert_eq!(stored_cards.len(), 3, "{stored_cards:?}");
        for stored_card in &stored_cards {
            assert_eq!(stored_card["shopper"], json!(SHOPPER_HREF));
            assert_eq!(stored_card["card"]["securityCode"], json!("123"));
            assert!(stored_card.get("total").is_none());
        }
        assert!(stored_cards[0].get("credentialOnFileType").is_none());
        assert_eq!(
            stored_cards[2]["credentialOnFileType"],
            json!("subscription")
        );

        let sales = bodies("/transactions");
        assert_eq!(sales.len(), 7, "{sales:?}");
        let cit = &sales[0];
        assert_eq!(cit["storedCard"], json!(STORED_CARD_HREF));
        assert_eq!(cit["shopperInteraction"], json!("ecommerce"));
        assert_eq!(cit["credentialOnFileType"], json!("unscheduled"));
        assert_eq!(cit["threeDSecure"]["transactionStatus"], json!("Y"));
        assert!(cit.get("card").is_none());
        for (repeat, credential_on_file_type) in sales[1..].iter().zip([
            "recurring",
            "recurring",
            "subscription",
            "unscheduled",
            "installment",
            "unscheduled",
        ]) {
            assert_eq!(repeat["storedCard"], json!(STORED_CARD_HREF));
            assert_eq!(repeat["shopperInteraction"], json!("merchantInitiated"));
            assert_eq!(
                repeat["credentialOnFileType"],
                json!(credential_on_file_type)
            );
            assert!(repeat.get("card").is_none());
            assert!(repeat.get("credentialOnFileData").is_none());
        }
    };

    tokio::select! {
        _ = server => panic!("gRPC server stopped before the assertions completed"),
        _ = test => {}
    }

    mock_server.abort();
}
