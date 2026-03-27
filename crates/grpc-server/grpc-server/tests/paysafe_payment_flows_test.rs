//! Paysafe Connector Integration Tests
//!
//! This test suite validates the Paysafe connector implementation for UCS v2.
//!
//! ## Running the Tests
//!
//! ### Method 1: Using Environment Variables (Quick Testing)
//! ```bash
//! TEST_PAYSAFE_API_KEY='your_api_key' \
//! TEST_PAYSAFE_KEY1='your_key1' \
//! cargo test --test paysafe_payment_flows_test
//! ```
//!
//! ### Method 2: Using Credentials File (Persistent Setup)
//! Add the following to `.github/test/creds.json`:
//! ```json
//! {
//!   "paysafe": {
//!     "connector_account_details": {
//!       "auth_type": "BodyKey",
//!       "api_key": "your_api_key",
//!       "key1": "your_key1"
//!     },
//!     "metadata": {
//!       "account_id": "1002696790"
//!     }
//!   }
//! }
//! ```
//! Then run:
//! ```bash
//! cargo test --test paysafe_payment_flows_test
//! ```
//!
//! ## Test Coverage
//! - Health check
//! - Payment authorization (auto capture)
//! - Payment authorization (manual capture)
//! - Payment capture
//! - Payment sync (PSync)
//! - Payment void
//! - Refund
//! - Refund sync (RSync)

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use cards::CardNumber;
use grpc_server::app;
use ucs_env::configs;
mod common;
mod utils;
use std::{
    collections::HashMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        direct_payment_service_client::DirectPaymentServiceClient, payment_method,
        refund_service_client::RefundServiceClient, AuthenticationType, CaptureMethod, CardDetails,
        Currency, PaymentMethod, PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentServiceVoidRequest, PaymentStatus, RefundResponse, RefundServiceGetRequest,
        RefundStatus,
    },
};
use hyperswitch_masking::{ExposeInterface, Secret};
use tonic::{transport::Channel, Request};

// Constants for Paysafe connector
const CONNECTOR_NAME: &str = "paysafe";
const AUTH_TYPE: &str = "body-key";
const MERCHANT_ID: &str = "merchant_paysafe_test";

// Test card data - Paysafe test cards
const TEST_AMOUNT: i64 = 1000;
const TEST_CARD_NUMBER: &str = "4000000000001091"; // Paysafe test card
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "30";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Test User";
const TEST_EMAIL: &str = "customer@example.com";

// Helper function to get current timestamp in microseconds for unique IDs
fn get_timestamp_micros() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
}

// Helper function to get current timestamp in seconds
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Helper function to load Paysafe credentials from environment or file
// Returns None if credentials are not available (for skipping tests)
fn load_paysafe_credentials() -> Option<(String, String)> {
    // Try environment variables first (for quick testing)
    if let (Ok(api_key), Ok(key1)) = (
        std::env::var("TEST_PAYSAFE_API_KEY"),
        std::env::var("TEST_PAYSAFE_KEY1"),
    ) {
        return Some((api_key, key1));
    }

    // Fallback to credentials file
    match utils::credential_utils::load_connector_auth(CONNECTOR_NAME) {
        Ok(auth) => match auth {
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
                Some((api_key.expose(), key1.expose()))
            }
            _ => panic!("Expected BodyKey auth type for paysafe"),
        },
        Err(_) => None, // Credentials not found - tests will be skipped
    }
}

// Helper function to add Paysafe metadata headers to a request
// Returns false if credentials are not available
fn add_paysafe_metadata<T>(request: &mut Request<T>) -> bool {
    let Some((api_key, key1)) = load_paysafe_credentials() else {
        return false;
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
    request
        .metadata_mut()
        .append("x-key1", key1.parse().expect("Failed to parse x-key1"));

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

    true
}

// Helper function to extract connector transaction ID from response
fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    match &response.connector_transaction_id {
        Some(id) => id.clone(),
        None => panic!("Transaction ID is None in response: {response:#?}"),
    }
}

