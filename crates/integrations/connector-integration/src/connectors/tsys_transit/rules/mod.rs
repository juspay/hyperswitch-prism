//! Per-wire-field rule functions.
//!
//! Each function takes a `&TxProfile` (and the raw merchant-supplied
//! value where relevant) and returns the value that should land on the
//! wire — or `None` when the profile says "do not send this tag".
//!
//! Empty for now; the next PR migrates the inline branching out of
//! `transformers::try_from` impls into this module, one field per
//! function with the matching cert CSV row as a comment.
