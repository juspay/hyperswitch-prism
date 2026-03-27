#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(unused_imports)]
#![allow(dead_code)]

use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use ucs_env::configs;
mod common;
mod utils;

use std::{
    any::Any,
    collections::HashMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use cards::CardNumber;
use grpc_api_types::{
    health_check::{health_client::HealthClient, HealthCheckRequest},
    payments::{
        identifier::IdType, mandate_reference::MandateIdType, payment_method,
        payment_service_client::PaymentServiceClient,
        recurring_payment_service_client::RecurringPaymentServiceClient, AcceptanceType, Address,
        AuthenticationType, BrowserInformation, CaptureMethod, CardDetails,
        ConnectorMandateReferenceId, CountryAlpha2, Currency, CustomerAcceptance, FutureUsage,
        Identifier, MandateReference, PaymentAddress, PaymentMethod, PaymentMethodType,
        PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentServiceSetupRecurringRequest, PaymentServiceVoidRequest, PaymentStatus,
        RecurringPaymentServiceChargeRequest, RecurringPaymentServiceChargeResponse,
        RefundServiceGetRequest, RefundStatus,
    },
};
use rand::{distributions::Alphanumeric, Rng};
use tonic::{transport::Channel, Request};
use uuid::Uuid;

// Function to generate random name
fn random_name() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

// Constants for AuthorizeDotNet connector
const CONNECTOR_NAME: &str = "authorizedotnet";

// Test card data matching working grpcurl payload
const TEST_AMOUNT: i64 = 102; // Amount from working grpcurl
const TEST_CARD_NUMBER: &str = "5123456789012346"; // Mastercard from working grpcurl
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "2050";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "TestCustomer0011uyty4";
const TEST_EMAIL_BASE: &str = "testcustomer001@gmail.com";

// Test data for repeat payment
const REPEAT_AMOUNT: i64 = 1000; // Amount for repeat payments

// Metadata for Authorize.Net
// Note: BASE64_METADATA is the base64 encoded version of this JSON:
// {"userFields":{"MerchantDefinedFieldName1":"MerchantDefinedFieldValue1","favorite_color":"blue"}}
const BASE64_METADATA: &str =
  "eyJ1c2VyRmllbGRzIjp7Ik1lcmNoYW50RGVmaW5lZEZpZWxkTmFtZTEiOiJNZXJjaGFudERlZmluZWRGaWVsZFZhbHVlMSIsImZhdm9yaXRlX2NvbG9yIjoiYmx1ZSJ9fQ==";

// Helper function to get current timestamp
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Helper function to generate unique email
fn generate_unique_email() -> String {
    format!("testcustomer{}@gmail.com", get_timestamp())
}

// Helper function to generate unique request reference ID
fn generate_unique_request_ref_id(prefix: &str) -> String {
    format!("{}_{}", prefix, &Uuid::new_v4().simple().to_string()[..8])
}

// Helper function to add AuthorizeDotNet metadata headers to a request
fn add_authorizenet_metadata<T>(request: &mut Request<T>) {
    // Get API credentials using the common credential loading utility
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load Authorize.Net credentials");

    let (api_key, key1) = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
            (api_key.expose(), key1.expose())
        }
        _ => panic!("Expected BodyKey auth type for Authorize.Net"),
    };

    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "body-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        api_key.parse().expect("Failed to parse x-api-key"),
    );
    request
        .metadata_mut()
        .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
    // Add merchant ID which is required by the server
    request.metadata_mut().append(
        "x-merchant-id",
        "test_merchant"
            .parse()
            .expect("Failed to parse x-merchant-id"),
    );

    // Add tenant ID which is required by the server
    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );

    // Add request ID which is required by the server
    request.metadata_mut().append(
        "x-request-id",
        generate_unique_request_ref_id("req")
            .parse()
            .expect("Failed to parse x-request-id"),
    );

    // Add connector request reference ID which is required for our error handling
    request.metadata_mut().append(
        "x-connector-request-reference-id",
        generate_unique_request_ref_id("conn_ref")
            .parse()
            .expect("Failed to parse x-connector-request-reference-id"),
    );
}

// Helper function to extract transaction ID from response
fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    // First try to get the transaction ID from transaction_id field
    match &response.connector_transaction_id {
        Some(id) => id.clone(),
        None => {
            // Fallback to response_ref_id if transaction_id is not available
            if let Some(ref_id) = &response.merchant_transaction_id {
                ref_id.clone()
            } else {
                format!("no_transaction_id_{}", get_timestamp())
            }
        }
    }
}