// Helper function to create a payment authorization request
fn create_payment_authorize_request(
    capture_method: CaptureMethod,
) -> PaymentServiceAuthorizeRequest {
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_network: Some(1),
        card_issuer: None,
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    };

    // Paysafe requires merchant_account_metadata with account_id mapping
    // This gets converted to connector_meta_data in domain_types
    // The structure should match PaysafeConnectorMetadataObject which has:
    // { "account_id": { "card": { "USD": { "no_three_ds": "..." } } } }
    let merchant_account_metadata_json = serde_json::json!({
        "account_id": {
            "card": {
                "USD": {
                    "no_three_ds": "1002696790"
                }
            }
        }
    })
    .to_string();

    PaymentServiceAuthorizeRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some("https://duck.com".to_string()),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            name: None,
            id: None,
            connector_customer_id: None,
            phone_number: None,
            phone_country_code: None,
        }),
        address: Some(grpc_api_types::payments::PaymentAddress {
            billing_address: Some(grpc_api_types::payments::Address {
                first_name: Some(Secret::new("John".to_string())),
                last_name: Some(Secret::new("Doe".to_string())),
                line1: Some(Secret::new("123 Main St".to_string())),
                line2: None,
                line3: None,
                city: Some(Secret::new("New York".to_string())),
                state: Some(Secret::new("NY".to_string())),
                zip_code: Some(Secret::new("10001".to_string())),
                country_alpha2_code: Some(grpc_api_types::payments::CountryAlpha2::Us.into()),
                email: None,
                phone_number: None,
                phone_country_code: None,
            }),
            shipping_address: None,
        }),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(format!(
            "paysafe_test_{}_{}",
            get_timestamp_micros(),
            uuid::Uuid::new_v4().simple()
        )),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        connector_feature_data: Some(Secret::new(merchant_account_metadata_json)),
        ..Default::default()
    }
}

// Helper function to create a payment sync request
fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        capture_method: None,
        handle_response: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        merchant_transaction_id: None,
        state: None,
        metadata: None,
        connector_feature_data: None,
        setup_future_usage: None,
        encoded_data: None,
        sync_type: None,
        connector_order_reference_id: None,
        test_mode: None,
        payment_experience: None,
    }
}

// Helper function to create a payment capture request
fn create_payment_capture_request(transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: transaction_id.to_string(),
        amount_to_capture: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        multiple_capture_data: None,
        merchant_capture_id: Some(format!(
            "paysafe_capture_{}_{}",
            get_timestamp_micros(),
            uuid::Uuid::new_v4().simple()
        )),
        ..Default::default()
    }
}

// Helper function to create a refund request
fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some(format!("refund_{}", get_timestamp_micros())),
        connector_transaction_id: transaction_id.to_string(),
        payment_amount: TEST_AMOUNT,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        reason: None,
        webhook_url: None,
        browser_info: None,
        merchant_account_id: Some("paysafe_test".to_string()),
        capture_method: None,
        ..Default::default()
    }
}

// Helper function to create a refund sync request
fn create_refund_sync_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    let mut refund_metadata_map = HashMap::new();
    refund_metadata_map.insert("account_id".to_string(), "1002696790".to_string());

    let refund_metadata_json = serde_json::to_string(&refund_metadata_map).unwrap();

    RefundServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        merchant_refund_id: Some(format!("rsync_ref_{}", get_timestamp_micros())),
        browser_info: None,
        test_mode: Some(true),
        refund_metadata: Some(Secret::new(refund_metadata_json)),
        state: None,
        connector_feature_data: None,
        payment_method_type: None,
    }
}

// Helper function to create a payment void request
fn create_payment_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: transaction_id.to_string(),
        cancellation_reason: None,
        merchant_void_id: Some(format!("void_ref_{}", get_timestamp_micros())),
        all_keys_required: None,
        browser_info: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        ..Default::default()
    }
}

