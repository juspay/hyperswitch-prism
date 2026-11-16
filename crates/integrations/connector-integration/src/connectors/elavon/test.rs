#![allow(clippy::expect_used, clippy::panic)]

use common_enums::MitCategory;
use hyperswitch_masking::Secret;

use super::transformers::*;

#[test]
fn eci_maps_to_converge_values_after_stripping_leading_zeros() {
    for (eci, expected) in [
        (Some("05"), 5),
        (Some("5"), 5),
        (Some("02"), 5),
        (Some("06"), 6),
        (Some("01"), 6),
        (Some(" 06 "), 6),
        (Some("07"), 7),
        (Some("03"), 7),
        (Some("00"), 7),
        (Some("0"), 7),
        (Some(""), 7),
        (None, 7),
    ] {
        assert_eq!(
            get_converge_eci(eci).expect("supported ECI"),
            expected,
            "eci {eci:?}"
        );
    }
}

#[test]
fn unknown_eci_is_refused() {
    for eci in ["04", "08", "9", "AB"] {
        assert!(get_converge_eci(Some(eci)).is_err(), "eci {eci}");
    }
}

#[test]
fn mit_category_maps_to_cof_indicator() {
    for (category, expected) in [
        (None, "M01"),
        (Some(MitCategory::Unscheduled), "M01"),
        (Some(MitCategory::Recurring), "M02"),
        (Some(MitCategory::Subscription), "M03"),
        (Some(MitCategory::Installment), "M04"),
        (Some(MitCategory::Resubmission), "M08"),
    ] {
        let indicator = CofIndicator::from(category.clone());
        assert_eq!(
            serde_json::to_value(indicator).expect("serialize indicator"),
            serde_json::json!(expected),
            "{category:?}"
        );
    }
    assert_eq!(
        serde_json::to_value(CofIndicator::ConsumerAdHoc).expect("serialize indicator"),
        serde_json::json!("C01")
    );
}

#[test]
fn fit_to_length_keeps_the_tail() {
    assert_eq!(fit_to_length("short", 25), "short");
    assert_eq!(fit_to_length("exactly5", 8), "exactly5");
    // Over the limit: the leading characters go, the unique tail stays.
    assert_eq!(
        fit_to_length("pay_abcdefghij_1234567890", 17),
        "efghij_1234567890"
    );
    assert_eq!(fit_to_length("ééééé", 3), "ééé");
    assert_eq!(fit_to_length("", 3), "");
}

fn approved(token_response: Option<&str>, token: Option<&str>) -> PaymentResponse {
    PaymentResponse {
        ssl_result: SslResult::Approved,
        ssl_txn_id: "txn-1".to_string(),
        ssl_result_message: "APPROVAL".to_string(),
        ssl_token: token.map(|t| Secret::new(t.to_string())),
        ssl_approval_code: Some("CVI877".to_string()),
        ssl_transaction_type: None,
        ssl_cvv2_response: None,
        ssl_avs_response: None,
        ssl_token_response: token_response.map(str::to_string),
    }
}

#[test]
fn saved_card_token_requires_a_stored_token() {
    assert_eq!(
        get_saved_card_token(&approved(Some("SUCCESS"), Some("4421912014039990"))).as_deref(),
        Some("4421912014039990")
    );
    // SetupMandate and Authorize report no mandate in these cases.
    assert_eq!(
        get_saved_card_token(&approved(Some("FAILURE"), Some("4421912014039990"))),
        None
    );
    assert_eq!(get_saved_card_token(&approved(Some("SUCCESS"), None)), None);
    assert_eq!(get_saved_card_token(&approved(None, None)), None);
}
