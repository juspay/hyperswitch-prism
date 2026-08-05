//! Rules: stored-credential / MIT signaling.
//!
//!   • `cardOnFile` — Visa-only "Y" marker. Sent on CIT-setup (storing a
//!     credential for future use) and any MIT. Cert csv:
//!       "cardOnFile tag must not be sent on the 2.00 Mastercard
//!        transaction in step 5 as on Direct Marketing this is a Visa
//!        Merchant Initiated Transaction (MIT) only tag."
//!       "cardOnFile tag must not be sent on the 25.50 Visa card on file
//!        transaction in step 5 as this tag is a Merchant initiated
//!        transaction (MIT) only and this test case is a Consumer
//!        initiated transaction (CIT)."
//!       "cardOnFile tag must be sent on 0.00 Visa card authentication to
//!        store credentials for future payments in step 5."
//!
//!   • `cardOnFileTransactionIdentifier` — MIT only. Cert csv:
//!       "cardOnFileTransactionIdentifier tag must not be sent on the
//!        25.50 Visa card on file transaction in step 5 as this tag is
//!        a Merchant initiated transaction (MIT) only..."
//!
//!   • `citStatusIndicator` — Mastercard:
//!       - CIT-setup unscheduled  → C101
//!       - CIT-setup recurring    → C102
//!       - CIT-setup subscription → C103
//!       - CIT-setup installment  → C104
//!       - CIT-using-stored       → C101 (the consumer is re-using a stored
//!         card for an unscheduled purchase). MIT does not send this.
//!
//!   • `mitStatusIndicator` — Discover-family + Mastercard unscheduled.
//!     Suppressed on CIT-using-stored ("mitStatusIndicator tag must not
//!     be sent on the 29.75 Mastercard transaction in step 5 as this test
//!     case is a Card on File CIT and not MIT").
//!
//!   • `mit` block (with `mit_indicator`) — used by Visa for the
//!     stored-vault flow.

use super::super::profile::{CardFamily, CofPhase, MitIntent, MitKind, TxProfile};
use super::super::transformers::{
    TsysTransitCardOnFile, TsysTransitMcCitStatusIndicator, TsysTransitMit, TsysTransitMitIndicator,
};

/// `cardOnFile` — "Y" marker, network- and phase-dependent:
///
///   • **Visa** — sent ONLY on CIT-setup (the transaction that stores the
///     credential). NOT on a Visa MIT: TSYS rejects it on the 25.50 Visa
///     card-on-file MIT ("cardOnFile tag must not be sent … in step 9").
///   • **Discover family** (Discover / JCB / Diners / UnionPay) — sent on the
///     recurring / installment MIT (cert rows 147, 155, 165, 172), where it
///     accompanies `cardOnFileTransactionIdentifier`. NOT on the CIT-setup
///     (store) transaction (cert rows 133, 134).
///   • **Mastercard / AMEX** — never; they signal the stored credential via
///     `mitStatusIndicator` / `citStatusIndicator` instead.
pub fn card_on_file(profile: &TxProfile) -> Option<TsysTransitCardOnFile> {
    match (profile.card_family, profile.cof_phase) {
        (CardFamily::Visa, CofPhase::CitSetup { .. } | CofPhase::Mit(..)) => Some(TsysTransitCardOnFile::Y),
        (family, CofPhase::Mit(MitKind::Recurring | MitKind::Installment))
            if is_discover_family(family)  =>
        {
            Some(TsysTransitCardOnFile::Y)
        }
        _ => None,
    }
}

/// The Discover "family" of networks that share stored-credential signalling
/// (Discover, JCB, Diners, UnionPay).
fn is_discover_family(family: CardFamily) -> bool {
    matches!(
        family,
        CardFamily::Discover | CardFamily::Jcb | CardFamily::Diners | CardFamily::UnionPay
    )
}

