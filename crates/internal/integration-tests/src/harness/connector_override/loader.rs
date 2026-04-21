use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::harness::{scenario_loader::connector_specs_root, scenario_types::ScenarioError};

/// Override patch payload for one specific `(suite, scenario)` pair.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScenarioOverridePatch {
    #[serde(default)]
    pub grpc_req: Option<Value>,
    #[serde(rename = "assert", default)]
    pub assert_rules: Option<BTreeMap<String, Value>>,
    /// Connector-specific context map patch. Keys are target paths in the
    /// scenario request; values are `req.` / `res.` source references into the
    /// scenario's dependency payloads. Applied AFTER the suite-level
    /// `context_map`, so it can fill in fields the suite doesn't set.
    #[serde(default)]
    pub context_map: Option<BTreeMap<String, String>>,
    /// Fire an HTTP request (fire-and-forget) before this scenario runs.
    /// Used to drive sandbox simulators that settle a payment outside of the
    /// normal connector API surface — e.g. Cashfree's `/pg/view/simulate`
    /// endpoint which flips a UPI Intent payment to SUCCESS so the subsequent
    /// sync returns `CHARGED` without browser automation.
    #[serde(default)]
    pub pre_request_http: Option<PreRequestHttpHook>,
}

/// Fire-and-forget HTTP call issued before the scenario's gRPC request.
/// Body supports `{{dep_res.<index>.<json-path>}}` templating from
/// dependency responses (e.g. pulling cf_payment_id out of the authorize
/// response at dep_res index 1).
#[derive(Debug, Clone, Deserialize)]
pub struct PreRequestHttpHook {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default = "default_hook_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_http_method() -> String {
    "POST".to_string()
}
fn default_hook_timeout_secs() -> u64 {
    10
}

type SuiteOverrideFile = BTreeMap<String, ScenarioOverridePatch>;
type ConnectorOverrideFile = BTreeMap<String, SuiteOverrideFile>;

/// Path to `<connector>/override.json` under connector override root.
pub fn connector_override_file_path(connector: &str) -> PathBuf {
    connector_override_root()
        .join(connector)
        .join("override.json")
}

