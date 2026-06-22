//! Cert-row conformance tests for the rules layer.
//!
//! Each test name documents the TSYS MOTO csv pushback row it satisfies.
//! New cert rows → new tests; failing → rule is wrong; missing test → cert
//! row not yet encoded.

use super::super::profile::{
    AcceptanceProfile, CaptureKind, CardFamily, CofPhase, CommercialLevel, MitIntent, MitKind,
    ThreeDsKind, TxProfile,
};
use super::super::transformers::{
    TsysTransitCardDataInputMode, TsysTransitCardDataSource, TsysTransitCardOnFile,
    TsysTransitMcCitStatusIndicator, TsysTransitMitIndicator, TsysTransitRegisteredUserIndicator,
    TsysTransitTerminalOperatingEnvironment, TsysTransitTerminalOutputCapability,
};
use super::{card_input_mode, cof_mit, commercial, network_indicators};

fn profile(
    acceptance: AcceptanceProfile,
    card_family: CardFamily,
    cof_phase: CofPhase,
    commercial_level: CommercialLevel,
    capture: CaptureKind,
) -> TxProfile {
    TxProfile {
        acceptance,
        card_family,
        cof_phase,
        commercial_level,
        three_ds: ThreeDsKind::None,
        capture,
    }
}

// ============================================================================
// AcceptanceProfile::terminal_data — bucket 1
// ============================================================================

#[test]
fn moto_phone_uses_on_merchant_premises_attended() {
    // Cert: "terminalOperatingEnvironment must be set to
    // 'ON_MERCHANT_PREMISES_ATTENDED' on all [MOTO] transactions."
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        block.terminal_operating_environment,
        TsysTransitTerminalOperatingEnvironment::OnMerchantPremisesAttended
    ));
}

#[test]
fn moto_phone_uses_display_only_output() {
    // Cert: "terminalOutputCapability must be set to 'DISPLAY_ONLY'
    // on all [MOTO] transactions."
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        block.terminal_output_capability,
        TsysTransitTerminalOutputCapability::DisplayOnly
    ));
}

#[test]
fn moto_phone_uses_phone_card_data_source() {
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        block.card_data_source,
        TsysTransitCardDataSource::Phone
    ));
}

#[test]
fn moto_mail_uses_mail_card_data_source() {
    let block = AcceptanceProfile::MotoMail.terminal_data();
    assert!(matches!(
        block.card_data_source,
        TsysTransitCardDataSource::Mail
    ));
}

#[test]
fn moto_mail_also_uses_on_merchant_premises_attended_and_display_only() {
    let block = AcceptanceProfile::MotoMail.terminal_data();
    assert!(matches!(
        block.terminal_operating_environment,
        TsysTransitTerminalOperatingEnvironment::OnMerchantPremisesAttended
    ));
    assert!(matches!(
        block.terminal_output_capability,
        TsysTransitTerminalOutputCapability::DisplayOnly
    ));
}

#[test]
fn ecom_internet_uses_no_terminal_and_none_output() {
    let block = AcceptanceProfile::EcomInternet.terminal_data();
    assert!(matches!(
        block.terminal_operating_environment,
        TsysTransitTerminalOperatingEnvironment::NoTerminal
    ));
    assert!(matches!(
        block.terminal_output_capability,
        TsysTransitTerminalOutputCapability::None
    ));
    assert!(matches!(
        block.card_data_source,
        TsysTransitCardDataSource::Internet
    ));
}

// ============================================================================
// rules::network_indicators — bucket 2 & 4
// ============================================================================

#[test]
fn partial_auth_support_is_always_none() {
    // Cert: "partialAuthSupport tag is a deprecated tag and should not
    // be sent."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::partial_auth_support(&p).is_none());
}

#[test]
fn registered_user_is_none_on_moto_phone_discover() {
    // Cert: "registeredUserIndicator and lastRegisteredChangeDate tags
    // must not be sent on the 12.00 Discover transaction in step 2 as
    // these tags are e-Commerce only."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Discover,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::registered_user(&p).is_none());
}

