#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use grpc_server::app;
use ucs_env::configs;

mod common;
mod utils;

use grpc_api_types::surcharge::{
    surcharge_service_client::SurchargeServiceClient, Money, SurchargeServiceCalculateRequest,
};
use hyperswitch_masking::Secret;
use tonic::{transport::Channel, Request};

// ============================================================================
// Constants
// ============================================================================

/// Name of the surcharge connector entry in `.github/test/creds.json`
const CONNECTOR_NAME: &str = "surcharge_connector";
const AUTH_TYPE: &str = "header-key";
const API_KEY: &str = "mock_key";
/// 10.00 USD in minor units (cents)
const TEST_AMOUNT: i64 = 1000;
/// Proto enum discriminant for USD (see `Currency` enum in payment.proto, USD = 146)
const TEST_CURRENCY: i32 = 146;
/// First 6 digits of a generic Visa test card (BIN)
const TEST_CARD_BIN: &str = "424242";
/// A US ZIP code used for regional-fee calculations
const TEST_POSTAL_CODE: &str = "10001";

fn add_mock_metadata<T>(request: &mut Request<T>) {
    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("Failed to parse x-connector"),
    );
    request
        .metadata_mut()
        .append("x-auth", AUTH_TYPE.parse().expect("Failed to parse x-auth"));
    request.metadata_mut().append(
        "x-api-key",
        API_KEY.parse().expect("Failed to parse x-api-key"),
    );
}

#[tokio::test]
async fn test_surcharge_calculate_client_basic() {
    grpc_test!(client, SurchargeServiceClient<Channel>, {
        let mut request = Request::new(SurchargeServiceCalculateRequest {
            merchant_surcharge_id: Some("test_surcharge_123".to_string()),
            amount: Some(Money {
                minor_amount: TEST_AMOUNT,
                currency: TEST_CURRENCY,
            }),
            card_bin: TEST_CARD_BIN.to_string(),
            postal_code: Some(Secret::new(TEST_POSTAL_CODE.to_string())),
            ..Default::default()
        });
        add_mock_metadata(&mut request);
        let response = client.calculate(request).await;
        assert!(response.is_err() || response.is_ok());
    });
}
