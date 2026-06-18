//! Commercial-card-level axis (L2 / L3) of `TxProfile`.
//!
//! Used by L2/L3 field-gating rules: AMEX never carries `purchaseOrder`,
//! `shipToZip`/`destinationCountryCode` are L3-only on Visa/Mastercard, etc.

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommercialLevel {
    None,
    L2,
    L3,
}
