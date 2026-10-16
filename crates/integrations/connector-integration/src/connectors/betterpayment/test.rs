#![allow(clippy::expect_used, clippy::indexing_slicing)]

use common_enums::{CountryAlpha2, Currency, RefundStatus};
use common_utils::types::{AmountConvertor, MinorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_types::{EventType, RequestDetails},
    payment_address::{Address, AddressDetails, PhoneDetails},
    payment_method_data::DefaultPCIHolder,
    router_data::ConnectorSpecificConfig,
};
use hyperswitch_masking::Secret;
use interfaces::{api::ConnectorCommon, connector_types::IncomingWebhook};
use serde_json::json;

use super::{transformers::*, Betterpayment};

fn config() -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Betterpayment {
        api_key: Secret::new("api-user".to_owned()),
        api_password: Secret::new("api-password".to_owned()),
        webhook_key: Secret::new("key".to_owned()),
        base_url: None,
    }
}

#[test]
fn payment_and_refund_amounts_are_major_unit_strings() {
    for (minor, expected) in [(1, "0.01"), (1000, "10.00"), (1590, "15.90")] {
        let amount = StringMajorUnitForConnector
            .convert(MinorUnit::new(minor), Currency::EUR)
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
        let mut payload = json!({
            "transaction_id": "payment-123", "order_id": "order-123",
            "status": "refunded", "refund_id": "refund-123"
        });
        if let Some(code) = code {
            payload["refund_status_code"] = json!(code);
        }
        let body: BetterpaymentWebhookBody =
            serde_json::from_value(payload).expect("parse refund webhook");
        assert_eq!(get_webhook_event_type(&body), expected_event);
        assert_eq!(
            get_refund_webhook_status(body.refund_status_code).expect("map refund status"),
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
        *webhook_key = Secret::new("wrong-key".to_owned());
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
