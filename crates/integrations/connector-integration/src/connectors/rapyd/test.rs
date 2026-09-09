//! Unit tests for the Rapyd incoming-webhook path.
//!
//! Two things here are worth pinning down in tests rather than in review:
//!
//! 1. The HMAC preimage and the double encoding of the signature
//!    (`base64(hex(hmac_sha256(secret_key, preimage)))`). The expected preimage
//!    string and the expected hex digest are asserted verbatim.
//! 2. The `#[serde(untagged)]` discrimination of `WebhookData`. Serde tries the
//!    variants in declaration order and silently falls through, so the variant
//!    each real Rapyd body selects is asserted explicitly.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashMap;

use base64::Engine;
use domain_types::{
    connector_types::{EventType, HttpMethod, RequestDetails},
    payment_method_data::DefaultPCIHolder,
    router_data::ConnectorSpecificConfig,
};
use hyperswitch_masking::Secret;
use interfaces::connector_types::IncomingWebhook;

use super::{
    rapyd_webhook_preimage, rapyd_webhook_url_path, transformers, Rapyd, BASE64_ENGINE_URL_SAFE,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const TEST_ACCESS_KEY: &str = "test_access_key";
const TEST_SECRET_KEY: &str = "test_secret_key";
const TEST_SALT: &str = "salt123456";
const TEST_TIMESTAMP: &str = "1711008868";
const TEST_URL_PATH: &str = "https://webhook.example.com/webhooks/rapyd";
const TEST_BODY: &str = r#"{"id":"wh_test","type":"PAYMENT_COMPLETED"}"#;

/// `hex(HMAC_SHA256(key = TEST_SECRET_KEY, msg = EXPECTED_PREIMAGE))`.
const EXPECTED_HEX_DIGEST: &str =
    "32ea1cfe5a763de62f4950c8e2c2170497ada1fdbcbe49251f12607d27bb91ca";

const EXPECTED_PREIMAGE: &str = concat!(
    "https://webhook.example.com/webhooks/rapyd",
    "salt123456",
    "1711008868",
    "test_access_key",
    "test_secret_key",
    r#"{"id":"wh_test","type":"PAYMENT_COMPLETED"}"#
);

/// `PAYMENT_COMPLETED` — Envelope A, full payment object (trimmed to the fields
/// this integration reads plus enough context to keep the shape realistic).
const PAYMENT_COMPLETED_BODY: &str = r#"{
    "id": "wh_e0afb507504b5eb901449993fadba20f",
    "type": "PAYMENT_COMPLETED",
    "data": {
        "id": "payment_0b645b1ee17a5c3ce79e20ff5524966f",
        "paid": true,
        "amount": 1.0,
        "status": "CLO",
        "captured": true,
        "created_at": 1711008868,
        "error_code": "",
        "is_partial": false,
        "next_action": "not_applicable",
        "country_code": "GB",
        "failure_code": "",
        "currency_code": "EUR",
        "transaction_id": "",
        "failure_message": "",
        "original_amount": 1.0,
        "merchant_reference_id": "order_1234",
        "payment_method": "card_0d83436af31300bf263dd5ef379b8230",
        "payment_method_data": {
            "id": "card_0d83436af31300bf263dd5ef379b8230",
            "last4": "1111",
            "network_reference_id": "MCC1234567890"
        }
    },
    "trigger_operation_id": "77b47127-a209-43ad-9806-90c46e33f950",
    "status": "NEW",
    "created_at": 1711008868
}"#;

/// `PAYMENT_FAILED` — the reduced payment object: no `next_action`, no
/// `transaction_id`. This is the body that used to fail deserialisation.
const PAYMENT_FAILED_BODY: &str = r#"{
    "id": "wh_6d0b1efc39424f33556a300434cccc74",
    "type": "PAYMENT_FAILED",
    "data": {
        "id": "payment_02e5ecc4b7e0395148e8eba68ad63120",
        "mid": "mid_5c5d6187ccd4dd5c16142459e8911350",
        "paid": false,
        "amount": 0.0,
        "status": "ERR",
        "currency_code": "ISK",
        "customer_token": "cus_ad212a1b326bb00dd6a663b44e574ad3",
        "payment_method_type": "gb_mastercard_card",
        "failure_code": "65",
        "failure_message": "Authentication Required or Activity Limit Exceeded",
        "error_code": "ERROR_PROCESSING_CARD - [65]",
        "created_at": 1736262161
    },
    "trigger_operation_id": "9846cf13-6983-4888-88d4-c0b378dec372",
    "status": "NEW",
    "created_at": 1736262296
}"#;

/// `REFUND_COMPLETED` — Envelope B: bare-UUID `id`, empty `status`, `created_at: 0`.
const REFUND_COMPLETED_BODY: &str = r#"{
    "id": "c73cd65e-988c-473a-8d03-06a15d52a3e7",
    "token": "wh_1abe7fd19f96fe553fc613a1f3ff20ca",
    "organization_id": "df301b60-8a9d-4a5a-a315-39b699230cb1",
    "org_pk": 0,
    "type": "REFUND_COMPLETED",
    "data": {
        "id": "refund_102ae6d1286b07ce8b3768b93eab9a24",
        "amount": 87.36,
        "payment": "payment_ccc04715a1021b821025b8d8c0a0eed8",
        "currency": "ILS",
        "failure_reason": "",
        "failure_code": "",
        "reason": "Direct Refund",
        "status": "Completed",
        "receipt_number": 0,
        "created_at": 1768959,
        "updated_at": 1768960,
        "merchant_reference_id": "030016",
        "proportional_refund": true
    },
    "attempts": [],
    "trigger_operation_id": "3567b315-a73c-4415-8206-c598d70ac665",
    "first_attempt_at": 0,
    "last_attempt_at": 0,
    "status": "",
    "created_at": 0,
    "next_attempt_at": 0,
    "uri": ""
}"#;

/// `PAYMENT_DISPUTE_CREATED` — Envelope A, dispute object, `status: ACT`.
const DISPUTE_CREATED_BODY: &str = r#"{
    "id": "wh_7a14ef258b3cd91f04e7d52803a6de22",
    "type": "PAYMENT_DISPUTE_CREATED",
    "data": {
        "id": "c82f4a17-3e5d-41bc-b07a-99d134258ef1",
        "rate": 1,
        "token": "dispute_a93c7f02e1dd48b6f035c78452a1b9d4",
        "amount": 10.0,
        "status": "ACT",
        "currency": "USD",
        "due_date": 1651726898,
        "created_at": 1650949298,
        "updated_at": 1650949298,
        "pre_dispute": false,
        "dispute_category": "Authorization",
        "original_dispute_amount": 10,
        "original_transaction_id": "payment_185b8ef92ef6f3a26ef0ac5fe41e6263",
        "original_dispute_currency": "USD",
        "dispute_reason_description": "Authorization Errors"
    },
    "status": "NEW",
    "created_at": 1650949298,
    "trigger_operation_id": "f1c3b82e-7041-49da-b358-16e4dc902f81"
}"#;

/// `PAYMENT_DISPUTE_UPDATED` with a `{status}` placeholder — the real event has
/// to come from `data.status`, not from the `type` field.
fn dispute_updated_body(status: &str) -> String {
    format!(
        r#"{{
    "id": "wh_b5d92e3f11a874cc0e2f81d94703b648",
    "type": "PAYMENT_DISPUTE_UPDATED",
    "data": {{
        "id": "d74e1b30-92ac-4f77-8c6e-3a05b8d671f2",
        "token": "dispute_c14b8e57f3a02d91e64870b52fc39da1",
        "amount": 10.0,
        "status": "{status}",
        "currency": "USD",
        "due_date": 1651753866,
        "created_at": 1650976266,
        "updated_at": 1650976268,
        "pre_dispute": false,
        "original_transaction_id": "payment_946ad0ce568c8ef2c811d672601eaed3",
        "dispute_reason_description": "Authorization Errors"
    }},
    "status": "NEW",
    "created_at": 1650976268,
    "trigger_operation_id": "e2b47d91-6c83-4f5e-a720-53c91e8b0d37"
}}"#
    )
}

fn connector() -> &'static Rapyd<DefaultPCIHolder> {
    Rapyd::<DefaultPCIHolder>::new()
}

fn test_config() -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Rapyd {
        access_key: Secret::new(TEST_ACCESS_KEY.to_string()),
        secret_key: Secret::new(TEST_SECRET_KEY.to_string()),
        base_url: None,
    }
}

fn request_with_body(body: &str) -> RequestDetails {
    RequestDetails {
        method: HttpMethod::Post,
        uri: None,
        headers: HashMap::new(),
        body: body.as_bytes().to_vec(),
        query_params: None,
    }
}

// ---------------------------------------------------------------------------
// Signature: preimage and digest
// ---------------------------------------------------------------------------

