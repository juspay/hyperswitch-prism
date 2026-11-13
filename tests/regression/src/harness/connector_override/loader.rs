use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

use crate::harness::{
    scenario_loader::connector_specs_root,
    scenario_types::{DependencyScope, ScenarioError, ScenarioFile, SuiteDependency},
};

/// Override patch payload for one specific `(suite, scenario)` pair.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioOverridePatch {
    /// Base scenario in the same suite. Defines a new connector-local variant.
    #[serde(default)]
    pub extends: Option<String>,
    /// Replace inherited prerequisites. An empty array runs without prerequisites.
    #[serde(default)]
    pub depends_on: Option<Vec<SuiteDependency>>,
    /// Rerun inherited prerequisites by setting this to `scenario`.
    #[serde(default)]
    pub dependency_scope: Option<DependencyScope>,
    #[serde(default)]
    pub required_payment_method: Option<String>,
    #[serde(default)]
    pub grpc_req: Option<Value>,
    #[serde(rename = "assert", default)]
    pub assert_rules: Option<BTreeMap<String, Value>>,
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
#[serde(deny_unknown_fields)]
pub struct PreRequestHttpHook {
    /// When `Some`, fire an HTTP request to the URL before the scenario runs.
    /// When `None`, the hook degenerates to a pure sleep of `timeout_secs`
    /// seconds — useful for waiting out sandbox-side dedup/rate windows
    /// without needing an unreachable URL as a sleep vehicle.
    #[serde(default)]
    pub url: Option<String>,
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
#[cfg(test)]
fn load_scenario_override_patch(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<ScenarioOverridePatch>, ScenarioError> {
    Ok(load_suite_override_patches(connector, suite)?.remove(scenario))
}

fn load_suite_override_patches(
    connector: &str,
    suite: &str,
) -> Result<SuiteOverrideFile, ScenarioError> {
    if let Some(mut connector_patch) = load_connector_override_file(connector)? {
        return Ok(connector_patch.remove(suite).unwrap_or_default());
    }
    // Backward-compatible fallback for suite-level override files.
    Ok(load_legacy_suite_override_file(connector, suite)?.unwrap_or_default())
}

/// Resolves patches base-first, keeping null deletions intact until application.
pub fn load_scenario_override_chain(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Vec<(String, ScenarioOverridePatch)>, ScenarioError> {
    override_chain(
        &load_suite_override_patches(connector, suite)?,
        suite,
        scenario,
        &connector_override_file_path(connector),
    )
}

fn override_chain(
    patches: &SuiteOverrideFile,
    suite: &str,
    scenario: &str,
    path: &Path,
) -> Result<Vec<(String, ScenarioOverridePatch)>, ScenarioError> {
    let mut chain = Vec::new();
    let mut visited = BTreeSet::new();
    let mut current = scenario;
    loop {
        if !visited.insert(current) {
            return Err(ScenarioError::InvalidConnectorSpec {
                path: path.to_path_buf(),
                message: format!("cyclic scenario inheritance in {suite}/{scenario}: {current}"),
            });
        }
        let patch = patches.get(current).cloned().unwrap_or_default();
        chain.push((current.to_string(), patch));
        match patches
            .get(current)
            .and_then(|patch| patch.extends.as_deref())
        {
            Some(base) => current = base,
            None => break,
        }
    }
    chain.reverse();
    Ok(chain)
}

/// Adds named variants and resolves their execution metadata. Request/assertion
/// patches are applied later, after dependency context, just like normal overrides.
pub fn merge_scenario_variants(
    connector: &str,
    suite: &str,
    scenarios: &mut ScenarioFile,
) -> Result<usize, ScenarioError> {
    merge_variants(
        &load_suite_override_patches(connector, suite)?,
        suite,
        scenarios,
        &connector_override_file_path(connector),
    )
}

fn merge_variants(
    patches: &SuiteOverrideFile,
    suite: &str,
    scenarios: &mut ScenarioFile,
    path: &Path,
) -> Result<usize, ScenarioError> {
    let baseline = scenarios.clone();
    let mut added = 0;
    for (name, patch) in patches {
        let invalid = |message| ScenarioError::InvalidConnectorSpec {
            path: path.to_path_buf(),
            message,
        };
        if patch.extends.is_some() && baseline.contains_key(name) {
            return Err(invalid(format!(
                "{suite}/{name}: a named variant cannot shadow an existing scenario"
            )));
        }
        let chain = override_chain(patches, suite, name, path)?;
        let Some((base, _)) = chain.first() else {
            continue;
        };
        let mut definition = baseline.get(base).cloned().ok_or_else(|| {
            invalid(format!(
                "{suite}/{name}: unknown base scenario {base}; new scenarios must specify extends"
            ))
        })?;
        if patch.extends.is_some() {
            definition.is_default = false;
            definition.display_name = None;
            added += 1;
        }
        for (_, inherited) in chain {
            if let Some(dependencies) = inherited.depends_on {
                definition.depends_on = Some(dependencies);
            }
            if let Some(scope) = inherited.dependency_scope {
                definition.dependency_scope = Some(scope);
            }
            if let Some(method) = inherited.required_payment_method {
                definition.required_payment_method = Some(method);
            }
        }
        scenarios.insert(name.clone(), definition);
    }
    Ok(added)
}

/// Loads the optional `pre_request_http` hook for a scenario override.
pub fn load_scenario_pre_request_http(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<PreRequestHttpHook>, ScenarioError> {
    Ok(load_scenario_override_chain(connector, suite, scenario)?
        .into_iter()
        .rev()
        .find_map(|(_, patch)| patch.pre_request_http))
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

    #[test]
    fn named_variants_inherit_connector_patches_and_independent_prerequisites() {
        use super::{merge_variants, override_chain, ScenarioFile, SuiteOverrideFile};
        use crate::harness::{
            connector_override::{apply_assertion_patch, json_merge::json_merge_patch},
            scenario_types::{DependencyScope, FieldAssert, SuiteSpec},
        };

        let mut scenarios: ScenarioFile = serde_json::from_value(json!({
            "base": {
                "is_default": true,
                "grpc_req": {"amount": {"minor_amount": 6000, "currency": "USD"}, "address": {"zip": "12345"}},
                "assert": {"status": {"equals": "AUTHORIZED"}, "error": {"must_not_exist": true}}
            }
        })).unwrap();
        // The child sorts before its parent, exercising forward references.
        let patches: SuiteOverrideFile = serde_json::from_value(json!({
            "base": {"grpc_req": {"amount": {"currency": "MYR"}}},
            "z_parent": {
                "extends": "base",
                "dependency_scope": "scenario",
                "grpc_req": {"address": null},
                "assert": {"status": {"equals": "CHARGED"}},
                "pre_request_http": {"timeout_secs": 0}
            },
            "a_child": {
                "extends": "z_parent",
                "depends_on": [{"suite": "PaymentService/Authorize", "scenario": "base",
                    "context_map": {"connector_transaction_id": "res.connector_transaction_id"}}],
                "grpc_req": {"amount": {"minor_amount": 3000}},
                "assert": {"error": null}
            },
            "empty_chain": {"extends": "a_child", "depends_on": []},
            "shared_chain": {"extends": "base"}
        }))
        .unwrap();
        let path = std::path::Path::new("override.json");
        assert_eq!(
            merge_variants(&patches, "PaymentService/Capture", &mut scenarios, path).unwrap(),
            4
        );
        let child = scenarios.get("a_child").unwrap();
        assert!(!child.is_default);
        let chain = override_chain(&patches, "PaymentService/Capture", "a_child", path).unwrap();
        assert_eq!(
            chain
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["base", "z_parent", "a_child"]
        );
        let mut req = child.grpc_req.clone();
        let mut rules = child.assert_rules.clone();
        for (_, patch) in &chain {
            if let Some(patch) = &patch.grpc_req {
                json_merge_patch(&mut req, patch);
            }
            if let Some(patch) = &patch.assert_rules {
                apply_assertion_patch(&mut rules, patch).unwrap();
            }
        }
        assert_eq!(
            req,
            json!({"amount": {"minor_amount": 3000, "currency": "MYR"}})
        );
        assert!(!rules.contains_key("error"));
        assert!(
            matches!(rules.get("status"), Some(FieldAssert::Equals { equals }) if equals == "CHARGED")
        );
        assert_eq!(
            chain
                .into_iter()
                .rev()
                .find_map(|(_, patch)| patch.pre_request_http)
                .unwrap()
                .timeout_secs,
            0
        );

        let suite: SuiteSpec = serde_json::from_value(json!({
            "suite": "PaymentService/Capture", "suite_type": "payment_flow",
            "depends_on": ["CustomerService/Create"], "strict_dependencies": true
        }))
        .unwrap();
        let (dependencies, scope) = child.prerequisites(&suite);
        assert_eq!(scope, DependencyScope::Scenario);
        assert_eq!(dependencies.len(), 1);
        let dependency = dependencies.first().unwrap();
        assert_eq!(dependency.scenario(), Some("base"));
        assert_eq!(
            dependency
                .context_map()
                .unwrap()
                .get("connector_transaction_id")
                .unwrap(),
            "res.connector_transaction_id"
        );
        let parent = scenarios.get("z_parent").unwrap();
        assert_eq!(parent.prerequisites(&suite).1, DependencyScope::Scenario);
        assert_eq!(
            parent.prerequisites(&suite).0.first().unwrap().suite(),
            "CustomerService/Create"
        );
        assert!(scenarios
            .get("empty_chain")
            .unwrap()
            .prerequisites(&suite)
            .0
            .is_empty());
        assert_eq!(
            scenarios
                .get("shared_chain")
                .unwrap()
                .prerequisites(&suite)
                .1,
            DependencyScope::Suite
        );
    }

    #[test]
    fn invalid_variant_inheritance_fails_with_a_useful_error() {
        use super::{merge_variants, ScenarioFile};
        for (patches, expected) in [
            (
                json!({"child": {"extends": "missing"}}),
                "unknown base scenario missing",
            ),
            (
                json!({"a": {"extends": "b"}, "b": {"extends": "a"}}),
                "cyclic scenario inheritance",
            ),
            (
                json!({"base": {"extends": "base"}}),
                "cannot shadow an existing scenario",
            ),
            (
                json!({"typo": {"grpc_req": {}}}),
                "new scenarios must specify extends",
            ),
        ] {
            let mut scenarios: ScenarioFile = serde_json::from_value(json!({
                "base": {"grpc_req": {}, "assert": {"status": {"must_exist": true}}}
            }))
            .unwrap();
            let error = merge_variants(
                &serde_json::from_value(patches).unwrap(),
                "PaymentService/Authorize",
                &mut scenarios,
                std::path::Path::new("override.json"),
            )
            .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

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
    fn an_invented_field_is_rejected_rather_than_ignored() {
        // Before `deny_unknown_fields`, an unknown key parsed cleanly and every
        // field came back None: the diff read as a declaration while the run
        // ignored it. The silence is worse than an error because nothing points
        // at it.
        let err = serde_json::from_value::<ScenarioOverridePatch>(json!({
            "unsuported": true,
            "reason": "typo in the field name"
        }))
        .expect_err("an unknown field must not parse into silence");
        assert!(
            err.to_string().contains("unsuported"),
            "the error should name the offending field, got: {err}"
        );
    }

    #[test]
    fn every_field_the_overrides_actually_use_still_parses() {
        let patch: ScenarioOverridePatch = serde_json::from_value(json!({
            "grpc_req": { "amount": { "minor_amount": 100 } },
            "assert": { "status": { "one_of": ["CHARGED"] } },
            "pre_request_http": {
                "url": "https://example.test/simulate",
                "method": "POST",
                "body": "{}",
                "timeout_secs": 5
            }
        }))
        .expect("the three documented fields must keep parsing");
        assert!(patch.grpc_req.is_some());
        assert!(patch.assert_rules.is_some());
        assert!(patch.pre_request_http.is_some());
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
}
