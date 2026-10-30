#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use common_enums::{CountryAlpha2, Currency, RefundStatus};
use common_utils::types::{AmountConvertor, MinorUnit, Money, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync},
    connector_types::{
        EventType, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsResponseData, RequestDetails, ResponseId,
    },
    payment_address::{Address, AddressDetails, PhoneDetails},
    payment_method_data::DefaultPCIHolder,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use hyperswitch_masking::Secret;
use interfaces::{api::ConnectorCommon, connector_types::IncomingWebhook};
use serde_json::json;

use super::{transformers::*, Betterpayment};
use crate::types::ResponseRouterData;

fn config() -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Betterpayment {
        api_key: Secret::new("api-user".to_owned()),
        api_password: Secret::new("api-password".to_owned()),
        webhook_key: Some(Secret::new("key".to_owned())),
        base_url: None,
    }
}

#[test]
fn payment_and_refund_amounts_are_major_unit_strings() {
    for (minor, expected) in [(1, "0.01"), (1000, "10.00"), (1590, "15.90")] {
        let amount = StringMajorUnitForConnector
            .convert(&Money {
                amount: MinorUnit::new(minor),
                currency: Currency::EUR,
            })
            .expect("convert EUR amount");
        let payment = BetterpaymentAuthorizeRequest {
            payment_type: BetterpaymentPaymentType::Wero,
            order_id: "order-123".to_owned(),
            amount: amount.clone(),
            currency: Currency::EUR,
            merchant_reference: None,
            postback_url: Some("https://example.com/webhook".to_owned()),
            success_url: Some("https://example.com/return".to_owned()),
            error_url: Some("https://example.com/return".to_owned()),
            wero_checkout_url: Some("https://example.com/checkout".to_owned()),
            billing: None,
        };
        let payment = serde_json::to_value(payment).expect("serialize payment");
        assert_eq!(payment["amount"], json!(expected));
        assert_eq!(payment["currency"], json!("EUR"));
        assert_eq!(payment["payment_type"], json!("wero"));
        assert!(payment.get("billing").is_none());
        assert!(payment.get("address").is_none());
        let refund = BetterpaymentRefundRequest {
            transaction_id: "original-transaction".to_owned(),
            amount,
        };
        assert_eq!(
            serde_json::to_value(refund).expect("serialize refund"),
            json!({"transaction_id": "original-transaction", "amount": expected})
        );
    }
}

#[test]
fn billing_maps_optional_contact_and_address_fields() {
    let mut billing = Address {
        address: Some(AddressDetails {
            line1: Some(Secret::new("Street 1".to_owned())),
            line2: Some(Secret::new("Apartment 2".to_owned())),
            city: Some(Secret::new("Berlin".to_owned())),
            zip: Some(Secret::new("10178".to_owned())),
            country: Some(CountryAlpha2::DE),
            state: Some(Secret::new("Berlin".to_owned())),
            first_name: Some(Secret::new("Jane".to_owned())),
            last_name: Some(Secret::new("Doe".to_owned())),
            ..Default::default()
        }),
        email: Some("jane@example.com".parse().expect("valid email")),
        phone: Some(PhoneDetails {
            country_code: Some("49".to_owned()),
            number: Some(Secret::new("301234567".to_owned())),
        }),
    };
    let mapped = BetterpaymentBillingAddress::try_from(&billing).expect("valid billing");
    let mapped = serde_json::to_value(mapped).expect("serialize billing");
    assert_eq!(mapped["email"], json!("jane@example.com"));
    assert_eq!(mapped["phone"], json!("+49301234567"));
    assert_eq!(mapped["address2"], json!("Apartment 2"));
    assert_eq!(mapped["country"], json!("DE"));

    billing.email = None;
    billing.phone = None;
    let address = billing.address.as_mut().expect("billing address");
    address.line2 = None;
    address.state = None;
    let mapped = serde_json::to_value(
        BetterpaymentBillingAddress::try_from(&billing).expect("optional fields omitted"),
    )
    .expect("serialize billing");
    for field in ["email", "phone", "address2", "state"] {
        assert!(mapped.get(field).is_none(), "unexpected field: {field}");
    }
    billing.address.as_mut().expect("billing address").zip = None;
    assert!(BetterpaymentBillingAddress::try_from(&billing).is_err());
}

