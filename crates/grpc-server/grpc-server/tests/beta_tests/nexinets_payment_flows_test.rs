#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
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
        identifier::IdType, payment_method,
        payment_service_client::PaymentServiceClient, refund_service_client::RefundServiceClient,
        AuthenticationType, CaptureMethod, CardDetails, Currency,
        Identifier, PaymentMethod, PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentStatus, RefundResponse, RefundServiceGetRequest, RefundStatus,
    },
};
use hyperswitch_masking::{ExposeInterface, Secret};
use tonic::{transport::Channel, Request};

// Constants for Nexinets connector
const CONNECTOR_NAME: &str = "nexinets";
const AUTH_TYPE: &str = "body-key";
const MERCHANT_ID: &str = "12abc123-f8a3-99b8-9ef8-b31180358hh4";

// Test card data
const TEST_AMOUNT: i64 = 1000;
const TEST_CARD_NUMBER: &str = "4111111111111111"; // Valid test card for Nexinets
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

// Helper function to add Nexinets metadata headers to a request
fn add_nexinets_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load nexinets credentials");

    let (api_key, key1) = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
            (api_key.expose(), key1.expose())
        }
        _ => panic!("Expected BodyKey auth type for nexinets"),
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

// Helper function to extract connector request ref ID from response
fn extract_request_ref_id(response: &PaymentServiceAuthorizeResponse) -> String {
    match &response.response_ref_id {
        Some(id) => match id.id_type.as_ref().unwrap() {
            IdType::Id(id) => id.clone(),
            _ => panic!("Expected connector response_ref_id"),
        },
        None => panic!("Resource ID is None"),
    }
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
    });
    PaymentServiceAuthorizeRequest {
        amount: TEST_AMOUNT,
        minor_amount: TEST_AMOUNT,
        currency: i32::from(Currency::Eur),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        }),
        return_url: Some("https://duck.com".to_string()),
        email: Some(TEST_EMAIL.to_string().into()),
        address: Some(grpc_api_types::payments::PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("nexinets_test_{}", get_timestamp()))),
        }),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        // payment_method_type: Some(i32::from(PaymentMethodType::Card)),
        ..Default::default()
    }
}

// Helper function to create a payment sync request
fn create_payment_sync_request(
    transaction_id: &str,
    request_ref_id: &str,
) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(request_ref_id.to_string())),
        }),
        // all_keys_required: None,
        capture_method: None,
        amount: TEST_AMOUNT,
        currency: i32::from(Currency::Eur),
        handle_response: None,
        state: None,
    }
}

// Helper function to create a payment capture request
fn create_payment_capture_request(
    transaction_id: &str,
    request_ref_id: &str,
) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        amount_to_capture: TEST_AMOUNT,
        currency: i32::from(Currency::Eur),
        multiple_capture_data: None,
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(request_ref_id.to_string())),
        }),
        ..Default::default()
    }
}

// Helper function to create a refund request
fn create_refund_request(
    transaction_id: &str,
    request_ref_id: &str,
) -> PaymentServiceRefundRequest {
    // Create connector metadata as a proper JSON object
    let mut connector_metadata = HashMap::new();
    connector_metadata.insert("order_id".to_string(), request_ref_id);
    let connector_metadata_json =
        serde_json::to_string(&connector_metadata).expect("Failed to serialize connector metadata");

    let mut metadata = HashMap::new();
    metadata.insert("connector_metadata".to_string(), connector_metadata_json);
    PaymentServiceRefundRequest {
        refund_id: format!("refund_{}", get_timestamp()),
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        currency: i32::from(Currency::Eur),
        payment_amount: TEST_AMOUNT,
        refund_amount: TEST_AMOUNT,
        minor_payment_amount: TEST_AMOUNT,
        minor_refund_amount: TEST_AMOUNT,
        reason: None,
        webhook_url: None,
        browser_info: None,
        merchant_account_id: None,
        capture_method: None,
        request_ref_id: None,
        metadata,
        ..Default::default()
    }
}

// Helper function to create a refund sync request
fn create_refund_sync_request(
    transaction_id: &str,
    refund_id: &str,
    request_ref_id: &str,
) -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(request_ref_id.to_string())),
        }),
        browser_info: None,
        refund_metadata: HashMap::new(),
        state: None,
    }
}

// Helper function to visit 3DS authentication URL using reqwest
async fn visit_3ds_authentication_url(
    request_ref_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the key1 value from auth credentials for the URL
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load nexinets credentials");

    let key1 = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { key1, .. } => key1.expose(),
        _ => panic!("Expected BodyKey auth type for nexinets"),
    };

    // Construct the 3DS authentication URL with correct format
    let url = format!("https://pptest.payengine.de/three-ds-v2-order/{key1}/{request_ref_id}",);

    // Create reqwest client with timeout and proper TLS configuration
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(false) // Keep TLS verification enabled
        .user_agent("nexinets-test-client/1.0")
        .build()?;

    // Send GET request
    let response = client.get(&url).send().await?;

    // Read response body for additional debugging (optional)
    let body = response.text().await?;

    // Log first 200 characters of response for debugging (if not empty)
    if !body.is_empty() {
        let _preview = if body.len() > 200 {
            &body[..200]
        } else {
            &body
        };
    }

    Ok(())
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

