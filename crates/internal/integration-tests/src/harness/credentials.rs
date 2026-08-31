use std::path::PathBuf;

pub use connector_creds::CredentialError;

/// Holds the fully-formed `x-connector-config` header JSON value for one connector.
///
/// The JSON has the shape:
/// ```json
/// {"config":{"Stripe":{"api_key":"sk_test_..."}}}
/// ```
/// where the variant name is PascalCase (first letter capitalised) matching the
/// proto `ConnectorSpecificConfig` oneof serde representation.
#[derive(Clone, Debug)]
pub struct ConnectorConfig {
    header_json: String,
}

impl ConnectorConfig {
    /// Returns the JSON string suitable for the `x-connector-config` header.
    pub fn header_value(&self) -> &str {
        &self.header_json
    }

    /// Constructs a [`ConnectorConfig`] directly from a pre-built header JSON
    /// string.  Primarily intended for testing.
    #[cfg(test)]
    pub fn from_header_json(header_json: String) -> Self {
        Self { header_json }
    }
}

/// Default local credentials path used when env overrides are not set.
fn default_creds_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../creds.json")
}

/// Resolves credentials path from env, then falls back to repo default.
pub(crate) fn creds_file_path() -> PathBuf {
    std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .or_else(|_| std::env::var("UCS_CREDS_PATH"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_creds_path())
}

/// Loads the connector's credentials from the configured creds file and
/// returns a [`ConnectorConfig`] whose [`ConnectorConfig::header_value`]
/// can be sent directly as the `x-connector-config` gRPC metadata header.
///
/// Reading, normalising and validating the entry lives in the `connector-creds`
/// crate, shared with the `grpc-server` tests so both consumers agree on the
/// file's shape.
pub fn load_connector_config(connector: &str) -> Result<ConnectorConfig, CredentialError> {
    let header_json = connector_creds::connector_config_header(&creds_file_path(), connector)?;
    Ok(ConnectorConfig { header_json })
}
