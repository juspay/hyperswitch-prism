//! Common credential loading utilities for test files
//!
//! The credentials file holds one entry per connector in the proto-native
//! `ConnectorSpecificConfig` shape:
//!
//! ```json
//! { "stripe": { "api_key": "sk_test_..." } }
//! ```
//!
//! Tests forward that entry verbatim as the `x-connector-config` header, which
//! is the path a real caller uses and the one connector transformers consume —
//! 97 of them implement `TryFrom<&ConnectorSpecificConfig>` directly. The value
//! is validated here so a malformed or missing entry fails with a clear message
//! instead of an opaque rejection from the server.

#![allow(dead_code)]

use std::fs;

/// Path to the credentials file — environment variable in CI, relative path locally.
fn get_creds_file_path() -> String {
    std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .unwrap_or_else(|_| "../../.github/test/creds.json".to_string())
}

/// Error type for credential loading operations
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Failed to read credentials file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Failed to parse credentials JSON: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Connector '{0}' not found in credentials")]
    ConnectorNotFound(String),
    #[error("Connector '{0}' has an invalid config: {1}")]
    InvalidConfig(String, String),
}

/// PascalCase variant name used by the `ConnectorSpecificConfig` oneof. Proto
/// field names are all-lowercase, so capitalising the first letter is enough.
fn pascal_connector_name(connector: &str) -> String {
    let mut chars = connector.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Builds the `x-connector-config` header value for one connector.
///
/// Returns the JSON exactly as the header expects it:
/// `{"config":{"Stripe":{"api_key":"..."}}}`
pub fn connector_config_header(connector_name: &str) -> Result<String, CredentialError> {
    let content = fs::read_to_string(get_creds_file_path())?;
    let root: serde_json::Value = serde_json::from_str(&content)?;

    let entry = root
        .get(connector_name)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector_name.to_string()))?;

    let wrapped = serde_json::json!({
        "config": { pascal_connector_name(connector_name): entry }
    });

    // Validate before sending, so a bad entry names itself here rather than
    // surfacing as a connector-side auth failure that looks like a real defect.
    serde_json::from_value::<grpc_api_types::payments::ConnectorSpecificConfig>(wrapped.clone())
        .map_err(|e| CredentialError::InvalidConfig(connector_name.to_string(), e.to_string()))?;

    Ok(wrapped.to_string())
}