#[test]
fn preimage_is_url_path_salt_timestamp_access_key_secret_key_body() {
    let preimage = rapyd_webhook_preimage(
        TEST_URL_PATH,
        TEST_SALT,
        TEST_TIMESTAMP,
        TEST_ACCESS_KEY,
        TEST_SECRET_KEY,
        TEST_BODY,
    );

    assert_eq!(preimage, EXPECTED_PREIMAGE);
    assert_eq!(
        preimage,
        "https://webhook.example.com/webhooks/rapydsalt1234561711008868test_access_keytest_secret_key{\"id\":\"wh_test\",\"type\":\"PAYMENT_COMPLETED\"}"
    );
}

#[test]
fn hmac_sha256_of_the_preimage_hex_encodes_to_the_expected_digest() {
    use common_utils::crypto::SignMessage;

    let digest = common_utils::crypto::HmacSha256
        .sign_message(TEST_SECRET_KEY.as_bytes(), EXPECTED_PREIMAGE.as_bytes())
        .expect("hmac signing must succeed");

    assert_eq!(hex::encode(digest), EXPECTED_HEX_DIGEST);
}

#[test]
fn verify_webhook_source_accepts_a_correctly_signed_webhook() {
    // Rapyd hex-encodes the digest first, then base64-encodes those 64 ASCII
    // characters (URL-safe alphabet) into the `signature` header.
    let signature_header = BASE64_ENGINE_URL_SAFE.encode(EXPECTED_HEX_DIGEST.as_bytes());

    let mut headers = HashMap::new();
    headers.insert("signature".to_string(), signature_header);
    headers.insert("salt".to_string(), TEST_SALT.to_string());
    headers.insert("timestamp".to_string(), TEST_TIMESTAMP.to_string());
    headers.insert("host".to_string(), "webhook.example.com".to_string());

    let request = RequestDetails {
        method: HttpMethod::Post,
        uri: Some(TEST_URL_PATH.to_string()),
        headers,
        body: TEST_BODY.as_bytes().to_vec(),
        query_params: None,
    };

    let verified = connector()
        .verify_webhook_source(request, None, Some(test_config()))
        .expect("verification must not error");

    assert!(verified, "correctly signed webhook must verify");
}

#[test]
fn verify_webhook_source_rejects_a_tampered_body() {
    let signature_header = BASE64_ENGINE_URL_SAFE.encode(EXPECTED_HEX_DIGEST.as_bytes());

    let mut headers = HashMap::new();
    headers.insert("signature".to_string(), signature_header);
    headers.insert("salt".to_string(), TEST_SALT.to_string());
    headers.insert("timestamp".to_string(), TEST_TIMESTAMP.to_string());
    headers.insert("host".to_string(), "webhook.example.com".to_string());

    let request = RequestDetails {
        method: HttpMethod::Post,
        uri: Some(TEST_URL_PATH.to_string()),
        headers,
        body: br#"{"id":"wh_test","type":"PAYMENT_FAILED"}"#.to_vec(),
        query_params: None,
    };

    let verified = connector()
        .verify_webhook_source(request, None, Some(test_config()))
        .expect("verification must not error");

    assert!(!verified, "a tampered body must not verify");
}

#[test]
fn url_path_is_taken_verbatim_when_the_uri_is_absolute() {
    let mut headers = HashMap::new();
    headers.insert("host".to_string(), "ignored.example.com".to_string());

    let request = RequestDetails {
        method: HttpMethod::Post,
        uri: Some("https://webhook.example.com/webhooks/rapyd?x=1".to_string()),
        headers,
        body: Vec::new(),
        query_params: None,
    };

    assert_eq!(
        rapyd_webhook_url_path(&request).expect("path must resolve"),
        TEST_URL_PATH
    );
}

#[test]
fn url_path_is_composed_from_the_host_header_when_the_uri_is_a_bare_path() {
    let mut headers = HashMap::new();
    headers.insert("Host".to_string(), "webhook.example.com".to_string());

    let request = RequestDetails {
        method: HttpMethod::Post,
        uri: Some("/webhooks/rapyd?x=1".to_string()),
        headers,
        body: Vec::new(),
        query_params: None,
    };

    assert_eq!(
        rapyd_webhook_url_path(&request).expect("path must resolve"),
        TEST_URL_PATH
    );
}

// ---------------------------------------------------------------------------
// `#[serde(untagged)]` discrimination — the most fragile part of this change
// ---------------------------------------------------------------------------

fn parse(body: &str) -> transformers::RapydIncomingWebhook {
    serde_json::from_str(body).expect("webhook body must deserialise")
}

#[test]
fn payment_body_selects_the_payment_variant() {
    let webhook = parse(PAYMENT_COMPLETED_BODY);
    match webhook.data {
        transformers::WebhookData::Payment(data) => {
            assert_eq!(data.id, "payment_0b645b1ee17a5c3ce79e20ff5524966f");
            assert_eq!(data.status, RapydPaymentStatus::Closed);
            assert_eq!(data.next_action, Some(NextAction::NotApplicable));
        }
        other => panic!("expected WebhookData::Payment, got {other:?}"),
    }
}

#[test]
fn reduced_payment_failed_body_still_selects_the_payment_variant() {
    // No `next_action`, no `transaction_id` — the shape that used to fail.
    let webhook = parse(PAYMENT_FAILED_BODY);
    match webhook.data {
        transformers::WebhookData::Payment(data) => {
            assert_eq!(data.id, "payment_02e5ecc4b7e0395148e8eba68ad63120");
            assert_eq!(data.status, RapydPaymentStatus::Error);
            assert_eq!(data.next_action, None);
            assert_eq!(data.transaction_id, None);
            assert_eq!(
                transformers::get_status_for_webhook(&data),
                AttemptStatus::Failure
            );
        }
        other => panic!("expected WebhookData::Payment, got {other:?}"),
    }
}

#[test]
fn refund_body_selects_the_refund_variant() {
    let webhook = parse(REFUND_COMPLETED_BODY);
    match webhook.data {
        transformers::WebhookData::Refund(data) => {
            assert_eq!(data.id, "refund_102ae6d1286b07ce8b3768b93eab9a24");
            assert_eq!(data.payment, "payment_ccc04715a1021b821025b8d8c0a0eed8");
            assert_eq!(data.status, transformers::RefundStatus::Completed);
            assert_eq!(data.currency, common_enums::Currency::ILS);
        }
        other => panic!("expected WebhookData::Refund, got {other:?}"),
    }
}

#[test]
fn dispute_body_selects_the_dispute_variant() {
    let webhook = parse(DISPUTE_CREATED_BODY);
    match webhook.data {
        transformers::WebhookData::Dispute(data) => {
            assert_eq!(data.token, "dispute_a93c7f02e1dd48b6f035c78452a1b9d4");
            assert_eq!(
                data.original_transaction_id,
                "payment_185b8ef92ef6f3a26ef0ac5fe41e6263"
            );
            assert_eq!(data.status, transformers::RapydWebhookDisputeStatus::Active);
            assert_eq!(data.currency, common_enums::Currency::USD);
        }
        other => panic!("expected WebhookData::Dispute, got {other:?}"),
    }
}

#[test]
fn sample_webhook_body_round_trips_into_the_payment_variant() {
    let body = connector().sample_webhook_body();
    let webhook: transformers::RapydIncomingWebhook =
        serde_json::from_slice(body).expect("sample body must deserialise");
    assert!(matches!(
        webhook.data,
        transformers::WebhookData::Payment(_)
    ));
}

// ---------------------------------------------------------------------------
// Event-type mapping
// ---------------------------------------------------------------------------

#[test]
fn event_types_map_to_the_expected_ucs_events() {
    let cases = [
        (PAYMENT_COMPLETED_BODY, EventType::PaymentIntentSuccess),
        (PAYMENT_FAILED_BODY, EventType::PaymentIntentFailure),
        (REFUND_COMPLETED_BODY, EventType::RefundSuccess),
        (DISPUTE_CREATED_BODY, EventType::DisputeOpened),
    ];

    for (body, expected) in cases {
        let event = connector()
            .get_event_type(request_with_body(body))
            .expect("event type must resolve");
        assert_eq!(event, expected, "body: {body}");
    }
}

#[test]
fn dispute_updated_takes_its_event_type_from_data_status() {
    let cases = [
        ("ACT", EventType::DisputeOpened),
        ("RVW", EventType::DisputeChallenged),
        ("LOS", EventType::DisputeLost),
        ("WIN", EventType::DisputeWon),
        // PRA / ARB / REV have no faithful UCS counterpart.
        ("PRA", EventType::IncomingWebhookEventUnspecified),
    ];

    for (status, expected) in cases {
        let body = dispute_updated_body(status);
        let event = connector()
            .get_event_type(request_with_body(&body))
            .expect("event type must resolve");
        assert_eq!(event, expected, "dispute status: {status}");
    }
}

