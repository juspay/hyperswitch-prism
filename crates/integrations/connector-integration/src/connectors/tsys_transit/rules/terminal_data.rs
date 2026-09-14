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
use super::super::transformers::{
    TsysTransitCardDataInputMode, TsysTransitCardDataOutputCapability, TsysTransitCardDataSource,
    TsysTransitCardPresentDetail, TsysTransitCardholderAuthenticationEntity,
    TsysTransitCardholderAuthenticationMethod, TsysTransitCardholderPresentDetail,
    TsysTransitMaxPinLength, TsysTransitTerminalAuthenticationCapability,
    TsysTransitTerminalCapability, TsysTransitTerminalCardCaptureCapability,
    TsysTransitTerminalOperatingEnvironment, TsysTransitTerminalOutputCapability,
};

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

/// The fully-resolved `terminalData` field set: the profile's
/// `TerminalDataBlock` defaults with any merchant-supplied
/// `terminal_overrides` merged on top (overrides win).
pub struct ResolvedTerminalData {
    pub card_data_source: TsysTransitCardDataSource,
    pub terminal_capability: TsysTransitTerminalCapability,
    pub terminal_operating_environment: TsysTransitTerminalOperatingEnvironment,
    pub cardholder_authentication_method: TsysTransitCardholderAuthenticationMethod,
    pub terminal_authentication_capability: TsysTransitTerminalAuthenticationCapability,
    pub terminal_output_capability: TsysTransitTerminalOutputCapability,
    pub max_pin_length: TsysTransitMaxPinLength,
    pub terminal_card_capture_capability: TsysTransitTerminalCardCaptureCapability,
    pub cardholder_present_detail: TsysTransitCardholderPresentDetail,
    pub card_present_detail: TsysTransitCardPresentDetail,
    pub card_data_input_mode: TsysTransitCardDataInputMode,
    pub cardholder_authentication_entity: TsysTransitCardholderAuthenticationEntity,
    pub card_data_output_capability: TsysTransitCardDataOutputCapability,
}

/// Merge merchant `terminal_overrides` on top of the profile's
/// `terminal_data` (TerminalDataBlock). Shared verbatim by the Authorize
/// and CardAuthentication request builders so the override-precedence and
/// rule fallbacks stay identical across both.
pub fn resolve(
    profile: &TxProfile,
    terminal_data: &TerminalDataBlock,
    cvv_present: bool,
) -> ResolvedTerminalData {
    // cardDataSource follows the acceptance CHANNEL, not the recurring
    // classification: a recurring MOTO card-on-file txn keeps PHONE/MAIL.
    // TSYS rejects `cardDataSource=RECURRING` on MOTO, so recurring/
    // installment profiles use the channel-derived value. Moto/Ecom blocks
    // already carry the correct channel value.
    let card_data_source = match profile.acceptance {
        AcceptanceProfile::RecurringMit | AcceptanceProfile::InstallmentMit => {
            profile.channel_source
        }
        _ => terminal_data.card_data_source,
    };

    ResolvedTerminalData {
        card_data_source,
        terminal_capability: terminal_data.terminal_capability.clone(),
        terminal_operating_environment: terminal_data.terminal_operating_environment.clone(),
        cardholder_authentication_method: terminal_data.cardholder_authentication_method.clone(),
        terminal_authentication_capability: terminal_data
            .terminal_authentication_capability
            .clone(),
        terminal_output_capability: terminal_data.terminal_output_capability.clone(),
        max_pin_length: terminal_data.max_pin_length.clone(),
        terminal_card_capture_capability: terminal_data.terminal_card_capture_capability.clone(),
        cardholder_present_detail: super::cardholder::cardholder_present_detail(
            profile,
            terminal_data,
        ),
        card_present_detail: TsysTransitCardPresentDetail::CardNotPresent,
        card_data_input_mode: super::card_input_mode::card_data_input_mode(
            profile,
            terminal_data,
            cvv_present,
        ),
        cardholder_authentication_entity: terminal_data.cardholder_authentication_entity.clone(),
        card_data_output_capability: terminal_data.card_data_output_capability.clone(),
    }
}
