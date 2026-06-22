//! Stored-credential phase axis of `TxProfile`.
//!
//! Splits the COF lifecycle into the four shapes TSYS cert rules
//! actually distinguish:
//!   • `NoCof`           — single-shot transaction
//!   • `CitSetup`        — first-time CIT that stores the card
//!   • `CitUsingStored`  — CIT re-using a stored card (consumer is present
//!                          but the card was already on file)
//!   • `Mit(MitKind)`    — merchant-initiated transaction
//!
//! The `CitUsingStored` vs `Mit` distinction is critical for the cert:
//! the MOTO step-5 "25.50 Visa COF" row is a CIT-using-stored, and TSYS
//! rejects it if we send `cardOnFile` / `cardOnFileTransactionIdentifier`
//! (those are MIT-only).

use common_enums::{FutureUsage, MitCategory};
use domain_types::connector_types::{MandateIds, MandateReferenceId};

/// What kind of future MIT a CIT-setup is preparing for. Drives
/// `citStatusIndicator` values (e.g. Mastercard C101 / C102 / C103 / C104).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MitIntent {
    Unscheduled,
    Recurring,
    Installment,
}

/// The kind of MIT being run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MitKind {
    Unscheduled,
    Recurring,
    Installment,
    Resubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CofPhase {
    /// One-shot transaction with no stored-credential involvement.
    NoCof,
    /// CIT that stores the card on file for future MIT use.
    CitSetup { intended_kind: MitIntent },
    /// CIT that re-uses a card already on file.
    CitUsingStored,
    /// Merchant-initiated transaction.
    Mit(MitKind),
}

impl CofPhase {
    /// Derive from the four signals available on `PaymentsAuthorizeData`:
    ///   • mandate present → MIT or CIT-using-stored
    ///   • `off_session=true` → MIT (vs CIT)
    ///   • `setup_future_usage=OffSession` (without mandate) → CIT-setup
    ///   • `mit_category` → disambiguates MIT kind / CIT-setup intent
    pub fn derive(
        mandate_id: Option<&MandateIds>,
        mit_category: Option<MitCategory>,
        setup_future_usage: Option<FutureUsage>,
        off_session: Option<bool>,
    ) -> Self {
        let has_mandate = mandate_id
            .and_then(|m| m.mandate_reference_id.as_ref())
            .is_some_and(|r| {
                matches!(
                    r,
                    MandateReferenceId::ConnectorMandateId(_)
                        | MandateReferenceId::NetworkMandateId(_)
                )
            });

        if has_mandate {
            // off_session=true (or any MIT category set) ⇒ MIT.
            // Otherwise consumer is present and just re-using the stored
            // card (CitUsingStored — MOTO step-5 25.50 Visa COF case).
            let is_mit = off_session == Some(true) || mit_category.is_some();
            if !is_mit {
                return Self::CitUsingStored;
            }
            let kind = match mit_category {
                Some(MitCategory::Recurring) => MitKind::Recurring,
                Some(MitCategory::Installment) => MitKind::Installment,
                Some(MitCategory::Resubmission) => MitKind::Resubmission,
                Some(MitCategory::Unscheduled) | None => MitKind::Unscheduled,
            };
            return Self::Mit(kind);
        }

        let is_setup =
            setup_future_usage == Some(FutureUsage::OffSession) || off_session == Some(true);
        if is_setup {
            let intended_kind = match mit_category {
                Some(MitCategory::Recurring) => MitIntent::Recurring,
                Some(MitCategory::Installment) => MitIntent::Installment,
                _ => MitIntent::Unscheduled,
            };
            return Self::CitSetup { intended_kind };
        }

        Self::NoCof
    }

    pub fn is_no_cof(self) -> bool {
        matches!(self, Self::NoCof)
    }

    pub fn is_cit_setup(self) -> bool {
        matches!(self, Self::CitSetup { .. })
    }

    pub fn is_cit_using_stored(self) -> bool {
        matches!(self, Self::CitUsingStored)
    }

    pub fn is_mit(self) -> bool {
        matches!(self, Self::Mit(_))
    }

    /// True for any flow that has stored credentials in play (CitSetup,
    /// CitUsingStored or any MIT). Useful for cardDataInputMode gating.
    pub fn involves_stored_credential(self) -> bool {
        !self.is_no_cof()
    }
}