#[test]
fn an_unhandled_event_type_is_unspecified() {
    let body = PAYMENT_COMPLETED_BODY.replace("PAYMENT_COMPLETED", "PAYMENT_SUCCEEDED");
    let event = connector()
        .get_event_type(request_with_body(&body))
        .expect("event type must resolve");
    assert_eq!(event, EventType::IncomingWebhookEventUnspecified);
}

// ---------------------------------------------------------------------------
// HandleEvent responses
// ---------------------------------------------------------------------------

#[test]
fn process_payment_webhook_maps_status_and_identifiers() {
    let response = connector()
        .process_payment_webhook(request_with_body(PAYMENT_COMPLETED_BODY), None, None, None)
        .expect("payment webhook must process");

    assert_eq!(response.status, AttemptStatus::Charged);
    assert_eq!(
        response.connector_response_reference_id.as_deref(),
        Some("order_1234")
    );
    assert_eq!(response.network_txn_id.as_deref(), Some("MCC1234567890"));
    assert_eq!(response.error_code, None);
}

#[test]
fn process_refund_webhook_maps_status_and_parent_payment() {
    let response = connector()
        .process_refund_webhook(request_with_body(REFUND_COMPLETED_BODY), None, None)
        .expect("refund webhook must process");

    assert_eq!(response.status, common_enums::RefundStatus::Success);
    assert_eq!(
        response.connector_refund_id.as_deref(),
        Some("refund_102ae6d1286b07ce8b3768b93eab9a24")
    );
    assert_eq!(
        response.connector_response_reference_id.as_deref(),
        Some("payment_ccc04715a1021b821025b8d8c0a0eed8")
    );
}

#[test]
fn process_dispute_webhook_converts_major_units_to_minor() {
    let response = connector()
        .process_dispute_webhook(request_with_body(DISPUTE_CREATED_BODY), None, None)
        .expect("dispute webhook must process");

    // `"amount": 10` with `"currency": "USD"` is ten dollars, i.e. 1000 minor units.
    assert_eq!(
        serde_json::to_value(&response.amount).expect("amount must serialise"),
        serde_json::json!("1000")
    );
    assert_eq!(response.currency, common_enums::Currency::USD);
    assert_eq!(
        response.dispute_id,
        "dispute_a93c7f02e1dd48b6f035c78452a1b9d4"
    );
    assert_eq!(response.status, common_enums::DisputeStatus::DisputeOpened);
    assert_eq!(response.stage, common_enums::DisputeStage::Dispute);
    assert_eq!(
        response.connector_response_reference_id.as_deref(),
        Some("payment_185b8ef92ef6f3a26ef0ac5fe41e6263")
    );
}

#[test]
fn process_dispute_webhook_errors_on_an_unmapped_status() {
    let body = dispute_updated_body("ARB");
    let result = connector().process_dispute_webhook(request_with_body(&body), None, None);
    assert!(
        result.is_err(),
        "an unmapped dispute status must not be guessed at"
    );
}

// ---------------------------------------------------------------------------
// ParseEvent references
// ---------------------------------------------------------------------------

#[test]
fn event_references_use_the_right_id_slots() {
    use domain_types::connector_types::WebhookResourceReference;

    let payment_reference = connector()
        .get_webhook_event_reference(request_with_body(PAYMENT_COMPLETED_BODY))
        .expect("reference must resolve")
        .expect("reference must be present");
    match payment_reference {
        WebhookResourceReference::Payment(reference) => {
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("payment_0b645b1ee17a5c3ce79e20ff5524966f")
            );
            assert_eq!(
                reference.merchant_transaction_id.as_deref(),
                Some("order_1234")
            );
        }
        other => panic!("expected a payment reference, got {other:?}"),
    }

    let refund_reference = connector()
        .get_webhook_event_reference(request_with_body(REFUND_COMPLETED_BODY))
        .expect("reference must resolve")
        .expect("reference must be present");
    match refund_reference {
        WebhookResourceReference::Refund(reference) => {
            assert_eq!(
                reference.connector_refund_id.as_deref(),
                Some("refund_102ae6d1286b07ce8b3768b93eab9a24")
            );
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("payment_ccc04715a1021b821025b8d8c0a0eed8")
            );
        }
        other => panic!("expected a refund reference, got {other:?}"),
    }

    let dispute_reference = connector()
        .get_webhook_event_reference(request_with_body(DISPUTE_CREATED_BODY))
        .expect("reference must resolve")
        .expect("reference must be present");
    match dispute_reference {
        WebhookResourceReference::Dispute(reference) => {
            assert_eq!(
                reference.connector_dispute_id.as_deref(),
                Some("dispute_a93c7f02e1dd48b6f035c78452a1b9d4")
            );
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("payment_185b8ef92ef6f3a26ef0ac5fe41e6263")
            );
        }
        other => panic!("expected a dispute reference, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 3DS on Authorize
//
// Rapyd has no standalone 3DS endpoint — both 3DS modes ride on
// `POST /v1/payments`, so everything worth pinning down here is transformer
// behaviour rather than a new flow:
//
// 1. `next_action` deserialization. It sits on `ResponseData`, which backs
//    Authorize, PSync, Capture, Void, SetupMandate, RepeatPayment *and* every
//    incoming payment webhook, so one unparsable value used to fail all of
//    them at once.
// 2. The external-3DS (Mode B) request mapping — which of our
//    `AuthenticationData` fields reach `payment_method_options`, which are
//    dropped, and which inputs are refused outright.
// 3. The silent-hang hole: `3d_verification` with no `redirect_url`.
// ---------------------------------------------------------------------------

use common_enums::{AttemptStatus, CardNetwork, ExemptionIndicator, TransactionStatus};
use common_utils::{request::Method, types::SemanticVersion};
use domain_types::{
    errors::ConnectorError,
    router_request_types::{AuthenticationData, BrowserInformation},
    router_response_types::RedirectForm,
};
use std::str::FromStr;
use transformers::{
    build_card_payment_method_options, build_connector_metadata, build_redirection_data,
    get_status, get_status_for_payment_response, NextAction, PaymentMethodOptions, RapydAuthResult,
    RapydClientDetails, RapydEciRequest, RapydPaymentStatus, RapydScaExemption,
    RapydThreeDsVersion, ResponseData,
};

/// A Mode-A (Rapyd-hosted) 3DS-pending payment object, as Rapyd's own sandbox
/// documentation prints it.
const THREE_DS_PENDING_BODY: &str = r#"{
    "id": "payment_3ds_pending",
    "amount": 0,
    "original_amount": 1050,
    "currency_code": "USD",
    "status": "ACT",
    "next_action": "3d_verification",
    "redirect_url": "https://sandboxcheckout.rapyd.net/3ds-payment?token=payment_3ds_pending",
    "payment_method": null,
    "authentication_result": {
        "eci": null,
        "result": "R",
        "version": "2.2.0",
        "cardholder_info": null
    },
    "paid": false,
    "captured": true
}"#;

fn authentication_data() -> AuthenticationData {
    AuthenticationData {
        trans_status: Some(TransactionStatus::Success),
        eci: Some("05".to_string()),
        cavv: Some(Secret::new("AAABBJg0VhI0VniQEjRWAAAAAAA=".to_string())),
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: Some("threeds-server-txn".to_string()),
        message_version: Some(SemanticVersion::from_str("2.2.0").expect("valid version")),
        ds_trans_id: Some("f38e6948-5388-41a6-bca4-b49723c19437".to_string()),
        acs_transaction_id: Some("acs-txn".to_string()),
        transaction_id: Some("txn".to_string()),
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }
}

fn options_json(options: &PaymentMethodOptions) -> serde_json::Value {
    serde_json::to_value(options).expect("payment_method_options must serialize")
}

// ---------------------------------------------------------------------------
// 1. next_action deserialization
// ---------------------------------------------------------------------------

/// The defect this change fixes: `pending_offline_capture` is a documented
/// Rapyd value that used to make the WHOLE payment object fail to deserialize,
/// turning a valid 200 into a parse error on Authorize, PSync and webhooks.
#[test]
fn next_action_pending_offline_capture_deserializes_and_is_authorized() {
    let body = THREE_DS_PENDING_BODY.replace("3d_verification", "pending_offline_capture");
    let data: ResponseData =
        serde_json::from_str(&body).expect("pending_offline_capture must deserialize");

    assert_eq!(data.next_action, Some(NextAction::PendingOfflineCapture));
    // Same semantics as `pending_capture`: it waits on us, not on Rapyd.
    assert_eq!(
        get_status(
            RapydPaymentStatus::Active,
            NextAction::PendingOfflineCapture
        ),
        AttemptStatus::Authorized
    );
}

