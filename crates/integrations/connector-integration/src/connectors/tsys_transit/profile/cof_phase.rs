//! Stored-credential phase axis of `TxProfile`.
//!
//! Splits the COF lifecycle into the four shapes TSYS cert rules
//! actually distinguish: no COF involvement; first-time CIT that stores
//! the card; CIT that re-uses a stored card; or MIT of a specific kind.

use common_enums::{FutureUsage, MitCategory};
use domain_types::connector_types::{MandateIds, MandateReferenceId};

/// What kind of future MIT a CIT-setup is preparing for. Drives
/// `citStatusIndicator` values (e.g. Mastercard C101/C102/C103/C104).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MitIntent {
    Unscheduled,
    Recurring,
    Installment,
}

/// The kind of MIT being run.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MitKind {
    Unscheduled,
    Recurring,
    Installment,
    Resubmission,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[allow(dead_code)]
impl CofPhase {
    /// Derive from the three signals available on `PaymentsAuthorizeData`:
    /// the mandate (present → MIT or CIT-using-stored), the setup-future-usage
    /// flag (off-session → CIT-setup), and the requested MIT category (which
    /// disambiguates the MIT kind).
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

        let is_setup = setup_future_usage == Some(FutureUsage::OffSession) || off_session == Some(true);
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
}