#[test]
fn refund_webhooks_map_documented_missing_and_future_codes() {
    for (code, expected_status, expected_event) in [
        (Some(0), RefundStatus::Pending, EventType::RefundProcessing),
        (Some(1), RefundStatus::Success, EventType::RefundSuccess),
        (Some(2), RefundStatus::Failure, EventType::RefundFailure),
        (Some(3), RefundStatus::Pending, EventType::RefundProcessing),
        (Some(4), RefundStatus::Failure, EventType::RefundFailure),
        (Some(5), RefundStatus::Failure, EventType::RefundFailure),
        (
            Some(999),
            RefundStatus::Pending,
            EventType::RefundProcessing,
        ),
        (None, RefundStatus::Pending, EventType::RefundProcessing),
    ] {
        let mut payload =
            "transaction_id=payment-123&order_id=order-123&status=refunded&refund_id=refund-123"
                .to_owned();
        if let Some(code) = code {
            payload.push_str(&format!("&refund_status_code={code}"));
        }
        let body: BetterpaymentWebhookBody =
            serde_urlencoded::from_bytes(payload.as_bytes()).expect("parse refund form webhook");
        assert_eq!(get_webhook_event_type(&body), expected_event);
        assert_eq!(
            body.refund_status_code
                .map(RefundStatus::from)
                .unwrap_or_default(),
            expected_status
        );
    }
}

#[test]
fn renamed_credentials_keep_basic_auth_separate_from_webhook_key() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let headers = connector.get_auth_header(&config()).expect("Basic auth");
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "Authorization");
    assert_eq!(
        headers[0].1.clone().into_inner(),
        "Basic YXBpLXVzZXI6YXBpLXBhc3N3b3Jk"
    );
}

#[test]
fn webhook_signature_rejects_tampered_body_wrong_key_and_invalid_headers() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    // Independent published HMAC-SHA256 vector: key="key", message below.
    let mut request = RequestDetails {
        method: domain_types::connector_types::HttpMethod::Post,
        uri: None,
        headers: std::collections::HashMap::from([(
            "gateway-webhook-signature".to_owned(),
            "97yD9DBThCSxMpjmqm+xQ+9NWaFJRhdZl0edvC0aPNg=".to_owned(),
        )]),
        body: b"The quick brown fox jumps over the lazy dog".to_vec(),
        query_params: None,
    };
    assert!(connector
        .verify_webhook_source(request.clone(), None, Some(config()))
        .expect("verify valid signature"));
    let mut wrong_key = config();
    if let ConnectorSpecificConfig::Betterpayment { webhook_key, .. } = &mut wrong_key {
        *webhook_key = Some(Secret::new("wrong-key".to_owned()));
    }
    assert!(!connector
        .verify_webhook_source(request.clone(), None, Some(wrong_key))
        .expect("reject wrong key"));
    request.body.push(b'!');
    assert!(!connector
        .verify_webhook_source(request.clone(), None, Some(config()))
        .expect("reject tampered body"));
    request.headers.insert(
        "gateway-webhook-signature".to_owned(),
        "not base64!".to_owned(),
    );
    assert!(connector
        .verify_webhook_source(request.clone(), None, Some(config()))
        .is_err());
    request.headers.clear();
    assert!(connector
        .verify_webhook_source(request, None, Some(config()))
        .is_err());
}

#[test]
fn refund_webhook_rejects_missing_or_empty_status_code() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    for suffix in ["", "&refund_status_code="] {
        let request = form_webhook(&format!(
            "transaction_id=payment-123&order_id=order-123&status=refunded&refund_id=refund-123{suffix}"
        ));
        let error = connector
            .process_refund_webhook(request, None, None)
            .expect_err("missing refund status must fail");
        assert!(matches!(
            error.current_context(),
            domain_types::errors::WebhookError::WebhookBodyDecodingFailed
        ));
    }
}

