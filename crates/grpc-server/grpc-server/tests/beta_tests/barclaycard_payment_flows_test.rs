#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

// NOTE: These tests use #[serial] attribute to run sequentially
// This stops tests from failing due to parallel run

use grpc_server::app;
use ucs_env::configs;
use hyperswitch_masking::{ExposeInterface, Secret};
use serial_test::serial;
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
        identifier::IdType, payment_method, payment_service_client::PaymentServiceClient,
        refund_service_client::RefundServiceClient, Address, AuthenticationType, CaptureMethod,
        CardDetails, CountryAlpha2, Currency, Identifier, PaymentAddress, PaymentMethod,
        PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceGetRequest,
        PaymentServiceRefundRequest, PaymentServiceVoidRequest, PaymentStatus,
        RefundServiceGetRequest, RefundStatus,
    },
};
use rand::Rng;
use tonic::{transport::Channel, Request};
use uuid::Uuid;

const CONNECTOR_NAME: &str = "barclaycard";
const AUTH_TYPE: &str = "signature-key";
const MERCHANT_ID: &str = "merchant_barclaycard_test";

const TEST_CARD_NUMBER: &str = "4111111111111111";
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "30";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "John Doe";
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

fn load_barclaycard_credentials() -> (Secret<String>, Secret<String>, Secret<String>) {
    if let (Ok(api_key), Ok(key1), Ok(api_secret)) = (
        std::env::var("TEST_BARCLAYCARD_API_KEY"),
        std::env::var("TEST_BARCLAYCARD_KEY1"),
        std::env::var("TEST_BARCLAYCARD_API_SECRET"),
    ) {
        return (
            Secret::new(api_key),
            Secret::new(key1),
            Secret::new(api_secret),
        );
    }

    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load Barclaycard credentials");

    match auth {
        domain_types::router_data::ConnectorAuthType::SignatureKey {
            api_key,
            key1,
            api_secret,
        } => (api_key, key1, api_secret),
        _ => panic!("Expected SignatureKey auth type for Barclaycard"),
    }
}

fn add_barclaycard_metadata<T>(request: &mut Request<T>) {
    let (api_key, key1, api_secret) = load_barclaycard_credentials();

    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request
        .metadata_mut()
        .append("x-auth", AUTH_TYPE.parse().expect("Failed to parse x-auth"));
    request.metadata_mut().append(
        "x-api-key",
        api_key.expose().parse().expect("Failed to parse x-api-key"),
    );
    request.metadata_mut().append(
        "x-key1",
        key1.expose().parse().expect("Failed to parse x-key1"),
    );
    request.metadata_mut().append(
        "x-api-secret",
        api_secret
            .expose()
            .parse()
            .expect("Failed to parse x-api-secret"),
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

    let mut rng = rand::thread_rng();
    let random_street_num = rng.gen_range(100..9999);
    let random_zip_suffix = rng.gen_range(10000..99999);

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

    let mut rng = rand::thread_rng();
    let unique_amount = rng.gen_range(1000..10000);

    PaymentServiceAuthorizeRequest {
        amount: unique_amount,
        minor_amount: unique_amount,
        currency: i32::from(Currency::Usd),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        return_url: Some("https://example.com/return".to_string()),
        webhook_url: Some("https://example.com/webhook".to_string()),
        email: Some(TEST_EMAIL.to_string().into()),
        address: Some(address),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("barclaycard_test"))),
        }),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        ..Default::default()
    }
}

fn create_payment_sync_request(transaction_id: &str, amount: i64) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("barclaycard_sync"))),
        }),
        capture_method: None,
        handle_response: None,
        amount,
        currency: i32::from(Currency::Usd),
        state: None,
        encoded_data: None,
        connector_metadata: HashMap::new(),
        setup_future_usage: None,
        sync_type: None,
    }
}

fn create_payment_capture_request(
    transaction_id: &str,
    amount: i64,
) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        amount_to_capture: amount,
        currency: i32::from(Currency::Usd),
        multiple_capture_data: None,
        request_ref_id: None,
        ..Default::default()
    }
}

