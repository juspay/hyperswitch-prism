//! Reads the shared CI credentials file and produces the `x-connector-config`
//! header a real caller sends.
//!
//! One file backs both consumers — the Jenkins report pipeline and the GitHub
//! Actions workflow — and both need the same three adjustments before the entry
//! is a valid [`ConnectorSpecificConfig`]:
//!
//! - secrets are stored as `{"value": "…"}`, while the proto fields are
//!   `Secret<String>`, whose `Deserialize` delegates to `String` and so rejects
//!   a map;
//! - a `metadata` sibling is carried for tooling that is not the config;
//! - the config is a oneof, so the entry has to be wrapped under its
//!   PascalCase variant name.
//!
//! Keeping this in one place is the point: a second copy that skips any of the
//! three fails only at runtime, against a live sandbox, with an error that
//! names a serde type rather than the omission.

use std::{collections::HashSet, fs, path::Path};

use grpc_api_types::payments::ConnectorSpecificConfig;

/// Non-config fields carried alongside the credentials.
const STRIP_FIELDS: &[&str] = &["metadata"];

/// Credential loading and validation failures, named by connector.
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
    #[error(
        "Connector '{0}' is in the legacy HyperSwitch shape (connector_account_details); \
         the credentials file must hold proto-native ConnectorSpecificConfig entries"
    )]
    LegacyFormat(String),
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

/// Unwraps `{"value": "…"}` into the string it holds, recursively. Every other
/// shape is returned unchanged.
fn normalize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            if map.len() == 1 {
                if let Some(inner) = map.remove("value") {
                    return normalize_value(inner);
                }
            }
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

/// Pulls one connector's entry out of the parsed file. Array-valued entries use
/// their first element; the legacy HyperSwitch wrapper is rejected by name so
/// the wrong file is diagnosed here rather than as a field-level type error.
fn extract_connector_block(
    root: &serde_json::Value,
    connector: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, CredentialError> {
    let entry = root
        .get(connector)
        .ok_or_else(|| CredentialError::ConnectorNotFound(connector.to_string()))?;

    let base = match entry {
        serde_json::Value::Array(arr) => arr
            .first()
            .ok_or_else(|| CredentialError::EmptyCredentials(connector.to_string()))?,
        other => other,
    };

    let obj = base
        .as_object()
        .ok_or_else(|| CredentialError::EmptyCredentials(connector.to_string()))?;

    if obj.contains_key("connector_account_details") {
        return Err(CredentialError::LegacyFormat(connector.to_string()));
    }

    Ok(obj.clone())
}

/// Builds the `x-connector-config` header value for one connector, and proves it
/// deserializes into [`ConnectorSpecificConfig`] before returning it.
///
/// Validating here means a malformed entry names itself and its connector,
/// instead of surfacing later as a connector-side auth rejection that reads like
/// a genuine defect.
pub fn connector_config_header(
    creds_file: &Path,
    connector: &str,
) -> Result<String, CredentialError> {
    let content = fs::read_to_string(creds_file)?;
    let root: serde_json::Value = serde_json::from_str(&content)?;

    let mut block = extract_connector_block(&root, connector)?;

    let strip: HashSet<&str> = STRIP_FIELDS.iter().copied().collect();
    block.retain(|k, _| !strip.contains(k.as_str()));

    if block.is_empty() {
        return Err(CredentialError::EmptyCredentials(connector.to_string()));
    }

    let normalized: serde_json::Map<String, serde_json::Value> = block
        .into_iter()
        .map(|(k, v)| (k, normalize_value(v)))
        .collect();

    let wrapped = serde_json::json!({
        "config": {
            pascal_connector_name(connector): serde_json::Value::Object(normalized)
        }
    });

    serde_json::from_value::<ConnectorSpecificConfig>(wrapped.clone()).map_err(|error| {
        CredentialError::InvalidConfig(connector.to_string(), error.to_string())
    })?;

    Ok(wrapped.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write(contents: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().expect("temp file");
        file.write_all(contents.as_bytes()).expect("write");
        file
    }

    #[test]
    fn pascal_names() {
        assert_eq!(pascal_connector_name("stripe"), "Stripe");
        assert_eq!(pascal_connector_name("authorizedotnet"), "Authorizedotnet");
        assert_eq!(pascal_connector_name(""), "");
    }

    #[test]
    fn normalize_unwraps_value_objects_but_leaves_others() {
        assert_eq!(
            normalize_value(serde_json::json!({"api_key": {"value": "sk"}, "other": "x"})),
            serde_json::json!({"api_key": "sk", "other": "x"})
        );
        assert_eq!(
            normalize_value(serde_json::json!({"a": "b"})),
            serde_json::json!({"a": "b"})
        );
    }

    // The regression this crate exists for: proto config fields are
    // `Secret<String>`, whose Deserialize delegates to String, so a `{"value":…}`
    // wrapper reaching the deserializer fails with "invalid type: map, expected
    // a string". Five connectors hit that when a second loader skipped the
    // unwrapping.
    #[test]
    fn value_wrapped_secrets_are_accepted() {
        let file = write(r#"{"stripe":{"api_key":{"value":"sk_test_123"}}}"#);
        let header = connector_config_header(file.path(), "stripe").expect("should load");
        assert_eq!(header, r#"{"config":{"Stripe":{"api_key":"sk_test_123"}}}"#);
    }

    #[test]
    fn metadata_sibling_is_stripped() {
        let file = write(r#"{"stripe":{"api_key":"sk","metadata":{"google_pay":{}}}}"#);
        let header = connector_config_header(file.path(), "stripe").expect("should load");
        assert!(!header.contains("metadata"), "got {header}");
    }

    #[test]
    fn legacy_hyperswitch_shape_is_named_not_guessed() {
        let file = write(r#"{"stripe":{"connector_account_details":{"api_key":"sk"}}}"#);
        let result = connector_config_header(file.path(), "stripe");
        assert!(
            matches!(result, Err(CredentialError::LegacyFormat(_))),
            "legacy shape must be rejected, got {result:?}"
        );
    }

    #[test]
    fn array_entries_use_the_first_element() {
        let file = write(r#"{"stripe":[{"api_key":"first"},{"api_key":"second"}]}"#);
        let header = connector_config_header(file.path(), "stripe").expect("should load");
        assert!(header.contains("first"), "got {header}");
    }

    #[test]
    fn missing_and_empty_connectors_are_distinguished() {
        let file = write(r#"{"stripe":{}}"#);
        assert!(matches!(
            connector_config_header(file.path(), "stripe"),
            Err(CredentialError::EmptyCredentials(_))
        ));
        assert!(matches!(
            connector_config_header(file.path(), "adyen"),
            Err(CredentialError::ConnectorNotFound(_))
        ));
    }
}