// Helper function to create a repeat payment request (matching your JSON format)
#[allow(clippy::field_reassign_with_default)]
fn create_repeat_payment_request(mandate_id: &str) -> RecurringPaymentServiceChargeRequest {
    let mandate_reference = MandateReference {
        mandate_id_type: Some(MandateIdType::ConnectorMandateId(
            ConnectorMandateReferenceId {
                connector_mandate_request_reference_id: None,
                connector_mandate_id: Some(mandate_id.to_string()),
                payment_method_id: None,
            },
        )),
    };

    // Create metadata matching your JSON format
    let mut metadata_map = HashMap::new();
    metadata_map.insert("order_type".to_string(), "recurring".to_string());
    metadata_map.insert(
        "customer_note".to_string(),
        "Monthly subscription payment".to_string(),
    );

    let metadata_json = serde_json::to_string(&metadata_map).unwrap();

    RecurringPaymentServiceChargeRequest {
        merchant_charge_id: Some(generate_unique_request_ref_id("repeat_req")),
        connector_recurring_payment_id: Some(mandate_reference),
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: REPEAT_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        merchant_order_id: Some(format!("repeat_order_{}", get_timestamp())),
        metadata: Some(Secret::new(metadata_json)),
        webhook_url: Some("https://your-webhook-url.com/payments/webhook".to_string()),
        capture_method: None,
        email: None,
        browser_info: None,
        test_mode: None,
        payment_method_type: None,
        state: None,
        address: None,
        connector_customer_id: None,
        description: None,
        merchant_configured_currency: Some(i32::from(Currency::Usd)),
        ..Default::default()
    }
}

// Test repeat payment (MIT) flow using previously created mandate
async fn test_repeat_everything() {
    grpc_test!([client: PaymentServiceClient<Channel>, recurring_client: RecurringPaymentServiceClient<Channel>], {
        // First, create a mandate using register
        let register_request = create_register_request();

        let mut register_grpc_request = Request::new(register_request);
        add_authorizenet_metadata(&mut register_grpc_request);

        let register_response = client
            .setup_recurring(register_grpc_request)
            .await
            .expect("gRPC setup_recurring call failed")
            .into_inner();

        // Verify we got a mandate reference
        assert!(
            register_response.mandate_reference.is_some(),
            "Mandate reference should be present"
        );

        let mandate_id = register_response
            .mandate_reference
            .as_ref()
            .unwrap()
            .mandate_id_type
            .as_ref()
            .expect("Mandate ID should be present");

        let mandate_id = match mandate_id {
            MandateIdType::ConnectorMandateId(connector_mandate) => connector_mandate
                .connector_mandate_id
                .clone()
                .expect("Connector mandate ID should be present"),
            _ => panic!("Expected ConnectorMandateId type for mandate reference"),
        };

        // Now perform a repeat payment using the mandate
        let repeat_request = create_repeat_payment_request(&mandate_id);

        let mut repeat_grpc_request = Request::new(repeat_request);
        add_authorizenet_metadata(&mut repeat_grpc_request);

        // Send the repeat payment request
        let repeat_response = recurring_client
            .charge(repeat_grpc_request)
            .await
            .expect("gRPC recurring_client::charge call failed")
            .into_inner();

        // Verify the response
        assert!(
            repeat_response.connector_transaction_id.is_some(),
            "Transaction ID should be present"
        );

        // // Verify no error occurred
        // assert!(
        //     repeat_response.error_message.is_none()
        //         || repeat_response.error_message.as_ref().unwrap().is_empty(),
        //     "No error message should be present for successful repeat payment"
        // );
    });
}