/// Override root path, configurable independently from connector specs root.
fn connector_override_root() -> PathBuf {
    std::env::var("UCS_CONNECTOR_OVERRIDE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| connector_specs_root())
}

/// Legacy path used by older suite-level override file layouts.
fn legacy_connector_suite_override_file_path(connector: &str, suite: &str) -> PathBuf {
    connector_override_root()
        .join(connector)
        .join("overrides")
        .join(format!("{suite}.overrides.json"))
}

/// Loads optional override patch for one connector/suite/scenario.
///
/// Resolution order:
/// 1. New unified connector override file (`<connector>/override.json`)
/// 2. Legacy suite-level override file (`<connector>/overrides/<suite>.overrides.json`)
pub fn load_scenario_override_patch(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<ScenarioOverridePatch>, ScenarioError> {
    if let Some(connector_patch) = load_connector_override_file(connector)? {
        return Ok(connector_patch
            .get(suite)
            .and_then(|suite_patch| suite_patch.get(scenario))
            .cloned());
    }

    // Backward-compatible fallback for suite-level override files.
    if let Some(suite_patch) = load_legacy_suite_override_file(connector, suite)? {
        return Ok(suite_patch.get(scenario).cloned());
    }

    Ok(None)
}

/// Loads the optional `pre_request_http` hook for a scenario override.
pub fn load_scenario_pre_request_http(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<PreRequestHttpHook>, ScenarioError> {
    Ok(load_scenario_override_patch(connector, suite, scenario)?
        .and_then(|patch| patch.pre_request_http))
}

/// Loads optional per-connector `context_map` patch for a given scenario.
///
/// Used to inject dependency-derived fields into the effective request for
/// connectors whose sync/flow transformers require fields the default suite
/// context_map doesn't set (e.g. PhonePe reads `connector_order_reference_id`
/// from the sync request as its `merchant_order_id`).
pub fn load_scenario_override_context_map(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<BTreeMap<String, String>>, ScenarioError> {
    Ok(load_scenario_override_patch(connector, suite, scenario)?
        .and_then(|patch| patch.context_map))
}

/// Path to `<connector>/webhook_payload.json` under connector override root.
pub fn connector_webhook_payload_file_path(connector: &str) -> PathBuf {
    connector_override_root()
        .join(connector)
        .join("webhook_payload.json")
}

/// Loads the webhook payload override for a specific connector/scenario.
///
/// The `webhook_payload.json` file uses the same structure as a single suite
/// section inside `override.json`: scenario names as keys, each containing a
/// `grpc_req` patch.  An optional `_webhook_config` key holds connector-level
/// webhook metadata (signature header, algorithm, etc.) and is returned
/// separately so callers can use it for post-merge transforms.
///
/// This function is only relevant for the `handle_event` suite; other suites
/// should not call it.
pub fn load_webhook_payload_patch(
    connector: &str,
    scenario: &str,
) -> Result<Option<(ScenarioOverridePatch, Option<Value>)>, ScenarioError> {
    let path = connector_webhook_payload_file_path(connector);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorOverrideRead {
            path: path.clone(),
            source,
        })?;

    let parsed: Value =
        serde_json::from_str(&content).map_err(|source| ScenarioError::ConnectorOverrideParse {
            path: path.clone(),
            source,
        })?;

    let webhook_config = parsed.get("_webhook_config").cloned();

    let Some(scenario_value) = parsed.get(scenario) else {
        return Ok(None);
    };

    let patch = serde_json::from_value::<ScenarioOverridePatch>(scenario_value.clone()).map_err(
        |source| ScenarioError::ConnectorOverrideParse {
            path: path.clone(),
            source,
        },
    )?;

    Ok(Some((patch, webhook_config)))
}

/// Loads and parses the unified connector override file if present.
fn load_connector_override_file(
    connector: &str,
) -> Result<Option<ConnectorOverrideFile>, ScenarioError> {
    let path = connector_override_file_path(connector);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorOverrideRead {
            path: path.clone(),
            source,
        })?;

    let parsed = serde_json::from_str::<ConnectorOverrideFile>(&content).map_err(|source| {
        ScenarioError::ConnectorOverrideParse {
            path: path.clone(),
            source,
        }
    })?;

    Ok(Some(parsed))
}

/// Loads and parses legacy suite override file if present.
fn load_legacy_suite_override_file(
    connector: &str,
    suite: &str,
) -> Result<Option<SuiteOverrideFile>, ScenarioError> {
    let path = legacy_connector_suite_override_file_path(connector, suite);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorOverrideRead {
            path: path.clone(),
            source,
        })?;

    let parsed = serde_json::from_str::<SuiteOverrideFile>(&content).map_err(|source| {
        ScenarioError::ConnectorOverrideParse {
            path: path.clone(),
            source,
        }
    })?;

    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::{
        connector_override_file_path, load_scenario_override_patch, ScenarioOverridePatch,
    };

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ucs_connector_override_test_{nanos}"))
    }

    #[test]
    fn missing_override_file_returns_none() {
        let env_lock = ENV_LOCK.lock().expect("env lock should acquire");
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).expect("temp root should be created");

        let previous = std::env::var("UCS_CONNECTOR_OVERRIDE_ROOT").ok();
        std::env::set_var("UCS_CONNECTOR_OVERRIDE_ROOT", &temp_root);

        let loaded = load_scenario_override_patch("stripe", "authorize", "no3ds_fail_payment")
            .expect("loading missing override should not fail");
        assert!(loaded.is_none());

        match previous {
            Some(value) => std::env::set_var("UCS_CONNECTOR_OVERRIDE_ROOT", value),
            None => std::env::remove_var("UCS_CONNECTOR_OVERRIDE_ROOT"),
        }
        drop(env_lock);
        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn loads_scenario_override_patch_from_connector_file() {
        let env_lock = ENV_LOCK.lock().expect("env lock should acquire");
        let temp_root = unique_temp_dir();
        let connector_dir = temp_root.join("stripe");
        fs::create_dir_all(&connector_dir).expect("connector directory should be created");

        let override_path = connector_dir.join("override.json");
        let file_content = json!({
            "authorize": {
                "no3ds_fail_payment": {
                    "grpc_req": {
                        "payment_method": {
                            "card": {
                                "card_number": {
                                    "value": "4000000000000002"
                                }
                            }
                        }
                    },
                    "assert": {
                        "status": {
                            "one_of": ["FAILURE"]
                        }
                    }
                }
            }
        });
        fs::write(
            &override_path,
            serde_json::to_string_pretty(&file_content).expect("override content should serialize"),
        )
        .expect("override file should be written");

        let previous = std::env::var("UCS_CONNECTOR_OVERRIDE_ROOT").ok();
        std::env::set_var("UCS_CONNECTOR_OVERRIDE_ROOT", &temp_root);

        let loaded = load_scenario_override_patch("stripe", "authorize", "no3ds_fail_payment")
            .expect("loading override patch should succeed")
            .expect("override patch should exist");
        assert!(matches!(loaded, ScenarioOverridePatch { .. }));
        assert_eq!(
            loaded.grpc_req,
            Some(json!({
                "payment_method": {
                    "card": {
                        "card_number": {
                            "value": "4000000000000002"
                        }
                    }
                }
            }))
        );
        assert!(loaded.assert_rules.is_some());

        let computed_path = connector_override_file_path("stripe");
        assert_eq!(computed_path, override_path);

        match previous {
            Some(value) => std::env::set_var("UCS_CONNECTOR_OVERRIDE_ROOT", value),
            None => std::env::remove_var("UCS_CONNECTOR_OVERRIDE_ROOT"),
        }
        drop(env_lock);
        let _ = fs::remove_dir_all(temp_root);
    }
}