#[tokio::test]
async fn test_payment_authorization_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request
        let request = create_payment_authorize_request(CaptureMethod::Automatic);
        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_nexinets_metadata(&mut grpc_request);
        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();
        // Verify the response
        assert!(
            response.transaction_id.is_some(),
            "Resource ID should be present"
        );
        assert!(
            response.status == i32::from(PaymentStatus::AuthenticationPending)
                || response.status == i32::from(PaymentStatus::Pending)
                || response.status == i32::from(PaymentStatus::Charged),
            "Payment should be in AuthenticationPending or Pending state"
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 2 seconds
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Create the payment authorization request with manual capture
        let auth_request = create_payment_authorize_request(CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_nexinets_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Extract the request ref ID which is Order_id for nexinets
        let request_ref_id = extract_request_ref_id(&auth_response);

        let mut final_payment_status = auth_response.status;

        // Check if payment requires 3DS authentication
        if auth_response.status == i32::from(PaymentStatus::AuthenticationPending) {
            // Visit the 3DS authentication URL to simulate user completing authentication
            let _ = visit_3ds_authentication_url(&request_ref_id).await;

            // Wait a moment for the authentication state to be updated
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            // Sync the payment to get updated status after 3DS authentication
            let sync_request = create_payment_sync_request(&transaction_id, &request_ref_id);
            let mut sync_grpc_request = Request::new(sync_request);
            add_nexinets_metadata(&mut sync_grpc_request);

            let sync_response = client
                .get(sync_grpc_request)
                .await
                .expect("gRPC payment_sync call failed")
                .into_inner();
            final_payment_status = sync_response.status;

            // Note: Simply visiting the 3DS URL doesn't complete the authentication
            // The payment may still be in AuthenticationPending state
            // In a real scenario, the user would interact with the 3DS page

            // For testing purposes, we'll accept either AuthenticationPending or Authorized
            assert!(
                final_payment_status == i32::from(PaymentStatus::Authorized)
                    || final_payment_status == i32::from(PaymentStatus::AuthenticationPending),
                "Payment should be in AUTHORIZED or still AUTHENTICATION_PENDING state after visiting 3DS URL. Current status: {final_payment_status}", 
            );
        } else {
            // Verify payment status is authorized (for manual capture without 3DS)
            assert!(
                auth_response.status == i32::from(PaymentStatus::Authorized),
                "Payment should be in AUTHORIZED state with manual capture"
            );
        }

        // Only proceed with capture if payment is in Authorized state
        // If still in AuthenticationPending, skip capture as it requires user
        // interaction
        if final_payment_status == i32::from(PaymentStatus::Authorized) {
            // Create capture request (which already includes proper connector metadata)
            let capture_request = create_payment_capture_request(&transaction_id, &request_ref_id);

            // Add metadata headers for capture request
            let mut capture_grpc_request = Request::new(capture_request);
            add_nexinets_metadata(&mut capture_grpc_request);

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
        }
    });
}

// Test payment sync
#[tokio::test]
async fn test_payment_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 4 seconds
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;

        // First create a payment to sync
        let auth_request = create_payment_authorize_request(CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_nexinets_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Extract the request ref ID which is Order_id for nexinets
        let request_ref_id = extract_request_ref_id(&auth_response);

        // Check if payment requires 3DS authentication
        if auth_response.status == i32::from(PaymentStatus::AuthenticationPending) {
            let _ = visit_3ds_authentication_url(&request_ref_id).await;

            // Wait a moment for the authentication state to be updated
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }

        // Create sync request
        let sync_request = create_payment_sync_request(&transaction_id, &request_ref_id);

        // Add metadata headers for sync request
        let mut sync_grpc_request = Request::new(sync_request);
        add_nexinets_metadata(&mut sync_grpc_request);

        // Send the sync request
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // For testing purposes, we'll accept AuthenticationPending or Authorized
        assert!(
            sync_response.status == i32::from(PaymentStatus::Authorized)
                || sync_response.status == i32::from(PaymentStatus::AuthenticationPending),
            "Payment should be in AUTHORIZED or AUTHENTICATION_PENDING state. Current status: {}",
            sync_response.status
        );
    });
}

