#![allow(clippy::result_large_err)]
// Future: restore forbid once the temporary payout currency exceptions are resolved.
// #![forbid(clippy::disallowed_methods, clippy::disallowed_types)]
// Temporary: deny permits the scoped payout expectations tracked in PR #2642.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(unfulfilled_lint_expectations)]

pub mod authenticator_connectors;
pub mod common_macros;
pub mod connectors;
pub mod default_implementations;
pub mod frm_connectors;
pub mod payout_connectors;
pub mod sanity;
pub mod surcharge_connectors;
pub mod types;
pub mod utils;
pub mod webhook_utils;

pub use domain_types::errors;
pub use domain_types::{ConnectorError, IntegrationError};
