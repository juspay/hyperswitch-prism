//! Credential loading for the live-sandbox `grpc-server` tests.
//!
//! Tests forward the connector's entry as the `x-connector-config` header —
//! the path a real caller uses, and the one connector transformers consume: 97
//! of them implement `TryFrom<&ConnectorSpecificConfig>` directly.
//!
//! Reading and validating the file lives in the `connector-creds` crate, shared
//! with the integration-test harness, so both consumers normalise the entry the
//! same way. See that crate for what the normalisation covers and why skipping
//! any part of it fails only at runtime.

#![allow(dead_code)]

use std::path::PathBuf;

pub use connector_creds::CredentialError;

/// Path to the credentials file — environment variable in CI, relative path locally.
fn creds_file_path() -> PathBuf {
    std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../.github/test/creds.json"))
}

/// Builds the `x-connector-config` header value for one connector.
///
/// Returns the JSON exactly as the header expects it:
/// `{"config":{"Stripe":{"api_key":"..."}}}`
pub fn connector_config_header(connector_name: &str) -> Result<String, CredentialError> {
    connector_creds::connector_config_header(&creds_file_path(), connector_name)
}
