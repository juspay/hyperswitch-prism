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
    TsysTransitCardholderPresentDetail, TsysTransitMcCitStatusIndicator, TsysTransitMitIndicator,
    TsysTransitRegisteredUserIndicator, TsysTransitTerminalOperatingEnvironment,
    TsysTransitTerminalOutputCapability,
};
use super::{card_input_mode, cof_mit, commercial, network_indicators, terminal_data};

fn profile(
    acceptance: AcceptanceProfile,
    card_family: CardFamily,
    cof_phase: CofPhase,
    commercial_level: CommercialLevel,
    capture: CaptureKind,
) -> TxProfile {
    // Channel source defaults from the acceptance profile; recurring/
    // installment default to a MOTO phone channel for test purposes.
    let channel_source = match acceptance {
        AcceptanceProfile::MotoMail => TsysTransitCardDataSource::Mail,
        AcceptanceProfile::EcomInternet => TsysTransitCardDataSource::Internet,
        _ => TsysTransitCardDataSource::Phone,
    };
    TxProfile {
        acceptance,
        card_family,
        cof_phase,
        commercial_level,
        three_ds: ThreeDsKind::None,
        capture,
        channel_source,
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
        TsysTransitTerminalOperatingEnvironment::OffMerchantPremisesUnattended
    ));
    assert!(matches!(
        block.terminal_output_capability,
        TsysTransitTerminalOutputCapability::DisplayOnly
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
fn authorization_indicator_for_card_auth_is_none_for_mastercard() {
    // The stagegw CardAuthentication XSD rejects authorizationIndicator
    // entirely (F9901), verified live on cert MOTO row 144. So it must NOT
    // be sent on Mastercard card-auth either — None, like every other
    // network. Supersedes the earlier cert-CSV "add it on MC" note.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator_for_card_auth(&p).is_none());
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

#[test]
fn sale_authorization_indicator_final_on_plain_mastercard_sale() {
    // TSYS_MOTO_V2 rows 126/128/133: every Mastercard sale carries FINAL.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator(&p).is_some());
}

#[test]
fn sale_authorization_indicator_final_on_mastercard_mit() {
    // Cert MOTO rows 161/162: Mastercard MIT replaying a stored credential
    // carries authorizationIndicator=FINAL.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator(&p).is_some());
}

#[test]
fn sale_authorization_indicator_always_on_amex() {
    // AMEX always carries authorizationIndicator (cert rows 127/130/139).
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::authorization_indicator(&p).is_some());
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
fn jcb_with_cvv_uses_key_entered_input() {
    // TSYS_MOTO_V3 row 135 (JCB 13.00, cvv2 present) uses KEY_ENTERED_INPUT —
    // JCB is NOT treated like AMEX here. Falls through to the channel default.
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
        TsysTransitCardDataInputMode::KeyEnteredInput
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
fn cit_using_stored_uses_stored_on_file_marker() {
    // TSYS_MOTO_V2 rows 161/162 (25.50 Visa / 29.75 MC CIT-using-stored) carry
    // the STORED_ON_FILE input mode, same as an MIT.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::CitUsingStored,
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
fn derive_stored_replay_off_session_without_mit_category_is_cit_using_stored() {
    // Hyperswitch sends off_session=true for EVERY stored-credential replay,
    // including consumer-present CIT-using-stored (cert MOTO 25.50 Visa /
    // 29.75 MC). Without an explicit mit_category it must resolve to
    // CIT-using-stored (→ C101, no NTID), NOT an MIT (M101).
    let mandate = domain_types::connector_types::MandateIds {
        mandate_id: None,
        mandate_reference_id: Some(
            domain_types::connector_types::MandateReferenceId::NetworkMandateId(
                domain_types::connector_types::NetworkMandateIdRef {
                    network_transaction_id: "VTL1".to_string(),
                    transaction_link_id: None,
                },
            ),
        ),
    };
    assert_eq!(
        CofPhase::derive(Some(&mandate), None, None, Some(true)),
        CofPhase::CitUsingStored
    );
}

#[test]
fn derive_stored_replay_with_mit_category_is_mit() {
    let mandate = domain_types::connector_types::MandateIds {
        mandate_id: None,
        mandate_reference_id: Some(
            domain_types::connector_types::MandateReferenceId::NetworkMandateId(
                domain_types::connector_types::NetworkMandateIdRef {
                    network_transaction_id: "VTL1".to_string(),
                    transaction_link_id: None,
                },
            ),
        ),
    };
    assert_eq!(
        CofPhase::derive(
            Some(&mandate),
            Some(common_enums::MitCategory::Unscheduled),
            None,
            Some(true),
        ),
        CofPhase::Mit(MitKind::Unscheduled)
    );
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
    assert!(matches!(
        commercial::purchase_order(&p, Some("PO123".to_string())),
        Ok(None)
    ));
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
    assert!(matches!(
        commercial::purchase_order(&p, Some("PO123".to_string())),
        Ok(Some(ref po)) if po == "PO123"
    ));
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
    assert!(matches!(
        commercial::customer_ref_id(&p, Some("REF123".to_string())),
        Ok(None)
    ));
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
    assert!(matches!(
        commercial::customer_ref_id(&p, Some("REF123".to_string())),
        Ok(Some(ref id)) if id == "REF123"
    ));
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
    assert!(matches!(
        commercial::supplier_reference_number(&p, Some("SUP123".to_string())),
        Ok(None)
    ));
}

#[test]
fn ship_to_zip_stripped_on_mastercard_l2() {
    // TSYS_MOTO_V2 row 126 (MC Level II): shipToZip is NOT sent. On L2 it is
    // AMEX-only; Visa/MC carry it only at L3.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Mastercard,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert!(matches!(
        commercial::ship_to_zip(&p, Some("12345".to_string())),
        Ok(None)
    ));
}

#[test]
fn ship_to_zip_sent_on_amex_l2() {
    // TSYS_MOTO_V3 row 127 (AMEX Level II) includes <shipToZip>85284</shipToZip>.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Amex,
        CofPhase::NoCof,
        CommercialLevel::L2,
        CaptureKind::Auto,
    );
    assert!(matches!(
        commercial::ship_to_zip(&p, Some("85284".to_string())),
        Ok(Some(ref zip)) if zip == "85284"
    ));
}

#[test]
fn ship_to_zip_stripped_when_not_commercial() {
    // Non-commercial (None level) never carries shipToZip.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(matches!(
        commercial::ship_to_zip(&p, Some("12345".to_string())),
        Ok(None)
    ));
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
    assert!(matches!(
        commercial::ship_to_zip(&p, Some("12345".to_string())),
        Ok(Some(ref zip)) if zip == "12345"
    ));
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
    assert!(matches!(
        commercial::destination_country_code(&p, Some("840".to_string())),
        Ok(None)
    ));
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
fn card_on_file_is_none_on_visa_mit() {
    // Cert step 9: "cardOnFile tag must not be sent on the 25.50 Visa card on
    // file transaction." cardOnFile is a storage marker for CIT-setup only; a
    // MIT references the stored credential via cardOnFileTransactionIdentifier.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Visa,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(cof_mit::card_on_file(&p).is_none());
}

#[test]
fn card_family_recognises_16_digit_diners_from_pan() {
    // The cert Diners test card 3055155515160018 is a modern 16-digit Diners
    // (TSYS routes it via Discover). The shared get_card_issuer only matches
    // 14-digit Diners, so the connector's PAN fallback must still classify it
    // as Diners for the Discover-family recurring/installment tags to fire.
    assert!(matches!(
        CardFamily::from_card_number("3055155515160018"),
        CardFamily::Diners
    ));
    // JCB (35xx) is not wrongly caught as Diners.
    assert!(matches!(
        CardFamily::from_card_number("3530142019945859"),
        CardFamily::Jcb
    ));
}

#[test]
fn card_on_file_and_nti_on_discover_family_recurring_installment_mit() {
    // Cert rows 147/155 (JCB recurring) and 165/172 (Diners installment): the
    // Discover-family recurring/installment MIT sends cardOnFile=Y AND
    // cardOnFileTransactionIdentifier.
    for family in [
        CardFamily::Jcb,
        CardFamily::Diners,
        CardFamily::Discover,
        CardFamily::UnionPay,
    ] {
        for kind in [MitKind::Recurring, MitKind::Installment] {
            let p = profile(
                AcceptanceProfile::RecurringMit,
                family,
                CofPhase::Mit(kind),
                CommercialLevel::None,
                CaptureKind::Auto,
            );
            assert!(
                matches!(cof_mit::card_on_file(&p), Some(TsysTransitCardOnFile::Y)),
                "cardOnFile=Y expected for {family:?} {kind:?}"
            );
            assert!(
                cof_mit::should_send_card_on_file_transaction_identifier(&p),
                "NTI expected for {family:?} {kind:?}"
            );
        }
    }
}

#[test]
fn card_on_file_and_nti_none_on_mastercard_recurring_installment_mit() {
    // Cert rows 162/169 (MasterCard installment): MasterCard signals the stored
    // credential via mitStatusIndicator=M10x, NOT cardOnFile / NTI.
    for kind in [MitKind::Recurring, MitKind::Installment] {
        let p = profile(
            AcceptanceProfile::RecurringMit,
            CardFamily::Mastercard,
            CofPhase::Mit(kind),
            CommercialLevel::None,
            CaptureKind::Auto,
        );
        assert!(cof_mit::card_on_file(&p).is_none());
        assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
            &p
        ));
    }
}

