use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use serde::de::Error as _;
use serde_json::Value;

use crate::harness::scenario_types::{
    ConnectorBrowserAutomationSpec, ConnectorSuiteSpec, FieldAssert, ScenarioDef, ScenarioError,
    ScenarioFile, SuiteSpec,
};

/// Converts a global-suite directory name to its canonical suite identifier.
///
/// Directory names use `_` as the single separator between the service name and
/// the method name (e.g. `PaymentService_Authorize` → `"PaymentService/Authorize"`).
/// The service-name component must be CamelCase with no underscores — this is
/// enforced by the `debug_assert!` below so any violation is caught at test time.
///
/// Returns `None` if `dir_name` contains no `_` (not a valid suite directory).
pub fn suite_dir_name_to_suite_name(dir_name: &str) -> Option<String> {
    let sep_pos = dir_name.find('_')?;
    let service = &dir_name[..sep_pos];
    let method = &dir_name[sep_pos + 1..];
    debug_assert!(
        !service.contains('_'),
        "suite directory name {dir_name:?} has an underscore in the service part {service:?} — service names must be single CamelCase words with no underscores",
    );
    Some(format!("{service}/{method}"))
}

/// Root directory containing `<ServiceName_FlowName>/scenario.json` and `suite_spec.json`.
pub fn scenario_root() -> PathBuf {
    std::env::var("UCS_SCENARIO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/global_suites"))
}

/// Root directory containing per-connector `specs.json` and `override.json`.
pub fn connector_specs_root() -> PathBuf {
    std::env::var("UCS_CONNECTOR_SPECS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            scenario_root()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"))
                .join("connector_specs")
        })
}

/// Connector-specific directory under `connector_specs/`.
pub fn connector_spec_dir(connector: &str) -> PathBuf {
    connector_specs_root().join(connector)
}

/// Converts a suite name (`ServiceName/FlowName`) into the directory name
/// used on disk (`ServiceName_FlowName`).
fn suite_dir_name(suite: &str) -> String {
    suite.replace('/', "_")
}

/// Absolute path to the suite scenario file.
pub fn scenario_file_path(suite: &str) -> PathBuf {
    scenario_root()
        .join(suite_dir_name(suite))
        .join("scenario.json")
}

/// Absolute path to the suite specification file.
pub fn suite_spec_file_path(suite: &str) -> PathBuf {
    scenario_root()
        .join(suite_dir_name(suite))
        .join("suite_spec.json")
}

/// Resolves connector spec path, preferring `<connector>/specs.json` and falling
/// back to legacy `<connector>.json` location.
pub fn connector_spec_file_path(connector: &str) -> PathBuf {
    let directory_spec_path = connector_spec_dir(connector).join("specs.json");
    if directory_spec_path.exists() {
        directory_spec_path
    } else {
        connector_specs_root().join(format!("{connector}.json"))
    }
}

/// Path to connector browser automation hook config file.
pub fn connector_browser_automation_spec_file_path(connector: &str) -> PathBuf {
    connector_spec_dir(connector).join("browser_automation_spec.json")
}

/// Loads all scenarios for a suite from `scenario.json`.
pub fn load_suite_scenarios(suite: &str) -> Result<ScenarioFile, ScenarioError> {
    let path = scenario_file_path(suite);
    let content = fs::read_to_string(&path).map_err(|source| ScenarioError::ScenarioFileRead {
        path: path.clone(),
        source,
    })?;

    serde_json::from_str::<ScenarioFile>(&content)
        .map_err(|source| ScenarioError::ScenarioFileParse { path, source })
}

/// Loads one named scenario definition from the suite file.
pub fn load_scenario(suite: &str, scenario: &str) -> Result<ScenarioDef, ScenarioError> {
    load_suite_scenarios(suite)?
        .get(scenario)
        .cloned()
        .ok_or_else(|| ScenarioError::ScenarioNotFound {
            suite: suite.to_string(),
            scenario: scenario.to_string(),
        })
}

/// Loads suite execution metadata including dependency graph and scope.
pub fn load_suite_spec(suite: &str) -> Result<SuiteSpec, ScenarioError> {
    let path = suite_spec_file_path(suite);
    if !path.exists() {
        return Err(ScenarioError::SuiteSpecMissing { path });
    }

    let content = fs::read_to_string(&path).map_err(|source| ScenarioError::SuiteSpecRead {
        path: path.clone(),
        source,
    })?;

    serde_json::from_str::<SuiteSpec>(&content)
        .map_err(|source| ScenarioError::SuiteSpecParse { path, source })
}

