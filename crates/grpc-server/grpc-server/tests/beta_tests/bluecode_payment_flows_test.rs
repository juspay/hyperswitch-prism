#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use ucs_env::configs;
mod common;
mod utils;

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        identifier::IdType, payment_method, payment_service_client::PaymentServiceClient,
        wallet_payment_method_type, Address, AuthenticationType, Bluecode, BrowserInformation,
        CaptureMethod, CountryAlpha2, Identifier, PaymentAddress, PaymentMethod,
        PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse, PaymentServiceGetRequest,
        PaymentStatus, WalletPaymentMethodType,
    },
};
use hyperswitch_masking::{ExposeInterface, Secret};
use rand::{distributions::Alphanumeric, Rng};
use tonic::{transport::Channel, Request};

// Constants for Bluecode connector
const CONNECTOR_NAME: &str = "bluecode";

// Test card data
const TEST_AMOUNT: i64 = 1000;

// Helper function to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Helper function to add Bluecode metadata headers to a request
fn add_bluecode_metadata<T>(request: &mut Request<T>) {
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load bluecode credentials");

    let api_key = match auth {
        domain_types::router_data::ConnectorAuthType::HeaderKey { api_key } => api_key.expose(),
        _ => panic!("Expected HeaderKey auth type for bluecode"),
    };

    // Get the shop_name from metadata
    let metadata = utils::credential_utils::load_connector_metadata(CONNECTOR_NAME)
        .expect("Failed to load bluecode metadata");
    let shop_name = metadata
        .get("shop_name")
        .expect("shop_name not found in bluecode metadata")
        .clone();

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

    // Add the terminal_id in the metadata JSON
    // This metadata must be in the proper format that the connector expects
    let metadata_json = format!(r#"{{"shop_name":"{shop_name}"}}"#);

    request.metadata_mut().append(
        "x-metadata",
        metadata_json.parse().expect("Failed to parse x-metadata"),
    );

    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );
    // Add request ID which is required by the server
    request.metadata_mut().append(
        "x-request-id",
        format!("mifinity_req_{}", get_timestamp())
            .parse()
            .expect("Failed to parse x-request-id"),
    );

    request.metadata_mut().append(
        "x-merchant-id",
        "12abc123-f8a3-99b8-9ef8-b31180358hh4"
            .parse()
            .expect("Failed to parse x-merchant-id"),
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

// Helper function to generate unique request reference ID
fn generate_unique_request_ref_id(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

// Helper function to generate unique email
fn generate_unique_email() -> String {
    format!("testcustomer{}@gmail.com", get_timestamp())
}

// Function to generate random name
fn random_name() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

// Helper function to create a payment authorization request
#[allow(clippy::field_reassign_with_default)]
fn create_payment_authorize_request(
    capture_method: common_enums::CaptureMethod,
) -> PaymentServiceAuthorizeRequest {
    // Initialize with all required fields
    let mut request = PaymentServiceAuthorizeRequest {
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Bluecode(Bluecode {})),
        }),
        ..Default::default()
    };

    if let common_enums::CaptureMethod::Manual = capture_method {
        request.capture_method = Some(i32::from(CaptureMethod::Manual));
        // request.request_incremental_authorization = Some(true);
    } else {
        request.capture_method = Some(i32::from(CaptureMethod::Automatic));
    }

    let mut request_ref_id = Identifier::default();
    request_ref_id.id_type = Some(IdType::Id(
        generate_unique_request_ref_id("req_"), // Using timestamp to make unique
    ));

    request.request_ref_id = Some(request_ref_id);
    // Set the basic payment details matching working grpcurl
    request.amount = TEST_AMOUNT;
    request.minor_amount = TEST_AMOUNT;
    request.currency = 146; // Currency value from working grpcurl

    request.email = Some(Secret::new(generate_unique_email()));

    // Generate random names for billing to prevent duplicate transaction errors
    let billing_first_name = random_name();
    let billing_last_name = random_name();

    // Minimal address structure matching working grpcurl
    request.address = Some(PaymentAddress {
        billing_address: Some(Address {
            first_name: Some(Secret::new(billing_first_name)),
            last_name: Some(Secret::new(billing_last_name)),
            line1: Some(Secret::new("14 Main Street".to_string())),
            line2: None,
            line3: None,
            city: Some(Secret::new("Pecan Springs".to_string())),
            state: Some(Secret::new("TX".to_string())),
            zip_code: Some(Secret::new("44628".to_string())),
            country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
            phone_number: None,
            phone_country_code: None,
            email: None,
        }),
        shipping_address: None, // Minimal address - no shipping for working grpcurl
    });

    let browser_info = BrowserInformation {
        color_depth: None,
        java_enabled: Some(false),
        screen_height: Some(1080),
        screen_width: Some(1920),
        user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string()),
        accept_header: Some(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
        ),
        java_script_enabled: Some(false),
        language: Some("en-US".to_string()),
        ip_address: None,
        os_type: None,
        os_version: None,
        device_model: None,
        accept_language: None,
        time_zone_offset_minutes: None,
        referer: None,
    };
    request.browser_info = Some(browser_info);

    request.return_url = Some("www.google.com".to_string());
    // Set the transaction details
    request.auth_type = i32::from(AuthenticationType::NoThreeDs);

    request.request_incremental_authorization = Some(true);

    request.enrolled_for_3ds = Some(true);

    // Set capture method
    // request.capture_method = Some(i32::from(CaptureMethod::from(capture_method)));

    // Get shop_name for metadata
    let metadata = utils::credential_utils::load_connector_metadata(CONNECTOR_NAME)
        .expect("Failed to load bluecode metadata");
    let shop_name = metadata
        .get("shop_name")
        .expect("shop_name not found in bluecode metadata")
        .clone();

    // Create connector metadata as a proper JSON object
    let mut connector_metadata = HashMap::new();
    connector_metadata.insert("shop_name".to_string(), shop_name);

    let connector_metadata_json =
        serde_json::to_string(&connector_metadata).expect("Failed to serialize connector metadata");

    let mut metadata = HashMap::new();
    metadata.insert("connector_meta_data".to_string(), connector_metadata_json);

    request.metadata = metadata;

    request
}

// Helper function to create a payment sync request
fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("fiserv_sync_{}", get_timestamp()))),
        }),
        capture_method: None,
        handle_response: None,
        // all_keys_required: None,
        amount: TEST_AMOUNT,
        currency: 146, // Currency value from working grpcurl
        state: None,
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
        let request = create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_bluecode_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Debug print has been removed

        // Verify the response
        assert!(
            response.transaction_id.is_some(),
            "Resource ID should be present"
        );

        // Extract the transaction ID
        let _transaction_id = extract_transaction_id(&response);

        // Verify payment status
        assert!(
            response.status == i32::from(PaymentStatus::AuthenticationPending),
            "Payment should be in AuthenticationPending state"
        );
    });
}

// Test payment sync
#[tokio::test]
async fn test_payment_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // First create a payment to sync
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_bluecode_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Create sync request
        let sync_request = create_payment_sync_request(&transaction_id);

        // Add metadata headers for sync request
        let mut sync_grpc_request = Request::new(sync_request);
        add_bluecode_metadata(&mut sync_grpc_request);

        // Send the sync request
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // Verify the sync response
        assert!(
            sync_response.status == i32::from(PaymentStatus::AuthenticationPending),
            "Payment should be in AuthenticationPending state"
        );
    });
}
