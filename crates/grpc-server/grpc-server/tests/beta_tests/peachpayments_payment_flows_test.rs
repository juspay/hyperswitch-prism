#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use ucs_env::configs;
mod common;
mod utils;

use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use cards::CardNumber;
use grpc_api_types::payments::{
    identifier::IdType, payment_method, direct_payment_service_client::DirectPaymentServiceClient,
    AuthenticationType, CaptureMethod, CardDetails, Currency, Identifier, PaymentMethod,
    PaymentServiceAuthorizeRequest, PaymentServiceAuthorizeResponse, PaymentServiceCaptureRequest,
    PaymentServiceGetRequest, PaymentServiceRefundRequest, PaymentServiceVoidRequest,
};
use tonic::{transport::Channel, Request};
use uuid::Uuid;

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn generate_unique_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4())
}

fn extract_transaction_id(response: &PaymentServiceAuthorizeResponse) -> String {
    match &response.connector_transaction_id {
        Some(id) => match id.id_type.as_ref() {
            Some(IdType::Id(id)) => id.clone(),
            _ => "test_transaction_id".to_string(),
        },
        None => "test_transaction_id".to_string(),
    }
}

const CONNECTOR_NAME: &str = "peachpayments";
const MERCHANT_ID: &str = "merchant_1234";
const TEST_AMOUNT: i64 = 10000;
const TEST_CARD_NUMBER: &str = "4000000000001091";
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "2027";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Test User";
const TEST_EMAIL: &str = "test@example.com";

fn add_peachpayments_metadata<T>(request: &mut Request<T>) {
    // Load API credentials using the common credential loading utility
    let auth = utils::credential_utils::load_connector_auth(CONNECTOR_NAME)
        .expect("Failed to load PeachPayments credentials");

    let (api_key, tenant_id): (String, String) = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
            (api_key.expose().to_string(), key1.expose().to_string())
        }
        _ => panic!("Expected BodyKey auth type for PeachPayments"),
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
        .append("x-key1", tenant_id.parse().expect("Failed to parse x-key1"));
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
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Zar),
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
        address: Some(grpc_api_types::payments::PaymentAddress::default()),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("peachpayments_test"))),
        }),
        enrolled_for_3ds: Some(false),
        request_incremental_authorization: Some(false),
        capture_method: Some(i32::from(capture_method)),
        ..Default::default()
    }
}

fn create_payment_capture_request(transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        amount_to_capture: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Zar),
        }),
        merchant_capture_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("capture"))),
        }),
        ..Default::default()
    }
}

fn create_payment_void_request(transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        connector_transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        cancellation_reason: Some("requested_by_customer".to_string()),
        merchant_void_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("void"))),
        }),
        ..Default::default()
    }
}

fn create_refund_request(transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some(Identifier {
            id_type: Some(IdType::Id(generate_unique_id("refund"))),
        }),
        connector_transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        payment_amount: TEST_AMOUNT,
        refund_amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Zar),
        }),
        reason: Some("requested_by_customer".to_string()),
        ..Default::default()
    }
}

fn create_payment_sync_request(transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        connector_transaction_id: Some(Identifier {
            id_type: Some(IdType::Id(transaction_id.to_string())),
        }),
        amount: Some(grpc_api_types::payments::Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Zar),
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_peachpayments_authorize_auto_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let request = create_authorize_request(CaptureMethod::Automatic);
        let mut grpc_request = Request::new(request);
        add_peachpayments_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            response.connector_transaction_id.is_some() || response.error.is_some(),
            "Response should have either transaction_id or error"
        );
    });
}

#[tokio::test]
async fn test_peachpayments_authorize_manual_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let request = create_authorize_request(CaptureMethod::Manual);
        let mut grpc_request = Request::new(request);
        add_peachpayments_metadata(&mut grpc_request);

        let response = client
            .authorize(grpc_request)
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        assert!(
            response.connector_transaction_id.is_some() || response.error.is_some(),
            "Response should have either transaction_id or error"
        );
    });
}