/// Loads optional connector-specific browser automation hooks.
///
/// Returns `None` when the spec file does not exist or cannot be read/parsed.
/// Read and parse failures are logged as warnings rather than propagated.
pub fn load_connector_browser_automation_spec(
    connector: &str,
) -> Option<ConnectorBrowserAutomationSpec> {
    let path = connector_browser_automation_spec_file_path(connector);
    if !path.exists() {
        return None;
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(source) => {
            tracing::warn!(
                path = %path.display(),
                %source,
                "failed to read browser automation spec"
            );
            return None;
        }
    };

    match serde_json::from_str::<ConnectorBrowserAutomationSpec>(&content) {
        Ok(spec) => Some(spec),
        Err(source) => {
            tracing::warn!(
                path = %path.display(),
                %source,
                "failed to parse browser automation spec"
            );
            None
        }
    }
}

/// Returns the unique default scenario name for a suite.
pub fn load_default_scenario_name(suite: &str) -> Result<String, ScenarioError> {
    let scenarios = load_suite_scenarios(suite)?;
    let defaults = scenarios
        .iter()
        .filter_map(|(name, def)| def.is_default.then_some(name.clone()))
        .collect::<Vec<_>>();

    match defaults.as_slice() {
        [] => Err(ScenarioError::DefaultScenarioMissing {
            suite: suite.to_string(),
        }),
        [single] => Ok(single.clone()),
        _ => Err(ScenarioError::MultipleDefaultScenarios {
            suite: suite.to_string(),
            scenarios: defaults.join(", "),
        }),
    }
}

/// Checks whether a connector explicitly supports a suite.
///
/// If connector specs are absent, this falls back to checking suite presence on disk.
pub fn is_suite_supported_for_connector(
    connector: &str,
    suite: &str,
) -> Result<bool, ScenarioError> {
    let path = connector_spec_file_path(connector);
    if path.exists() {
        let content =
            fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorSpecRead {
                path: path.clone(),
                source,
            })?;
        let spec = serde_json::from_str::<ConnectorSuiteSpec>(&content).map_err(|source| {
            ScenarioError::ConnectorSpecParse {
                path: path.clone(),
                source,
            }
        })?;
        return Ok(spec
            .supported_suites
            .iter()
            .any(|supported| supported == suite));
    }

    Ok(scenario_file_path(suite).exists())
}

/// Lists all suites supported by a connector, preserving order from connector
/// spec and removing duplicates.
/// Payment methods this connector declares support for, as `payment_method`
/// oneof variant names. Empty means it has not declared any, which is read as
/// "run everything" rather than "supports nothing".
pub fn load_supported_payment_methods_for_connector(
    connector: &str,
) -> Result<Vec<String>, ScenarioError> {
    let path = connector_spec_file_path(connector);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorSpecRead {
        path: path.clone(),
        source,
    })?;
    let spec = serde_json::from_str::<ConnectorSuiteSpec>(&content).map_err(|source| {
        ScenarioError::ConnectorSpecParse {
            path: path.clone(),
            source,
        }
    })?;
    Ok(spec.supported_payment_methods)
}

/// Scenarios that exist only for one connector, keyed by suite.
///
/// Held apart from `global_suites/` on purpose: the global file stays the single
/// baseline every connector is measured against, and this covers the cases that
/// cannot exist elsewhere — a sandbox-specific trigger, or a production bug
/// pinned as a permanent test. Additive only; shadowing a global scenario is an
/// error, since changing a shared scenario is what `override.json` is for.
pub fn load_connector_specific_scenarios(
    connector: &str,
    suite: &str,
) -> Result<ScenarioFile, ScenarioError> {
    load_connector_specific_scenarios_in(&connector_specs_root(), connector, suite)
}

