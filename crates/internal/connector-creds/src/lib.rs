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

/// Collects every key the entry carries, as a dotted path, skipping nulls.
fn key_paths(value: &serde_json::Value, prefix: &str, out: &mut Vec<String>) {
    if let serde_json::Value::Object(map) = value {
        for (k, v) in map {
            if v.is_null() {
                continue;
            }
            let path = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            out.push(path.clone());
            key_paths(v, &path, out);
        }
    }
}

/// Reads one connector's entry and shapes it into the `x-connector-config`
/// payload: legacy shape rejected, `metadata` stripped, `{"value": …}` unwrapped,
/// wrapped under the PascalCase oneof variant.
fn build_wrapped_config(
    creds_file: &Path,
    connector: &str,
) -> Result<serde_json::Value, CredentialError> {
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

    Ok(serde_json::json!({
        "config": {
            pascal_connector_name(connector): serde_json::Value::Object(normalized)
        }
    }))
}

/// Key paths present in the entry but absent after a round-trip through
/// [`ConnectorSpecificConfig`] — i.e. fields the config message does not define
/// and therefore silently discards.
fn dropped_field_paths(
    sent_value: &serde_json::Value,
    kept_value: &serde_json::Value,
) -> Vec<String> {
    let (mut sent, mut kept) = (Vec::new(), Vec::new());
    key_paths(sent_value, "", &mut sent);
    key_paths(kept_value, "", &mut kept);
    sent.into_iter().filter(|k| !kept.contains(k)).collect()
}

/// Names the credential fields this connector's config message does not define.
///
/// Empty means every field in the entry reached the config. Callers that can
/// afford to reject a questionable entry — a gate reviewing a newly added
/// connector, say — can use this to do so; [`connector_config_header`] only
/// warns, because its other callers skip a connector whose credentials fail to
/// load and a skipped connector is invisible.
pub fn unknown_config_fields(
    creds_file: &Path,
    connector: &str,
) -> Result<Vec<String>, CredentialError> {
    let wrapped = build_wrapped_config(creds_file, connector)?;
    let parsed: ConnectorSpecificConfig = serde_json::from_value(wrapped.clone())
        .map_err(|e| CredentialError::InvalidConfig(connector.to_string(), e.to_string()))?;
    let round_tripped = serde_json::to_value(&parsed)
        .map_err(|e| CredentialError::InvalidConfig(connector.to_string(), e.to_string()))?;
    Ok(dropped_field_paths(&wrapped, &round_tripped))
}

