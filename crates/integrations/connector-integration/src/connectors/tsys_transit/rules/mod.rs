//! Per-wire-field rule functions.
//!
//! Each function takes a `&TxProfile` (and the raw merchant-supplied value
//! where relevant) and returns the value that should land on the wire — or
//! `None` when the profile says "do not send this tag".
//!
//! The TSYS cert CSV
//! (`TSYS certification issues - MOTO.csv`) maps 1:1 into rule edits.
//! Each rule's doc-comment cites the cert row it satisfies.

pub mod card_input_mode;
pub mod cardholder;
pub mod cof_mit;
pub mod commercial;
pub mod network_indicators;
pub mod terminal_data;
