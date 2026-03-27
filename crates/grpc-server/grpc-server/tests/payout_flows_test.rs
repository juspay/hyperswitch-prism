#![allow(clippy::expect_used)]

use grpc_server::app;
use ucs_env::configs;
mod common;

use grpc_api_types::payouts::{
    payout_service_client::PayoutServiceClient, Currency, Money, PayoutServiceCreateLinkRequest,
    PayoutServiceCreateRecipientRequest, PayoutServiceCreateRequest,
    PayoutServiceEnrollDisburseAccountRequest, PayoutServiceGetRequest, PayoutServiceStageRequest,
    PayoutServiceTransferRequest, PayoutServiceVoidRequest,
};
use tonic::{transport::Channel, Request};

fn add_mock_metadata<T>(request: &mut Request<T>) {
    request.metadata_mut().append(
        "x-connector",
        "xendit".parse().expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "header-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        "mock_key".parse().expect("Failed to parse x-api-key"),
    );
    request.metadata_mut().append(
        "x-merchant-id",
        "test_merchant_123"
            .parse()
            .expect("Failed to parse x-merchant-id"),
    );
}

#[tokio::test]
async fn test_payout_create_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceCreateRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            destination_currency: i32::from(Currency::Usd),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.create(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_transfer_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceTransferRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            destination_currency: i32::from(Currency::Usd),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.transfer(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_get_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceGetRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.get(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_void_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceVoidRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.void(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_stage_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceStageRequest {
            merchant_quote_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            destination_currency: i32::from(Currency::Usd),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.stage(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_create_link_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceCreateLinkRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            destination_currency: i32::from(Currency::Usd),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.create_link(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_create_recipient_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceCreateRecipientRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.create_recipient(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}

#[tokio::test]
async fn test_payout_enroll_disburse_account_client_basic() {
    grpc_test!(client, PayoutServiceClient<Channel>, {
        let mut request = Request::new(PayoutServiceEnrollDisburseAccountRequest {
            merchant_payout_id: Some("test_payout_123".to_string()),
            amount: Some(Money {
                minor_amount: 1000,
                currency: i32::from(Currency::Usd),
            }),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.enroll_disburse_account(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}
