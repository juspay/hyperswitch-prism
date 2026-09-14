//! Rule: `cardholderPresentDetail`.
//!
//! Always derived from the acceptance profile + cof_phase. The cert
//! csv pushback on this field is:
//!   • "cardholderPresentDetail tag must be set to
//!     'CARDHOLDER_NOT_PRESENT_PHONE_TRANSACTION' on the 2.00 Mastercard
//!     transaction in step 5." → Phone for MOTO regardless of COF state.
//!   • Various rows on step 5 MC / Visa / Discover / AMEX COF transactions
//!     all map to Phone or Mail (channel-driven), never to
//!     RecurringTransaction unless the merchant explicitly asked for
//!     `MitCategory::Recurring`/`Installment`.
//!
//! The acceptance profile already encodes this: MotoPhone⇒Phone,
//! MotoMail⇒Mail, RecurringMit⇒Recurring, InstallmentMit⇒Installment.
//! No card_family / cof_phase override is needed.

use super::super::profile::{TerminalDataBlock, TxProfile};
use super::super::transformers::TsysTransitCardholderPresentDetail;

pub fn cardholder_present_detail(
    _profile: &TxProfile,
    terminal_data: &TerminalDataBlock,
) -> TsysTransitCardholderPresentDetail {
    terminal_data.default_cardholder_present_detail.clone()
}
