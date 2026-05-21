//! HTTP handlers — thin protocol-layer wrappers that parse Stripe-shape
//! form bodies (via `crate::form::StripeForm`), delegate the actual business
//! logic to `crate::services::dummy::*`, and serialize the response to JSON.

pub mod authorize;
pub mod capture;
pub mod common;
pub mod redirect;
pub mod refund;
pub mod refund_sync;
pub mod sync;
pub mod void;
pub mod webhook_trigger;
