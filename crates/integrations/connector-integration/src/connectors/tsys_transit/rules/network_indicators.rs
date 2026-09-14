//! Rules: network-driven indicator fields.
//!
//!   • `authorizationIndicator` — Mastercard + AMEX always; capture method
//!      decides PREAUTH vs FINAL.  Cert pushback rows on MOTO step 2 and
//!      step 3 ("authorizationIndicator tag is missing on all Mastercard
//!      transactions in steps 2 and 3") motivate sending this on card
//!      authentications too.
//!
//!   • `registeredUserIndicator` / `lastRegisteredChangeDate` — e-commerce
//!      only, Discover-family only.  Cert rows from MOTO csv (paraphrased):
//!      "registeredUserIndicator and lastRegisteredChangeDate tags must
//!      not be sent on the 12.00 Discover transaction in step 2 as these
//!      tags are e-Commerce only." → strip for any MOTO transaction.
//!
//!   • `partialAuthSupport` — deprecated.  Cert row:
//!      "partialAuthSupport tag is a deprecated tag and should not be sent."
//!      → always None.

use super::super::profile::{CaptureKind, CardFamily, CofPhase, TxProfile};
use super::super::transformers::{
    TsysTransitAuthorizationIndicator, TsysTransitRegisteredUserIndicator,
};

/// `authorizationIndicator` for the Sale/Auth flow.
///
/// Mastercard AND AMEX: always send (PREAUTH on manual capture, else FINAL).
/// Source of truth TSYS_MOTO_V2: every Mastercard row carries FINAL — plain
/// sales (126/128/133) as well as COF/CIT/MIT (160/162/163); AMEX rows
/// (127/130/139/166) too. Suppressed only on the Mastercard scheduled-MIT +
/// manual-capture combo (which TSYS rejects). Other networks: don't send.
pub fn authorization_indicator(profile: &TxProfile) -> Option<TsysTransitAuthorizationIndicator> {
    use TsysTransitAuthorizationIndicator::*;
    let preauth_or_final = if profile.capture.is_manual() {
        Preauth
    } else {
        Final
    };
    match profile.card_family {
        CardFamily::Mastercard => {
            let recurring_manual = profile.acceptance.is_scheduled_mit()
                && matches!(profile.capture, CaptureKind::Manual);
            (!recurring_manual).then_some(preauth_or_final)
        }
        CardFamily::Amex => Some(preauth_or_final),
        _ => None,
    }
}

/// `authorizationIndicator` for the CardAuthentication flow.
///
/// Always `None`: the TSYS stagegw CardAuthentication XSD does not permit an
/// `authorizationIndicator` element at all (sending it yields F9901 "Invalid
/// content … element 'authorizationIndicator'"). Verified live against
/// stagegw — Mastercard card-auth (cert MOTO row 144) is rejected when the tag
/// is present, while Visa/AMEX pass precisely because the tag is omitted. This
/// supersedes the earlier cert-CSV note that asked for the tag on Mastercard
/// card-auth; the gateway schema is authoritative for acceptance.
pub fn authorization_indicator_for_card_auth(
    _profile: &TxProfile,
) -> Option<TsysTransitAuthorizationIndicator> {
    None
}

/// `registeredUserIndicator` + `lastRegisteredChangeDate` pair.
///
/// E-com only, Discover-family only, single-shot only (no MIT / CIT-setup
/// / CIT-using-stored). Cert csv strips these for MOTO, recurring and
/// any COF-related step-5 transaction.
pub fn registered_user(
    profile: &TxProfile,
) -> Option<(TsysTransitRegisteredUserIndicator, String)> {
    if !matches!(
        profile.acceptance,
        super::super::profile::AcceptanceProfile::EcomInternet
    ) {
        return None;
    }
    if !matches!(profile.cof_phase, CofPhase::NoCof) {
        return None;
    }
    profile
        .card_family
        .is_discover_like()
        .then(|| (TsysTransitRegisteredUserIndicator::No, "00/00/0000".into()))
}

/// `partialAuthSupport`.
///
/// Cert csv: deprecated tag, never send.
pub fn partial_auth_support(_profile: &TxProfile) -> Option<String> {
    None
}
