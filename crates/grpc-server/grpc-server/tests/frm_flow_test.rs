#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

//! FRM (Fraud and Risk Management) flow tests.
//!
//! `clearsale_br_session_id_*` replicate the Buser PreRiskCheck request shape:
//! the device-fingerprint `sessionID` must come from the request `metadata`
//! (`sessionId` / snake_case `session_id` — euler's `buildUcsFrmMetadata`
//! encodes via `camelTo2 '_'`), and a `sessionID` smuggled in
//! `connector_feature_data` must be ignored, never a fallback.

use grpc_server::app;
use hyperswitch_masking::Secret;
use ucs_env::configs;
use uuid::Uuid;
mod common;
mod utils;

use std::time::{SystemTime, UNIX_EPOCH};

use grpc_api_types::{
    frm::{
        fraud_and_risk_management_service_client::FraudAndRiskManagementServiceClient,
        FrmServicePreRiskCheckRequest,
    },
    payments::{Currency, Customer, Money, OrderDetailsWithAmount},
};
use tonic::{transport::Channel, Request};

const CONNECTOR_NAME: &str = "clearsale_br";
const MERCHANT_ID: &str = "merchant_1234";
const TEST_AMOUNT: i64 = 10_000; // R$ 100.00
const TEST_EMAIL: &str = "sabujohn@example.com";

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn add_frm_metadata<T>(request: &mut Request<T>) {
    // ClearSale credentials are required for the upstream order call. When the
    // creds file has no entry the call fails at the connector step instead —
    // the tests below only assert the *request-validation* behaviour around
    // `sessionID`, which runs before any credential use.
    if let Ok(connector_config) = utils::credential_utils::connector_config_header(CONNECTOR_NAME) {
        request.metadata_mut().append(
            "x-connector-config",
            connector_config
                .parse()
                .expect("Failed to parse x-connector-config"),
        );
    }
    // FRM connectors are addressed via `x-frm-connector`: `x-connector` is
    // parsed against the payments Connector enum, which has no ClearSale FRM
    // variant ("Matching variant not found").
    request.metadata_mut().append(
        "x-frm-connector",
        CONNECTOR_NAME
            .parse()
            .expect("Failed to parse x-frm-connector"),
    );
    // Legacy BodyKey auth headers: ClearSale's api_key = account username,
    // key1 = account password. Dummy values suffice — validation order is
    // header parse → request validation, so a sessionID error still surfaces.
    request.metadata_mut().append(
        "x-auth",
        "body-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        "dummy-user".parse().expect("Failed to parse x-api-key"),
    );
    request.metadata_mut().append(
        "x-key1",
        "dummy-pass".parse().expect("Failed to parse x-key1"),
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

/// Build the Buser-style request: `metadata.session_id` (snake_case, exactly
/// as euler's `buildUcsFrmMetadata` emits it through `camelTo2 '_'`), the
/// camelCase `sessionId` carried inside `custom_fields` as a nested
/// JSON-string key (as in the original attempt), and a decoy
/// `connector_feature_data.sessionID` that must be ignored.
fn pre_risk_check_request() -> FrmServicePreRiskCheckRequest {
    let metadata = serde_json::json!({
        "merchant_order_id": "order-test-001",
        "user_ip": "10.0.0.1",
        "session_id": "metadata-test-session-01",
        "custom_fields": {
            "{\"sessionId\": \"metadata-test-session-girish-02\"}": "x",
            "origin": "WebCheckout"
        }
    })
    .to_string();

    FrmServicePreRiskCheckRequest {
        amount: Some(Money {
            minor_amount: TEST_AMOUNT,
            currency: i32::from(Currency::Brl),
        }),
        customer_info: Some(Customer {
            email: Some(Secret::new(TEST_EMAIL.to_string())),
            ..Default::default()
        }),
        merchant_transaction_id: Some(format!("test-order-metadata-{}", Uuid::new_v4())),
        metadata: Some(Secret::new(metadata)),
        connector_feature_data: Some(Secret::new(
            r#"{"sessionID": "feature-data-session-SHOULD-BE-IGNORED"}"#.to_string(),
        )),
        order_details: vec![OrderDetailsWithAmount {
            product_name: "Bus ticket".to_string(),
            quantity: 1,
            amount: TEST_AMOUNT,
            ..Default::default()
        }],
        test_mode: Some(true),
        ..Default::default()
    }
}

/// Regression: with `session_id` in the metadata, request validation must not
/// report `sessionID` as missing — whatever fails next (missing creds,
/// billing/shipping, upstream auth) must be a *different* field.
#[tokio::test]
async fn clearsale_br_session_id_from_metadata_is_accepted() {
    grpc_test!(client, FraudAndRiskManagementServiceClient<Channel>, {
        let mut grpc_request = Request::new(pre_risk_check_request());
        add_frm_metadata(&mut grpc_request);

        let result = client.pre_risk_check(grpc_request).await;

        let error_text = match result {
            Ok(response) => {
                // Call went through (valid creds + a reachable/homologation
                // endpoint). The order decision itself is out of scope here.
                format!("{:?}", response.into_inner())
            }
            Err(status) => format!("{status:?}"),
        };

        // `sessionID` must not be among the *missing fields*. (The
        // aggregate error's boilerplate names sessionID generically, so
        // match the quoted field-list marker, not the bare word.)
        assert!(
            !error_text.contains("\\\"sessionID\\\""),
            "sessionID must come from the request metadata, not feature data. Got: {error_text}"
        );
    });
}

/// Counterpart: when the metadata carries no session id AT ALL (even with a
/// decoy `connector_feature_data.sessionID` present), the request must fail
/// validation with `sessionID` listed among the missing fields — proving the
/// feature-data key is ignored rather than used as a fallback.
#[tokio::test]
async fn clearsale_br_feature_data_session_id_is_ignored() {
    grpc_test!(client, FraudAndRiskManagementServiceClient<Channel>, {
        // No top-level session_id in the metadata blob.
        let metadata = serde_json::json!({
            "merchant_order_id": "order-test-001",
            "custom_fields": { "origin": "WebCheckout" }
        })
        .to_string();

        let request = FrmServicePreRiskCheckRequest {
            amount: Some(Money {
                minor_amount: TEST_AMOUNT,
                currency: i32::from(Currency::Brl),
            }),
            customer_info: Some(Customer {
                email: Some(Secret::new(TEST_EMAIL.to_string())),
                ..Default::default()
            }),
            merchant_transaction_id: Some(format!("test-order-metadata-{}", Uuid::new_v4())),
            metadata: Some(Secret::new(metadata)),
            // Decoy — must not be picked up as the session id.
            connector_feature_data: Some(Secret::new(
                r#"{"sessionID": "feature-data-session-SHOULD-BE-IGNORED"}"#.to_string(),
            )),
            order_details: vec![OrderDetailsWithAmount {
                product_name: "Bus ticket".to_string(),
                quantity: 1,
                amount: TEST_AMOUNT,
                ..Default::default()
            }],
            test_mode: Some(true),
            ..Default::default()
        };

        let mut grpc_request = Request::new(request);
        add_frm_metadata(&mut grpc_request);

        let result = client.pre_risk_check(grpc_request).await;

        let Err(status) = result else {
            panic!(
                "expected the call to fail validation without a metadata session id, \
                     but it succeeded — connector_feature_data.sessionID leaked through"
            );
        };
        let error_text = format!("{status:?}");
        assert!(
            error_text.contains("\\\"sessionID\\\""),
            "missing-session error must list sessionID among missing fields. Got: {error_text}"
        );
    });
}

/// The camelCase `sessionId` at the metadata top level is also a valid source
/// (both casings decode through `FrmMetadata`'s serde aliases).
#[tokio::test]
async fn clearsale_br_session_id_camel_case_metadata_is_accepted() {
    grpc_test!(client, FraudAndRiskManagementServiceClient<Channel>, {
        let metadata = serde_json::json!({
            "sessionId": "camelcase-session-99",
            "custom_fields": { "origin": "WebCheckout" }
        })
        .to_string();

        let request = FrmServicePreRiskCheckRequest {
            amount: Some(Money {
                minor_amount: TEST_AMOUNT,
                currency: i32::from(Currency::Brl),
            }),
            customer_info: Some(Customer {
                email: Some(Secret::new(TEST_EMAIL.to_string())),
                ..Default::default()
            }),
            merchant_transaction_id: Some(format!("test-order-metadata-{}", Uuid::new_v4())),
            metadata: Some(Secret::new(metadata)),
            order_details: vec![OrderDetailsWithAmount {
                product_name: "Bus ticket".to_string(),
                quantity: 1,
                amount: TEST_AMOUNT,
                ..Default::default()
            }],
            test_mode: Some(true),
            ..Default::default()
        };

        let mut grpc_request = Request::new(request);
        add_frm_metadata(&mut grpc_request);

        let result = client.pre_risk_check(grpc_request).await;

        let error_text = match result {
            Ok(response) => format!("{:?}", response.into_inner()),
            Err(status) => format!("{status:?}"),
        };

        assert!(
            !error_text.contains("\\\"sessionID\\\""),
            "camelCase metadata sessionId must be picked up. Got: {error_text}"
        );
    });
}
