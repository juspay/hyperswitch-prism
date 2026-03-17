#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use hyperswitch_masking::Secret;
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
        mandate_reference::MandateIdType, payment_method,
        payment_service_client::PaymentServiceClient,
        recurring_payment_service_client::RecurringPaymentServiceClient, AcceptanceType, Address,
        AuthenticationType, CaptureMethod, CardDetails, ConnectorMandateReferenceId, CountryAlpha2,
        Currency, CustomerAcceptance, FutureUsage, MandateReference, PaymentAddress, PaymentMethod,
        PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse,
        PaymentServiceCaptureRequest, PaymentServiceGetRequest, PaymentServiceRefundRequest,
        PaymentServiceSetupRecurringRequest, PaymentServiceVoidRequest, PaymentStatus,
        RecurringPaymentServiceChargeRequest, RefundStatus,
    },
};
use rand::Rng;
use std::collections::HashMap;
use tonic::{transport::Channel, Request};
use uuid::Uuid;

const CONNECTOR_NAME: &str = "payload";
const AUTH_TYPE: &str = "currency-auth-key";
const MERCHANT_ID: &str = "merchant_payload_test";

// Test card data
const TEST_CARD_NUMBER: &str = "4111111111111111";
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "2050";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Test User";
const TEST_EMAIL: &str = "customer@example.com";

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn generate_unique_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4())
}

fn add_payload_metadata<T>(request: &mut Request<T>) {
    // Get API credentials using the common credential loading utility
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load Payload credentials");

    let auth_key_map_json = match auth {
        domain_types::router_data::ConnectorAuthType::CurrencyAuthKey { auth_key_map } => {
            // Convert the auth_key_map to JSON string format expected by the metadata
            serde_json::to_string(&auth_key_map).expect("Failed to serialize auth_key_map")
        }
        _ => panic!("Expected CurrencyAuthKey auth type for Payload"),
    };

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

    // Generate random billing address to avoid duplicates
    let mut rng = rand::thread_rng();
    let random_street_num = rng.gen_range(100..9999);
    let random_zip_suffix = rng.gen_range(1000..9999);

    let address = PaymentAddress {
        billing_address: Some(Address {
            first_name: Some("John".to_string().into()),
            last_name: Some("Doe".to_string().into()),
            email: Some(TEST_EMAIL.to_string().into()),
            line1: Some(format!("{random_street_num} Main St").into()),
            city: Some("San Francisco".to_string().into()),
            state: Some("CA".to_string().into()),
            zip_code: Some(format!("{random_zip_suffix}").into()),
            country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
            ..Default::default()
        }),
        shipping_address: None,
    };

    // Use random amount to avoid duplicates
    let mut rng = rand::thread_rng();
    let unique_amount = rng.gen_range(1000..10000); // Amount between $10.00 and $100.00

    PaymentServiceAuthorizeRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: unique_amount,
            currency: i32::from(Currency::Usd),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some("https://example.com/return".to_string()),
        webhook_url: Some("https://example.com/webhook".to_string()),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            name: None,
            id: None,
            connector_customer_id: None,
            phone_number: None,
            phone_country_code: None,
        }),
        address: Some(address),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(generate_unique_id("payload_test")),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        ..Default::default()
    }
}

fn create_payment_sync_request(transaction_id: &str, amount: i64) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: transaction_id.to_string(),
        encoded_data: None,
        capture_method: None,
        merchant_transaction_id: None,
        handle_response: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: amount,
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
    }
}

fn create_payment_capture_request(
    transaction_id: &str,
    amount: i64,
) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: transaction_id.to_string(),
        amount_to_capture: Some(grpc_api_types::payments::Money {
            minor_amount: amount,
            currency: i32::from(Currency::Usd),
        }),
        multiple_capture_data: None,
        merchant_capture_id: None,
        ..Default::default()
    }
}

