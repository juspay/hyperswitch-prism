//! Dummy connector mock — business logic per flow.
//!
//! Each `<flow>.rs` implements one operation: takes a Stripe-shape request
//! struct (defined alongside the matching handler in `crate::handlers`),
//! mutates `AppState` as needed, and returns a Stripe-shape response struct
//! from `crate::types` (or `Result<_, axum::response::Response>` when the
//! flow can produce a typed HTTP error).

pub mod authorize;
pub mod capture;
pub mod refund;
pub mod refund_sync;
pub mod sync;
pub mod void;
