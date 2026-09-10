//! Stored-credential phase axis of `TxProfile`.
//!
//! Splits the COF lifecycle into the four shapes TSYS cert rules
//! actually distinguish:
//!   • `NoCof`           — single-shot transaction
//!   • `CitSetup`        — first-time CIT that stores the card
//!   • `Mit(MitKind)`    — merchant-initiated transaction

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

    pub fn is_mit(self) -> bool {
        matches!(self, Self::Mit(_))
    }

    /// True for any flow that has stored credentials in play (CitSetup,
    pub fn involves_stored_credential(self) -> bool {
        !self.is_no_cof()
    }
}
