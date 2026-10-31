//! Signature known-answer tests use synthetic credentials, never live secrets.
//! Expected digests were independently generated with Python hashlib.
//! Merchant API PDF pp. 4–6 defines MD5 signing; pp. 8–10 defines generic SHA-512.
#![allow(clippy::unwrap_used)]

use super::transformers::{
    compute_generic_signature, compute_merchant_api_signature, compute_payment_signature,
    PaymentSignatureInput,
};

use super::transformers::{ReddotPSyncResponse, ReddotRefundResponse};
use crate::types::ResponseRouterData;
use common_enums::AttemptStatus;
use domain_types::{
    connector_flow::{PSync, Refund},
    connector_types::{
        PaymentFlowData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData,
        RefundsResponseData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use hyperswitch_masking::Secret;
use serde_json::json;

#[test]
fn payment_signature_matches_fixed_sha512_vector() {
    // MID + order + type + amount + currency + first6/last4 + expiry + CVC last digit + secret.
    let signature = compute_payment_signature(PaymentSignatureInput {
        mid: "test-mid",
        order_id: "TST103",
        payment_type: "S",
        amount: "0.01",
        ccy: "SGD",
        card_no: "4111111111111111",
        exp_date: "122030",
        cvv2: "123",
        secret_key: "test-secret",
    })
    .unwrap();
    assert_eq!(signature, "32acd66b55e4a0270ddc220e84d44b28e7c1466cb06be91698bab21582ac52f6e07cc18eefdc5d2b3824e1401e5b62f8fa40930dce340c15fe491276e0c72a25");
}

#[test]
fn generic_signature_sorts_keys_and_matches_documented_sha512_algorithm() {
    let signature = compute_generic_signature(
        vec![
            ("transaction_id", "po01_9403311911650893436"),
            ("request_mid", "test-mid"),
        ],
        "test-secret",
    );
    assert_eq!(signature, "47354386c1899f5b24a1cbe5289cd82518b4cf31139db19dc96b14072d82436ff295be55dbe6023a117a4f6f6a55642df94af20923d30b3258581e705c6122cf");
}

#[test]
fn merchant_signature_matches_documented_md5_algorithm() {
    // TST103, 0.01 SGD and the transaction ID follow the PDF's request examples.
    // Include refund_id to cover the additional field sent by this connector.
    let signature = compute_merchant_api_signature(
        vec![
            ("transaction_id", "po01_9403311911650893436".to_string()),
            ("response_type", "json".to_string()),
            ("refund_id", "refund-1".to_string()),
            ("order_number", "TST103".to_string()),
            ("mid", "test-mid".to_string()),
            ("currency", "SGD".to_string()),
            ("amount", "0.01".to_string()),
            ("action_type", "refund".to_string()),
        ],
        "test-secret",
    )
    .unwrap();
    assert_eq!(signature, "50d2138ab6f4d5ef780988a09f08777e");
}

fn config() -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Reddot {
        mid: Secret::new("test-mid".to_string()),
        secret: Secret::new("test-secret".to_string()),
        acquirer_cybersource: "false".to_string(),
        base_url: None,
    }
}

fn payment_sync_data(
) -> RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData> {
    RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: PaymentFlowData {
            merchant_id: Default::default(),
            customer_id: Default::default(),
            connector_customer: Default::default(),
            payment_id: Default::default(),
            attempt_id: Default::default(),
            status: Default::default(),
            payment_method: Default::default(),
            payment_method_type: Default::default(),
            description: Default::default(),
            return_url: Default::default(),
            address: Default::default(),
            auth_type: Default::default(),
            connector_feature_data: Default::default(),
            amount_captured: Default::default(),
            amount_capturable: Default::default(),
            amount: Default::default(),
            access_token: Default::default(),
            session_token: Default::default(),
            reference_id: Default::default(),
            connector_order_id: Default::default(),
            preprocessing_id: Default::default(),
            connector_api_version: Default::default(),
            connector_request_reference_id: Default::default(),
            test_mode: Default::default(),
            connector_http_status_code: Default::default(),
            connector_response_headers: Default::default(),
            external_latency: Default::default(),
            connectors: Default::default(),
            raw_connector_response: Default::default(),
            typed_connector_response: Default::default(),
            raw_connector_request: Default::default(),
            typed_connector_request: Default::default(),
            vault_headers: Default::default(),
            connector_response: Default::default(),
            recurring_mandate_payment_data: Default::default(),
            order_details: Default::default(),
            amount_authorized: Default::default(),
            l2_l3_data: Default::default(),
            merchant_request_id: Default::default(),
            sender_payment_instrument_id: Default::default(),
            connector_returned_payment_method_details: Default::default(),
            settlement_status: Default::default(),
            raw_connector_status: Default::default(),
        },
        connector_config: config(),
        request: PaymentsSyncData::default(),
        response: Err(ErrorResponse::default()),
    }
}