/// A `next_action` Rapyd adds in the future must parse to `Unknown` rather than
/// fail the body — and must NOT be folded into `not_applicable`, which reads as
/// `Authorized`.
#[test]
fn next_action_unknown_value_parses_and_stays_non_terminal() {
    let body = THREE_DS_PENDING_BODY.replace("3d_verification", "some_future_rapyd_action");
    let data: ResponseData =
        serde_json::from_str(&body).expect("an unknown next_action must not fail the body");

    assert_eq!(data.next_action, Some(NextAction::Unknown));
    assert_eq!(
        get_status(RapydPaymentStatus::Active, NextAction::Unknown),
        AttemptStatus::Pending
    );
}

/// `ACT` + `3d_verification` is precisely "challenge outstanding". Rapyd's own
/// `ERROR_CAPTURE_PAYMENT_3DS_INCOMPLETE` confirms Capture cannot advance it, so
/// it must not read as `Authorized`.
#[test]
fn three_ds_pending_maps_to_authentication_pending() {
    let data: ResponseData =
        serde_json::from_str(THREE_DS_PENDING_BODY).expect("3DS-pending body must deserialize");

    assert_eq!(
        get_status(
            data.status.clone(),
            data.next_action.clone().expect("next_action present")
        ),
        AttemptStatus::AuthenticationPending
    );
}

