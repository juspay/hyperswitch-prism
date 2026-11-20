//! Unit tests for the Checkout AFT `sender` object.

use domain_types::{
    connector_types::{
        CorporateSenderDetails, CustomerInfo, IndividualSenderDetails, PaymentFlowData,
        SenderAddress, SenderDetails,
    },
    errors::IntegrationError,
    payment_address::{Address, AddressDetails, PaymentAddress, PhoneDetails},
    types::Connectors,
};
use hyperswitch_masking::Secret;
use serde_json::{json, Value};

use super::transformers::{build_checkout_sender, build_checkout_sender_address};
use super::Checkout;
use common_enums::PayoutStatus;
use domain_types::{
    connector_types::{EventType, HttpMethod, RequestDetails, WebhookResourceReference},
    payment_method_data::DefaultPCIHolder,
};
use interfaces::webhooks::IncomingWebhook;
use std::collections::HashMap;

fn secret(value: &str) -> Secret<String> {
    Secret::new(value.to_string())
}

/// A complete US billing address (Jane Doe, New York) with a phone number.
fn billing_address() -> Address {
    Address {
        address: Some(AddressDetails {
            first_name: Some(secret("Jane")),
            last_name: Some(secret("Doe")),
            line1: Some(secret("1 Billing St")),
            line2: None,
            line3: None,
            city: Some(secret("New York")),
            state: Some(domain_types::payment_address::StateDetails::from_code(
                secret("NY"),
            )),
            zip: Some(secret("10001")),
            country: Some(common_enums::CountryAlpha2::US),
            origin_zip: None,
        }),
        phone: Some(PhoneDetails {
            number: Some(secret("2125550100")),
            country_code: Some("+1".to_string()),
        }),
        email: None,
    }
}

/// Payment flow data whose only meaningful content is the given billing address.
fn payment_flow_data(billing: Option<Address>) -> PaymentFlowData {
    PaymentFlowData {
        merchant_id: common_utils::id_type::MerchantId::default(),
        customer_id: None,
        connector_customer: None,
        payment_id: "pay_test".to_string(),
        attempt_id: "attempt_test".to_string(),
        status: common_enums::AttemptStatus::Pending,
        payment_method: common_enums::PaymentMethod::Card,
        payment_method_type: None,
        description: None,
        return_url: None,
        address: PaymentAddress::new(None, billing, None, None),
        auth_type: common_enums::AuthenticationType::NoThreeDs,
        connector_feature_data: None,
        captured_money: None,
        capturable_money: None,
        money: None,
        access_token: None,
        session_token: None,
        reference_id: None,
        connector_order_id: None,
        preprocessing_id: None,
        connector_api_version: None,
        connector_request_reference_id: "ref_test".to_string(),
        test_mode: None,
        connector_http_status_code: None,
        connector_response_headers: None,
        external_latency: None,
        connectors: Connectors::default().into(),
        raw_connector_response: None,
        typed_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
        vault_headers: None,
        connector_response: None,
        recurring_mandate_payment_data: None,
        order_details: None,
        authorized_money: None,
        l2_l3_data: None,
        merchant_request_id: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
        settlement_status: None,
        raw_connector_status: None,
    }
}

fn sender_address() -> SenderAddress {
    SenderAddress {
        line1: Some(secret("10 Downing St")),
        line2: Some(secret("Flat 2")),
        line3: None,
        city: Some(secret("London")),
        state: None,
        zip: Some(secret("SW1A 2AA")),
        country: Some(common_enums::CountryAlpha2::GB),
    }
}

fn customer(date_of_birth: Option<time::Date>) -> CustomerInfo {
    CustomerInfo {
        customer_id: None,
        customer_email: None,
        customer_name: None,
        first_name: None,
        last_name: None,
        customer_phone_number: None,
        customer_phone_country_code: None,
        salutation: None,
        date_of_birth: date_of_birth.map(Secret::new),
    }
}

fn individual(date_of_birth: Option<time::Date>) -> SenderDetails {
    SenderDetails::Individual(individual_details(date_of_birth))
}

fn build(
    sender_details: Option<&SenderDetails>,
    customer: Option<&CustomerInfo>,
) -> Result<Value, error_stack::Report<IntegrationError>> {
    build_checkout_sender(
        &payment_flow_data(Some(billing_address())),
        sender_details,
        customer,
    )
    .map(|sender| serde_json::to_value(sender).expect("sender serializes"))
}