fn create_payment_void_request(transaction_id: &str, amount: i64) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        cancellation_reason: Some("Customer requested cancellation".to_string()),
        request_ref_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("barclaycard_void"))),
        }),
        all_keys_required: None,
        browser_info: None,
        connector_metadata: Default::default(),
        amount: Some(amount),
        currency: Some(i32::from(Currency::Usd)),
        state: None,
        merchant_account_metadata: Default::default(),
    }
}

fn create_refund_request(
    transaction_id: &str,
    amount: i64,
    refund_amount: i64,
) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        refund_id: generate_unique_id("refund"),
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        currency: i32::from(Currency::Usd),
        payment_amount: amount,
        refund_amount,
        minor_payment_amount: amount,
        minor_refund_amount: refund_amount,
        reason: None,
        webhook_url: None,
        browser_info: None,
        merchant_account_id: None,
        capture_method: None,
        request_ref_id: None,
        ..Default::default()
    }
}

fn create_refund_sync_request(transaction_id: &str, refund_id: &str) -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        refund_id: refund_id.to_string(),
        refund_reason: None,
        request_ref_id: None,
        ..Default::default()
    }
}

#[tokio::test]
#[serial]
async fn test_health() {
    grpc_test!(client, HealthClient<Channel>, {
        let health_request = HealthCheckRequest {
            service: "".to_string(),
        };
        let response = client
            .check(health_request)
            .await
            .expect("Health check failed");
        let health_response = response.into_inner();
        assert_eq!(health_response.status, 1);
    });
}

#[tokio::test]
#[serial]
async fn test_payment_authorization_auto_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let authorize_request = create_authorize_request(CaptureMethod::Automatic);
        let mut request = Request::new(authorize_request);
        add_barclaycard_metadata(&mut request);

        let response = client
            .authorize(request)
            .await
            .expect("Payment authorization failed");
        let authorize_response = response.into_inner();

        if let Some(error_message) = &authorize_response.error_message {
            panic!(
                "Authorization failed with error: {} (code: {:?}, reason: {:?})",
                error_message, authorize_response.error_code, authorize_response.error_reason
            );
        }

        assert!(authorize_response.transaction_id.is_some());
        let status = PaymentStatus::try_from(authorize_response.status).unwrap();
        assert!(
            matches!(status, PaymentStatus::Charged | PaymentStatus::Pending),
            "Expected Charged or Pending status, got {status:?}"
        );
    });
}

#[tokio::test]
#[serial]
async fn test_payment_authorization_manual_capture() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let authorize_request = create_authorize_request(CaptureMethod::Manual);
        let amount = authorize_request.amount;
        let mut request = Request::new(authorize_request);
        add_barclaycard_metadata(&mut request);

        let response = client
            .authorize(request)
            .await
            .expect("Payment authorization failed");
        let authorize_response = response.into_inner();

        if let Some(error_message) = &authorize_response.error_message {
            panic!(
                "Manual capture authorization failed with error: {} (code: {:?}, reason: {:?})",
                error_message, authorize_response.error_code, authorize_response.error_reason
            );
        }

        assert!(authorize_response.transaction_id.is_some());
        let transaction_id = authorize_response
            .transaction_id
            .as_ref()
            .and_then(|id| id.id_type.as_ref())
            .and_then(|id_type| match id_type {
                IdType::Id(id) => Some(id.clone()),
                _ => None,
            })
            .expect("Failed to extract transaction ID");

        let status = PaymentStatus::try_from(authorize_response.status).unwrap();
        assert!(
            matches!(status, PaymentStatus::Authorized | PaymentStatus::Pending),
            "Expected Authorized or Pending status after authorization, got {status:?}"
        );

        let capture_request = create_payment_capture_request(&transaction_id, amount);
        let mut capture_req = Request::new(capture_request);
        add_barclaycard_metadata(&mut capture_req);

        let capture_response = client
            .capture(capture_req)
            .await
            .expect("Payment capture failed");
        let capture_result = capture_response.into_inner();

        let capture_status = PaymentStatus::try_from(capture_result.status).unwrap();
        assert!(
            matches!(
                capture_status,
                PaymentStatus::Charged | PaymentStatus::Pending
            ),
            "Expected Charged or Pending status after capture, got {capture_status:?}"
        );
    });
}