fn refund_data() -> RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData> {
    RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: RefundFlowData {
            merchant_id: Default::default(),
            status: Default::default(),
            refund_id: Default::default(),
            connectors: Default::default(),
            connector_request_reference_id: Default::default(),
            raw_connector_response: Default::default(),
            typed_connector_response: Default::default(),
            connector_response_headers: Default::default(),
            raw_connector_request: Default::default(),
            typed_connector_request: Default::default(),
            access_token: Default::default(),
            connector_feature_data: Default::default(),
            test_mode: Default::default(),
            payment_method: Default::default(),
            merchant_request_id: Default::default(),
            raw_connector_status: Default::default(),
        },
        connector_config: config(),
        request: RefundsData {
            refund_id: "refund-1".to_string(),
            connector_transaction_id: Default::default(),
            connector_refund_id: Default::default(),
            customer_id: Default::default(),
            currency: Default::default(),
            reason: Default::default(),
            webhook_url: Default::default(),
            connector_feature_data: Default::default(),
            refund_connector_metadata: Default::default(),
            payment_amount: Default::default(),
            refund_amount: Default::default(),
            refund_status: Default::default(),
            merchant_account_id: Default::default(),
            capture_method: Default::default(),
            integrity_object: Default::default(),
            browser_info: Default::default(),
            split_refunds: Default::default(),
            split_settlement_refund: Default::default(),
            connector_order_id: Default::default(),
            payment_method_data: Default::default(),
            currency_conversion_data: Default::default(),
        },
        response: Err(ErrorResponse::default()),
    }
}

#[test]
fn enquiry_errors_without_a_transaction_preserve_the_existing_payment_state() {
    for transaction_id in [json!(null), json!("")] {
        for code in ["-1003", "-1009", "unknown-code"] {
            let response: ReddotPSyncResponse = serde_json::from_value(json!({
                "response_code": code, "response_msg": "Enquiry error", "transaction_id": transaction_id,
            })).unwrap();
            let mut router_data = payment_sync_data();
            router_data.resource_common_data.status = AttemptStatus::AuthenticationPending;
            let result = RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data,
                http_code: 200,
            })
            .unwrap();
            assert_eq!(
                result.resource_common_data.status,
                AttemptStatus::AuthenticationPending
            );
            let error = result.response.unwrap_err();
            assert!(error.attempt_status.is_none());
            assert_eq!(error.code, code);
        }
    }
}

#[test]
fn transaction_errors_follow_rdp_confirmed_status_mapping() {
    for code in ["-1003", "-1009", "-1", "unknown-code"] {
        let response: ReddotPSyncResponse = serde_json::from_value(json!({
            "response_code": code, "transaction_id": "payment-1"
        }))
        .unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: payment_sync_data(),
            http_code: 200,
        })
        .unwrap();
        assert_eq!(
            result.resource_common_data.status,
            AttemptStatus::AuthenticationFailed
        );
        assert!(result.response.unwrap_err().attempt_status.is_some());
    }
}