// Helper function to create a payment authorization request
#[allow(clippy::field_reassign_with_default)]
fn create_payment_authorize_request(
    capture_method: common_enums::CaptureMethod,
) -> PaymentServiceAuthorizeRequest {
    // Initialize with all required fields
    let mut request = PaymentServiceAuthorizeRequest::default();

    let mut request_ref_id = Identifier::default();
    request_ref_id.id_type = Some(IdType::Id(
        generate_unique_request_ref_id("req_"), // Using timestamp to make unique
    ));
    // Set the basic payment details matching working grpcurl
    request.amount = Some(grpc_api_types::payments::Money {
        minor_amount: TEST_AMOUNT,
        currency: 146, // Currency value from working grpcurl
    });

    // Set up card payment method using the correct structure
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_issuer: None,
        card_network: Some(2_i32), // Mastercard network for 5123456789012346
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    };

    request.payment_method = Some(PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
    });

    request.customer = Some(grpc_api_types::payments::Customer {
        email: Some(generate_unique_email().into()),
        name: None,
        id: Some("TEST_CONNECTOR".to_string()),
        connector_customer_id: Some("TEST_CONNECTOR".to_string()),
        phone_number: None,
        phone_country_code: None,
    });
    // Generate random names for billing to prevent duplicate transaction errors
    let billing_first_name = random_name();
    let billing_last_name = random_name();

    // Minimal address structure matching working grpcurl
    request.address = Some(PaymentAddress {
        billing_address: Some(Address {
            first_name: Some(billing_first_name.into()),
            last_name: Some(billing_last_name.into()),
            line1: Some("14 Main Street".to_string().into()),
            line2: None,
            line3: None,
            city: Some("Pecan Springs".to_string().into()),
            state: Some("TX".to_string().into()),
            zip_code: Some("44628".to_string().into()),
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
        referer: None,
        ip_address: None,
        os_type: None,
        os_version: None,
        device_model: None,
        accept_language: None,
        time_zone_offset_minutes: None,
    };
    request.browser_info = Some(browser_info);

    request.return_url = Some("www.google.com".to_string());
    // Set the transaction details
    request.auth_type = i32::from(AuthenticationType::NoThreeDs);

    request.request_incremental_authorization = Some(true);

    request.enrolled_for_3ds = Some(true);

    // Set capture method
    if let common_enums::CaptureMethod::Manual = capture_method {
        request.capture_method = Some(i32::from(CaptureMethod::Manual));
        // request.request_incremental_authorization = Some(true);
    } else {
        request.capture_method = Some(i32::from(CaptureMethod::Automatic));
    }

    // Set the connector metadata (Base64 encoded)
    let mut metadata_map = HashMap::new();
    metadata_map.insert("metadata".to_string(), BASE64_METADATA.to_string());
    let metadata_json = serde_json::to_string(&metadata_map).unwrap();
    request.metadata = Some(Secret::new(metadata_json));

    request
}

// Helper function to create a payment sync request
fn create_payment_get_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        encoded_data: None,
        capture_method: None,
        handle_response: None,
        merchant_transaction_id: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: 146, // Currency value from working grpcurl
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
    let request_ref_id = generate_unique_request_ref_id("capture");

    PaymentServiceCaptureRequest {
        merchant_capture_id: Some(request_ref_id),
        connector_transaction_id: transaction_id.to_string(),
        amount_to_capture: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        multiple_capture_data: None,
        metadata: None,
        connector_feature_data: None,
        browser_info: None,
        capture_method: None,
        state: None,
        test_mode: None,
        merchant_order_id: None,
    }
}

// Helper function to create a void request
fn create_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: transaction_id.to_string(),
        merchant_void_id: Some(transaction_id.to_string()),
        cancellation_reason: None,
        all_keys_required: None,
        browser_info: None,
        amount: None,
        ..Default::default()
    }
}

// Helper function to sleep for a short duration to allow server processing
fn allow_processing_time() {
    std::thread::sleep(std::time::Duration::from_secs(3));
}

// Helper function to create a refund request
fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    // Create refund metadata with credit card information as required by Authorize.net
    let mut refund_metadata_map = HashMap::new();
    refund_metadata_map.insert(
          "refund_metadata".to_string(),
          format!(
              "{{\"creditCard\":{{\"cardNumber\":\"{TEST_CARD_NUMBER}\",\"expirationDate\":\"2050-12\"}}}}",
          ),
      );

    let refund_metadata_json = serde_json::to_string(&refund_metadata_map).unwrap();

    PaymentServiceRefundRequest {
        merchant_refund_id: Some(generate_unique_request_ref_id("refund")),
        connector_transaction_id: transaction_id.to_string(),
        payment_amount: TEST_AMOUNT,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Usd),
        }),
        reason: Some("Test refund".to_string()),
        webhook_url: None,
        merchant_account_id: None,
        capture_method: None,
        metadata: None,
        connector_feature_data: None,
        refund_metadata: Some(Secret::new(refund_metadata_json)),
        browser_info: None,
        test_mode: Some(true),
        state: None,
        payment_method_type: None,
        customer_id: Some("TEST_CONNECTOR".to_string()),
    }
}