/// An issuer can report `result: "N"` under a liability shift while Rapyd still
/// closes the payment as paid. `authentication_result` is diagnostic only and
/// must never downgrade a `CLO`.
#[test]
fn authentication_result_n_does_not_downgrade_a_closed_payment() {
    let body = THREE_DS_PENDING_BODY
        .replace(r#""status": "ACT""#, r#""status": "CLO""#)
        .replace(r#""result": "R""#, r#""result": "N""#)
        .replace(r#""paid": false"#, r#""paid": true"#);
    let data: ResponseData = serde_json::from_str(&body).expect("closed body must deserialize");

    assert_eq!(
        data.authentication_result
            .as_deref()
            .and_then(|result| result.result),
        Some(RapydAuthResult::NotAuthenticated)
    );
    assert_eq!(
        get_status(
            data.status.clone(),
            data.next_action.clone().expect("next_action present")
        ),
        AttemptStatus::Charged
    );
    // …and it is still surfaced, as metadata, for liability-shift reporting.
    let metadata = build_connector_metadata(&data).expect("metadata must be present");
    assert_eq!(metadata["authentication_result"]["result"], "N");
}

/// An unrecognised `authentication_result.result` must parse to `Unknown`
/// instead of being guessed into `A`/`N`.
#[test]
fn authentication_result_unknown_code_parses() {
    let body = THREE_DS_PENDING_BODY.replace(r#""result": "R""#, r#""result": "Z""#);
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    assert_eq!(
        data.authentication_result
            .as_deref()
            .and_then(|result| result.result),
        Some(RapydAuthResult::Unknown)
    );
}

/// Rapyd's docs type the echoed `3d_required` as boolean on one page and
/// `string|boolean` on another. Either encoding must parse; neither may fail the
/// whole payment object.
#[test]
fn echoed_three_ds_required_accepts_bool_or_string() {
    for (encoded, expected) in [("true", Some(true)), (r#""true""#, Some(true))] {
        let body = format!(
            r#"{{"id":"p","amount":250,"status":"ACT","next_action":"pending_capture",
                 "payment_method_options":{{"3d_required":{encoded},"eci":"05",
                 "3d_version":"2.2.0","ds_trans_id":"ds-1"}}}}"#
        );
        let data: ResponseData = serde_json::from_str(&body).expect("echo must deserialize");
        let echo = data
            .payment_method_options
            .as_ref()
            .expect("echo must be present");
        assert_eq!(echo.three_ds, expected);
        assert_eq!(echo.eci.as_deref(), Some("05"));
        assert_eq!(echo.three_ds_version.as_deref(), Some("2.2.0"));
    }
}

// ---------------------------------------------------------------------------
// 2. External 3DS (Mode B) request mapping
// ---------------------------------------------------------------------------

/// The four fields Rapyd accepts — and nothing else. Notably `3d_required` goes
/// out as `false` (we already authenticated, Rapyd must not challenge again),
/// and `ds_trans_id` is NOT substituted into `xid`: Rapyd has both slots, so the
/// Datatrans-style substitution would be wrong here.
#[test]
fn external_three_ds_maps_only_the_four_supported_fields() {
    let auth = authentication_data();
    let options = build_card_payment_method_options(Some(&auth), true, Some(&CardNetwork::Visa))
        .expect("external 3DS mapping must succeed");
    let json = options_json(&options);

    assert_eq!(json["3d_required"], serde_json::json!(false));
    assert_eq!(json["cavv"], "AAABBJg0VhI0VniQEjRWAAAAAAA=");
    assert_eq!(json["eci"], "05");
    assert_eq!(json["3d_version"], "2.2.0");
    assert_eq!(json["ds_trans_id"], "f38e6948-5388-41a6-bca4-b49723c19437");

    // Nothing else may appear: `xid` and `tavv` have no source field, and every
    // remaining `AuthenticationData` member has no Rapyd target at all.
    for absent in [
        "xid",
        "tavv",
        "cvv",
        "sca_exemption",
        "trans_status",
        "threeds_server_transaction_id",
        "acs_transaction_id",
        "transaction_id",
        "ucaf_collection_indicator",
    ] {
        assert!(
            json.get(absent).is_none(),
            "payment_method_options must not carry `{absent}`"
        );
    }
    assert_eq!(
        options.three_ds_version,
        Some(RapydThreeDsVersion::V2_2_0),
        "the version must go through the closed enum, not a raw string"
    );
    assert_eq!(options.eci, Some(RapydEciRequest::E05));
}

/// Mode B takes precedence over `auth_type`, and Mode A / no-3DS bodies must be
/// byte-for-byte what this connector sent before external 3DS existed.
#[test]
fn rapyd_hosted_and_no_three_ds_bodies_are_unchanged() {
    let hosted = options_json(
        &build_card_payment_method_options(None, true, Some(&CardNetwork::Visa))
            .expect("mode A must build"),
    );
    assert_eq!(hosted, serde_json::json!({ "3d_required": true }));

    let plain = options_json(
        &build_card_payment_method_options(None, false, Some(&CardNetwork::Visa))
            .expect("mode N must build"),
    );
    assert_eq!(plain, serde_json::json!({ "3d_required": false }));

    assert_eq!(
        options_json(&PaymentMethodOptions::rapyd_hosted(true)),
        serde_json::json!({ "3d_required": true })
    );
}

/// A pass-through with no cryptogram is not an external 3DS at all.
#[test]
fn external_three_ds_without_cavv_is_rejected() {
    let auth = AuthenticationData {
        cavv: None,
        ..authentication_data()
    };
    let error = build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
        .expect_err("a missing CAVV must not produce a silently non-authenticated payment");
    assert!(
        format!("{error:?}").contains("authentication_data.cavv"),
        "error must name the missing field, got: {error:?}"
    );
}

/// Rapyd accepts exactly `1.0.2`, `2.1.0` and `2.2.0`. Anything else is a clean
/// local error, never a malformed body Rapyd rejects at the network.
#[test]
fn external_three_ds_rejects_an_unsupported_protocol_version() {
    let auth = AuthenticationData {
        message_version: Some(SemanticVersion::from_str("2.3.0").expect("valid semver")),
        ..authentication_data()
    };
    let error = build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
        .expect_err("2.3.0 is outside Rapyd's accepted set");
    assert!(
        format!("{error:?}").contains("2.3.0"),
        "error must name the offending version, got: {error:?}"
    );
}

/// The request-side ECI regex is `(01|02|05|06|07|08)`; a value outside it must
/// not be stringified into the body.
#[test]
fn external_three_ds_rejects_an_out_of_set_eci() {
    let auth = AuthenticationData {
        eci: Some("99".to_string()),
        ..authentication_data()
    };
    let _ = build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
        .expect_err("ECI 99 is outside Rapyd's accepted set");
}

/// "ds_trans_id — required for Mastercard 2.0".
#[test]
fn external_three_ds_requires_ds_trans_id_for_mastercard_two_x() {
    let auth = AuthenticationData {
        ds_trans_id: None,
        ..authentication_data()
    };
    let error =
        build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Mastercard))
            .expect_err("Mastercard 2.x needs the directory-server transaction id");
    assert!(
        format!("{error:?}").contains("ds_trans_id"),
        "error must name the missing field, got: {error:?}"
    );

    // Visa has no such requirement.
    build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
        .expect("Visa 2.x without ds_trans_id is fine");
}

/// `trans_status` has no Rapyd target field, so it is used purely as a local
/// guard: a cryptogram from an authentication that did not succeed carries no
/// liability shift and must not be sent.
#[test]
fn external_three_ds_guards_on_trans_status() {
    for allowed in [TransactionStatus::Success, TransactionStatus::NotVerified] {
        let auth = AuthenticationData {
            trans_status: Some(allowed),
            ..authentication_data()
        };
        build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
            .expect("Y and A both carry a usable liability shift");
    }

    for refused in [
        TransactionStatus::Failure,
        TransactionStatus::Rejected,
        TransactionStatus::ChallengeRequired,
        TransactionStatus::VerificationNotPerformed,
    ] {
        let auth = AuthenticationData {
            trans_status: Some(refused),
            ..authentication_data()
        };
        let _ = build_card_payment_method_options(Some(&auth), false, Some(&CardNetwork::Visa))
            .expect_err("a non-authenticated 3DS result must not be passed through");
    }
}

/// Our `ExemptionIndicator` is wider than Rapyd's four values. Where there is no
/// equivalent the field is omitted entirely — never approximated, never sent as
/// an empty string.
#[test]
fn sca_exemption_is_mapped_or_omitted_but_never_approximated() {
    let mapped = [
        (ExemptionIndicator::LowValue, RapydScaExemption::LowValue),
        (
            ExemptionIndicator::TransactionRiskAssessment,
            RapydScaExemption::TransactionRiskAnalysis,
        ),
        (
            ExemptionIndicator::ThreeDsOutage,
            RapydScaExemption::AuthenticationOutage,
        ),
        (
            ExemptionIndicator::SecureCorporatePayment,
            RapydScaExemption::SecureCorporatePayments,
        ),
    ];
    for (ours, theirs) in mapped {
        let auth = AuthenticationData {
            exemption_indicator: Some(ours),
            ..authentication_data()
        };
        let options = build_card_payment_method_options(Some(&auth), false, None)
            .expect("mapping must succeed");
        assert_eq!(options.sca_exemption, Some(theirs));
    }

    for unmappable in [
        ExemptionIndicator::TrustedListing,
        ExemptionIndicator::ScaDelegation,
        ExemptionIndicator::OutOfScaScope,
        ExemptionIndicator::LowRiskProgram,
        ExemptionIndicator::RecurringOperation,
        ExemptionIndicator::Other,
    ] {
        let auth = AuthenticationData {
            exemption_indicator: Some(unmappable),
            ..authentication_data()
        };
        let options = build_card_payment_method_options(Some(&auth), false, None)
            .expect("mapping must succeed");
        assert_eq!(options.sca_exemption, None);
        assert!(options_json(&options).get("sca_exemption").is_none());
    }
}

/// `client_details` is best-effort: whatever the browser reported is forwarded,
/// an out-of-set colour depth is dropped rather than rejected, and an empty
/// browser payload produces no object at all (rather than `{}`).
#[test]
fn client_details_are_best_effort() {
    let browser = BrowserInformation {
        screen_height: Some(1080),
        screen_width: Some(1920),
        color_depth: Some(31), // not in Rapyd's set {1,4,8,15,16,24,32,48}
        accept_header: Some("text/html".to_string()),
        language: Some("en-US".to_string()),
        time_zone: Some(-330),
        ..Default::default()
    };
    let details = RapydClientDetails::from_browser_info(&browser).expect("must be populated");
    let json = serde_json::to_value(&details).expect("client_details must serialize");
    assert_eq!(json["screen_height"], 1080);
    assert_eq!(json["screen_width"], 1920);
    assert_eq!(json["accept_header"], "text/html");
    // Passed through unchanged — Rapyd does not state a sign convention.
    assert_eq!(json["time_zone_offset"], -330);
    assert!(json.get("screen_color_depth").is_none());
    assert!(json.get("ip_address").is_none());

    assert!(RapydClientDetails::from_browser_info(&BrowserInformation::default()).is_none());
}

// ---------------------------------------------------------------------------
// 3. The silent-hang hole
// ---------------------------------------------------------------------------

/// `ACT` + `3d_verification` + no `redirect_url` used to produce
/// `AuthenticationPending` with `redirection_data: None` — an attempt the
/// cardholder can never authenticate, which then polls until Rapyd's 15-minute
/// window expires. It must fail loudly instead.
#[test]
fn three_ds_verification_without_a_redirect_url_fails_loudly() {
    for absent in [r#""redirect_url": null"#, r#""redirect_url": """#] {
        let body = THREE_DS_PENDING_BODY.replace(
            r#""redirect_url": "https://sandboxcheckout.rapyd.net/3ds-payment?token=payment_3ds_pending""#,
            absent,
        );
        let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
        let error = build_redirection_data(&data, 200)
            .expect_err("a 3DS challenge with nowhere to go must not be reported as pending");
        match error.current_context() {
            ConnectorError::UnexpectedResponseError { context } => {
                let detail = context
                    .additional_context
                    .as_deref()
                    .expect("the failure must say why");
                assert!(
                    detail.contains("3d_verification") && detail.contains("redirect_url"),
                    "error must explain the cause, got: {detail}"
                );
            }
            other => panic!("expected an unexpected-response error, got {other:?}"),
        }
    }
}

/// The happy path: a top-level `redirect_url` becomes a GET redirect form.
#[test]
fn three_ds_redirect_url_is_read_from_the_top_level_of_the_payment_object() {
    let data: ResponseData =
        serde_json::from_str(THREE_DS_PENDING_BODY).expect("body must deserialize");
    let redirect = build_redirection_data(&data, 200)
        .expect("a well-formed 3DS response must build a redirect")
        .expect("redirect must be present");
    match redirect {
        RedirectForm::Form {
            endpoint,
            method,
            form_fields,
        } => {
            assert_eq!(endpoint, "https://sandboxcheckout.rapyd.net/3ds-payment");
            // An HTTP GET to a Rapyd-hosted page: no form POST, and the query
            // string Rapyd handed back is preserved as the redirect's fields.
            assert_eq!(method, Method::Get);
            assert_eq!(
                form_fields.get("token").map(String::as_str),
                Some("payment_3ds_pending")
            );
        }
        other => panic!("expected a form redirect, got {other:?}"),
    }
}

/// The override caveat: Rapyd may return a challenge even when we asked for
/// `3d_required: false` (PSD2/SCA, issuer discretion). Redirect handling is
/// therefore mode-agnostic — it never consults what the request asked for.
#[test]
fn redirect_is_honoured_even_when_no_three_ds_was_requested() {
    let body = THREE_DS_PENDING_BODY.replace(
        r#""authentication_result": {"#,
        r#""payment_method_options": {"3d_required": false},
           "authentication_result": {"#,
    );
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    assert_eq!(
        data.payment_method_options
            .as_ref()
            .and_then(|options| options.three_ds),
        Some(false)
    );
    assert!(build_redirection_data(&data, 200)
        .expect("redirect must still be built")
        .is_some());
}

/// `next_action` is optional only because Rapyd's reduced `PAYMENT_FAILED`
/// webhook payload omits it. On the payment API path, defaulting it on a live
/// (`ACT`) payment would report a 3DS-pending attempt as `Authorized`; a
/// terminal payment does not consult it at all and must keep working.
#[test]
fn missing_next_action_errors_on_an_active_payment_but_not_on_a_closed_one() {
    let active = THREE_DS_PENDING_BODY.replace(r#""next_action": "3d_verification","#, "");
    let data: ResponseData = serde_json::from_str(&active).expect("body must deserialize");
    assert!(data.next_action.is_none());
    let _ = get_status_for_payment_response(&data, 200)
        .expect_err("an ACT payment with no next_action is undeterminable");

    let closed = active.replace(r#""status": "ACT""#, r#""status": "CLO""#);
    let data: ResponseData = serde_json::from_str(&closed).expect("body must deserialize");
    assert_eq!(
        get_status_for_payment_response(&data, 200).expect("a closed payment must still resolve"),
        AttemptStatus::Charged
    );

    // The webhook helper keeps the permissive default — `PAYMENT_FAILED` bodies
    // legitimately omit `next_action`.
    assert_eq!(
        transformers::get_status_for_webhook(&data),
        AttemptStatus::Charged
    );
}

// ---------------------------------------------------------------------------
// Response-side error, AVS/CVV and GSM mapping
//
// Rapyd's three sandbox decline cards all return the *status-only* envelope, so
// live testing exercises only the fallback extraction path and can never
// produce a populated `merchant_advice_code`, a `cvv_check: "fail"` or any AVS
// result at all. Everything below is therefore pinned against payloads copied
// verbatim out of Rapyd's own published examples (saved as
// `grace/rulesbook/codegen/references/rapyd/source_err_*.md`), which is the only
// way to cover those paths honestly.
// ---------------------------------------------------------------------------

use domain_types::router_data::FlowStatus;
use transformers::{
    classify_rapyd_error, network_code_from_qualified, rapyd_error_response,
    rapyd_network_error_fields, rapyd_short_message, RapydCheckResult, RapydErrorClass,
    RapydPaymentsResponse,
};

/// `POST /v1/payments` error, verbatim from
/// <https://docs.rapyd.net/en/create-payment-error-examples.html> (second
/// example) — the shape that carries a full `data` payment object beside the
/// envelope. Trimmed to the fields this connector reads; every value is
/// unchanged.
const CREATE_PAYMENT_ERROR_65: &str = r#"{
  "status": {
    "error_code": "ERROR_PROCESSING_CARD - [65]",
    "status": "ERROR",
    "message": "[Authentication Required or Activity Limit Exceeded] The request attempted a card operation, but 3DS was not completed or the transaction would exceed the card's activity limit. The request was rejected. Corrective action: Resubmit with 3DS required or use another payment method.",
    "response_code": "ERROR_PROCESSING_CARD - [65]",
    "operation_id": "b853d6ab-a944-4878-a8fb-09b20798d97a"
  },
  "data": {
    "id": "payment_3756efe04a548f33d863b9ed2ab47e4a",
    "amount": 1065,
    "original_amount": 1065,
    "is_partial": false,
    "currency_code": "ISK",
    "country_code": "IS",
    "status": "ERR",
    "merchant_reference_id": "",
    "payment_method": null,
    "payment_method_data": {
      "type": "is_visa_card",
      "category": "card",
      "next_action": "not_applicable",
      "last4": "1111",
      "acs_check": "unchecked",
      "cvv_check": "unchecked"
    },
    "captured": true,
    "transaction_id": "",
    "failure_code": "65",
    "failure_message": "Authentication Required or Activity Limit Exceeded",
    "paid": false,
    "outcome": null,
    "payment_method_options": { "3d_required": false },
    "next_action": "not_applicable",
    "error_code": "ERROR_PROCESSING_CARD - [65]",
    "merchant_advice_code": "02",
    "merchant_advice_message": "Try again later"
  }
}"#;

/// `POST /v1/payments` error, verbatim from the *first* example on the same
/// page: an Account Funding Transaction the merchant is not entitled to. The
/// request never reached the card network, so every `data.*` error field is
/// empty and `merchant_advice_code` is `null`.
const CREATE_PAYMENT_ERROR_AFT: &str = r#"{
  "status": {
    "error_code": "ERROR_ACCOUNT_FUNDING_TRANSACTION",
    "status": "ERROR",
    "message": "The request tried to create an account funding transaction, but your organization is not configured for such transactions. The request was rejected. Corrective action: Contact Rapyd Client Support.",
    "response_code": "ERROR_ACCOUNT_FUNDING_TRANSACTION",
    "operation_id": "0d7a7a2e-6ab6-4f2c-9c17-4a2c66a1f0a6"
  },
  "data": {
    "id": "payment_2f0a1b6ab0a54a1197c62a49c9ee8f2b",
    "amount": 4,
    "currency_code": "EUR",
    "status": "ERR",
    "payment_method_data": {
      "type": "at_visa_card",
      "category": "card",
      "acs_check": "unchecked",
      "cvv_check": "unchecked"
    },
    "captured": true,
    "failure_code": "",
    "failure_message": "",
    "paid": false,
    "outcome": null,
    "next_action": "not_applicable",
    "error_code": "",
    "merchant_advice_code": null,
    "merchant_advice_message": null
  }
}"#;

/// The status-only envelope the sandbox decline cards return — verbatim from
/// <https://docs.rapyd.net/en/card-numbers-for-testing.html> (`4111111111111151`).
/// There is no `data` object at all, so the network code is recoverable only
/// from the bracket group of `error_code`.
const STATUS_ONLY_DECLINE_51: &str = r#"{
  "status": {
    "error_code": "ERROR_PROCESSING_CARD - [51]",
    "status": "ERROR",
    "message": "Insufficient Funds",
    "response_code": "ERROR_PROCESSING_CARD - [51]",
    "operation_id": "563694e5-3454-474a-92b0-24ae720538b7"
  }
}"#;

/// The `PAYMENT_FAILED` webhook payload, verbatim from
/// <https://docs.rapyd.net/en/payment-failed-webhook.html> — the one published
/// payload where `cvv_check` is `fail` and `acs_check` is `unavailable` at the
/// same time, proving the two checks are independent, and the only one carrying
/// both a populated `merchant_advice_code` and the qualified `data.error_code`.
///
/// Trimmed to the fields this connector reads. The one further deviation:
/// `authentication_result.cardholder_info` is dropped, because Rapyd's own
/// published text for it contains a spelling error that the repo's `typos` CI
/// gate rejects. Nothing here asserts on that field.
const WEBHOOK_FAILED_65: &str = r#"{
  "id": "payment_bb9c69b6d9b0aa4a5f1d0cb2d4e4f9e0",
  "amount": 1065,
  "status": "ERR",
  "next_action": "not_applicable",
  "currency_code": "ISK",
  "captured": true,
  "paid": false,
  "transaction_id": "",
  "merchant_reference_id": "",
  "error_code": "ERROR_PROCESSING_CARD - [65]",
  "failure_code": "65",
  "failure_message": "Authentication Required or Activity Limit Exceeded",
  "merchant_advice_code": "02",
  "merchant_advice_message": "Try again later",
  "payment_method_data": {
    "type": "gb_mastercard_card",
    "last4": "2867",
    "category": "card",
    "acs_check": "unavailable",
    "cvv_check": "fail",
    "next_action": "not_applicable",
    "payment_account_reference": "V0010013018036782991622965076"
  },
  "authentication_result": {
    "eci": "07",
    "result": "N",
    "version": "2.2.0"
  },
  "payment_method_options": { "3d_required": true }
}"#;

