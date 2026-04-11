#![allow(clippy::too_many_arguments)]

//! Core orchestration layer for UCS scenario execution.
//!
//! Responsibilities include loading scenario templates, applying connector
//! overrides, resolving dependency context, building grpcurl/tonic payloads,
//! dispatching RPC calls, and returning structured per-scenario results.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use reqwest::{blocking::Client, Url};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::harness::{
    auto_gen::resolve_auto_generate,
    connector_override::{
        apply_connector_overrides, normalize_tonic_request_for_connector,
        transform_response_for_connector,
    },
    credentials::{creds_file_path, load_connector_config},
    metadata::add_connector_metadata,
    scenario_assert::do_assertion as do_assertion_impl,
    scenario_loader::{
        configured_all_connectors, get_the_assertion as get_the_assertion_impl,
        get_the_grpc_req as get_the_grpc_req_impl, is_suite_supported_for_connector,
        load_connector_browser_automation_spec, load_connector_spec, load_default_scenario_name,
        load_scenario, load_suite_scenarios, load_suite_spec, load_supported_suites_for_connector,
    },
    scenario_types::{
        BrowserAutomationHook, BrowserAutomationPhase, CliPreRequestHookConfig, ContextMap,
        DependencyScope, FieldAssert, ScenarioError, SuiteDependency, SuiteSpec,
    },
    sdk_executor::execute_sdk_request_from_payload,
};

/// Loads raw request template for `(suite, scenario)` without connector override.
pub fn get_the_grpc_req(suite: &str, scenario: &str) -> Result<Value, ScenarioError> {
    get_the_grpc_req_impl(suite, scenario)
}

/// Loads assertion rules for `(suite, scenario)` without connector override.
pub fn get_the_assertion(
    suite: &str,
    scenario: &str,
) -> Result<BTreeMap<String, FieldAssert>, ScenarioError> {
    get_the_assertion_impl(suite, scenario)
}

/// Loads scenario and applies connector-specific request/assertion patches.
fn load_effective_scenario_for_connector(
    suite: &str,
    scenario: &str,
    connector: &str,
) -> Result<(Value, BTreeMap<String, FieldAssert>), ScenarioError> {
    let base_scenario = load_scenario(suite, scenario)?;
    let mut grpc_req = base_scenario.grpc_req;
    let mut assertions = base_scenario.assert_rules;

    apply_connector_overrides(connector, suite, scenario, &mut grpc_req, &mut assertions)?;
    Ok((grpc_req, assertions))
}

/// Loads request template after applying connector-specific overrides.
pub fn get_the_grpc_req_for_connector(
    suite: &str,
    scenario: &str,
    connector: &str,
) -> Result<Value, ScenarioError> {
    let (grpc_req, _assertions) =
        load_effective_scenario_for_connector(suite, scenario, connector)?;
    Ok(grpc_req)
}

/// Loads assertion rules after applying connector-specific overrides.
pub fn get_the_assertion_for_connector(
    suite: &str,
    scenario: &str,
    connector: &str,
) -> Result<BTreeMap<String, FieldAssert>, ScenarioError> {
    let (_grpc_req, assertions) =
        load_effective_scenario_for_connector(suite, scenario, connector)?;
    Ok(assertions)
}

/// Public assertion entrypoint used by runners.
pub fn do_assertion(
    assertions_for_that_req: &BTreeMap<String, FieldAssert>,
    response_json: &Value,
    grpc_req: &Value,
) -> Result<(), ScenarioError> {
    do_assertion_impl(assertions_for_that_req, response_json, grpc_req)
}

pub const DEFAULT_SUITE: &str = "PaymentService/Authorize";
pub const DEFAULT_SCENARIO: &str = "no3ds_auto_capture_credit_card";
pub const DEFAULT_ENDPOINT: &str = "localhost:8000";
pub const DEFAULT_CONNECTOR: &str = "stripe";
pub const DEFAULT_MERCHANT_ID: &str = "test_merchant";
pub const DEFAULT_TENANT_ID: &str = "default";
const DEFAULT_BROWSER_AUTOMATION_URL: &str = "http://localhost:3000/run";

/// Materialized grpcurl command pieces used by CLI output and execution.
#[derive(Debug, Clone)]
pub struct GrpcurlRequest {
    pub endpoint: String,
    pub method: String,
    pub payload: String,
    pub headers: Vec<String>,
    pub plaintext: bool,
}

#[derive(Debug, Clone)]
pub struct GrpcExecutionResult {
    pub response_body: String,
    pub request_command: String,
    pub response_output: String,
    pub success: bool,
}

type ExplicitContextEntry = (ContextMap, Value, Value);
#[derive(Debug, Clone)]
struct ExecutedDependency {
    suite: String,
    scenario: String,
    res: Value,
}

#[derive(Debug, Clone)]
struct BrowserRedirectTarget {
    url: String,
    redirect_uri: Option<String>,
    cleanup_path: Option<PathBuf>,
    form_fields: BTreeMap<String, String>,
}

type DependencyContext = (
    Vec<Value>,
    Vec<Value>,
    Vec<String>,
    Vec<ExplicitContextEntry>,
    Vec<ExecutedDependency>,
);

impl GrpcurlRequest {
    /// Renders a shell-friendly multi-line grpcurl command.
    pub fn to_command_string(&self) -> String {
        let mut cmd = String::new();
        cmd.push_str("grpcurl");
        if self.plaintext {
            cmd.push_str(" -plaintext");
        }
        cmd.push_str(" \\\n");

        for header in &self.headers {
            cmd.push_str(&format!("  -H \"{header}\" \\\n"));
        }

        cmd.push_str(&format!(
            "  -d @ {} {} <<'JSON'\n",
            self.endpoint, self.method
        ));
        cmd.push_str(&self.payload);
        cmd.push_str("\nJSON");
        cmd
    }
}

/// Validates request template loading for one scenario target.
pub fn run_test(
    suite: Option<&str>,
    scenario: Option<&str>,
    connector: Option<&str>,
) -> Result<(), ScenarioError> {
    let suite = suite.unwrap_or(DEFAULT_SUITE);
    let scenario = scenario.unwrap_or(DEFAULT_SCENARIO);
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);

    let _ = get_the_grpc_req_for_connector(suite, scenario, connector)?;
    Ok(())
}

fn connector_request_reference_id_for(
    connector: &str,
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
) -> String {
    // Load connector spec to check for custom reference ID configuration.
    // Silently ignore errors — an absent spec means default behaviour.
    if let Some(spec) = load_connector_spec(connector) {
        if let Some(source_field) = spec.request_id_source_field.as_deref() {
            if let Some(value) =
                lookup_json_path_with_case_fallback(grpc_req, source_field).and_then(Value::as_str)
            {
                if !value.trim().is_empty() {
                    return value.to_string();
                }
            }

            // Source field absent or empty — generate with optional prefix/length.
            let prefix = spec.request_id_prefix.as_deref().unwrap_or("");
            let uuid_part = format!("{}{}", prefix, Uuid::new_v4().simple());
            return match spec.request_id_length {
                Some(len) => uuid_part.chars().take(len).collect(),
                None => uuid_part,
            };
        }
    }

    format!("{suite}_{scenario}_ref")
}

/// Applies implicit context propagation by matching similarly-named fields from
/// previously executed requests/responses.
pub fn add_context(
    prev_grpc_reqs: &[Value],
    prev_grpc_res: &[Value],
    current_grpc_req: &mut Value,
) {
    let mut paths = Vec::new();
    collect_leaf_paths(current_grpc_req, String::new(), &mut paths);

    for path in paths {
        let mut selected: Option<Value> = None;
        let source_paths = source_path_candidates(&path);
        let max_len = prev_grpc_reqs.len().max(prev_grpc_res.len());
        for index in 0..max_len {
            if let Some(req) = prev_grpc_reqs.get(index) {
                if let Some(value) = lookup_first_non_null_path(req, &source_paths) {
                    selected = Some(value.clone());
                }
            }
            if let Some(res) = prev_grpc_res.get(index) {
                if let Some(value) = lookup_first_non_null_path(res, &source_paths) {
                    selected = Some(value.clone());
                }
            }
        }

        if let Some(value) = selected {
            let value_to_set = if path.ends_with(".value") {
                value.get("value").cloned().unwrap_or_else(|| value.clone())
            } else {
                value
            };
            let _ = set_json_path_value(current_grpc_req, &path, value_to_set);
        }
    }
}

fn prepare_context_placeholders(suite: &str, _connector: &str, current_grpc_req: &mut Value) {
    // Ensure metadata target exists for flows that need dependency-carried connector metadata.
    if matches!(
        suite,
        "PaymentService/Capture"
            | "PaymentService/Void"
            | "PaymentService/Refund"
            | "PaymentService/Get"
            | "RefundService/Get"
    ) && lookup_json_path_with_case_fallback(current_grpc_req, "connector_feature_data.value")
        .is_none()
    {
        let _ = deep_set_json_path(
            current_grpc_req,
            "connector_feature_data.value",
            Value::String("auto_generate".to_string()),
        );
    }

    // NOTE: We intentionally do NOT normalize empty/null values to "auto_generate".
    // Templates use explicit "auto_generate" sentinels for fields that should be
    // generated.  Empty strings ("") and null values mean "omit or send empty" —
    // they will be filled by dependency context if available, otherwise left as-is.
}

/// Cleans up empty wrapper objects that may remain after context propagation
/// and auto-generation (e.g. an empty `state.access_token` wrapper when none
/// of its children were filled).
///
/// Uses `has_only_default_leaves` to detect subtrees where ALL leaf values are
/// defaults (`""`, `0`, `null`, `false`).  This handles cases like an
/// `access_token` block with `{"token": {"value": ""}, "expires_in_seconds": 0}`
/// that should be pruned when dependency context didn't fill any real values.
fn prune_empty_context_wrappers(current_grpc_req: &mut Value) {
    let should_remove_connector_feature =
        lookup_json_path_with_case_fallback(current_grpc_req, "connector_feature_data")
            .map(is_unresolved_connector_feature_data)
            .unwrap_or(false);
    if should_remove_connector_feature {
        let _ = remove_json_path(current_grpc_req, "connector_feature_data");
    }

    // Cleanup optional wrappers that contain only default leaf values.
    // Order matters: prune inner paths first, then check outer wrappers.
    let _ = remove_json_path_if_all_defaults(current_grpc_req, "state.access_token.token");
    let _ = remove_json_path_if_all_defaults(current_grpc_req, "state.access_token");
    let _ = remove_json_path_if_all_defaults(current_grpc_req, "state");
    let _ = remove_json_path_if_all_defaults(current_grpc_req, "connector_feature_data");
}

fn maybe_execute_browser_automation_for_suite(
    suite: &str,
    scenario: &str,
    connector: &str,
    dependency_entries: &[ExecutedDependency],
    effective_req: &mut Value,
) -> Result<(), ScenarioError> {
    // Convention-based Google Pay token generation
    // If the request has payment_method.google_pay.tokenization_data.encrypted_data.token,
    // automatically generate a real token via browser automation
    if let Some(token_field) =
        effective_req.pointer("/payment_method/google_pay/tokenization_data/encrypted_data/token")
    {
        if token_field.is_string() {
            return execute_google_pay_token_generation(suite, scenario, connector, effective_req);
        }
    }

    // Load connector-specific browser automation spec (for custom flows like 3DS)
    let Some(config) = load_connector_browser_automation_spec(connector) else {
        return Ok(());
    };

    let Some(browser_automation) =
        select_connector_browser_automation_hook(&config.hooks, suite, scenario)
    else {
        return Ok(());
    };

    if browser_automation.phase == BrowserAutomationPhase::CliPreRequest {
        let Some(cli_config) = browser_automation.cli_pre_request.as_ref() else {
            return Err(ScenarioError::GrpcurlExecution {
                message: format!(
                    "browser automation hook for '{suite}/{scenario}' has phase \
                     'cli_pre_request' but no 'cli_pre_request' config block"
                ),
            });
        };
        return execute_cli_pre_request(suite, scenario, connector, cli_config, effective_req);
    }

    if browser_automation.phase != BrowserAutomationPhase::BeforeRequest {
        return Ok(());
    }

    if browser_automation.rules.is_empty() {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!("browser automation configuration for suite '{suite}' has no rules"),
        });
    }

    let Some(redirect_target) = extract_redirect_target_from_dependencies(
        dependency_entries,
        effective_req,
        browser_automation,
    )?
    else {
        return Ok(());
    };

    let rules = materialize_browser_rules(
        &browser_automation.rules,
        redirect_target.redirect_uri.as_deref(),
    );
    if rules.is_empty() {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation rules resolved to empty set for '{suite}/{scenario}'"
            ),
        });
    }

    let managed_engine = env_bool("UCS_BROWSER_AUTOMATION_MANAGED", true);
    let headed = env_bool("UCS_BROWSER_AUTOMATION_HEADED", true);
    let slow_mo_ms = std::env::var("UCS_BROWSER_AUTOMATION_SLOW_MO_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let payload = serde_json::json!({
        "url": redirect_target.url,
        "rules": rules,
        "options": {
            "headless": !headed,
            "slowMoMs": slow_mo_ms,
            "defaultTimeoutMs": 12000,
            "navigationTimeoutMs": 25000
        }
    });

    let execution_result = if managed_engine {
        let host = std::env::var("UCS_BROWSER_AUTOMATION_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("UCS_BROWSER_AUTOMATION_PORT")
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .unwrap_or(3000);
        let run_url = format!("http://{host}:{port}/run");

        let mut engine_process = start_browser_automation_engine(&host, port)?;
        let execution_result = execute_browser_automation_run(&run_url, &payload, suite, scenario);
        stop_browser_automation_engine(&mut engine_process);
        execution_result
    } else {
        let run_url = std::env::var("UCS_BROWSER_AUTOMATION_URL")
            .unwrap_or_else(|_| DEFAULT_BROWSER_AUTOMATION_URL.to_string());
        execute_browser_automation_run(&run_url, &payload, suite, scenario)
    };

    if let Some(cleanup_path) = redirect_target.cleanup_path.as_ref() {
        let _ = std::fs::remove_file(cleanup_path);
    }

    let browser_response = execution_result?;
    apply_browser_result_to_request(
        effective_req,
        browser_automation,
        &browser_response,
        &redirect_target,
    )?;

    Ok(())
}

fn apply_browser_result_to_request(
    effective_req: &mut Value,
    hook: &BrowserAutomationHook,
    browser_response: &Value,
    redirect_target: &BrowserRedirectTarget,
) -> Result<(), ScenarioError> {
    if hook.final_url_query_param_map.is_empty()
        && hook.fallback_form_field_map.is_empty()
        && hook.browser_data_map.is_empty()
    {
        return Ok(());
    }

    let final_url = browser_response
        .get("finalUrl")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let parsed_url = match final_url.as_deref() {
        Some(url) => Some(
            Url::parse(url).map_err(|error| ScenarioError::GrpcurlExecution {
                message: format!("invalid browser finalUrl '{url}': {error}"),
            })?,
        ),
        None => None,
    };

    let mut target_paths = hook
        .final_url_query_param_map
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for target_path in hook.browser_data_map.keys() {
        if !target_paths.iter().any(|existing| existing == target_path) {
            target_paths.push(target_path.clone());
        }
    }
    for target_path in hook.fallback_form_field_map.keys() {
        if !target_paths.iter().any(|existing| existing == target_path) {
            target_paths.push(target_path.clone());
        }
    }

    let browser_data = browser_response
        .get("data")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for target_path in target_paths {
        let query_value = hook
            .final_url_query_param_map
            .get(&target_path)
            .and_then(|query_key| {
                parsed_url.as_ref().and_then(|url| {
                    url.query_pairs()
                        .find(|(key, _)| key == query_key)
                        .map(|(_, value)| value.into_owned())
                })
            })
            .or_else(|| {
                hook.browser_data_map.get(&target_path).and_then(|data_key| {
                    browser_data.get(data_key).and_then(|value| {
                        if let Some(text) = value.as_str() {
                            return Some(text.to_string());
                        }
                        if let Some(items) = value.as_array() {
                            return items.first().and_then(|first| {
                                first
                                    .as_str()
                                    .map(ToString::to_string)
                                    .or_else(|| Some(first.to_string()))
                            });
                        }
                        Some(value.to_string())
                    })
                })
            })
            .or_else(|| {
                hook.fallback_form_field_map
                    .get(&target_path)
                    .and_then(|field_key| redirect_target.form_fields.get(field_key))
                    .cloned()
            })
            .ok_or_else(|| ScenarioError::GrpcurlExecution {
                message: format!(
                    "browser automation could not resolve value for request target '{target_path}'; finalUrl='{}'",
                    final_url.as_deref().unwrap_or("<missing>")
                ),
            })?;

        let _ = set_json_path_value(effective_req, &target_path, Value::String(query_value));
    }

    Ok(())
}

fn render_auto_submit_form_file(
    endpoint: &str,
    form_fields: &serde_json::Map<String, Value>,
) -> Result<String, ScenarioError> {
    let mut hidden_inputs = String::new();
    for (key, value) in form_fields {
        let field_value = value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| value.to_string());
        hidden_inputs.push_str(&format!(
            "<input type=\"hidden\" name=\"{}\" value=\"{}\" />",
            html_escape(key),
            html_escape(&field_value)
        ));
    }

    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head><body><form id=\"ucs-3ds-auto-form\" method=\"POST\" action=\"{}\">{}<noscript><button type=\"submit\">Continue</button></noscript></form><script>document.getElementById('ucs-3ds-auto-form').submit();</script></body></html>",
        html_escape(endpoint),
        hidden_inputs
    );

    let file_name = format!("ucs-connector-3ds-{}.html", Uuid::new_v4().simple());
    let file_path = std::env::temp_dir().join(file_name);
    std::fs::write(&file_path, html).map_err(|error| ScenarioError::GrpcurlExecution {
        message: format!(
            "failed to write browser automation POST form file '{}': {error}",
            file_path.display()
        ),
    })?;

    let file_url =
        Url::from_file_path(&file_path).map_err(|_| ScenarioError::GrpcurlExecution {
            message: format!(
                "failed to convert browser automation form path '{}' to file URL",
                file_path.display()
            ),
        })?;

    Ok(file_url.to_string())
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn is_post_redirect_method(method: Option<&str>) -> bool {
    method
        .map(|value| value.trim().to_ascii_uppercase().contains("POST"))
        .unwrap_or(false)
}

fn select_connector_browser_automation_hook<'a>(
    hooks: &'a [BrowserAutomationHook],
    suite: &str,
    scenario: &str,
) -> Option<&'a BrowserAutomationHook> {
    hooks.iter().find(|hook| {
        hook.suite == suite
            && (hook.scenarios.is_empty() || hook.scenarios.iter().any(|name| name == scenario))
    })
}