fn missing_field(error: &error_stack::Report<IntegrationError>) -> &'static str {
    match error.current_context() {
        IntegrationError::MissingRequiredField { field_name, .. } => field_name,
        other => panic!("expected a missing field error, got {other:?}"),
    }
}

#[test]
fn individual_from_sender_details() {
    let sender = individual(Some(time::macros::date!(1985 - 04 - 12)));
    let customer = customer(Some(time::macros::date!(1970 - 01 - 01)));
    assert_eq!(
        build(Some(&sender), Some(&customer)).expect("sender builds"),
        json!({
            "type": "individual",
            "first_name": "John",
            "last_name": "Smith",
            "date_of_birth": "1985-04-12",
            "address": {
                "address_line1": "10 Downing St",
                "address_line2": "Flat 2",
                "city": "London",
                "zip": "SW1A 2AA",
                "country": "GB"
            }
        })
    );
}

#[test]
fn individual_date_of_birth_falls_back_to_customer() {
    let sender = individual(None);
    let customer = customer(Some(time::macros::date!(1970 - 01 - 01)));
    let value = build(Some(&sender), Some(&customer)).expect("sender builds");
    assert_eq!(value.get("date_of_birth"), Some(&json!("1970-01-01")));

    let error = build(Some(&sender), None).expect_err("date of birth is required");
    assert_eq!(missing_field(&error), "sender_details.date_of_birth");
}

#[test]
fn corporate_from_sender_details() {
    let sender = SenderDetails::Corporate(CorporateSenderDetails {
        company_name: Some(secret("Example Trading Ltd")),
        address: Some(sender_address()),
        email: None,
        phone_number: None,
        phone_country_code: None,
    });
    // A corporate sender has no date of birth, so the customer's is neither needed nor sent.
    assert_eq!(
        build(Some(&sender), None).expect("sender builds"),
        json!({
            "type": "corporate",
            "company_name": "Example Trading Ltd",
            "address": {
                "address_line1": "10 Downing St",
                "address_line2": "Flat 2",
                "city": "London",
                "zip": "SW1A 2AA",
                "country": "GB"
            }
        })
    );
}

#[test]
fn sender_details_require_an_address() {
    let sender = SenderDetails::Individual(IndividualSenderDetails {
        first_name: Some(secret("John")),
        last_name: Some(secret("Smith")),
        date_of_birth: Some(Secret::new(time::macros::date!(1985 - 04 - 12))),
        address: None,
        email: None,
        phone_number: None,
        phone_country_code: None,
    });
    let error = build(Some(&sender), None).expect_err("address is required");
    assert_eq!(missing_field(&error), "sender_details.address");
}

#[test]
fn sender_address_requires_checkout_mandatory_fields() {
    assert!(build_checkout_sender_address(&sender_address()).is_ok());
    for (address, field_name) in [
        (
            SenderAddress {
                line1: None,
                ..sender_address()
            },
            "sender_details.address.line1",
        ),
        (
            SenderAddress {
                city: None,
                ..sender_address()
            },
            "sender_details.address.city",
        ),
        (
            SenderAddress {
                zip: None,
                ..sender_address()
            },
            "sender_details.address.zip",
        ),
        (
            SenderAddress {
                country: None,
                ..sender_address()
            },
            "sender_details.address.country",
        ),
        (
            SenderAddress {
                country: Some(common_enums::CountryAlpha2::US),
                ..sender_address()
            },
            "sender_details.address.state",
        ),
    ] {
        let error = build_checkout_sender_address(&address).expect_err(field_name);
        assert_eq!(missing_field(&error), field_name);
    }
}

#[test]
fn without_sender_details_the_request_is_unchanged() {
    let customer = customer(Some(time::macros::date!(1985 - 04 - 12)));
    assert_eq!(
        build(None, Some(&customer)).expect("sender builds"),
        json!({
            "type": "individual",
            "first_name": "Jane",
            "last_name": "Doe",
            "date_of_birth": "1985-04-12",
            "address": {
                "address_line1": "1 Billing St",
                "city": "New York",
                "state": "NY",
                "zip": "10001",
                "country": "US"
            }
        })
    );

    let error = build(None, None).expect_err("date of birth is required");
    assert_eq!(missing_field(&error), "customer.date_of_birth");
}