#[test]
fn refund_omitting_its_optional_id_retains_the_sent_enquiry_key() {
    for refund_id in [json!(null), json!(""), json!("gateway-echo")] {
        let response: ReddotRefundResponse = serde_json::from_value(json!({
            "result_status": "pending", "refund_id": refund_id
        }))
        .unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: refund_data(),
            http_code: 200,
        })
        .unwrap();
        let result = result.response.unwrap();
        let expected = refund_id
            .as_str()
            .filter(|id| !id.is_empty())
            .unwrap_or("refund-1");
        assert_eq!(result.connector_refund_id, expected);
    }
}

use super::transformers::{
    ReddotAuthType, ReddotAuthorizeRequest, ReddotAuthorizeResponse, ReddotPSyncRequest,
    ReddotRefundRequest, ReddotRefundSyncResponse,
};
use super::{Reddot, ReddotRouterData};
use common_enums::{CaptureMethod, Currency, RefundStatus};
use common_utils::types::{MinorUnit, Money};
use domain_types::{
    connector_flow::{Authorize, RSync},
    connector_types::{PaymentsAuthorizeData, RefundSyncData},
    payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData},
};
use hyperswitch_masking::PeekInterface;

type AuthorizeData = RouterDataV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<DefaultPCIHolder>,
    PaymentsResponseData,
>;

fn authorize_data() -> AuthorizeData {
    let mut data = RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: payment_sync_data().resource_common_data,
        connector_config: config(),
        request: PaymentsAuthorizeData {
            payment_method_data: PaymentMethodData::Card(Card::default()),
            amount: Default::default(),
            order_tax_amount: Default::default(),
            surcharge_amount: Default::default(),
            email: Default::default(),
            customer_document_details: Default::default(),
            customer_date_of_birth: Default::default(),
            customer_name: Default::default(),
            currency: Default::default(),
            confirm: Default::default(),
            billing_descriptor: Default::default(),
            capture_method: Default::default(),
            router_return_url: Default::default(),
            webhook_url: Default::default(),
            complete_authorize_url: Default::default(),
            mandate_id: Default::default(),
            setup_future_usage: Default::default(),
            off_session: Default::default(),
            browser_info: Default::default(),
            order_category: Default::default(),
            session_token: Default::default(),
            access_token: Default::default(),
            customer_acceptance: Default::default(),
            enrolled_for_3ds: Default::default(),
            related_transaction_id: Default::default(),
            payment_experience: Default::default(),
            payment_method_type: Default::default(),
            customer_id: Default::default(),
            request_incremental_authorization: Default::default(),
            metadata: Default::default(),
            authentication_data: Default::default(),
            split_payments: Default::default(),
            split_settlement: Default::default(),
            merchant_order_id: Default::default(),
            shipping_cost: Default::default(),
            merchant_account_id: Default::default(),
            integrity_object: Default::default(),
            integrity_specifications: Default::default(),
            merchant_config_currency: Default::default(),
            all_keys_required: Default::default(),
            request_extended_authorization: Default::default(),
            enable_overcapture: Default::default(),
            setup_mandate_details: Default::default(),
            connector_feature_data: Default::default(),
            connector_testing_data: Default::default(),
            payment_channel: Default::default(),
            enable_partial_authorization: Default::default(),
            locale: Default::default(),
            redirect_response: Default::default(),
            threeds_method_comp_ind: Default::default(),
            continue_redirection_url: Default::default(),
            tokenization: Default::default(),
            mit_category: Default::default(),
            domain_data: Default::default(),
            risk_assessment_data: Default::default(),
            partner_merchant_identifier_details: Default::default(),
            currency_conversion_data: Default::default(),
            is_account_funding_transaction: Default::default(),
            recipient_details: Default::default(),
            additional_connector_details: Default::default(),
            customer: Default::default(),
            business_country: Default::default(),
            enable_avs_check: Default::default(),
        },
        response: Err(ErrorResponse::default()),
    };
    data.resource_common_data.connector_request_reference_id = "attempt-1".to_string();
    data.request.amount = Money {
        amount: MinorUnit::new(110),
        currency: Currency::SGD,
    };
    data.request.currency = Currency::JPY; // Deliberately differ from the authoritative Money.
    data.request.email = Some("payer@example.com".parse().unwrap());
    data.request.router_return_url = Some("https://example.com/return".to_string());
    data.request.webhook_url = Some("https://example.com/webhook".to_string());
    data.request.payment_method_data = PaymentMethodData::Card(Card {
        card_number: domain_types::payment_method_data::RawCardNumber(
            "4111111111111111".parse().unwrap(),
        ),
        card_exp_month: Secret::new("12".to_string()),
        card_exp_year: Secret::new("2030".to_string()),
        card_cvc: Secret::new("123".to_string()),
        card_holder_name: Some(Secret::new("Test Payer".to_string())),
        ..Default::default()
    });
    data
}

