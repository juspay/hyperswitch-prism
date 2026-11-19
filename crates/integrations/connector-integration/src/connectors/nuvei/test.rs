#![allow(clippy::expect_used)]

use domain_types::connector_types::{
    AirlineData, AirlineJourneyDetails, AirlineLocation, AirlineSegment, DomainData,
};
use hyperswitch_masking::ExposeInterface;
use serde_json::{json, Value};

use super::transformers::get_nuvei_airline_addendums;

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
fn offsetless_dates_preserve_wall_clock_and_drop_fractional_seconds() {
    for input in ["2026-12-01T10:30:45", "2026-12-01T10:30:45.123"] {
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

fn dispute_status_for(code: &str) -> Result<common_enums::DisputeStatus, String> {
    let chargeback: super::transformers::ChargebackData = serde_json::from_value(json!({
        "Amount": 10.0,
        "Currency": "USD",
        "ReportedAmount": 10.0,
        "ReportedCurrency": "USD",
        "DisputeUnifiedStatusCode": code,
    }))
    .map_err(|e| format!("chargeback does not deserialize: {e}"))?;
    super::transformers::get_dispute_status(&chargeback).map_err(|e| format!("{e:?}"))
}

#[test]
fn closed_recall_chargeback_is_cancelled() {
    assert_eq!(
        dispute_status_for("FC-CLSD-RCL"),
        Ok(common_enums::DisputeStatus::DisputeCancelled)
    );
}

#[test]
fn collaboration_and_inquiry_rejections_are_challenged() {
    for code in ["MCC-M-RJCT", "MCC-A-RJCT", "INQ-A-RJCT"] {
        assert_eq!(
            dispute_status_for(code),
            Ok(common_enums::DisputeStatus::DisputeChallenged),
            "{code}"
        );
    }
}

#[test]
fn partial_inquiry_refund_is_accepted() {
    assert_eq!(
        dispute_status_for("INQ-M-P-RFND"),
        Ok(common_enums::DisputeStatus::DisputeAccepted)
    );
}

#[test]
fn inquiry_update_still_has_no_dispute_status() {
    assert!(dispute_status_for("INQ-UPD").is_err());
}

#[test]
fn rejected_and_unavailable_challenge_results_are_failed() {
    use super::transformers::NuveiAcsTransStatus;
    for status in ["N", "R", "U"] {
        let parsed: NuveiAcsTransStatus =
            serde_json::from_value(json!(status)).expect("transStatus deserializes");
        assert_eq!(parsed, NuveiAcsTransStatus::Failed, "{status}");
    }
    let attempted: NuveiAcsTransStatus =
        serde_json::from_value(json!("A")).expect("transStatus deserializes");
    assert_eq!(attempted, NuveiAcsTransStatus::Unknown);
}