#[test]
fn card_on_file_none_on_discover_family_unscheduled_mit() {
    // Discover-family *unscheduled* MIT uses mitStatusIndicator=U (MOTO_V2),
    // not cardOnFile / NTI — only recurring/installment carry those.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Jcb,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(cof_mit::card_on_file(&p).is_none());
    assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
        &p
    ));
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

#[test]
fn should_not_send_cof_txn_id_on_mastercard_mit() {
    // Cert MOTO rows 161/162: Mastercard MIT conveys the stored credential
    // via mitStatusIndicator, NOT cardOnFileTransactionIdentifier.
    let p = profile(
        AcceptanceProfile::RecurringMit,
        CardFamily::Mastercard,
        CofPhase::Mit(MitKind::Recurring),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
        &p
    ));
}

#[test]
fn should_not_send_cof_txn_id_on_amex_mit() {
    // Cert MOTO row 165: AMEX COF omits cardOnFileTransactionIdentifier.
    let p = profile(
        AcceptanceProfile::RecurringMit,
        CardFamily::Amex,
        CofPhase::Mit(MitKind::Recurring),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
        &p
    ));
}

#[test]
fn should_not_send_cof_txn_id_on_discover_mit() {
    // TSYS_MOTO_V2 row 165: Discover MIT does NOT send
    // cardOnFileTransactionIdentifier — it uses mitStatusIndicator=U instead.
    // Only Visa (row 164) sends the NTID tag.
    let p = profile(
        AcceptanceProfile::MotoPhone,
        CardFamily::Discover,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(!cof_mit::should_send_card_on_file_transaction_identifier(
        &p
    ));
}

// ====== Recurring / e-Commerce (characterization; not yet cert-run-verified) ======
//
// The connector is cert-verified for MOTO only; the recurring/installment and
// e-Commerce arms below are PINNED to current code behavior. Any expectation
// taken from the cert workbook rather than confirmed against an already-passing
// cert run carries a `// TODO(cert)` marker.

#[test]
fn recurring_mit_card_data_source_follows_channel_not_recurring() {
    // cardDataSource must reflect the acceptance CHANNEL (PHONE here), never
    // RECURRING — TSYS rejects `cardDataSource=RECURRING` on MOTO (cert rows
    // 159/160/161/162/165 use PHONE/MAIL). The recurring semantics ride other
    // fields (isRecurring / cardholderPresentDetail), not cardDataSource.
    let p = profile(
        AcceptanceProfile::RecurringMit,
        CardFamily::Mastercard,
        CofPhase::Mit(MitKind::Recurring),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let resolved = terminal_data::resolve(&p, &p.acceptance.terminal_data(), false);
    assert!(matches!(
        resolved.card_data_source,
        TsysTransitCardDataSource::Phone
    ));
    assert!(!matches!(
        resolved.card_data_source,
        TsysTransitCardDataSource::Recurring
    ));
}

#[test]
fn card_family_falls_back_to_bin_when_network_absent() {
    // A MIT card fetched from the locker (CardDetailsForNetworkTransactionId)
    // can arrive without a network. cardOnFile / cardOnFileTransactionIdentifier
    // are gated on CardFamily::Visa, so the family must be recovered from the
    // PAN when the network is absent.
    use common_enums::CardNetwork;
    assert!(matches!(
        CardFamily::from_card_number("4012000098765439"),
        CardFamily::Visa
    ));
    assert!(matches!(
        CardFamily::from_network_or_number(None, "4012000098765439"),
        CardFamily::Visa
    ));
    // An explicit network still wins over BIN detection.
    assert!(matches!(
        CardFamily::from_network_or_number(Some(&CardNetwork::Mastercard), "4012000098765439"),
        CardFamily::Mastercard
    ));
}

#[test]
fn recurring_mit_mastercard_cardholder_present_detail_is_recurring() {
    // Cert (Recurring tab): Mastercard recurring CIT uses
    // CARDHOLDER_NOT_PRESENT_RECURRING_TRANSACTION.
    // Confirmed: RecurringMit default_cardholder_present_detail in acceptance.rs.
    let block = AcceptanceProfile::RecurringMit.terminal_data();
    assert!(matches!(
        block.default_cardholder_present_detail,
        TsysTransitCardholderPresentDetail::CardholderNotPresentRecurringTransaction
    ));
}

#[test]
fn installment_mit_cardholder_present_detail_is_installment() {
    // Cert (Installments tab): installment uses
    // CARDHOLDER_NOT_PRESENT_INSTALLMENT_TRANSACTION.
    // Confirmed: InstallmentMit default_cardholder_present_detail in acceptance.rs.
    let block = AcceptanceProfile::InstallmentMit.terminal_data();
    assert!(matches!(
        block.default_cardholder_present_detail,
        TsysTransitCardholderPresentDetail::CardholderNotPresentInstallmentTransaction
    ));
}

#[test]
fn recurring_mastercard_operating_environment_is_no_terminal() {
    // Cert (Recurring tab): Mastercard recurring operating environment is
    // NO_TERMINAL. Confirmed: rules::terminal_data flips MC recurring to
    // NoTerminal on top of the base OFF_MERCHANT_PREMISES_UNATTENDED default.
    let p = profile(
        AcceptanceProfile::RecurringMit,
        CardFamily::Mastercard,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = terminal_data::terminal_data(&p);
    assert!(matches!(
        block.terminal_operating_environment,
        TsysTransitTerminalOperatingEnvironment::NoTerminal
    ));
}

#[test]
fn recurring_visa_operating_environment_is_off_premises_unattended() {
    // Cert (Recurring tab): non-MC recurring operating environment is
    // OFF_MERCHANT_PREMISES_UNATTENDED. Confirmed: rules::terminal_data only
    // flips MC; Visa falls through to the base recurring default.
    let p = profile(
        AcceptanceProfile::RecurringMit,
        CardFamily::Visa,
        CofPhase::Mit(MitKind::Unscheduled),
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    let block = terminal_data::terminal_data(&p);
    assert!(matches!(
        block.terminal_operating_environment,
        TsysTransitTerminalOperatingEnvironment::OffMerchantPremisesUnattended
    ));
}

#[test]
fn ecom_internet_card_data_source_is_internet() {
    // Cert (e-Commerce tab): cardDataSource = INTERNET.
    // Confirmed: EcomInternet → Source::Internet in acceptance.rs.
    let block = AcceptanceProfile::EcomInternet.terminal_data();
    assert!(matches!(
        block.card_data_source,
        TsysTransitCardDataSource::Internet
    ));
}

#[test]
fn registered_user_is_none_on_ecom_visa_no_cof() {
    // Characterization: registeredUserIndicator is Discover-family only, so an
    // e-Commerce Visa transaction with no card-on-file does NOT carry it.
    // Confirmed: rules::network_indicators::registered_user gates on
    // card_family.is_discover_like(), which excludes Visa.
    let p = profile(
        AcceptanceProfile::EcomInternet,
        CardFamily::Visa,
        CofPhase::NoCof,
        CommercialLevel::None,
        CaptureKind::Auto,
    );
    assert!(network_indicators::registered_user(&p).is_none());
}

// TODO(cert): three_ds eci needs RawInputs-level test once rules::three_ds is
// folded. There is no rules::three_ds module yet (eciIndicator 5/6/7 handling
// is not unit-testable from a profile alone), so the e-Commerce 3DS ECI cert
// rows are not encoded here.

/// Golden-XML characterization tests: lock the EXACT serialized wire
/// format (field order / casing / skip-if-none). TSYS is XSD-order-
/// sensitive, so any reorder or casing change must break these on purpose.
/// (Formerly the separate `golden_tests.rs` module.)
mod golden {
    #![allow(clippy::expect_used)]
    use common_utils::types::StringMajorUnit;

    use super::super::super::transformers::*;

    fn smu(value: &str) -> StringMajorUnit {
        serde_json::from_str(&format!("\"{value}\"")).expect("StringMajorUnit")
    }

    /// Cert row 125: Visa L2 PHONE sale, $0.52.
    ///
    /// Locks the serialized XML wire format (field order / casing /
    /// skip-if-none behavior). TSYS is XSD-order-sensitive, so any future
    /// refactor that reorders struct fields or changes casing must break this
    /// test on purpose.
    #[test]
    fn golden_row_125_visa_l2_phone_sale() {
        let body = TsysTransitAuthorizeBody {
            device_id: "88800023319201".to_string().into(),
            transaction_key: "G3L97P4C6KBDX2H6L385O43CT2VOWCR2".to_string().into(),
            card_data_source: TsysTransitCardDataSource::Phone,
            transaction_amount: smu("0.52"),
            sales_tax: Some(smu("0.01")),
            surcharge: None,
            additional_tax_details: Vec::new(),
            shipping_charges: None,
            duty_charges: None,
            card_number: Some("4012000098765439".to_string().into()),
            expiration_date: Some("12/28".to_string().into()),
            cvv2: Some("999".to_string().into()),
            secure_code: None,
            ucaf_collection_indicator: None,
            directory_server_transaction_id: None,
            eci_indicator: None,
            customer_code: None,
            wallet_details: None,
            card_on_file_transaction_identifier: None,
            previous_network_transaction_id: None,
            cit_status_indicator: None,
            mit_status_indicator: None,
            address_line1: "8320 S HARDY DR".to_string().into(),
            zip: "85284".to_string().into(),
            external_reference_id: "r125XREFSUFFIXX".to_string(),
            product_details: Vec::new(),
            commercial_card_level: Some(TsysTransitCommercialCardLevel::Level2),
            purchase_order: Some("PO125".to_string()),
            charge_descriptor: None,
            customer_vat_number: None,
            customer_ref_id: None,
            supplier_reference_number: None,
            order_date: None,
            summary_commodity_code: None,
            vat_invoice: None,
            ship_from_zip: None,
            ship_to_zip: None,
            destination_country_code: None,
            card_on_file: None,
            partial_auth_support: None,
            terminal_capability: TsysTransitTerminalCapability::KeyedEntryOnly,
            terminal_operating_environment:
                TsysTransitTerminalOperatingEnvironment::OnMerchantPremisesAttended,
            cardholder_authentication_method:
                TsysTransitCardholderAuthenticationMethod::NotAuthenticated,
            terminal_authentication_capability:
                TsysTransitTerminalAuthenticationCapability::NoCapability,
            terminal_output_capability: TsysTransitTerminalOutputCapability::DisplayOnly,
            max_pin_length: TsysTransitMaxPinLength::NotSupported,
            terminal_card_capture_capability:
                TsysTransitTerminalCardCaptureCapability::NoCapability,
            cardholder_present_detail:
                TsysTransitCardholderPresentDetail::CardholderNotPresentPhoneTransaction,
            card_present_detail: TsysTransitCardPresentDetail::CardNotPresent,
            card_data_input_mode: TsysTransitCardDataInputMode::KeyEnteredInput,
            cardholder_authentication_entity:
                TsysTransitCardholderAuthenticationEntity::NotAuthenticated,
            card_data_output_capability: TsysTransitCardDataOutputCapability::None,
            developer_id: "003651G001".to_string().into(),
            is_recurring: None,
            billing_type: None,
            payment_count: None,
            current_payment_count: None,
            original_recurring_amount: None,
            registered_user_indicator: None,
            last_registered_change_date: None,
            authorization_indicator: None,
            mit: None,
            acceptor_customer_service_phone_number: None,
            acceptor_phone_number: None,
            acceptor_street_address: None,
            acceptor_u_r_l_address: None,
        };

        let req = TsysTransitAuthorizeRequest::Sale(body);
        let xml = generate_xml(&req).expect("serialize");

        // Snapshot of the CURRENT serialized output. Field order follows the
        // `TsysTransitAuthorizeBody` struct definition (not the cert script's
        // ordering). If a refactor reorders fields or changes casing, this
        // breaks — intentionally.
        let expected = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<Sale>",
        "<deviceID>88800023319201</deviceID>",
        "<transactionKey>G3L97P4C6KBDX2H6L385O43CT2VOWCR2</transactionKey>",
        "<cardDataSource>PHONE</cardDataSource>",
        "<transactionAmount>0.52</transactionAmount>",
        "<salesTax>0.01</salesTax>",
        "<cardNumber>4012000098765439</cardNumber>",
        "<expirationDate>12/28</expirationDate>",
        "<cvv2>999</cvv2>",
        "<addressLine1>8320 S HARDY DR</addressLine1>",
        "<zip>85284</zip>",
        "<externalReferenceID>r125XREFSUFFIXX</externalReferenceID>",
        "<commercialCardLevel>LEVEL2</commercialCardLevel>",
        "<purchaseOrder>PO125</purchaseOrder>",
        "<terminalCapability>KEYED_ENTRY_ONLY</terminalCapability>",
        "<terminalOperatingEnvironment>ON_MERCHANT_PREMISES_ATTENDED</terminalOperatingEnvironment>",
        "<cardholderAuthenticationMethod>NOT_AUTHENTICATED</cardholderAuthenticationMethod>",
        "<terminalAuthenticationCapability>NO_CAPABILITY</terminalAuthenticationCapability>",
        "<terminalOutputCapability>DISPLAY_ONLY</terminalOutputCapability>",
        "<maxPinLength>NOT_SUPPORTED</maxPinLength>",
        "<terminalCardCaptureCapability>NO_CAPABILITY</terminalCardCaptureCapability>",
        "<cardholderPresentDetail>CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION</cardholderPresentDetail>",
        "<cardPresentDetail>CARD_NOT_PRESENT</cardPresentDetail>",
        "<cardDataInputMode>KEY_ENTERED_INPUT</cardDataInputMode>",
        "<cardholderAuthenticationEntity>NOT_AUTHENTICATED</cardholderAuthenticationEntity>",
        "<cardDataOutputCapability>NONE</cardDataOutputCapability>",
        "<developerID>003651G001</developerID>",
        "</Sale>",
    );

        assert_eq!(xml, expected);
    }

    /// MOTO "cardAuthentication | Visa | PHONE" (step-5 Visa CIT-setup probe).
    ///
    /// Locks the serialized XML wire format of `TsysTransitCardAuthentication
    /// Request` so the `rules::terminal_data::resolve` extraction is provably
    /// behavior-preserving on the CardAuthentication builder too (the Authorize
    /// builder is already guarded by `golden_row_125_visa_l2_phone_sale`).
    ///
    /// The struct is built by hand with the MotoPhone terminalData values a
    /// Visa phone card-auth resolves to (cvv2 999, Visa Account Name Inquiry
    /// firstName/lastName, cardOnFile=Y). authorizationIndicator is NOT sent on
    /// CardAuthentication — the stagegw XSD rejects it (F9901). TSYS is
    /// XSD-order-sensitive, so any field reorder / casing change breaks this on
    /// purpose.
    #[test]
    fn golden_card_authentication_visa_phone() {
        let req = TsysTransitCardAuthenticationRequest {
            device_id: "88800023319201".to_string().into(),
            transaction_key: "G3L97P4C6KBDX2H6L385O43CT2VOWCR2".to_string().into(),
            card_data_source: TsysTransitCardDataSource::Phone,
            card_number: "4012000098765439".to_string().into(),
            expiration_date: "12/28".to_string().into(),
            cvv2: Some("999".to_string().into()),
            address_line1: "8320 S HARDY DR".to_string().into(),
            zip: "85284".to_string().into(),
            external_reference_id: "rCAXREFSUFFIXXX".to_string(),
            card_on_file: Some(TsysTransitCardOnFile::Y),
            cit_status_indicator: None,
            authorization_indicator: None,
            first_name: Some("JANE".to_string().into()),
            middle_name: None,
            last_name: Some("DOE".to_string().into()),
            developer_id: "003651G001".to_string().into(),
            terminal_capability: TsysTransitTerminalCapability::KeyedEntryOnly,
            terminal_operating_environment:
                TsysTransitTerminalOperatingEnvironment::OnMerchantPremisesAttended,
            cardholder_authentication_method:
                TsysTransitCardholderAuthenticationMethod::NotAuthenticated,
            terminal_authentication_capability:
                TsysTransitTerminalAuthenticationCapability::NoCapability,
            terminal_output_capability: TsysTransitTerminalOutputCapability::DisplayOnly,
            max_pin_length: TsysTransitMaxPinLength::NotSupported,
            terminal_card_capture_capability:
                TsysTransitTerminalCardCaptureCapability::NoCapability,
            cardholder_present_detail:
                TsysTransitCardholderPresentDetail::CardholderNotPresentPhoneTransaction,
            card_present_detail: TsysTransitCardPresentDetail::CardNotPresent,
            card_data_input_mode: TsysTransitCardDataInputMode::KeyEnteredInput,
            cardholder_authentication_entity:
                TsysTransitCardholderAuthenticationEntity::NotAuthenticated,
            card_data_output_capability: TsysTransitCardDataOutputCapability::None,
            m_pos_acceptance_device_type: Some("0".to_string()),
            acceptor_customer_service_phone_number: None,
            acceptor_phone_number: None,
            acceptor_u_r_l_address: None,
            acceptor_street_address: None,
        };

        let xml = generate_xml(&req).expect("serialize");

        let expected = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<CardAuthentication>",
        "<deviceID>88800023319201</deviceID>",
        "<transactionKey>G3L97P4C6KBDX2H6L385O43CT2VOWCR2</transactionKey>",
        "<cardDataSource>PHONE</cardDataSource>",
        "<cardNumber>4012000098765439</cardNumber>",
        "<expirationDate>12/28</expirationDate>",
        "<cvv2>999</cvv2>",
        "<addressLine1>8320 S HARDY DR</addressLine1>",
        "<zip>85284</zip>",
        "<externalReferenceID>rCAXREFSUFFIXXX</externalReferenceID>",
        "<cardOnFile>Y</cardOnFile>",
        "<firstName>JANE</firstName>",
        "<lastName>DOE</lastName>",
        "<developerID>003651G001</developerID>",
        "<terminalCapability>KEYED_ENTRY_ONLY</terminalCapability>",
        "<terminalOperatingEnvironment>ON_MERCHANT_PREMISES_ATTENDED</terminalOperatingEnvironment>",
        "<cardholderAuthenticationMethod>NOT_AUTHENTICATED</cardholderAuthenticationMethod>",
        "<terminalAuthenticationCapability>NO_CAPABILITY</terminalAuthenticationCapability>",
        "<terminalOutputCapability>DISPLAY_ONLY</terminalOutputCapability>",
        "<maxPinLength>NOT_SUPPORTED</maxPinLength>",
        "<terminalCardCaptureCapability>NO_CAPABILITY</terminalCardCaptureCapability>",
        "<cardholderPresentDetail>CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION</cardholderPresentDetail>",
        "<cardPresentDetail>CARD_NOT_PRESENT</cardPresentDetail>",
        "<cardDataInputMode>KEY_ENTERED_INPUT</cardDataInputMode>",
        "<cardholderAuthenticationEntity>NOT_AUTHENTICATED</cardholderAuthenticationEntity>",
        "<cardDataOutputCapability>NONE</cardDataOutputCapability>",
        "<mPosAcceptanceDeviceType>0</mPosAcceptanceDeviceType>",
        "</CardAuthentication>",
    );

        assert_eq!(xml, expected);
    }
}
