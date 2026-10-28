#![recursion_limit = "512"]
#![allow(clippy::result_large_err)]
// Status mapping bodies intentionally isolate legacy expressions before classifying them.
#![allow(clippy::redundant_closure_call)]

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
