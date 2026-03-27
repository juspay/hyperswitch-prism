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
        identifier::IdType, payment_method, payment_service_client::PaymentServiceClient,
        refund_service_client::RefundServiceClient, AuthenticationType, CaptureMethod, CardDetails,
        Currency, Identifier, PaymentMethod, PaymentServiceAuthorizeRequest,
        PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest, PaymentServiceGetRequest,
        PaymentServiceRefundRequest, PaymentServiceVoidRequest, PaymentStatus, RefundResponse,
        RefundServiceGetRequest, RefundStatus,
    },
};
use hyperswitch_masking::{ExposeInterface, Secret};
use tonic::{transport::Channel, Request};

// Constants for Braintree connector
const CONNECTOR_NAME: &str = "braintree";
const AUTH_TYPE: &str = "signature-key";
const MERCHANT_ID: &str = "merchant_17555143863";

// Test card data
const TEST_AMOUNT: i64 = 1000;
const TEST_CARD_NUMBER: &str = "4242424242424242"; // Valid test card for Braintree
const TEST_CARD_EXP_MONTH: &str = "10";
const TEST_CARD_EXP_YEAR: &str = "25";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Test User";
const TEST_EMAIL: &str = "customer@example.com";

// Helper function to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Helper function to add Braintree metadata headers to a request
fn add_braintree_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load braintree credentials");

    let (api_key, key1, api_secret) = match auth {
        domain_types::router_data::ConnectorAuthType::SignatureKey {
            api_key,
            key1,
            api_secret,
        } => (api_key.expose(), key1.expose(), api_secret.expose()),
        _ => panic!("Expected SignatureKey auth type for braintree"),
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
        "x-api-secret",
        api_secret.parse().expect("Failed to parse x-api-secret"),
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

// Helper function to extract connector transaction ID from response
fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    match &response.transaction_id {
        Some(id) => match id.id_type.as_ref().unwrap() {
            IdType::Id(id) => id.clone(),
            _ => panic!("Expected connector transaction ID"),
        },
        None => panic!("Resource ID is None"),
    }
}

// Helper function to extract connector Refund ID from response
fn extract_refund_id(response: &RefundResponse) -> &String {
    &response.refund_id
}

// Helper function to create a payment authorize request
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
    let mut merchant_account_metadata_map = HashMap::new();
    merchant_account_metadata_map.insert("merchant_account_id".to_string(), "Anand".to_string());
    merchant_account_metadata_map.insert("merchant_config_currency".to_string(), "USD".to_string());
    merchant_account_metadata_map.insert("currency".to_string(), "USD".to_string());
    let merchant_account_metadata_map_json =
        serde_json::to_string(&merchant_account_metadata_map).unwrap();
    PaymentServiceAuthorizeRequest {
        amount: TEST_AMOUNT,
        minor_amount: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some("https://duck.com".to_string()),
        email: Some(TEST_EMAIL.to_string().into()),
        address: Some(grpc_api_types::payments::PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("braintree_test_{}", get_timestamp()))),
        }),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        metadata: None,
        merchant_account_metadata: Some(Secret::new(merchant_account_metadata_map_json)),
        // payment_method_type: Some(i32::from(PaymentMethodType::Card)),
        ..Default::default()
    }
}

// Helper function to create a payment sync request
fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        encoded_data: None,
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("braintree_sync_{}", get_timestamp()))),
        }),
        capture_method: None,
        handle_response: None,
        amount: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
        state: None,
        metadata: None,
        merchant_account_metadata: None,
        connector_metadata: None,
        setup_future_usage: None,
        sync_type: None,
        connector_order_reference_id: None,
        test_mode: None,
    }
}

// Helper function to create a payment capture request
fn create_payment_capture_request(transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        amount_to_capture: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
        multiple_capture_data: None,
        request_ref_id: None,
        ..Default::default()
    }
}

// Helper function to create a payment void request
fn create_payment_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        cancellation_reason: None,
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("void_ref_{}", get_timestamp()))),
        }),
        all_keys_required: None,
        browser_info: None,
        amount: None,
        currency: None,
        ..Default::default()
    }
}

// Helper function to create a refund request
fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        refund_id: format!("refund_{}", get_timestamp()),
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        currency: i32::from(Currency::Usd),
        payment_amount: TEST_AMOUNT,
        refund_amount: TEST_AMOUNT,
        minor_payment_amount: TEST_AMOUNT,
        minor_refund_amount: TEST_AMOUNT,
        reason: None,
        webhook_url: None,
        browser_info: None,
        merchant_account_id: Some("Anand".to_string()),
        capture_method: None,
        request_ref_id: None,
        ..Default::default()
    }
}