#[test]
fn registered_user_present_on_ecom_discover_no_cof() {
    let p = profile(
        AcceptanceProfile::EcomInternet,
        CardFamily::Discover,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let result = network_indicators::registered_user(&p);
    assert!(matches!(
        result,
        Some((TsysTransitRegisteredUserIndicator::No, ref date)) if date == "00/00/0000"
    ));
}

#[test]
fn authorization_indicator_for_card_auth_returns_final_for_mastercard() {
    // Cert: "authorizationIndicator tag is missing on all Mastercard
    // transactions in steps 2 and 3."  (step 3 = card auth)
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator_for_card_auth(&p).is_some());
}

#[test]
fn authorization_indicator_for_card_auth_returns_none_for_visa() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator_for_card_auth(&p).is_none());
}

// ============================================================================
// rules::card_input_mode — bucket 4
// ============================================================================

#[test]
fn amex_with_cvv_uses_manually_entered_with_keyed_cid() {
    // Cert: "cardDataInputMode tag must be set to
    // 'MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB' on the 1.50 AMEX level
    // II transaction in step 2 as the cvv was sent."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        card_input_mode::card_data_input_mode(&p, &block, true),
        TsysTransitCardDataInputMode::ManuallyEnteredWithKeyedCidAmexJcb
    ));
}

#[test]
fn jcb_with_cvv_uses_manually_entered_with_keyed_cid() {
    // Cert: same rule as AMEX, JCB row in step 2 (13.00 JCB).
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Jcb,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        card_input_mode::card_data_input_mode(&p, &block, true),
        TsysTransitCardDataInputMode::ManuallyEnteredWithKeyedCidAmexJcb
    ));
}

#[test]
fn cit_setup_uses_key_entered_input() {
    // Cert: "cardDataInputMode tag must be set to 'KEY_ENTERED_INPUT'
    // on the 0.00 Visa card authentication in step 5 as this transaction
    // will be used to store credentials for payment."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::CitSetup {
            intended_kind: MitIntent::Unscheduled,
        },
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        card_input_mode::card_data_input_mode(&p, &block, false),
        TsysTransitCardDataInputMode::KeyEnteredInput
    ));
}

#[test]
fn mit_uses_stored_on_file_marker() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        card_input_mode::card_data_input_mode(&p, &block, false),
        TsysTransitCardDataInputMode::MerchantInitiatedTransactionCardCredentialStoredOnFile
    ));
}

#[test]
fn moto_phone_no_cof_no_cvv_uses_key_entered_input() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = AcceptanceProfile::MotoPhone.terminal_data();
    assert!(matches!(
        card_input_mode::card_data_input_mode(&p, &block, true),
        TsysTransitCardDataInputMode::KeyEnteredInput
    ));
}

// ============================================================================
// rules::commercial — bucket 3
// ============================================================================

#[test]
fn purchase_order_strips_on_amex() {
    // Cert: "purchaseOrder tag should not be sent on the 1.50 AMEX level
    // II transaction in step 2 as this is a Visa and Mastercard only tag."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::purchase_order(&p, Some("PO123".to_string())),
        None
    );
}

#[test]
fn purchase_order_passes_through_on_visa_l2() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::purchase_order(&p, Some("PO123".to_string())),
        Some("PO123".to_string())
    );
}

#[test]
fn customer_ref_id_strips_on_mastercard() {
    // Cert: "customerRefID tag must not be sent on the .52 Mastercard
    // transaction in step 2 as this is an AMEX only tag."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::customer_ref_id(&p, Some("REF123".to_string())),
        None
    );
}

#[test]
fn customer_ref_id_passes_through_on_amex() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::customer_ref_id(&p, Some("REF123".to_string())),
        Some("REF123".to_string())
    );
}

#[test]
fn supplier_reference_number_strips_on_mastercard() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::supplier_reference_number(&p, Some("SUP123".to_string())),
        None
    );
}

#[test]
fn ship_to_zip_strips_on_mastercard_l2() {
    // Cert: "shipToZip tag should not be sent on the .52 Mastercard
    // transaction in step 2 as this is a Level III tag and we are only
    // testing Level II on this test case."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(commercial::ship_to_zip(&p, Some("12345".to_string())), None);
}

#[test]
fn ship_to_zip_strips_on_amex_l3() {
    // Cert: "shipToZip tag must not be sent on the 1.50 AMEX level II
    // transaction in step 1 as this is a Visa and Mastercard level III
    // only tag." — AMEX never carries this regardless of level.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::L3,
        CaptureKind::Auto,
    );
    assert_eq!(commercial::ship_to_zip(&p, Some("12345".to_string())), None);
}

