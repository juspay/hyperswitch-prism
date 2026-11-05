#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use domain_types::{
    connector_types::{
        ConnectorEnum, ConnectorVariant, EventType, HttpMethod, PayoutConnectorEnum,
        PayoutWebhookReference, RequestDetails, WebhookParseResponse, WebhookResourceReference,
    },
    errors::WebhookError,
};
use interfaces::webhook_parse::WebhookParse;

use crate::{types::WebhookConnectorData, webhook_utils::parse_webhook_event};

fn request(body: &[u8]) -> RequestDetails {
    RequestDetails {
        method: HttpMethod::Post,
        uri: None,
        headers: Default::default(),
        body: body.to_vec(),
        query_params: None,
    }
}

struct PayoutParser;

struct RejectingParser;

impl WebhookParse for RejectingParser {
    fn parse_event(
        &self,
        _request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhookReferenceIdNotFound.into())
    }
}

impl WebhookParse for PayoutParser {
    fn parse_event(
        &self,
        _request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
        Ok(WebhookParseResponse {
            event_type: EventType::PayoutSuccess,
            reference: Some(WebhookResourceReference::Payout(PayoutWebhookReference {
                connector_payout_id: Some("payout_123".into()),
                merchant_payout_id: None,
            })),
        })
    }
}

#[test]
fn standalone_parser_returns_payout_reference_without_payment_connector() {
    let response = parse_webhook_event(
        WebhookConnectorData {
            connector: Box::new(PayoutParser),
        },
        request(b"{}"),
    )
    .expect("standalone parser should succeed");

    assert_eq!(
        response.event_type,
        Some(i32::from(
            grpc_api_types::payments::WebhookEventType::PayoutSuccess
        ))
    );
    match response.reference.and_then(|reference| reference.resource) {
        Some(grpc_api_types::payments::event_reference::Resource::Payout(reference)) => {
            assert_eq!(reference.connector_payout_id.as_deref(), Some("payout_123"));
        }
        other => panic!("expected payout reference, got {other:?}"),
    }
}

#[test]
fn unmigrated_payment_connector_keeps_existing_parser() {
    let variant = ConnectorVariant::Payment(ConnectorEnum::Cashtocode);
    assert!(super::get_connector(&variant).is_none());
    let connector = WebhookConnectorData::from_connector_variant(&variant)
        .expect("payment fallback should resolve");
    let response = parse_webhook_event(connector, request(b"{}"))
        .expect("legacy payment parser should succeed");
    assert_eq!(
        response.event_type,
        Some(i32::from(
            grpc_api_types::payments::WebhookEventType::PaymentIntentSuccess
        ))
    );
    assert!(response.reference.is_none());
}

#[test]
fn malformed_legacy_payload_preserves_connector_error() {
    let connector = WebhookConnectorData::from_connector_variant(&ConnectorVariant::Payment(
        ConnectorEnum::Stripe,
    ))
    .expect("payment fallback should resolve");
    let error = parse_webhook_event(connector, request(b"not json"))
        .expect_err("malformed input must fail");
    assert!(matches!(
        error.current_context(),
        WebhookError::WebhookEventTypeNotFound
    ));
}

#[test]
fn selected_parser_error_is_returned_without_legacy_retry() {
    let error = parse_webhook_event(
        WebhookConnectorData {
            connector: Box::new(RejectingParser),
        },
        request(br#"{"type":"payout_executed","event_version":1,"event_id":"evt_1","payout_id":"payout_legacy"}"#),
    )
    .expect_err("selected parser failure must propagate");
    assert!(matches!(
        error.current_context(),
        WebhookError::WebhookReferenceIdNotFound
    ));
}

#[test]
fn unregistered_payout_variant_does_not_silently_use_payment_lookup() {
    assert!(
        WebhookConnectorData::from_connector_variant(&ConnectorVariant::Payout(
            PayoutConnectorEnum::Paypal,
        ))
        .is_none()
    );
}

fn assert_reference(
    response: grpc_api_types::payments::EventServiceParseResponse,
    kind: &str,
    id: &str,
) {
    use grpc_api_types::payments::event_reference::Resource;
    match (kind, response.reference.and_then(|r| r.resource)) {
        ("payment", Some(Resource::Payment(r))) => {
            assert_eq!(r.connector_transaction_id.as_deref(), Some(id))
        }
        ("payout", Some(Resource::Payout(r))) => {
            assert_eq!(r.connector_payout_id.as_deref(), Some(id))
        }
        ("refund", Some(Resource::Refund(r))) => {
            assert_eq!(r.connector_refund_id.as_deref(), Some(id))
        }
        ("none", None) => {}
        (_, actual) => panic!("expected {kind} reference, got {actual:?}"),
    }
}

#[test]
fn truelayer_registered_parser_supports_payment_and_payout_variants() {
    use grpc_api_types::payments::WebhookEventType as Event;
    let cases = [
        ("payout_executed", Event::PayoutSuccess, "payout", "po_1"),
        ("payout_failed", Event::PayoutFailure, "payout", "po_1"),
        (
            "payment_settled",
            Event::PaymentIntentSuccess,
            "payment",
            "pay_1",
        ),
        (
            "payment_failed",
            Event::PaymentIntentFailure,
            "payment",
            "pay_1",
        ),
        (
            "payment_executed",
            Event::PaymentIntentProcessing,
            "payment",
            "pay_1",
        ),
        (
            "refund_executed",
            Event::WebhookRefundSuccess,
            "refund",
            "re_1",
        ),
        (
            "refund_failed",
            Event::WebhookRefundFailure,
            "refund",
            "re_1",
        ),
        ("new_unknown_event", Event::Unspecified, "none", ""),
        ("payment_disputed", Event::Unspecified, "none", ""),
    ];
    for variant in [
        ConnectorVariant::Payment(ConnectorEnum::Truelayer),
        ConnectorVariant::Payout(PayoutConnectorEnum::Truelayer),
    ] {
        assert!(super::get_connector(&variant).is_some());
        for (event, expected, kind, id) in cases {
            let mut payload =
                serde_json::json!({"type": event, "event_version": 1, "event_id":"evt_1"});
            let fields = payload.as_object_mut().expect("object fixture");
            match kind {
                "payout" => {
                    fields.insert("payout_id".into(), "po_1".into());
                }
                "payment" => {
                    fields.insert("payment_id".into(), "pay_1".into());
                }
                "refund" => {
                    fields.insert("payment_id".into(), "pay_1".into());
                    fields.insert("refund_id".into(), "re_1".into());
                }
                _ => {}
            }
            let response = parse_webhook_event(
                WebhookConnectorData::from_connector_variant(&variant).unwrap(),
                request(&serde_json::to_vec(&payload).unwrap()),
            )
            .unwrap();
            assert_eq!(response.event_type, Some(i32::from(expected)), "{event}");
            assert_reference(response, kind, id);
        }
    }
}

fn trustly_payload(method: &str, message_id: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "method":method, "version":"1.1",
        "params":{"signature":"sig", "uuid":"uuid_1", "data":{
            "messageid":message_id, "orderid":"order_1", "notificationid":"notice_1"
        }}
    }))
    .unwrap()
}