/// `sender_details` can reach an AFT without names (e.g. built from a nameless billing address);
/// the names are enforced here, when the AFT sender is built, not when the request is parsed.
#[test]
fn aft_individual_sender_requires_both_names() {
    let date_of_birth = Some(time::macros::date!(1985 - 04 - 12));
    for (sender, field_name) in [
        (
            SenderDetails::Individual(IndividualSenderDetails {
                first_name: None,
                ..individual_details(date_of_birth)
            }),
            "sender_details.individual.first_name",
        ),
        (
            SenderDetails::Individual(IndividualSenderDetails {
                last_name: None,
                ..individual_details(date_of_birth)
            }),
            "sender_details.last_name",
        ),
    ] {
        let error = build(Some(&sender), None).expect_err("names are required for AFT");
        assert_eq!(missing_field(&error), field_name);
    }
}

fn individual_details(date_of_birth: Option<time::Date>) -> IndividualSenderDetails {
    IndividualSenderDetails {
        first_name: Some(secret("John")),
        last_name: Some(secret("Smith")),
        date_of_birth: date_of_birth.map(Secret::new),
        address: Some(sender_address()),
        email: None,
        phone_number: None,
        phone_country_code: None,
    }
}

fn webhook_request(body: Value) -> RequestDetails {
    RequestDetails {
        method: HttpMethod::Post,
        uri: None,
        headers: HashMap::from([(
            "content-type".to_owned(),
            Secret::new("application/json".to_owned()),
        )]),
        body: Secret::new(serde_json::to_vec(&body).expect("serialize")),
        query_params: None,
    }
}

fn payout_body(event_type: &str, destination: bool, declined: bool) -> Value {
    let mut data = json!({"id": "pay_test_payout", "amount": 200});
    if destination {
        data["destination"] = json!({"type": "card"});
    }
    if declined {
        data["response_code"] = json!("70000");
        data["response_summary"] = json!("Declined");
    }
    json!({
        "id": "evt_test_payout",
        "type": event_type,
        "version": "1.0.0",
        "data": data,
    })
}

#[test]
fn card_payout_approved_webhook_routes_to_payout_success() {
    let connector = Checkout::<DefaultPCIHolder>::new();
    let request = webhook_request(payout_body("payment_approved", true, false));
    assert_eq!(
        connector.get_event_type(request.clone()).expect("event"),
        EventType::PayoutSuccess
    );
    match connector
        .get_webhook_event_reference(request.clone())
        .expect("reference")
        .expect("present")
    {
        WebhookResourceReference::Payout(reference) => {
            assert_eq!(
                reference.connector_payout_id.as_deref(),
                Some("pay_test_payout")
            );
        }
        _ => panic!("expected payout reference"),
    }
    let response = connector
        .process_payout_webhook(request, None, None)
        .expect("payout webhook");
    assert_eq!(response.status, PayoutStatus::Success);
    assert_eq!(
        response.connector_payout_id.as_deref(),
        Some("pay_test_payout")
    );
    assert_eq!(response.status_code, 200);
}

#[test]
fn card_payout_declined_webhook_maps_to_payout_failure() {
    let connector = Checkout::<DefaultPCIHolder>::new();
    let request = webhook_request(payout_body("payment_declined", true, true));
    assert_eq!(
        connector.get_event_type(request.clone()).expect("event"),
        EventType::PayoutFailure
    );
    let response = connector
        .process_payout_webhook(request, None, None)
        .expect("payout webhook");
    assert_eq!(response.status, PayoutStatus::Failure);
    assert_eq!(response.error_code.as_deref(), Some("70000"));
    assert_eq!(response.error_message.as_deref(), Some("Declined"));
}

#[test]
fn payment_approved_without_destination_is_not_a_payout() {
    let connector = Checkout::<DefaultPCIHolder>::new();
    let request = webhook_request(payout_body("payment_approved", false, false));
    assert_eq!(
        connector.get_event_type(request.clone()).expect("event"),
        EventType::PaymentIntentAuthorizationSuccess
    );
    assert!(connector
        .process_payout_webhook(request, None, None)
        .is_err());
}

#[test]
fn payment_paid_event_routes_to_payout_success() {
    let connector = Checkout::<DefaultPCIHolder>::new();
    let request = webhook_request(payout_body("payment_paid", false, false));
    assert_eq!(
        connector.get_event_type(request.clone()).expect("event"),
        EventType::PayoutSuccess
    );
    let response = connector
        .process_payout_webhook(request, None, None)
        .expect("payout webhook");
    assert_eq!(response.status, PayoutStatus::Success);
}

#[test]
fn non_payment_event_does_not_require_full_body_amount() {
    let connector = Checkout::<DefaultPCIHolder>::new();
    let request = webhook_request(json!({"type": "dispute_received", "data": {"id": "dsp_test"}}));
    assert_eq!(
        connector.get_event_type(request).expect("event"),
        EventType::DisputeOpened
    );
}