fn form_webhook(body: &str) -> RequestDetails {
    RequestDetails {
        method: domain_types::connector_types::HttpMethod::Post,
        uri: None,
        headers: std::collections::HashMap::from([(
            "content-type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        )]),
        body: body.as_bytes().to_vec(),
        query_params: None,
    }
}

#[test]
fn captured_wero_form_postback_decodes_and_routes() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let raw = "transaction_id=f585d8d3-93c0-4e1a-9021-30611639674e&payment_type=wero&status_code=5&status=canceled&order_id=34533343&message=CONSENT%20DENIED&additional_transaction_data=customer_id%3D123ABC%2C%20billing_info%3D12345678&amount=15.9&recurring=0&included_fees=0.00&checksum=bc2423a3f1c94d1f49c3af563a3f16726f483a63";
    let request = form_webhook(raw);
    let parsed: BetterpaymentWebhookBody =
        serde_urlencoded::from_bytes(&request.body).expect("decode captured form");
    assert_eq!(parsed.message.as_deref(), Some("CONSENT DENIED"));
    assert_eq!(
        connector.get_event_type(request.clone()).expect("event"),
        EventType::PaymentIntentFailure
    );
    match connector
        .get_webhook_event_reference(request.clone())
        .expect("reference")
        .expect("present")
    {
        domain_types::connector_types::WebhookResourceReference::Payment(reference) => {
            assert!(reference.merchant_transaction_id.is_none());
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("f585d8d3-93c0-4e1a-9021-30611639674e")
            );
        }
        _ => panic!("expected payment reference"),
    }
    let response = connector
        .process_payment_webhook(request, None, None, None)
        .expect("payment webhook");
    assert_eq!(response.status, common_enums::AttemptStatus::Failure);
    assert_eq!(response.error_message.as_deref(), Some("CONSENT DENIED"));
    assert_eq!(response.raw_connector_response.as_deref(), Some(raw));
    assert!(response.amount_captured.is_none());
}

#[test]
fn form_refund_postbacks_decode_numeric_codes_and_route() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    for (code, expected) in [
        (0, RefundStatus::Pending),
        (1, RefundStatus::Success),
        (2, RefundStatus::Failure),
        (999, RefundStatus::Pending),
    ] {
        let request = form_webhook(&format!("transaction_id=payment123&order_id=order123&status=refunded&refund_id=refund123&refund_status_code={code}"));
        let response = connector
            .process_refund_webhook(request.clone(), None, None)
            .expect("refund form");
        assert_eq!(response.status, expected);
        assert_eq!(response.connector_refund_id.as_deref(), Some("refund123"));
        assert!(matches!(
            connector
                .get_webhook_event_reference(request.clone())
                .expect("refund reference"),
            Some(domain_types::connector_types::WebhookResourceReference::Refund(_))
        ));
        assert!(connector.get_event_type(request).is_ok());
    }
    for suffix in ["", "&refund_status_code=-1", "&refund_status_code=bad"] {
        let request = form_webhook(&format!(
            "transaction_id=p&order_id=o&status=refunded&refund_id=r{suffix}"
        ));
        assert!(connector
            .process_refund_webhook(request, None, None)
            .is_err());
    }
}

#[test]
fn charged_form_webhook_without_currency_does_not_guess_captured_amount() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let request = form_webhook("transaction_id=p&order_id=o&status=completed&amount=15.9");
    let response = connector
        .process_payment_webhook(request, None, None, None)
        .expect("completed form");
    assert_eq!(response.status, common_enums::AttemptStatus::Charged);
    assert!(response.amount_captured.is_none());
}

#[test]
fn form_signature_is_verified_on_original_encoded_bytes() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let mut request = form_webhook("id=123&name=John%20Doe");
    request.headers.insert(
        "gateway-webhook-signature".to_owned(),
        "bsixvztdAkDkFFcfrKTSHfv0lv+Mu7BQCg6IZKwBSBQ=".to_owned(),
    );
    let mut auth = config();
    if let ConnectorSpecificConfig::Betterpayment { webhook_key, .. } = &mut auth {
        *webhook_key = Some(Secret::new("c54b7f22bc69f87b731c".to_owned()));
    }
    assert!(connector
        .verify_webhook_source(request.clone(), None, Some(auth.clone()))
        .expect("form signature"));
    request.body = b"id=123&name=John+Doe".to_vec();
    assert!(!connector
        .verify_webhook_source(request, None, Some(auth))
        .expect("changed encoding invalidates signature"));
}

#[test]
fn malformed_and_json_webhook_bodies_fail() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    for raw in [
        "",
        "status=completed",
        "{invalid-json",
        r#"{"transaction_id":"p","order_id":"o","status":"completed"}"#,
    ] {
        let request = form_webhook(raw);
        assert!(connector.get_event_type(request.clone()).is_err());
        assert!(connector
            .get_webhook_event_reference(request.clone())
            .is_err());
        assert!(connector
            .process_payment_webhook(request.clone(), None, None, None)
            .is_err());
        assert!(connector
            .process_refund_webhook(request, None, None)
            .is_err());
    }
}

