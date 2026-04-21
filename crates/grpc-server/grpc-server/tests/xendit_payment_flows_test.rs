#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use ucs_env::configs;
mod common;
mod utils;

use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use cards::CardNumber;
use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        payment_method, payment_service_client::PaymentServiceClient,
        refund_service_client::RefundServiceClient, AuthenticationType, CaptureMethod, CardDetails,
        Currency, PaymentMethod, PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentStatus, RefundResponse, RefundServiceGetRequest, RefundStatus,
    },
};
use hyperswitch_masking::{ExposeInterface, Secret};
use tonic::{transport::Channel, Request};

// Helper function to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Constants for Xendit connector - Updated to match provided JSON payload
const CONNECTOR_NAME: &str = "xendit";
const MERCHANT_ID: &str = "merchant_1753672298";
const CONNECTOR_CUSTOMER_ID: &str = "abc123";

// Test card data - Updated to match new JSON payload
const TEST_AMOUNT: i64 = 10000000000; // 10 trillion from new payload
const TEST_CARD_NUMBER: &str = "4000000000001091"; // Valid test card for Xendit
const TEST_CARD_EXP_MONTH: &str = "10";
const TEST_CARD_EXP_YEAR: &str = "2027"; // Full year format
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "joseph Doe";
const TEST_EMAIL: &str = "test@t.com";
const TEST_REQUEST_REF_ID: &str = "12345678_123";

fn add_xendit_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load xendit credentials");

    let api_key = match auth {
        domain_types::router_data::ConnectorAuthType::HeaderKey { api_key } => api_key.expose(),
        _ => panic!("Expected HeaderKey auth type for xendit"),
    };

    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "header-key".parse().expect("Failed to parse x-auth"),
    );
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
}

// Helper function to extract connector transaction ID from response
fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    match &response.connector_transaction_id {
        Some(id) => id.clone(),
        None => panic!("Resource ID is None"),
    }
}

// Helper function to extract connector Refund ID from response
fn extract_refund_id(response: &RefundResponse) -> &String {
    &response.connector_refund_id
}

// Helper function to create a payment authorize request
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
        amount:  Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Idr),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some(
            "http://localhost:8080/payments/pay_h6dmtWPxiJ4jgtFpk8JK/merchant_1753672298/redirect/response/novalnet".to_string(),
        ),
        webhook_url: Some(
            "http://localhost:8080/webhooks/merchant_1753672298/mca_8rIwEeXmFvrIA59fMH75".to_string(),
        ),
        address: Some(grpc_api_types::payments::PaymentAddress {
            billing_address: Some(grpc_api_types::payments::Address {
                phone_number: Some(Secret::new("9123456789".to_string())),
                phone_country_code: Some("+91".to_string()),
                email: Some(Secret::new("kalo@hul.com".to_string())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(TEST_REQUEST_REF_ID.to_string()),
        enrolled_for_3ds: Some(true),
        request_incremental_authorization: Some(false),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            name: None,
            id: Some(CONNECTOR_CUSTOMER_ID.to_string()),
            connector_customer_id: Some(CONNECTOR_CUSTOMER_ID.to_string()),
            phone_number: None,
            phone_country_code: None,
        }),
        // browser_info: TODO - BrowserInfo type not available in grpc_api_types
        capture_method: Some(i32::from(capture_method)),
        // payment_method_type: Some(i32::from(PaymentMethodType::Card)),
        ..Default::default()
    }
}

// Helper function to create a payment sync request
fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        encoded_data: None,
        merchant_transaction_id: None,
        capture_method: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Idr),
        }),
        state: None,
        metadata: None,
        connector_feature_data: None,
        setup_future_usage: None,
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
            currency: i32::from(Currency::Idr),
        }),
        multiple_capture_data: None,
        merchant_capture_id: None,
        ..Default::default()
    }
}

// Helper function to create a refund request
fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some(format!("refund_{}", get_timestamp())),
        connector_transaction_id: transaction_id.to_string(),
        payment_amount: TEST_AMOUNT,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Idr),
        }),
        reason: None,
        webhook_url: None,
        browser_info: None,
        merchant_account_id: None,
        capture_method: None,
        ..Default::default()
    }
}

