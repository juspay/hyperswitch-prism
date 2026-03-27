#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use ucs_env::configs;
mod common;
mod utils;

use std::time::{SystemTime, UNIX_EPOCH};

use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        payment_method, payment_service_client::PaymentServiceClient, AuthenticationType,
        CaptureMethod, ClassicReward, Currency, PaymentMethod, PaymentServiceAuthorizeRequest,
        PaymentStatus,
    },
};
use tonic::{transport::Channel, Request};

// Helper function to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Constants for Cashtocode connector
const CONNECTOR_NAME: &str = "cashtocode";
const AUTH_TYPE: &str = "currency-auth-key";
const MERCHANT_ID: &str = "merchant_1234";

const TEST_EMAIL: &str = "customer@example.com";

// Test data
const TEST_AMOUNT: i64 = 1000;

fn add_cashtocode_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load cashtocode credentials");

    let auth_key_map = match auth {
        domain_types::router_data::ConnectorAuthType::CurrencyAuthKey { auth_key_map } => {
            auth_key_map
        }
        _ => panic!("Expected CurrencyAuthKey auth type for cashtocode"),
    };

    // Serialize the auth_key_map to JSON for metadata
    let auth_key_map_json =
        serde_json::to_string(&auth_key_map).expect("Failed to serialize auth_key_map");

    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request
        .metadata_mut()
        .append("x-auth", AUTH_TYPE.parse().expect("Failed to parse x-auth"));
    request.metadata_mut().append(
        "x-auth-key-map",
        auth_key_map_json
            .parse()
            .expect("Failed to parse x-auth-key-map"),
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
}

// Helper function to create a payment authorize request
fn create_authorize_request(capture_method: CaptureMethod) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Eur),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::ClassicReward(
                ClassicReward {},
            )),
        }),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            name: None,
            id: Some("cust_1233".to_string()),
            connector_customer_id: Some("cust_1233".to_string()),
            phone_number: None,
            phone_country_code: None,
        }),
        return_url: Some("https://hyperswitch.io/connector-service".to_string()),
        webhook_url: Some("https://hyperswitch.io/connector-service".to_string()),
        address: Some(grpc_api_types::payments::PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(format!("cashtocode_test_{}", get_timestamp())),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        ..Default::default()
    }
}

// Test for basic health check
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

// Test payment authorization with auto capture
#[tokio::test]
#[ignore] // skip in CI
async fn test_payment_authorization() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request
        let request = create_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_cashtocode_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            response.status == i32::from(PaymentStatus::AuthenticationPending),
            "Payment should be in AuthenticationPending state"
        );
    });
}