fn payment_flow() -> PaymentFlowData {
    PaymentFlowData {
        merchant_id: Default::default(),
        customer_id: None,
        connector_customer: None,
        payment_id: Default::default(),
        attempt_id: Default::default(),
        status: common_enums::AttemptStatus::Pending,
        payment_method: common_enums::PaymentMethod::Wallet,
        payment_method_type: None,
        description: None,
        return_url: None,
        address: Default::default(),
        auth_type: Default::default(),
        connector_feature_data: None,
        amount_captured: None,
        amount_capturable: None,
        amount: None,
        access_token: None,
        session_token: None,
        reference_id: None,
        connector_order_id: None,
        preprocessing_id: None,
        connector_api_version: None,
        connector_request_reference_id: Default::default(),
        test_mode: None,
        connector_http_status_code: None,
        connector_response_headers: None,
        external_latency: None,
        connectors: domain_types::types::Connectors::default().into(),
        raw_connector_response: None,
        typed_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
        vault_headers: None,
        connector_response: None,
        recurring_mandate_payment_data: None,
        order_details: None,
        amount_authorized: None,
        l2_l3_data: None,
        merchant_request_id: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
        settlement_status: None,
        raw_connector_status: None,
    }
}

fn authorize_request() -> PaymentsAuthorizeData<DefaultPCIHolder> {
    PaymentsAuthorizeData {
        payment_method_data: domain_types::payment_method_data::PaymentMethodData::Wallet(
            domain_types::payment_method_data::WalletData::Wero(
                domain_types::payment_method_data::WeroData {},
            ),
        ),
        amount: Default::default(),
        order_tax_amount: None,
        surcharge_amount: None,
        email: None,
        customer_document_details: None,
        customer_date_of_birth: None,
        customer_name: None,
        currency: Currency::EUR,
        confirm: Default::default(),
        billing_descriptor: None,
        capture_method: None,
        router_return_url: None,
        webhook_url: None,
        complete_authorize_url: None,
        mandate_id: None,
        setup_future_usage: None,
        off_session: None,
        browser_info: None,
        order_category: None,
        session_token: None,
        access_token: None,
        customer_acceptance: None,
        enrolled_for_3ds: None,
        related_transaction_id: None,
        payment_experience: None,
        payment_method_type: None,
        customer_id: None,
        request_incremental_authorization: None,
        metadata: None,
        authentication_data: None,
        split_payments: None,
        split_settlement: None,
        merchant_order_id: None,
        shipping_cost: None,
        merchant_account_id: None,
        integrity_object: None,
        integrity_specifications: None,
        merchant_config_currency: None,
        all_keys_required: None,
        request_extended_authorization: None,
        enable_overcapture: None,
        setup_mandate_details: None,
        connector_feature_data: None,
        connector_testing_data: None,
        payment_channel: None,
        enable_partial_authorization: None,
        locale: None,
        redirect_response: None,
        threeds_method_comp_ind: None,
        continue_redirection_url: None,
        tokenization: None,
        mit_category: None,
        domain_data: None,
        risk_assessment_data: None,
        partner_merchant_identifier_details: None,
        currency_conversion_data: None,
        is_account_funding_transaction: None,
        recipient_details: None,
        additional_connector_details: None,
        customer: None,
        business_country: None,
        enable_avs_check: None,
    }
}

fn refund_flow() -> RefundFlowData {
    RefundFlowData {
        merchant_id: Default::default(),
        status: RefundStatus::Pending,
        refund_id: None,
        connectors: domain_types::types::Connectors::default().into(),
        connector_request_reference_id: Default::default(),
        raw_connector_response: None,
        typed_connector_response: None,
        connector_response_headers: None,
        raw_connector_request: None,
        typed_connector_request: None,
        access_token: None,
        connector_feature_data: None,
        test_mode: None,
        payment_method: None,
        merchant_request_id: None,
        raw_connector_status: None,
    }
}

fn payment_router<F, Req>(
    request: Req,
) -> RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData> {
    RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: payment_flow(),
        connector_config: config(),
        request,
        response: Err(Default::default()),
    }
}