#[test]
fn trustly_registered_parser_distinguishes_payouts_from_refunds() {
    use grpc_api_types::payments::WebhookEventType as Event;
    let cases = [
        (
            "payoutconfirmation",
            "payout_1",
            Event::PayoutSuccess,
            "payout",
        ),
        ("payoutfailed", "payout_1", Event::PayoutFailure, "payout"),
        ("credit", "payout_1", Event::PayoutReversed, "payout"),
        ("cancel", "payout_1", Event::PayoutCancelled, "payout"),
        (
            "payoutconfirmation",
            "refund_1",
            Event::WebhookRefundSuccess,
            "refund",
        ),
        (
            "payoutfailed",
            "refund_1",
            Event::WebhookRefundFailure,
            "refund",
        ),
        (
            "credit",
            "payment_1",
            Event::PaymentIntentSuccess,
            "payment",
        ),
        ("debit", "payment_1", Event::PaymentIntentFailure, "payment"),
        (
            "cancel",
            "payment_1",
            Event::PaymentIntentCancelled,
            "payment",
        ),
        (
            "pending",
            "payment_1",
            Event::PaymentIntentProcessing,
            "payment",
        ),
        (
            "account",
            "payment_1",
            Event::PaymentAssociatedDataUpdate,
            "payment",
        ),
    ];
    for variant in [
        ConnectorVariant::Payment(ConnectorEnum::Trustly),
        ConnectorVariant::Payout(PayoutConnectorEnum::Trustly),
    ] {
        assert!(super::get_connector(&variant).is_some());
        for (method, message_id, expected, kind) in cases {
            let response = parse_webhook_event(
                WebhookConnectorData::from_connector_variant(&variant).unwrap(),
                request(&trustly_payload(method, message_id)),
            )
            .unwrap();
            assert_eq!(
                response.event_type,
                Some(i32::from(expected)),
                "{method}/{message_id}"
            );
            assert_reference(response, kind, "order_1");
        }
    }
}

#[test]
fn migrated_parsers_reject_invalid_payloads_without_legacy_fallback() {
    for variant in [
        ConnectorVariant::Payout(PayoutConnectorEnum::Truelayer),
        ConnectorVariant::Payout(PayoutConnectorEnum::Trustly),
    ] {
        for body in [b"not json".as_slice(), b"{}".as_slice()] {
            let error = parse_webhook_event(
                WebhookConnectorData::from_connector_variant(&variant).unwrap(),
                request(body),
            )
            .unwrap_err();
            assert!(matches!(
                error.current_context(),
                WebhookError::WebhookBodyDecodingFailed
            ));
        }
    }
    let missing_id = br#"{"type":"payout_executed","event_version":1,"event_id":"evt_1"}"#;
    let error = parse_webhook_event(
        WebhookConnectorData::from_connector_variant(&ConnectorVariant::Payout(
            PayoutConnectorEnum::Truelayer,
        ))
        .unwrap(),
        request(missing_id),
    )
    .unwrap_err();
    assert!(matches!(
        error.current_context(),
        WebhookError::WebhookBodyDecodingFailed
    ));
}

#[test]
fn shared_truelayer_account_identifiers_preserve_payment_serialization() {
    // Payment request construction still uses the re-exported shared wire type.
    use crate::connectors::truelayer::transformers::TruelayerAccountIdentifier;

    for payload in [
        serde_json::json!({"type":"iban", "iban":"test_iban"}),
        serde_json::json!({"type":"sort_code_account_number", "sort_code":"123456", "account_number":"12345678"}),
    ] {
        let identifier: TruelayerAccountIdentifier =
            serde_json::from_value(payload.clone()).expect("valid account identifier");
        assert_eq!(
            serde_json::to_value(identifier).expect("serializable account identifier"),
            payload,
            "unused account fields must remain absent from outgoing payment requests"
        );
    }
}