fn create_payment_void_request(transaction_id: &str, amount: i64) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: transaction_id.to_string(),
        cancellation_reason: None,
        merchant_void_id: Some(generate_unique_id("payload_void")),
        all_keys_required: None,
        browser_info: None,
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: amount,
            currency: i32::from(Currency::Usd),
        }),
        ..Default::default()
    }
}

fn create_refund_request(transaction_id: &str, amount: i64) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some(generate_unique_id("refund")),
        connector_transaction_id: transaction_id.to_string(),
        payment_amount: amount,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: amount,
            currency: i32::from(Currency::Usd),
        }),
        ..Default::default()
    }
}

fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    response
        .connector_transaction_id
        .as_ref()
        .expect("Failed to extract connector transaction ID from response")
        .clone()
}

#[allow(clippy::field_reassign_with_default)]
fn create_repeat_payment_request(mandate_id: &str) -> RecurringPaymentServiceChargeRequest {
    // Use random amount to avoid duplicates
    let mut rng = rand::thread_rng();
    let unique_amount = rng.gen_range(1000..10000); // Amount between $10.00 and $100.00

    let mandate_reference = MandateReference {
        mandate_id_type: Some(MandateIdType::ConnectorMandateId(
            ConnectorMandateReferenceId {
                connector_mandate_request_reference_id: None,
                connector_mandate_id: Some(mandate_id.to_string()),
                payment_method_id: None,
            },
        )),
    };

    let mut metadata_map = HashMap::new();
    metadata_map.insert("order_type".to_string(), "recurring".to_string());
    metadata_map.insert(
        "customer_note".to_string(),
        "Recurring payment using saved payment method".to_string(),
    );

    let metadata_json = serde_json::to_string(&metadata_map).unwrap();

    RecurringPaymentServiceChargeRequest {
        merchant_charge_id: Some(generate_unique_id("repeat")),
        connector_recurring_payment_id: Some(mandate_reference),
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: unique_amount,
            currency: i32::from(Currency::Usd),
        }),
        merchant_order_id: Some(generate_unique_id("repeat_order")),
        metadata: Some(Secret::new(metadata_json)),
        webhook_url: None,
        capture_method: None,
        email: Some(Secret::new(TEST_EMAIL.to_string())),
        browser_info: None,
        test_mode: None,
        payment_method_type: None,
        state: None,
        ..Default::default()
    }
}

fn create_register_request() -> PaymentServiceSetupRecurringRequest {
    create_register_request_with_prefix("payload_mandate")
}

fn create_register_request_with_prefix(_prefix: &str) -> PaymentServiceSetupRecurringRequest {
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

    // Use random values to create unique data to avoid duplicate detection
    let mut rng = rand::thread_rng();
    let random_street_num = rng.gen_range(1000..9999);
    let unique_zip = format!("{}", rng.gen_range(10000..99999));
    let random_id = rng.gen_range(1000..9999);
    let unique_email = format!("customer{random_id}@example.com");
    let unique_first_name = format!("John{random_id}");

    PaymentServiceSetupRecurringRequest {
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: 0, // Setup mandate with 0 amount
            currency: i32::from(Currency::Usd),
        }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(unique_email.clone().into()),
            name: Some(format!("{unique_first_name} Doe")),
            id: None,
            connector_customer_id: None,
            phone_number: None,
            phone_country_code: None,
        }),
        customer_acceptance: Some(CustomerAcceptance {
            acceptance_type: i32::from(AcceptanceType::Offline),
            accepted_at: 0,
            online_mandate_details: None,
        }),
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                first_name: Some(unique_first_name.into()),
                last_name: Some("Doe".to_string().into()),
                line1: Some(format!("{random_street_num} Market St").into()),
                line2: None,
                line3: None,
                city: Some("San Francisco".to_string().into()),
                state: Some("CA".to_string().into()),
                zip_code: Some(unique_zip.into()),
                country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
                phone_number: None,
                phone_country_code: None,
                email: Some(unique_email.into()),
            }),
            shipping_address: None,
        }),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        setup_future_usage: Some(i32::from(FutureUsage::OffSession)),
        enrolled_for_3ds: false,
        metadata: None,
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
            grpc_api_types::health_check::health_check_response::ServingStatus::Serving,
            "Health check should return Serving status"
        );
    });
}

