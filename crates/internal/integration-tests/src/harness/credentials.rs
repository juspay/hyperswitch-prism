use std::{collections::HashSet, fs, path::PathBuf};

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

/// Credential loading/validation failures surfaced with connector context.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Failed to read credentials file: {0}")]
    FileRead(#[from] std::io::Error),
    #[error("Failed to parse credentials file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Connector '{0}' not found in credentials file")]
    ConnectorNotFound(String),
    #[error("Connector '{0}' has an empty credentials block")]
    EmptyCredentials(String),
    #[error("Connector '{0}' has invalid credentials shape: {1}")]
    InvalidCredentials(String, String),
}

/// Non-auth metadata fields present in the creds file that must be stripped
/// before wrapping as a connector config.
const STRIP_FIELDS: &[&str] = &["metadata"];

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

/// Returns the PascalCase variant name used in the `ConnectorSpecificConfig`
/// oneof serde representation.  Proto field names are all-lowercase so a simple
/// first-letter capitalisation is all that is needed.
fn pascal_connector_name(connector: &str) -> String {
    let mut chars = connector.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Recursively normalises a JSON value by unwrapping single-field `{"value": "..."}
/// objects into plain strings.  All other shapes are left intact.
fn normalize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            if map.len() == 1 {
                if let Some(inner) = map.remove("value") {
                    // Single-key object whose only key is "value" → unwrap.
                    return normalize_value(inner);
                }
                // Single-key object with a different key — restore and recurse.
                // (map was partially consumed above; this branch is unreachable
                //  because we only remove when the key IS "value".)
            }
            // Re-insert (map still has all original entries here because the
            // remove above only happens in the branch that returns early).
            serde_json::Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, normalize_value(v)))
                    .collect(),
            )
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(normalize_value).collect())
        }
        other => other,
    }
}

/// Extracts the connector's auth JSON block from the creds file, handling:
/// - Array-valued connectors (picks first entry).
/// - Existing CI `connector_account_details` wrappers.
/// - Nested connector accounts, keyed by environment or account name.
///
/// For the nested form, `default` wins if present; otherwise the
/// lexicographically first key does. Picking whichever key happened to come
/// first in the file would let CI certify against a different account than the
/// one it certified against yesterday.
#[allow(clippy::print_stderr)]
fn extract_connector_block(
    root: &serde_json::Value,
    connector: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, CredentialError> {
    let connector_value = root
        .get(connector)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector.to_string()))?;

    // If the connector entry is an array, use the first element.
    let base = match connector_value {
        serde_json::Value::Array(arr) => arr
            .first()
            .ok_or_else(|| CredentialError::EmptyCredentials(connector.to_string()))?,
        other => other,
    };

    let obj = base
        .as_object()
        .ok_or_else(|| CredentialError::EmptyCredentials(connector.to_string()))?;

    if let Some(details) = obj.get("connector_account_details") {
        return legacy_connector_account_details_to_block(connector, details);
    }

    let mut nested_keys: Vec<&String> = obj
        .iter()
        .filter(|(_, v)| {
            v.as_object()
                .is_some_and(|o| o.contains_key("connector_account_details"))
        })
        .map(|(k, _)| k)
        .collect();
    nested_keys.sort();

    let chosen = nested_keys
        .iter()
        .find(|k| k.as_str() == "default")
        .or_else(|| nested_keys.first());

    if let Some(key) = chosen {
        if let Some(details) = obj
            .get(key.as_str())
            .and_then(|v| v.get("connector_account_details"))
        {
            if nested_keys.len() > 1 {
                eprintln!(
                    "[credentials] '{connector}' has {} nested accounts ({}); using '{key}'",
                    nested_keys.len(),
                    nested_keys
                        .iter()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            return legacy_connector_account_details_to_block(connector, details);
        }
    }

    Ok(obj.clone())
}

fn legacy_connector_account_details_to_block(
    connector: &str,
    details: &serde_json::Value,
) -> Result<serde_json::Map<String, serde_json::Value>, CredentialError> {
    let mut block = details
        .as_object()
        .ok_or_else(|| {
            CredentialError::InvalidCredentials(
                connector.to_string(),
                "connector_account_details must be an object".to_string(),
            )
        })?
        .clone();

    block.remove("auth_type");

    if block.is_empty() {
        return Err(CredentialError::EmptyCredentials(connector.to_string()));
    }

    Ok(block)
}

/// Loads the connector's credentials from the configured creds file and
/// returns a [`ConnectorConfig`] whose [`ConnectorConfig::header_value`]
/// can be sent directly as the `x-connector-config` gRPC metadata header.
pub fn load_connector_config(connector: &str) -> Result<ConnectorConfig, CredentialError> {
    let content = fs::read_to_string(creds_file_path())?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let mut block = extract_connector_block(&json, connector)?;

    // Strip non-auth fields that are not part of the proto config message.
    let strip: HashSet<&str> = STRIP_FIELDS.iter().copied().collect();
    block.retain(|k, _| !strip.contains(k.as_str()));

    if block.is_empty() {
        return Err(CredentialError::EmptyCredentials(connector.to_string()));
    }

    // Normalise {"value": "..."} wrappers → plain strings throughout.
    let normalized: serde_json::Map<String, serde_json::Value> = block
        .into_iter()
        .map(|(k, v)| (k, normalize_value(v)))
        .collect();

    let variant = pascal_connector_name(connector);
    let inner = serde_json::Value::Object(normalized);

    // Build {"config":{"Stripe":{...}}}
    let header_json = serde_json::json!({ "config": { variant: inner } }).to_string();

    Ok(ConnectorConfig { header_json })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_name_stripe() {
        assert_eq!(pascal_connector_name("stripe"), "Stripe");
    }

    #[test]
    fn pascal_name_authorizedotnet() {
        assert_eq!(pascal_connector_name("authorizedotnet"), "Authorizedotnet");
    }

    #[test]
    fn pascal_name_bankofamerica() {
        assert_eq!(pascal_connector_name("bankofamerica"), "Bankofamerica");
    }

    #[test]
    fn normalize_value_wrapper() {
        let v = serde_json::json!({"value": "secret"});
        assert_eq!(normalize_value(v), serde_json::json!("secret"));
    }

    #[test]
    fn normalize_plain_string() {
        let v = serde_json::json!("plain");
        assert_eq!(normalize_value(v), serde_json::json!("plain"));
    }

    #[test]
    fn normalize_nested_object() {
        let v = serde_json::json!({"api_key": {"value": "sk_test"}, "other": "x"});
        let expected = serde_json::json!({"api_key": "sk_test", "other": "x"});
        assert_eq!(normalize_value(v), expected);
    }

    #[test]
    fn strip_metadata_from_block() {
        // Simulate what load_connector_config does for a stripe-like entry.
        let block: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{"api_key":{"value":"sk_test"},"metadata":{"google_pay":{}}}"#)
                .expect("test JSON should parse");

        let strip: HashSet<&str> = STRIP_FIELDS.iter().copied().collect();
        let mut cleaned = block;
        cleaned.retain(|k, _| !strip.contains(k.as_str()));

        assert!(cleaned.contains_key("api_key"));
        assert!(!cleaned.contains_key("metadata"));
    }

    #[test]
    fn extracts_legacy_flat_connector_account_details() {
        let root = serde_json::json!({
            "stripe": {
                "connector_account_details": {
                    "auth_type": "HeaderKey",
                    "api_key": "sk_test"
                },
                "metadata": {
                    "webhook_secret": "whsec"
                }
            }
        });

        let block = extract_connector_block(&root, "stripe").expect("legacy block should load");

        assert_eq!(block.get("api_key"), Some(&serde_json::json!("sk_test")));
        assert!(!block.contains_key("auth_type"));
        assert!(!block.contains_key("metadata"));
    }

    #[test]
    fn extracts_legacy_nested_connector_account_details() {
        let root = serde_json::json!({
            "stripe": {
                "default": {
                    "connector_account_details": {
                        "auth_type": "HeaderKey",
                        "api_key": "sk_test"
                    }
                }
            }
        });

        let block =
            extract_connector_block(&root, "stripe").expect("nested legacy block should load");

        assert_eq!(block.get("api_key"), Some(&serde_json::json!("sk_test")));
        assert!(!block.contains_key("default"));
    }

    #[test]
    fn nested_account_selection_is_deterministic() {
        // No `default`: the lexicographically first account wins, regardless of
        // the order the keys appear in the file.
        let root = serde_json::json!({
            "stripe": {
                "zeta":  { "connector_account_details": { "api_key": "from_zeta" } },
                "alpha": { "connector_account_details": { "api_key": "from_alpha" } }
            }
        });
        let block = extract_connector_block(&root, "stripe").expect("block should load");
        assert_eq!(block.get("api_key"), Some(&serde_json::json!("from_alpha")));

        // `default` takes precedence over an earlier-sorting sibling.
        let root = serde_json::json!({
            "stripe": {
                "alpha":   { "connector_account_details": { "api_key": "from_alpha" } },
                "default": { "connector_account_details": { "api_key": "from_default" } }
            }
        });
        let block = extract_connector_block(&root, "stripe").expect("block should load");
        assert_eq!(
            block.get("api_key"),
            Some(&serde_json::json!("from_default"))
        );
    }
}