fn parse_error(body: &str) -> RapydPaymentsResponse {
    serde_json::from_str(body).expect("Rapyd's own published payload must deserialize")
}

// ---------------------------------------------------------------------------
// 1. The GSM triple
// ---------------------------------------------------------------------------

/// The load-bearing case. Every one of the three fields used to be hardcoded
/// `None` at all eight construction sites, so no Hyperswitch GSM rule could
/// ever fire on a Rapyd decline.
#[test]
fn a_network_decline_populates_all_three_gsm_fields() {
    let response = parse_error(CREATE_PAYMENT_ERROR_65);
    let data = response.data.as_ref().expect("data object must parse");
    let class = classify_rapyd_error(400, &response.status, Some(data));
    assert_eq!(class, RapydErrorClass::IssuerDecline);

    let fields = rapyd_network_error_fields(class, &response.status, Some(data));
    // The BARE scheme code, not the qualified "ERROR_PROCESSING_CARD - [65]".
    assert_eq!(fields.decline_code.as_deref(), Some("65"));
    // The Merchant Advice Code, zero-padded and passed through unmodified.
    assert_eq!(fields.advice_code.as_deref(), Some("02"));
    // The network's short decline reason — NOT "Try again later", which is the
    // MAC's retry advice and would destroy the GSM signal if routed here.
    assert_eq!(
        fields.error_message.as_deref(),
        Some("Authentication Required or Activity Limit Exceeded")
    );

    let error = rapyd_error_response(400, class, &response.status, Some(data), None, None);
    assert_eq!(error.code, "ERROR_PROCESSING_CARD - [65]");
    assert_eq!(
        error.message,
        "Authentication Required or Activity Limit Exceeded"
    );
    let reason = error.reason.expect("reason must carry Rapyd's own advice");
    assert!(reason.contains("Corrective action: Resubmit with 3DS required"));
    assert!(reason.contains("Try again later"));
    assert_eq!(error.network_decline_code.as_deref(), Some("65"));
    assert_eq!(error.network_advice_code.as_deref(), Some("02"));
}

/// A merchant-configuration rejection is NOT a card decline. Retrying it is
/// guaranteed to fail, so it must not feed GSM — and Rapyd tells us so by
/// leaving every `data.*` error field empty.
#[test]
fn a_merchant_configuration_rejection_feeds_no_gsm_field() {
    let response = parse_error(CREATE_PAYMENT_ERROR_AFT);
    let data = response.data.as_ref().expect("data object must parse");
    let class = classify_rapyd_error(400, &response.status, Some(data));
    assert_eq!(class, RapydErrorClass::MerchantConfiguration);

    let error = rapyd_error_response(400, class, &response.status, Some(data), None, None);
    assert_eq!(error.code, "ERROR_ACCOUNT_FUNDING_TRANSACTION");
    assert_eq!(error.network_decline_code, None);
    assert_eq!(error.network_advice_code, None);
    assert_eq!(error.network_error_message, None);
}