#[tokio::test]
async fn test_authorize_psync_void() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Wait 30 seconds before making API call to avoid parallel test conflicts
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        // Step 1: Authorize with manual capture
        let request = create_authorize_request(CaptureMethod::Manual);
        let amount = request.amount; // Capture amount from request
        let mut grpc_request = Request::new(request);
        add_payload_metadata(&mut grpc_request);

        let auth_response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        let transaction_id = extract_transaction_id(&auth_response);

        assert_eq!(
            auth_response.status,
            i32::from(PaymentStatus::Authorized),
            "Payment should be in Authorized state"
        );

        // Step 2: PSync
        let sync_request =
            create_payment_sync_request(&transaction_id, amount.unwrap().minor_amount);
        let mut sync_grpc_request = Request::new(sync_request);
        add_payload_metadata(&mut sync_grpc_request);

        let sync_response = client
            .get(sync_grpc_request)
            .await
            .expect("gRPC sync call failed")
            .into_inner();

        assert!(
            !sync_response.connector_transaction_id.is_empty(),
            "Sync response should contain transaction ID"
        );

        // Step 3: Void
        let void_request =
            create_payment_void_request(&transaction_id, amount.unwrap().minor_amount);
        let mut void_grpc_request = Request::new(void_request);
        add_payload_metadata(&mut void_grpc_request);

        let void_response = client
            .void(void_grpc_request)
            .await
            .expect("gRPC void call failed")
            .into_inner();

        assert_eq!(
            void_response.status,
            i32::from(PaymentStatus::Voided),
            "Payment should be in Voided state after void"
        );
    });
}

#[tokio::test]
async fn test_authorize_capture_refund_rsync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Wait 30 seconds before making API call to avoid parallel test conflicts
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        // Step 1: Authorize with manual capture
        let request = create_authorize_request(CaptureMethod::Manual);
        let amount = request.amount; // Capture amount from request
        let mut grpc_request = Request::new(request);
        add_payload_metadata(&mut grpc_request);

        let auth_response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        let transaction_id = extract_transaction_id(&auth_response);

        assert_eq!(
            auth_response.status,
            i32::from(PaymentStatus::Authorized),
            "Payment should be in Authorized state"
        );

        // Step 2: Capture
        let capture_request =
            create_payment_capture_request(&transaction_id, amount.unwrap().minor_amount);
        let mut capture_grpc_request = Request::new(capture_request);
        add_payload_metadata(&mut capture_grpc_request);

        let capture_response = client
            .capture(capture_grpc_request)
            .await
            .expect("gRPC capture call failed")
            .into_inner();

        assert_eq!(
            capture_response.status,
            i32::from(PaymentStatus::Charged),
            "Payment should be in Charged state after capture"
        );

        // Step 3: Refund
        let refund_request = create_refund_request(&transaction_id, amount.unwrap().minor_amount);
        let mut refund_grpc_request = Request::new(refund_request);
        add_payload_metadata(&mut refund_grpc_request);

        let refund_response = client
            .refund(refund_grpc_request)
            .await
            .expect("gRPC refund call failed")
            .into_inner();

        let refund_id = refund_response.connector_refund_id.clone();

        assert!(
            refund_response.status == i32::from(RefundStatus::RefundSuccess)
                || refund_response.status == i32::from(RefundStatus::RefundPending),
            "Refund should be in RefundSuccess or RefundPending state"
        );

        // Step 4: RSync (Refund Sync)
        let rsync_request = PaymentServiceGetRequest {
            connector_transaction_id: refund_id,
            encoded_data: None,
            capture_method: None,
            handle_response: None,
            merchant_transaction_id: None,
            amount,
            state: None,
            metadata: None,
            connector_feature_data: None,
            setup_future_usage: None,
            sync_type: None,
            connector_order_reference_id: None,
            test_mode: None,
            payment_experience: None,
        };
        let mut rsync_grpc_request = Request::new(rsync_request);
        add_payload_metadata(&mut rsync_grpc_request);

        let rsync_response = client
            .get(rsync_grpc_request)
            .await
            .expect("gRPC refund sync call failed")
            .into_inner();

        assert!(
            !rsync_response.connector_transaction_id.is_empty(),
            "Refund sync response should contain transaction ID"
        );
    });
}