/// `citStatusIndicator`.
pub fn cit_status_indicator(profile: &TxProfile) -> Option<TsysTransitMcCitStatusIndicator> {
    use TsysTransitMcCitStatusIndicator::*;
    if !matches!(profile.card_family, CardFamily::Mastercard) {
        return None;
    }
    match profile.cof_phase {
        CofPhase::CitSetup { intended_kind } => Some(match intended_kind {
            MitIntent::Unscheduled => C101,
            MitIntent::Recurring => C102,
            MitIntent::Installment => C104,
        }),
        CofPhase::CitUsingStored => Some(C101),
        CofPhase::NoCof | CofPhase::Mit(_) => None,
    }
}

/// `mitStatusIndicator`.
pub fn mit_status_indicator(profile: &TxProfile) -> Option<TsysTransitMitIndicator> {
    use TsysTransitMitIndicator::*;
    // CIT-using-stored never sends MIT status (use citStatusIndicator).
    if matches!(profile.cof_phase, CofPhase::CitUsingStored) {
        return None;
    }
    let CofPhase::Mit(kind) = profile.cof_phase else {
        return None;
    };
    match (profile.card_family, kind) {
        // Mastercard
        (CardFamily::Mastercard, MitKind::Unscheduled | MitKind::Resubmission) => Some(M101),
        (CardFamily::Mastercard, MitKind::Recurring) => Some(M102),
        (CardFamily::Mastercard, MitKind::Installment) => Some(M104),
        // Discover family unscheduled / resubmission
        (
            CardFamily::Discover | CardFamily::Jcb | CardFamily::Diners | CardFamily::UnionPay,
            MitKind::Unscheduled | MitKind::Resubmission,
        ) => Some(U),
        // Discover-family recurring → R, installment → S/T
        (
            CardFamily::Discover | CardFamily::Jcb | CardFamily::Diners | CardFamily::UnionPay,
            MitKind::Recurring,
        ) => Some(R),
        (
            CardFamily::Discover | CardFamily::Jcb | CardFamily::Diners | CardFamily::UnionPay,
            MitKind::Installment,
        ) => Some(S),
        _ => None,
    }
}

/// Whether to send the `cardOnFileTransactionIdentifier` tag at all.
///
/// MIT only (suppressed on CIT-using-stored / CIT-setup). Sent for:
///   • **Visa** — any MIT kind. Source of truth TSYS_MOTO_V2 step 5: the Visa
///     unscheduled MIT (row 164) carries it (`000000000640845`).
///   • **Discover family** (Discover / JCB / Diners / UnionPay) — on the
///     recurring / installment MIT (cert rows 147, 155, 165, 172), alongside
///     `cardOnFile` and `mitStatusIndicator` (R / S / T).
/// NOT sent for Mastercard (uses `mitStatusIndicator=M10x`), AMEX (omits it),
/// nor for the Discover-family *unscheduled* MIT (MOTO_V2 uses `U` instead).
pub fn should_send_card_on_file_transaction_identifier(profile: &TxProfile) -> bool {
    match (profile.card_family, profile.cof_phase) {
        (CardFamily::Visa, phase) => phase.is_mit(),
        (family, CofPhase::Mit(MitKind::Recurring | MitKind::Installment)) => {
            is_discover_family(family)
        }
        _ => false,
    }
}

/// The `mit` block — sent for vault-stored mandates to indicate the MIT
/// kind (`R`, `S`, `T`, etc.). For NTID-mandates the `cardOnFileTransactionIdentifier`
/// carries the network transaction ID instead.
pub fn mit_block_indicator(profile: &TxProfile) -> Option<TsysTransitMit> {
    // Only sent on vault MIT (not NTID, not CIT-anything).
    // For now we mirror the original `build_card_on_file_context` logic:
    // vault + non-recurring MIT got `R`. Recurring/installment values come
    // through `mit_status_indicator` on the body instead.
    if !profile.cof_phase.is_mit() {
        return None;
    }
    Some(TsysTransitMit {
        mit_indicator: TsysTransitMitIndicator::R,
    })
}
