//! Commercial-card-level axis (L2 / L3) of `TxProfile`.
//!
//! Used by L2/L3 field-gating rules:
//!   • AMEX never carries `purchaseOrder`
//!   • `shipToZip` / `destinationCountryCode` are L3-only on Visa/Mastercard
//!   • `customerRefID` / `supplierReferenceNumber` are AMEX-only
//!
//! See `rules::commercial` for the per-field decisions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommercialLevel {
    None,
    L2,
    L3,
}

impl CommercialLevel {
    /// Decide commercial level from the L2/L3 inputs the merchant supplied.
    ///
    /// Heuristic mirrors the original `compute_commercial_card_context`
    /// classification: no inputs ⇒ None; line items present ⇒ L3; tax /
    /// shipping / duty present without line items ⇒ L2.
    pub fn derive(
        has_order_details: bool,
        has_tax_amount: bool,
        has_shipping_charges: bool,
        has_duty_charges: bool,
    ) -> Self {
        if !has_order_details && !has_tax_amount && !has_shipping_charges && !has_duty_charges {
            Self::None
        } else if has_order_details {
            Self::L3
        } else {
            Self::L2
        }
    }

    pub fn is_l2(self) -> bool {
        matches!(self, Self::L2)
    }

    pub fn is_l3(self) -> bool {
        matches!(self, Self::L3)
    }

    pub fn is_l2_or_l3(self) -> bool {
        matches!(self, Self::L2 | Self::L3)
    }
}
