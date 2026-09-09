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

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

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
            assert_eq!(data.status, transformers::RapydPaymentStatus::Closed);
            assert_eq!(
                data.next_action,
                Some(transformers::NextAction::NotApplicable)
            );
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
            assert_eq!(data.status, transformers::RapydPaymentStatus::Error);
            assert_eq!(data.next_action, None);
            assert_eq!(data.transaction_id, None);
            assert_eq!(
                transformers::get_status_for_webhook(&data),
                common_enums::AttemptStatus::Failure
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

    assert_eq!(response.status, common_enums::AttemptStatus::Charged);
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
