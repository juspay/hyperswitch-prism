//! Rules: L2 / L3 commercial-card field gating.
//!
//! Each function takes the profile (used for `card_family` /
//! `commercial_level`) and the raw value the merchant supplied. The
//! function decides whether the field should land on the wire.
//!
//! Cert csv pushback rows this module addresses:
//!   • "purchaseOrder tag should not be sent on the 1.50 AMEX level II
//!      transaction in step 2 as this is a Visa and Mastercard only tag."
//!      → `purchase_order` returns None on AMEX.
//!   • "customerRefID tag must not be sent on the .52 Mastercard
//!      transaction in step 2 as this is an AMEX only tag."
//!   • "supplierReferenceNumber tag must not be sent on the .52 Mastercard
//!      transaction in step 2 as this is an AMEX only tag."
//!      → `customer_ref_id` / `supplier_reference_number` AMEX-only.
//!   • "shipToZip tag should not be sent on the .52 Mastercard transaction
//!      in step 2 as this is a Level III tag and we are only testing
//!      Level II on this test case."
//!   • "destinationCountryCode tag should not be sent on the .52
//!      Mastercard transaction in step 2 as this is a Level III tag..."
//!      → `ship_to_zip` / `destination_country_code` L3-only on V/MC.
//!   • "shipToZip tag must not be sent on the 1.50 AMEX level II
//!      transaction in step 1 as this is a Visa and Mastercard level III
//!      only tag." (reinforces V/MC-only L3 gating)

use super::super::profile::{CardFamily, CommercialLevel, TxProfile};

/// `purchaseOrder` — Visa / Mastercard only. Strip on AMEX.
pub fn purchase_order(profile: &TxProfile, raw: Option<String>) -> Option<String> {
    if profile.card_family.is_amex() {
        return None;
    }
    raw
}

/// `customerRefID` — AMEX only.
pub fn customer_ref_id(profile: &TxProfile, raw: Option<String>) -> Option<String> {
    if profile.card_family.is_amex() {
        raw
    } else {
        None
    }
}

/// `supplierReferenceNumber` — AMEX only.
pub fn supplier_reference_number(profile: &TxProfile, raw: Option<String>) -> Option<String> {
    if profile.card_family.is_amex() {
        raw
    } else {
        None
    }
}

/// `shipToZip` — AMEX (any commercial L2/L3) OR Visa/MC Level III.
/// Source of truth: TSYS_MOTO_V3 row 127 (AMEX Level II) carries
/// `<shipToZip>85284</shipToZip>` (V3 primary); TSYS_MOTO_V2 shows Mastercard
/// L2 (row 126) does NOT carry it. So on L2 it is AMEX-only; Visa/MC only send
/// it at L3. (`destinationCountryCode` stays L3-only.)
pub fn ship_to_zip(profile: &TxProfile, raw: Option<String>) -> Option<String> {
    let amex_commercial = profile.card_family.is_amex() && profile.commercial_level.is_l2_or_l3();
    let vmc_l3 = profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard();
    if amex_commercial || vmc_l3 {
        raw
    } else {
        None
    }
}

/// `destinationCountryCode` — Visa / Mastercard L3 only.
pub fn destination_country_code(profile: &TxProfile, raw: Option<String>) -> Option<String> {
    if profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard() {
        raw
    } else {
        None
    }
}

/// `commercialCardLevel` — sent only when L2 or L3.
pub fn commercial_card_level(
    profile: &TxProfile,
) -> Option<super::super::transformers::TsysTransitCommercialCardLevel> {
    use super::super::transformers::TsysTransitCommercialCardLevel as L;
    match profile.commercial_level {
        CommercialLevel::None => None,
        CommercialLevel::L2 => Some(L::Level2),
        CommercialLevel::L3 => Some(L::Level3),
    }
}

/// True when `purchaseOrder` is a required field for this profile.
/// Used by the assembler to convert "missing" into a hard error.
pub fn purchase_order_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l2_or_l3() && profile.card_family.is_visa_or_mastercard()
}

/// True when `salesTax` is required (any L2 or L3 transaction).
pub fn sales_tax_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l2_or_l3()
}

/// True when the AMEX-L2-required quartet (supplierReferenceNumber,
/// customerRefID, chargeDescriptor) is required. shipToZip is NOT in
/// the required set because the cert explicitly excludes it from AMEX L2.
pub fn amex_l2_extras_required(profile: &TxProfile) -> bool {
    matches!(profile.card_family, CardFamily::Amex)
        && matches!(profile.commercial_level, CommercialLevel::L2)
}

/// True when L3 Visa/MC required fields are required (purchaseOrder,
/// orderDate, summaryCommodityCode, vatInvoice, shipFromZip, shipToZip,
/// destinationCountryCode).
pub fn l3_visa_mc_required(profile: &TxProfile) -> bool {
    profile.commercial_level.is_l3() && profile.card_family.is_visa_or_mastercard()
}