/// Same, against an explicit specs root. Tests use this rather than rewriting
/// `UCS_CONNECTOR_SPECS_ROOT`, which is process-wide and races every other test
/// that discovers connectors from that directory.
pub fn load_connector_specific_scenarios_in(
    root: &std::path::Path,
    connector: &str,
    suite: &str,
) -> Result<ScenarioFile, ScenarioError> {
    let path = root.join(connector).join(CONNECTOR_SCENARIOS_FILE);
    if !path.exists() {
        return Ok(ScenarioFile::new());
    }
    let content = fs::read_to_string(&path).map_err(|source| ScenarioError::ScenarioFileRead {
        path: path.clone(),
        source,
    })?;
    let by_suite = serde_json::from_str::<BTreeMap<String, Value>>(&content).map_err(|source| {
        ScenarioError::ScenarioFileParse {
            path: path.clone(),
            source,
        }
    })?;
    let Some(scenarios) = by_suite.get(suite) else {
        return Ok(ScenarioFile::new());
    };
    serde_json::from_value::<ScenarioFile>(scenarios.clone())
        .map_err(|source| ScenarioError::ScenarioFileParse { path, source })
}

/// The suite set a connector actually runs: the global baseline plus whatever it
/// declares privately.
///
/// A private name that already exists globally is rejected rather than merged —
/// silently winning over the baseline is exactly how connector-private coverage
/// would start hiding shared regressions.
pub fn merge_connector_specific_scenarios(
    connector: &str,
    suite: &str,
    baseline: &mut ScenarioFile,
) -> Result<usize, ScenarioError> {
    merge_connector_specific_scenarios_in(&connector_specs_root(), connector, suite, baseline)
}

/// Same, against an explicit specs root.
pub fn merge_connector_specific_scenarios_in(
    root: &std::path::Path,
    connector: &str,
    suite: &str,
    baseline: &mut ScenarioFile,
) -> Result<usize, ScenarioError> {
    let private = load_connector_specific_scenarios_in(root, connector, suite)?;
    let count = private.len();
    for (name, def) in private {
        if baseline.contains_key(&name) {
            return Err(ScenarioError::ConnectorSpecParse {
                path: root.join(connector).join(CONNECTOR_SCENARIOS_FILE),
                source: serde::de::Error::custom(format!(
                    "{connector} defines {suite}/{name}, which already exists in the global \
                     suite. This file can only add scenarios; use override.json to change a \
                     shared one."
                )),
            });
        }
        baseline.insert(name, def);
    }
    Ok(count)
}

/// Why this connector declares the scenario unsupported, if it does.
///
/// Declared in `specs.json` next to the rest of the connector's capabilities, so
/// what a connector cannot do is answered by one small file. The reason is the
/// map value, so a declaration without one cannot be expressed.
pub fn scenario_unsupported_reason(
    connector: &str,
    suite: &str,
    scenario: &str,
) -> Result<Option<String>, ScenarioError> {
    let path = connector_spec_file_path(connector);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorSpecRead {
        path: path.clone(),
        source,
    })?;
    let spec = serde_json::from_str::<ConnectorSuiteSpec>(&content).map_err(|source| {
        ScenarioError::ConnectorSpecParse {
            path: path.clone(),
            source,
        }
    })?;
    Ok(spec
        .unsupported_scenarios
        .get(suite)
        .and_then(|scenarios| scenarios.get(scenario))
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty()))
}

/// The `payment_method` oneof variant a scenario populates, if it names one.
///
/// Scenarios that name none act on a payment their dependency already made and
/// inherit its method, so they are never filtered.
pub fn scenario_payment_method(scenario: &ScenarioDef) -> Option<String> {
    scenario
        .grpc_req
        .get("payment_method")?
        .as_object()?
        .keys()
        .next()
        .cloned()
}

/// True when `scenario` should run for a connector declaring `supported`.
pub fn scenario_matches_supported_payment_methods(
    scenario: &ScenarioDef,
    supported: &[String],
) -> bool {
    if supported.is_empty() {
        return true;
    }
    match scenario_payment_method(scenario) {
        Some(method) => supported.iter().any(|m| m == &method),
        None => true,
    }
}

