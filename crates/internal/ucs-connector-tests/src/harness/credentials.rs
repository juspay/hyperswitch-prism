use std::{fs, path::PathBuf};

/// Connector authentication shapes supported by the harness.
#[derive(Clone, Debug)]
pub enum ConnectorAuth {
    HeaderKey {
        api_key: String,
    },
    BodyKey {
        api_key: String,
        key1: String,
    },
    SignatureKey {
        api_key: String,
        key1: String,
        api_secret: String,
    },
}

/// Credential loading/validation failures surfaced with connector context.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Failed to read credentials file: {0}")]
    FileRead(#[from] std::io::Error),
    #[error("Failed to parse credentials file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Connector '{0}' not found in credentials file")]
    ConnectorNotFound(String),
    #[error("Missing connector_account_details for '{0}'")]
    MissingAccountDetails(String),
    #[error("Invalid auth_type '{auth_type}' for '{connector}'")]
    InvalidAuthType {
        connector: String,
        auth_type: String,
    },
    #[error("Missing field '{field}' for connector '{connector}'")]
    MissingField { connector: String, field: String },
}

/// Default local credentials path used when env overrides are not set.
fn default_creds_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/test/creds.json")
}

/// Resolves credentials path from env, then falls back to repo default.
fn creds_file_path() -> PathBuf {
    std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .or_else(|_| std::env::var("UCS_CREDS_PATH"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_creds_path())
}

/// Extracts `connector_account_details` for a connector with compatibility for
/// both flat and nested credential file layouts.
fn extract_account_details<'a>(
    root: &'a serde_json::Value,
    connector: &str,
) -> Result<&'a serde_json::Value, CredentialError> {
    let connector_value = root
        .get(connector)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector.to_string()))?;

    if let Some(account_details) = connector_value.get("connector_account_details") {
        return Ok(account_details);
    }

    if let Some(connector_obj) = connector_value.as_object() {
        // Allow forcing a specific connector label (connector_1/connector_2)
        // through env var when multiple accounts exist.
        let env_key = format!(
            "UCS_CONNECTOR_LABEL_{}",
            connector.to_ascii_uppercase().replace('-', "_")
        );

        if let Ok(label) = std::env::var(&env_key) {
            if let Some(account_details) = connector_obj
                .get(&label)
                .and_then(|value| value.get("connector_account_details"))
            {
                return Ok(account_details);
            }
        }

        let preferred_labels: &[&str] = if connector == "cybersource" {
            &["connector_2", "connector_1"]
        } else {
            &["connector_1", "connector_2"]
        };

        // Prefer deterministic labels first, then fall back to any nested
        // account details to stay resilient to file shape drift.
        for label in preferred_labels {
            if let Some(account_details) = connector_obj
                .get(*label)
                .and_then(|value| value.get("connector_account_details"))
            {
                return Ok(account_details);
            }
        }

        for nested_value in connector_obj.values() {
            if let Some(account_details) = nested_value.get("connector_account_details") {
                return Ok(account_details);
            }
        }
    }

    Err(CredentialError::MissingAccountDetails(
        connector.to_string(),
    ))
}

/// Reads a required string field from connector account details.
fn get_required_string(
    account_details: &serde_json::Value,
    connector: &str,
    field: &str,
) -> Result<String, CredentialError> {
    account_details
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CredentialError::MissingField {
            connector: connector.to_string(),
            field: field.to_string(),
        })
        .map(ToString::to_string)
}

/// Loads connector auth credentials from the configured credential file.
///
/// Supported auth_type values are normalized for underscore/hyphen variants.
pub fn load_connector_auth(connector: &str) -> Result<ConnectorAuth, CredentialError> {
    let content = fs::read_to_string(creds_file_path())?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    let account_details = extract_account_details(&json, connector)?;

    let auth_type = account_details
        .get("auth_type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CredentialError::MissingField {
            connector: connector.to_string(),
            field: "auth_type".to_string(),
        })?;

    let normalized_auth_type = auth_type.to_ascii_lowercase();

    match normalized_auth_type.as_str() {
        "header_key" | "header-key" | "headerkey" => Ok(ConnectorAuth::HeaderKey {
            api_key: get_required_string(account_details, connector, "api_key")?,
        }),
        "body_key" | "body-key" | "bodykey" => Ok(ConnectorAuth::BodyKey {
            api_key: get_required_string(account_details, connector, "api_key")?,
            key1: get_required_string(account_details, connector, "key1")?,
        }),
        "signature_key" | "signature-key" | "signaturekey" => Ok(ConnectorAuth::SignatureKey {
            api_key: get_required_string(account_details, connector, "api_key")?,
            key1: get_required_string(account_details, connector, "key1")?,
            api_secret: get_required_string(account_details, connector, "api_secret")?,
        }),
        _ => Err(CredentialError::InvalidAuthType {
            connector: connector.to_string(),
            auth_type: auth_type.to_string(),
        }),
    }
}