// NOTE: Payment sync test requires a 10-second delay due to Barclaycard's transaction
// search endpoint (/tss/v2/transactions/{id}) having indexing delays. Transactions
// need time to propagate to their search system before they can be queried.
#[tokio::test]
#[serial]
async fn test_payment_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let authorize_request = create_authorize_request(CaptureMethod::Automatic);
        let amount = authorize_request.amount;
        let mut request = Request::new(authorize_request);
        add_barclaycard_metadata(&mut request);

        let response = client
            .authorize(request)
            .await
            .expect("Payment authorization failed");
        let authorize_response = response.into_inner();

        if let Some(error_message) = &authorize_response.error_message {
            panic!(
                "Payment sync authorization failed with error: {} (code: {:?}, reason: {:?})",
                error_message, authorize_response.error_code, authorize_response.error_reason
            );
        }

        assert!(authorize_response.transaction_id.is_some());
        let transaction_id = authorize_response
            .transaction_id
            .as_ref()
            .and_then(|id| id.id_type.as_ref())
            .and_then(|id_type| match id_type {
                IdType::Id(id) => Some(id.clone()),
                _ => None,
            })
            .expect("Failed to extract transaction ID");

        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        let sync_request = create_payment_sync_request(&transaction_id, amount);
        let mut sync_req = Request::new(sync_request);
        add_barclaycard_metadata(&mut sync_req);

        let sync_response = client.get(sync_req).await.expect("Payment sync failed");
        let sync_result = sync_response.into_inner();

        if let Some(error_message) = &sync_result.error_message {
            panic!(
                "Payment sync failed with error: {} (code: {:?}, reason: {:?})",
                error_message, sync_result.error_code, sync_result.error_reason
            );
        }

        assert!(sync_result.transaction_id.is_some());
        let sync_status = PaymentStatus::try_from(sync_result.status).unwrap();
        assert!(
            matches!(
                sync_status,
                PaymentStatus::Charged | PaymentStatus::Pending | PaymentStatus::Authorized
            ),
            "Expected valid payment status, got {sync_status:?}"
        );
    });
}

#[tokio::test]
#[serial]
async fn test_payment_void() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let authorize_request = create_authorize_request(CaptureMethod::Manual);
        let amount = authorize_request.amount;
        let mut request = Request::new(authorize_request);
        add_barclaycard_metadata(&mut request);

        let response = client
            .authorize(request)
            .await
            .expect("Payment authorization failed");
        let authorize_response = response.into_inner();

        if let Some(error_message) = &authorize_response.error_message {
            panic!(
                "Void authorization failed with error: {} (code: {:?}, reason: {:?})",
                error_message, authorize_response.error_code, authorize_response.error_reason
            );
        }

        assert!(authorize_response.transaction_id.is_some());
        let transaction_id = authorize_response
            .transaction_id
            .as_ref()
            .and_then(|id| id.id_type.as_ref())
            .and_then(|id_type| match id_type {
                IdType::Id(id) => Some(id.clone()),
                _ => None,
            })
            .expect("Failed to extract transaction ID");

        let status = PaymentStatus::try_from(authorize_response.status).unwrap();
        assert!(
            matches!(status, PaymentStatus::Authorized | PaymentStatus::Pending),
            "Expected Authorized or Pending status after authorization, got {status:?}"
        );

        let void_request = create_payment_void_request(&transaction_id, amount);
        let mut void_req = Request::new(void_request);
        add_barclaycard_metadata(&mut void_req);

        let void_response = client.void(void_req).await.expect("Payment void failed");
        let void_result = void_response.into_inner();

        if let Some(error_message) = &void_result.error_message {
            panic!(
                "Payment void failed with error: {} (code: {:?}, reason: {:?})",
                error_message, void_result.error_code, void_result.error_reason
            );
        }

        let void_status = PaymentStatus::try_from(void_result.status).unwrap();
        assert!(
            matches!(void_status, PaymentStatus::Voided | PaymentStatus::Pending),
            "Expected Voided or Pending status after void, got {void_status:?}"
        );
    });
}