type AuthorizeRouter = RouterDataV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<DefaultPCIHolder>,
    PaymentsResponseData,
>;
type SyncRouter = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;
type RefundSyncRouter = RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>;

#[test]
fn authorize_redirect_response_preserves_transaction_and_redirect() {
    let response = serde_json::from_value::<BetterpaymentAuthorizeResponse>(json!({
        "transaction_id": "payment-123", "order_id": "order-123", "status": "started",
        "error_code": 0, "client_action": "redirect",
        "action_data": {"url": "https://checkout.example/wero?token=123&return_url=https%3A%2F%2Fmerchant.example%2Freturn%3Fa%3D1%26b%3D2"}
    }))
    .expect("authorize redirect fixture");
    let result = AuthorizeRouter::try_from(ResponseRouterData {
        response,
        router_data: payment_router(authorize_request()),
        http_code: 200,
    })
    .expect("transform authorize redirect");
    assert_eq!(
        result.resource_common_data.status,
        common_enums::AttemptStatus::AuthenticationPending
    );
    match result.response.expect("successful authorize") {
        PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data,
            connector_response_reference_id,
            status_code,
            ..
        } => {
            assert!(
                matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == "payment-123")
            );
            assert_eq!(
                connector_response_reference_id.as_deref(),
                Some("order-123")
            );
            assert_eq!(status_code, 200);
            match *redirection_data.expect("redirect required") {
                RedirectForm::Form {
                    endpoint,
                    method,
                    form_fields,
                } => {
                    assert_eq!(endpoint, "https://checkout.example/wero");
                    assert_eq!(method, common_utils::request::Method::Get);
                    assert_eq!(form_fields.get("token").map(String::as_str), Some("123"));
                    assert_eq!(
                        form_fields.get("return_url").map(String::as_str),
                        Some("https://merchant.example/return?a=1&b=2")
                    );
                }
                _ => panic!("expected GET redirect"),
            }
        }
        _ => panic!("expected transaction response"),
    }
}

#[test]
fn authorize_in_band_error_is_failure_even_on_http_200() {
    for fixture in [
        json!({"error_code": 42, "error_message": "Invalid order"}),
        json!({"error_code": 42, "message": "Invalid order", "error_message": "fallback", "status": "completed", "transaction_id": "payment-123"}),
    ] {
        let expected_id = fixture
            .get("transaction_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let result = AuthorizeRouter::try_from(ResponseRouterData {
            response: serde_json::from_value::<BetterpaymentAuthorizeResponse>(fixture)
                .expect("authorize error fixture"),
            router_data: payment_router(authorize_request()),
            http_code: 200,
        })
        .expect("transform in-band error");
        assert_eq!(
            result.resource_common_data.status,
            common_enums::AttemptStatus::Failure
        );
        let error = result.response.expect_err("in-band error must not succeed");
        assert_eq!(error.code, "42");
        assert_eq!(error.message, "Invalid order");
        assert_eq!(error.reason.as_deref(), Some("Invalid order"));
        assert_eq!(error.status_code, 200);
        assert_eq!(error.connector_transaction_id, expected_id);
        assert!(matches!(
            error.attempt_status,
            Some(domain_types::router_data::FlowStatus::Payment(
                common_enums::AttemptStatus::Failure
            ))
        ));
    }
}

#[test]
fn psync_response_fixtures_map_all_payment_statuses() {
    use common_enums::AttemptStatus;
    for (status, expected, failure) in [
        ("started", AttemptStatus::AuthenticationPending, false),
        ("pending", AttemptStatus::Pending, false),
        ("completed", AttemptStatus::Charged, false),
        ("declined", AttemptStatus::Failure, true),
        ("canceled", AttemptStatus::Failure, true),
        ("error", AttemptStatus::Failure, true),
        ("future_status", AttemptStatus::Pending, false),
    ] {
        let result = SyncRouter::try_from(ResponseRouterData {
            response: serde_json::from_value::<BetterpaymentPSyncResponse>(json!({
                "transaction_id": "payment-123", "order_id": "order-123", "status": status,
                "error_code": if failure { 17 } else { 0 }, "message": "Payment declined"
            }))
            .expect("PSync fixture"),
            router_data: payment_router(PaymentsSyncData::default()),
            http_code: 200,
        })
        .expect("transform PSync");
        assert_eq!(result.resource_common_data.status, expected, "{status}");
        if failure {
            let error = result.response.expect_err("failed payment");
            assert_eq!(error.code, "17");
            assert_eq!(error.message, "Payment declined");
            assert_eq!(
                error.connector_transaction_id.as_deref(),
                Some("payment-123")
            );
        } else {
            match result.response.expect("successful sync") {
                PaymentsResponseData::TransactionResponse {
                    resource_id,
                    redirection_data,
                    connector_response_reference_id,
                    ..
                } => {
                    assert!(
                        matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == "payment-123")
                    );
                    assert_eq!(
                        connector_response_reference_id.as_deref(),
                        Some("order-123")
                    );
                    assert!(redirection_data.is_none());
                }
                _ => panic!("expected transaction response"),
            }
        }
    }
}

