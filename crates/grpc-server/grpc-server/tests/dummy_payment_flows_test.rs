#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use ucs_env::configs;
mod common;
mod utils;

use std::{
    collections::HashMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use cards::CardNumber;
use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        payment_method, payment_service_client::PaymentServiceClient,
        refund_service_client::RefundServiceClient, AuthenticationType, CaptureMethod, CardDetails,
        Currency, HttpMethod, PaymentMethod, PaymentServiceAuthorizeRequest,
        PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceGetRequest,
        PaymentServiceRefundRequest, PaymentServiceVerifyRedirectResponseRequest,
        PaymentServiceVoidRequest, PaymentStatus, RefundResponse, RefundServiceGetRequest,
        RefundStatus, RequestDetails,
    },
};
use tonic::{transport::Channel, Request};
use uuid::Uuid;

// Helper to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Helper to generate a unique ID using UUID
fn generate_unique_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4())
}

// Constants for the Dummy connector. Dummy is a self-hosted mock so the api
// key value is unused; HeaderKey is required to match the auth scheme the
// connector's `DummyAuthType::TryFrom<ConnectorSpecificConfig>` expects.
const CONNECTOR_NAME: &str = "dummy";
const AUTH_TYPE: &str = "header-key";
const MERCHANT_ID: &str = "merchant_1234";

// Test data. Card numbers match the values used in
// `crates/internal/integration-tests/src/connector_specs/dummy/override.json`
// so the same scenarios can be exercised by either the JSON spec runner or
// this Rust client.
const TEST_AMOUNT: i64 = 1000;
const TEST_CARD_NUMBER: &str = "4111111111111111";
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "2050";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Test User";
const TEST_EMAIL: &str = "customer@example.com";

fn add_dummy_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load Dummy credentials");

    let api_key = match auth {
        domain_types::router_data::ConnectorAuthType::HeaderKey { api_key } => api_key.expose(),
        _ => panic!("Expected HeaderKey auth type for Dummy"),
    };

    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request
        .metadata_mut()
        .append("x-auth", AUTH_TYPE.parse().expect("Failed to parse x-auth"));
    request.metadata_mut().append(
        "x-api-key",
        api_key.parse().expect("Failed to parse x-api-key"),
    );
    request.metadata_mut().append(
        "x-merchant-id",
        MERCHANT_ID.parse().expect("Failed to parse x-merchant-id"),
    );
    request.metadata_mut().append(
        "x-request-id",
        format!("test_request_{}", get_timestamp())
            .parse()
            .expect("Failed to parse x-request-id"),
    );
    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );
    request.metadata_mut().append(
        "x-connector-request-reference-id",
        format!("conn_ref_{}", get_timestamp())
            .parse()
            .expect("Failed to parse x-connector-request-reference-id"),
    );
}

fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    response
        .connector_transaction_id
        .clone()
        .expect("connector_transaction_id is None")
}

fn extract_refund_id(response: &RefundResponse) -> &String {
    &response.connector_refund_id
}

fn create_authorize_request(capture_method: CaptureMethod) -> PaymentServiceAuthorizeRequest {
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_issuer: None,
        card_network: Some(1),
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    };
    PaymentServiceAuthorizeRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some("https://hyperswitch.io/connector-service/dummy_redirect".to_string()),
        webhook_url: Some("https://hyperswitch.io/connector-service/dummy_webhook".to_string()),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            name: None,
            id: Some("cus_dummy_test".to_string()),
            connector_customer_id: Some("cus_dummy_test".to_string()),
            phone_number: None,
            phone_country_code: None,
        }),
        address: Some(grpc_api_types::payments::PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(generate_unique_id("dummy_test")),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        ..Default::default()
    }
}

fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        encoded_data: None,
        capture_method: None,
        merchant_transaction_id: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        state: None,
        metadata: None,
        connector_feature_data: None,
        setup_future_usage: None,
        sync_type: None,
        connector_order_reference_id: None,
        test_mode: None,
        payment_experience: None,
        merchant_request_id: None,
    }
}

fn create_payment_capture_request(transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: transaction_id.to_string(),
        amount_to_capture: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        multiple_capture_data: None,
        merchant_capture_id: None,
        ..Default::default()
    }
}

fn create_payment_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: transaction_id.to_string(),
        cancellation_reason: None,
        merchant_void_id: Some(generate_unique_id("dummy_void")),
        all_keys_required: None,
        browser_info: None,
        amount: None,
        ..Default::default()
    }
}

fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some(format!("refund_{}", generate_unique_id("test"))),
        connector_transaction_id: transaction_id.to_string(),
        payment_amount: TEST_AMOUNT,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        reason: None,
        browser_info: None,
        merchant_account_id: None,
        capture_method: None,
        webhook_url: Some("https://hyperswitch.io/connector-service/dummy_webhook".to_string()),
        ..Default::default()
    }
}

fn create_refund_sync_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        merchant_refund_id: None,
        ..Default::default()
    }
}

// VerifyRedirectResponse is unique to the Dummy connector. The mock parses
// `dummy_status` and `dummy_id` from `request_details.query_params` (not the
// URI's query string) to decide whether the redirect is valid.
fn create_verify_redirect_request(
    uri: &str,
    query_params: &str,
) -> PaymentServiceVerifyRedirectResponseRequest {
    PaymentServiceVerifyRedirectResponseRequest {
        merchant_order_id: Some(generate_unique_id("dummy_redirect")),
        request_details: Some(RequestDetails {
            method: i32::from(HttpMethod::Get),
            uri: Some(uri.to_string()),
            headers: HashMap::new(),
            body: vec![],
            query_params: Some(query_params.to_string()),
        }),
        redirect_response_secrets: None,
    }
}