pub fn load_supported_suites_for_connector(connector: &str) -> Result<Vec<String>, ScenarioError> {
    let path = connector_spec_file_path(connector);
    if path.exists() {
        let content =
            fs::read_to_string(&path).map_err(|source| ScenarioError::ConnectorSpecRead {
                path: path.clone(),
                source,
            })?;
        let spec = serde_json::from_str::<ConnectorSuiteSpec>(&content).map_err(|source| {
            ScenarioError::ConnectorSpecParse {
                path: path.clone(),
                source,
            }
        })?;

        let mut suites = Vec::new();
        for suite in spec.supported_suites {
            if !suites.contains(&suite) {
                suites.push(suite);
            }
        }
        return Ok(suites);
    }

    let mut suites = BTreeSet::new();
    for entry in
        fs::read_dir(scenario_root()).map_err(|source| ScenarioError::ScenarioFileRead {
            path: scenario_root(),
            source,
        })?
    {
        let entry = entry.map_err(|source| ScenarioError::ScenarioFileRead {
            path: scenario_root(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !path.join("scenario.json").exists() {
            continue;
        }

        if let Some(suite_name) = suite_dir_name_to_suite_name(dir_name) {
            suites.insert(suite_name);
        }
    }

    Ok(suites.into_iter().collect())
}

/// Loads the full connector spec (`specs.json`) for a connector.
///
/// Returns `None` when no spec file exists or when reading/parsing fails.
/// Read and parse failures are logged as warnings rather than propagated.
pub fn load_connector_spec(connector: &str) -> Option<ConnectorSuiteSpec> {
    let path = connector_spec_file_path(connector);
    if !path.exists() {
        return None;
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(source) => {
            tracing::warn!(
                path = %path.display(),
                %source,
                "failed to read connector spec"
            );
            return None;
        }
    };

    match serde_json::from_str::<ConnectorSuiteSpec>(&content) {
        Ok(spec) => Some(spec),
        Err(source) => {
            tracing::warn!(
                path = %path.display(),
                %source,
                "failed to parse connector spec"
            );
            None
        }
    }
}

/// Discovers connector names by scanning `connector_specs/`.
/// Sits in `connector_specs/` beside the per-connector directories but describes
/// CI policy, not a connector. See `.github/scripts/certify-connectors.sh`.
pub const ALPHA_CONNECTORS_FILE: &str = "alpha_connectors.json";

/// Scenarios that exist only for one connector, beside its `specs.json`.
pub const CONNECTOR_SCENARIOS_FILE: &str = "connector_specific_scenarios.json";

pub fn discover_all_connectors() -> Result<Vec<String>, ScenarioError> {
    let specs_dir = connector_specs_root();

    if !specs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut connectors = BTreeSet::new();
    for entry in fs::read_dir(&specs_dir).map_err(|source| ScenarioError::ScenarioFileRead {
        path: specs_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| ScenarioError::ScenarioFileRead {
            path: specs_dir.clone(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            let has_specs_file = path.join("specs.json").is_file();
            if has_specs_file {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    connectors.insert(name.to_string());
                }
            }
            continue;
        }

        if !path.is_file() {
            continue;
        }

        // Records which connectors CI does not certify — not a connector spec.
        // The flat-layout branch below would otherwise read it as a connector
        // named "alpha_connectors" and fail to parse it as one.
        if path.file_name().and_then(|s| s.to_str()) == Some(ALPHA_CONNECTORS_FILE) {
            continue;
        }

        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if ext == "json" {
            connectors.insert(name.to_string());
        }
    }

    Ok(connectors.into_iter().collect())
}

/// Resolves connector list for all-connector runs.
///
/// Environment override format: `UCS_ALL_CONNECTORS=stripe,paypal,authorizedotnet`.
/// When no override is set, the list is auto-discovered from `connector_specs/`
/// directories that contain a `specs.json` file.
pub fn configured_all_connectors() -> Vec<String> {
    if let Ok(raw) = std::env::var("UCS_ALL_CONNECTORS") {
        let connectors = raw
            .split(',')
            .map(str::trim)
            .filter(|connector| !connector.is_empty())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        if !connectors.is_empty() {
            return connectors;
        }
    }

    discover_all_connectors().unwrap_or_else(|err| {
        tracing::warn!(
            %err,
            "failed to discover connectors in connector_specs/"
        );
        Vec::new()
    })
}

/// Convenience accessor used by runners to load request template JSON.
pub fn get_the_grpc_req(suite: &str, scenario: &str) -> Result<Value, ScenarioError> {
    Ok(load_scenario(suite, scenario)?.grpc_req)
}

/// Convenience accessor used by runners to load assertion rules.
pub fn get_the_assertion(
    suite: &str,
    scenario: &str,
) -> Result<std::collections::BTreeMap<String, FieldAssert>, ScenarioError> {
    Ok(load_scenario(suite, scenario)?.assert_rules)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{merge_connector_specific_scenarios_in, CONNECTOR_SCENARIOS_FILE};
    use crate::harness::scenario_types::{ConnectorSuiteSpec, DependencyScope, ScenarioFile};

    fn unique_specs_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("ucs_connector_specs_test_{nanos}"))
    }

    use serde_json::json;

    use crate::harness::scenario_loader::{
        configured_all_connectors, discover_all_connectors, get_the_assertion, get_the_grpc_req,
        load_scenario, load_suite_scenarios, load_suite_spec, load_supported_suites_for_connector,
        scenario_matches_supported_payment_methods, scenario_payment_method, scenario_root,
        suite_dir_name_to_suite_name,
    };
    use crate::harness::scenario_types::ScenarioDef;

    /// A scenario carrying only the request shape these tests care about.
    fn def(grpc_req: serde_json::Value) -> ScenarioDef {
        ScenarioDef {
            grpc_req,
            assert_rules: std::collections::BTreeMap::new(),
            is_default: false,
            display_name: None,
        }
    }

    #[test]
    fn suite_dir_name_to_suite_name_splits_at_first_underscore() {
        assert_eq!(
            suite_dir_name_to_suite_name("PaymentService_Authorize"),
            Some("PaymentService/Authorize".to_string())
        );
        assert_eq!(
            suite_dir_name_to_suite_name(
                "MerchantAuthenticationService_CreateClientAuthenticationToken"
            ),
            Some("MerchantAuthenticationService/CreateClientAuthenticationToken".to_string())
        );
        assert_eq!(
            suite_dir_name_to_suite_name("RefundService_Get"),
            Some("RefundService/Get".to_string())
        );
    }

    #[test]
    fn suite_dir_name_to_suite_name_returns_none_when_no_underscore() {
        assert_eq!(suite_dir_name_to_suite_name("PaymentService"), None);
        assert_eq!(suite_dir_name_to_suite_name(""), None);
    }

    #[test]
    fn all_global_suite_dirs_produce_valid_suite_names() {
        let entries = fs::read_dir(scenario_root()).expect("scenario root should be readable");
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("dir name should be valid UTF-8");
            let suite_name = suite_dir_name_to_suite_name(dir_name)
                .unwrap_or_else(|| panic!("directory {dir_name:?} has no underscore separator"));
            let (service, method) = suite_name
                .split_once('/')
                .unwrap_or_else(|| panic!("suite name {suite_name:?} must contain '/'"));
            assert!(
                !service.is_empty() && !method.is_empty(),
                "suite name {suite_name:?} must have non-empty service and method"
            );
        }
    }

    fn discover_suites() -> Vec<String> {
        fs::read_dir(scenario_root())
            .expect("scenario root should be readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let path = entry.path();
                let has_scenario_file = path.join("scenario.json").is_file();
                let dir_name = path.file_name()?.to_str()?;
                if !has_scenario_file {
                    return None;
                }
                suite_dir_name_to_suite_name(dir_name)
            })
            .collect()
    }

    #[test]
    fn can_load_any_scenario_by_name_if_present() {
        let suites = discover_suites();
        assert!(!suites.is_empty(), "at least one suite should exist");

        for suite in suites {
            let scenarios =
                load_suite_scenarios(&suite).expect("suite scenarios should be readable");
            assert!(
                !scenarios.is_empty(),
                "suite '{suite}' should contain at least one scenario"
            );

            for scenario_name in scenarios.keys() {
                let scenario =
                    load_scenario(&suite, scenario_name).expect("scenario should be loadable");

                assert!(
                    scenario.grpc_req.is_object(),
                    "scenario '{scenario_name}' in suite '{suite}' should have object grpc_req"
                );

                assert!(
                    !scenario.assert_rules.is_empty(),
                    "scenario '{scenario_name}' in suite '{suite}' should have assertion rules"
                );
            }
        }
    }

    #[test]
    fn can_get_grpc_req_and_assertions_for_any_existing_scenario() {
        let suites = discover_suites();
        assert!(!suites.is_empty(), "at least one suite should exist");

        for suite in suites {
            let scenarios =
                load_suite_scenarios(&suite).expect("suite scenarios should be readable");
            for scenario_name in scenarios.keys() {
                let req = get_the_grpc_req(&suite, scenario_name)
                    .expect("grpc request should be available for scenario");
                let assertions = get_the_assertion(&suite, scenario_name)
                    .expect("assertions should be available for scenario");

                assert!(
                    req.is_object(),
                    "grpc_req should be object for '{suite}/{scenario_name}'"
                );

                assert!(
                    !assertions.is_empty(),
                    "assertions should be present for '{suite}/{scenario_name}'"
                );
            }
        }
    }

    #[test]
    fn can_load_suite_specs_for_all_suites() {
        let suites = discover_suites();
        assert!(!suites.is_empty(), "at least one suite should exist");

        for suite in suites {
            let spec = load_suite_spec(&suite).expect("suite spec should be readable");
            assert_eq!(
                spec.suite, suite,
                "suite spec name should match folder name"
            );
            for dependency in &spec.depends_on {
                let dependency_suite = dependency.suite();
                assert!(
                    !dependency_suite.is_empty(),
                    "dependency suite name should not be empty"
                );

                if let Some(dependency_scenario) = dependency.scenario() {
                    load_scenario(dependency_suite, dependency_scenario)
                        .expect("dependency override scenario should exist");
                }
            }
        }
    }

    #[test]
    fn dependency_scope_defaults_and_overrides_are_loaded() {
        let authorize_spec =
            load_suite_spec("PaymentService/Authorize").expect("authorize spec should load");
        assert_eq!(authorize_spec.dependency_scope, DependencyScope::Scenario);

        for suite in [
            "PaymentService/Capture",
            "PaymentService/Void",
            "PaymentService/Refund",
            "PaymentService/Get",
            "RefundService/Get",
        ] {
            let spec = load_suite_spec(suite).expect("suite spec should load");
            assert_eq!(
                spec.dependency_scope,
                DependencyScope::Scenario,
                "suite '{suite}' should run dependencies per scenario"
            );
        }
    }

    #[test]
    fn explicit_context_maps_exist_for_name_mismatch_dependencies() {
        let recurring_spec = load_suite_spec("RecurringPaymentService/Charge")
            .expect("RecurringPaymentService/Charge spec should load");
        let recurring_has_mandate_mapping = recurring_spec.depends_on.iter().any(|dependency| {
            dependency
                .context_map()
                .and_then(|map| {
                    map.get(
                        "connector_recurring_payment_id.connector_mandate_id.connector_mandate_id",
                    )
                })
                .map(|source| {
                    source == "res.mandate_reference.connector_mandate_id.connector_mandate_id"
                })
                .unwrap_or(false)
        });
        assert!(
            recurring_has_mandate_mapping,
            "RecurringPaymentService/Charge should explicitly map mandate reference into connector recurring id"
        );

        let refund_sync_spec =
            load_suite_spec("RefundService/Get").expect("RefundService/Get spec should load");
        let refund_sync_has_refund_mapping = refund_sync_spec.depends_on.iter().any(|dependency| {
            dependency
                .context_map()
                .and_then(|map| map.get("refund_id"))
                .map(|source| source == "res.connector_refund_id")
                .unwrap_or(false)
        });
        assert!(
            refund_sync_has_refund_mapping,
            "RefundService/Get should explicitly map refund_id from connector_refund_id"
        );
    }

    #[test]
    fn can_load_supported_suites_for_known_connector() {
        let suites = load_supported_suites_for_connector("stripe")
            .expect("supported suites should load for stripe connector");
        assert!(
            suites
                .iter()
                .any(|suite| suite == "PaymentService/Authorize"),
            "stripe should support PaymentService/Authorize suite"
        );
    }

    #[test]
    fn can_discover_all_connectors() {
        let connectors =
            discover_all_connectors().expect("should discover connectors from connector_specs/");
        assert!(
            !connectors.is_empty(),
            "at least one connector spec should exist"
        );
        assert!(
            connectors.iter().any(|c| c == "stripe"),
            "stripe connector spec should be discoverable"
        );
        // Should be sorted
        let mut sorted = connectors.clone();
        sorted.sort();
        assert_eq!(connectors, sorted, "connectors should be sorted");
    }

    #[test]
    fn configured_connectors_defaults_to_static_run_list() {
        let previous = std::env::var("UCS_ALL_CONNECTORS").ok();
        std::env::remove_var("UCS_ALL_CONNECTORS");

        let connectors = configured_all_connectors();

        match previous {
            Some(value) => std::env::set_var("UCS_ALL_CONNECTORS", value),
            None => std::env::remove_var("UCS_ALL_CONNECTORS"),
        }

        assert!(connectors.iter().any(|connector| connector == "stripe"));
        assert!(connectors
            .iter()
            .any(|connector| connector == "authorizedotnet"));
        assert!(connectors.iter().any(|connector| connector == "paypal"));
        assert!(!connectors.is_empty());
    }

    #[test]
    fn connector_specific_scenarios_are_added_but_may_not_shadow() {
        let temp_root = unique_specs_dir();
        let connector_dir = temp_root.join("tsys");
        fs::create_dir_all(&connector_dir).expect("connector dir should be created");
        fs::write(
            connector_dir.join(CONNECTOR_SCENARIOS_FILE),
            serde_json::json!({
                "PaymentService/Authorize": {
                    "tsys_soft_decline": {
                        "grpc_req": { "amount": { "minor_amount": 5205 } },
                        "assert": { "status": { "one_of": ["FAILURE"] } }
                    }
                }
            })
            .to_string(),
        )
        .expect("scenario file should be written");

        // Additive: a name the baseline does not have is merged in.
        let mut baseline = ScenarioFile::new();
        let added = merge_connector_specific_scenarios_in(
            &temp_root,
            "tsys",
            "PaymentService/Authorize",
            &mut baseline,
        )
        .expect("a private scenario should merge into an empty baseline");
        assert_eq!(added, 1);
        assert!(baseline.contains_key("tsys_soft_decline"));

        // A suite the connector declares nothing for is left alone.
        let mut other = ScenarioFile::new();
        let none = merge_connector_specific_scenarios_in(
            &temp_root,
            "tsys",
            "PaymentService/Refund",
            &mut other,
        )
        .expect("a suite with no private scenarios should be a no-op");
        assert_eq!(none, 0);
        assert!(other.is_empty());

        // Shadowing is the failure mode this file must never enable: a private
        // scenario silently winning over the baseline would hide a shared
        // regression behind connector-private coverage.
        let mut collides = baseline.clone();
        let err = merge_connector_specific_scenarios_in(
            &temp_root,
            "tsys",
            "PaymentService/Authorize",
            &mut collides,
        )
        .expect_err("a name already in the baseline must be rejected");
        assert!(
            err.to_string()
                .contains("already exists in the global suite"),
            "the error should explain the collision, got: {err}"
        );

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn a_connector_without_the_file_adds_nothing() {
        let temp_root = unique_specs_dir();
        fs::create_dir_all(temp_root.join("stripe")).expect("dir should be created");
        let mut baseline = ScenarioFile::new();
        let added = merge_connector_specific_scenarios_in(
            &temp_root,
            "stripe",
            "PaymentService/Authorize",
            &mut baseline,
        )
        .expect("a missing file is not an error");
        assert_eq!(added, 0);

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn unsupported_scenarios_are_read_from_the_spec() {
        // The reason is the map value, so "declared unsupported with no reason"
        // is not expressible — the case the override-based version needed a
        // runtime error and a test to catch.
        let spec: ConnectorSuiteSpec = serde_json::from_value(serde_json::json!({
            "connector": "stripe",
            "supported_suites": ["PaymentService/Authorize"],
            "unsupported_scenarios": {
                "PaymentService/Authorize": {
                    "no3ds_auto_capture_upi_qr": "stripe returns NotImplemented for UPI"
                }
            }
        }))
        .expect("spec with unsupported_scenarios should parse");

        assert_eq!(
            spec.unsupported_scenarios["PaymentService/Authorize"]["no3ds_auto_capture_upi_qr"],
            "stripe returns NotImplemented for UPI"
        );
        assert!(spec
            .unsupported_scenarios
            .get("PaymentService/Capture")
            .is_none());
    }

    #[test]
    fn a_spec_without_unsupported_scenarios_still_parses() {
        let spec: ConnectorSuiteSpec = serde_json::from_value(serde_json::json!({
            "connector": "tsys",
            "supported_suites": ["PaymentService/Authorize"]
        }))
        .expect("the field is optional");
        assert!(spec.unsupported_scenarios.is_empty());
    }

    #[test]
    fn configured_connectors_supports_env_override() {
        let previous = std::env::var("UCS_ALL_CONNECTORS").ok();
        std::env::set_var("UCS_ALL_CONNECTORS", "stripe, adyen, stripe, ,rapyd");

        let connectors = configured_all_connectors();

        match previous {
            Some(value) => std::env::set_var("UCS_ALL_CONNECTORS", value),
            None => std::env::remove_var("UCS_ALL_CONNECTORS"),
        }

        assert_eq!(connectors, vec!["adyen", "rapyd", "stripe"]);
    }

    #[test]
    fn recurring_charge_scenarios_exclude_unsupported_connector_transaction_field() {
        for scenario_name in [
            "recurring_charge",
            "recurring_charge_low_amount",
            "recurring_charge_with_order_context",
        ] {
            let req = get_the_grpc_req("RecurringPaymentService/Charge", scenario_name)
                .expect("recurring charge grpc_req should be loadable");
            assert!(
                req.get("connector_transaction_id").is_none(),
                "RecurringPaymentService/Charge/{scenario_name} should not include connector_transaction_id"
            );
        }
    }

    #[test]
    fn setup_recurring_extended_scenarios_have_billing_address() {
        for scenario_name in [
            "setup_recurring_with_webhook",
            "setup_recurring_with_order_context",
        ] {
            let req = get_the_grpc_req("PaymentService/SetupRecurring", scenario_name)
                .expect("setup_recurring grpc_req should be loadable");

            let has_billing_address = req
                .get("address")
                .and_then(|address| address.get("billing_address"))
                .is_some();
            assert!(
                has_billing_address,
                "PaymentService/SetupRecurring/{scenario_name} should include address.billing_address"
            );
        }
    }

    #[test]
    fn three_connector_suite_coverage_includes_recurring_flows() {
        let authorizedotnet = load_supported_suites_for_connector("authorizedotnet")
            .expect("authorizedotnet supported suites should load");
        assert!(
            authorizedotnet.contains(&"PaymentService/SetupRecurring".to_string())
                && authorizedotnet.contains(&"RecurringPaymentService/Charge".to_string()),
            "authorizedotnet should cover recurring suites"
        );

        let stripe =
            load_supported_suites_for_connector("stripe").expect("stripe suites should load");
        assert!(
            stripe.contains(&"CustomerService/Create".to_string())
                && stripe.contains(&"PaymentService/SetupRecurring".to_string())
                && stripe.contains(&"RecurringPaymentService/Charge".to_string()),
            "stripe should include CustomerService/Create + recurring suites"
        );

        let paypal =
            load_supported_suites_for_connector("paypal").expect("paypal suites should load");
        assert!(
            paypal.contains(
                &"MerchantAuthenticationService/CreateServerAuthenticationToken".to_string()
            ) && paypal.contains(&"PaymentService/SetupRecurring".to_string())
                && paypal.contains(&"RecurringPaymentService/Charge".to_string()),
            "paypal should include token + recurring suites"
        );
    }

    #[test]
    fn a_scenario_naming_no_payment_method_always_runs() {
        // Capture, Void, Get, Refund and the rest act on a payment their
        // dependency already made, so they carry no method of their own.
        let scenario = def(json!({ "amount": { "minor_amount": 100 } }));
        assert!(scenario_payment_method(&scenario).is_none());
        assert!(scenario_matches_supported_payment_methods(&scenario, &[]));
        assert!(scenario_matches_supported_payment_methods(
            &scenario,
            &["card".to_string()]
        ));
    }

    #[test]
    fn declaring_nothing_keeps_every_scenario() {
        // The field is opt-in: a connector that has not declared its methods
        // must behave exactly as it did before the field existed.
        let upi = def(json!({ "payment_method": { "upi_intent": {} } }));
        assert!(scenario_matches_supported_payment_methods(&upi, &[]));
    }

    #[test]
    fn a_declared_method_runs_and_an_undeclared_one_does_not() {
        let card = def(json!({ "payment_method": { "card": { "card_number": "4242" } } }));
        let upi = def(json!({ "payment_method": { "upi_intent": {} } }));
        let declared = vec!["card".to_string()];

        assert_eq!(scenario_payment_method(&card).as_deref(), Some("card"));
        assert!(scenario_matches_supported_payment_methods(&card, &declared));
        assert!(!scenario_matches_supported_payment_methods(&upi, &declared));
    }
}