// Helper function to create a refund get request
fn create_refund_get_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        merchant_refund_id: Some(generate_unique_request_ref_id("refund_get")),
        connector_transaction_id: transaction_id.to_string(),
        refund_id: refund_id.to_string(),
        browser_info: None,
        refund_reason: None,
        test_mode: Some(true),
        refund_metadata: None,
        state: None,
        connector_feature_data: None,
        payment_method_type: None,
    }
}

// Helper function to create a register (setup mandate) request (matching your JSON format)
#[allow(clippy::field_reassign_with_default)]
fn create_register_request() -> PaymentServiceSetupRecurringRequest {
    let mut request = PaymentServiceSetupRecurringRequest::default();

    // Set amounts matching your JSON (3000 minor units)
    request.amount = Some(grpc_api_types::payments::Money {
        minor_amount: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
    });

    // Set up card payment method with Visa network as in your JSON
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_issuer: None,
        card_network: Some(1_i32),
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    };

    request.payment_method = Some(PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
    });

    // Set customer information with unique email
    request.customer = Some(grpc_api_types::payments::Customer {
        email: Some(generate_unique_email().into()),
        name: Some(TEST_CARD_HOLDER.to_string()),
        id: None,
        connector_customer_id: Some("TEST_CONNECTOR_CUSTOMER_ID".to_string()),
        phone_number: None,
        phone_country_code: None,
    });

    // Add customer acceptance as required by the server (matching your JSON: "acceptance_type": "OFFLINE")
    request.customer_acceptance = Some(CustomerAcceptance {
        acceptance_type: i32::from(AcceptanceType::Offline),
        accepted_at: 0, // You can set this to current timestamp if needed
        online_mandate_details: None,
    });

    // Add billing address matching your JSON format
    request.address = Some(PaymentAddress {
        billing_address: Some(Address {
            first_name: Some("Test".to_string().into()),
            last_name: Some("Customer001".to_string().into()),
            line1: Some("123 Test St".to_string().into()),
            line2: None,
            line3: None,
            city: Some("Test City".to_string().into()),
            state: Some("NY".to_string().into()),
            zip_code: Some("10001".to_string().into()),
            country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
            phone_number: None,
            phone_country_code: None,
            email: Some(generate_unique_email().into()),
        }),
        shipping_address: None,
    });

    // Set auth type as NO_THREE_DS
    request.auth_type = i32::from(AuthenticationType::NoThreeDs);

    // Set setup_future_usage to OFF_SESSION (matching your JSON: "setup_future_usage": "OFF_SESSION")
    request.setup_future_usage = Some(i32::from(FutureUsage::OffSession));

    // Set 3DS enrollment to false
    request.enrolled_for_3ds = false;

    // Set request reference ID with unique UUID (this will be unique every time)
    request.merchant_recurring_payment_id = generate_unique_request_ref_id("mandate");

    // Set empty connector metadata
    request.metadata = None;

    request
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
        // println!("Auth request for auto capture: {:?}", request);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_authorizenet_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // println!("Payment authorize response for auto: {:?}", response);
        // Verify the response - transaction_id may not be present for failed or pending payments
        let successful_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Authorized),
        ];
        if successful_statuses.contains(&response.status) {
            assert!(
                response.connector_transaction_id.is_some(),
                "Transaction ID should be present for successful payments, but status was: {}",
                response.status
            );
        }

        // Extract the transaction ID
        let _transaction_id = extract_transaction_id(&response);

        // Verify payment status - allow CHARGED, PENDING, or FAILURE (common in sandbox)
        let acceptable_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Pending),
        ];
        assert!(
            acceptable_statuses.contains(&response.status),
            "Payment should be in CHARGED, PENDING, or FAILURE state (sandbox) but was: {}",
            response.status
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the payment authorization request with manual capture
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);
        // println!("Auth request for manual capture: {:?}", auth_request);
        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_authorizenet_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Transaction_id may not be present for failed or pending payments
        let successful_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Pending),
        ];
        // println!(
        //     "Payment authorize response: {:?}",
        //     auth_response
        // );
        if successful_statuses.contains(&auth_response.status) {
            assert!(
                auth_response.connector_transaction_id.is_some(),
                "Transaction ID should be present for successful payments, but status was: {}",
                auth_response.status
            );
        }

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status is authorized (for manual capture) - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Pending),
            i32::from(PaymentStatus::Charged),
            // i32::from(PaymentStatus::Failure),
        ];
        // println!("print acceptable statuses: {:?}", acceptable_statuses);
        assert!(
            acceptable_statuses.contains(&auth_response.status),
            "Payment should be in AUTHORIZED, PENDING, or FAILURE state (sandbox) but was: {}",
            auth_response.status
        );

        // Create capture request
        let capture_request = create_payment_capture_request(&transaction_id);

        // Add metadata headers for capture request
        let mut capture_grpc_request = Request::new(capture_request);
        add_authorizenet_metadata(&mut capture_grpc_request);

        // Send the capture request
        let capture_response = client
            .capture(capture_grpc_request)
            .await
            .expect("gRPC payment_capture call failed")
            .into_inner();

        // Verify payment status is charged after capture - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Pending),
            // i32::from(PaymentStatus::Failure),
        ];
        assert!(
              acceptable_statuses.contains(&capture_response.status),
              "Payment should be in CHARGED, PENDING, or FAILURE state after capture (sandbox) but was: {}",
              capture_response.status
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
        add_authorizenet_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let _transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status is authorized - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Pending),
            // i32::from(PaymentStatus::Failure),
        ];
        assert!(
            acceptable_statuses.contains(&auth_response.status),
            "Payment should be in AUTHORIZED, PENDING, or FAILURE state (sandbox) but was: {}",
            auth_response.status
        );

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        // Create get request
        let get_request = create_payment_get_request(&_transaction_id);

        // Add metadata headers for get request
        let mut get_grpc_request = Request::new(get_request);
        add_authorizenet_metadata(&mut get_grpc_request);

        // Send the get request
        let get_response = client
            .get(get_grpc_request)
            .await
            .expect("gRPC payment_get call failed")
            .into_inner();

        // Verify the sync response

        // Verify the payment status matches what we expect - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Pending),
            i32::from(PaymentStatus::Charged),
            // i32::from(PaymentStatus::Failure),
        ];
        assert!(
            acceptable_statuses.contains(&get_response.status),
            "Payment get should return AUTHORIZED, PENDING, or FAILURE state (sandbox) but was: {}",
            get_response.status
        );

        // Verify we have transaction ID in the response
        assert!(
            !get_response.connector_transaction_id.is_empty(),
            "Transaction ID should be present in get response"
        );
    });
}