#[tokio::test]
#[serial]
async fn test_refund() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let authorize_request = create_authorize_request(CaptureMethod::Automatic);
        let amount = authorize_request.amount;
        let mut request = Request::new(authorize_request);
        add_barclaycard_metadata(&mut request);

        let response = client
            .authorize(request)
            .await
            .expect("Payment authorization failed");
        let authorize_response = response.into_inner();

        if let Some(error_message) = &authorize_response.error_message {
            panic!(
                "Refund authorization failed with error: {} (code: {:?}, reason: {:?})",
                error_message, authorize_response.error_code, authorize_response.error_reason
            );
        }

        assert!(authorize_response.transaction_id.is_some());
        let transaction_id = authorize_response
            .transaction_id
            .as_ref()
            .and_then(|id| id.id_type.as_ref())
            .and_then(|id_type| match id_type {
                IdType::Id(id) => Some(id.clone()),
                _ => None,
            })
            .expect("Failed to extract transaction ID");

        let status = PaymentStatus::try_from(authorize_response.status).unwrap();
        assert!(
            matches!(status, PaymentStatus::Charged | PaymentStatus::Pending),
            "Expected Charged or Pending status, got {status:?}"
        );

        let refund_amount = amount;
        let refund_request = create_refund_request(&transaction_id, amount, refund_amount);
        let mut refund_req = Request::new(refund_request);
        add_barclaycard_metadata(&mut refund_req);

        let refund_response = client.refund(refund_req).await.expect("Refund failed");
        let refund_result = refund_response.into_inner();

        let refund_status = RefundStatus::try_from(refund_result.status).unwrap();
        assert!(
            matches!(
                refund_status,
                RefundStatus::RefundSuccess | RefundStatus::RefundPending
            ),
            "Expected RefundSuccess or RefundPending refund status, got {refund_status:?}"
        );
    });
}

// NOTE: Refund sync test requires a 10-second delay due to Barclaycard's transaction
// search endpoint (/tss/v2/transactions/{id}) having indexing delays. Refunds need
// time to propagate to their search system before they can be queried.
#[tokio::test]
#[serial]
async fn test_refund_sync() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        grpc_test!(refund_client, RefundServiceClient<Channel>, {
            let authorize_request = create_authorize_request(CaptureMethod::Automatic);
            let amount = authorize_request.amount;
            let mut request = Request::new(authorize_request);
            add_barclaycard_metadata(&mut request);

            let response = client
                .authorize(request)
                .await
                .expect("Payment authorization failed");
            let authorize_response = response.into_inner();

            if let Some(error_message) = &authorize_response.error_message {
                panic!(
                    "Refund sync authorization failed with error: {} (code: {:?}, reason: {:?})",
                    error_message, authorize_response.error_code, authorize_response.error_reason
                );
            }

            assert!(authorize_response.transaction_id.is_some());
            let transaction_id = authorize_response
                .transaction_id
                .as_ref()
                .and_then(|id| id.id_type.as_ref())
                .and_then(|id_type| match id_type {
                    IdType::Id(id) => Some(id.clone()),
                    _ => None,
                })
                .expect("Failed to extract transaction ID");

            let refund_amount = amount;
            let refund_request = create_refund_request(&transaction_id, amount, refund_amount);
            let mut refund_req = Request::new(refund_request);
            add_barclaycard_metadata(&mut refund_req);

            let refund_response = client.refund(refund_req).await.expect("Refund failed");
            let refund_result = refund_response.into_inner();

            let refund_id = &refund_result.refund_id;

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

            let refund_sync_request = create_refund_sync_request(&transaction_id, refund_id);
            let mut refund_sync_req = Request::new(refund_sync_request);
            add_barclaycard_metadata(&mut refund_sync_req);

            let refund_sync_response = refund_client
                .get(refund_sync_req)
                .await
                .expect("Refund sync failed");
            let refund_sync_result = refund_sync_response.into_inner();

            if let Some(error_message) = &refund_sync_result.error_message {
                panic!(
                    "Refund sync failed with error: {} (code: {:?}, reason: {:?})",
                    error_message, refund_sync_result.error_code, refund_sync_result.error_reason
                );
            }

            let refund_sync_status = RefundStatus::try_from(refund_sync_result.status).unwrap();
            assert!(
                matches!(
                    refund_sync_status,
                    RefundStatus::RefundSuccess | RefundStatus::RefundPending
                ),
                "Expected RefundSuccess or RefundPending refund status in sync, got {refund_sync_status:?}"
            );
        });
    });
}