/// The shape the sandbox decline cards actually return: no `data` object, so
/// the scheme code survives only inside the bracket group.
#[test]
fn the_status_only_envelope_still_yields_a_decline_code() {
    let response = parse_error(STATUS_ONLY_DECLINE_51);
    assert!(response.data.is_none());
    let class = classify_rapyd_error(400, &response.status, None);
    assert_eq!(class, RapydErrorClass::IssuerDecline);

    let error = rapyd_error_response(400, class, &response.status, None, None, None);
    assert_eq!(error.code, "ERROR_PROCESSING_CARD - [51]");
    assert_eq!(error.network_decline_code.as_deref(), Some("51"));
    assert_eq!(
        error.network_error_message.as_deref(),
        Some("Insufficient Funds")
    );
    // Nothing in this shape carries a MAC, so it must stay absent rather than
    // being guessed from the decline code.
    assert_eq!(error.network_advice_code, None);
}

/// A webhook has no `status` envelope, so `data.error_code` is the only source
/// of the qualified code. The same decline must report the same code whether it
/// arrives over REST or over a webhook.
#[test]
fn a_webhook_payload_reports_the_same_codes_as_the_rest_error() {
    let data: ResponseData = serde_json::from_str(WEBHOOK_FAILED_65)
        .expect("Rapyd's own PAYMENT_FAILED payload must deserialize");
    let synthesised = RapydPaymentsResponse::from(data.clone());
    let class = classify_rapyd_error(200, &synthesised.status, Some(&data));
    assert_eq!(class, RapydErrorClass::IssuerDecline);

    let error = rapyd_error_response(200, class, &synthesised.status, Some(&data), None, None);
    assert_eq!(error.code, "ERROR_PROCESSING_CARD - [65]");
    assert_eq!(error.network_decline_code.as_deref(), Some("65"));
    assert_eq!(error.network_advice_code.as_deref(), Some("02"));
}

/// The card-network code is alphanumeric — Rapyd's own tables list `OF`, `TJ`,
/// `5C`, `N7`, `W1`, `XA`, `3X`, `1A`, `6P`, `B1`, `Q1`, `Z1`, `CV`. Parsing it
/// as a number would silently drop every one of them.
#[test]
fn the_network_code_is_an_opaque_alphanumeric_token() {
    for (qualified, expected) in [
        ("ERROR_PROCESSING_CARD - [51]", Some("51")),
        ("ERROR_PROCESSING_CARD - [05]", Some("05")),
        ("ERROR_PROCESSING_CARD - [OF]", Some("OF")),
        ("ERROR_PROCESSING_CARD - [5C]", Some("5C")),
        ("ERROR_PROCESSING_CARD - [N7]", Some("N7")),
        ("ERROR_PROCESSING_CARD - [XA]", Some("XA")),
        ("ERROR_PROCESSING_CARD - [1A]", Some("1A")),
        // No bracket group, an empty one, or a truncated one: never a panic,
        // never a fabricated code.
        ("ERROR_ACCOUNT_FUNDING_TRANSACTION", None),
        ("ERROR_PROCESSING_CARD - []", None),
        ("ERROR_PROCESSING_CARD - [51", None),
    ] {
        assert_eq!(
            network_code_from_qualified(qualified).as_deref(),
            expected,
            "code {qualified}"
        );
    }
}

/// Rapyd's prose says a webhook's `failure_message` is bracketed; both of
/// Rapyd's own payloads show it bare; a REST `status.message` is
/// `"[short] long"`. All three must reduce to the short reason.
#[test]
fn the_short_message_survives_rapyds_three_bracket_conventions() {
    assert_eq!(
        rapyd_short_message("[Insufficient Funds]"),
        "Insufficient Funds"
    );
    assert_eq!(
        rapyd_short_message("Insufficient Funds"),
        "Insufficient Funds"
    );
    assert_eq!(
        rapyd_short_message("[Do Not Honor] The request attempted a card operation."),
        "Do Not Honor"
    );
    // A malformed bracket group must not swallow the message.
    assert_eq!(rapyd_short_message("[unterminated"), "[unterminated");
    assert_eq!(rapyd_short_message("[]  something"), "[]  something");
}

// ---------------------------------------------------------------------------
// 2. Error classification — the transport class must never look like a decline
// ---------------------------------------------------------------------------

/// A rejected signature, a replayed idempotency key, a Rapyd-side fault or a
/// rate limit says nothing about whether the card was charged. Reporting any of
/// them as a card decline — or terminally failing the attempt — is how a
/// charged payment gets reported as FAILURE.
#[test]
fn transport_failures_are_never_declines() {
    for code in [
        "MISSING_AUTHENTICATION_HEADERS",
        "UNAUTHENTICATED_API_CALL",
        "IDEMPOTENCY_ERROR",
        "GENERAL_ERROR",
        "ERROR_REPORTS_RATE_LIMIT_EXCEEDED",
    ] {
        let body = format!(
            r#"{{"status":{{"error_code":"{code}","status":"ERROR","message":"rejected","response_code":"{code}","operation_id":"op"}}}}"#
        );
        let response = parse_error(&body);
        assert_eq!(
            classify_rapyd_error(400, &response.status, None),
            RapydErrorClass::Transport,
            "code {code}"
        );
        let error = rapyd_error_response(
            400,
            RapydErrorClass::Transport,
            &response.status,
            None,
            None,
            None,
        );
        assert_eq!(error.network_decline_code, None, "code {code}");
        assert_eq!(error.network_advice_code, None, "code {code}");
        assert_eq!(error.network_error_message, None, "code {code}");
    }
}

/// Rapyd documents no mapping from error code to HTTP status, so the status is
/// never used to *prove* a decline — only to widen the transport class, which
/// can only ever prevent a false terminal failure.
#[test]
fn an_auth_or_rate_limit_status_widens_the_transport_class() {
    let body = r#"{"status":{"error_code":"SOMETHING_RAPYD_ADDED_LATER","status":"ERROR","message":"x","response_code":"","operation_id":"op"}}"#;
    let response = parse_error(body);
    for status_code in [401, 403, 408, 409, 429] {
        assert_eq!(
            classify_rapyd_error(status_code, &response.status, None),
            RapydErrorClass::Transport,
            "http {status_code}"
        );
    }
    // On an ordinary 4xx the same unknown code is a merchant/request problem,
    // not a decline: it still feeds no GSM field.
    assert_eq!(
        classify_rapyd_error(400, &response.status, None),
        RapydErrorClass::MerchantConfiguration
    );
}