// Test health check
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

// Test payment authorization with automatic capture
#[tokio::test]
async fn test_payment_authorization_auto_capture() {
    // Skip test if credentials are not available
    if load_paysafe_credentials().is_none() {
        return;
    }

    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let request = create_payment_authorize_request(CaptureMethod::Automatic);

        let mut grpc_request = Request::new(request);
        assert!(
            add_paysafe_metadata(&mut grpc_request),
            "Failed to add credentials"
        );

        let response = client
            .authorize(grpc_request)
            .await
            .expect("Payment authorization failed");

        let response_inner = response.into_inner();
        // Payment authorization response logged

        // Verify payment status is charged (auto capture)
        assert!(
            response_inner.status == i32::from(PaymentStatus::Charged),
            "Payment should be charged, got status: {}",
            response_inner.status
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    // Skip test if credentials are not available
    if load_paysafe_credentials().is_none() {
        return;
    }

    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // Step 1: Authorize payment with manual capture
        let request = create_payment_authorize_request(CaptureMethod::Manual);
        let mut grpc_request = Request::new(request);
        assert!(
            add_paysafe_metadata(&mut grpc_request),
            "Failed to add credentials"
        );

        let response = client
            .authorize(grpc_request)
            .await
            .expect("Payment authorization failed");

        let response_inner = response.into_inner();
        // Payment authorization response logged

        let transaction_id = extract_transaction_id(&response_inner);

        // Verify payment is in authorized state
        assert_eq!(
            response_inner.status,
            i32::from(PaymentStatus::Authorized),
            "Payment should be in Authorized status"
        );

        // Wait a moment for the payment to be ready for capture
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Step 2: Capture the payment
        let capture_request = create_payment_capture_request(&transaction_id);
        let mut grpc_capture_request = Request::new(capture_request);
        assert!(
            add_paysafe_metadata(&mut grpc_capture_request),
            "Failed to add credentials"
        );

        let capture_response = client
            .capture(grpc_capture_request)
            .await
            .expect("Payment capture failed");

        let capture_response_inner = capture_response.into_inner();
        // Payment capture response logged

        // Verify payment is now charged or pending (Paysafe may return Pending/Processing initially)
        assert!(
            capture_response_inner.status == i32::from(PaymentStatus::Charged)
                || capture_response_inner.status == i32::from(PaymentStatus::Pending),
            "Payment should be charged or pending after capture, got status: {} (expected {} for Charged or {} for Pending). Full response: {:#?}",
            capture_response_inner.status,
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Pending),
            capture_response_inner
        );
    });
}

// Test payment sync
#[tokio::test]
async fn test_payment_sync() {
    // Skip test if credentials are not available
    if load_paysafe_credentials().is_none() {
        return;
    }

    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create a payment to sync
        let request = create_payment_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        assert!(
            add_paysafe_metadata(&mut grpc_request),
            "Failed to add credentials"
        );

        let response = client
            .authorize(grpc_request)
            .await
            .expect("Payment authorization failed");

        let transaction_id = extract_transaction_id(&response.into_inner());

        // Sync the payment
        let sync_request = create_payment_sync_request(&transaction_id);
        let mut grpc_sync_request = Request::new(sync_request);
        assert!(
            add_paysafe_metadata(&mut grpc_sync_request),
            "Failed to add credentials"
        );

        let sync_response = client
            .get(grpc_sync_request)
            .await
            .expect("Payment sync failed");

        let sync_response_inner = sync_response.into_inner();
        // Payment sync response logged

        // Verify sync response has valid status
        assert!(
            sync_response_inner.status == i32::from(PaymentStatus::Charged),
            "Synced payment should have a valid status"
        );
    });
}

