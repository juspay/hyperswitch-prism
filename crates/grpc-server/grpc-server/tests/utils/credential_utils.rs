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
///
/// When the credentials file exists — which is always the case in CI — a
/// connector missing or malformed in it is a defect, and this panics rather
/// than returning `Err`.
///
/// The distinction matters because of how the error tends to get handled.
/// `Err` invites `.ok()` and an early `return`, which Rust records as a passing
/// test: four Paysafe cases reported PASS in 15ms for exactly that reason,
/// never having issued a request. A panic cannot be turned into a silent pass.
/// Without a credentials file at all — a local checkout — the `Err` is
/// preserved, so tests can still skip off-CI.
// The panic is the point: returning Err here is what let a missing entry become
// a silent pass. Result is kept so a local checkout with no credentials file
// still skips.
#[allow(clippy::panic_in_result_fn)]
pub fn connector_config_header(connector_name: &str) -> Result<String, CredentialError> {
    let path = creds_file_path();
    let result = connector_creds::connector_config_header(&path, connector_name);

    if let Err(error) = &result {
        assert!(
            !path.exists(),
            "credentials file {} is present but '{connector_name}' could not be loaded from it: \
             {error}. In CI this is a defect, not a reason to skip. Fix the entry, or mark the \
             test #[ignore] with a reason so the gap is visible instead of counted as a pass.",
            path.display()
        );
    }

    result
}