#[test]
fn rsync_response_selects_requested_refund_and_rejects_missing_id() {
    let fixture = json!([
        {"transaction_id": "payment-123", "refund_id": "refund-first", "status": "error", "error_code": 9},
        {"transaction_id": "payment-123", "refund_id": "refund-target", "status": "successful"},
        {"transaction_id": "payment-123", "refund_id": "refund-last", "status": "started"}
    ]);
    for (id, expected) in [
        ("refund-target", RefundStatus::Success),
        ("refund-first", RefundStatus::Failure),
        ("refund-last", RefundStatus::Pending),
    ] {
        let result = RefundSyncRouter::try_from(ResponseRouterData {
            response: serde_json::from_value::<BetterpaymentRSyncResponse>(fixture.clone())
                .expect("RSync list fixture"),
            router_data: RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: refund_flow(),
                connector_config: config(),
                request: RefundSyncData {
                    connector_refund_id: id.to_owned(),
                    ..Default::default()
                },
                response: Err(Default::default()),
            },
            http_code: 200,
        })
        .expect("transform matching refund");
        assert_eq!(result.resource_common_data.status, expected);
        if expected == RefundStatus::Failure {
            assert_eq!(result.response.expect_err("failed refund").code, "9");
        } else {
            let response = result.response.expect("refund response");
            assert_eq!(response.connector_refund_id, id);
            assert_eq!(response.refund_status, expected);
            assert_eq!(response.status_code, 200);
        }
    }
    for id in ["refund-absent", ""] {
        let result = RefundSyncRouter::try_from(ResponseRouterData {
            response: serde_json::from_value::<BetterpaymentRSyncResponse>(fixture.clone())
                .expect("RSync list fixture"),
            router_data: RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: refund_flow(),
                connector_config: config(),
                request: RefundSyncData {
                    connector_refund_id: id.to_owned(),
                    ..Default::default()
                },
                response: Err(Default::default()),
            },
            http_code: 200,
        });
        assert!(matches!(
            result
                .expect_err("must not select unrelated refund")
                .current_context(),
            domain_types::errors::ConnectorError::UnexpectedResponseError { .. }
        ));
    }
}

#[test]
fn canceled_webhook_event_and_status_both_report_failure() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let request = form_webhook("transaction_id=p&order_id=o&status=canceled");
    assert_eq!(
        connector
            .get_event_type(request.clone())
            .expect("canceled event"),
        EventType::PaymentIntentFailure
    );
    assert_eq!(
        connector
            .process_payment_webhook(request, None, None, None)
            .expect("canceled webhook")
            .status,
        common_enums::AttemptStatus::Failure
    );
}

#[test]
fn sanitized_webhook_order_id_is_not_returned_as_merchant_reference() {
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    // Both pay_ab_1 and pay_a_b1 sanitize to payab1; neither can be recovered.
    let payment = form_webhook("transaction_id=payment-123&order_id=payab1&status=completed");
    match connector
        .get_webhook_event_reference(payment.clone())
        .expect("payment reference")
        .expect("present")
    {
        domain_types::connector_types::WebhookResourceReference::Payment(reference) => {
            assert!(reference.merchant_transaction_id.is_none());
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("payment-123")
            );
        }
        _ => panic!("expected payment reference"),
    }
    let response = connector
        .process_payment_webhook(payment, None, None, None)
        .expect("payment webhook");
    assert_eq!(
        response.connector_request_reference_id.as_deref(),
        Some("payab1")
    );
    assert!(
        matches!(response.resource_id, Some(ResponseId::ConnectorTransactionId(id)) if id == "payment-123")
    );

    let refund = form_webhook("transaction_id=payment-123&order_id=payab1&status=refunded&refund_id=refund-123&refund_status_code=1");
    match connector
        .get_webhook_event_reference(refund.clone())
        .expect("refund reference")
        .expect("present")
    {
        domain_types::connector_types::WebhookResourceReference::Refund(reference) => {
            assert!(reference.merchant_transaction_id.is_none());
            assert!(reference.merchant_refund_id.is_none());
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("payment-123")
            );
            assert_eq!(reference.connector_refund_id.as_deref(), Some("refund-123"));
        }
        _ => panic!("expected refund reference"),
    }
    let response = connector
        .process_refund_webhook(refund, None, None)
        .expect("refund webhook");
    assert!(response.merchant_transaction_id.is_none());
    assert_eq!(response.connector_refund_id.as_deref(), Some("refund-123"));
}