/// Named issuer declines that carry no bracketed network code are still class
/// (a) — and their near-twins on the request side are still class (b).
/// `ERROR_CARD_CVV_NOT_VALID` ("correctly formatted, but not valid" — the
/// issuer rejected it) vs `INVALID_CARD_CVV` ("set cvv to a valid value" — we
/// sent a malformed value) is the pair that is easiest to get backwards.
#[test]
fn named_issuer_declines_are_separated_from_their_request_side_twins() {
    for (code, expected) in [
        ("ERROR_CARD_CVV_NOT_VALID", RapydErrorClass::IssuerDecline),
        ("INVALID_CARD_CVV", RapydErrorClass::MerchantConfiguration),
        (
            "ERROR_CARD_INFORMATION_NOT_VALID",
            RapydErrorClass::IssuerDecline,
        ),
        (
            "INVALID_CARD_NUMBER",
            RapydErrorClass::MerchantConfiguration,
        ),
        (
            "ERROR_CREATE_PAYMENT_ADDRESS_VERIFICATION_FAILURE",
            RapydErrorClass::IssuerDecline,
        ),
        (
            "ERROR_CREATE_PAYMENT_INSUFFICIENT_FUNDS",
            RapydErrorClass::IssuerDecline,
        ),
        // An amount limit set by the payment method is NOT an issuer decline,
        // however much it reads like one.
        (
            "ERROR_CREATE_PAYMENT_AMOUNT_EXCEEDS_MAXIMUM",
            RapydErrorClass::MerchantConfiguration,
        ),
    ] {
        let body = format!(
            r#"{{"status":{{"error_code":"{code}","status":"ERROR","message":"x","response_code":"{code}","operation_id":"op"}}}}"#
        );
        let response = parse_error(&body);
        assert_eq!(
            classify_rapyd_error(400, &response.status, None),
            expected,
            "code {code}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. attempt_status — the flow-aware treatment
// ---------------------------------------------------------------------------

/// The Refund flow's status must be a `FlowStatus::Refund(..)`.
/// `generate_refund_response` reads `attempt_status` and nothing else, so a
/// `None` reports REFUND_STATUS_UNSPECIFIED; and `ForeignFrom<FlowStatus> for
/// RefundStatus` maps every `Payment(_)` to `RefundFailure`, so a payment status
/// would be laundered into a terminal refund failure.
#[test]
fn refund_and_payment_statuses_do_not_leak_into_each_other() {
    let response = parse_error(CREATE_PAYMENT_ERROR_65);
    let data = response.data.as_ref().expect("data object must parse");
    let class = classify_rapyd_error(400, &response.status, Some(data));

    let refund_error = rapyd_error_response(
        400,
        class,
        &response.status,
        Some(data),
        None,
        Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
    );
    assert!(matches!(
        refund_error.attempt_status,
        Some(FlowStatus::Refund(common_enums::RefundStatus::Failure))
    ));
    assert_eq!(
        refund_error
            .attempt_status
            .as_ref()
            .unwrap()
            .as_attempt_status(),
        None
    );

    // A capture rejection reports CaptureFailed, not a failed payment: the
    // authorization it was capturing still stands.
    let capture_error = rapyd_error_response(
        400,
        class,
        &response.status,
        Some(data),
        None,
        Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
    );
    assert_eq!(
        capture_error.attempt_status.unwrap().as_attempt_status(),
        Some(AttemptStatus::CaptureFailed)
    );
}

/// A 2xx carrying Rapyd's status-only ERROR envelope on the refund path used to
/// be reported as `Ok`, with the ERROR CODE stuffed into `connector_refund_id`
/// and the reason discarded entirely.
#[test]
fn a_two_hundred_carrying_a_refund_error_becomes_an_error_response() {
    use domain_types::{
        connector_flow::Refund,
        connector_types::{RefundFlowData, RefundsData, RefundsResponseData},
        router_data_v2::RouterDataV2,
    };
    use transformers::RefundResponse;

    let parsed: RefundResponse =
        serde_json::from_str(STATUS_ONLY_DECLINE_51).expect("status-only envelope must parse");
    assert!(parsed.data.is_none());

    let class = classify_rapyd_error(200, &parsed.status, None);
    let error = rapyd_error_response(
        200,
        class,
        &parsed.status,
        None,
        None,
        Some(FlowStatus::Refund(common_enums::RefundStatus::Failure)),
    );
    // The refund id is NOT the error code.
    assert_eq!(error.code, "ERROR_PROCESSING_CARD - [51]");
    assert_eq!(error.message, "Insufficient Funds");
    assert!(matches!(
        error.attempt_status,
        Some(FlowStatus::Refund(common_enums::RefundStatus::Failure))
    ));

    // Keep the type parameters of the real impl referenced so this test breaks
    // if the Refund router-data shape moves.
    fn _assert_shape(_: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) {}
}

// ---------------------------------------------------------------------------
// 4. AVS / CVV / ACS verification results
// ---------------------------------------------------------------------------

/// `cvv_check` and `acs_check` are independent: Rapyd's own `PAYMENT_FAILED`
/// payload carries `acs_check: "unavailable"` alongside `cvv_check: "fail"`.
/// Both are surfaced, and neither drives status.
#[test]
fn cvv_and_acs_checks_are_parsed_and_surfaced_independently() {
    let data: ResponseData = serde_json::from_str(WEBHOOK_FAILED_65)
        .expect("Rapyd's own PAYMENT_FAILED payload must deserialize");
    let pmd = data
        .payment_method_data
        .as_ref()
        .expect("payment_method_data must parse");
    assert_eq!(pmd.cvv_check, Some(RapydCheckResult::Fail));
    assert_eq!(pmd.acs_check, Some(RapydCheckResult::Unavailable));

    let metadata = build_connector_metadata(&data).expect("metadata must be present");
    let checks = &metadata["verification_checks"];
    assert_eq!(checks["cvv_check"], serde_json::json!("fail"));
    assert_eq!(checks["acs_check"], serde_json::json!("unavailable"));
    // Absent checks are omitted rather than serialised as null.
    assert!(checks.get("avs_check").is_none());
    assert!(checks.get("avs_result").is_none());
}

/// Rapyd's AVS field is documented with NO value set on any page and appears in
/// zero published examples, while the one `avs_required: true` example returns a
/// differently-named `avs_result` whose only observed value is `"X"`. Both are
/// modelled, both opaque — an enum here would be inventing a contract.
#[test]
fn avs_results_are_opaque_strings_under_both_names() {
    let body = WEBHOOK_FAILED_65.replace(
        r#""cvv_check": "fail","#,
        r#""cvv_check": "fail", "avs_check": "Y", "avs_result": "X","#,
    );
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    let pmd = data
        .payment_method_data
        .as_ref()
        .expect("payment_method_data must parse");
    assert_eq!(pmd.avs_check.as_deref(), Some("Y"));
    assert_eq!(pmd.avs_result.as_deref(), Some("X"));

    let metadata = build_connector_metadata(&data).expect("metadata must be present");
    assert_eq!(
        metadata["verification_checks"]["avs_check"],
        serde_json::json!("Y")
    );
    assert_eq!(
        metadata["verification_checks"]["avs_result"],
        serde_json::json!("X")
    );

    // Neither AVS field is allowed to move the status.
    assert_eq!(
        transformers::get_status_for_webhook(&data),
        AttemptStatus::Failure
    );
}

/// The verification checks ride on `payment_method_data`, which is the SAME
/// struct across every payment method. A value Rapyd adds later, and a non-card
/// payment method that omits them entirely, must both parse.
#[test]
fn an_unknown_check_value_parses_instead_of_failing_the_whole_payment() {
    let body = WEBHOOK_FAILED_65.replace(r#""cvv_check": "fail""#, r#""cvv_check": "deferred""#);
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    assert_eq!(
        data.payment_method_data
            .as_ref()
            .and_then(|pmd| pmd.cvv_check),
        Some(RapydCheckResult::Unknown)
    );

    let bare = r#"{"id":"payment_x","amount":10,"status":"CLO","next_action":"not_applicable",
                   "payment_method_data":{"type":"pl_p24_bank","category":"bank_redirect"}}"#;
    let data: ResponseData = serde_json::from_str(bare).expect("non-card body must deserialize");
    let pmd = data
        .payment_method_data
        .as_ref()
        .expect("payment_method_data must parse");
    assert_eq!(pmd.cvv_check, None);
    assert_eq!(pmd.acs_check, None);
    assert_eq!(build_connector_metadata(&data), None);
}

/// A non-null Merchant Advice Code is NOT evidence of a decline: codes `15` and
/// `16` are documented to ride on *successful* payments to flag a
/// non-reloadable prepaid or single-use virtual card. The advice code is only
/// ever read while building an `ErrorResponse`.
#[test]
fn a_merchant_advice_code_on_a_success_is_not_a_decline() {
    let body = r#"{
      "id": "payment_success_with_mac",
      "amount": 10,
      "status": "CLO",
      "next_action": "not_applicable",
      "captured": true,
      "paid": true,
      "failure_code": "",
      "failure_message": "",
      "error_code": "",
      "merchant_advice_code": "16",
      "merchant_advice_message": "The issuer recognizes the product as a consumer single-use virtual card number"
    }"#;
    let data: ResponseData = serde_json::from_str(body).expect("body must deserialize");
    assert_eq!(data.merchant_advice_code.as_deref(), Some("16"));
    assert_eq!(
        get_status_for_payment_response(&data, 200).expect("a closed payment must resolve"),
        AttemptStatus::Charged
    );
    // And the classifier does not read a decline out of it either.
    let synthesised = RapydPaymentsResponse::from(data.clone());
    assert_eq!(
        classify_rapyd_error(200, &synthesised.status, Some(&data)),
        RapydErrorClass::MerchantConfiguration
    );
}

/// `outcome` is documented but `null` in every published Rapyd example, so it
/// is parsed for diagnostics and never drives a decision. Both `null` and a
/// populated object must parse.
#[test]
fn the_outcome_object_parses_but_drives_nothing() {
    let response = parse_error(CREATE_PAYMENT_ERROR_65);
    let data = response.data.as_ref().expect("data object must parse");
    assert!(data.outcome.is_none());

    let body = WEBHOOK_FAILED_65.replace(
        r#""failure_code": "65","#,
        r#""failure_code": "65", "outcome": {"network_status":"declined_by_network","risk_level":"normal","seller_message":"declined","type":"issuer_declined","reason":null},"#,
    );
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    let outcome = data.outcome.as_deref().expect("outcome must parse");
    assert_eq!(
        outcome.network_status,
        Some(transformers::RapydNetworkStatus::DeclinedByNetwork)
    );
    // A value Rapyd adds later parses to Unknown rather than failing the body.
    let body = body.replace("declined_by_network", "partially_approved_by_network");
    let data: ResponseData = serde_json::from_str(&body).expect("body must deserialize");
    assert_eq!(
        data.outcome
            .as_deref()
            .and_then(|outcome| outcome.network_status),
        Some(transformers::RapydNetworkStatus::Unknown)
    );
}
