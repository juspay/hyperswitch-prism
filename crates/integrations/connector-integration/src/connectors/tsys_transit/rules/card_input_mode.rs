//! Rule: `cardDataInputMode`.
//!
//! Layered overrides on top of the acceptance-profile default. Order
//! matters — the cert csv documents these as hard precedence rules:
//!
//!   1. **AMEX with CVV** ⇒ `MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB`
//!      Cert pushback rows:
//!      "cardDataInputMode tag must be set to
//!      'MANUALLY_ENTERED_WITH_KEYED_CID_AMEX_JCB' on the 1.50 AMEX
//!      level II transaction in step 2 as the cvv was sent." (and similar
//!      for 4.00 AMEX, 14.50 AMEX).
//!      NOTE: JCB is NOT included — the TSYS_MOTO_V3 reference (row 135,
//!      JCB 13.00 with cvv2) uses `KEY_ENTERED_INPUT`, so JCB falls through
//!      to the channel default below.
//!
//!   2. **CIT-setup that stores credentials** ⇒ `KEY_ENTERED_INPUT`
//!      Cert pushback row:
//!      "cardDataInputMode tag must be set to 'KEY_ENTERED_INPUT' on the
//!      0.00 Visa card authentication in step 5 as this transaction will
//!      be used to store credentials for payment" (and the 2.00 Mastercard
//!      step-5 row).
//!
//!   3. **MIT** ⇒ `MERCHANT_INITIATED_TRANSACTION_CARD_CREDENTIAL_STORED_ON_FILE`
//!
//!   4. Otherwise the channel default from `TerminalDataBlock`.

use super::super::profile::{TerminalDataBlock, TxProfile};
use super::super::transformers::TsysTransitCardDataInputMode;

pub fn card_data_input_mode(
    profile: &TxProfile,
    terminal_data: &TerminalDataBlock,
    cvv_present: bool,
) -> TsysTransitCardDataInputMode {
    use TsysTransitCardDataInputMode::*;

    // 1. AMEX+CVV wins over everything (including CIT-setup / MIT). JCB is
    //    excluded per TSYS_MOTO_V3 (JCB uses the channel default).
    if cvv_present && profile.card_family.is_amex() {
        return ManuallyEnteredWithKeyedCidAmexJcb;
    }

    // 2. CIT-setup that stores credentials.
    if profile.cof_phase.is_cit_setup() {
        return KeyEnteredInput;
    }

    // 3. MIT (any kind) OR CIT-using-stored — both replay a stored
    //    credential, so both carry the STORED_ON_FILE input mode (cert MOTO
    //    25.50 Visa / 29.75 MC CIT-using-stored rows use this too).
    if profile.cof_phase.is_mit() || profile.cof_phase.is_cit_using_stored() {
        return MerchantInitiatedTransactionCardCredentialStoredOnFile;
    }

    // 4. Acceptance-profile default.
    terminal_data.default_card_data_input_mode.clone()
}
