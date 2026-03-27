#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use ucs_env::configs;
use hyperswitch_masking::{ExposeInterface, Secret};
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
        direct_payment_service_client::DirectPaymentServiceClient, AcceptanceType, Address, AuthenticationType,
        BrowserInformation, CaptureMethod, CardDetails, CountryAlpha2,
        Currency, CustomerAcceptance, FutureUsage, Identifier, MandateReference, PaymentAddress,
        PaymentMethod, PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentServiceSetupRecurringRequest, RecurringPaymentServiceChargeRequest,
        PaymentServiceVoidRequest, PaymentStatus, RefundStatus,
    },
};
use tonic::{transport::Channel, Request};

// Constants for aci connector
const CONNECTOR_NAME: &str = "aci";

const TEST_AMOUNT: i64 = 1000;
const TEST_CARD_NUMBER: &str = "4111111111111111";
const TEST_CARD_EXP_MONTH: &str = "10";
const TEST_CARD_EXP_YEAR: &str = "2030";
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

// Helper function to add aci metadata headers to a request
fn add_aci_metadata<T>(request: &mut Request<T>) {
    // Get API credentials using the common credential loading utility
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load ACI credentials");

    let (api_key, key1) = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
            (api_key.expose(), key1.expose())
        }
        _ => panic!("Expected BodyKey auth type for ACI"),
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
    request.metadata_mut().append(
        "x-merchant-id",
        "test_merchant"
            .parse()
            .expect("Failed to parse x-merchant-id"),
    );
    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );
    request.metadata_mut().append(
        "x-request-id",
        format!("test_request_{}", get_timestamp())
            .parse()
            .expect("Failed to parse x-request-id"),
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

// Helper function to create a payment authorization request
#[allow(clippy::field_reassign_with_default)]
fn create_payment_authorize_request(
    capture_method: common_enums::CaptureMethod,
) -> PaymentServiceAuthorizeRequest {
    // Initialize with all required fields
    let mut request = PaymentServiceAuthorizeRequest::default();

    // Set request reference ID
    request.request_ref_id = Some(Identifier {
        id_type: Some(IdType::Id(format!("aci_test_{}", get_timestamp()))),
    });

    // Set the basic payment details
    request.amount = TEST_AMOUNT;
    request.minor_amount = TEST_AMOUNT;
    request.currency = i32::from(Currency::Usd);

    // Set up card payment method using the correct structure
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_issuer: None,
        card_network: Some(1_i32), // Default to Visa network
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    });

    request.payment_method = Some(PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
    });

    // Set connector customer ID
    request.customer_id = Some("TEST_CONNECTOR".to_string());

    // Set the customer information with static email (can be made dynamic)
    request.email = Some(TEST_EMAIL.to_string().into());

    // Set up address structure
    request.address = Some(PaymentAddress {
        billing_address: Some(Address {
            first_name: Some("Test".to_string().into()),
            last_name: Some("User".to_string().into()),
            line1: Some("123 Test Street".to_string().into()),
            line2: None,
            line3: None,
            city: Some("Test City".to_string().into()),
            state: Some("NY".to_string().into()),
            zip_code: Some("10001".to_string().into()),
            country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
            phone_number: None,
            phone_country_code: None,
            email: None,
        }),
        shipping_address: None,
    });

    // Set up browser information
    let browser_info = BrowserInformation {
        color_depth: None,
        java_enabled: Some(false),
        screen_height: Some(1080),
        screen_width: Some(1920),
        user_agent: Some("Mozilla/5.0 (compatible; TestAgent/1.0)".to_string()),
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

    // Set return URL
    request.return_url = Some("https://example.com/return".to_string());

    // Set the transaction details
    request.auth_type = i32::from(AuthenticationType::NoThreeDs);
    request.request_incremental_authorization = Some(true);
    request.enrolled_for_3ds = Some(true);

    // Set capture method with proper conversion
    if let common_enums::CaptureMethod::Manual = capture_method {
        request.capture_method = Some(i32::from(CaptureMethod::Manual));
    } else {
        request.capture_method = Some(i32::from(CaptureMethod::Automatic));
    }

    // Set connector metadata (empty for generic template)
    request.metadata = HashMap::new();

    request
}

// Helper function to create a payment sync request
fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("aci_sync_{}", get_timestamp()))),
        }),
        capture_method: Some(i32::from(CaptureMethod::Automatic)),
        handle_response: None,
        amount: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
        state: None,
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
        connector_metadata: HashMap::new(),
        request_ref_id: None,
        browser_info: None,
        capture_method: None,
        state: None,
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
        metadata: HashMap::new(),
        refund_metadata: HashMap::new(),
        browser_info: None,
        merchant_account_id: None,
        capture_method: None,
        request_ref_id: None,
        state: None,
    }
}

// Helper function to create a payment void request
fn create_payment_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        cancellation_reason: Some("Customer requested cancellation".to_string()),
        request_ref_id: None,
        all_keys_required: None,
        browser_info: None,
        amount: None,
        currency: None,
        ..Default::default()
    }
}

