#![allow(clippy::result_large_err)]

pub mod authenticator_connectors;
pub mod common_macros;
pub mod connectors;
pub mod default_implementations;
pub mod payout_connectors;
pub mod surcharge_connectors;
pub mod types;
pub mod utils;
pub mod webhook_utils;

#[cfg(test)]
mod typed_observability_lint;

pub use domain_types::errors;
pub use domain_types::{ConnectorError, IntegrationError};