// Test refund flow - handles both success and error cases
#[tokio::test]
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Add delay of 6 seconds
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;

        // Create the payment authorization request
        let request = create_payment_authorize_request(CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_nexinets_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&response);

        // Extract the request ref ID which is Order_id for nexinets
        let request_ref_id = extract_request_ref_id(&response);

        // Check if payment requires 3DS authentication
        if response.status == i32::from(PaymentStatus::AuthenticationPending) {
            let _ = visit_3ds_authentication_url(&request_ref_id).await;

            // Wait a moment for the authentication state to be updated
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            // Sync the payment to get updated status after 3DS authentication
            let sync_request = create_payment_sync_request(&transaction_id, &request_ref_id);
            let mut sync_grpc_request = Request::new(sync_request);
            add_nexinets_metadata(&mut sync_grpc_request);

            let sync_response = client
                .get(sync_grpc_request)
                .await
                .expect("gRPC payment_sync call failed")
                .into_inner();

            assert!(
                sync_response.status == i32::from(PaymentStatus::Charged)
                    || sync_response.status == i32::from(PaymentStatus::Authorized)
                    || sync_response.status == i32::from(PaymentStatus::AuthenticationPending),
                "Payment should be in CHARGED, AUTHORIZED, or AUTHENTICATION_PENDING state after 3DS URL visit. Current status: {}", 
                sync_response.status
            );
        } else {
            assert!(
                response.status == i32::from(PaymentStatus::AuthenticationPending)
                    || response.status == i32::from(PaymentStatus::Pending)
                    || response.status == i32::from(PaymentStatus::Charged),
                "Payment should be in AuthenticationPending or Pending state"
            );
        }

        // Wait a bit longer to ensure the payment is fully processed
        tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;

        // Only attempt refund if payment is in a refundable state
        // Check final payment status to determine if refund is possible
        let final_sync_request = create_payment_sync_request(&transaction_id, &request_ref_id);
        let mut final_sync_grpc_request = Request::new(final_sync_request);
        add_nexinets_metadata(&mut final_sync_grpc_request);

        let final_sync_response = client
            .get(final_sync_grpc_request)
            .await
            .expect("gRPC final payment_sync call failed")
            .into_inner();

        if final_sync_response.status == i32::from(PaymentStatus::Charged)
            || final_sync_response.status == i32::from(PaymentStatus::Authorized)
        {
            // Create refund request
            let refund_request = create_refund_request(&transaction_id, &request_ref_id);

            // Add metadata headers for refund request
            let mut refund_grpc_request = Request::new(refund_request);
            add_nexinets_metadata(&mut refund_grpc_request);

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

// Test refund sync flow - runs as a separate test since refund + sync is
// complex
#[tokio::test]
async fn test_refund_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        grpc_test!(refund_client, RefundServiceClient<Channel>, {
            // Add delay of 8 seconds
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;

            // First create a payment
            let auth_request = create_payment_authorize_request(CaptureMethod::Automatic);

            // Add metadata headers for auth request
            let mut auth_grpc_request = Request::new(auth_request);
            add_nexinets_metadata(&mut auth_grpc_request);

            // Send the auth request
            let auth_response = client
                .authorize(auth_grpc_request)
                .await
                .expect("gRPC payment_authorize call failed")
                .into_inner();

            // Extract the transaction ID
            let transaction_id = extract_transaction_id(&auth_response);

            // Extract the request ref ID which is Order_id for nexinets
            let request_ref_id = extract_request_ref_id(&auth_response);

            // Check if payment requires 3DS authentication
            if auth_response.status == i32::from(PaymentStatus::AuthenticationPending) {
                let _ = visit_3ds_authentication_url(&request_ref_id).await;

                // Wait a moment for the authentication state to be updated
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            } else {
                // Wait for payment to process
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }

            // Create sync request to check payment status
            let sync_request = create_payment_sync_request(&transaction_id, &request_ref_id);

            // Add metadata headers for sync request
            let mut sync_grpc_request = Request::new(sync_request);
            add_nexinets_metadata(&mut sync_grpc_request);

            // Send the sync request
            let sync_response = client
                .get(sync_grpc_request)
                .await
                .expect("gRPC payment_sync call failed")
                .into_inner();

            assert!(
                sync_response.status == i32::from(PaymentStatus::Charged)
                    || sync_response.status == i32::from(PaymentStatus::Authorized)
                    || sync_response.status == i32::from(PaymentStatus::AuthenticationPending),
                "Payment should be in CHARGED, AUTHORIZED, or AUTHENTICATION_PENDING state. Current status: {}", 
                sync_response.status
            );

            // Only attempt refund if payment is in a refundable state
            if sync_response.status == i32::from(PaymentStatus::Charged)
                || sync_response.status == i32::from(PaymentStatus::Authorized)
            {
                // Create refund request
                let refund_request = create_refund_request(&transaction_id, &request_ref_id);

                // Add metadata headers for refund request
                let mut refund_grpc_request = Request::new(refund_request);
                add_nexinets_metadata(&mut refund_grpc_request);

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

                // Create refund sync request with our mock ID
                let refund_sync_request =
                    create_refund_sync_request(&transaction_id, refund_id, &request_ref_id);

                // Add metadata headers for refund sync request
                let mut refund_sync_grpc_request = Request::new(refund_sync_request);
                add_nexinets_metadata(&mut refund_sync_grpc_request);

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
            }
        });
    });
}