// Helper function to create a register (setup mandate) request
fn create_register_request() -> PaymentServiceSetupRecurringRequest {
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        card_issuer: None,
        card_network: Some(1_i32), // Visa network
        card_type: None,
        card_issuing_country_alpha2: None,
        bank_code: None,
        nick_name: None,
    });

    PaymentServiceSetupRecurringRequest {
        minor_amount: Some(TEST_AMOUNT),
        currency: i32::from(Currency::Usd),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details))
            })),
        }),
        customer_name: Some(TEST_CARD_HOLDER.to_string()),
        email: Some(TEST_EMAIL.to_string().into()),
        customer_acceptance: Some(CustomerAcceptance {
            acceptance_type: i32::from(AcceptanceType::Offline),
            accepted_at: 0,
            online_mandate_details: None,
        }),
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                first_name: Some("Test".to_string().into()),
                last_name: Some("Customer".to_string().into()),
                line1: Some("123 Test St".to_string().into()),
                line2: None,
                line3: None,
                city: Some("Test City".to_string().into()),
                state: Some("NY".to_string().into()),
                zip_code: Some("10001".to_string().into()),
                country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
                phone_number: None,
                phone_country_code: None,
                email: Some(TEST_EMAIL.to_string().into()),
card_details),
            shipping_address: None,
        }),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        setup_future_usage: Some(i32::from(FutureUsage::OffSession)),
        enrolled_for_3ds: Some(false),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("mandate_{}", get_timestamp()))),
        sync_type: None,
        }),
        metadata: HashMap::new(),
        ..Default::default()
    }
}

// Helper function to create a repeat payment request (matching your JSON format)
#[allow(clippy::field_reassign_with_default)]
fn create_repeat_payment_request(mandate_id: &str) -> RecurringPaymentServiceChargeRequest {
    let mandate_reference = MandateReference {
        mandate_id: Some(mandate_id.to_string()),
        payment_method_id: None,
    };

    // Create metadata matching your JSON format
    let mut metadata = HashMap::new();
    metadata.insert("order_type".to_string(), "recurring".to_string());
    metadata.insert(
        "customer_note".to_string(),
        "Monthly subscription payment".to_string(),
    );

    RecurringPaymentServiceChargeRequest {
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(format!("mandate_{}", get_timestamp()))),
        }),
        mandate_reference: Some(mandate_reference),
        amount: TEST_AMOUNT,
        currency: i32::from(Currency::Usd),
        minor_amount: TEST_AMOUNT,
        merchant_order_id: Some(format!("repeat_order_{}", get_timestamp())),
        metadata,
        webhook_url: Some("https://your-webhook-url.com/payments/webhook".to_string()),
        capture_method: None,
        email: None,
        browser_info: None,
        test_mode: None,
        payment_method_type: None,
        merchant_account_metadata: HashMap::new(),
        state: None,
        ..Default::default()
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

// Test payment authorization with auto capture
#[tokio::test]
async fn test_payment_authorization_auto_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // Create the payment authorization request
        let request = create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_aci_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Verify the response
        assert!(
            response.transaction_id.is_some(),
            "Transaction ID should be present"
        );

        // Extract the transaction ID
        let _transaction_id = extract_transaction_id(&response);

        // Verify payment status
        assert_eq!(
            response.status,
            i32::from(PaymentStatus::Charged),
            "Payment should be in CHARGED state for automatic capture"
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // Create the payment authorization request with manual capture
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_aci_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        assert!(
            auth_response.transaction_id.is_some(),
            "Transaction ID should be present"
        );

        // Extract the transaction ID
        let _transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status is authorized (for manual capture)
        assert_eq!(
            auth_response.status,
            i32::from(PaymentStatus::Authorized),
            "Payment should be in AUTHORIZED state with manual capture"
        );
    });
}

// Test payment sync
#[tokio::test]
async fn test_payment_sync() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create a payment to sync
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_aci_metadata(&mut auth_grpc_request);

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
        add_aci_metadata(&mut sync_grpc_request);

        // Send the sync request
        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC payment_sync call failed")
            .into_inner();

        // Verify the sync response - allow both AUTHORIZED and PENDING states
        let acceptable_sync_statuses = [
            i32::from(PaymentStatus::Authorized),
            i32::from(PaymentStatus::Charged),
        ];
        assert!(
            acceptable_sync_statuses.contains(&sync_response.status),
            "Payment should be in AUTHORIZED or CHARGED state, but was: {}",
            sync_response.status
        );
    });
}

// Test payment authorization with manual capture
#[tokio::test]
async fn test_payment_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // Create the payment authorization request with manual capture
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_aci_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        assert!(
            auth_response.transaction_id.is_some(),
            "Transaction ID should be present"
        );

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status is authorized (for manual capture)
        assert!(
            auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in AUTHORIZED state with manual capture"
        );

        // Create capture request
        let capture_request = create_payment_capture_request(&transaction_id);

        // Add metadata headers for capture request
        let mut capture_grpc_request = Request::new(capture_request);
        add_aci_metadata(&mut capture_grpc_request);

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

