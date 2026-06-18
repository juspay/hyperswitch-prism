//! Acceptance-profile axis of `TxProfile`.
//!
//! Channel + recurring-or-not selects a fixed block of TSYS `terminalData`
//! tags (operating environment, output capability, default input mode,
//! default cardholder-present detail, etc.). This is the level the cert
//! script's per-tab "see top requirement section for proper terminalData
//! tags & values" callouts operate at.

use common_enums::{MitCategory, PaymentChannel};

use super::super::transformers::{
    TsysTransitCardDataInputMode, TsysTransitCardDataOutputCapability, TsysTransitCardDataSource,
    TsysTransitCardholderAuthenticationEntity, TsysTransitCardholderAuthenticationMethod,
    TsysTransitCardholderPresentDetail, TsysTransitMaxPinLength, TsysTransitTerminalAuthenticationCapability,
    TsysTransitTerminalCapability, TsysTransitTerminalCardCaptureCapability,
    TsysTransitTerminalOperatingEnvironment, TsysTransitTerminalOutputCapability,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceProfile {
    /// e-Commerce tab: `cardDataSource = INTERNET`.
    EcomInternet,
    /// Direct-Marketing / MOTO tab, phone variant.
    MotoPhone,
    /// Direct-Marketing / MOTO tab, mail variant.
    MotoMail,
    /// Recurring & Installments tab, recurring (non-installment) schedule.
    RecurringMit,
    /// Recurring & Installments tab, installment schedule.
    InstallmentMit,
}

/// Block of TSYS `terminalData` tags that ride together with an
/// acceptance profile. Populated by `AcceptanceProfile::terminal_data`
/// in the rules-extraction PR.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TerminalDataBlock {
    pub card_data_source: TsysTransitCardDataSource,
    pub terminal_operating_environment: TsysTransitTerminalOperatingEnvironment,
    pub terminal_output_capability: TsysTransitTerminalOutputCapability,
    pub terminal_capability: TsysTransitTerminalCapability,
    pub terminal_authentication_capability: TsysTransitTerminalAuthenticationCapability,
    pub max_pin_length: TsysTransitMaxPinLength,
    pub terminal_card_capture_capability: TsysTransitTerminalCardCaptureCapability,
    pub cardholder_authentication_method: TsysTransitCardholderAuthenticationMethod,
    pub cardholder_authentication_entity: TsysTransitCardholderAuthenticationEntity,
    pub card_data_output_capability: TsysTransitCardDataOutputCapability,
    pub default_card_data_input_mode: TsysTransitCardDataInputMode,
    pub default_cardholder_present_detail: TsysTransitCardholderPresentDetail,
}

#[allow(dead_code)]
impl AcceptanceProfile {
    /// Map `(channel, mit_category)` to the acceptance profile this
    /// transaction lives in. Recurring/installment classification dominates
    /// channel — the cert script puts those in their own tab regardless of
    /// how the merchant first accepted the card.
    pub fn derive(channel: Option<PaymentChannel>, mit_category: Option<MitCategory>) -> Self {
        match mit_category {
            Some(MitCategory::Recurring) => return Self::RecurringMit,
            Some(MitCategory::Installment) => return Self::InstallmentMit,
            _ => {}
        }
        match channel {
            Some(PaymentChannel::TelephoneOrder) => Self::MotoPhone,
            Some(PaymentChannel::MailOrder) => Self::MotoMail,
            Some(PaymentChannel::Ecommerce) | None => Self::EcomInternet,
        }
    }
}