#[test]
fn ship_to_zip_passes_through_on_visa_l3() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::L3,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::ship_to_zip(&p, Some("12345".to_string())),
        Some("12345".to_string())
    );
}

#[test]
fn destination_country_code_strips_on_mastercard_l2() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert_eq!(
        commercial::destination_country_code(&p, Some("840".to_string())),
        None
    );
}

// ============================================================================
// rules::cof_mit — bucket 5
// ============================================================================

#[test]
fn card_on_file_is_none_on_mastercard_cit_setup() {
    // Cert: "cardOnFile tag must not be sent on the 2.00 Mastercard
    // transaction in step 5 as on Direct Marketing this is a Visa
    // Merchant Initiated Transaction (MIT) only tag."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::CitSetup {
            intended_kind: MitIntent::Unscheduled,
        },
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(cof_mit::card_on_file(&p).is_none());
}

#[test]
fn card_on_file_is_y_on_visa_cit_setup() {
    // Cert: "cardOnFile tag must be sent on 0.00 Visa card authentication
    // to store credentials for future payments in step 5."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::CitSetup {
            intended_kind: MitIntent::Unscheduled,
        },
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        cof_mit::card_on_file(&p),
        Some(TsysTransitCardOnFile::Y)
    ));
}

#[test]
fn card_on_file_is_none_on_visa_cit_using_stored() {
    // Cert: "cardOnFile tag must not be sent on the 25.50 Visa card on
    // file transaction in step 5 as this tag is a Merchant initiated
    // transaction (MIT) only and this test case is a Consumer initiated
    // transaction (CIT)."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::CitUsingStored,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(cof_mit::card_on_file(&p).is_none());
}

#[test]
fn cit_status_indicator_c101_on_mastercard_cit_using_stored() {
    // Cert: "mitStatusIndicator tag must not be sent on the 29.75
    // Mastercard transaction in step 5 as this test case is a Card on
    // File CIT and not MIT. The citStatusIndicator tag will need to be
    // sent on this test case with a value of 'C101'."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::CitUsingStored,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        cof_mit::cit_status_indicator(&p),
        Some(TsysTransitMcCitStatusIndicator::C101)
    ));
    assert!(cof_mit::mit_status_indicator(&p).is_none());
}

#[test]
fn cit_status_indicator_c101_on_mastercard_cit_setup() {
    // Cert: "citStatusIndicator tag must be sent on the 2.00 Mastercard
    // transaction in step 5 with a value of C101 as this transaction
    // will be used to store credentials for future unscheduled payments."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::CitSetup {
            intended_kind: MitIntent::Unscheduled,
        },
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        cof_mit::cit_status_indicator(&p),
        Some(TsysTransitMcCitStatusIndicator::C101)
    ));
}

#[test]
fn mit_status_m101_on_mastercard_unscheduled_mit() {
    // Cert: "mitStatusIndicator should sent a value of 'M101' on the
    // 29.85 Mastercard Card on File transaction in step 5 as this test
    // case will be an unscheduled Merchant Initiated transaction."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        cof_mit::mit_status_indicator(&p),
        Some(TsysTransitMitIndicator::M101)
    ));
    assert!(cof_mit::cit_status_indicator(&p).is_none());
}

#[test]
fn mit_status_u_on_discover_unscheduled_mit() {
    // Cert: "mitStatusIndicator must be sent with a value of 'U' on the
    // 34.03 Discover Merchant Initiated Card on File transaction in
    // step 5."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Discover,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        cof_mit::mit_status_indicator(&p),
        Some(TsysTransitMitIndicator::U)
    ));
}

#[test]
fn should_not_send_cof_txn_id_on_cit_using_stored() {
    // Cert: "cardOnFileTransactionIdentifier tag must not be sent on
    // the 25.50 Visa card on file transaction in step 5 as this tag is
    // a Merchant initiated transaction (MIT) only..."
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::CitUsingStored,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
        &p
    ));
}

#[test]
fn should_send_cof_txn_id_on_mit() {
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(cof_mit::should_send_card_on_file_transaction_identifier(&p));
}
