use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::harness::{scenario_loader::connector_specs_root, scenario_types::ScenarioError};

/// Polling configuration for retry logic.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PollingConfig {
    /// Maximum number of retries for polling operations.
    pub max_retries: Option<u32>,
    /// Delay in milliseconds between retry attempts.
    pub retry_delay_ms: Option<u64>,
}

impl PollingConfig {
    /// Merges this config with another, where the other takes precedence.
    fn merge_with(self, override_config: Self) -> Self {
        Self {
            max_retries: override_config.max_retries.or(self.max_retries),
            retry_delay_ms: override_config.retry_delay_ms.or(self.retry_delay_ms),
        }
    }
}

/// Suite or scenario configuration options.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SuiteConfig {
    /// Delay in milliseconds to wait before/during suite execution.
    pub delay_ms: Option<u64>,
    /// Polling configuration for retry operations.
    pub polling_config: Option<PollingConfig>,
}

impl SuiteConfig {
    /// Merges this config with another, where the other takes precedence.
    fn merge_with(self, override_config: Self) -> Self {
        Self {
            delay_ms: override_config.delay_ms.or(self.delay_ms),
            polling_config: match (self.polling_config, override_config.polling_config) {
                (Some(base), Some(override_cfg)) => Some(base.merge_with(override_cfg)),
                (config @ Some(_), None) | (None, config @ Some(_)) => config,
                (None, None) => None,
            },
        }
    }
}

/// Override patch payload for one specific `(suite, scenario)` pair.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScenarioOverridePatch {
    #[serde(default)]
    pub grpc_req: Option<Value>,
    #[serde(rename = "assert", default)]
    pub assert_rules: Option<BTreeMap<String, Value>>,
    /// Configuration options using __config__ key.
    #[serde(rename = "__config__", default)]
    pub config: Option<SuiteConfig>,
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

/// Loads suite-level configuration from the `__config__` key in override.json.
///
/// This provides default configuration for all scenarios in the suite.
/// Individual scenarios can override this with their own `__config__` key.
pub fn load_suite_config(
    connector: &str,
    suite: &str,
) -> Result<Option<SuiteConfig>, ScenarioError> {
    // We need to load the raw JSON because suite-level __config__ is stored
    // as a raw config object, not wrapped in ScenarioOverridePatch
    let path = connector_override_file_path(connector);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).map_err(|source| {
        ScenarioError::ConnectorOverrideRead {
            path: path.clone(),
            source,
        }
    })?;

    let parsed: Value = serde_json::from_str(&content).map_err(|source| {
        ScenarioError::ConnectorOverrideParse {
            path: path.clone(),
            source,
        }
    })?;

    // Navigate to suite -> __config__
    let suite_config_value = parsed
        .get(suite)
        .and_then(|suite_obj| suite_obj.get("__config__"));

    if let Some(config_value) = suite_config_value {
        let config = serde_json::from_value::<SuiteConfig>(config_value.clone()).map_err(
            |source| ScenarioError::ConnectorOverrideParse {
                path: path.clone(),
                source,
            },
        )?;
        return Ok(Some(config));
    }

    Ok(None)
}

/// Loads merged configuration for a specific scenario, combining suite-level
/// and scenario-level configs. Scenario-level config takes precedence.
pub fn load_scenario_config(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<SuiteConfig, ScenarioError> {
    let suite_config = load_suite_config(connector, suite)?.unwrap_or_default();
    let scenario_config = load_scenario_override_patch(connector, suite, scenario)?
        .and_then(|patch| patch.config)
        .unwrap_or_default();

    // Merge configs: scenario-level overrides suite-level
    Ok(suite_config.merge_with(scenario_config))
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

        let loaded = load_scenario_override_patch(
            "stripe",
            "PaymentService/Authorize",
            "no3ds_fail_payment",
        )
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
            "PaymentService/Authorize": {
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

        let loaded = load_scenario_override_patch(
            "stripe",
            "PaymentService/Authorize",
            "no3ds_fail_payment",
        )
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

    #[test]
    fn loads_suite_and_scenario_config_with_merge() {
        use super::load_scenario_config;

        let env_lock = ENV_LOCK.lock().expect("env lock should acquire");
        let temp_root = unique_temp_dir();
        let connector_dir = temp_root.join("testconnector");
        fs::create_dir_all(&connector_dir).expect("connector directory should be created");

        let override_path = connector_dir.join("override.json");
        let file_content = json!({
            "PaymentService/Get": {
                "__config__": {
                    "delay_ms": 1000,
                    "polling_config": {
                        "max_retries": 10,
                        "retry_delay_ms": 500
                    }
                },
                "get_payment": {
                    "__config__": {
                        "polling_config": {
                            "max_retries": 20
                        }
                    },
                    "grpc_req": {}
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

        // Load scenario config - should merge suite and scenario levels
        let config = load_scenario_config("testconnector", "PaymentService/Get", "get_payment")
            .expect("loading config should succeed");

        // Verify merged config
        assert_eq!(config.delay_ms, Some(1000)); // From suite level
        assert!(config.polling_config.is_some());
        let polling = config.polling_config.unwrap();
        assert_eq!(polling.max_retries, Some(20)); // Overridden by scenario level
        assert_eq!(polling.retry_delay_ms, Some(500)); // From suite level (not overridden)

        match previous {
            Some(value) => std::env::set_var("UCS_CONNECTOR_OVERRIDE_ROOT", value),
            None => std::env::remove_var("UCS_CONNECTOR_OVERRIDE_ROOT"),
        }
        drop(env_lock);
        let _ = fs::remove_dir_all(temp_root);
    }
}