fn authorize_request(data: AuthorizeData) -> ReddotAuthorizeRequest<DefaultPCIHolder> {
    ReddotAuthorizeRequest::try_from(ReddotRouterData {
        connector: Reddot::<DefaultPCIHolder>::new().clone(),
        router_data: data,
    })
    .unwrap()
}

fn refund_sync_data(
    id: &str,
) -> RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData> {
    RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: refund_data().resource_common_data,
        connector_config: config(),
        request: RefundSyncData {
            connector_transaction_id: "original-payment".to_string(),
            connector_refund_id: id.to_string(),
            reason: Default::default(),
            refund_connector_metadata: Default::default(),
            refund_status: Default::default(),
            all_keys_required: Default::default(),
            integrity_object: Default::default(),
            browser_info: Default::default(),
            split_refunds: Default::default(),
            connector_feature_data: Default::default(),
            refund_money: Default::default(),
            connector_order_id: Default::default(),
        },
        response: Err(ErrorResponse::default()),
    }
}

#[test]
fn authorize_rejections_fail_before_authentication() {
    for code in ["-1906", "-1003", "unknown-code"] {
        let response: ReddotAuthorizeResponse =
            serde_json::from_value(json!({"response_code": code, "response_msg": "Rejected"}))
                .unwrap();
        let result = AuthorizeData::try_from(ResponseRouterData {
            response,
            router_data: authorize_data(),
            http_code: 200,
        })
        .unwrap();
        assert_eq!(result.resource_common_data.status, AttemptStatus::Failure);
        let error = result.response.unwrap_err();
        assert!(matches!(
            error.attempt_status,
            Some(domain_types::router_data::FlowStatus::Payment(
                AttemptStatus::Failure
            ))
        ));
        assert_eq!(error.code, code);
    }
}

#[test]
fn authorize_uses_money_currency_and_stable_order_id_ignoring_incoming_metadata() {
    let data = authorize_data();
    let first = authorize_request(data.clone());
    let mut retry = data;
    retry.resource_common_data.connector_feature_data =
        Some(Secret::new(json!({"reddot_order_id": 123})));
    let retry = authorize_request(retry);
    assert_eq!(first.order_id, retry.order_id);
    assert_eq!(first.order_id.len(), 20);
    assert_eq!(first.ccy, Currency::SGD);
    assert_eq!(first.amount.get_amount_as_string(), "1.10");
    assert_eq!(first.signature.peek(), "435ee25a1e24b09d0a81d1f0e1316f6798368a556d8a2423d0241cf5d6af8bd120d353a4f0a642171d794082ca5b75d8c9102528550f51ad7e3636cdee34b677");
}

#[test]
fn authorize_persists_request_order_id_when_gateway_omits_it() {
    for (code, expected) in [
        (json!(0), AttemptStatus::AuthenticationPending),
        (json!("0"), AttemptStatus::AuthenticationPending),
        (json!("-01"), AttemptStatus::Pending),
    ] {
        let data = authorize_data();
        let order_id = authorize_request(data.clone()).order_id;
        let response: ReddotAuthorizeResponse = serde_json::from_value(json!({
            "response_code": code, "transaction_id": "payment-1", "payment_url": "https://example.com/pay"
        })).unwrap();
        let result = AuthorizeData::try_from(ResponseRouterData {
            response,
            router_data: data,
            http_code: 200,
        })
        .unwrap();
        assert_eq!(result.resource_common_data.status, expected);
        let metadata = match result.response.unwrap() {
            PaymentsResponseData::TransactionResponse {
                connector_metadata, ..
            } => connector_metadata,
            _ => None,
        };
        assert_eq!(
            metadata
                .as_ref()
                .and_then(|metadata| metadata.get("reddot_order_id"))
                .and_then(|value| value.as_str()),
            Some(order_id.as_str())
        );
    }
}

