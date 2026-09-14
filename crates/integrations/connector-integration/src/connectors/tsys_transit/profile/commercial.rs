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
    /// Level III is OUT OF SCOPE for this connector: any commercial signal
    /// (line items, tax, shipping, duty) resolves to at most Level II. No
    /// productDetails / vatInvoice / shipFromZip / enhanced tax is emitted.
    pub fn derive(
        has_order_details: bool,
        has_tax_amount: bool,
        has_shipping_charges: bool,
        has_duty_charges: bool,
    ) -> Self {
        if !has_order_details && !has_tax_amount && !has_shipping_charges && !has_duty_charges {
            Self::None
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
