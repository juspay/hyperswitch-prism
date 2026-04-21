use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;

use crate::harness::scenario_types::{FieldAssert, ScenarioError};

mod cybersource;
mod default;
mod helcim;
mod json_merge;
mod loader;

/// Connector-specific behavior extension points.
///
/// The default implementation is file-driven via JSON patches, but this trait
/// also allows richer connector logic (request normalization, response shaping,
/// and deferred context paths).
pub trait ConnectorOverride: Send + Sync {
    fn connector_name(&self) -> &str;

    fn apply_overrides(
        &self,
        suite: &str,
        scenario: &str,
        grpc_req: &mut Value,
        assertions: &mut BTreeMap<String, FieldAssert>,
    ) -> Result<(), ScenarioError> {
        // 1. Apply regular override.json patches (works for all suites).
        if let Some(scenario_patch) =
            loader::load_scenario_override_patch(self.connector_name(), suite, scenario)?
        {
            if let Some(req_patch) = scenario_patch.grpc_req.as_ref() {
                json_merge::json_merge_patch(grpc_req, req_patch);
            }
            if let Some(assertion_patch) = scenario_patch.assert_rules.as_ref() {
                apply_assertion_patch(assertions, assertion_patch)?;
            }
        }

        // 2. For handle_event suite: load webhook_payload.json as an
        //    additional merge-patch layer, then run post-merge transforms
        //    (base64-encode body, compute HMAC signatures, inject webhook_secrets).
        if suite == "handle_event" {
            apply_webhook_payload_overrides(self.connector_name(), scenario, grpc_req)?;
        }

        Ok(())
    }

    fn normalize_tonic_request(&self, _suite: &str, _scenario: &str, _req: &mut Value) {}

    fn transform_response(&self, _suite: &str, _scenario: &str, _response: &mut Value) {}

