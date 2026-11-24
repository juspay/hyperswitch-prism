#![allow(clippy::expect_used, clippy::panic)]

use common_enums::MitCategory;
use std::str::FromStr;

use domain_types::payment_method_data::{DefaultPCIHolder, RawCardNumber};
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
    for (category, expected) in [
        (None, "C01"),
        (Some(MitCategory::Unscheduled), "C01"),
        (Some(MitCategory::Resubmission), "C01"),
        (Some(MitCategory::Recurring), "C02"),
        (Some(MitCategory::Subscription), "C03"),
        (Some(MitCategory::Installment), "C04"),
    ] {
        assert_eq!(
            serde_json::to_value(CofIndicator::for_cit(category.as_ref()))
                .expect("serialize indicator"),
            serde_json::json!(expected),
            "CIT {category:?}"
        );
    }
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
        ssl_ps2000_data: Some("A7540760482850875100A".to_string()),
        ssl_oar_data: Some("010012345600000000001".to_string()),
    }
}

#[test]
fn cit_returns_ps2000_and_approval_code_for_mit() {
    assert_eq!(
        get_cof_reference(&approved(Some("SUCCESS"), None)),
        Some(("A7540760482850875100A".to_string(), "CVI877".to_string()))
    );
    let mut no_ps2000 = approved(None, None);
    no_ps2000.ssl_ps2000_data = None;
    assert_eq!(get_cof_reference(&no_ps2000), None);
}

#[test]
fn mit_by_card_sends_card_and_cit_data_without_token() {
    let request = MandatePaymentRequest::<DefaultPCIHolder> {
        ssl_transaction_type: TransactionType::CcSale,
        ssl_account_id: Secret::new("acct".to_string()),
        ssl_user_id: Secret::new("user".to_string()),
        ssl_pin: Secret::new("pin".to_string()),
        ssl_vendor_id: None,
        ssl_amount: common_utils::types::StringMajorUnit::zero(),
        ssl_email: None,
        ssl_token: None,
        ssl_card_number: Some(RawCardNumber(
            cards::CardNumber::from_str("4000000000000002").expect("card number"),
        )),
        ssl_exp_date: Some(Secret::new("1230".to_string())),
        ssl_entry_mode: "12",
        ssl_avs_address: None,
        ssl_avs_zip: Some(Secret::new("12345".to_string())),
        ssl_cit_mit_cof_indicator: CofIndicator::MerchantStandingOrder,
        ssl_recurring_flag: Some(1),
        ssl_invoice_number: None,
        ssl_ps2000_data: Some("A7563282508072134235A".to_string()),
        ssl_oar_data: None,
        ssl_approval_code: Some("291146".to_string()),
    };
    let xml = quick_xml::se::to_string_with_root("txn", &request).expect("serialize");
    for field in [
        "<ssl_card_number>4000000000000002</ssl_card_number>",
        "<ssl_exp_date>1230</ssl_exp_date>",
        "<ssl_entry_mode>12</ssl_entry_mode>",
        "<ssl_cit_mit_cof_indicator>M02</ssl_cit_mit_cof_indicator>",
        "<ssl_recurring_flag>1</ssl_recurring_flag>",
        "<ssl_ps2000_data>A7563282508072134235A</ssl_ps2000_data>",
        "<ssl_approval_code>291146</ssl_approval_code>",
    ] {
        assert!(xml.contains(field), "{field} missing from {xml}");
    }
    assert!(!xml.contains("ssl_token"), "{xml}");
    assert!(!xml.contains("ssl_cvv2cvc2"), "{xml}");
}