fn execute_browser_automation_run(
    run_url: &str,
    payload: &Value,
    suite: &str,
    scenario: &str,
) -> Result<Value, ScenarioError> {
    let timeout_secs = std::env::var("UCS_BROWSER_AUTOMATION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(90);
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation client initialization failed for '{suite}/{scenario}': {error}"
            ),
        })?;
    let response = client.post(run_url).json(payload).send().map_err(|error| {
        ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation request failed for '{suite}/{scenario}' via '{}': {error}",
                run_url
            ),
        }
    })?;

    let status = response.status();
    let response_text = response
        .text()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation response read failed for '{suite}/{scenario}': {error}"
            ),
        })?;

    if !status.is_success() {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation returned HTTP {} for '{suite}/{scenario}': {}",
                status, response_text
            ),
        });
    }

    let response_json: Value =
        serde_json::from_str(&response_text).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation returned non-JSON payload for '{suite}/{scenario}': {error}; body={response_text}"
            ),
        })?;

    let success = response_json
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !success {
        let error_message = response_json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("browser automation marked run as failed");
        return Err(ScenarioError::GrpcurlExecution {
            message: format!("browser automation failed for '{suite}/{scenario}': {error_message}"),
        });
    }

    Ok(response_json)
}

fn start_browser_automation_engine(host: &str, port: u16) -> Result<Child, ScenarioError> {
    let engine_dir = browser_automation_engine_dir()?;
    let mut child = Command::new("npm")
        .arg("run")
        .arg("dev")
        .current_dir(&engine_dir)
        .env("HOST", host)
        .env("PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "failed to start browser automation engine from '{}': {error}",
                engine_dir.display()
            ),
        })?;

    wait_for_browser_automation_engine(host, port, &mut child)?;
    Ok(child)
}

fn stop_browser_automation_engine(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_browser_automation_engine(
    host: &str,
    port: u16,
    child: &mut Child,
) -> Result<(), ScenarioError> {
    let endpoint = format!("{host}:{port}");
    for _ in 0..60 {
        if std::net::TcpStream::connect(&endpoint).is_ok() {
            return Ok(());
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|error| ScenarioError::GrpcurlExecution {
                message: format!(
                    "failed while waiting for browser automation engine startup: {error}"
                ),
            })?
        {
            return Err(ScenarioError::GrpcurlExecution {
                message: format!(
                    "browser automation engine exited before startup on {endpoint} with status: {status}"
                ),
            });
        }

        thread::sleep(Duration::from_millis(250));
    }

    Err(ScenarioError::GrpcurlExecution {
        message: format!("timed out waiting for browser automation engine startup on {endpoint}"),
    })
}

fn browser_automation_engine_dir() -> Result<PathBuf, ScenarioError> {
    let candidate = if let Ok(path) = std::env::var("UCS_BROWSER_AUTOMATION_DIR") {
        PathBuf::from(path)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("browser-automation-engine")
    };

    let resolved = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if !resolved.exists() {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "browser automation engine directory does not exist: '{}'",
                resolved.display()
            ),
        });
    }

    Ok(resolved)
}

fn connector_has_google_pay_metadata(connector: &str) -> Result<bool, ScenarioError> {
    let creds_path = creds_file_path();
    let content =
        std::fs::read_to_string(&creds_path).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "failed to read credentials file '{}': {error}",
                creds_path.display()
            ),
        })?;
    let json: Value =
        serde_json::from_str(&content).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "failed to parse credentials file '{}': {error}",
                creds_path.display()
            ),
        })?;

    let Some(connector_value) = json.get(connector) else {
        return Ok(false);
    };

    let base = match connector_value {
        Value::Array(entries) => match entries.first() {
            Some(entry) => entry,
            None => return Ok(false),
        },
        other => other,
    };

    Ok(base
        .get("metadata")
        .and_then(|metadata| metadata.get("google_pay"))
        .is_some())
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

/// Convention-based Google Pay token generation.
/// Automatically executes browser automation to generate a real Google Pay token
/// when the request contains the Google Pay encrypted token field.
fn execute_google_pay_token_generation(
    suite: &str,
    scenario: &str,
    connector: &str,
    effective_req: &mut Value,
) -> Result<(), ScenarioError> {
    // 1. Check required env var — skip if GPAY_HOSTED_URL is not set.
    if std::env::var("GPAY_HOSTED_URL").is_err() {
        return Err(ScenarioError::Skipped {
            reason: "GPAY_HOSTED_URL not set".to_string(),
        });
    }

    if !connector_has_google_pay_metadata(connector)? {
        let creds_path = creds_file_path()
            .canonicalize()
            .unwrap_or_else(|_| creds_file_path());
        let reason = format!(
            "credentials for connector '{connector}' do not include metadata.google_pay; \
             add a `metadata.google_pay` block under `{connector}` in '{}'. \
             Refer to `browser-automation-engine/src/gpay-token-gen.ts` for the expected shape \
             and use any existing connector entry in `creds.json` that already has \
             `metadata.google_pay` as a template",
            creds_path.display()
        );
        return Err(ScenarioError::Skipped { reason });
    }

    // 2. Resolve creds path and temp output file.
    let creds_path = creds_file_path()
        .canonicalize()
        .unwrap_or_else(|_| creds_file_path());
    let creds_path_str = creds_path.to_string_lossy().into_owned();

    let tmp_dir = std::env::temp_dir();
    let output_file = tmp_dir.join(format!("ucs_gpay_{}.json", Uuid::new_v4()));
    let output_file_str = output_file.to_string_lossy().into_owned();

    // 3. Build the npm command arguments.
    let args = vec![
        "run".to_string(),
        "gpay".to_string(),
        "--".to_string(),
        "--connector".to_string(),
        connector.to_string(),
        "--headless".to_string(),
        "--output".to_string(),
        output_file_str.clone(),
    ];

    // 4. Determine the working directory (browser-automation-engine).
    let work_dir = browser_automation_engine_dir()?;

    // 5. Spawn the npm process.
    let mut cmd = Command::new("npm");
    cmd.args(&args)
        .current_dir(&work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("CONNECTOR_AUTH_FILE_PATH", creds_path_str);

    let mut child = cmd
        .spawn()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
            "[google_pay_token_gen] {connector}/{suite}/{scenario}: failed to spawn npm: {error}"
        ),
        })?;

    // 6. Wait for the child process to exit.
    let status = child
        .wait()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
            "[google_pay_token_gen] {connector}/{suite}/{scenario}: failed waiting for npm: {error}"
        ),
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&output_file);
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "[google_pay_token_gen] {connector}/{suite}/{scenario}: npm exited with {status}"
            ),
        });
    }

    // 7. Read and parse the CLI output JSON.
    let output_content =
        std::fs::read_to_string(&output_file).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[google_pay_token_gen] {connector}/{suite}/{scenario}: \
                 failed to read output file '{}': {error}",
                output_file.display()
            ),
        })?;
    let _ = std::fs::remove_file(&output_file);

    let output_json: Value =
        serde_json::from_str(&output_content).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[google_pay_token_gen] {connector}/{suite}/{scenario}: \
                 failed to parse CLI output JSON: {error}"
            ),
        })?;

    // 8. Extract the token from the output JSON.
    let token_path = "paymentData.paymentMethodData.tokenizationData.token";
    let Some(token_value) = lookup_json_path_with_case_fallback(&output_json, token_path).cloned()
    else {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "[google_pay_token_gen] {connector}/{suite}/{scenario}: \
                 token path '{token_path}' not found in CLI output"
            ),
        });
    };

    // 9. Inject the token into the request.
    let target_path = "payment_method.google_pay.tokenization_data.encrypted_data.token";
    if !set_json_path_value(effective_req, target_path, token_value) {
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "[google_pay_token_gen] {connector}/{suite}/{scenario}: \
                 failed to set target path '{target_path}' in request"
            ),
        });
    }

    Ok(())
}

/// Executes the `cli_pre_request` hook: runs an arbitrary CLI tool, reads
/// its JSON output from a temp file, and injects fields into `effective_req`.
///
/// If any `required_env` variable is absent from the current environment the
/// hook is silently skipped (returns `Ok(())`) with a warning to stderr.
fn execute_cli_pre_request(
    suite: &str,
    scenario: &str,
    connector: &str,
    config: &CliPreRequestHookConfig,
    effective_req: &mut Value,
) -> Result<(), ScenarioError> {
    // 1. Check required env vars — skip with warning if any are missing.
    let missing_env: Vec<&str> = config
        .required_env
        .iter()
        .filter(|name| std::env::var(name).is_err())
        .map(String::as_str)
        .collect();
    if !missing_env.is_empty() {
        return Err(ScenarioError::Skipped {
            reason: format!("required env vars not set: {}", missing_env.join(", ")),
        });
    }

    // 2. Resolve creds path and temp output file.
    let creds_path = creds_file_path()
        .canonicalize()
        .unwrap_or_else(|_| creds_file_path());
    let creds_path_str = creds_path.to_string_lossy().into_owned();

    let tmp_dir = std::env::temp_dir();
    let output_file = tmp_dir.join(format!("ucs_gpay_{}.json", Uuid::new_v4()));
    let output_file_str = output_file.to_string_lossy().into_owned();

    // 3. Substitute placeholders in args and env values.
    let resolve = |s: &str| -> String {
        s.replace("{{connector}}", connector)
            .replace("{{creds_path}}", &creds_path_str)
            .replace("{{output_file}}", &output_file_str)
    };

    let resolved_args: Vec<String> = config.args.iter().map(|a| resolve(a)).collect();
    let resolved_env: Vec<(String, String)> = config
        .env
        .iter()
        .map(|(k, v)| (k.clone(), resolve(v)))
        .collect();

    // 4. Determine the working directory (browser-automation-engine).
    let work_dir = browser_automation_engine_dir()?;

    // 5. Spawn the CLI process.
    let mut cmd = Command::new(&config.command);
    cmd.args(&resolved_args)
        .current_dir(&work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (key, val) in &resolved_env {
        cmd.env(key, val);
    }

    let mut child = cmd
        .spawn()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[cli_pre_request] {connector}/{suite}/{scenario}: failed to spawn '{}': {error}",
                config.command
            ),
        })?;

    // 6. Wait for the child process to exit
    let status = child
        .wait()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[cli_pre_request] {connector}/{suite}/{scenario}: failed waiting for '{}': {error}",
                config.command
            ),
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&output_file);
        return Err(ScenarioError::GrpcurlExecution {
            message: format!(
                "[cli_pre_request] {connector}/{suite}/{scenario}: '{}' exited with {status}",
                config.command
            ),
        });
    }

    // 7. Read and parse the CLI output JSON.
    let output_content =
        std::fs::read_to_string(&output_file).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[cli_pre_request] {connector}/{suite}/{scenario}: \
                 failed to read output file '{}': {error}",
                output_file.display()
            ),
        })?;
    let _ = std::fs::remove_file(&output_file);

    let output_json: Value =
        serde_json::from_str(&output_content).map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!(
                "[cli_pre_request] {connector}/{suite}/{scenario}: \
                 failed to parse CLI output JSON: {error}"
            ),
        })?;

    // 8. Apply output_map: inject CLI output fields into effective_req.
    for (target_path, source_path) in &config.output_map {
        let Some(source_value) =
            lookup_json_path_with_case_fallback(&output_json, source_path).cloned()
        else {
            return Err(ScenarioError::GrpcurlExecution {
                message: format!(
                    "[cli_pre_request] {connector}/{suite}/{scenario}: \
                     source path '{source_path}' not found in CLI output"
                ),
            });
        };
        if !set_json_path_value(effective_req, target_path, source_value) {
            return Err(ScenarioError::GrpcurlExecution {
                message: format!(
                    "[cli_pre_request] {connector}/{suite}/{scenario}: \
                     failed to set target path '{target_path}' in request"
                ),
            });
        }
    }

    Ok(())
}

fn extract_redirect_target_from_dependencies(
    dependency_entries: &[ExecutedDependency],
    effective_req: &Value,
    config: &BrowserAutomationHook,
) -> Result<Option<BrowserRedirectTarget>, ScenarioError> {
    for dependency_entry in dependency_entries.iter().rev() {
        if let Some(expected_suite) = config.after_dependency_suite.as_deref() {
            if dependency_entry.suite != expected_suite {
                continue;
            }
        }
        if let Some(expected_scenario) = config.after_dependency_scenario.as_deref() {
            if dependency_entry.scenario != expected_scenario {
                continue;
            }
        }

        let dependency_response = &dependency_entry.res;
        let Some(endpoint) =
            lookup_json_path_with_case_fallback(dependency_response, &config.endpoint_path)
                .and_then(Value::as_str)
        else {
            continue;
        };

        let method = lookup_json_path_with_case_fallback(dependency_response, &config.method_path)
            .and_then(Value::as_str);

        let form_fields =
            lookup_json_path_with_case_fallback(dependency_response, &config.query_params_path)
                .and_then(Value::as_object)
                .cloned();
        let form_field_values = form_fields
            .as_ref()
            .map(|fields| {
                fields
                    .iter()
                    .map(|(key, value)| {
                        let normalized = value
                            .as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| value.to_string());
                        (key.clone(), normalized)
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();

        let mut redirect_uri = None;
        if let Some(fields) = form_fields.as_ref() {
            for (key, value) in fields {
                let query_value = value
                    .as_str()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| value.to_string());
                if key.eq_ignore_ascii_case("redirect_uri") {
                    redirect_uri = Some(query_value);
                    break;
                }
            }
        }

        if let Some(fallback_path) = config.redirect_uri_fallback_request_path.as_deref() {
            if redirect_uri.is_none() {
                redirect_uri = lookup_json_path_with_case_fallback(effective_req, fallback_path)
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
            }
        }

        if is_post_redirect_method(method) {
            if let Some(fields) = form_fields.as_ref() {
                let file_url = render_auto_submit_form_file(endpoint, fields)?;
                let cleanup_path = Url::parse(&file_url)
                    .ok()
                    .and_then(|url| url.to_file_path().ok());
                return Ok(Some(BrowserRedirectTarget {
                    url: file_url,
                    redirect_uri,
                    cleanup_path,
                    form_fields: form_field_values,
                }));
            }
        }

        let mut redirect_url =
            Url::parse(endpoint).map_err(|error| ScenarioError::GrpcurlExecution {
                message: format!("invalid redirect endpoint '{endpoint}': {error}"),
            })?;

        if let Some(fields) = form_fields.as_ref() {
            {
                let mut query_pairs = redirect_url.query_pairs_mut();
                for (key, value) in fields {
                    let query_value = value
                        .as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| value.to_string());
                    query_pairs.append_pair(key, &query_value);
                }
            }
        }

        return Ok(Some(BrowserRedirectTarget {
            url: redirect_url.to_string(),
            redirect_uri,
            cleanup_path: None,
            form_fields: form_field_values,
        }));
    }

    Ok(None)
}

fn materialize_browser_rules(rule_templates: &[Value], redirect_uri: Option<&str>) -> Vec<Value> {
    rule_templates
        .iter()
        .filter_map(|rule| interpolate_rule_template(rule, redirect_uri))
        .collect()
}

fn interpolate_rule_template(value: &Value, redirect_uri: Option<&str>) -> Option<Value> {
    match value {
        Value::String(text) => {
            if text.contains("{{redirect_uri}}") {
                redirect_uri.map(|uri| Value::String(text.replace("{{redirect_uri}}", uri)))
            } else {
                Some(Value::String(text.clone()))
            }
        }
        Value::Array(items) => {
            let mapped = items
                .iter()
                .filter_map(|item| interpolate_rule_template(item, redirect_uri))
                .collect::<Vec<_>>();
            Some(Value::Array(mapped))
        }
        Value::Object(map) => {
            let mut mapped = serde_json::Map::with_capacity(map.len());
            for (key, item) in map {
                if let Some(materialized) = interpolate_rule_template(item, redirect_uri) {
                    mapped.insert(key.clone(), materialized);
                }
            }
            Some(Value::Object(mapped))
        }
        _ => Some(value.clone()),
    }
}

fn is_unresolved_connector_feature_data(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            normalized.is_empty() || normalized.contains("auto_generate")
        }
        Value::Object(map) => map
            .get("value")
            .map(|inner| match inner {
                Value::Null => true,
                Value::String(text) => {
                    let normalized = text.trim().to_ascii_lowercase();
                    normalized.is_empty() || normalized.contains("auto_generate")
                }
                _ => false,
            })
            .unwrap_or(true),
        _ => false,
    }
}

/// Applies explicit context mappings from dependency results into the target request.
///
/// Each entry in `collected_context` is a `(context_map, dependency_req, dependency_res)` tuple
/// from one dependency suite. The `context_map` declares exactly which fields flow where:
///
/// ```json
/// { "state.access_token.token.value": "res.access_token" }
/// ```
///
/// - Left side (key) = target path in `current_grpc_req`
/// - Right side (value) = source reference prefixed with `res.` or `req.`
///   (if no prefix, `res.` is assumed)
///
/// Missing intermediate objects are created automatically (deep-set).
pub fn apply_context_map(
    collected_context: &[(ContextMap, Value, Value)],
    current_grpc_req: &mut Value,
) {
    for (context_map, dep_req, dep_res) in collected_context {
        for (target_path, source_ref) in context_map {
            let (source_json, source_path) = if let Some(path) = source_ref.strip_prefix("req.") {
                (dep_req, path)
            } else if let Some(path) = source_ref.strip_prefix("res.") {
                (dep_res, path)
            } else {
                // Default to response if no prefix
                (dep_res, source_ref.as_str())
            };

            // Try direct path, then with .id_type.id unwrapping for Identifier fields
            let source_value = lookup_json_path_with_case_fallback(source_json, source_path)
                .or_else(|| {
                    // If source path ends with .id, also try .id_type.id
                    if source_path.ends_with(".id") {
                        let alt = format!(
                            "{}.id_type.id",
                            source_path.strip_suffix(".id").unwrap_or(source_path)
                        );
                        lookup_json_path_with_case_fallback(source_json, &alt)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    // Try camelCase version of source path
                    let camel = source_path
                        .split('.')
                        .map(snake_to_camel_case)
                        .collect::<Vec<_>>()
                        .join(".");
                    lookup_json_path_with_case_fallback(source_json, &camel)
                });

            if let Some(value) = source_value {
                if !value.is_null() {
                    let _ = deep_set_json_path(current_grpc_req, target_path, value.clone());
                }
            }
        }
    }
}

/// Like `set_json_path_value` but creates intermediate objects if they don't exist.
fn deep_set_json_path(root: &mut Value, path: &str, value: Value) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (i, segment) in segments.iter().enumerate() {
        let is_last = i == segments.len() - 1;

        if is_last {
            if let Some(map) = current.as_object_mut() {
                map.insert(segment.to_string(), value);
                return true;
            }
            return false;
        }

        // Navigate or create intermediate object
        if current.is_object() {
            let Some(map) = current.as_object_mut() else {
                return false;
            };
            if !map.contains_key(*segment) {
                map.insert(segment.to_string(), Value::Object(serde_json::Map::new()));
            }
            let Some(next) = map.get_mut(*segment) else {
                return false;
            };
            current = next;
        } else {
            return false;
        }
    }

    false
}

