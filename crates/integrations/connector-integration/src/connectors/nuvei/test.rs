#![allow(clippy::expect_used)]

use common_enums::{AttemptStatus, AuthenticationType, CountryAlpha2, PaymentMethod};
use common_utils::pii::Email;
use domain_types::{
    connector_types::{
        AirlineData, AirlineJourneyDetails, AirlineLocation, AirlineSegment, DomainData,
        PaymentFlowData,
    },
    errors::IntegrationError,
    payment_address::{Address, AddressDetails, PaymentAddress, StateDetails},
    utils::ForeignTryFrom,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde_json::{json, Value};

use super::transformers::{
    billing_names, get_nuvei_airline_addendums, get_required_billing_address,
    nuvei_airline_carrier_and_number,
};

fn legs(data: AirlineData) -> Value {
    let domain = DomainData {
        airline_data: Some(data),
        ..Default::default()
    };
    let addendums = get_nuvei_airline_addendums(Some(&domain))
        .expect("airline mapping succeeds")
        .expect("airline addendums exist");
    let value = serde_json::to_value(addendums.expose()).expect("addendums serialize");
    value
        .pointer("/airlines/reservationDetails/flightLegDetails")
        .expect("flight leg details exist")
        .clone()
}

fn segment(journey_no: Option<u32>, number: &str) -> AirlineSegment {
    AirlineSegment {
        journey_no,
        marketing_carrier_code: Some("AI".to_owned()),
        flight_number: Some(number.to_owned()),
        operating_carrier_code: Some("AI".to_owned()),
        operating_flight_number: Some(number.to_owned()),
        ..Default::default()
    }
}

#[test]
fn journeys_group_connections_and_preserve_return_flight() {
    let result = legs(AirlineData {
        flight_journeys: vec![
            AirlineJourneyDetails {
                journey_no: Some(1),
                ..Default::default()
            },
            AirlineJourneyDetails {
                journey_no: Some(2),
                ..Default::default()
            },
        ],
        flight_segments: vec![
            segment(Some(1), "101"),
            segment(Some(1), "102"),
            segment(Some(2), "201"),
        ],
        ..Default::default()
    });
    assert_eq!(result.as_array().expect("legs are an array").len(), 2);
    assert_eq!(
        result
            .pointer("/0/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "X"
    );
    assert_eq!(
        result
            .pointer("/0/flightNumber")
            .expect("expected flight leg field exists")
            .clone(),
        "AI 101"
    );
    assert_eq!(
        result
            .pointer("/1/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "O"
    );
    assert_eq!(
        result
            .pointer("/1/flightNumber")
            .expect("expected flight leg field exists")
            .clone(),
        "AI 201"
    );
}

#[test]
fn no_journeys_preserves_segments_and_explicit_stopover() {
    let mut explicit = segment(Some(2), "201");
    explicit.stopover_code = Some("X".to_owned());
    let result = legs(AirlineData {
        flight_segments: vec![segment(Some(1), "101"), segment(Some(1), "102"), explicit],
        ..Default::default()
    });
    assert_eq!(result.as_array().expect("legs are an array").len(), 3);
    assert_eq!(
        result
            .pointer("/0/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "X"
    );
    assert_eq!(
        result
            .pointer("/1/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "O"
    );
    assert_eq!(
        result
            .pointer("/2/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "X"
    );
}

#[test]
fn missing_journey_numbers_do_not_group_unrelated_segments() {
    let result = legs(AirlineData {
        flight_segments: vec![segment(None, "101"), segment(None, "201")],
        ..Default::default()
    });
    assert_eq!(
        result
            .pointer("/0/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "O"
    );
    assert_eq!(
        result
            .pointer("/1/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "O"
    );
}

#[test]
fn journey_without_segments_retains_endpoints_and_omits_missing_flight_fields() {
    let result = legs(AirlineData {
        flight_journeys: vec![AirlineJourneyDetails {
            journey_no: Some(1),
            departure: Some(AirlineLocation {
                airport_code: Some("DEL".to_owned()),
                ..Default::default()
            }),
            arrival: Some(AirlineLocation {
                airport_code: Some("BLR".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    });
    assert_eq!(
        result
            .pointer("/0/departureAirport")
            .expect("expected flight leg field exists")
            .clone(),
        "DEL"
    );
    assert_eq!(
        result
            .pointer("/0/destinationAirport")
            .expect("expected flight leg field exists")
            .clone(),
        "BLR"
    );
    assert_eq!(
        result
            .pointer("/0/stopOverCode")
            .expect("expected flight leg field exists")
            .clone(),
        "O"
    );
    assert!(result
        .get(0)
        .expect("expected flight leg exists")
        .get("airlineCode")
        .is_none());
    assert!(result
        .get(0)
        .expect("expected flight leg exists")
        .get("flightNumber")
        .is_none());
}

#[test]
fn airline_dates_preserve_wall_clock_and_drop_fractional_seconds() {
    for input in [
        "2026-12-01T10:30:45",
        "2026-12-01T10:30:45.123",
        "2026-12-01T10:30:45 +05:30",
        "2026-12-01T10:30:45 -05:00",
    ] {
        let result = legs(AirlineData {
            flight_segments: vec![AirlineSegment {
                departure: Some(AirlineLocation {
                    date_time: Some(input.to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(
            result
                .pointer("/0/departureDate")
                .expect("expected flight leg field exists")
                .clone(),
            json!("2026-12-01 10:30:45")
        );
    }
}

#[test]
fn billing_names_fill_each_missing_field() {
    for (first, last, holder, expected) in [
        (
            Some("Jane"),
            Some("Smith"),
            Some("John Doe"),
            (Some("Jane"), Some("Smith")),
        ),
        (
            None,
            Some("Smith"),
            Some("John Doe"),
            (Some("John"), Some("Smith")),
        ),
        (
            Some("Jane"),
            None,
            Some("John Doe"),
            (Some("Jane"), Some("Doe")),
        ),
        (
            Some(" "),
            None,
            Some("John Michael Doe"),
            (Some("John Michael"), Some("Doe")),
        ),
        (None, None, Some("Cher"), (Some("Cher"), None)),
        (Some("Jane"), None, Some("Cher"), (Some("Jane"), None)),
        (
            None,
            Some("Smith"),
            Some("Cher"),
            (Some("Cher"), Some("Smith")),
        ),
        (None, None, None, (None, None)),
        (Some("Jane"), None, Some(" "), (Some("Jane"), None)),
    ] {
        let secret = |value: &str| Secret::new(value.to_owned());
        let (first, last) = billing_names(first.map(secret), last.map(secret), holder.map(secret));
        let first = first.map(|name| name.expose());
        let last = last.map(|name| name.expose());
        assert_eq!((first.as_deref(), last.as_deref()), expected);
    }
}

#[test]
fn airline_leg_uses_operating_fields_only() {
    let mut leg = segment(Some(1), "101");
    leg.marketing_carrier_code = Some("XX".to_owned());
    leg.operating_carrier_code = Some(" AI ".to_owned());
    leg.operating_flight_number = Some(" 202 ".to_owned());
    assert_eq!(
        nuvei_airline_carrier_and_number(Some(&leg)),
        (Some("AI"), Some("202"))
    );

    leg.operating_carrier_code = Some(" ".to_owned());
    assert_eq!(
        nuvei_airline_carrier_and_number(Some(&leg)),
        (None, Some("202"))
    );

    leg.operating_carrier_code = Some("AI".to_owned());
    leg.operating_flight_number = None;
    assert_eq!(
        nuvei_airline_carrier_and_number(Some(&leg)),
        (Some("AI"), None)
    );
    assert_eq!(nuvei_airline_carrier_and_number(None), (None, None));
}

#[test]
fn billing_state_formats_the_supplied_iso_code_up_to_five_characters() {
    let email = Email::try_from("test@example.com".to_owned()).expect("valid test email");
    for (country, supplied, normalized, expected) in [
        (CountryAlpha2::US, "US-AL", "AL", Some("US-AL")),
        (CountryAlpha2::US, "US-ABC", "ABC", None),
        (CountryAlpha2::IN, "IN-KA", "KA", Some("IN-KA")),
    ] {
        let state = StateDetails::foreign_try_from((
            Some(Secret::new("California".to_owned())),
            Some(Secret::new(supplied.to_owned())),
            Some(country),
        ))
        .expect("valid supplied ISO code");
        assert_eq!(
            state.code().cloned().map(|code| code.expose()).as_deref(),
            Some(normalized)
        );

        let resource = payment_with_billing(Some(country), Some(state));
        let address = get_required_billing_address(
            &resource,
            Some(email.clone()),
            Some(Secret::new("Jane Doe".to_owned())),
            None,
        )
        .expect("complete billing address");
        assert_eq!(address.state.map(|code| code.expose()).as_deref(), expected);
    }
}

#[test]
fn billing_state_normalizes_names_and_enforces_five_character_cap() {
    let email = Email::try_from("test@example.com".to_owned()).expect("valid test email");
    for (country, state, expected) in [
        (
            CountryAlpha2::US,
            StateDetails::from_name(Secret::new("California".to_owned())),
            Some("US-CA"),
        ),
        (
            CountryAlpha2::US,
            StateDetails::from_code(Secret::new("ABCDE".to_owned())),
            Some("ABCDE"),
        ),
        (
            CountryAlpha2::US,
            StateDetails::from_code(Secret::new("ABCDEF".to_owned())),
            None,
        ),
        (
            CountryAlpha2::IN,
            StateDetails::from_name(Secret::new("Karnataka".to_owned())),
            Some("IN-KA"),
        ),
    ] {
        let resource = payment_with_billing(Some(country), Some(state));
        let address = get_required_billing_address(
            &resource,
            Some(email.clone()),
            Some(Secret::new("Jane Doe".to_owned())),
            None,
        )
        .expect("complete billing address");
        let state = address.state.map(|value| value.expose());
        assert_eq!(state.as_deref(), expected);
    }
}

#[test]
fn billing_country_prefers_customer_country_and_falls_back_to_merchant_country() {
    let email = Email::try_from("test@example.com".to_owned()).expect("valid test email");
    for (billing_country, expected) in [
        (Some(CountryAlpha2::US), CountryAlpha2::US),
        (None, CountryAlpha2::GB),
    ] {
        let resource = payment_with_billing(billing_country, None);
        let address = get_required_billing_address(
            &resource,
            Some(email.clone()),
            Some(Secret::new("Jane Doe".to_owned())),
            Some(CountryAlpha2::GB),
        )
        .expect("billing or merchant country is available");
        assert_eq!(address.country, expected);
    }

    let error =
        get_required_billing_address(&payment_with_billing(None, None), Some(email), None, None)
            .expect_err("billing country is required without a merchant country");
    assert!(matches!(
        error.current_context(),
        IntegrationError::MissingRequiredField {
            field_name: "billing_address.country",
            ..
        }
    ));
}

#[test]
fn payment_billing_rejects_missing_names_after_cardholder_fallback() {
    let resource = payment_with_billing(Some(CountryAlpha2::US), None);
    let email = Email::try_from("test@example.com".to_owned()).expect("valid test email");
    for (holder, expected_field) in [
        (None, "billing_address.first_name"),
        (Some("Cher"), "billing_address.last_name"),
    ] {
        let error = get_required_billing_address(
            &resource,
            Some(email.clone()),
            holder.map(|name| Secret::new(name.to_owned())),
            None,
        )
        .expect_err("both billing names are required for payment");
        assert!(matches!(
            error.current_context(),
            IntegrationError::MissingRequiredField { field_name, .. } if *field_name == expected_field
        ));
    }
}

fn payment_with_billing(
    country: Option<CountryAlpha2>,
    state: Option<StateDetails>,
) -> PaymentFlowData {
    let billing = Address {
        address: Some(AddressDetails {
            country,
            state,
            ..Default::default()
        }),
        ..Default::default()
    };
    PaymentFlowData {
        merchant_id: Default::default(),
        customer_id: None,
        connector_customer: None,
        payment_id: String::new(),
        attempt_id: String::new(),
        status: AttemptStatus::Pending,
        payment_method: PaymentMethod::Card,
        payment_method_type: None,
        description: None,
        return_url: None,
        address: PaymentAddress::new(None, Some(billing), None, None),
        auth_type: AuthenticationType::ThreeDs,
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
        connector_request_reference_id: String::new(),
        test_mode: None,
        connector_http_status_code: None,
        connector_response_headers: None,
        external_latency: None,
        connectors: Default::default(),
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