#[tokio::test]
async fn test_setup_mandate() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // Wait 30 seconds before making API call to avoid parallel test conflicts
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        // Create setup mandate request (zero amount payment to save card)
        let request = create_register_request();
        let mut grpc_request = Request::new(request);
        add_payload_metadata(&mut grpc_request);

        let response = client
            .setup_recurring(grpc_request)
            .await
            .expect("gRPC setup_recurring call failed")
            .into_inner();

        // Verify we got a mandate reference
        assert!(
            response.mandate_reference.is_some(),
            "Mandate reference should be present"
        );

        if let Some(MandateIdType::ConnectorMandateId(mandate_ref)) =
            &response.mandate_reference.and_then(|m| m.mandate_id_type)
        {
            assert!(
                mandate_ref.connector_mandate_id.is_some()
                    || mandate_ref.payment_method_id.is_some(),
                "Mandate ID or payment method ID should be present"
            );

            if let Some(mandate_id) = &mandate_ref.connector_mandate_id {
                assert!(!mandate_id.is_empty(), "Mandate ID should not be empty");
            }

            if let Some(pm_id) = &mandate_ref.payment_method_id {
                assert!(!pm_id.is_empty(), "Payment method ID should not be empty");
            }
        }

        // Verify status is success
        assert_eq!(
            response.status,
            i32::from(PaymentStatus::Charged),
            "Setup mandate should be in Charged/Success state"
        );
    });
}

#[tokio::test]
//Ignored as getting "duplicate transaction" error when run in CI pipeline
#[ignore]
async fn test_repeat_payment() {
    grpc_test!([client: PaymentServiceClient<Channel>, recurring_client: RecurringPaymentServiceClient<Channel>], {
        // NOTE: This test may fail with "duplicate transaction" error if run too soon
        // after other tests that use the same test card. Payload has duplicate detection.
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        let register_request = create_register_request_with_prefix("payload_repeat_test");
        let mut register_grpc_request = Request::new(register_request);
        add_payload_metadata(&mut register_grpc_request);

        let register_response = client
            .setup_recurring(register_grpc_request)
            .await
            .expect("gRPC setup_recurring call failed")
            .into_inner();

        if register_response.mandate_reference.is_none() {
            panic!(
                "Mandate reference should be present. Status: {}, Error: {:?}",
                register_response.status,
                register_response
                    .error
                    .and_then(|e| e.connector_details)
                    .and_then(|d| d.message)
            );
        }

        let mandate_ref = register_response
            .mandate_reference
            .as_ref()
            .expect("Mandate reference should be present");

        let mandate_id_opt = mandate_ref
            .mandate_id_type
            .clone()
            .and_then(|id| match id {
                MandateIdType::ConnectorMandateId(connector_id) => {
                    connector_id.connector_mandate_id
                }
                _ => None,
            });
        let mandate_id = mandate_id_opt.as_deref().expect("mandate_id should be present");

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let repeat_request = create_repeat_payment_request(mandate_id);
        let mut repeat_grpc_request = Request::new(repeat_request);
        add_payload_metadata(&mut repeat_grpc_request);

        let repeat_response = recurring_client
            .charge(repeat_grpc_request)
            .await
            .expect("gRPC charge call failed")
            .into_inner();

        assert!(
            repeat_response.connector_transaction_id.is_some(),
            "Transaction ID should be present in repeat payment response"
        );

        assert_eq!(
            repeat_response.status,
            i32::from(PaymentStatus::Charged),
            "Repeat payment should be in Charged state with automatic capture"
        );
    });
}