// Test refund flow
#[tokio::test]
async fn test_refund() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create a payment to refund
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Automatic);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_aci_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment status - allow both CHARGED and PENDING states
        let acceptable_payment_statuses = [i32::from(PaymentStatus::Charged)];
        assert!(
            acceptable_payment_statuses.contains(&auth_response.status),
            "Payment should be in CHARGED state before attempting refund, but was: {}",
            auth_response.status
        );
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        // Create refund request
        let refund_request = create_refund_request(&transaction_id);

        // Add metadata headers for refund request
        let mut refund_grpc_request = Request::new(refund_request);
        add_aci_metadata(&mut refund_grpc_request);

        // Send the refund request
        let refund_response = client
            .refund(refund_grpc_request)
            .await
            .expect("gRPC refund call failed")
            .into_inner();

        // Extract the refund ID
        let refund_id = refund_response.refund_id.clone();

        // Verify the refund response
        assert!(!refund_id.is_empty(), "Refund ID should not be empty");
        assert!(
            refund_response.status == i32::from(RefundStatus::RefundSuccess)
                || refund_response.status == i32::from(RefundStatus::RefundPending),
            "Refund should be in SUCCESS or PENDING state"
        );
    });
}

// Test payment void flow
#[tokio::test]
async fn test_payment_void() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // First create a payment with manual capture (so it stays in authorized state)
        let auth_request = create_payment_authorize_request(common_enums::CaptureMethod::Manual);

        // Add metadata headers for auth request
        let mut auth_grpc_request = Request::new(auth_request);
        add_aci_metadata(&mut auth_grpc_request);

        // Send the auth request
        let auth_response = client
            .authorize(auth_grpc_request)
            .await
            .expect("gRPC payment_authorize call failed")
            .into_inner();

        // Extract the transaction ID
        let transaction_id = extract_transaction_id(&auth_response);

        // Verify payment is in authorized state
        assert!(
            auth_response.status == i32::from(PaymentStatus::Authorized),
            "Payment should be in AUTHORIZED state before void"
        );

        // Create void request
        let void_request = create_payment_void_request(&transaction_id);

        // Add metadata headers for void request
        let mut void_grpc_request = Request::new(void_request);
        add_aci_metadata(&mut void_grpc_request);

        // Send the void request
        let void_response = client
            .void(void_grpc_request)
            .await
            .expect("gRPC payment_void call failed")
            .into_inner();

        // Verify the void response
        assert!(
            void_response.status == i32::from(PaymentStatus::Voided),
            "Payment should be in VOIDED state after void"
        );
    });
}

// Test register (setup mandate) flow
#[tokio::test]
async fn test_register() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        // Create the register request
        let request = create_register_request();

        // Add metadata headers
        let mut grpc_request = Request::new(request);
        add_aci_metadata(&mut grpc_request);

        // Send the request
        let response = client
            .register(grpc_request)
            .await
            .expect("gRPC register call failed")
            .into_inner();

        // Verify the response
        assert!(
            response.registration_id.is_some(),
            "Registration ID should be present"
        );

        // Check if we have a mandate reference
        assert!(
            response.mandate_reference.is_some(),
            "Mandate reference should be present"
        );

        // Verify the mandate reference has the expected structure
        if let Some(mandate_ref) = &response.mandate_reference {
            assert!(
                mandate_ref.mandate_id.is_some(),
                "Mandate ID should be present"
            );

            // Verify the mandate ID is not empty
            if let Some(mandate_id) = &mandate_ref.mandate_id {
                assert!(!mandate_id.is_empty(), "Mandate ID should not be empty");
            }
        }

        // Verify no error occurred
        assert!(
            response.error_message.is_none() || response.error_message.as_ref().unwrap().is_empty(),
            "No error message should be present for successful register"
        );
    });
}

// Test repeat payment (MIT) flow using previously created mandate
#[tokio::test]
async fn test_repeat_everything() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        // First, create a mandate using register
        let register_request = create_register_request();

        let mut register_grpc_request = Request::new(register_request);
        add_aci_metadata(&mut register_grpc_request);

        let register_response = client
            .register(register_grpc_request)
            .await
            .expect("gRPC register call failed")
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
            .mandate_id
            .as_ref()
            .expect("Mandate ID should be present");

        // Now perform a repeat payment using the mandate
        let repeat_request = create_repeat_payment_request(mandate_id);

        let mut repeat_grpc_request = Request::new(repeat_request);
        add_aci_metadata(&mut repeat_grpc_request);

        // Send the repeat payment request
        let repeat_response = client
            .repeat_everything(repeat_grpc_request)
            .await
            .expect("gRPC repeat_everything call failed")
            .into_inner();

        // Verify the response
        assert!(
            repeat_response.transaction_id.is_some(),
            "Transaction ID should be present"
        );
    });
}