// Helper function to create a refund sync request
fn create_refund_sync_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        merchant_refund_id: None,
        browser_info: None,
        test_mode: Some(true),
        refund_metadata: None,
        state: None,
        connector_feature_data: None,
        payment_method_type: None,
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
async fn test_payment_authorization_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request
        let request = create_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_xendit_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            response.status == i32::from(PaymentStatus::AuthenticationPending)
                || response.status == i32::from(PaymentStatus::Pending)
                || response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in AuthenticationPending, Pending, or Charged state. Got status: {}",
            response.status
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request with manual capture
        let auth_request = create_authorize_request(CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_xendit_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Verify payment status
        assert!(
            auth_response.status == i32::from(PaymentStatus::AuthenticationPending)
                || auth_response.status == i32::from(PaymentStatus::Pending)
                || auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in AuthenticationPending, Pending, or Authorized state. Got status: {}",
            auth_response.status
        );

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Add delay of 15 seconds
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        // Only attempt capture if payment is in AUTHORIZED state
        if auth_response.status == i32::from(PaymentStatus::Authorized) {
            // Create capture request
            let capture_request = create_payment_capture_request(&transaction_id);

            // Add metadata headers for capture request
            let mut capture_grpc_request = Request::new(capture_request);
            add_xendit_metadata(&mut capture_grpc_request);

            // Send the capture request
            let capture_response = client
                .capture(capture_grpc_request)
                .await
                .expect("gRPC payment_capture call failed")
                .into_inner();

            // Verify payment status is charged after capture
            assert!(
                capture_response.status == i32::from(PaymentStatus::Charged),
                "Payment should be in Charged state after capture"
            );
        }
    });
}

// Test payment sync with auto capture
#[tokio::test]
async fn test_payment_sync_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request
        let request = create_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_xendit_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&response);

        // Add delay of 10 seconds
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        // Create sync request
        let sync_request = create_payment_sync_request(&transaction_id);

        // Add metadata headers for sync request
        let mut sync_grpc_request = Request::new(sync_request);
        add_xendit_metadata(&mut sync_grpc_request);

        // Send the sync request
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // Verify the sync response
        assert!(
            sync_response.status == i32::from(PaymentStatus::Charged)
                || sync_response.status == i32::from(PaymentStatus::Pending),
            "Payment should be in Charged or Pending state."
        );
    });
}

// Test refund flow - only attempts refund when payment is in captured/charged state
#[tokio::test]
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request with auto capture
        let request = create_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_xendit_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&response);

        assert!(
            response.status == i32::from(PaymentStatus::AuthenticationPending)
                || response.status == i32::from(PaymentStatus::Pending)
                || response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in AuthenticationPending, Pending, or Charged state"
        );

        // Only attempt refund if payment is already in charged/captured state
        if response.status == i32::from(PaymentStatus::Charged) {
            // Create refund request
            let refund_request = create_refund_request(&transaction_id);

            // Add metadata headers for refund request
            let mut refund_grpc_request = Request::new(refund_request);
            add_xendit_metadata(&mut refund_grpc_request);

            // Send the refund request
            let refund_response = client
                .refund(refund_grpc_request)
                .await
                .expect("gRPC refund call failed")
                .into_inner();

            // Verify the refund response
            assert!(
                refund_response.status == i32::from(RefundStatus::RefundSuccess),
                "Refund should be in RefundSuccess state"
            );
        }
    });
}

// Test refund sync flow - runs as a separate test since refund + sync is complex
#[tokio::test]
#[ignore] // Service not implemented on server side - Status code: Unimplemented
async fn test_refund_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        grpc_test!(refund_client, RefundServiceClient<Channel>, {
            // Create the payment authorization request
            let request = create_authorize_request(CaptureMethod::Automatic);

            // Add metadata headers
            let mut grpc_request = Request::new(request);
            add_xendit_metadata(&mut grpc_request);

            // Send the request
            let response = client
                .authorize(grpc_request)
                .await
                .expect("gRPC authorize call failed")
                .into_inner();

            // Extract the transaction ID
            let transaction_id = extract_transaction_id(&response);

            assert!(
                response.status == i32::from(PaymentStatus::AuthenticationPending)
                    || response.status == i32::from(PaymentStatus::Pending)
                    || response.status == i32::from(PaymentStatus::Charged),
                "Payment should be in AuthenticationPending or Pending state"
            );

            // Wait a bit longer to ensure the payment is fully processed
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            // Create refund request
            let refund_request = create_refund_request(&transaction_id);

            // Add metadata headers for refund request
            let mut refund_grpc_request = Request::new(refund_request);
            add_xendit_metadata(&mut refund_grpc_request);

            // Send the refund request
            let refund_response = client
                .refund(refund_grpc_request)
                .await
                .expect("gRPC refund call failed")
                .into_inner();

            // Verify the refund response
            assert!(
                refund_response.status == i32::from(RefundStatus::RefundSuccess),
                "Refund should be in RefundSuccess state"
            );

            let refund_id = extract_refund_id(&refund_response);

            // Wait a bit longer to ensure the refund is fully processed
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Create refund sync request
            let refund_sync_request = create_refund_sync_request(&transaction_id, refund_id);

            // Add metadata headers for refund sync request
            let mut refund_sync_grpc_request = Request::new(refund_sync_request);
            add_xendit_metadata(&mut refund_sync_grpc_request);

            // Send the refund sync request
            let refund_sync_response = refund_client
                .get(refund_sync_grpc_request)
                .await
                .expect("gRPC refund sync call failed")
                .into_inner();

            // Verify the refund sync response
            assert!(
                refund_sync_response.status == i32::from(RefundStatus::RefundSuccess),
                "Refund Sync should be in RefundSuccess state"
            );
        });
    });
}
