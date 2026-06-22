//! Acceptance-profile axis of `TxProfile`.
//!
//! Channel + recurring-or-not selects a fixed block of TSYS `terminalData`
//! tags (operating environment, output capability, default input mode,
//! default cardholder-present detail, etc.). This is the level the cert
//! script's per-tab "see top requirement section for proper terminalData
//! tags & values" callouts operate at.
//!
//! Each variant maps to one `TerminalDataBlock` via [`Self::terminal_data`].
//! That block defines the "static" terminalData; per-tx rules (in `rules/`)
//! may override individual fields based on card_family / cof_phase.

use common_enums::{MitCategory, PaymentChannel};

use super::super::transformers::{
    TsysTransitCardDataInputMode, TsysTransitCardDataOutputCapability, TsysTransitCardDataSource,
    TsysTransitCardholderAuthenticationEntity, TsysTransitCardholderAuthenticationMethod,
    TsysTransitCardholderPresentDetail, TsysTransitMaxPinLength,
    TsysTransitTerminalAuthenticationCapability, TsysTransitTerminalCapability,
    TsysTransitTerminalCardCaptureCapability, TsysTransitTerminalOperatingEnvironment,
    TsysTransitTerminalOutputCapability,
};

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
/// acceptance profile. Selected by `AcceptanceProfile::terminal_data`.
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

    /// Fixed terminalData block for this acceptance profile.
    ///
    /// The cert script's "see top requirement section for proper
    /// terminalData tags & values" callouts come from this table.
    /// Per-row pushback in `TSYS certification issues - MOTO.csv`
    /// corresponds directly to entries here for MotoPhone/MotoMail.
    pub fn terminal_data(self) -> TerminalDataBlock {
        // Shorthand aliases (kept fully-qualified at use sites because
        // several enums share variant names like `NotAuthenticated` and
        // `NoCapability`).
        type Cap = TsysTransitTerminalCapability;
        type Env = TsysTransitTerminalOperatingEnvironment;
        type Out = TsysTransitTerminalOutputCapability;
        type AuthCap = TsysTransitTerminalAuthenticationCapability;
        type Pin = TsysTransitMaxPinLength;
        type CardCap = TsysTransitTerminalCardCaptureCapability;
        type AuthMethod = TsysTransitCardholderAuthenticationMethod;
        type AuthEnt = TsysTransitCardholderAuthenticationEntity;
        type DataOut = TsysTransitCardDataOutputCapability;
        type DataIn = TsysTransitCardDataInputMode;
        type Present = TsysTransitCardholderPresentDetail;
        type Source = TsysTransitCardDataSource;

        match self {
            // ── MOTO PHONE ─────────────────────────────────────────────
            // Cert pushback rows on MOTO csv (paraphrased):
            //   • terminalOperatingEnvironment must be
            //     ON_MERCHANT_PREMISES_ATTENDED on all transactions
            //   • terminalOutputCapability must be DISPLAY_ONLY
            //   • cardholderPresentDetail must be
            //     CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION
            //   • cardDataSource must be PHONE
            //   • cardDataInputMode must be KEY_ENTERED_INPUT
            //     (overridden to MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB
            //     when CVV is sent on AMEX/JCB; that's handled in
            //     rules::card_input_mode, not here)
            Self::MotoPhone => TerminalDataBlock {
                card_data_source: Source::Phone,
                terminal_operating_environment: Env::OnMerchantPremisesAttended,
                terminal_output_capability: Out::DisplayOnly,
                terminal_capability: Cap::KeyedEntryOnly,
                terminal_authentication_capability: AuthCap::NoCapability,
                max_pin_length: Pin::NotSupported,
                terminal_card_capture_capability: CardCap::NoCapability,
                cardholder_authentication_method: AuthMethod::NotAuthenticated,
                cardholder_authentication_entity: AuthEnt::NotAuthenticated,
                card_data_output_capability: DataOut::None,
                default_card_data_input_mode: DataIn::KeyEnteredInput,
                default_cardholder_present_detail: Present::CardholderNotPresentPhoneTransaction,
            },
            // ── MOTO MAIL ──────────────────────────────────────────────
            Self::MotoMail => TerminalDataBlock {
                card_data_source: Source::Mail,
                terminal_operating_environment: Env::OnMerchantPremisesAttended,
                terminal_output_capability: Out::DisplayOnly,
                terminal_capability: Cap::KeyedEntryOnly,
                terminal_authentication_capability: AuthCap::NoCapability,
                max_pin_length: Pin::NotSupported,
                terminal_card_capture_capability: CardCap::NoCapability,
                cardholder_authentication_method: AuthMethod::NotAuthenticated,
                cardholder_authentication_entity: AuthEnt::NotAuthenticated,
                card_data_output_capability: DataOut::None,
                default_card_data_input_mode: DataIn::KeyEnteredInput,
                default_cardholder_present_detail: Present::CardholderNotPresentMailTransaction,
            },
            // ── E-COMMERCE INTERNET ────────────────────────────────────
            Self::EcomInternet => TerminalDataBlock {
                card_data_source: Source::Internet,
                terminal_operating_environment: Env::NoTerminal,
                terminal_output_capability: Out::None,
                terminal_capability: Cap::KeyedEntryOnly,
                terminal_authentication_capability: AuthCap::NoCapability,
                max_pin_length: Pin::NotSupported,
                terminal_card_capture_capability: CardCap::NoCapability,
                cardholder_authentication_method: AuthMethod::NotAuthenticated,
                cardholder_authentication_entity: AuthEnt::NotAuthenticated,
                card_data_output_capability: DataOut::None,
                default_card_data_input_mode: DataIn::PanEntryElectronicCommerceIncludingRemoteChip,
                default_cardholder_present_detail: Present::CardholderNotPresentElectronicCommerce,
            },
            // ── RECURRING MIT ──────────────────────────────────────────
            // Mastercard recurring uses NO_TERMINAL; others use
            // OFF_MERCHANT_PREMISES_UNATTENDED. This base value matches
            // the non-MC default; rules::terminal_data flips it back to
            // NoTerminal for MC recurring.
            Self::RecurringMit => TerminalDataBlock {
                card_data_source: Source::Recurring,
                terminal_operating_environment: Env::OffMerchantPremisesUnattended,
                terminal_output_capability: Out::DisplayOnly,
                terminal_capability: Cap::KeyedEntryOnly,
                terminal_authentication_capability: AuthCap::NoCapability,
                max_pin_length: Pin::NotSupported,
                terminal_card_capture_capability: CardCap::NoCapability,
                cardholder_authentication_method: AuthMethod::NotAuthenticated,
                cardholder_authentication_entity: AuthEnt::NotAuthenticated,
                card_data_output_capability: DataOut::None,
                default_card_data_input_mode:
                    DataIn::MerchantInitiatedTransactionCardCredentialStoredOnFile,
                default_cardholder_present_detail:
                    Present::CardholderNotPresentRecurringTransaction,
            },
            // ── INSTALLMENT MIT ────────────────────────────────────────
            Self::InstallmentMit => TerminalDataBlock {
                card_data_source: Source::Recurring,
                terminal_operating_environment: Env::OffMerchantPremisesUnattended,
                terminal_output_capability: Out::DisplayOnly,
                terminal_capability: Cap::KeyedEntryOnly,
                terminal_authentication_capability: AuthCap::NoCapability,
                max_pin_length: Pin::NotSupported,
                terminal_card_capture_capability: CardCap::NoCapability,
                cardholder_authentication_method: AuthMethod::NotAuthenticated,
                cardholder_authentication_entity: AuthEnt::NotAuthenticated,
                card_data_output_capability: DataOut::None,
                default_card_data_input_mode:
                    DataIn::MerchantInitiatedTransactionCardCredentialStoredOnFile,
                default_cardholder_present_detail:
                    Present::CardholderNotPresentInstallmentTransaction,
            },
        }
    }

    /// True when this transaction was accepted via a MOTO channel
    /// (phone or mail). Several cert rules key off this.
    pub fn is_moto(self) -> bool {
        matches!(self, Self::MotoPhone | Self::MotoMail)
    }

    /// True when this transaction is a recurring or installment MIT.
    pub fn is_scheduled_mit(self) -> bool {
        matches!(self, Self::RecurringMit | Self::InstallmentMit)
    }
}