// Test refund
// Ignored because Paysafe requires settlements to be batched before refunds can be processed
// This is a test environment limitation, not a code issue
#[tokio::test]
#[ignore]
async fn test_refund() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create and capture a payment
        let request = create_payment_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        add_paysafe_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("Payment authorization failed");

        let transaction_id = extract_transaction_id(&response.into_inner());

        // Wait a moment for payment to settle
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Create refund
        let refund_request = create_refund_request(&transaction_id);
        let mut grpc_refund_request = Request::new(refund_request);
        add_paysafe_metadata(&mut grpc_refund_request);

        let refund_response = client
            .refund(grpc_refund_request)
            .await
            .expect("Refund failed");

        let refund_response_inner = refund_response.into_inner();
        // Refund response logged

        // Verify refund status
        assert!(
            refund_response_inner.status == i32::from(RefundStatus::RefundSuccess)
                || refund_response_inner.status == i32::from(RefundStatus::RefundPending),
            "Refund should be success or pending"
        );
    });
}

// Test refund sync
// Ignored because it depends on refund which is also ignored
#[tokio::test]
#[ignore]
async fn test_refund_sync() {
    grpc_test!(refund_client, RefundServiceClient<Channel>, {
        // First create and capture a payment
        grpc_test!(payment_client, DirectPaymentServiceClient<Channel>, {
            let request = create_payment_authorize_request(CaptureMethod::Automatic);
            let mut grpc_request = Request::new(request);
            add_paysafe_metadata(&mut grpc_request);

            let response = payment_client
                .authorize(grpc_request)
                .await
                .expect("Payment authorization failed");

            let transaction_id = extract_transaction_id(&response.into_inner());

            // Wait for settlement
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Create refund
            let refund_request = create_refund_request(&transaction_id);
            let mut grpc_refund_request = Request::new(refund_request);
            add_paysafe_metadata(&mut grpc_refund_request);

            let refund_response = payment_client
                .refund(grpc_refund_request)
                .await
                .expect("Refund failed");

            let refund_response_inner: RefundResponse = refund_response.into_inner();
            let refund_id = &refund_response_inner.connector_refund_id;

            // Sync the refund
            let refund_sync_request = create_refund_sync_request(&transaction_id, refund_id);
            let mut grpc_refund_sync_request = Request::new(refund_sync_request);
            add_paysafe_metadata(&mut grpc_refund_sync_request);

            let refund_sync_response = refund_client
                .get(grpc_refund_sync_request)
                .await
                .expect("Refund sync failed");

            let refund_sync_response_inner = refund_sync_response.into_inner();
            // Refund sync response logged

            // Verify refund sync status
            assert!(
                refund_sync_response_inner.status == i32::from(RefundStatus::RefundSuccess)
                    || refund_sync_response_inner.status == i32::from(RefundStatus::RefundPending),
                "Refund sync should have valid status"
            );
        });
    });
}

// Test payment void (cancellation)
#[tokio::test]
async fn test_payment_void() {
    // Skip test if credentials are not available
    if load_paysafe_credentials().is_none() {
        return;
    }

    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create a payment with manual capture (so we can void it)
        let request = create_payment_authorize_request(CaptureMethod::Manual);
        let mut grpc_request = Request::new(request);
        assert!(
            add_paysafe_metadata(&mut grpc_request),
            "Failed to add credentials"
        );

        let response = client
            .authorize(grpc_request)
            .await
            .expect("Payment authorization failed");

        let transaction_id = extract_transaction_id(&response.into_inner());

        // Void the payment
        let void_request = create_payment_void_request(&transaction_id);
        let mut grpc_void_request = Request::new(void_request);
        assert!(
            add_paysafe_metadata(&mut grpc_void_request),
            "Failed to add credentials"
        );

        let void_response = client
            .void(grpc_void_request)
            .await
            .expect("Payment void failed");

        let void_response_inner = void_response.into_inner();
        // Payment void response logged

        // Verify payment is voided
        assert_eq!(
            void_response_inner.status,
            i32::from(PaymentStatus::Voided),
            "Payment should be voided after void"
        );
    });
}