// Helper function to create a refund sync request
fn create_refund_sync_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    let mut merchant_account_metadata_map = HashMap::new();
    merchant_account_metadata_map.insert("merchant_account_id".to_string(), "Anand".to_string());
    merchant_account_metadata_map.insert("merchant_config_currency".to_string(), "USD".to_string());
    merchant_account_metadata_map.insert("currency".to_string(), "USD".to_string());

    let merchant_account_metadata_json =
        serde_json::to_string(&merchant_account_metadata_map).unwrap();

    RefundServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("rsync_ref_{}", get_timestamp()))),
        }),
        browser_info: None,
        test_mode: Some(true),
        refund_metadata: None,
        state: None,
        merchant_account_metadata: Some(Secret::new(merchant_account_metadata_json)),
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
        let request = create_payment_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_braintree_metadata(&mut grpc_request);

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
            "Payment should be in AuthenticationPending or Pending or Charged state"
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 4 seconds
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        // Create the payment authorization request with manual capture
        let auth_request = create_payment_authorize_request(CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_braintree_metadata(&mut auth_grpc_request);

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
            "Payment should be in AuthenticationPending or Pending state"
        );

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Create capture request
        let capture_request = create_payment_capture_request(&transaction_id);

        // Add metadata headers for capture request - make sure they include the terminal_id
        let mut capture_grpc_request = Request::new(capture_request);
        add_braintree_metadata(&mut capture_grpc_request);

        // Send the capture request
        let capture_response = client
            .capture(capture_grpc_request)
            .await
            .expect("gRPC payment_capture call failed")
            .into_inner();

        // Verify payment status is charged after capture
        assert!(
            capture_response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in CHARGED state after capture"
        );
    });
}

// Test payment sync with auto capture
#[tokio::test]
async fn test_payment_sync_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 8 seconds
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;

        // Create the payment authorization request
        let request = create_payment_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_braintree_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&response);

        // Create sync request
        let sync_request = create_payment_sync_request(&transaction_id);

        // Add metadata headers for sync request
        let mut sync_grpc_request = Request::new(sync_request);
        add_braintree_metadata(&mut sync_grpc_request);

        // Send the sync request
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // Verify the sync response
        assert!(
            sync_response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state"
        );
    });
}

// Test payment void
#[tokio::test]
async fn test_payment_void() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 12 seconds
        tokio::time::sleep(std::time::Duration::from_secs(12)).await;

        // First create a payment with manual capture to void
        let auth_request = create_payment_authorize_request(CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_braintree_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status
        assert!(
            auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in AUTHORIZED state before voiding"
        );

        // Create void request with a unique reference ID
        let void_request = create_payment_void_request(&transaction_id);

        // Add metadata headers for void request
        let mut void_grpc_request = Request::new(void_request);
        add_braintree_metadata(&mut void_grpc_request);

        // Send the void request
        let void_response = client
            .void(void_grpc_request)
            .await
            .expect("gRPC void_payment call failed")
            .into_inner();

        // Verify the void response

        assert!(
            void_response.status == i32::from(PaymentStatus::Voided),
            "Payment should be in VOIDED state after void"
        );

        // Verify the payment status with a sync operation
        let sync_request = create_payment_sync_request(&transaction_id);
        let mut sync_grpc_request = Request::new(sync_request);
        add_braintree_metadata(&mut sync_grpc_request);

        // Send the sync request to verify void status
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // Verify the payment is properly voided
        assert!(
            sync_response.status == i32::from(PaymentStatus::Voided),
            "Payment should be in VOIDED state after void sync"
        );
    });
}

// Test refund flow - handles both success and error cases
// Ignored as refund status in Settling or Settled state may take time in Braintree sandbox.
#[tokio::test]
#[ignore]
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 16 seconds
        tokio::time::sleep(std::time::Duration::from_secs(16)).await;

        // Create the payment authorization request
        let request = create_payment_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_braintree_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&response);

        assert!(
            response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state"
        );

        // Create refund request
        let refund_request = create_refund_request(&transaction_id);

        // Add metadata headers for refund request
        let mut refund_grpc_request = Request::new(refund_request);
        add_braintree_metadata(&mut refund_grpc_request);

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
    });
}

// Test refund sync flow - runs as a separate test since refund + sync is complex
// Ignored as refund status in Settling or Settled state may take time in Braintree sandbox.
#[tokio::test]
#[ignore]
async fn test_refund_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        grpc_test!(refund_client, RefundServiceClient<Channel>, {
            // Add delay of 20 seconds
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;

            // Create the payment authorization request
            let request = create_payment_authorize_request(CaptureMethod::Automatic);

            // Add metadata headers
            let mut grpc_request = Request::new(request);
            add_braintree_metadata(&mut grpc_request);

            // Send the request
            let response = client
                .authorize(grpc_request)
                .await
                .expect("gRPC authorize call failed")
                .into_inner();

            // Extract the transaction ID
            let transaction_id = extract_transaction_id(&response);

            assert!(
                response.status == i32::from(PaymentStatus::Charged),
                "Payment should be in Charged state"
            );

            // Create refund request
            let refund_request = create_refund_request(&transaction_id);

            // Add metadata headers for refund request
            let mut refund_grpc_request = Request::new(refund_request);
            add_braintree_metadata(&mut refund_grpc_request);

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

            // Create refund sync request
            let refund_sync_request = create_refund_sync_request(&transaction_id, refund_id);

            // Add metadata headers for refund sync request
            let mut refund_sync_grpc_request = Request::new(refund_sync_request);
            add_braintree_metadata(&mut refund_sync_grpc_request);

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