fn lookup_first_non_null_path<'a>(value: &'a Value, paths: &[String]) -> Option<&'a Value> {
    for path in paths {
        if let Some(found) = lookup_json_path_with_case_fallback(value, path) {
            if !found.is_null() {
                return Some(found);
            }
        }
    }
    None
}

fn source_path_candidates(path: &str) -> Vec<String> {
    let mut candidates = vec![path.to_string()];

    if path.ends_with(".connector_customer_id") {
        candidates.push("connector_customer_id".to_string());
    }

    if path == "state.access_token.token.value" {
        candidates.push("access_token.value".to_string());
        candidates.push("access_token".to_string());
    }

    if path == "state.access_token.token_type" {
        candidates.push("token_type".to_string());
    }

    if path == "state.access_token.expires_in_seconds" {
        candidates.push("expires_in_seconds".to_string());
    }

    if let Some(rest) = path.strip_prefix("state.") {
        candidates.push(rest.to_string());
    }

    if path == "connector_feature_data.value" {
        candidates.push("connector_feature_data".to_string());
    }

    if path == "payment_method_token.value" {
        candidates.push("payment_method_token".to_string());
    }

    if path == "refund_id" {
        candidates.push("connector_refund_id".to_string());
    }

    if path == "connector_mandate_id" {
        candidates.push("mandate_reference.connector_mandate_id.connector_mandate_id".to_string());
        candidates
            .push("mandate_reference_id.connector_mandate_id.connector_mandate_id".to_string());
    }

    if let Some(rest) = path.strip_prefix("mandate_reference_id.") {
        candidates.push(format!("mandate_reference.{rest}"));
    }

    if let Some(prefix) = path.strip_suffix(".id") {
        candidates.push(format!("{prefix}.id_type.id"));
        candidates.push(format!("{prefix}.id_type.encoded_data"));
    }

    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn collect_leaf_paths(value: &Value, current: String, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if current.is_empty() {
                    key.to_string()
                } else {
                    format!("{current}.{key}")
                };
                collect_leaf_paths(child, next, paths);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let next = if current.is_empty() {
                    index.to_string()
                } else {
                    format!("{current}.{index}")
                };
                collect_leaf_paths(child, next, paths);
            }
        }
        _ => paths.push(current),
    }
}

fn lookup_json_path_with_case_fallback<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }

        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)?
        } else {
            current
                .get(segment)
                .or_else(|| current.get(snake_to_camel_case(segment)))
                .or_else(|| current.get(camel_to_snake_case(segment)))
                .or_else(|| current.get(to_pascal_case(segment)))?
        };
    }

    Some(current)
}

fn set_json_path_value(root: &mut Value, path: &str, value: Value) -> bool {
    let mut segments = path.split('.').peekable();
    let mut current = root;

    while let Some(segment) = segments.next() {
        let is_last = segments.peek().is_none();

        if let Ok(index) = segment.parse::<usize>() {
            let Some(items) = current.as_array_mut() else {
                return false;
            };
            let Some(next) = items.get_mut(index) else {
                return false;
            };
            if is_last {
                *next = value;
                return true;
            }
            current = next;
            continue;
        }

        let Some(map) = current.as_object_mut() else {
            return false;
        };

        if is_last {
            if map.contains_key(segment) {
                map.insert(segment.to_string(), value);
                return true;
            }
            let camel = snake_to_camel_case(segment);
            if map.contains_key(&camel) {
                map.insert(camel, value);
                return true;
            }
            let snake = camel_to_snake_case(segment);
            if map.contains_key(&snake) {
                map.insert(snake, value);
                return true;
            }
            return false;
        }

        let next_key = if map.contains_key(segment) {
            segment.to_string()
        } else {
            let camel = snake_to_camel_case(segment);
            if map.contains_key(&camel) {
                camel
            } else {
                let snake = camel_to_snake_case(segment);
                if map.contains_key(&snake) {
                    snake
                } else {
                    return false;
                }
            }
        };

        let Some(next) = map.get_mut(&next_key) else {
            return false;
        };
        current = next;
    }

    false
}

fn remove_json_path(root: &mut Value, path: &str) -> bool {
    let mut segments = path.split('.').peekable();
    let mut current = root;

    while let Some(segment) = segments.next() {
        let is_last = segments.peek().is_none();

        if is_last {
            if let Ok(index) = segment.parse::<usize>() {
                if let Some(items) = current.as_array_mut() {
                    if let Some(target) = items.get_mut(index) {
                        *target = Value::Null;
                        return true;
                    }
                }
                return false;
            }

            if let Some(map) = current.as_object_mut() {
                return map.remove(segment).is_some();
            }
            return false;
        }

        if let Ok(index) = segment.parse::<usize>() {
            let Some(items) = current.as_array_mut() else {
                return false;
            };
            let Some(next) = items.get_mut(index) else {
                return false;
            };
            current = next;
            continue;
        }

        let Some(map) = current.as_object_mut() else {
            return false;
        };
        let Some(next) = map.get_mut(segment) else {
            return false;
        };
        current = next;
    }

    false
}

/// Recursively checks whether all leaf values in a JSON subtree are "default"
/// values (`""`, `0`, `0.0`, `null`, `false`, empty arrays, or objects/arrays
/// where all children are themselves defaults).
///
/// This is used to prune wrapper objects that remain after context propagation
/// when none of their fields were actually filled with meaningful data.
fn has_only_default_leaves(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::String(s) => s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f == 0.0).unwrap_or(false),
        Value::Array(items) => items.is_empty() || items.iter().all(has_only_default_leaves),
        Value::Object(map) => map.is_empty() || map.values().all(has_only_default_leaves),
        Value::Bool(true) => false,
    }
}

/// Removes a JSON path if the value at that path has only default leaves
/// (empty strings, zeros, nulls, false, or nested objects/arrays of the same).
fn remove_json_path_if_all_defaults(root: &mut Value, path: &str) -> bool {
    let should_remove = lookup_json_path_with_case_fallback(root, path)
        .map(has_only_default_leaves)
        .unwrap_or(false);
    if should_remove {
        return remove_json_path(root, path);
    }
    false
}

