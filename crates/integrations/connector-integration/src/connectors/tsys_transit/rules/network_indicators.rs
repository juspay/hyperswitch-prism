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

use super::super::profile::{CardFamily, CaptureKind, CofPhase, TxProfile};
use super::super::transformers::{
    TsysTransitAuthorizationIndicator, TsysTransitRegisteredUserIndicator,
};

/// `authorizationIndicator` for the Sale/Auth flow.
///
/// Mastercard: PREAUTH when manual capture, FINAL otherwise — but
/// suppressed entirely on Mastercard recurring + manual capture (that's
/// the only combo TSYS rejects with this tag).
/// AMEX: always send (PREAUTH or FINAL based on capture).
/// Other networks: don't send.
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
/// Cert pushback specifically calls out Mastercard card-auth in step 3.
/// Card-auth is a self-contained 0.00 probe, so always send FINAL.
pub fn authorization_indicator_for_card_auth(
    profile: &TxProfile,
) -> Option<TsysTransitAuthorizationIndicator> {
    matches!(profile.card_family, CardFamily::Mastercard)
        .then_some(TsysTransitAuthorizationIndicator::Final)
}

/// `registeredUserIndicator` + `lastRegisteredChangeDate` pair.
///
/// E-com only, Discover-family only, single-shot only (no MIT / CIT-setup
/// / CIT-using-stored). Cert csv strips these for MOTO, recurring and
/// any COF-related step-5 transaction.
pub fn registered_user(
    profile: &TxProfile,
) -> Option<(TsysTransitRegisteredUserIndicator, String)> {
    if !matches!(profile.acceptance, super::super::profile::AcceptanceProfile::EcomInternet) {
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
