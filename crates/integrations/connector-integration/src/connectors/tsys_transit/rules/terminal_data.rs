//! Rule: `terminalData` block selection.
//!
//! Starts from `AcceptanceProfile::terminal_data` and applies network-
//! specific overrides that depend on `CardFamily`:
//!
//!   • Mastercard recurring uses `NO_TERMINAL` (per the v6.2 script's
//!     recurring tab "see top requirement section" callout). The base
//!     recurring block uses `OFF_MERCHANT_PREMISES_UNATTENDED` (the
//!     non-MC default); this rule swaps it for MC.

use super::super::profile::{AcceptanceProfile, CardFamily, TerminalDataBlock, TxProfile};
use super::super::transformers::TsysTransitTerminalOperatingEnvironment;

/// Resolve the final `TerminalDataBlock` for this profile, applying any
/// card-family overrides on top of the acceptance-profile defaults.
pub fn terminal_data(profile: &TxProfile) -> TerminalDataBlock {
    let mut block = profile.acceptance.terminal_data();

    if matches!(profile.acceptance, AcceptanceProfile::RecurringMit)
        && matches!(profile.card_family, CardFamily::Mastercard)
    {
        block.terminal_operating_environment = TsysTransitTerminalOperatingEnvironment::NoTerminal;
    }

    block
}