fn snake_to_camel_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut uppercase_next = false;
    for ch in input.chars() {
        if ch == '_' {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn camel_to_snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for (idx, ch) in input.chars().enumerate() {
        if ch.is_ascii_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// Builds a grpcurl request by loading payload from suite/scenario templates.
pub fn build_grpcurl_request(
    suite: Option<&str>,
    scenario: Option<&str>,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
    require_auth: bool,
) -> Result<GrpcurlRequest, ScenarioError> {
    let suite = suite.unwrap_or(DEFAULT_SUITE);
    let scenario = scenario.unwrap_or(DEFAULT_SCENARIO);
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let mut grpc_req = get_the_grpc_req_for_connector(suite, scenario, connector)?;
    resolve_auto_generate(&mut grpc_req, connector)?;
    build_grpcurl_request_from_payload(
        suite,
        scenario,
        &grpc_req,
        endpoint,
        Some(connector),
        merchant_id,
        tenant_id,
        plaintext,
        require_auth,
    )
}

/// Builds a grpcurl request from an already materialized JSON payload.
pub fn build_grpcurl_request_from_payload(
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
    require_auth: bool,
) -> Result<GrpcurlRequest, ScenarioError> {
    let endpoint = endpoint.unwrap_or(DEFAULT_ENDPOINT);
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let merchant_id = merchant_id.unwrap_or(DEFAULT_MERCHANT_ID);
    let tenant_id = tenant_id.unwrap_or(DEFAULT_TENANT_ID);

    let payload = serde_json::to_string_pretty(grpc_req)
        .map_err(|source| ScenarioError::JsonSerialize { source })?;

    let config = load_connector_config(connector).map_or_else(
        |error| {
            if require_auth {
                Err(ScenarioError::CredentialLoad {
                    connector: connector.to_string(),
                    message: error.to_string(),
                })
            } else {
                Ok(None)
            }
        },
        |config| Ok(Some(config)),
    )?;

    let request_id = format!("{suite}_{scenario}_req");
    let connector_request_reference_id =
        connector_request_reference_id_for(connector, suite, scenario, grpc_req);
    let suite_spec = load_suite_spec(suite).ok();
    let method = grpc_method_for_suite(suite, suite_spec.as_ref())?;

    let mut headers = vec![
        format!("x-merchant-id: {merchant_id}"),
        format!("x-tenant-id: {tenant_id}"),
        format!("x-request-id: {request_id}"),
        format!("x-connector-request-reference-id: {connector_request_reference_id}"),
    ];

    if let Some(config) = config.as_ref() {
        headers.push(format!("x-connector-config: {}", config.header_value()));
    } else {
        headers.push("x-connector-config: <paste JSON here>".to_string());
    }

    Ok(GrpcurlRequest {
        endpoint: endpoint.to_string(),
        method: method.to_string(),
        payload,
        headers,
        plaintext,
    })
}

/// Convenience helper that returns only the shell command string.
pub fn build_grpcurl_command(
    suite: Option<&str>,
    scenario: Option<&str>,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<String, ScenarioError> {
    let request = build_grpcurl_request(
        suite,
        scenario,
        endpoint,
        connector,
        merchant_id,
        tenant_id,
        plaintext,
        false,
    )?;
    Ok(request.to_command_string())
}

/// Executes grpcurl by resolving payload from suite/scenario templates.
pub fn execute_grpcurl_request(
    suite: Option<&str>,
    scenario: Option<&str>,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<String, ScenarioError> {
    let result = execute_grpcurl_request_with_trace(
        suite,
        scenario,
        endpoint,
        connector,
        merchant_id,
        tenant_id,
        plaintext,
    )?;
    if !result.success {
        return Err(ScenarioError::GrpcurlExecution {
            message: result.response_output,
        });
    }
    Ok(result.response_body)
}

/// Executes grpcurl by resolving payload from suite/scenario templates and
/// returns full request/response trace.
pub fn execute_grpcurl_request_with_trace(
    suite: Option<&str>,
    scenario: Option<&str>,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<GrpcExecutionResult, ScenarioError> {
    let suite = suite.unwrap_or(DEFAULT_SUITE);
    let scenario = scenario.unwrap_or(DEFAULT_SCENARIO);
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let mut grpc_req = get_the_grpc_req_for_connector(suite, scenario, connector)?;
    resolve_auto_generate(&mut grpc_req, connector)?;
    execute_grpcurl_request_from_payload_with_trace(
        suite,
        scenario,
        &grpc_req,
        endpoint,
        Some(connector),
        merchant_id,
        tenant_id,
        plaintext,
    )
}

/// Executes grpcurl with a prepared payload.
pub fn execute_grpcurl_request_from_payload(
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<String, ScenarioError> {
    let result = execute_grpcurl_request_from_payload_with_trace(
        suite,
        scenario,
        grpc_req,
        endpoint,
        connector,
        merchant_id,
        tenant_id,
        plaintext,
    )?;
    if !result.success {
        return Err(ScenarioError::GrpcurlExecution {
            message: result.response_output,
        });
    }
    Ok(result.response_body)
}

/// Executes grpcurl with a prepared payload and returns full request/response
/// trace (including verbose header/trailer output).
pub fn execute_grpcurl_request_from_payload_with_trace(
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<GrpcExecutionResult, ScenarioError> {
    let request = build_grpcurl_request_from_payload(
        suite,
        scenario,
        grpc_req,
        endpoint,
        connector,
        merchant_id,
        tenant_id,
        plaintext,
        true,
    )?;
    execute_grpcurl_from_request(request)
}

fn execute_grpcurl_from_request(
    request: GrpcurlRequest,
) -> Result<GrpcExecutionResult, ScenarioError> {
    let request_command = request.to_command_string();
    let mut args = Vec::new();
    args.push("-v".to_string());
    if request.plaintext {
        args.push("-plaintext".to_string());
    }

    // Add timeout to prevent hanging indefinitely
    let timeout_secs = std::env::var("UCS_GRPC_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30); // Default 30 seconds
    args.push("-max-time".to_string());
    args.push(timeout_secs.to_string());

    for header in &request.headers {
        args.push("-H".to_string());
        args.push(header.clone());
    }

    args.push("-d".to_string());
    args.push(request.payload.clone());
    args.push(request.endpoint.clone());
    args.push(request.method.clone());

    let output = Command::new("grpcurl")
        .args(&args)
        .output()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!("failed to spawn grpcurl: {error}"),
        })?;

    let stdout_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_output = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let response_output = build_grpc_response_output(&stdout_output, &stderr_output);

    let response_body = extract_json_body_from_grpc_output(&stdout_output, &stderr_output)
        .unwrap_or_else(|| stdout_output.clone());

    Ok(GrpcExecutionResult {
        response_body,
        request_command,
        response_output,
        success: output.status.success(),
    })
}

fn build_grpc_response_output(stdout_output: &str, stderr_output: &str) -> String {
    if !stdout_output.is_empty() && !stderr_output.is_empty() {
        format!("{stdout_output}\n\n{stderr_output}")
    } else if !stdout_output.is_empty() {
        stdout_output.to_string()
    } else {
        stderr_output.to_string()
    }
}

fn extract_json_body_from_grpc_output(stdout_output: &str, stderr_output: &str) -> Option<String> {
    for source in [stdout_output, stderr_output] {
        let trimmed = source.trim();
        if trimmed.is_empty() {
            continue;
        }

        if serde_json::from_str::<Value>(trimmed).is_ok() {
            return Some(trimmed.to_string());
        }

        if let Some(marker_offset) = trimmed.find("Response contents:") {
            let marker_tail = &trimmed[(marker_offset + "Response contents:".len())..];
            if let Some(extracted) = extract_first_json_value(marker_tail) {
                return Some(extracted);
            }
        }

        if let Some(extracted) = extract_best_json_value(trimmed) {
            return Some(extracted);
        }
    }

    None
}

fn extract_first_json_value(text: &str) -> Option<String> {
    for (start_index, ch) in text.char_indices() {
        if ch != '{' && ch != '[' {
            continue;
        }

        let Some(end_index) = find_json_value_end(text, start_index) else {
            continue;
        };

        let candidate = text[start_index..end_index].trim();
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }

    None
}

fn extract_best_json_value(text: &str) -> Option<String> {
    let mut best: Option<&str> = None;

    for (start_index, ch) in text.char_indices() {
        if ch != '{' && ch != '[' {
            continue;
        }

        let Some(end_index) = find_json_value_end(text, start_index) else {
            continue;
        };

        let candidate = text[start_index..end_index].trim();
        if serde_json::from_str::<Value>(candidate).is_ok() {
            let replace = best.is_none_or(|existing| candidate.len() > existing.len());
            if replace {
                best = Some(candidate);
            }
        }
    }

    best.map(ToString::to_string)
}

fn find_json_value_end(text: &str, start_index: usize) -> Option<usize> {
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in text[start_index..].char_indices() {
        let index = start_index + offset;

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' | '[' => stack.push(ch),
            '}' => {
                if stack.pop() != Some('{') {
                    return None;
                }
                if stack.is_empty() {
                    return Some(index + ch.len_utf8());
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return None;
                }
                if stack.is_empty() {
                    return Some(index + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}

/// Executes one request through tonic backend using template payload.
pub fn execute_tonic_request(
    suite: Option<&str>,
    scenario: Option<&str>,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<String, ScenarioError> {
    let suite = suite.unwrap_or(DEFAULT_SUITE);
    let scenario = scenario.unwrap_or(DEFAULT_SCENARIO);
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let mut grpc_req = get_the_grpc_req_for_connector(suite, scenario, connector)?;
    resolve_auto_generate(&mut grpc_req, connector)?;
    execute_tonic_request_from_payload(
        suite,
        scenario,
        &grpc_req,
        endpoint,
        Some(connector),
        merchant_id,
        tenant_id,
        plaintext,
    )
}

/// Executes one request through tonic backend using prepared payload.
pub fn execute_tonic_request_from_payload(
    suite: &str,
    scenario: &str,
    grpc_req: &Value,
    endpoint: Option<&str>,
    connector: Option<&str>,
    merchant_id: Option<&str>,
    tenant_id: Option<&str>,
    plaintext: bool,
) -> Result<String, ScenarioError> {
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR).to_string();
    let merchant_id = merchant_id.unwrap_or(DEFAULT_MERCHANT_ID).to_string();
    let tenant_id = tenant_id.unwrap_or(DEFAULT_TENANT_ID).to_string();
    let endpoint = endpoint.unwrap_or(DEFAULT_ENDPOINT).to_string();
    let config =
        load_connector_config(&connector).map_err(|error| ScenarioError::CredentialLoad {
            connector: connector.clone(),
            message: error.to_string(),
        })?;

    let request_id = format!("{suite}_{scenario}_req");
    let connector_request_reference_id =
        connector_request_reference_id_for(&connector, suite, scenario, grpc_req);
    let grpc_req = grpc_req.clone();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!("failed to initialize tonic runtime: {error}"),
        })?;

    runtime.block_on(async move {
        let channel = Channel::from_shared(to_tonic_endpoint(&endpoint, plaintext))
            .map_err(|error| ScenarioError::GrpcurlExecution {
                message: format!("failed to prepare tonic endpoint '{endpoint}': {error}"),
            })?
            .connect()
            .await
            .map_err(|error| ScenarioError::GrpcurlExecution {
                message: format!("failed to connect to endpoint '{endpoint}': {error}"),
            })?;

        // Resolve alias_for from suite_spec so data-defined suite aliases can
        // reuse standard dispatch paths without extra harness code.
        let suite_spec_for_dispatch = load_suite_spec(suite).ok();
        let effective_suite = suite_spec_for_dispatch
            .as_ref()
            .and_then(|s| s.alias_for.as_deref())
            .unwrap_or(suite);

        match effective_suite {
            "MerchantAuthenticationService/CreateServerAuthenticationToken" => {
                let payload: grpc_api_types::payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::merchant_authentication_service_client::MerchantAuthenticationServiceClient::new(channel.clone());
                let response = client.create_server_authentication_token(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "CustomerService/Create" => {
                let payload: grpc_api_types::payments::CustomerServiceCreateRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::customer_service_client::CustomerServiceClient::new(channel.clone());
                let response = client.create(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentMethodAuthenticationService/PreAuthenticate" => {
                let payload: grpc_api_types::payments::PaymentMethodAuthenticationServicePreAuthenticateRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient::new(channel.clone());
                let response = client.pre_authenticate(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentMethodAuthenticationService/Authenticate" => {
                let payload: grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient::new(channel.clone());
                let response = client.authenticate(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentMethodAuthenticationService/PostAuthenticate" => {
                let payload: grpc_api_types::payments::PaymentMethodAuthenticationServicePostAuthenticateRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient::new(channel.clone());
                let response = client.post_authenticate(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Authorize" | "PaymentService/CompleteAuthorize" => {
                let payload: grpc_api_types::payments::PaymentServiceAuthorizeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.authorize(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Capture" => {
                let payload: grpc_api_types::payments::PaymentServiceCaptureRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.capture(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Refund" => {
                let payload: grpc_api_types::payments::PaymentServiceRefundRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.refund(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Void" => {
                let payload: grpc_api_types::payments::PaymentServiceVoidRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.void(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Get" => {
                let payload: grpc_api_types::payments::PaymentServiceGetRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.get(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "RefundService/Get" => {
                let payload: grpc_api_types::payments::RefundServiceGetRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::refund_service_client::RefundServiceClient::new(channel.clone());
                let response = client.get(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/SetupRecurring" => {
                let payload: grpc_api_types::payments::PaymentServiceSetupRecurringRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.setup_recurring(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "RecurringPaymentService/Charge" => {
                let payload: grpc_api_types::payments::RecurringPaymentServiceChargeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::recurring_payment_service_client::RecurringPaymentServiceClient::new(channel.clone());
                let response = client.charge(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "RecurringPaymentService/Revoke" => {
                let payload: grpc_api_types::payments::RecurringPaymentServiceRevokeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::recurring_payment_service_client::RecurringPaymentServiceClient::new(channel.clone());
                let response = client.revoke(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentMethodService/Tokenize" => {
                let payload: grpc_api_types::payments::PaymentMethodServiceTokenizeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_method_service_client::PaymentMethodServiceClient::new(channel.clone());
                let response = client.tokenize(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/IncrementalAuthorization" => {
                let payload: grpc_api_types::payments::PaymentServiceIncrementalAuthorizationRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.incremental_authorization(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "MerchantAuthenticationService/CreateServerSessionAuthenticationToken" => {
                let payload: grpc_api_types::payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::merchant_authentication_service_client::MerchantAuthenticationServiceClient::new(channel.clone());
                let response = client.create_server_session_authentication_token(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "MerchantAuthenticationService/CreateClientAuthenticationToken" => {
                let payload: grpc_api_types::payments::MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::merchant_authentication_service_client::MerchantAuthenticationServiceClient::new(channel.clone());
                let response = client.create_client_authentication_token(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/CreateOrder" => {
                let payload: grpc_api_types::payments::PaymentServiceCreateOrderRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.create_order(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/Reverse" => {
                let payload: grpc_api_types::payments::PaymentServiceReverseRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.reverse(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/VerifyRedirectResponse" => {
                let payload: grpc_api_types::payments::PaymentServiceVerifyRedirectResponseRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.verify_redirect_response(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/TokenAuthorize" => {
                let payload: grpc_api_types::payments::PaymentServiceTokenAuthorizeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.token_authorize(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/TokenSetupRecurring" => {
                let payload: grpc_api_types::payments::PaymentServiceTokenSetupRecurringRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.token_setup_recurring(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/ProxyAuthorize" => {
                let payload: grpc_api_types::payments::PaymentServiceProxyAuthorizeRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.proxy_authorize(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentService/ProxySetupRecurring" => {
                let payload: grpc_api_types::payments::PaymentServiceProxySetupRecurringRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_service_client::PaymentServiceClient::new(channel.clone());
                let response = client.proxy_setup_recurring(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            "PaymentMethodService/Eligibility" => {
                let payload: grpc_api_types::payments::PayoutMethodEligibilityRequest =
                    parse_tonic_payload(suite, scenario, &connector, &grpc_req)?;
                let mut request = tonic::Request::new(payload);
                add_connector_metadata(
                    &mut request,
                    &config,
                    &merchant_id,
                    &tenant_id,
                    &request_id,
                    &connector_request_reference_id,
                );
                let mut client = grpc_api_types::payments::payment_method_service_client::PaymentMethodServiceClient::new(channel.clone());
                let response = client.eligibility(request).await.map_err(|error| {
                    ScenarioError::GrpcurlExecution {
                        message: format!(
                            "tonic execution failed for '{suite}/{scenario}': {error}"
                        ),
                    }
                })?;
                serialize_tonic_response(&response.into_inner())
            }
            _ => Err(ScenarioError::UnsupportedSuite {
                suite: effective_suite.to_string(),
            }),
        }
    })
}

pub(crate) fn parse_tonic_payload<T: DeserializeOwned>(
    suite: &str,
    scenario: &str,
    connector: &str,
    grpc_req: &Value,
) -> Result<T, ScenarioError> {
    let normalized = normalize_tonic_request_json(connector, suite, scenario, grpc_req.clone());
    serde_json::from_value(normalized).map_err(|error| ScenarioError::GrpcurlExecution {
        message: format!(
            "failed to parse request JSON into tonic payload for '{suite}/{scenario}': {error}"
        ),
    })
}

pub fn normalize_tonic_request_json(
    connector: &str,
    suite: &str,
    scenario: &str,
    mut value: Value,
) -> Value {
    normalize_value_wrappers(&mut value);
    normalize_proto_oneof_shapes(&mut value);

    normalize_request_common(connector, suite, scenario, &mut value);
    value
}

/// Normalises a scenario JSON payload for the grpcurl execution path.
///
/// Unlike [`normalize_tonic_request_json`], this does **not** strip `{"value": "..."}`
/// wrappers (`SecretString`, `CardNumberType`, etc.) because grpcurl expects the
/// full proto-native JSON shape.  It also does **not** wrap oneofs into the prost
/// `{"payment_method": {"payment_method": {…}}}` double-nesting shape – grpcurl
/// expects the proto field name directly.
///
/// What it does:
/// - Suite-specific field renames (legacy → proto names)
/// - Suite-specific field hoisting (e.g. flat fields → `domain_context`)
/// - `convert_prost_oneofs_to_grpcurl` for `domain_context` (PascalCase → snake)
/// - Connector-specific transforms
pub fn normalize_grpcurl_request_json(
    connector: &str,
    suite: &str,
    scenario: &str,
    mut value: Value,
) -> Value {
    normalize_request_common(connector, suite, scenario, &mut value);

    // domain_context hoisting above uses the prost format (`"Payment"` PascalCase).
    // grpcurl expects the raw proto field name (`"payment"` lowercase).
    convert_prost_oneofs_to_grpcurl(&mut value);

    value
}

/// Shared normalisation logic used by both tonic and grpcurl paths.
fn normalize_request_common(connector: &str, suite: &str, scenario: &str, value: &mut Value) {
    // Resolve alias_for so data-defined suite aliases apply the same
    // normalization rules as their canonical suite.
    let suite_spec_opt = load_suite_spec(suite).ok();
    let effective_suite = suite_spec_opt
        .as_ref()
        .and_then(|s| s.alias_for.as_deref())
        .unwrap_or(suite);

    // Legacy scenario payloads used in grpcurl contain fields that do not map
    // directly to current proto request shapes used by tonic serde.
    // Drop or adjust known mismatches here so scenarios remain unchanged.
    if let Value::Object(map) = value {
        if matches!(
            effective_suite,
            "PaymentService/Authorize" | "PaymentService/CompleteAuthorize"
        ) {
            map.entry("order_details".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
        }

        if matches!(
            effective_suite,
            "PaymentService/Authorize"
                | "PaymentService/CompleteAuthorize"
                | "PaymentService/SetupRecurring"
                | "PaymentService/ProxySetupRecurring"
                | "PaymentService/TokenSetupRecurring"
        ) {
            if let Some(Value::Object(customer_acceptance)) = map.get_mut("customer_acceptance") {
                if !customer_acceptance.contains_key("accepted_at") {
                    let accepted_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                        .unwrap_or(0);
                    customer_acceptance.insert("accepted_at".to_string(), Value::from(accepted_at));
                }
            }
        }

        if effective_suite == "PaymentService/SetupRecurring" {
            map.entry("request_incremental_authorization".to_string())
                .or_insert_with(|| Value::Bool(false));
        }

        if effective_suite == "PaymentService/Get" {
            if let Some(handle_response) = map.get("handle_response") {
                if handle_response.is_boolean() {
                    map.remove("handle_response");
                }
            }
        }

        // client_authentication_token: flat scenario fields → nested proto shape.
        // Proto: merchant_client_session_id (string),
        //        oneof domain_context { payment (PaymentClientSessionContext) }
        //        PaymentClientSessionContext { amount, order_tax_amount,
        //        shipping_cost, payment_method_type, country_alpha2_code, customer, metadata }
        //
        // NOTE: prost/serde expects `{"domainContext":{"Payment":{...}}}` (camelCase
        //       wrapper + PascalCase variant) while grpcurl expects `{"payment":{...}}`
        //       (raw field name).  We normalise to the prost shape here; the grpcurl
        //       execution path converts to the grpcurl shape separately.
        if effective_suite == "MerchantAuthenticationService/CreateClientAuthenticationToken" {
            // Rename legacy field name → proto field name
            if let Some(val) = map.remove("merchant_sdk_session_id") {
                map.entry("merchant_client_session_id".to_string())
                    .or_insert(val);
            }

            // Hoist payment-domain fields into `domain_context: { "Payment": { ... } }`
            let payment_keys = [
                "amount",
                "order_tax_amount",
                "shipping_cost",
                "payment_method_type",
                "country_alpha2_code",
                "customer",
                "metadata",
            ];
            let mut payment_ctx = serde_json::Map::new();
            for key in &payment_keys {
                if let Some(val) = map.remove(*key) {
                    payment_ctx.insert((*key).to_string(), val);
                }
            }
            if !payment_ctx.is_empty() {
                let mut domain_ctx = serde_json::Map::new();
                domain_ctx.insert("Payment".to_string(), Value::Object(payment_ctx));
                map.entry("domain_context".to_string())
                    .or_insert(Value::Object(domain_ctx));
            }
        }

        // server_session_authentication_token: flat scenario fields → nested proto shape.
        // Proto: merchant_server_session_id (optional string), connector_feature_data,
        //        state, test_mode, oneof domain_context { payment (PaymentSessionContext) }
        //        PaymentSessionContext { amount, metadata, browser_info }
        if effective_suite == "MerchantAuthenticationService/CreateServerSessionAuthenticationToken"
        {
            // Rename legacy field name → proto field name
            if let Some(val) = map.remove("merchant_session_id") {
                map.entry("merchant_server_session_id".to_string())
                    .or_insert(val);
            }

            // Hoist payment-domain fields into `domain_context: { "Payment": { ... } }`
            let payment_keys = ["amount", "metadata", "browser_info"];
            let mut payment_ctx = serde_json::Map::new();
            for key in &payment_keys {
                if let Some(val) = map.remove(*key) {
                    payment_ctx.insert((*key).to_string(), val);
                }
            }
            if !payment_ctx.is_empty() {
                let mut domain_ctx = serde_json::Map::new();
                domain_ctx.insert("Payment".to_string(), Value::Object(payment_ctx));
                map.entry("domain_context".to_string())
                    .or_insert(Value::Object(domain_ctx));
            }
        }
    }

    normalize_tonic_request_for_connector(connector, suite, scenario, value);
}

/// Converts prost/serde-style oneof wrappers to grpcurl-compatible field names.
///
/// Prost serialises oneofs as `{"domain_context": {"Payment": {...}}}` (camelCase
/// wrapper key + PascalCase variant).  Grpcurl expects the proto field name
/// directly: `{"payment": {...}}`.  This function performs that translation for
/// known oneof patterns.
pub fn convert_prost_oneofs_to_grpcurl(value: &mut Value) {
    if let Value::Object(map) = value {
        // Recurse into all children first so nested oneofs are handled.
        for child in map.values_mut() {
            convert_prost_oneofs_to_grpcurl(child);
        }

        // domain_context → extract the single variant and use lowercase field name.
        // Prost: {"domain_context": {"Payment": {...}}}
        // grpcurl: {"payment": {...}}
        if let Some(Value::Object(domain_ctx)) = map.remove("domain_context") {
            for (variant_name, variant_value) in domain_ctx {
                let field_name = pascal_to_snake_case(&variant_name);
                map.entry(field_name).or_insert(variant_value);
            }
        }

        // payment_method oneof: unwrap the inner prost wrapper.
        // Prost: {"payment_method": {"payment_method": {"card": {...}}}}
        // grpcurl: {"payment_method": {"card": {...}}}
        if let Some(Value::Object(pm_outer)) = map.get_mut("payment_method") {
            if let Some(Value::Object(inner_map)) = pm_outer.remove("payment_method") {
                for (k, v) in inner_map {
                    pm_outer.entry(k).or_insert(v);
                }
            }
        }

        // mandate_id_type oneof inside connector_recurring_payment_id:
        // Prost: {"connector_recurring_payment_id": {"mandate_id_type": {"ConnectorMandateId": {...}}}}
        // grpcurl: {"connector_recurring_payment_id": {"connector_mandate_id": {...}}}
        if let Some(Value::Object(mandate_ref)) = map.get_mut("connector_recurring_payment_id") {
            if let Some(Value::Object(mandate_type)) = mandate_ref.remove("mandate_id_type") {
                for (variant_name, variant_value) in mandate_type {
                    let field_name = pascal_to_snake_case(&variant_name);
                    mandate_ref.entry(field_name).or_insert(variant_value);
                }
            }
        }

        // tokenization_data oneof: unwrap the inner prost wrapper.
        // Prost: {"tokenization_data": {"tokenization_data": {"encrypted_data": {...}}}}
        // grpcurl: {"tokenization_data": {"encrypted_data": {...}}}
        if let Some(Value::Object(td_outer)) = map.get_mut("tokenization_data") {
            if let Some(Value::Object(inner_map)) = td_outer.remove("tokenization_data") {
                for (k, v) in inner_map {
                    td_outer.entry(k).or_insert(v);
                }
            }
        }
    } else if let Value::Array(items) = value {
        for item in items {
            convert_prost_oneofs_to_grpcurl(item);
        }
    }
}

/// Converts a PascalCase or camelCase string to snake_case.
fn pascal_to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn normalize_proto_oneof_shapes(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                normalize_proto_oneof_shapes(child);
            }

            normalize_payment_method_oneof(map);
            normalize_mandate_reference_oneofs(map);
            normalize_google_pay_tokenization_data_oneof(map);
        }
        Value::Array(items) => {
            for item in items {
                normalize_proto_oneof_shapes(item);
            }
        }
        _ => {}
    }
}

fn normalize_payment_method_oneof(map: &mut serde_json::Map<String, Value>) {
    let Some(Value::Object(payment_method_obj)) = map.get_mut("payment_method") else {
        return;
    };

    if payment_method_obj.contains_key("payment_method") {
        return;
    }

    if payment_method_obj.len() != 1 {
        return;
    }

    let original = std::mem::take(payment_method_obj);
    let Some((variant, payload)) = original.into_iter().next() else {
        return;
    };

    let mut oneof_obj = serde_json::Map::new();
    oneof_obj.insert(normalize_oneof_variant_key(&variant), payload);

    payment_method_obj.insert("payment_method".to_string(), Value::Object(oneof_obj));
}

fn normalize_mandate_reference_oneofs(map: &mut serde_json::Map<String, Value>) {
    let Some(Value::Object(mandate_reference_obj)) = map.get_mut("connector_recurring_payment_id")
    else {
        return;
    };

    if mandate_reference_obj.contains_key("mandate_id_type") {
        return;
    }

    if mandate_reference_obj.len() != 1 {
        return;
    }

    let original = std::mem::take(mandate_reference_obj);
    let Some((variant, payload)) = original.into_iter().next() else {
        return;
    };

    let mut wrapped_variant = serde_json::Map::new();
    wrapped_variant.insert(to_pascal_case(&variant), payload);

    mandate_reference_obj.insert(
        "mandate_id_type".to_string(),
        Value::Object(wrapped_variant),
    );
}

/// Normalizes the `google_pay.tokenization_data` oneof so that the flat
/// scenario JSON shape (`{"encrypted_data": {...}}`) is rewritten into the
/// nested shape the proto struct expects:
/// `{"tokenization_data": {"EncryptedData": {...}}}`.
///
/// The `TokenizationData` proto message has a field called `tokenization_data`
/// that holds the oneof enum. Scenario JSON omits that extra level and uses the
/// variant name in snake_case directly, so we need to re-wrap it here.
fn normalize_google_pay_tokenization_data_oneof(map: &mut serde_json::Map<String, Value>) {
    let Some(Value::Object(tokenization_data_obj)) = map.get_mut("tokenization_data") else {
        return;
    };

    // Already normalized (has the inner "tokenization_data" oneof wrapper).
    if tokenization_data_obj.contains_key("tokenization_data") {
        return;
    }

    // Must have exactly one key — the variant name in snake_case.
    if tokenization_data_obj.len() != 1 {
        return;
    }

    let original = std::mem::take(tokenization_data_obj);
    let Some((variant_snake, payload)) = original.into_iter().next() else {
        return;
    };

    // Only rewrite known GoogleWallet tokenization_data variants.
    if variant_snake != "encrypted_data" && variant_snake != "decrypted_data" {
        // Restore the map as-is if it's not a recognized variant.
        tokenization_data_obj.insert(variant_snake, payload);
        return;
    }

    let mut oneof_map = serde_json::Map::new();
    oneof_map.insert(variant_snake, payload);

    tokenization_data_obj.insert("tokenization_data".to_string(), Value::Object(oneof_map));
}

fn to_pascal_case(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut capitalize_next = true;

    for ch in value.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
            continue;
        }

        if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
        }
    }

    out
}

fn normalize_oneof_variant_key(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return value.to_string();
    }

    let mut out = String::with_capacity(value.len() + 8);
    let mut previous_was_lower_or_digit = false;

    for ch in value.chars() {
        if ch == '-' || ch == ' ' {
            if !out.ends_with('_') {
                out.push('_');
            }
            previous_was_lower_or_digit = false;
            continue;
        }

        if ch.is_ascii_uppercase() {
            if previous_was_lower_or_digit && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            previous_was_lower_or_digit = false;
            continue;
        }

        out.push(ch);
        previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    while out.ends_with('_') {
        out.pop();
    }

    if out.is_empty() {
        value.to_string()
    } else {
        out
    }
}

fn normalize_value_wrappers(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                normalize_value_wrappers(child);
            }

            if map.len() == 1 {
                if let Some(inner) = map.get("value") {
                    *value = inner.clone();
                    normalize_value_wrappers(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_value_wrappers(item);
            }
        }
        _ => {}
    }
}

fn serialize_tonic_response<T: Serialize>(response: &T) -> Result<String, ScenarioError> {
    serde_json::to_string_pretty(response).map_err(|source| ScenarioError::JsonSerialize { source })
}

fn to_tonic_endpoint(endpoint: &str, plaintext: bool) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.to_string();
    }

    if plaintext {
        format!("http://{endpoint}")
    } else {
        format!("https://{endpoint}")
    }
}

#[derive(Debug, Clone)]
pub struct SuiteScenarioResult {
    pub suite: String,
    pub scenario: String,
    pub is_dependency: bool,
    pub passed: bool,
    pub skipped: bool,
    pub error: Option<String>,
    pub dependency: Vec<String>,
    pub req_body: Option<Value>,
    pub res_body: Option<Value>,
    pub grpc_request: Option<String>,
    pub grpc_response: Option<String>,
}

#[derive(Debug, Clone)]
struct ExecutedScenario {
    effective_req: Value,
    response_json: Value,
    assertions: BTreeMap<String, FieldAssert>,
    grpc_request: Option<String>,
    grpc_response: Option<String>,
    execution_error: Option<String>,
}

fn dependency_label(suite: &str, scenario: &str) -> String {
    format!("{suite}({scenario})")
}

#[derive(Debug, Clone)]
pub struct SuiteRunSummary {
    pub suite: String,
    pub connector: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub results: Vec<SuiteScenarioResult>,
}

#[derive(Debug, Clone)]
pub struct AllSuitesRunSummary {
    pub connector: String,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub suites: Vec<SuiteRunSummary>,
}

#[derive(Debug, Clone)]
pub struct AllConnectorsRunSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub connectors: Vec<AllSuitesRunSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionBackend {
    Grpcurl,
    SdkFfi,
}

#[derive(Debug, Clone, Copy)]
pub struct SuiteRunOptions<'a> {
    pub endpoint: Option<&'a str>,
    pub merchant_id: Option<&'a str>,
    pub tenant_id: Option<&'a str>,
    pub plaintext: bool,
    pub backend: ExecutionBackend,
    pub report: bool,
}

impl Default for SuiteRunOptions<'_> {
    fn default() -> Self {
        Self {
            endpoint: None,
            merchant_id: None,
            tenant_id: None,
            plaintext: true,
            backend: ExecutionBackend::Grpcurl,
            report: false,
        }
    }
}

/// Runs all scenarios in one suite using default execution options.
pub fn run_suite_test(
    suite: &str,
    connector: Option<&str>,
) -> Result<SuiteRunSummary, ScenarioError> {
    run_suite_test_with_options(suite, connector, SuiteRunOptions::default())
}

/// Runs one specific scenario with explicit execution options.
pub fn run_scenario_test_with_options(
    suite: &str,
    scenario: &str,
    connector: Option<&str>,
    options: SuiteRunOptions<'_>,
) -> Result<SuiteRunSummary, ScenarioError> {
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let target_suite_spec = load_suite_spec(suite)?;
    let scenarios = load_suite_scenarios(suite)?;

    if !scenarios.contains_key(scenario) {
        return Err(ScenarioError::ScenarioNotFound {
            suite: suite.to_string(),
            scenario: scenario.to_string(),
        });
    }

    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    let dependency_chain = execute_dependency_chain(
        &target_suite_spec.depends_on,
        connector,
        options,
        target_suite_spec.strict_dependencies,
        &mut passed,
        &mut failed,
        &mut results,
    )?;

    let Some((
        dependency_reqs,
        dependency_res,
        dependency_labels,
        explicit_context_entries,
        dependency_entries,
    )) = dependency_chain
    else {
        return Ok(SuiteRunSummary {
            suite: suite.to_string(),
            connector: connector.to_string(),
            passed,
            failed,
            skipped: 0,
            results,
        });
    };

    match execute_single_scenario_with_context(
        suite,
        scenario,
        connector,
        options,
        &dependency_reqs,
        &dependency_res,
        &explicit_context_entries,
        &dependency_entries,
    ) {
        Ok(executed) => {
            match do_assertion(
                &executed.assertions,
                &executed.response_json,
                &executed.effective_req,
            ) {
                Ok(()) => {
                    passed += 1;
                    results.push(SuiteScenarioResult {
                        suite: suite.to_string(),
                        scenario: scenario.to_string(),
                        is_dependency: false,
                        passed: true,
                        skipped: false,
                        error: None,
                        dependency: dependency_labels,
                        req_body: Some(executed.effective_req),
                        res_body: Some(executed.response_json),
                        grpc_request: executed.grpc_request,
                        grpc_response: executed.grpc_response,
                    });
                }
                Err(error) => {
                    failed += 1;
                    results.push(SuiteScenarioResult {
                        suite: suite.to_string(),
                        scenario: scenario.to_string(),
                        is_dependency: false,
                        passed: false,
                        skipped: false,
                        error: Some(error.to_string()),
                        dependency: dependency_labels,
                        req_body: Some(executed.effective_req),
                        res_body: Some(executed.response_json),
                        grpc_request: executed.grpc_request,
                        grpc_response: executed.grpc_response,
                    });
                }
            }
        }
        Err(ScenarioError::Skipped { reason }) => {
            tracing::info!(
                scenario,
                %reason,
                "assertion result: SKIP"
            );
            results.push(SuiteScenarioResult {
                suite: suite.to_string(),
                scenario: scenario.to_string(),
                is_dependency: false,
                passed: false,
                skipped: true,
                error: Some(reason),
                dependency: dependency_labels,
                req_body: None,
                res_body: None,
                grpc_request: None,
                grpc_response: None,
            });
        }
        Err(error) => {
            failed += 1;
            results.push(SuiteScenarioResult {
                suite: suite.to_string(),
                scenario: scenario.to_string(),
                is_dependency: false,
                passed: false,
                skipped: false,
                error: Some(error.to_string()),
                dependency: dependency_labels,
                req_body: None,
                res_body: None,
                grpc_request: None,
                grpc_response: None,
            });
        }
    }

    Ok(SuiteRunSummary {
        suite: suite.to_string(),
        connector: connector.to_string(),
        passed,
        failed,
        skipped: results
            .iter()
            .filter(|r| r.skipped && !r.is_dependency)
            .count(),
        results,
    })
}

/// Runs all supported suites for one connector using default options.
pub fn run_all_suites(connector: Option<&str>) -> Result<AllSuitesRunSummary, ScenarioError> {
    run_all_suites_with_options(connector, SuiteRunOptions::default())
}

/// Runs all supported suites for one connector using explicit options.
pub fn run_all_suites_with_options(
    connector: Option<&str>,
    options: SuiteRunOptions<'_>,
) -> Result<AllSuitesRunSummary, ScenarioError> {
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let supported_suites = load_supported_suites_for_connector(connector)?;

    let mut suite_summaries = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for suite in supported_suites {
        let summary = run_suite_test_with_options(&suite, Some(connector), options)?;
        passed += summary.passed;
        failed += summary.failed;
        suite_summaries.push(summary);
    }

    Ok(AllSuitesRunSummary {
        connector: connector.to_string(),
        passed,
        failed,
        skipped: suite_summaries.iter().map(|s| s.skipped).sum(),
        suites: suite_summaries,
    })
}

/// Runs all configured connectors using default options.
pub fn run_all_connectors() -> Result<AllConnectorsRunSummary, ScenarioError> {
    run_all_connectors_with_options(SuiteRunOptions::default())
}

/// Runs all configured connectors using explicit execution options.
pub fn run_all_connectors_with_options(
    options: SuiteRunOptions<'_>,
) -> Result<AllConnectorsRunSummary, ScenarioError> {
    let all_connectors = configured_all_connectors();
    let mut runnable_connectors = Vec::new();

    for connector in all_connectors {
        match load_connector_config(&connector) {
            Ok(_) => runnable_connectors.push(connector),
            Err(error) => {
                tracing::warn!(
                    connector,
                    %error,
                    "skipping connector due to missing/invalid credentials"
                );
            }
        }
    }

    let mut connector_summaries = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for connector in runnable_connectors {
        let summary = run_all_suites_with_options(Some(&connector), options)?;
        passed += summary.passed;
        failed += summary.failed;
        connector_summaries.push(summary);
    }

    Ok(AllConnectorsRunSummary {
        passed,
        failed,
        skipped: connector_summaries.iter().map(|s| s.skipped).sum(),
        connectors: connector_summaries,
    })
}

/// Runs one suite with explicit execution options.
pub fn run_suite_test_with_options(
    suite: &str,
    connector: Option<&str>,
    options: SuiteRunOptions<'_>,
) -> Result<SuiteRunSummary, ScenarioError> {
    let connector = connector.unwrap_or(DEFAULT_CONNECTOR);
    let target_suite_spec = load_suite_spec(suite)?;
    let scenarios = load_suite_scenarios(suite)?;

    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    match target_suite_spec.dependency_scope {
        DependencyScope::Suite => {
            let dependency_chain = execute_dependency_chain(
                &target_suite_spec.depends_on,
                connector,
                options,
                target_suite_spec.strict_dependencies,
                &mut passed,
                &mut failed,
                &mut results,
            )?;
            let Some((
                dependency_reqs,
                dependency_res,
                dependency_labels,
                explicit_context_entries,
                dependency_entries,
            )) = dependency_chain
            else {
                return Ok(SuiteRunSummary {
                    suite: suite.to_string(),
                    connector: connector.to_string(),
                    passed,
                    failed,
                    skipped,
                    results,
                });
            };

            for scenario in scenarios.keys() {
                match execute_single_scenario_with_context(
                    suite,
                    scenario,
                    connector,
                    options,
                    &dependency_reqs,
                    &dependency_res,
                    &explicit_context_entries,
                    &dependency_entries,
                ) {
                    Ok(executed) => {
                        if let Some(execution_error) = executed.execution_error.clone() {
                            failed += 1;
                            results.push(SuiteScenarioResult {
                                suite: suite.to_string(),
                                scenario: scenario.to_string(),
                                is_dependency: false,
                                passed: false,
                                skipped: false,
                                error: Some(execution_error),
                                dependency: dependency_labels.clone(),
                                req_body: Some(executed.effective_req),
                                res_body: Some(executed.response_json),
                                grpc_request: executed.grpc_request,
                                grpc_response: executed.grpc_response,
                            });
                            continue;
                        }

                        match do_assertion(
                            &executed.assertions,
                            &executed.response_json,
                            &executed.effective_req,
                        ) {
                            Ok(()) => {
                                passed += 1;
                                results.push(SuiteScenarioResult {
                                    suite: suite.to_string(),
                                    scenario: scenario.to_string(),
                                    is_dependency: false,
                                    passed: true,
                                    skipped: false,
                                    error: None,
                                    dependency: dependency_labels.clone(),
                                    req_body: Some(executed.effective_req),
                                    res_body: Some(executed.response_json),
                                    grpc_request: executed.grpc_request,
                                    grpc_response: executed.grpc_response,
                                });
                            }
                            Err(error) => {
                                failed += 1;
                                results.push(SuiteScenarioResult {
                                    suite: suite.to_string(),
                                    scenario: scenario.to_string(),
                                    is_dependency: false,
                                    passed: false,
                                    skipped: false,
                                    error: Some(error.to_string()),
                                    dependency: dependency_labels.clone(),
                                    req_body: Some(executed.effective_req),
                                    res_body: Some(executed.response_json),
                                    grpc_request: executed.grpc_request,
                                    grpc_response: executed.grpc_response,
                                });
                            }
                        }
                    }
                    Err(ScenarioError::Skipped { reason }) => {
                        skipped += 1;
                        results.push(SuiteScenarioResult {
                            suite: suite.to_string(),
                            scenario: scenario.to_string(),
                            is_dependency: false,
                            passed: false,
                            skipped: true,
                            error: Some(reason),
                            dependency: dependency_labels.clone(),
                            req_body: None,
                            res_body: None,
                            grpc_request: None,
                            grpc_response: None,
                        });
                    }
                    Err(error) => {
                        failed += 1;
                        results.push(SuiteScenarioResult {
                            suite: suite.to_string(),
                            scenario: scenario.to_string(),
                            is_dependency: false,
                            passed: false,
                            skipped: false,
                            error: Some(error.to_string()),
                            dependency: dependency_labels.clone(),
                            req_body: None,
                            res_body: None,
                            grpc_request: None,
                            grpc_response: None,
                        });
                    }
                }
            }
        }
        DependencyScope::Scenario => {
            for scenario in scenarios.keys() {
                let dependency_chain = execute_dependency_chain(
                    &target_suite_spec.depends_on,
                    connector,
                    options,
                    target_suite_spec.strict_dependencies,
                    &mut passed,
                    &mut failed,
                    &mut results,
                )?;
                let Some((
                    dependency_reqs,
                    dependency_res,
                    dependency_labels,
                    explicit_context_entries,
                    dependency_entries,
                )) = dependency_chain
                else {
                    return Ok(SuiteRunSummary {
                        suite: suite.to_string(),
                        connector: connector.to_string(),
                        passed,
                        failed,
                        skipped,
                        results,
                    });
                };

                match execute_single_scenario_with_context(
                    suite,
                    scenario,
                    connector,
                    options,
                    &dependency_reqs,
                    &dependency_res,
                    &explicit_context_entries,
                    &dependency_entries,
                ) {
                    Ok(executed) => {
                        if let Some(execution_error) = executed.execution_error.clone() {
                            failed += 1;
                            results.push(SuiteScenarioResult {
                                suite: suite.to_string(),
                                scenario: scenario.to_string(),
                                is_dependency: false,
                                passed: false,
                                skipped: false,
                                error: Some(execution_error),
                                dependency: dependency_labels,
                                req_body: Some(executed.effective_req),
                                res_body: Some(executed.response_json),
                                grpc_request: executed.grpc_request,
                                grpc_response: executed.grpc_response,
                            });
                            continue;
                        }

                        match do_assertion(
                            &executed.assertions,
                            &executed.response_json,
                            &executed.effective_req,
                        ) {
                            Ok(()) => {
                                passed += 1;
                                results.push(SuiteScenarioResult {
                                    suite: suite.to_string(),
                                    scenario: scenario.to_string(),
                                    is_dependency: false,
                                    passed: true,
                                    skipped: false,
                                    error: None,
                                    dependency: dependency_labels,
                                    req_body: Some(executed.effective_req),
                                    res_body: Some(executed.response_json),
                                    grpc_request: executed.grpc_request,
                                    grpc_response: executed.grpc_response,
                                });
                            }
                            Err(error) => {
                                failed += 1;
                                results.push(SuiteScenarioResult {
                                    suite: suite.to_string(),
                                    scenario: scenario.to_string(),
                                    is_dependency: false,
                                    passed: false,
                                    skipped: false,
                                    error: Some(error.to_string()),
                                    dependency: dependency_labels,
                                    req_body: Some(executed.effective_req),
                                    res_body: Some(executed.response_json),
                                    grpc_request: executed.grpc_request,
                                    grpc_response: executed.grpc_response,
                                });
                            }
                        }
                    }
                    Err(ScenarioError::Skipped { reason }) => {
                        skipped += 1;
                        results.push(SuiteScenarioResult {
                            suite: suite.to_string(),
                            scenario: scenario.to_string(),
                            is_dependency: false,
                            passed: false,
                            skipped: true,
                            error: Some(reason),
                            dependency: dependency_labels,
                            req_body: None,
                            res_body: None,
                            grpc_request: None,
                            grpc_response: None,
                        });
                    }
                    Err(error) => {
                        failed += 1;
                        results.push(SuiteScenarioResult {
                            suite: suite.to_string(),
                            scenario: scenario.to_string(),
                            is_dependency: false,
                            passed: false,
                            skipped: false,
                            error: Some(error.to_string()),
                            dependency: dependency_labels,
                            req_body: None,
                            res_body: None,
                            grpc_request: None,
                            grpc_response: None,
                        });
                    }
                }
            }
        }
    }

    Ok(SuiteRunSummary {
        suite: suite.to_string(),
        connector: connector.to_string(),
        passed,
        failed,
        skipped,
        results,
    })
}

fn execute_dependency_chain(
    dependencies: &[SuiteDependency],
    connector: &str,
    options: SuiteRunOptions<'_>,
    strict_dependencies: bool,
    passed: &mut usize,
    failed: &mut usize,
    results: &mut Vec<SuiteScenarioResult>,
) -> Result<Option<DependencyContext>, ScenarioError> {
    let mut dependency_reqs = Vec::new();
    let mut dependency_res = Vec::new();
    let mut dependency_labels = Vec::new();
    let mut explicit_context_entries = Vec::new();
    let mut dependency_entries = Vec::new();

    for dependency in dependencies {
        let dependency_suite = dependency.suite();
        let is_supported = is_suite_supported_for_connector(connector, dependency_suite)?;
        if !is_supported {
            continue;
        }

        let dependency_scenario = if let Some(scenario) = dependency.scenario() {
            scenario.to_string()
        } else {
            load_default_scenario_name(dependency_suite)?
        };
        let current_label = dependency_label(dependency_suite, &dependency_scenario);

        let dep_result = execute_single_scenario_with_context(
            dependency_suite,
            &dependency_scenario,
            connector,
            options,
            &dependency_reqs,
            &dependency_res,
            &[],
            &dependency_entries,
        );

        match dep_result {
            Ok(executed) => {
                if let Some(execution_error) = executed.execution_error.clone() {
                    if let Some(context_map) = dependency.context_map() {
                        if !context_map.is_empty() {
                            explicit_context_entries.push((
                                context_map.clone(),
                                executed.effective_req.clone(),
                                executed.response_json.clone(),
                            ));
                        }
                    }
                    dependency_reqs.push(executed.effective_req.clone());
                    dependency_res.push(executed.response_json.clone());
                    dependency_entries.push(ExecutedDependency {
                        suite: dependency_suite.to_string(),
                        scenario: dependency_scenario.clone(),
                        res: executed.response_json.clone(),
                    });

                    *failed += 1;
                    results.push(SuiteScenarioResult {
                        suite: dependency_suite.to_string(),
                        scenario: dependency_scenario,
                        is_dependency: true,
                        passed: false,
                        skipped: false,
                        error: Some(execution_error),
                        dependency: dependency_labels.clone(),
                        req_body: Some(executed.effective_req),
                        res_body: Some(executed.response_json),
                        grpc_request: executed.grpc_request,
                        grpc_response: executed.grpc_response,
                    });
                    dependency_labels.push(current_label);

                    if strict_dependencies {
                        return Ok(None);
                    }
                    continue;
                }

                match do_assertion(
                    &executed.assertions,
                    &executed.response_json,
                    &executed.effective_req,
                ) {
                    Ok(()) => {
                        *passed += 1;
                        if let Some(context_map) = dependency.context_map() {
                            if !context_map.is_empty() {
                                explicit_context_entries.push((
                                    context_map.clone(),
                                    executed.effective_req.clone(),
                                    executed.response_json.clone(),
                                ));
                            }
                        }
                        dependency_reqs.push(executed.effective_req.clone());
                        dependency_res.push(executed.response_json.clone());
                        dependency_entries.push(ExecutedDependency {
                            suite: dependency_suite.to_string(),
                            scenario: dependency_scenario.clone(),
                            res: executed.response_json.clone(),
                        });
                        results.push(SuiteScenarioResult {
                            suite: dependency_suite.to_string(),
                            scenario: dependency_scenario,
                            is_dependency: true,
                            passed: true,
                            skipped: false,
                            error: None,
                            dependency: dependency_labels.clone(),
                            req_body: Some(executed.effective_req),
                            res_body: Some(executed.response_json),
                            grpc_request: executed.grpc_request,
                            grpc_response: executed.grpc_response,
                        });
                        dependency_labels.push(current_label);
                    }
                    Err(error) => {
                        if let Some(mut normalized_response) =
                            normalize_nexixpay_notneeded_postauth_dependency(
                                connector,
                                dependency_suite,
                                &executed.effective_req,
                                &executed.response_json,
                            )
                        {
                            *passed += 1;
                            if let Some(context_map) = dependency.context_map() {
                                if !context_map.is_empty() {
                                    explicit_context_entries.push((
                                        context_map.clone(),
                                        executed.effective_req.clone(),
                                        normalized_response.clone(),
                                    ));
                                }
                            }
                            dependency_reqs.push(executed.effective_req.clone());
                            dependency_res.push(normalized_response.clone());
                            dependency_entries.push(ExecutedDependency {
                                suite: dependency_suite.to_string(),
                                scenario: dependency_scenario.clone(),
                                res: normalized_response.clone(),
                            });
                            results.push(SuiteScenarioResult {
                                suite: dependency_suite.to_string(),
                                scenario: dependency_scenario,
                                is_dependency: true,
                                passed: true,
                                skipped: false,
                                error: None,
                                dependency: dependency_labels.clone(),
                                req_body: Some(executed.effective_req),
                                res_body: Some(std::mem::take(&mut normalized_response)),
                                grpc_request: executed.grpc_request,
                                grpc_response: executed.grpc_response,
                            });
                            dependency_labels.push(current_label);
                            continue;
                        }

                        if let Some(context_map) = dependency.context_map() {
                            if !context_map.is_empty() {
                                explicit_context_entries.push((
                                    context_map.clone(),
                                    executed.effective_req.clone(),
                                    executed.response_json.clone(),
                                ));
                            }
                        }
                        dependency_reqs.push(executed.effective_req.clone());
                        dependency_res.push(executed.response_json.clone());
                        dependency_entries.push(ExecutedDependency {
                            suite: dependency_suite.to_string(),
                            scenario: dependency_scenario.clone(),
                            res: executed.response_json.clone(),
                        });

                        *failed += 1;
                        results.push(SuiteScenarioResult {
                            suite: dependency_suite.to_string(),
                            scenario: dependency_scenario,
                            is_dependency: true,
                            passed: false,
                            skipped: false,
                            error: Some(error.to_string()),
                            dependency: dependency_labels.clone(),
                            req_body: Some(executed.effective_req),
                            res_body: Some(executed.response_json),
                            grpc_request: executed.grpc_request,
                            grpc_response: executed.grpc_response,
                        });
                        dependency_labels.push(current_label);

                        if strict_dependencies {
                            return Ok(None);
                        }
                    }
                }
            }
            Err(ScenarioError::Skipped { reason }) => {
                // A dependency that skips: treat as non-fatal, skip the whole chain entry
                tracing::info!(
                    suite = dependency_suite,
                    scenario = dependency_scenario,
                    %reason,
                    "dependency skipped"
                );
                results.push(SuiteScenarioResult {
                    suite: dependency_suite.to_string(),
                    scenario: dependency_scenario,
                    is_dependency: true,
                    passed: false,
                    skipped: true,
                    error: Some(reason),
                    dependency: dependency_labels.clone(),
                    req_body: None,
                    res_body: None,
                    grpc_request: None,
                    grpc_response: None,
                });
                dependency_labels.push(current_label);

                if strict_dependencies {
                    return Ok(None);
                }
            }
            Err(error) => {
                *failed += 1;
                results.push(SuiteScenarioResult {
                    suite: dependency_suite.to_string(),
                    scenario: dependency_scenario,
                    is_dependency: true,
                    passed: false,
                    skipped: false,
                    error: Some(error.to_string()),
                    dependency: dependency_labels.clone(),
                    req_body: None,
                    res_body: None,
                    grpc_request: None,
                    grpc_response: None,
                });
                dependency_labels.push(current_label);

                if strict_dependencies {
                    return Ok(None);
                }
            }
        }
    }

    Ok(Some((
        dependency_reqs,
        dependency_res,
        dependency_labels,
        explicit_context_entries,
        dependency_entries,
    )))
}

fn normalize_nexixpay_notneeded_postauth_dependency(
    connector: &str,
    dependency_suite: &str,
    effective_req: &Value,
    response_json: &Value,
) -> Option<Value> {
    if !connector.eq_ignore_ascii_case("nexixpay")
        || dependency_suite != "PaymentMethodAuthenticationService/PostAuthenticate"
    {
        return None;
    }

    let connector_code =
        lookup_json_path_with_case_fallback(response_json, "error.connector_details.code")
            .and_then(Value::as_str)
            .or_else(|| {
                lookup_json_path_with_case_fallback(response_json, "error.connectorDetails.code")
                    .and_then(Value::as_str)
            });
    if connector_code != Some("GW00488") {
        return None;
    }

    let pa_res =
        lookup_json_path_with_case_fallback(effective_req, "redirection_response.payload.PaRes")
            .and_then(Value::as_str)?;
    if !pa_res.eq_ignore_ascii_case("notneeded") {
        return None;
    }

    let payment_id = lookup_json_path_with_case_fallback(
        effective_req,
        "redirection_response.payload.paymentId",
    )
    .and_then(Value::as_str)?;

    let mut normalized = response_json.clone();
    let _ = set_json_path_value(
        &mut normalized,
        "status",
        Value::String("AUTHENTICATION_SUCCESSFUL".to_string()),
    );
    let _ = set_json_path_value(
        &mut normalized,
        "authentication_data.transaction_id",
        Value::String(payment_id.to_string()),
    );
    let _ = set_json_path_value(
        &mut normalized,
        "authentication_data.connector_transaction_id",
        Value::String(payment_id.to_string()),
    );

    Some(normalized)
}

fn append_follow_up_trace(existing: &mut Option<String>, heading: &str, payload: String) {
    let merged = match existing.take() {
        Some(previous) => format!("{previous}\n\n{heading}\n{payload}"),
        None => payload,
    };
    *existing = Some(merged);
}

fn maybe_sync_complete_authorize_pending(
    suite: &str,
    connector: &str,
    options: SuiteRunOptions<'_>,
    effective_req: &Value,
    response_json: &mut Value,
    grpc_request: &mut Option<String>,
    grpc_response: &mut Option<String>,
) -> Result<(), ScenarioError> {
    if suite != "PaymentService/CompleteAuthorize" || options.backend != ExecutionBackend::Grpcurl {
        return Ok(());
    }

    let Some(status) = lookup_json_path_with_case_fallback(response_json, "status")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
    else {
        return Ok(());
    };

    if status != "AUTHENTICATION_PENDING" {
        return Ok(());
    }

    let Some(connector_transaction_id) =
        lookup_json_path_with_case_fallback(effective_req, "merchant_order_id")
            .or_else(|| {
                lookup_json_path_with_case_fallback(effective_req, "merchant_transaction_id")
            })
            .or_else(|| {
                lookup_json_path_with_case_fallback(response_json, "connector_transaction_id")
            })
            .or_else(|| {
                lookup_json_path_with_case_fallback(response_json, "connectorTransactionId")
            })
            .and_then(Value::as_str)
            .map(|value| value.to_string())
    else {
        return Ok(());
    };

    let amount = lookup_json_path_with_case_fallback(effective_req, "amount")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "minor_amount": 0, "currency": "USD" }));
    let state = lookup_json_path_with_case_fallback(effective_req, "state")
        .cloned()
        .unwrap_or_else(|| {
            serde_json::json!({
                "connector_customer_id": "",
                "access_token": {
                    "token": { "value": "" },
                    "token_type": "",
                    "expires_in_seconds": 0
                }
            })
        });

    let sync_request = serde_json::json!({
        "connector_transaction_id": connector_transaction_id,
        "amount": amount,
        "state": state
    });

    for attempt in 1..=4 {
        let trace = execute_grpcurl_request_from_payload_with_trace(
            "PaymentService/Get",
            "sync_payment",
            &sync_request,
            options.endpoint,
            Some(connector),
            options.merchant_id,
            options.tenant_id,
            options.plaintext,
        )?;

        if trace.success {
            let mut sync_json = match serde_json::from_str::<Value>(&trace.response_body) {
                Ok(value) => value,
                Err(_) => {
                    if attempt < 4 {
                        thread::sleep(Duration::from_secs(2));
                    }
                    continue;
                }
            };

            transform_response_for_connector(
                connector,
                "PaymentService/Get",
                "sync_payment",
                &mut sync_json,
            );

            let sync_status = lookup_json_path_with_case_fallback(&sync_json, "status")
                .and_then(Value::as_str)
                .unwrap_or("");

            if !matches!(sync_status, "AUTHENTICATION_PENDING" | "PENDING") {
                *response_json = sync_json;
                append_follow_up_trace(
                    grpc_request,
                    "# follow-up sync request",
                    trace.request_command,
                );
                append_follow_up_trace(
                    grpc_response,
                    "# follow-up sync response",
                    trace.response_output,
                );
                return Ok(());
            }
        }

        if attempt < 4 {
            thread::sleep(Duration::from_secs(2));
        }
    }

    Ok(())
}

#[allow(clippy::print_stdout, clippy::print_stderr)]
fn execute_single_scenario_with_context(
    suite: &str,
    scenario: &str,
    connector: &str,
    options: SuiteRunOptions<'_>,
    dependency_reqs: &[Value],
    dependency_res: &[Value],
    explicit_context_entries: &[ExplicitContextEntry],
    dependency_entries: &[ExecutedDependency],
) -> Result<ExecutedScenario, ScenarioError> {
    run_test(Some(suite), Some(scenario), Some(connector))?;

    let (mut effective_req, assertions) =
        load_effective_scenario_for_connector(suite, scenario, connector)?;

    // Normalize legacy empty placeholders to auto_generate sentinels where needed.
    prepare_context_placeholders(suite, connector, &mut effective_req);

    // Context first — fill fields from dependency responses.
    add_context(dependency_reqs, dependency_res, &mut effective_req);

    // Apply any explicit dependency path mappings from suite_spec.json.
    apply_context_map(explicit_context_entries, &mut effective_req);

    // Clean up empty wrapper objects left over from context propagation.
    // This MUST run before resolve_auto_generate so that unresolved
    // "auto_generate" sentinels inside wrapper objects like
    // connector_feature_data and state.access_token are detected and
    // removed — otherwise resolve_auto_generate would replace them with
    // random "gen_XXXXX" values that the server cannot parse.
    prune_empty_context_wrappers(&mut effective_req);

    // Generate values for any remaining "auto_generate" sentinels and resolve
    // "connector_name" placeholders to the uppercase connector enum name.
    // Since context has already been applied, dependency-carried fields are
    // already filled and won't be touched.
    resolve_auto_generate(&mut effective_req, connector)?;

    if std::env::var("UCS_DEBUG_EFFECTIVE_REQ").as_deref() == Ok("1") {
        if let Ok(request_json) = serde_json::to_string_pretty(&effective_req) {
            println!(
                "[suite_run_test] effective_grpc_req suite={suite} scenario={scenario}:\n{request_json}"
            );
        }
    }

    maybe_execute_browser_automation_for_suite(
        suite,
        scenario,
        connector,
        dependency_entries,
        &mut effective_req,
    )?;

    let (response, mut grpc_request, mut grpc_response) = match options.backend {
        ExecutionBackend::Grpcurl => {
            // grpcurl validates field names against the proto schema, so the
            // payload must use proto-native field names and nested shapes.
            // Unlike the tonic path, grpcurl needs the full proto JSON shape
            // (e.g. `{"value": "..."}` wrappers for SecretString/CardNumberType)
            // and does NOT need prost-style oneof double-nesting.
            let grpcurl_payload =
                normalize_grpcurl_request_json(connector, suite, scenario, effective_req.clone());
            if std::env::var("UCS_DEBUG_GRPCURL_PAYLOAD").as_deref() == Ok("1") {
                if let Ok(dbg_json) = serde_json::to_string_pretty(&grpcurl_payload) {
                    eprintln!(
                        "[DEBUG] grpcurl_payload suite={suite} scenario={scenario}:\n{dbg_json}"
                    );
                }
            }
            let trace = execute_grpcurl_request_from_payload_with_trace(
                suite,
                scenario,
                &grpcurl_payload,
                options.endpoint,
                Some(connector),
                options.merchant_id,
                options.tenant_id,
                options.plaintext,
            )?;

            if !trace.success {
                let response_output = trace.response_output;
                let response_json = serde_json::from_str::<Value>(&trace.response_body)
                    .unwrap_or_else(|_| {
                        serde_json::json!({
                            "raw_response": response_output.clone(),
                        })
                    });

                return Ok(ExecutedScenario {
                    effective_req,
                    response_json,
                    assertions,
                    grpc_request: Some(trace.request_command),
                    grpc_response: Some(response_output.clone()),
                    execution_error: Some(response_output),
                });
            }

            (
                trace.response_body,
                Some(trace.request_command),
                Some(trace.response_output),
            )
        }
        ExecutionBackend::SdkFfi => (
            execute_sdk_request_from_payload(suite, scenario, &effective_req, connector)?,
            None,
            None,
        ),
    };

    let mut response_json: Value = serde_json::from_str(&response).map_err(|error| {
        let mut message = format!("failed to parse grpc response JSON: {error}");
        if let Some(trace) = grpc_response.as_deref() {
            message.push_str("\n\n");
            message.push_str(trace);
        }
        ScenarioError::GrpcurlExecution { message }
    })?;

    transform_response_for_connector(connector, suite, scenario, &mut response_json);

    maybe_sync_complete_authorize_pending(
        suite,
        connector,
        options,
        &effective_req,
        &mut response_json,
        &mut grpc_request,
        &mut grpc_response,
    )?;

    Ok(ExecutedScenario {
        effective_req,
        response_json,
        assertions,
        grpc_request,
        grpc_response,
        execution_error: None,
    })
}

fn grpc_method_for_suite(suite: &str, spec: Option<&SuiteSpec>) -> Result<String, ScenarioError> {
    // If suite_spec declares an explicit gRPC method, use it so suite dispatch
    // remains data-driven.
    if let Some(method) = spec.and_then(|s| s.grpc_method.as_deref()) {
        return Ok(method.to_string());
    }

    let method = match suite {
        "MerchantAuthenticationService/CreateServerAuthenticationToken" => {
            "types.MerchantAuthenticationService/CreateServerAuthenticationToken"
        }
        "CustomerService/Create" => "types.CustomerService/Create",
        "PaymentMethodAuthenticationService/PreAuthenticate" => {
            "types.PaymentMethodAuthenticationService/PreAuthenticate"
        }
        "PaymentMethodAuthenticationService/Authenticate" => {
            "types.PaymentMethodAuthenticationService/Authenticate"
        }
        "PaymentMethodAuthenticationService/PostAuthenticate" => {
            "types.PaymentMethodAuthenticationService/PostAuthenticate"
        }
        "PaymentService/Authorize" => "types.PaymentService/Authorize",
        "PaymentService/CompleteAuthorize" => "types.PaymentService/Authorize",
        "PaymentService/Capture" => "types.PaymentService/Capture",
        "PaymentService/Refund" => "types.PaymentService/Refund",
        "PaymentService/Void" => "types.PaymentService/Void",
        "PaymentService/Get" => "types.PaymentService/Get",
        "RefundService/Get" => "types.RefundService/Get",
        "PaymentService/SetupRecurring" => "types.PaymentService/SetupRecurring",
        "RecurringPaymentService/Charge" => "types.RecurringPaymentService/Charge",
        "PaymentService/CreateOrder" => "types.PaymentService/CreateOrder",
        "PaymentMethodService/Tokenize" => "types.PaymentMethodService/Tokenize",
        "RecurringPaymentService/Revoke" => "types.RecurringPaymentService/Revoke",
        "PaymentService/IncrementalAuthorization" => {
            "types.PaymentService/IncrementalAuthorization"
        }
        "PaymentService/Reverse" => "types.PaymentService/Reverse",
        "MerchantAuthenticationService/CreateServerSessionAuthenticationToken" => {
            "types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken"
        }
        "MerchantAuthenticationService/CreateClientAuthenticationToken" => {
            "types.MerchantAuthenticationService/CreateClientAuthenticationToken"
        }
        "PaymentService/VerifyRedirectResponse" => "types.PaymentService/VerifyRedirectResponse",
        "PaymentService/TokenAuthorize" => "types.PaymentService/TokenAuthorize",
        "PaymentService/TokenSetupRecurring" => "types.PaymentService/TokenSetupRecurring",
        "PaymentService/ProxyAuthorize" => "types.PaymentService/ProxyAuthorize",
        "PaymentService/ProxySetupRecurring" => "types.PaymentService/ProxySetupRecurring",
        "PaymentMethodService/Eligibility" => "types.PaymentMethodService/Eligibility",
        "EventService/HandleEvent" => "types.EventService/HandleEvent",
        _ => {
            return Err(ScenarioError::UnsupportedSuite {
                suite: suite.to_string(),
            })
        }
    };
    Ok(method.to_string())
}

/// Returns every suite name that has a mapping in `grpc_method_for_suite`.
///
/// This is the single authoritative list of known proto suites for the
/// integration-test harness. It must be kept in sync with the match arms
/// in `grpc_method_for_suite` above — both live in this file so any drift
/// is immediately visible.
///
/// Services that are out of scope for integration tests (PayoutService,
/// DisputeService) are already absent from this list.
pub fn all_known_suites() -> &'static [&'static str] {
    &[
        "PaymentMethodAuthenticationService/Authenticate",
        "PaymentService/Authorize",
        "PaymentService/Capture",
        "MerchantAuthenticationService/CreateClientAuthenticationToken",
        "PaymentService/CompleteAuthorize",
        "CustomerService/Create",
        "PaymentService/CreateOrder",
        "PaymentService/Get",
        "EventService/HandleEvent",
        "PaymentService/IncrementalAuthorization",
        "PaymentMethodService/Eligibility",
        "PaymentMethodAuthenticationService/PostAuthenticate",
        "PaymentMethodAuthenticationService/PreAuthenticate",
        "PaymentService/ProxyAuthorize",
        "PaymentService/ProxySetupRecurring",
        "RecurringPaymentService/Charge",
        "PaymentService/Refund",
        "RefundService/Get",
        "PaymentService/Reverse",
        "RecurringPaymentService/Revoke",
        "MerchantAuthenticationService/CreateServerAuthenticationToken",
        "MerchantAuthenticationService/CreateServerSessionAuthenticationToken",
        "PaymentService/SetupRecurring",
        "PaymentService/TokenAuthorize",
        "PaymentService/TokenSetupRecurring",
        "PaymentMethodService/Tokenize",
        "PaymentService/VerifyRedirectResponse",
        "PaymentService/Void",
    ]
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use grpc_api_types::payments;
    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};

    use std::collections::HashMap;

    use super::{
        add_context, apply_context_map, build_grpcurl_command, build_grpcurl_request,
        deep_set_json_path, extract_json_body_from_grpc_output, get_the_assertion,
        get_the_assertion_for_connector, get_the_grpc_req_for_connector, has_only_default_leaves,
        normalize_tonic_request_json, prepare_context_placeholders, prune_empty_context_wrappers,
        remove_json_path_if_all_defaults, run_test, DEFAULT_SCENARIO, DEFAULT_SUITE,
    };
    use crate::harness::auto_gen::resolve_auto_generate;
    use crate::harness::scenario_loader::{
        connector_spec_dir, discover_all_connectors, load_suite_scenarios, load_suite_spec,
        load_supported_suites_for_connector,
    };
    use crate::harness::scenario_types::{ContextMap, FieldAssert, ScenarioError};

    fn validate_tonic_payload_shape<T: DeserializeOwned>(
        connector: &str,
        suite: &str,
        scenario: &str,
        grpc_req: &Value,
    ) -> Result<(), String> {
        let normalized = normalize_tonic_request_json(connector, suite, scenario, grpc_req.clone());
        let serialized = serde_json::to_string(&normalized).map_err(|error| {
            format!(
                "{connector}/{suite}/{scenario}: failed to serialize normalized payload: {error}"
            )
        })?;

        let mut ignored_paths = BTreeSet::new();
        let mut deserializer = serde_json::Deserializer::from_str(&serialized);
        let _: T = serde_ignored::deserialize(&mut deserializer, |path| {
            ignored_paths.insert(path.to_string());
        })
        .map_err(|error| {
            format!(
                "{connector}/{suite}/{scenario}: proto parse failed (type/enum mismatch): {error}"
            )
        })?;

        if !ignored_paths.is_empty() {
            return Err(format!(
                "{connector}/{suite}/{scenario}: unknown/ignored request fields: {}",
                ignored_paths.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }

        Ok(())
    }

    fn validate_suite_scenario_schema(
        connector: &str,
        suite: &str,
        scenario: &str,
        grpc_req: &Value,
    ) -> Result<(), String> {
        // Resolve alias_for so connector-specific suites reuse standard proto types.
        let suite_spec_opt = load_suite_spec(suite).ok();
        let effective_suite = suite_spec_opt
            .as_ref()
            .and_then(|s| s.alias_for.as_deref())
            .unwrap_or(suite);

        match effective_suite {
            "MerchantAuthenticationService/CreateServerAuthenticationToken" => validate_tonic_payload_shape::<
                payments::MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest,
            >(connector, suite, scenario, grpc_req),
            "CustomerService/Create" => validate_tonic_payload_shape::<
                payments::CustomerServiceCreateRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentMethodAuthenticationService/PreAuthenticate" => validate_tonic_payload_shape::<
                payments::PaymentMethodAuthenticationServicePreAuthenticateRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentMethodAuthenticationService/Authenticate" => validate_tonic_payload_shape::<
                payments::PaymentMethodAuthenticationServiceAuthenticateRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentMethodAuthenticationService/PostAuthenticate" => validate_tonic_payload_shape::<
                payments::PaymentMethodAuthenticationServicePostAuthenticateRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/Authorize" => {
                validate_tonic_payload_shape::<payments::PaymentServiceAuthorizeRequest>(
                    connector, suite, scenario, grpc_req,
                )
            }
            "PaymentService/CompleteAuthorize" => validate_tonic_payload_shape::<
                payments::PaymentServiceAuthorizeRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/Capture" => validate_tonic_payload_shape::<payments::PaymentServiceCaptureRequest>(
                connector, suite, scenario, grpc_req,
            ),
            "PaymentService/Void" => validate_tonic_payload_shape::<payments::PaymentServiceVoidRequest>(
                connector, suite, scenario, grpc_req,
            ),
            "PaymentService/Refund" => validate_tonic_payload_shape::<payments::PaymentServiceRefundRequest>(
                connector, suite, scenario, grpc_req,
            ),
            "PaymentService/Get" => validate_tonic_payload_shape::<payments::PaymentServiceGetRequest>(
                connector, suite, scenario, grpc_req,
            ),
            "RefundService/Get" => validate_tonic_payload_shape::<payments::RefundServiceGetRequest>(
                connector, suite, scenario, grpc_req,
            ),
            "PaymentService/SetupRecurring" => validate_tonic_payload_shape::<
                payments::PaymentServiceSetupRecurringRequest,
            >(connector, suite, scenario, grpc_req),
            "RecurringPaymentService/Charge" => validate_tonic_payload_shape::<
                payments::RecurringPaymentServiceChargeRequest,
            >(connector, suite, scenario, grpc_req),
            "RecurringPaymentService/Revoke" => validate_tonic_payload_shape::<
                payments::RecurringPaymentServiceRevokeRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentMethodService/Tokenize" => validate_tonic_payload_shape::<
                payments::PaymentMethodServiceTokenizeRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/IncrementalAuthorization" => validate_tonic_payload_shape::<
                payments::PaymentServiceIncrementalAuthorizationRequest,
            >(connector, suite, scenario, grpc_req),
            "MerchantAuthenticationService/CreateServerSessionAuthenticationToken" => validate_tonic_payload_shape::<
                payments::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest,
            >(connector, suite, scenario, grpc_req),
            "MerchantAuthenticationService/CreateClientAuthenticationToken" => validate_tonic_payload_shape::<
                payments::MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/CreateOrder" => validate_tonic_payload_shape::<
                payments::PaymentServiceCreateOrderRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/Reverse" => validate_tonic_payload_shape::<
                payments::PaymentServiceReverseRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/VerifyRedirectResponse" => validate_tonic_payload_shape::<
                payments::PaymentServiceVerifyRedirectResponseRequest,
            >(connector, suite, scenario, grpc_req),
            "EventService/HandleEvent" => {
                // Webhook requests use base64 for the proto `bytes body` field,
                // which grpcurl interprets correctly but tonic serde expects a
                // byte array.  Skip tonic-level shape validation; the runtime
                // grpcurl path is the authoritative check.
                Ok(())
            }
            "PaymentService/TokenAuthorize" => validate_tonic_payload_shape::<
                payments::PaymentServiceTokenAuthorizeRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/TokenSetupRecurring" => validate_tonic_payload_shape::<
                payments::PaymentServiceTokenSetupRecurringRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/ProxyAuthorize" => validate_tonic_payload_shape::<
                payments::PaymentServiceProxyAuthorizeRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentService/ProxySetupRecurring" => validate_tonic_payload_shape::<
                payments::PaymentServiceProxySetupRecurringRequest,
            >(connector, suite, scenario, grpc_req),
            "PaymentMethodService/Eligibility" => validate_tonic_payload_shape::<
                payments::PayoutMethodEligibilityRequest,
            >(connector, suite, scenario, grpc_req),
            _ => Err(format!(
                "{connector}/{suite}/{scenario}: suite '{effective_suite}' is not mapped to a tonic request type"
            )),
        }
    }

    #[test]
    fn run_test_accepts_explicit_suite_and_scenario() {
        run_test(
            Some("PaymentService/Authorize"),
            Some("no3ds_manual_capture_credit_card"),
            Some("stripe"),
        )
        .expect("run_test should succeed for explicit inputs");
    }

    #[test]
    fn run_test_uses_default_suite_and_scenario() {
        assert_eq!(DEFAULT_SUITE, "PaymentService/Authorize");
        assert_eq!(DEFAULT_SCENARIO, "no3ds_auto_capture_credit_card");
        run_test(None, None, None).expect("run_test should succeed with defaults");
    }

    #[test]
    fn connector_override_is_applied_to_assertions() {
        let base_assertions = get_the_assertion("PaymentService/Authorize", "no3ds_fail_payment")
            .expect("base assertions load");
        let overridden_assertions = get_the_assertion_for_connector(
            "PaymentService/Authorize",
            "no3ds_fail_payment",
            "stripe",
        )
        .expect("connector assertions load");

        let base_message_rule = base_assertions
            .get("error.connector_details.message")
            .expect("base contains message assertion");
        let overridden_message_rule = overridden_assertions
            .get("error.connector_details.message")
            .expect("overridden contains message assertion");

        assert!(matches!(
            base_message_rule,
            FieldAssert::Contains { contains }
                if contains == "decline"
        ));
        assert!(matches!(
            overridden_message_rule,
            FieldAssert::Contains { contains }
                if contains == "declin"
        ));
        assert!(base_assertions.contains_key("status"));
        assert!(!overridden_assertions.contains_key("status"));
    }

    #[test]
    fn builds_grpcurl_command() {
        let command = build_grpcurl_command(
            Some("PaymentService/Authorize"),
            Some("no3ds_auto_capture_credit_card"),
            Some("localhost:50051"),
            Some("stripe"),
            Some("test_merchant"),
            Some("default"),
            true,
        )
        .expect("grpcurl command should build");

        assert!(command.contains("grpcurl -plaintext"));
        assert!(command.contains("types.PaymentService/Authorize"));
        assert!(command.contains("\"x-connector-config:"));
        assert!(command.contains("\"auth_type\": \"NO_THREE_DS\""));
    }

    #[test]
    fn builds_grpcurl_request_struct() {
        let request = build_grpcurl_request(
            Some("PaymentService/Authorize"),
            Some("no3ds_auto_capture_credit_card"),
            Some("localhost:50051"),
            Some("stripe"),
            Some("test_merchant"),
            Some("default"),
            true,
            false,
        )
        .expect("grpcurl request should build");

        assert_eq!(request.endpoint, "localhost:50051");
        assert_eq!(request.method, "types.PaymentService/Authorize");
        assert!(request.payload.contains("\"auth_type\": \"NO_THREE_DS\""));
        assert!(!request.headers.is_empty());
    }

    #[test]
    fn extracts_json_body_from_verbose_grpc_output() {
        let verbose_output = r#"
Resolved method descriptor:
rpc Authorize (...)

Request metadata to send:
x-connector: stripe

Response headers received:
content-type: application/grpc

Response contents:
{
  "status": "CHARGED",
  "connector_transaction_id": {
    "id": "txn_123"
  }
}

Response trailers received:
grpc-status: 0
"#;

        let body = extract_json_body_from_grpc_output(verbose_output, "")
            .expect("json body should be extracted from verbose output");
        let parsed: Value =
            serde_json::from_str(&body).expect("extracted body should parse as json");

        assert_eq!(parsed["status"], json!("CHARGED"));
        assert_eq!(parsed["connector_transaction_id"]["id"], json!("txn_123"));
    }

    #[test]
    fn extracts_plain_json_body_without_verbose_sections() {
        let body = extract_json_body_from_grpc_output("{\n  \"status\": \"PENDING\"\n}", "")
            .expect("plain json output should be returned");
        let parsed: Value = serde_json::from_str(&body).expect("plain json output should parse");
        assert_eq!(parsed["status"], json!("PENDING"));
    }

    #[test]
    fn build_grpcurl_request_resolves_auto_generate_placeholders() {
        let request = build_grpcurl_request(
            Some("PaymentService/Authorize"),
            Some("no3ds_manual_capture_credit_card"),
            Some("localhost:50051"),
            Some("stripe"),
            Some("test_merchant"),
            Some("default"),
            true,
            false,
        )
        .expect("grpcurl request should build");

        assert!(
            !request.payload.contains("auto_generate"),
            "payload should not contain unresolved placeholders"
        );
        assert!(
            !request.payload.contains("cust_global_no3ds"),
            "payload should not contain old static customer ids"
        );
        assert!(
            !request.payload.contains("+918056594427"),
            "payload should not contain old static phone numbers"
        );

        let payload: Value =
            serde_json::from_str(&request.payload).expect("payload should parse as json");
        let merchant_id = payload["merchant_transaction_id"]
            .as_str()
            .expect("merchant_transaction_id should be present");
        assert!(
            merchant_id.starts_with("mti_"),
            "merchant_transaction_id should be generated"
        );

        let customer_id = payload["customer"]["id"]
            .as_str()
            .expect("customer.id should be present");
        assert!(
            customer_id.starts_with("cust_"),
            "customer.id should be generated"
        );
    }

    #[test]
    fn add_context_overrides_with_latest_index_preference() {
        let prev_reqs = vec![
            json!({"customer": {"id": "cust_old"}}),
            json!({"customer": {"id": "cust_new"}}),
        ];
        let prev_res = vec![
            json!({"connectorTransactionId": {"id": "txn_old"}}),
            json!({"connectorTransactionId": {"id": "txn_new"}}),
        ];
        let mut current = json!({
            "customer": {"id": "cust_default"},
            "connector_transaction_id": {"id": "txn_default"}
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(current["customer"]["id"], json!("cust_new"));
        assert_eq!(current["connector_transaction_id"]["id"], json!("txn_new"));
    }

    #[test]
    fn add_context_keeps_target_scenario_specific_values_when_context_is_dependency_only() {
        let dependency_reqs = vec![json!({"customer": {"id": "cust_dep"}})];
        let dependency_res = vec![json!({"accessToken": "token_dep"})];

        let mut scenario_one_req = json!({
            "capture_method": "AUTOMATIC",
            "customer": {"id": "auto_generate"},
            "access_token": "auto_generate"
        });
        add_context(&dependency_reqs, &dependency_res, &mut scenario_one_req);
        assert_eq!(scenario_one_req["capture_method"], json!("AUTOMATIC"));
        assert_eq!(scenario_one_req["customer"]["id"], json!("cust_dep"));
        assert_eq!(scenario_one_req["access_token"], json!("token_dep"));

        let mut scenario_two_req = json!({
            "capture_method": "MANUAL",
            "customer": {"id": "auto_generate"},
            "access_token": "auto_generate"
        });
        add_context(&dependency_reqs, &dependency_res, &mut scenario_two_req);
        assert_eq!(scenario_two_req["capture_method"], json!("MANUAL"));
        assert_eq!(scenario_two_req["customer"]["id"], json!("cust_dep"));
        assert_eq!(scenario_two_req["access_token"], json!("token_dep"));
    }

    #[test]
    fn add_context_maps_refund_id_from_connector_refund_id() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({"connectorRefundId": "rf_123"})];
        let mut current = json!({
            "refund_id": "auto_generate"
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(current["refund_id"], json!("rf_123"));
    }

    #[test]
    fn add_context_maps_identifier_pascal_case_oneof_variant() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "connector_transaction_id": {
                "id_type": {
                    "Id": "txn_sdk_123"
                }
            }
        })];
        let mut current = json!({
            "connector_transaction_id": {
                "id": "auto_generate"
            }
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(
            current["connector_transaction_id"]["id"],
            json!("txn_sdk_123")
        );
    }

    #[test]
    fn add_context_maps_mandate_reference_into_mandate_reference_id() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "mandateReference": {
                "connectorMandateId": {
                    "connectorMandateId": "mdt_123"
                }
            }
        })];
        let mut current = json!({
            "mandate_reference_id": {
                "connector_mandate_id": {
                    "connector_mandate_id": "auto_generate"
                }
            }
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(
            current["mandate_reference_id"]["connector_mandate_id"]["connector_mandate_id"],
            json!("mdt_123")
        );
    }

    #[test]
    fn add_context_does_not_map_mandate_reference_into_connector_recurring_payment_id() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "mandateReference": {
                "connectorMandateId": {
                    "connectorMandateId": "mdt_456"
                }
            }
        })];
        let mut current = json!({
            "connector_recurring_payment_id": {
                "connector_mandate_id": {
                    "connector_mandate_id": "auto_generate"
                }
            }
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(
            current["connector_recurring_payment_id"]["connector_mandate_id"]
                ["connector_mandate_id"],
            json!("auto_generate")
        );
    }

    #[test]
    fn add_context_maps_access_token_fields_into_state_access_token() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "access_token": "tok_123",
            "token_type": "Bearer",
            "expires_in_seconds": 3600
        })];
        let mut current = json!({
            "state": {
                "access_token": {
                    "token": {
                        "value": ""
                    },
                    "token_type": "",
                    "expires_in_seconds": 0
                }
            }
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(
            current["state"]["access_token"]["token"]["value"],
            json!("tok_123")
        );
        assert_eq!(
            current["state"]["access_token"]["token_type"],
            json!("Bearer")
        );
        assert_eq!(
            current["state"]["access_token"]["expires_in_seconds"],
            json!(3600)
        );
    }

    #[test]
    fn add_context_maps_connector_customer_id_to_nested_targets() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "connector_customer_id": "cust_dep_123"
        })];

        let mut authorize_req = json!({
            "customer": {
                "connector_customer_id": ""
            }
        });
        add_context(&prev_reqs, &prev_res, &mut authorize_req);
        assert_eq!(
            authorize_req["customer"]["connector_customer_id"],
            json!("cust_dep_123")
        );

        let mut capture_req = json!({
            "state": {
                "connector_customer_id": ""
            }
        });
        add_context(&prev_reqs, &prev_res, &mut capture_req);
        assert_eq!(
            capture_req["state"]["connector_customer_id"],
            json!("cust_dep_123")
        );
    }

    #[test]
    fn add_context_maps_connector_feature_data_value() {
        let prev_reqs = vec![];
        let prev_res = vec![json!({
            "connectorFeatureData": {
                "value": "{\"authorize_id\":\"auth_123\"}"
            }
        })];
        let mut current = json!({
            "connector_feature_data": {
                "value": "auto_generate"
            }
        });

        add_context(&prev_reqs, &prev_res, &mut current);

        assert_eq!(
            current["connector_feature_data"]["value"],
            json!("{\"authorize_id\":\"auth_123\"}")
        );
    }

    #[test]
    fn prepare_context_placeholders_preserves_empty_values_and_injects_connector_feature_data() {
        let mut req = json!({
            "customer": { "connector_customer_id": "" },
            "state": {
                "connector_customer_id": "",
                "access_token": {
                    "token": { "value": "" },
                    "token_type": "",
                    "expires_in_seconds": 0
                }
            }
        });

        prepare_context_placeholders("PaymentService/Capture", "stripe", &mut req);

        // Empty values should be left as-is (NOT converted to "auto_generate").
        assert_eq!(req["customer"]["connector_customer_id"], json!(""));
        assert_eq!(req["state"]["connector_customer_id"], json!(""));
        assert_eq!(req["state"]["access_token"]["token"]["value"], json!(""));
        assert_eq!(req["state"]["access_token"]["token_type"], json!(""));
        assert_eq!(req["state"]["access_token"]["expires_in_seconds"], json!(0));

        // connector_feature_data should be injected for dependent suites.
        assert_eq!(
            req["connector_feature_data"]["value"],
            json!("auto_generate")
        );
    }

    #[test]
    fn prune_empty_context_wrappers_removes_unresolved_connector_feature_data() {
        let mut req = json!({
            "connector_feature_data": { "value": "auto_generate" },
            "state": {
                "access_token": {
                    "token": {}
                }
            },
            "merchant_transaction_id": { "id": "mti_real" }
        });

        prune_empty_context_wrappers(&mut req);

        // connector_feature_data with unresolved value should be removed.
        assert!(
            req.get("connector_feature_data").is_none() || req["connector_feature_data"].is_null()
        );
        // Empty nested wrappers should be cleaned up.
        assert!(req.get("state").is_none() || req["state"].is_null());
        // Real values should be kept.
        assert_eq!(req["merchant_transaction_id"]["id"], json!("mti_real"));
    }

    #[test]
    fn prune_empty_context_wrappers_keeps_resolved_values() {
        let mut req = json!({
            "customer": { "connector_customer_id": "cust_123" },
            "state": {
                "connector_customer_id": "cust_state_123",
                "access_token": {
                    "token": { "value": "tok_123" },
                    "token_type": "Bearer",
                    "expires_in_seconds": 3600
                }
            },
            "connector_feature_data": { "value": "{\"authorize_id\":\"auth_123\"}" },
            "connector_transaction_id": { "id": "pi_123" },
            "refund_id": "re_123"
        });

        prune_empty_context_wrappers(&mut req);

        assert_eq!(req["customer"]["connector_customer_id"], json!("cust_123"));
        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("tok_123")
        );
        assert_eq!(
            req["connector_feature_data"]["value"],
            json!("{\"authorize_id\":\"auth_123\"}")
        );
        assert_eq!(req["connector_transaction_id"]["id"], json!("pi_123"));
        assert_eq!(req["refund_id"], json!("re_123"));
    }

    #[test]
    fn has_only_default_leaves_detects_all_default_shapes() {
        // Primitive defaults.
        assert!(has_only_default_leaves(&json!("")));
        assert!(has_only_default_leaves(&json!(0)));
        assert!(has_only_default_leaves(&json!(0.0)));
        assert!(has_only_default_leaves(&json!(null)));
        assert!(has_only_default_leaves(&json!(false)));

        // Non-default primitives.
        assert!(!has_only_default_leaves(&json!("hello")));
        assert!(!has_only_default_leaves(&json!(42)));
        assert!(!has_only_default_leaves(&json!(3.14)));
        assert!(!has_only_default_leaves(&json!(true)));

        // Empty containers are all-default.
        assert!(has_only_default_leaves(&json!({})));
        assert!(has_only_default_leaves(&json!([])));

        // Nested all-default objects.
        assert!(has_only_default_leaves(&json!({
            "token": {"value": ""},
            "token_type": "",
            "expires_in_seconds": 0
        })));

        // Mixed: one real value makes it non-default.
        assert!(!has_only_default_leaves(&json!({
            "token": {"value": "tok_123"},
            "token_type": "",
            "expires_in_seconds": 0
        })));

        // Deeply nested all-default.
        assert!(has_only_default_leaves(&json!({
            "a": {"b": {"c": ""}, "d": 0},
            "e": null
        })));

        // Array of defaults.
        assert!(has_only_default_leaves(&json!(["", 0, null, false])));

        // Array with one real value.
        assert!(!has_only_default_leaves(&json!(["", "real", 0])));
    }

    #[test]
    fn prune_removes_all_default_subtree_for_access_token() {
        // This is the critical Bug 2 scenario: access_token has default
        // values (empty string, 0) that should be pruned when context
        // didn't fill any real values.
        let mut req = json!({
            "merchant_transaction_id": "mti_abc123",
            "state": {
                "access_token": {
                    "token": {"value": ""},
                    "token_type": "",
                    "expires_in_seconds": 0
                }
            }
        });

        prune_empty_context_wrappers(&mut req);

        // The entire state block should be pruned since all leaves are defaults.
        assert!(
            req.get("state").is_none() || req["state"].is_null(),
            "state with all-default access_token should be pruned"
        );
        // Real values should be kept.
        assert_eq!(req["merchant_transaction_id"], json!("mti_abc123"));
    }

    #[test]
    fn prune_keeps_access_token_with_real_values() {
        let mut req = json!({
            "state": {
                "access_token": {
                    "token": {"value": "tok_real_123"},
                    "token_type": "Bearer",
                    "expires_in_seconds": 3600
                }
            }
        });

        prune_empty_context_wrappers(&mut req);

        // Nothing should be pruned — real values present.
        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("tok_real_123")
        );
        assert_eq!(
            req["state"]["access_token"]["expires_in_seconds"],
            json!(3600)
        );
    }

    #[test]
    fn remove_json_path_if_all_defaults_removes_default_subtree() {
        let mut root = json!({
            "wrapper": {
                "inner": {"value": ""},
                "count": 0
            },
            "keep": "real_data"
        });

        let removed = remove_json_path_if_all_defaults(&mut root, "wrapper");
        assert!(removed, "should remove all-default subtree");
        assert!(root.get("wrapper").is_none());
        assert_eq!(root["keep"], json!("real_data"));
    }

    #[test]
    fn remove_json_path_if_all_defaults_keeps_non_default_subtree() {
        let mut root = json!({
            "wrapper": {
                "inner": {"value": "real"},
                "count": 0
            }
        });

        let removed = remove_json_path_if_all_defaults(&mut root, "wrapper");
        assert!(!removed, "should NOT remove subtree with real values");
        assert_eq!(root["wrapper"]["inner"]["value"], json!("real"));
    }

    #[test]
    fn normalizer_unwraps_value_wrappers() {
        let original = json!({
            "payment_method": {
                "card": {
                    "card_number": { "value": "4111111111111111" },
                    "card_holder_name": { "value": "John Doe" }
                }
            },
            "customer": {
                "email": { "value": "john@example.com" }
            }
        });

        let normalized = normalize_tonic_request_json(
            "stripe",
            "PaymentService/Authorize",
            "normalizer_unwraps_value_wrappers",
            original,
        );

        assert_eq!(
            normalized["payment_method"]["payment_method"]["card"]["card_number"],
            json!("4111111111111111")
        );
        assert_eq!(
            normalized["payment_method"]["payment_method"]["card"]["card_holder_name"],
            json!("John Doe")
        );
        assert_eq!(normalized["customer"]["email"], json!("john@example.com"));
    }

    #[test]
    fn normalizer_drops_legacy_get_handle_response_bool() {
        let original = json!({
            "connector_transaction_id": "txn_123",
            "handle_response": true
        });

        let normalized = normalize_tonic_request_json(
            "stripe",
            "PaymentService/Get",
            "normalizer_drops_legacy_get_handle_response_bool",
            original,
        );
        assert!(normalized.get("handle_response").is_none());
        assert_eq!(normalized["connector_transaction_id"], json!("txn_123"));
    }

    #[test]
    fn normalizer_adds_authorize_order_details_default() {
        let original = json!({
            "merchant_transaction_id": "m_123",
            "amount": {"minor_amount": 1000, "currency": "USD"}
        });

        let normalized = normalize_tonic_request_json(
            "stripe",
            "PaymentService/Authorize",
            "normalizer_adds_authorize_order_details_default",
            original,
        );
        assert_eq!(normalized["order_details"], json!([]));
    }

    #[test]
    fn normalizer_adds_customer_acceptance_accepted_at_default() {
        let original = json!({
            "customer_acceptance": {
                "acceptance_type": "OFFLINE"
            }
        });

        let normalized = normalize_tonic_request_json(
            "stripe",
            "PaymentService/SetupRecurring",
            "normalizer_adds_customer_acceptance_accepted_at_default",
            original,
        );
        let accepted_at = normalized["customer_acceptance"]["accepted_at"]
            .as_i64()
            .expect("accepted_at should be injected as i64");
        assert!(accepted_at >= 0);
    }

    #[test]
    fn normalizer_wraps_connector_recurring_mandate_oneof() {
        let original = json!({
            "connector_recurring_payment_id": {
                "connector_mandate_id": {
                    "connector_mandate_id": "mandate_123"
                }
            }
        });

        let normalized = normalize_tonic_request_json(
            "paypal",
            "RecurringPaymentService/Charge",
            "normalizer_wraps_connector_recurring_mandate_oneof",
            original,
        );

        assert_eq!(
            normalized["connector_recurring_payment_id"]["mandate_id_type"]["ConnectorMandateId"]
                ["connector_mandate_id"],
            json!("mandate_123")
        );
    }

    // ─── deep_set_json_path tests ───

    #[test]
    fn deep_set_creates_intermediate_objects() {
        let mut root = json!({});
        let ok = deep_set_json_path(
            &mut root,
            "state.access_token.token.value",
            json!("tok_abc"),
        );
        assert!(ok);
        assert_eq!(
            root["state"]["access_token"]["token"]["value"],
            json!("tok_abc")
        );
    }

    #[test]
    fn deep_set_overwrites_existing_leaf() {
        let mut root = json!({"state": {"access_token": {"token": {"value": "old"}}}});
        let ok = deep_set_json_path(&mut root, "state.access_token.token.value", json!("new"));
        assert!(ok);
        assert_eq!(
            root["state"]["access_token"]["token"]["value"],
            json!("new")
        );
    }

    #[test]
    fn deep_set_single_segment() {
        let mut root = json!({"foo": "bar"});
        let ok = deep_set_json_path(&mut root, "baz", json!(42));
        assert!(ok);
        assert_eq!(root["baz"], json!(42));
        // original field untouched
        assert_eq!(root["foo"], json!("bar"));
    }

    #[test]
    fn deep_set_partial_existing_path() {
        let mut root = json!({"state": {"existing": true}});
        let ok = deep_set_json_path(
            &mut root,
            "state.access_token.token.value",
            json!("tok_xyz"),
        );
        assert!(ok);
        assert_eq!(
            root["state"]["access_token"]["token"]["value"],
            json!("tok_xyz")
        );
        // existing sibling untouched
        assert_eq!(root["state"]["existing"], json!(true));
    }

    // ─── apply_context_map tests ───

    #[test]
    fn apply_context_map_maps_response_field_to_deep_target() {
        let mut context_map: ContextMap = HashMap::new();
        context_map.insert(
            "state.access_token.token.value".to_string(),
            "res.access_token".to_string(),
        );

        let dep_req = json!({});
        let dep_res = json!({"access_token": "paypal_tok_123"});
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({"amount": {"minor_amount": 1000}});
        apply_context_map(&collected, &mut req);

        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("paypal_tok_123")
        );
        // original field untouched
        assert_eq!(req["amount"]["minor_amount"], json!(1000));
    }

    #[test]
    fn apply_context_map_maps_request_field_with_req_prefix() {
        let mut context_map: ContextMap = HashMap::new();
        context_map.insert("customer.id".to_string(), "req.customer.id".to_string());

        let dep_req = json!({"customer": {"id": "cust_from_dep"}});
        let dep_res = json!({});
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({"customer": {"id": "placeholder"}});
        apply_context_map(&collected, &mut req);

        assert_eq!(req["customer"]["id"], json!("cust_from_dep"));
    }

    #[test]
    fn apply_context_map_defaults_to_response_when_no_prefix() {
        let mut context_map: ContextMap = HashMap::new();
        // No "res." prefix — should default to response
        context_map.insert(
            "connector_transaction_id.id".to_string(),
            "connectorTransactionId.id".to_string(),
        );

        let dep_req = json!({});
        let dep_res = json!({"connectorTransactionId": {"id": "txn_abc"}});
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({"connector_transaction_id": {"id": "placeholder"}});
        apply_context_map(&collected, &mut req);

        assert_eq!(req["connector_transaction_id"]["id"], json!("txn_abc"));
    }

    #[test]
    fn apply_context_map_skips_null_source_values() {
        let mut context_map: ContextMap = HashMap::new();
        context_map.insert("field_a".to_string(), "res.missing_field".to_string());

        let dep_req = json!({});
        let dep_res = json!({"other_field": "val"});
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({"field_a": "original"});
        apply_context_map(&collected, &mut req);

        // Should remain unchanged since source doesn't exist
        assert_eq!(req["field_a"], json!("original"));
    }

    #[test]
    fn apply_context_map_multiple_dependencies() {
        let mut map1: ContextMap = HashMap::new();
        map1.insert(
            "state.access_token.token.value".to_string(),
            "res.access_token".to_string(),
        );

        let mut map2: ContextMap = HashMap::new();
        map2.insert("customer.id".to_string(), "res.customer_id".to_string());

        let collected = vec![
            (map1, json!({}), json!({"access_token": "tok_paypal"})),
            (map2, json!({}), json!({"customer_id": "cust_stripe_123"})),
        ];

        let mut req = json!({"amount": {"minor_amount": 500}});
        apply_context_map(&collected, &mut req);

        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("tok_paypal")
        );
        assert_eq!(req["customer"]["id"], json!("cust_stripe_123"));
        assert_eq!(req["amount"]["minor_amount"], json!(500));
    }

    #[test]
    fn apply_context_map_camel_case_response_lookup() {
        let mut context_map: ContextMap = HashMap::new();
        context_map.insert(
            "state.access_token.token_type".to_string(),
            "res.token_type".to_string(),
        );

        let dep_req = json!({});
        // Response uses camelCase (as grpcurl returns proto-JSON)
        let dep_res = json!({"tokenType": "Bearer"});
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({});
        apply_context_map(&collected, &mut req);

        assert_eq!(req["state"]["access_token"]["token_type"], json!("Bearer"));
    }

    #[test]
    fn apply_context_map_empty_map_is_noop() {
        let context_map: ContextMap = HashMap::new();
        let collected = vec![(context_map, json!({"some": "req"}), json!({"some": "res"}))];

        let mut req = json!({"field": "original"});
        apply_context_map(&collected, &mut req);

        assert_eq!(req["field"], json!("original"));
    }

    #[test]
    fn apply_context_map_id_type_id_unwrapping() {
        let mut context_map: ContextMap = HashMap::new();
        context_map.insert(
            "connector_transaction_id.id".to_string(),
            "res.connector_transaction_id.id".to_string(),
        );

        let dep_req = json!({});
        // Response has the id wrapped in id_type.id (proto Identifier pattern)
        let dep_res = json!({
            "connectorTransactionId": {
                "idType": {
                    "id": "pi_3ABC"
                }
            }
        });
        let collected = vec![(context_map, dep_req, dep_res)];

        let mut req = json!({"connector_transaction_id": {"id": "placeholder"}});
        apply_context_map(&collected, &mut req);

        assert_eq!(req["connector_transaction_id"]["id"], json!("pi_3ABC"));
    }

    #[test]
    fn explicit_context_map_overrides_implicit_context_value() {
        let mut req = json!({"state": {"access_token": {"token": {"value": ""}}}});

        let implicit_dep_res = vec![json!({"access_token": "implicit_tok"})];
        add_context(&[], &implicit_dep_res, &mut req);

        let mut context_map: ContextMap = HashMap::new();
        context_map.insert(
            "state.access_token.token.value".to_string(),
            "res.access_token".to_string(),
        );
        let explicit_dep_res = json!({"access_token": "explicit_tok"});
        apply_context_map(&[(context_map, json!({}), explicit_dep_res)], &mut req);

        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("explicit_tok")
        );
    }

    #[test]
    fn all_supported_scenarios_match_proto_schema_for_all_connectors() {
        let connectors =
            discover_all_connectors().expect("connector discovery should work for schema checks");
        assert!(
            !connectors.is_empty(),
            "at least one connector must exist for schema checks"
        );

        let mut failures = Vec::new();

        for connector in &connectors {
            let suites = match load_supported_suites_for_connector(connector) {
                Ok(suites) => suites,
                Err(error) => {
                    failures.push(format!(
                        "{connector}: failed to load supported suites: {error}"
                    ));
                    continue;
                }
            };

            for suite in suites {
                let suite_scenarios = match load_suite_scenarios(&suite) {
                    Ok(file) => file,
                    Err(error) => {
                        failures.push(format!(
                            "{connector}/{suite}: failed to load scenario file: {error}"
                        ));
                        continue;
                    }
                };

                let mut scenario_names = suite_scenarios.keys().cloned().collect::<Vec<_>>();
                scenario_names.sort();

                for scenario in scenario_names {
                    // Skip negative-test scenarios that deliberately use invalid
                    // proto data (e.g. invalid card numbers, unknown enum values)
                    // to trigger connector errors.  These will always fail schema
                    // validation, which is expected.
                    if let Some(def) = suite_scenarios.get(&scenario) {
                        if let Some(FieldAssert::MustExist { must_exist: true }) =
                            def.assert_rules.get("error")
                        {
                            continue;
                        }
                    }

                    let grpc_req = match get_the_grpc_req_for_connector(
                        &suite, &scenario, connector,
                    ) {
                        Ok(req) => req,
                        Err(ScenarioError::Skipped { .. }) => {
                            // Some scenarios may be skipped at build time
                            // (e.g. missing env vars) — this is expected.
                            continue;
                        }
                        Err(error) => {
                            failures.push(format!(
                                "{connector}/{suite}/{scenario}: failed to build effective request: {error}"
                            ));
                            continue;
                        }
                    };

                    // Resolve sentinels (e.g. "connector_name" → "STRIPE")
                    // before schema validation so template placeholders don't
                    // cause spurious proto parse failures.
                    let mut resolved_req = grpc_req;
                    if let Err(error) = resolve_auto_generate(&mut resolved_req, connector) {
                        failures.push(format!(
                            "{connector}/{suite}/{scenario}: sentinel resolution failed: {error}"
                        ));
                        continue;
                    }

                    if let Err(error) =
                        validate_suite_scenario_schema(connector, &suite, &scenario, &resolved_req)
                    {
                        failures.push(error);
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "proto schema compatibility failures:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn all_override_entries_match_existing_scenarios_and_proto_schema() {
        let connectors =
            discover_all_connectors().expect("connector discovery should work for override checks");
        let mut failures = Vec::new();

        for connector in &connectors {
            let override_path = connector_spec_dir(connector).join("override.json");
            if !override_path.is_file() {
                continue;
            }

            let raw = match fs::read_to_string(&override_path) {
                Ok(content) => content,
                Err(error) => {
                    failures.push(format!(
                        "{}: failed to read override file: {error}",
                        override_path.display()
                    ));
                    continue;
                }
            };

            let json: Value = match serde_json::from_str(&raw) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(format!(
                        "{}: failed to parse override JSON: {error}",
                        override_path.display()
                    ));
                    continue;
                }
            };

            let Some(suites_obj) = json.as_object() else {
                failures.push(format!(
                    "{}: override root must be an object keyed by suite",
                    override_path.display()
                ));
                continue;
            };

            for (suite, suite_value) in suites_obj {
                let suite_scenarios = match load_suite_scenarios(suite) {
                    Ok(file) => file,
                    Err(error) => {
                        failures.push(format!(
                            "{connector}/{suite}: override references unknown or invalid suite: {error}"
                        ));
                        continue;
                    }
                };

                let Some(scenario_obj) = suite_value.as_object() else {
                    failures.push(format!(
                        "{connector}/{suite}: override suite entry must be an object keyed by scenario"
                    ));
                    continue;
                };

                for scenario in scenario_obj.keys() {
                    if !suite_scenarios.contains_key(scenario) {
                        failures.push(format!(
                            "{connector}/{suite}/{scenario}: override references missing scenario in suite file"
                        ));
                        continue;
                    }

                    let grpc_req = match get_the_grpc_req_for_connector(suite, scenario, connector)
                    {
                        Ok(req) => req,
                        Err(error) => {
                            failures.push(format!(
                                "{connector}/{suite}/{scenario}: failed to materialize request with override: {error}"
                            ));
                            continue;
                        }
                    };

                    if let Err(error) =
                        validate_suite_scenario_schema(connector, suite, scenario, &grpc_req)
                    {
                        failures.push(error);
                    }
                }
            }
        }

        assert!(
            failures.is_empty(),
            "override schema compatibility failures:\n{}",
            failures.join("\n")
        );
    }
}