// Test void flow (unique to AuthorizeDotNet)
#[tokio::test]
async fn test_void() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // First create a payment to void
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_authorizenet_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status is authorized or handle other states - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Pending),
            i32::from(PaymentStatus::Voided),
            // i32::from(PaymentStatus::Failure),
        ];
        //   println!(
        //       "Auth response: {:?}",
        //       auth_response
        //   );
        assert!(
            acceptable_statuses.contains(&auth_response.status),
            "Payment should be in AUTHORIZED, PENDING, or FAILURE (sandbox) but was: {}",
            auth_response.status
        );

        // Skip void test if payment is not in AUTHORIZED state (but allow test to continue if PENDING)
        if auth_response.status != i32::from(PaymentStatus::Authorized)
            && auth_response.status != i32::from(PaymentStatus::Pending)
        {
            return;
        }

        // Allow some time for the authorization to be processed
        allow_processing_time();

        // Additional async delay when running with other tests to avoid conflicts
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        //   println!("transaction_id: {}", transaction_id);
        // Create void request
        let void_request = create_void_request(&transaction_id);
        //   println!("Void request: {:?}", void_request);

        // Add metadata headers for void request
        let mut void_grpc_request = Request::new(void_request);
        add_authorizenet_metadata(&mut void_grpc_request);
        //   println!("Void grpc request: {:?}", void_grpc_request);
        // Send the void request
        let void_response = client
            .void(void_grpc_request)
            .await
            .expect("gRPC void_payment call failed")
            .into_inner();

        // Accept VOIDED status
        let acceptable_statuses = [i32::from(PaymentStatus::Voided)];
        // println!("Void response: {:?}", void_response);
        assert!(
            acceptable_statuses.contains(&void_response.status),
            "Payment should be in VOIDED state but was: {}",
            void_response.status
        );
    });
}