#[test]
fn authorize_rejects_missing_name_and_manual_capture() {
    let mut data = authorize_data();
    if let PaymentMethodData::Card(card) = &mut data.request.payment_method_data {
        card.card_holder_name = None;
    }
    assert!(ReddotAuthorizeRequest::try_from(ReddotRouterData {
        connector: Reddot::<DefaultPCIHolder>::new().clone(),
        router_data: data
    })
    .is_err());
    let mut data = authorize_data();
    data.request.capture_method = Some(CaptureMethod::Manual);
    assert!(ReddotAuthorizeRequest::try_from(ReddotRouterData {
        connector: Reddot::<DefaultPCIHolder>::new().clone(),
        router_data: data
    })
    .is_err());
}

#[test]
fn refund_request_uses_money_currency_and_original_order_id() {
    let mut data = refund_data();
    data.request.refund_amount = Money {
        amount: MinorUnit::new(110),
        currency: Currency::SGD,
    };
    data.request.currency = Currency::JPY;
    data.request.connector_transaction_id = "payment-1".to_string();
    data.request.connector_feature_data =
        Some(Secret::new(json!({"reddot_order_id": "original-order"})));
    let request = ReddotRefundRequest::try_from(ReddotRouterData {
        connector: Reddot::<DefaultPCIHolder>::new().clone(),
        router_data: data,
    })
    .unwrap();
    assert_eq!(request.currency, Currency::SGD);
    assert_eq!(request.amount.get_amount_as_string(), "1.10");
    assert_eq!(request.order_number, "original-order");
    assert_eq!(request.refund_id, "refund-1");
    assert_eq!(request.signature.peek(), "2b01603f19407add1eb03bac703b80e4");
}

#[test]
fn refund_sync_requests_query_each_refund_independently() {
    for id in ["refund-1", "refund-2"] {
        let request = ReddotPSyncRequest::try_from(ReddotRouterData {
            connector: Reddot::<DefaultPCIHolder>::new().clone(),
            router_data: refund_sync_data(id),
        })
        .unwrap();
        assert_eq!(request.transaction_id, id);
    }
}

#[test]
fn refund_sync_handles_observed_full_and_partial_refund_responses() {
    for (id, status, history) in [
        ("refund-full", "voided", None),
        (
            "refund-1",
            "refunded",
            Some(
                json!([{"refund_id": "refund-1", "refund_amount": "0.01", "refund_currency": "SGD", "refund_at": "2026-10-28 13:57:28"}]),
            ),
        ),
        (
            "refund-2",
            "refunded",
            Some(
                json!([{"refund_id": "refund-2", "refund_amount": "0.02", "refund_currency": "SGD", "refund_at": "2026-10-28 13:57:52"}]),
            ),
        ),
    ] {
        let response: ReddotRefundSyncResponse = serde_json::from_value(json!({
            "transaction_id": "original-payment", "transaction_status": status, "authorized_amount": "1.10", "authorized_currency": "SGD", "refund": history
        })).unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: refund_sync_data(id),
            http_code: 200,
        })
        .unwrap();
        let response = result.response.unwrap();
        assert_eq!(response.refund_status, RefundStatus::Success);
        assert_eq!(response.connector_refund_id, id);
    }
}