/// Builds the `x-connector-config` header value for one connector, and proves it
/// deserializes into [`ConnectorSpecificConfig`] before returning it.
///
/// Two failures are possible and only one of them is loud by default. A field of
/// the wrong *type* fails deserialization. A field of the wrong *name* does not:
/// the generated config messages derive a plain `Deserialize` with no
/// `deny_unknown_fields`, so an unrecognised key is dropped and the connector
/// receives an all-empty config, which comes back as an authentication failure
/// that looks like a connector defect. Field names differ per connector
/// (`authorizedotnet` uses `name`/`transaction_key`, `paysafe`
/// `username`/`password`/`account_id`), so that is the likelier mistake of the
/// two. Round-tripping the parsed config and comparing key paths catches it
/// here, where it can name the offending field.
#[allow(clippy::print_stderr)]
pub fn connector_config_header(
    creds_file: &Path,
    connector: &str,
) -> Result<String, CredentialError> {
    let wrapped = build_wrapped_config(creds_file, connector)?;

    let parsed: ConnectorSpecificConfig =
        serde_json::from_value(wrapped.clone()).map_err(|error| {
            CredentialError::InvalidConfig(connector.to_string(), error.to_string())
        })?;

    let round_tripped = serde_json::to_value(&parsed).map_err(|error| {
        CredentialError::InvalidConfig(connector.to_string(), error.to_string())
    })?;

    let dropped = dropped_field_paths(&wrapped, &round_tripped);
    if !dropped.is_empty() {
        // Warn rather than fail. The sweep path in the harness skips a connector
        // whose credentials do not load, so rejecting here would make it vanish
        // from the report entirely — worse than running it with a field the
        // config ignores, because an absent connector reads as "nothing to see".
        // A connector that genuinely needs the field still fails visibly, at the
        // connector, with this line in the log to explain why.
        eprintln!(
            "[credentials] '{connector}': {} ignored by the config message ({}). \
             Check the field names against this connector's ConnectorSpecificConfig \
             variant — unknown fields are dropped, which reaches the connector as \
             an authentication failure.",
            if dropped.len() == 1 {
                "1 field is"
            } else {
                "fields are"
            },
            dropped.join(", ")
        );
    }

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
    fn a_correct_entry_drops_nothing() {
        let file = write(r#"{"stripe":{"api_key":{"value":"sk_test_123"}}}"#);
        assert!(unknown_config_fields(file.path(), "stripe")
            .expect("should load")
            .is_empty());
    }

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

    // Not every config field is a flat Secret. Paysafe nests a message under
    // account_id and Cashtocode keys a map by currency, and normalize_value
    // recurses through both. No *Config message declares a field named "value"
    // (checked against the generated types), so unwrapping cannot shadow a real
    // field — but these shapes are the ones that would break silently, by
    // producing an all-None config rather than an error.
    #[test]
    fn nested_message_fields_survive_normalisation() {
        let file = write(
            r#"{"paysafe":{"username":{"value":"u"},"password":{"value":"p"},
                "account_id":{"card":{"USD":{"no_three_ds":{"value":"acct_card_usd"}}}}}}"#,
        );
        let header = connector_config_header(file.path(), "paysafe").expect("should load");
        assert!(header.contains(r#""username":"u""#), "got {header}");
        assert!(
            header.contains("acct_card_usd"),
            "account_id was dropped: {header}"
        );
    }

    #[test]
    fn map_valued_fields_survive_normalisation() {
        let file = write(
            r#"{"cashtocode":{"auth_key_map":{"EUR":{"password_classic":{"value":"pw"},
                                                     "username_classic":{"value":"un"}}}}}"#,
        );
        let header = connector_config_header(file.path(), "cashtocode").expect("should load");
        assert!(header.contains("EUR"), "map key lost: {header}");
        assert!(
            header.contains(r#""password_classic":"pw""#),
            "nested secret not unwrapped: {header}"
        );
    }

    // Renamed fields are the quiet failure: serde ignores unknown keys, so a
    // wrong name yields an all-None config that reaches the connector and comes
    // back as an auth error.
    #[test]
    fn renamed_fields_are_carried_through_verbatim() {
        let file =
            write(r#"{"authorizedotnet":{"name":{"value":"n"},"transaction_key":{"value":"tk"}}}"#);
        let header = connector_config_header(file.path(), "authorizedotnet").expect("should load");
        assert!(header.contains(r#""name":"n""#), "got {header}");
        assert!(header.contains(r#""transaction_key":"tk""#), "got {header}");
    }

    // The quiet failure this check exists for: `password` is not a field of
    // CashtocodeCurrencyAuthData (it is `password_classic`). Serde drops it, and
    // without the round-trip comparison the entry would validate as an
    // all-empty config and fail at the connector instead.
    #[test]
    fn a_field_the_config_does_not_define_is_named() {
        let file = write(r#"{"cashtocode":{"auth_key_map":{"EUR":{"password":{"value":"pw"}}}}}"#);
        let dropped = unknown_config_fields(file.path(), "cashtocode").expect("should load");
        assert_eq!(
            dropped,
            vec!["config.Cashtocode.auth_key_map.EUR.password".to_string()]
        );
        // Still usable: the loader warns rather than making the connector vanish
        // from a sweep that skips connectors whose credentials fail to load.
        assert!(connector_config_header(file.path(), "cashtocode").is_ok());
    }

    #[test]
    fn a_misspelled_top_level_field_is_named() {
        let file = write(r#"{"stripe":{"api_ky":"sk_test_x"}}"#);
        let dropped = unknown_config_fields(file.path(), "stripe").expect("should load");
        assert_eq!(dropped, vec!["config.Stripe.api_ky".to_string()]);
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