#[tokio::test]
async fn test_health() {
    grpc_test!(client, HealthClient<Channel>, {
        let response = client
            .check(Request::new(HealthCheckRequest {
                service: "connector_service".to_string(),
            }))
            .await
            .expect("Failed to call health check")
            .into_inner();

        assert_eq!(
            response.status(),
            grpc_api_types::health_check::health_check_response::ServingStatus::Serving
        );
    });
}

#[tokio::test]
async fn test_payment_authorization_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let request = create_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        add_dummy_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state, got status={}",
            response.status
        );
    });
}

#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Manual);
        let mut auth_grpc_request = Request::new(auth_request);
        add_dummy_metadata(&mut auth_grpc_request);

        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in Authorized state, got status={}",
            auth_response.status
        );

        let transaction_id = extract_transaction_id(&auth_response);
        let capture_request = create_payment_capture_request(&transaction_id);
        let mut capture_grpc_request = Request::new(capture_request);
        add_dummy_metadata(&mut capture_grpc_request);

        let capture_response = client
            .capture(capture_grpc_request)
            .await
            .expect("gRPC capture call failed")
            .into_inner();

        assert!(
            capture_response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state after capture, got status={}",
            capture_response.status
        );
    });
}

#[tokio::test]
async fn test_payment_sync_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let request = create_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        add_dummy_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        let transaction_id = extract_transaction_id(&response);
        let sync_request = create_payment_sync_request(&transaction_id);
        let mut sync_grpc_request = Request::new(sync_request);
        add_dummy_metadata(&mut sync_grpc_request);

        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC psync call failed")
            .into_inner();

        assert!(
            sync_response.status == i32::from(PaymentStatus::Charged),
            "Synced payment should be in Charged state, got status={}",
            sync_response.status
        );
    });
}

#[tokio::test]
async fn test_payment_void() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Manual);
        let mut auth_grpc_request = Request::new(auth_request);
        add_dummy_metadata(&mut auth_grpc_request);

        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        let transaction_id = extract_transaction_id(&auth_response);

        assert!(
            auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in Authorized state before voiding"
        );

        let void_request = create_payment_void_request(&transaction_id);
        let mut void_grpc_request = Request::new(void_request);
        add_dummy_metadata(&mut void_grpc_request);

        let void_response = client
            .void(void_grpc_request)
            .await
            .expect("gRPC void call failed")
            .into_inner();

        assert!(
            !void_response.connector_transaction_id.is_empty(),
            "Transaction ID should be present in void response"
        );
        assert!(
            void_response.status == i32::from(PaymentStatus::Voided),
            "Payment should be in Voided state after void, got status={}",
            void_response.status
        );

        let sync_request = create_payment_sync_request(&transaction_id);
        let mut sync_grpc_request = Request::new(sync_request);
        add_dummy_metadata(&mut sync_grpc_request);

        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC psync call failed")
            .into_inner();

        assert!(
            sync_response.status == i32::from(PaymentStatus::Voided),
            "Synced payment should be in Voided state, got status={}",
            sync_response.status
        );
    });
}

#[tokio::test]
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let request = create_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        add_dummy_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        let transaction_id = extract_transaction_id(&response);

        assert!(
            response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state before refund"
        );

        let refund_request = create_refund_request(&transaction_id);
        let mut refund_grpc_request = Request::new(refund_request);
        add_dummy_metadata(&mut refund_grpc_request);

        let refund_response = client
            .refund(refund_grpc_request)
            .await
            .expect("gRPC refund call failed")
            .into_inner();

        assert!(
            refund_response.status == i32::from(RefundStatus::RefundSuccess),
            "Refund should be in RefundSuccess state, got status={}",
            refund_response.status
        );
    });
}

#[tokio::test]
async fn test_refund_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        grpc_test!(refund_client, RefundServiceClient<Channel>, {
            let request = create_authorize_request(CaptureMethod::Automatic);
            let mut grpc_request = Request::new(request);
            add_dummy_metadata(&mut grpc_request);

            let response = client
                .authorize(grpc_request)
                .await
                .expect("gRPC authorize call failed")
                .into_inner();

            let transaction_id = extract_transaction_id(&response);

            let refund_request = create_refund_request(&transaction_id);
            let mut refund_grpc_request = Request::new(refund_request);
            add_dummy_metadata(&mut refund_grpc_request);

            let refund_response = client
                .refund(refund_grpc_request)
                .await
                .expect("gRPC refund call failed")
                .into_inner();

            let refund_id = extract_refund_id(&refund_response);
            let refund_sync_request = create_refund_sync_request(&transaction_id, refund_id);
            let mut refund_sync_grpc_request = Request::new(refund_sync_request);
            add_dummy_metadata(&mut refund_sync_grpc_request);

            let refund_sync_response = refund_client
                .get(refund_sync_grpc_request)
                .await
                .expect("gRPC refund sync call failed")
                .into_inner();

            assert!(
                refund_sync_response.status == i32::from(RefundStatus::RefundSuccess),
                "Refund sync should be in RefundSuccess state, got status={}",
                refund_sync_response.status
            );
        });
    });
}

#[tokio::test]
async fn test_verify_redirect_response_success() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let request = create_verify_redirect_request(
            "https://example.com/payment/redirect?dummy_status=success&dummy_id=DUMMY-pi_test_12345",
            "dummy_status=success&dummy_id=DUMMY-pi_test_12345",
        );
        let mut grpc_request = Request::new(request);
        add_dummy_metadata(&mut grpc_request);

        let response = client
            .verify_redirect_response(grpc_request)
            .await
            .expect("gRPC verify_redirect_response call failed")
            .into_inner();

        assert!(
            response.source_verified,
            "VerifyRedirectResponse should report source_verified=true for a valid dummy redirect"
        );
    });
}