#[test]
fn refund_sync_preserves_status_on_enquiry_error_and_handles_pending_or_failure() {
    for (status, expected) in [
        ("refund pending", RefundStatus::Pending),
        ("paid", RefundStatus::Pending),
        ("unknown", RefundStatus::Pending),
        ("rejected", RefundStatus::Failure),
    ] {
        let response: ReddotRefundSyncResponse =
            serde_json::from_value(json!({"transaction_status": status})).unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: refund_sync_data("refund-1"),
            http_code: 200,
        })
        .unwrap();
        assert_eq!(result.response.unwrap().refund_status, expected);
    }
    let response: ReddotRefundSyncResponse =
        serde_json::from_value(json!({"response_code": "-1003", "response_status": "error"}))
            .unwrap();
    let mut data = refund_sync_data("refund-1");
    data.resource_common_data.status = RefundStatus::Pending;
    let result = RouterDataV2::try_from(ResponseRouterData {
        response,
        router_data: data,
        http_code: 200,
    })
    .unwrap();
    assert_eq!(result.resource_common_data.status, RefundStatus::Pending);
    assert!(result.response.unwrap_err().attempt_status.is_none());
}

#[test]
fn payment_sync_success_and_pending_preserve_money_and_reference() {
    for (code, expected) in [
        (json!(0), AttemptStatus::Charged),
        (json!("0"), AttemptStatus::Charged),
        (json!("-01"), AttemptStatus::Pending),
    ] {
        let response: ReddotPSyncResponse = serde_json::from_value(json!({"response_code": code, "transaction_id": "payment-1", "authorized_amount": "1.10", "authorized_ccy": "SGD", "merchant_reference": "attempt-1"})).unwrap();
        let result = RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: payment_sync_data(),
            http_code: 200,
        })
        .unwrap();
        assert_eq!(result.resource_common_data.status, expected);
        let integrity = result.request.integrity_object.unwrap();
        assert_eq!(integrity.currency, Currency::SGD);
        assert_eq!(integrity.amount, MinorUnit::new(110));
        if expected == AttemptStatus::Charged {
            assert_eq!(
                result.resource_common_data.amount_captured,
                Some(Money {
                    amount: MinorUnit::new(110),
                    currency: Currency::SGD
                })
            );
        }
    }
}

#[test]
fn cybersource_flag_is_strictly_validated() {
    for (value, expected) in [
        ("TRUE", Some(true)),
        ("false", Some(false)),
        ("true!", None),
        ("", None),
    ] {
        let mut config = config();
        if let ConnectorSpecificConfig::Reddot {
            acquirer_cybersource,
            ..
        } = &mut config
        {
            *acquirer_cybersource = value.to_string();
        }
        match expected {
            Some(expected) => assert_eq!(
                ReddotAuthType::try_from(&config)
                    .unwrap()
                    .acquirer_cybersource,
                expected
            ),
            None => assert!(ReddotAuthType::try_from(&config).is_err()),
        }
    }
}

#[test]
fn webhook_statuses_follow_rdp_mapping_and_are_never_treated_as_verified() {
    use domain_types::connector_types::{EventType, RequestDetails};
    use interfaces::connector_types::IncomingWebhook;
    let connector = Reddot::<DefaultPCIHolder>::new();
    for (code, expected_status, expected_event) in [
        (
            json!(0),
            AttemptStatus::Charged,
            EventType::PaymentIntentSuccess,
        ),
        (
            json!("-01"),
            AttemptStatus::Pending,
            EventType::PaymentIntentProcessing,
        ),
        (
            json!("-1003"),
            AttemptStatus::AuthenticationFailed,
            EventType::PaymentIntentFailure,
        ),
        (
            json!("unknown-code"),
            AttemptStatus::AuthenticationFailed,
            EventType::PaymentIntentFailure,
        ),
    ] {
        let request = RequestDetails {
            method: domain_types::connector_types::HttpMethod::Post, uri: None, headers: Default::default(), query_params: None,
            body: serde_json::to_vec(&json!({"response_code": code, "transaction_id": "payment-1", "merchant_reference": "attempt-1"})).unwrap(),
        };
        assert!(!connector
            .verify_webhook_source(request.clone(), None, None)
            .unwrap());
        assert_eq!(
            connector.get_event_type(request.clone()).unwrap(),
            expected_event
        );
        let response = connector
            .process_payment_webhook(request, None, None, None)
            .unwrap();
        assert_eq!(response.status, expected_status);
        assert_eq!(
            response.connector_response_reference_id.as_deref(),
            Some("attempt-1")
        );
    }
}