#[test]
fn psync_in_band_errors_map_to_failure_without_status_or_transaction_id() {
    for fixture in [
        json!({"error_code": 42, "message": "Invalid transaction"}),
        json!({"error_code": 42, "error_message": "Invalid transaction"}),
        json!({"error_code": 42, "message": "Invalid transaction", "error_message": "fallback", "status": "completed", "transaction_id": "payment-123"}),
    ] {
        let expected_id = fixture
            .get("transaction_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let result = SyncRouter::try_from(ResponseRouterData {
            response: serde_json::from_value::<BetterpaymentPSyncResponse>(fixture)
                .expect("PSync in-band error fixture"),
            router_data: payment_router(PaymentsSyncData::default()),
            http_code: 200,
        })
        .expect("transform PSync in-band error");
        assert_eq!(
            result.resource_common_data.status,
            common_enums::AttemptStatus::Failure
        );
        let error = result.response.expect_err("in-band error must fail");
        assert_eq!(error.code, "42");
        assert_eq!(error.message, "Invalid transaction");
        assert_eq!(error.reason.as_deref(), Some("Invalid transaction"));
        assert_eq!(error.status_code, 200);
        assert_eq!(error.connector_transaction_id, expected_id);
        assert!(matches!(
            error.attempt_status,
            Some(domain_types::router_data::FlowStatus::Payment(
                common_enums::AttemptStatus::Failure
            ))
        ));
    }
}

#[test]
fn authorize_invalid_redirect_url_returns_transformation_error() {
    let result = AuthorizeRouter::try_from(ResponseRouterData {
        response: serde_json::from_value::<BetterpaymentAuthorizeResponse>(json!({
            "transaction_id": "payment-123", "status": "started", "client_action": "redirect",
            "action_data": {"url": "not a URL"}
        }))
        .expect("invalid redirect fixture"),
        router_data: payment_router(authorize_request()),
        http_code: 200,
    });
    assert!(matches!(
        result
            .expect_err("invalid redirect must fail")
            .current_context(),
        domain_types::errors::ConnectorError::ResponseDeserializationFailed { .. }
    ));
}

#[test]
fn api_credentials_without_webhook_key_are_accepted_but_verification_requires_it() {
    use domain_types::utils::ForeignTryFrom;
    let grpc_config = grpc_api_types::payments::ConnectorSpecificConfig {
        config: Some(
            grpc_api_types::payments::connector_specific_config::Config::Betterpayment(
                grpc_api_types::payments::BetterpaymentConfig {
                    api_key: Some(Secret::new("api-user".to_owned())),
                    api_password: Some(Secret::new("api-password".to_owned())),
                    webhook_key: None,
                    base_url: None,
                },
            ),
        ),
    };
    let auth = ConnectorSpecificConfig::foreign_try_from(grpc_config)
        .expect("API credentials are sufficient");
    let connector = Betterpayment::<DefaultPCIHolder>::new();
    let headers = connector
        .get_auth_header(&auth)
        .expect("Basic auth without webhook key");
    assert_eq!(
        headers[0].1.clone().into_inner(),
        "Basic YXBpLXVzZXI6YXBpLXBhc3N3b3Jk"
    );
    let error = connector
        .verify_webhook_source(
            form_webhook("transaction_id=p&order_id=o&status=completed"),
            None,
            Some(auth),
        )
        .expect_err("verification must require a webhook key");
    assert!(matches!(
        error.current_context(),
        domain_types::errors::WebhookError::WebhookVerificationSecretNotFound
    ));
}