#[tokio::test]
async fn test_peachpayments_authorize_and_capture() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Manual);
        let mut auth_grpc_request = Request::new(auth_request);
        add_peachpayments_metadata(&mut auth_grpc_request);

        let auth_result = client.authorize(auth_grpc_request).await;

        if let Ok(auth_response) = auth_result {
            let auth_response = auth_response.into_inner();

            if auth_response.error.is_none() {
                let transaction_id = extract_transaction_id(&auth_response);

                let capture_request = create_payment_capture_request(&transaction_id);
                let mut capture_grpc_request = Request::new(capture_request);
                add_peachpayments_metadata(&mut capture_grpc_request);

                let capture_response = client
                    .capture(capture_grpc_request)
                    .await
                    .expect("gRPC capture call failed")
                    .into_inner();

                assert!(
                    capture_response.connector_transaction_id.is_some(),
                    "Transaction ID should be present in capture response"
                );
            }
        }
    });
}

#[tokio::test]
async fn test_peachpayments_authorize_and_void() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Manual);
        let mut auth_grpc_request = Request::new(auth_request);
        add_peachpayments_metadata(&mut auth_grpc_request);

        let auth_result = client.authorize(auth_grpc_request).await;

        if let Ok(auth_response) = auth_result {
            let auth_response = auth_response.into_inner();

            if auth_response.error.is_none() {
                let transaction_id = extract_transaction_id(&auth_response);

                let void_request = create_payment_void_request(&transaction_id);
                let mut void_grpc_request = Request::new(void_request);
                add_peachpayments_metadata(&mut void_grpc_request);

                let void_response = client
                    .void(void_grpc_request)
                    .await
                    .expect("gRPC void call failed")
                    .into_inner();

                assert!(
                    void_response.connector_transaction_id.is_some(),
                    "Transaction ID should be present in void response"
                );
            }
        }
    });
}

#[tokio::test]
async fn test_peachpayments_refund() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Automatic);
        let mut auth_grpc_request = Request::new(auth_request);
        add_peachpayments_metadata(&mut auth_grpc_request);

        let auth_result = client.authorize(auth_grpc_request).await;

        if let Ok(auth_response) = auth_result {
            let auth_response = auth_response.into_inner();

            if auth_response.error.is_none() {
                let transaction_id = extract_transaction_id(&auth_response);

                let refund_request = create_refund_request(&transaction_id);
                let mut refund_grpc_request = Request::new(refund_request);
                add_peachpayments_metadata(&mut refund_grpc_request);

                let refund_response = client
                    .refund(refund_grpc_request)
                    .await
                    .expect("gRPC refund call failed")
                    .into_inner();

                assert!(
                    refund_response.connector_transaction_id.is_some(),
                    "Transaction ID should be present in refund response"
                );
            }
        }
    });
}

#[tokio::test]
async fn test_peachpayments_payment_sync() {
    grpc_test!(client, DirectPaymentServiceClient<Channel>, {
        let auth_request = create_authorize_request(CaptureMethod::Manual);
        let mut auth_grpc_request = Request::new(auth_request);
        add_peachpayments_metadata(&mut auth_grpc_request);

        let auth_result = client.authorize(auth_grpc_request).await;

        if let Ok(auth_response) = auth_result {
            let auth_response = auth_response.into_inner();

            if auth_response.error.is_none() {
                let transaction_id = extract_transaction_id(&auth_response);

                let sync_request = create_payment_sync_request(&transaction_id);
                let mut sync_grpc_request = Request::new(sync_request);
                add_peachpayments_metadata(&mut sync_grpc_request);

                let sync_response = client
                    .get(sync_grpc_request)
                    .await
                    .expect("gRPC payment sync call failed")
                    .into_inner();

                assert!(
                    sync_response.connector_transaction_id.is_some(),
                    "Transaction ID should be present in sync response"
                );
            }
        }
    });
}