    fn extra_context_deferred_paths(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Minimal registry wrapper used to resolve override strategy by connector.
#[derive(Debug, Default)]
pub struct OverrideRegistry;

impl OverrideRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns currently configured strategy for a connector.
    ///
    /// At the moment all connectors use the default file-backed strategy.
    pub fn resolve(&self, connector: &str) -> Box<dyn ConnectorOverride> {
        if connector.eq_ignore_ascii_case("cybersource") {
            return Box::new(cybersource::CybersourceConnectorOverride::new());
        }

        if connector.eq_ignore_ascii_case("helcim") {
            return Box::new(helcim::HelcimConnectorOverride::new());
        }

        Box::new(default::DefaultConnectorOverride::new(
            connector.to_string(),
        ))
    }
}

pub use loader::PreRequestHttpHook;

/// Returns the optional `pre_request_http` hook spec for a scenario.
pub fn connector_pre_request_http_hook(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<PreRequestHttpHook>, ScenarioError> {
    loader::load_scenario_pre_request_http(connector, suite, scenario)
}

/// Returns the connector's per-scenario `context_map` override, if any.
pub fn connector_override_context_map(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<BTreeMap<String, String>>, ScenarioError> {
    loader::load_scenario_override_context_map(connector, suite, scenario)
}

/// Applies connector override patches to request payload and assertions.
pub fn apply_connector_overrides(
    connector: &str,
    suite: &str,
    scenario: &str,
    grpc_req: &mut Value,
    assertions: &mut BTreeMap<String, FieldAssert>,
) -> Result<(), ScenarioError> {
    let strategy = OverrideRegistry::new().resolve(connector);
    strategy.apply_overrides(suite, scenario, grpc_req, assertions)
}

/// Applies optional connector-level request normalization before tonic decoding.
pub fn normalize_tonic_request_for_connector(
    connector: &str,
    suite: &str,
    scenario: &str,
    grpc_req: &mut Value,
) {
    let strategy = OverrideRegistry::new().resolve(connector);
    strategy.normalize_tonic_request(suite, scenario, grpc_req);
}

/// Applies optional connector-level response normalization before assertions.
pub fn transform_response_for_connector(
    connector: &str,
    suite: &str,
    scenario: &str,
    response: &mut Value,
) {
    let strategy = OverrideRegistry::new().resolve(connector);
    strategy.transform_response(suite, scenario, response);
}

/// Returns request paths that should defer auto-generation until dependency
/// context propagation.
pub fn context_deferred_paths_for_connector(connector: &str) -> Vec<String> {
    let strategy = OverrideRegistry::new().resolve(connector);
    strategy.extra_context_deferred_paths()
}

/// Loads `webhook_payload.json` for the connector/scenario, merges the
/// `grpc_req` patch, then runs post-merge transforms:
///
/// 1. If `request_details.body` is a JSON object (readable form), serialize
///    it to a compact JSON string and base64-encode it (proto `bytes` field).
/// 2. If the webhook config has a `signature_header` and no explicit signature
///    already exists in the merged headers, attempt to compute an HMAC signature
///    from `webhook_secrets` in the connector's creds.json.
/// 3. Inject `webhook_secrets` at the top level if the credential exists.
fn apply_webhook_payload_overrides(
    connector: &str,
    scenario: &str,
    grpc_req: &mut Value,
) -> Result<(), ScenarioError> {
    let Some((webhook_patch, webhook_config)) =
        loader::load_webhook_payload_patch(connector, scenario)?
    else {
        return Ok(());
    };

    // Apply the webhook payload grpc_req as a merge-patch.
    if let Some(req_patch) = webhook_patch.grpc_req.as_ref() {
        json_merge::json_merge_patch(grpc_req, req_patch);
    }

    // Post-merge transform: base64-encode `request_details.body` if it's a JSON
    // object (the readable form from webhook_payload.json).
    base64_encode_body_if_object(grpc_req);

    // Resolve webhook secret from creds.json.
    let secret_key = webhook_config
        .as_ref()
        .and_then(|c| c.get("webhook_secret_key"))
        .and_then(Value::as_str)
        .unwrap_or("webhook_secret");

    let webhook_secret = load_webhook_secret(connector, secret_key);

    // Compute HMAC signature if:
    //   - The webhook config specifies a signature_header
    //   - The header is not already set (i.e., not overridden in the payload file,
    //     e.g., for invalid_signature scenarios)
    //   - A webhook secret is available
    let signature_header = webhook_config
        .as_ref()
        .and_then(|c| c.get("signature_header"))
        .and_then(Value::as_str);

    if let Some(header_name) = signature_header {
        let existing_header = grpc_req
            .pointer("/request_details/headers")
            .and_then(Value::as_object)
            .and_then(|h| h.get(header_name))
            .and_then(Value::as_str);

        let header_is_empty_or_absent = existing_header.map(|v| v.is_empty()).unwrap_or(true);

        if header_is_empty_or_absent {
            if let Some(ref secret) = webhook_secret {
                // Decode the body back from base64 to compute signature over raw bytes.
                if let Some(body_b64) = grpc_req
                    .pointer("/request_details/body")
                    .and_then(Value::as_str)
                {
                    if let Ok(body_bytes) = STANDARD.decode(body_b64) {
                        if let Ok(sig) = crate::webhook_signatures::generate_signature(
                            connector,
                            &body_bytes,
                            secret,
                            None,
                        ) {
                            if let Some(headers) = grpc_req
                                .pointer_mut("/request_details/headers")
                                .and_then(Value::as_object_mut)
                            {
                                headers.insert(header_name.to_string(), Value::String(sig));
                            }
                        }
                    }
                }
            }
        }
    }

    // Inject webhook_secrets at the top level if a secret exists.
    if let Some(secret) = &webhook_secret {
        if let Some(root) = grpc_req.as_object_mut() {
            root.insert(
                "webhook_secrets".to_string(),
                serde_json::json!({ "secret": secret }),
            );
        }
    }

    Ok(())
}

/// If `request_details.body` is a JSON object (the readable form from
/// `webhook_payload.json`), serialize it to compact JSON and base64-encode it.
/// Proto `bytes` fields require base64 encoding in JSON representation.
fn base64_encode_body_if_object(grpc_req: &mut Value) {
    let body = match grpc_req.pointer("/request_details/body") {
        Some(v) if v.is_object() => v.clone(),
        _ => return,
    };

    let body_json_string = match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(_) => return,
    };

    let body_base64 = STANDARD.encode(body_json_string.as_bytes());

    if let Some(details) = grpc_req
        .pointer_mut("/request_details")
        .and_then(Value::as_object_mut)
    {
        details.insert("body".to_string(), Value::String(body_base64));
    }
}

/// Extracts the webhook secret from the connector credentials file.
fn load_webhook_secret(connector: &str, secret_key: &str) -> Option<String> {
    let creds_path = crate::harness::credentials::creds_file_path();
    let content = std::fs::read_to_string(&creds_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;

    let connector_block = json.get(connector)?;
    let block = match connector_block {
        Value::Array(arr) => arr.first()?,
        other => other,
    };

    block
        .get(secret_key)
        .and_then(|v| {
            if let Some(obj) = v.as_object() {
                if obj.len() == 1 {
                    return obj.get("value").and_then(Value::as_str);
                }
            }
            v.as_str()
        })
        .map(ToString::to_string)
}

/// Merges assertion patches:
/// - `null` removes a rule
/// - object value replaces/adds a rule
fn apply_assertion_patch(
    assertions: &mut BTreeMap<String, FieldAssert>,
    assertion_patch: &BTreeMap<String, Value>,
) -> Result<(), ScenarioError> {
    for (field, patch_value) in assertion_patch {
        if patch_value.is_null() {
            assertions.remove(field);
            continue;
        }

        let patched_rule =
            serde_json::from_value::<FieldAssert>(patch_value.clone()).map_err(|source| {
                ScenarioError::InvalidAssertionRule {
                    field: field.clone(),
                    message: format!(
                        "invalid connector override assertion patch for field '{field}': {source}"
                    ),
                }
            })?;
        assertions.insert(field.clone(), patched_rule);
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use serde_json::{json, Value};

    use super::apply_assertion_patch;
    use crate::harness::scenario_types::FieldAssert;

    #[test]
    fn assertion_patch_adds_replaces_and_removes_rules() {
        let mut assertions = std::collections::BTreeMap::new();
        assertions.insert(
            "status".to_string(),
            FieldAssert::OneOf {
                one_of: vec![Value::String("AUTHORIZED".to_string())],
            },
        );
        assertions.insert(
            "error".to_string(),
            FieldAssert::MustNotExist {
                must_not_exist: true,
            },
        );

        let patch = std::collections::BTreeMap::from([
            ("status".to_string(), json!({"one_of": ["CHARGED"]})),
            ("error".to_string(), Value::Null),
            (
                "connector_transaction_id".to_string(),
                json!({"must_exist": true}),
            ),
        ]);

        apply_assertion_patch(&mut assertions, &patch).expect("assertion patch should succeed");

        assert!(matches!(
            assertions.get("status"),
            Some(FieldAssert::OneOf { one_of }) if one_of == &vec![Value::String("CHARGED".to_string())]
        ));
        assert!(!assertions.contains_key("error"));
        assert!(matches!(
            assertions.get("connector_transaction_id"),
            Some(FieldAssert::MustExist { must_exist: true })
        ));
    }
}
