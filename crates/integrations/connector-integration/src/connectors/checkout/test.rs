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
use serde_json::json;

use super::transformers::{build_checkout_sender, build_checkout_sender_address};

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
            state: Some(secret("NY")),
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
        amount_captured: None,
        amount_capturable: None,
        amount: None,
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
        amount_authorized: None,
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
    SenderDetails::Individual(IndividualSenderDetails {
        first_name: secret("John"),
        last_name: secret("Smith"),
        date_of_birth: date_of_birth.map(Secret::new),
        address: Some(sender_address()),
        email: None,
        phone_number: None,
        phone_country_code: None,
    })
}

fn build(
    sender_details: Option<&SenderDetails>,
    customer: Option<&CustomerInfo>,
) -> Result<serde_json::Value, error_stack::Report<IntegrationError>> {
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
    assert_eq!(missing_field(&error), "customer.date_of_birth");
}

#[test]
fn corporate_from_sender_details() {
    let sender = SenderDetails::Corporate(CorporateSenderDetails {
        company_name: secret("Example Trading Ltd"),
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
        first_name: secret("John"),
        last_name: secret("Smith"),
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