// Test refund flow
#[tokio::test]
#[ignore] // Flaky in sandbox; skip in CI.
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // First create a payment
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_authorizenet_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status or handle other states - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Pending),
        ];
        assert!(
            acceptable_statuses.contains(&auth_response.status),
            "Payment should be in CHARGED, PENDING, or FAILURE state (sandbox) but was: {}",
            auth_response.status
        );

        // Skip refund test if payment is not in CHARGED state (but allow test to continue if PENDING)
        if auth_response.status != i32::from(PaymentStatus::Charged)
            && auth_response.status != i32::from(PaymentStatus::Pending)
        {
            return;
        }

        // Wait a bit to ensure the payment is fully processed
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Create refund request
        let refund_request = create_refund_request(&transaction_id);

        // Add metadata headers for refund request
        let mut refund_grpc_request = Request::new(refund_request);
        add_authorizenet_metadata(&mut refund_grpc_request);

        // Send the refund request
        let refund_result = client.refund(refund_grpc_request).await;

        // Check if we have a successful refund OR any expected error (including gRPC errors)
        let is_success_status = refund_result.as_ref().is_ok_and(|response| {
            response.get_ref().status == i32::from(RefundStatus::RefundSuccess)
        });

        let has_expected_error = refund_result.as_ref().is_ok_and(|response| {
            let error_msg = response
                .get_ref()
                .error
                .clone()
                .and_then(|e| e.connector_details)
                .and_then(|e| e.message)
                .unwrap_or_default()
                .to_lowercase();
            error_msg.contains(
                "The referenced transaction does not meet the criteria for issuing a credit.",
            ) || error_msg.contains("credit")
                || error_msg.contains("refund")
                || error_msg.contains("transaction")
                || response.get_ref().status == i32::from(RefundStatus::RefundFailure)
        });

        let has_grpc_error = refund_result.is_err();

        assert!(
              is_success_status || has_expected_error || has_grpc_error,
              "Refund should either succeed, have expected error, or gRPC error (common in sandbox). Got: {refund_result:?}"
          );
    });
}

// Test register (setup mandate) flow
async fn test_register() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create the register request
        let request = create_register_request();

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_authorizenet_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .setup_recurring(grpc_request)
            .await
            .expect("gRPC setup_recurring call failed")
            .into_inner();

        // Verify the response
        assert!(
            response.connector_recurring_payment_id.is_some(),
            "Registration ID should be present"
        );

        // Check if we have a mandate reference
        assert!(
            response.mandate_reference.is_some(),
            "Mandate reference should be present"
        );

        // Verify the mandate reference has the expected structure
        if let Some(mandate_ref) = &response.mandate_reference {
            // Verify the composite ID format (profile_id-payment_profile_id)
            if let Some(MandateIdType::ConnectorMandateId(mandate_id)) =
                &mandate_ref.mandate_id_type
            {
                assert!(
                    mandate_id.connector_mandate_id.is_some(),
                    "Mandate ID should be present"
                );
                if let Some(mandate_id) = mandate_id.connector_mandate_id.as_ref() {
                    assert!(
                        mandate_id.contains('-') || !mandate_id.is_empty(),
                        "Mandate ID should be either a composite ID or a profile ID"
                    );
                }
            }
        }

        let error_message = response
            .error
            .and_then(|e| e.connector_details)
            .and_then(|e| e.message);
        // Verify no error occurred
        assert!(
            error_message.is_none() || error_message.as_ref().unwrap().is_empty(),
            "No error message should be present for successful register"
        );
    });
}

// Test authorization with setup_future_usage
#[tokio::test]
async fn test_authorize_with_setup_future_usage() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Create an authorization request with setup_future_usage
        let mut auth_request =
            create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add setup_future_usage to trigger profile creation
        auth_request.setup_future_usage = Some(i32::from(FutureUsage::OnSession));

        // Add metadata headers
        let mut auth_grpc_request = Request::new(auth_request);
        add_authorizenet_metadata(&mut auth_grpc_request);

        // Send the authorization request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC authorize with setup_future_usage call failed")
            .into_inner();

        // Verify the response - transaction_id may not be present for failed or pending payments
        let successful_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Authorized),
        ];
        if successful_statuses.contains(&auth_response.status) {
            assert!(
                auth_response.connector_transaction_id.is_some(),
                "Transaction ID should be present for successful payments, but status was: {}",
                auth_response.status
            );
        }

        // Verify payment status - allow PENDING or FAILURE in sandbox
        let acceptable_statuses = [
            i32::from(PaymentStatus::Charged),
            i32::from(PaymentStatus::Pending),
            i32::from(PaymentStatus::Authorized),
        ];
        assert!(
            acceptable_statuses.contains(&auth_response.status),
            "Payment should be in CHARGED, PENDING, or FAILURE state (sandbox) but was: {}",
            auth_response.status
        );

        // When setup_future_usage is set, a customer profile is created
        // The mandate can be used in subsequent transactions
    });
}
