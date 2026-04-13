//! Report persistence and markdown rendering for harness runs.
//!
//! This module appends `ReportEntry` rows into `report.json` and regenerates
//! a markdown report directory (`test_report/`) after each write so the latest
//! execution state is always available in both machine-readable and
//! human-readable formats.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::cred_masking::{mask_json_value, mask_sensitive_text};
use crate::harness::scenario_display_name::generate_style_a_display_name;
use crate::harness::scenario_loader::{
    discover_all_connectors, load_connector_spec, load_suite_scenarios, load_suite_spec,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntry {
    /// Execution timestamp in epoch milliseconds.
    pub run_at_epoch_ms: u128,
    /// Suite name (e.g. `authorize`).
    pub suite: String,
    /// Scenario name inside the suite.
    pub scenario: String,
    /// Optional human-friendly scenario name from scenario definition.
    #[serde(default)]
    pub scenario_display_name: Option<String>,
    /// Connector slug used for execution.
    pub connector: String,
    /// Payment method extracted from request template, when available.
    pub pm: Option<String>,
    /// Payment method type extracted from request template, when available.
    pub pmt: Option<String>,
    /// Endpoint used by execution backend.
    pub endpoint: String,
    /// Marks whether this row is from dependency execution.
    #[serde(default)]
    pub is_dependency: bool,
    /// Assertion outcome (`PASS`/`FAIL`).
    pub assertion_result: String,
    /// Optional response status extracted from response JSON.
    pub response_status: Option<String>,
    /// Optional failure reason / assertion error text.
    pub error: Option<String>,
    /// Dependency chain captured at execution time.
    #[serde(default)]
    pub dependency: Vec<String>,
    /// Effective request payload used for execution.
    pub req_body: Option<Value>,
    /// Parsed response payload captured for reporting/debugging.
    pub res_body: Option<Value>,
    /// Full grpc request trace (command + headers + payload), when available.
    #[serde(default)]
    pub grpc_request: Option<String>,
    /// Full grpc response trace (headers/trailers + body), when available.
    #[serde(default)]
    pub grpc_response: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScenarioRunReport {
    /// Chronological list of all run entries in current report file.
    pub runs: Vec<ReportEntry>,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn report_path() -> PathBuf {
    if let Ok(path) = std::env::var("UCS_RUN_TEST_REPORT_PATH") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("report.json")
}

fn md_path(json_path: &Path) -> PathBuf {
    json_path
        .with_file_name("test_report")
        .join("test_overview.md")
}

fn report_dir_path(json_path: &Path) -> PathBuf {
    json_path.with_file_name("test_report")
}

fn legacy_md_path(json_path: &Path) -> PathBuf {
    json_path.with_file_name("test_report.md")
}

// ---------------------------------------------------------------------------
// Report operations
// ---------------------------------------------------------------------------

/// Resets report artifacts (`report.json` and `test_report/`).
pub fn clear_report() {
    let path = report_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, "{\"runs\":[]}");

    // Also clear markdown report outputs so they stay in sync.
    let report_dir = report_dir_path(&path);
    if report_dir.exists() {
        let _ = fs::remove_dir_all(&report_dir);
    }

    let legacy_md = legacy_md_path(&path);
    if legacy_md.exists() {
        let _ = fs::remove_file(&legacy_md);
    }
}

/// Appends one report entry and regenerates markdown output.
pub fn append_report(entry: ReportEntry) -> Result<(), String> {
    append_report_batch(vec![entry])
}

/// Appends many report entries and regenerates markdown output once.
pub fn append_report_batch(entries: Vec<ReportEntry>) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }

    let path = report_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create report directory '{}': {e}",
                parent.display()
            )
        })?;
    }

    let mut report = if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str::<ScenarioRunReport>(&content).unwrap_or_default(),
            Err(_) => ScenarioRunReport::default(),
        }
    } else {
        ScenarioRunReport::default()
    };

    // Existing entries are assumed to be already sanitized from previous writes.
    // Only sanitize new entries being appended now.
    let sanitized_entries = entries.into_iter().map(|mut entry| {
        sanitize_report_entry_in_place(&mut entry);
        entry
    });
    report.runs.extend(sanitized_entries);

    let serialized = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("failed to serialize report: {e}"))?;
    fs::write(&path, &serialized)
        .map_err(|e| format!("failed to write report '{}': {e}", path.display()))?;

    // Auto-generate markdown after every write
    if let Err(e) = generate_md(&path, &report) {
        tracing::warn!(%e, "failed to generate markdown report");
    }

    Ok(())
}

/// Regenerates markdown artifacts from the report JSON at `json_path`.
pub fn regenerate_markdown_from_path(json_path: &Path) -> Result<PathBuf, String> {
    let content = fs::read_to_string(json_path)
        .map_err(|e| format!("failed to read report '{}': {e}", json_path.display()))?;
    let mut report = serde_json::from_str::<ScenarioRunReport>(&content)
        .map_err(|e| format!("failed to parse report '{}': {e}", json_path.display()))?;

    for run in &mut report.runs {
        sanitize_report_entry_in_place(run);
    }

    generate_md(json_path, &report)?;
    Ok(md_path(json_path))
}

/// Regenerates markdown artifacts from the default report path.
pub fn regenerate_markdown_from_disk() -> Result<PathBuf, String> {
    let path = report_path();
    regenerate_markdown_from_path(&path)
}

/// Best-effort wrapper around `append_report` that logs failures instead of
/// bubbling them.
pub fn append_report_best_effort(entry: ReportEntry) {
    if let Err(e) = append_report(entry) {
        tracing::warn!(%e, "report write failed");
    }
}

/// Best-effort wrapper around `append_report_batch`.
pub fn append_report_batch_best_effort(entries: Vec<ReportEntry>) {
    if let Err(e) = append_report_batch(entries) {
        tracing::warn!(%e, "report batch write failed");
    }
}

// ---------------------------------------------------------------------------
// Helpers shared by binaries
// ---------------------------------------------------------------------------

/// Returns current timestamp in epoch milliseconds.
pub fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Best-effort extraction of payment method and payment method type from a
/// request payload, used by report tables.
pub fn extract_pm_and_pmt(grpc_req: Option<&Value>) -> (Option<String>, Option<String>) {
    let Some(grpc_req) = grpc_req else {
        return (None, None);
    };
    let Some(payment_method_obj) = grpc_req.get("payment_method").and_then(Value::as_object) else {
        return (None, None);
    };
    let Some((pm, pm_value)) = payment_method_obj.iter().next() else {
        return (None, None);
    };

    let pmt = pm_value
        .get("card_type")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            pm_value
                .get("type")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });

    (Some(pm.clone()), pmt)
}

// ---------------------------------------------------------------------------
// Markdown generation
// ---------------------------------------------------------------------------

/// Canonical suite ordering for table columns.
/// Core payment flows come first, then recurring/mandate flows, then auxiliary
/// setup flows (customer, auth/3DS, tokens) at the end.
const SUITE_ORDER: &[&str] = &[
    // Core payment flows
    "authorize",
    "capture",
    "void",
    "refund",
    "get",
    "refund_sync",
    "complete_authorize",
    // Recurring / mandate flows
    "setup_recurring",
    "recurring_charge",
    "revoke_mandate",
    // Auxiliary / setup flows
    "server_authentication_token",
    "create_customer",
    "pre_authenticate",
    "authenticate",
    "post_authenticate",
];

/// Human-readable display names for suite columns.
fn suite_display_name(suite: &str) -> String {
    let name = match suite {
        "authorize" => "Authorize",
        "capture" => "Capture",
        "void" => "Void",
        "refund" => "Refund",
        "get" => "Payment Sync",
        "refund_sync" => "Refund Sync",
        "complete_authorize" => "Complete Auth",
        "setup_recurring" => "Setup Mandate",
        "recurring_charge" => "Mandate Pay",
        "revoke_mandate" => "Revoke Mandate",
        "server_authentication_token" => "Create Token",
        "create_customer" => "Customer",
        "pre_authenticate" => "Pre Auth",
        "authenticate" => "Auth",
        "post_authenticate" => "Post Auth",
        "server_session_authentication_token" => "Session Token",
        "client_authentication_token" => "SDK Session",
        "tokenize_payment_method" => "Tokenize PM",
        "incremental_authorization" => "Incremental Auth",
        "create_order" => "Create Order",
        other => {
            return other
                .split('_')
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    name.to_string()
}

/// Cache for suite -> service name mappings loaded from disk.
/// Initialized once on first access to avoid repeated file I/O.
static SUITE_SERVICE_CACHE: OnceLock<BTreeMap<String, String>> = OnceLock::new();

/// Builds the suite service name cache by reading all suite specs from disk.
fn build_suite_service_cache() -> BTreeMap<String, String> {
    let mut cache = BTreeMap::new();

    // Hardcoded mappings for core suites (always present).
    cache.insert(
        "server_authentication_token".to_string(),
        "MerchantAuthenticationService/CreateServerAuthenticationToken".to_string(),
    );
    cache.insert(
        "create_customer".to_string(),
        "CustomerService/Create".to_string(),
    );
    cache.insert(
        "pre_authenticate".to_string(),
        "PaymentMethodAuthenticationService/PreAuthenticate".to_string(),
    );
    cache.insert(
        "authenticate".to_string(),
        "PaymentMethodAuthenticationService/Authenticate".to_string(),
    );
    cache.insert(
        "post_authenticate".to_string(),
        "PaymentMethodAuthenticationService/PostAuthenticate".to_string(),
    );
    cache.insert(
        "authorize".to_string(),
        "PaymentService/Authorize".to_string(),
    );
    cache.insert(
        "complete_authorize".to_string(),
        "PaymentService/Authorize".to_string(),
    );
    cache.insert("capture".to_string(), "PaymentService/Capture".to_string());
    cache.insert("refund".to_string(), "PaymentService/Refund".to_string());
    cache.insert("void".to_string(), "PaymentService/Void".to_string());
    cache.insert("get".to_string(), "PaymentService/Get".to_string());
    cache.insert("refund_sync".to_string(), "RefundService/Get".to_string());
    cache.insert(
        "setup_recurring".to_string(),
        "PaymentService/SetupRecurring".to_string(),
    );
    cache.insert(
        "recurring_charge".to_string(),
        "RecurringPaymentService/Charge".to_string(),
    );

    // For connector-specific suites, load from suite specs on disk.
    // This is done once at cache initialization rather than per-call.
    if let Ok(all_connectors) = discover_all_connectors() {
        for connector in all_connectors {
            if let Some(spec) = load_connector_spec(&connector) {
                for suite in spec.supported_suites {
                    if !cache.contains_key(&suite) {
                        if let Ok(suite_spec) = load_suite_spec(&suite) {
                            if let Some(method) = suite_spec.grpc_method {
                                let stripped = method.strip_prefix("types.").unwrap_or(&method);
                                cache.insert(suite.clone(), stripped.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    cache
}

fn suite_service_name(suite: &str) -> String {
    let cache = SUITE_SERVICE_CACHE.get_or_init(build_suite_service_cache);
    cache
        .get(suite)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string())
}

fn suite_sort_key(suite: &str) -> usize {
    SUITE_ORDER
        .iter()
        .position(|&s| s == suite)
        .unwrap_or(usize::MAX)
}

fn build_scenario_display_name_map(suites: &[String]) -> BTreeMap<(String, String), String> {
    let mut map = BTreeMap::new();

    for suite in suites {
        if let Ok(scenarios) = load_suite_scenarios(suite) {
            for (scenario_name, scenario_def) in scenarios {
                let display_name = scenario_def
                    .display_name
                    .unwrap_or_else(|| generate_style_a_display_name(suite, &scenario_name));
                map.insert((suite.clone(), scenario_name), display_name);
            }
        }
    }

    map
}

fn resolve_scenario_display_name(
    scenario_display_name_map: &BTreeMap<(String, String), String>,
    suite: &str,
    scenario: &str,
) -> String {
    scenario_display_name_map
        .get(&(suite.to_string(), scenario.to_string()))
        .cloned()
        .unwrap_or_else(|| generate_style_a_display_name(suite, scenario))
}

fn escape_markdown_table_text(value: &str) -> String {
    value.replace('|', "\\|")
}

/// Deduplicated, non-dependency entry keyed by (suite, scenario, connector).
#[derive(Debug, Clone)]
struct MatrixEntry {
    suite: String,
    scenario: String,
    connector: String,
    pm: String,
    pmt: String,
    result: String,
    error: Option<String>,
    response_status: Option<String>,
    run_at: u128,
    run_index: usize,
    dependency: Vec<String>,
    req_body: Option<Value>,
    res_body: Option<Value>,
    grpc_request: Option<String>,
    grpc_response: Option<String>,
}

fn sanitize_report_entry_in_place(entry: &mut ReportEntry) {
    if let Some(error) = entry.error.as_mut() {
        *error = mask_sensitive_text(error);
    }

    if let Some(grpc_request) = entry.grpc_request.as_mut() {
        *grpc_request = mask_sensitive_text(grpc_request);
    }

    if let Some(grpc_response) = entry.grpc_response.as_mut() {
        *grpc_response = mask_sensitive_text(grpc_response);
    }

    if let Some(req_body) = entry.req_body.as_mut() {
        mask_json_value(req_body);
    }

    if let Some(res_body) = entry.res_body.as_mut() {
        mask_json_value(res_body);
    }
}

fn sanitize_anchor(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_hyphen = false;

    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

fn connector_suite_relative_path(connector: &str, suite: &str) -> String {
    format!(
        "./connectors/{}/{}.md",
        sanitize_anchor(connector),
        sanitize_anchor(suite)
    )
}

fn connector_suite_file_path(report_dir: &Path, connector: &str, suite: &str) -> PathBuf {
    report_dir
        .join("connectors")
        .join(sanitize_anchor(connector))
        .join(format!("{}.md", sanitize_anchor(suite)))
}

fn connector_scenario_relative_path_from_suite(suite: &str, scenario: &str) -> String {
    format!(
        "./{}/{}.md",
        sanitize_anchor(suite),
        sanitize_anchor(scenario)
    )
}

fn connector_scenario_file_path(
    report_dir: &Path,
    connector: &str,
    suite: &str,
    scenario: &str,
) -> PathBuf {
    report_dir
        .join("connectors")
        .join(sanitize_anchor(connector))
        .join(sanitize_anchor(suite))
        .join(format!("{}.md", sanitize_anchor(scenario)))
}

fn split_dependency_label(label: &str) -> Option<(&str, &str)> {
    if !label.ends_with(')') {
        return None;
    }

    let open = label.rfind('(')?;
    if open == 0 {
        return None;
    }

    let suite = &label[..open];
    let scenario = &label[(open + 1)..(label.len() - 1)];
    if suite.is_empty() || scenario.is_empty() {
        return None;
    }

    Some((suite, scenario))
}

fn latest_dependency_entry_before(
    report: &ScenarioRunReport,
    main: &MatrixEntry,
    dependency_label: &str,
) -> Option<ReportEntry> {
    let (dep_suite, dep_scenario) = split_dependency_label(dependency_label)?;

    report
        .runs
        .get(..main.run_index)?
        .iter()
        .rev()
        .find(|entry| {
            entry.is_dependency
                && entry.connector == main.connector
                && entry.suite == dep_suite
                && entry.scenario == dep_scenario
        })
        .cloned()
}

fn dependency_chain_summary(report: &ScenarioRunReport, main: &MatrixEntry) -> String {
    if main.dependency.is_empty() {
        return "None".to_string();
    }

    let mut chain = Vec::with_capacity(main.dependency.len());
    for dependency_label in &main.dependency {
        let status = latest_dependency_entry_before(report, main, dependency_label)
            .map(|entry| entry.assertion_result)
            .unwrap_or_else(|| "NOT_FOUND".to_string());
        chain.push(format!("`{dependency_label}` ({status})"));
    }

    chain.join(" -> ")
}

fn generate_md(json_path: &Path, report: &ScenarioRunReport) -> Result<(), String> {
    let report_dir = report_dir_path(json_path);
    fs::create_dir_all(&report_dir).map_err(|e| {
        format!(
            "failed to create report directory '{}': {e}",
            report_dir.display()
        )
    })?;

    let connectors_dir = report_dir.join("connectors");
    fs::create_dir_all(&connectors_dir).map_err(|e| {
        format!(
            "failed to create connector report directory '{}': {e}",
            connectors_dir.display()
        )
    })?;

    // 1. Filter out dependency entries and deduplicate by (suite, scenario, connector).
    //    When duplicates exist, keep the latest by run_at_epoch_ms.
    let mut deduped: BTreeMap<(String, String, String), MatrixEntry> = BTreeMap::new();

    for (run_index, entry) in report.runs.iter().enumerate() {
        if entry.is_dependency {
            continue;
        }

        let key = (
            entry.suite.clone(),
            entry.scenario.clone(),
            entry.connector.clone(),
        );

        let candidate = MatrixEntry {
            suite: entry.suite.clone(),
            scenario: entry.scenario.clone(),
            connector: entry.connector.clone(),
            pm: entry.pm.clone().unwrap_or_else(|| "-".to_string()),
            pmt: entry.pmt.clone().unwrap_or_else(|| "-".to_string()),
            result: entry.assertion_result.clone(),
            error: entry.error.clone(),
            response_status: entry.response_status.clone(),
            run_at: entry.run_at_epoch_ms,
            run_index,
            dependency: entry.dependency.clone(),
            req_body: entry.req_body.clone(),
            res_body: entry.res_body.clone(),
            grpc_request: entry.grpc_request.clone(),
            grpc_response: entry.grpc_response.clone(),
        };

        let should_insert = deduped.get(&key).is_none_or(|existing| {
            candidate.run_at > existing.run_at
                || (candidate.run_at == existing.run_at
                    && candidate.run_index >= existing.run_index)
        });

        if should_insert {
            deduped.insert(key, candidate);
        }
    }

    // 2. Build full connector × suite universe from specs.json files.
    //    Falls back to only tested connectors/suites when discovery fails.
    let all_connectors: Vec<String> = discover_all_connectors().unwrap_or_else(|_| {
        let mut set = BTreeSet::new();
        for entry in deduped.values() {
            set.insert(entry.connector.clone());
        }
        set.into_iter().collect()
    });

    // Map: connector -> set of supported suites (from specs.json)
    let mut connector_supported_suites: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_suites_set = BTreeSet::new();
    for connector in &all_connectors {
        if let Some(spec) = load_connector_spec(connector) {
            let suite_set: BTreeSet<String> = spec.supported_suites.into_iter().collect();
            for suite in &suite_set {
                all_suites_set.insert(suite.clone());
            }
            connector_supported_suites.insert(connector.clone(), suite_set);
        }
    }
    // Also include any suites that appear in test results but not in specs
    for entry in deduped.values() {
        all_suites_set.insert(entry.suite.clone());
    }

    // Sort suites: first by SUITE_ORDER, then by popularity (# connectors supporting), then alphabetically
    let mut all_suites: Vec<String> = all_suites_set.into_iter().collect();
    all_suites.sort_by(|left, right| {
        let left_key = suite_sort_key(left);
        let right_key = suite_sort_key(right);
        left_key
            .cmp(&right_key)
            .then_with(|| {
                // For suites not in SUITE_ORDER, sort by popularity descending
                let left_count = connector_supported_suites
                    .values()
                    .filter(|suites| suites.contains(left))
                    .count();
                let right_count = connector_supported_suites
                    .values()
                    .filter(|suites| suites.contains(right))
                    .count();
                right_count.cmp(&left_count)
            })
            .then_with(|| left.cmp(right))
    });

    let scenario_display_name_map = build_scenario_display_name_map(&all_suites);

    let connector_suite_map: BTreeMap<(String, String), Vec<MatrixEntry>> = {
        let mut map: BTreeMap<(String, String), Vec<MatrixEntry>> = BTreeMap::new();
        for entry in deduped.values() {
            map.entry((entry.connector.clone(), entry.suite.clone()))
                .or_default()
                .push(entry.clone());
        }
        for entries in map.values_mut() {
            entries.sort_by(|left, right| left.scenario.cmp(&right.scenario));
        }
        map
    };

    // Compute tested connectors set for summary
    let tested_connectors: BTreeSet<String> =
        deduped.values().map(|e| e.connector.clone()).collect();

    // 3. Build overview markdown with full matrix.
    let mut md = String::with_capacity(8192);
    md.push_str(
        "# UCS Connector Test Report

",
    );
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    md.push_str(&format!(
        "> Generated: epoch {epoch_secs}

"
    ));

    // Summary statistics
    let total_entries: usize = connector_suite_map.values().map(|v| v.len()).sum();
    let total_passed: usize = connector_suite_map
        .values()
        .flat_map(|v| v.iter())
        .filter(|e| e.result == "PASS")
        .count();
    let total_failed = total_entries.saturating_sub(total_passed);
    md.push_str(&format!(
        "**Summary**: {} connectors discovered, {} tested | {} passed, {} failed across {} scenarios\n\n",
        all_connectors.len(),
        tested_connectors.len(),
        total_passed,
        total_failed,
        total_entries
    ));

    md.push_str(
        "## Connector Flow Matrix

",
    );
    md.push_str(
        "Legend: percentage = tested (links to details), `—` = supported but not yet tested, `-` = not supported\n\n",
    );

    md.push_str("| Connector |");
    for suite in &all_suites {
        md.push_str(&format!(" {} |", suite_display_name(suite)));
    }
    md.push('\n');

    md.push_str("|:----------|");
    for _ in &all_suites {
        md.push_str(":------:|");
    }
    md.push('\n');

    for connector in &all_connectors {
        md.push_str(&format!("| `{}` |", connector));
        let supported = connector_supported_suites.get(connector);
        for suite in &all_suites {
            let key = (connector.clone(), suite.clone());
            if let Some(entries) = connector_suite_map.get(&key) {
                // Tested: show pass rate with link
                let total = entries.len();
                let passed = entries
                    .iter()
                    .filter(|entry| entry.result.as_str() == "PASS")
                    .count();
                let link = connector_suite_relative_path(connector, suite);
                md.push_str(&format!(" [{:.1}%]({}) |", percent(passed, total), link));
            } else if supported.is_some_and(|s| s.contains(suite)) {
                // Supported but not tested
                md.push_str(" — |");
            } else {
                // Not supported
                md.push_str(" - |");
            }
        }
        md.push('\n');
    }

    md.push_str(
        "
> Each percentage links to connector-specific suite results.
",
    );

    // 4. Write overview markdown.
    let out_path = md_path(json_path);
    fs::write(&out_path, &md)
        .map_err(|e| format!("failed to write markdown '{}': {e}", out_path.display()))?;

    // 5. Write connector suite + scenario detail markdown files (only for tested data).
    for ((connector, suite), entries) in &connector_suite_map {
        let suite_path = connector_suite_file_path(&report_dir, connector, suite);
        if let Some(parent) = suite_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "failed to create connector suite directory '{}': {e}",
                    parent.display()
                )
            })?;
        }

        let suite_content = render_connector_suite_markdown(
            report,
            connector,
            suite,
            entries,
            &scenario_display_name_map,
        );
        fs::write(&suite_path, suite_content).map_err(|e| {
            format!(
                "failed to write connector suite markdown '{}': {e}",
                suite_path.display()
            )
        })?;

        for entry in entries {
            let scenario_path =
                connector_scenario_file_path(&report_dir, connector, suite, &entry.scenario);
            if let Some(parent) = scenario_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "failed to create connector scenario directory '{}': {e}",
                        parent.display()
                    )
                })?;
            }

            let scenario_content =
                render_connector_scenario_markdown(report, entry, &scenario_display_name_map);
            fs::write(&scenario_path, scenario_content).map_err(|e| {
                format!(
                    "failed to write connector scenario markdown '{}': {e}",
                    scenario_path.display()
                )
            })?;
        }
    }

    let legacy_md = legacy_md_path(json_path);
    if legacy_md.exists() {
        let _ = fs::remove_file(legacy_md);
    }

    Ok(())
}

fn push_collapsible_code_block(
    md: &mut String,
    summary: &str,
    language: &str,
    content: Option<&str>,
    empty_message: &str,
) {
    md.push_str(
        "<details>
",
    );
    md.push_str(&format!(
        "<summary>{summary}</summary>

"
    ));
    if let Some(content) = content {
        md.push_str(&format!(
            "```{language}
"
        ));
        md.push_str(content);
        md.push_str(
            "
```

",
        );
    } else {
        md.push_str(empty_message);
        md.push_str(
            "

",
        );
    }
    md.push_str(
        "</details>

",
    );
}

fn render_connector_suite_markdown(
    report: &ScenarioRunReport,
    connector: &str,
    suite: &str,
    entries: &[MatrixEntry],
    scenario_display_name_map: &BTreeMap<(String, String), String>,
) -> String {
    let mut md = String::with_capacity(4096);
    let total = entries.len();
    let passed = entries
        .iter()
        .filter(|entry| entry.result.as_str() == "PASS")
        .count();
    md.push_str(&format!(
        "# Connector `{connector}` / Suite `{suite}`

",
    ));
    md.push_str(&format!(
        "- Service: `{}`
",
        suite_service_name(suite)
    ));
    md.push_str(&format!(
        "- Pass Rate: `{:.1}%` (`{}` / `{}`)

",
        percent(passed, total),
        passed,
        total
    ));
    md.push_str("[Back to Overview](../../test_overview.md)\n\n");

    md.push_str("## Scenario Matrix\n\n");
    md.push_str(
        "| Scenario | PM | PMT | Result | Prerequisites |
",
    );
    md.push_str(
        "|:---------|:--:|:---:|:------:|:--------------|
",
    );
    for entry in entries {
        let scenario_link = connector_scenario_relative_path_from_suite(suite, &entry.scenario);
        let scenario_display_name =
            resolve_scenario_display_name(scenario_display_name_map, suite, &entry.scenario);
        let scenario_display_name_for_table = escape_markdown_table_text(&scenario_display_name);
        let prerequisites = dependency_chain_summary(report, entry);
        md.push_str(&format!(
            "| [`{}`]({}) | {} | {} | `{}` | {} |\n",
            scenario_display_name_for_table,
            scenario_link,
            entry.pm,
            entry.pmt,
            entry.result,
            prerequisites
        ));
    }

    let failed_entries: Vec<&MatrixEntry> = entries
        .iter()
        .filter(|entry| entry.result.as_str() != "PASS")
        .collect();
    if !failed_entries.is_empty() {
        md.push_str("\n## Failed Scenarios\n\n");
        for entry in failed_entries {
            let scenario_link = connector_scenario_relative_path_from_suite(suite, &entry.scenario);
            let scenario_display_name =
                resolve_scenario_display_name(scenario_display_name_map, suite, &entry.scenario);
            if let Some(error) = entry.error.as_deref() {
                let summary = error.lines().next().unwrap_or("Unknown failure");
                md.push_str(&format!(
                    "- [`{}`]({}) — {}\n",
                    scenario_display_name, scenario_link, summary
                ));
            } else {
                md.push_str(&format!(
                    "- [`{}`]({})\n",
                    scenario_display_name, scenario_link
                ));
            }
        }
    }

    md
}

fn render_connector_scenario_markdown(
    report: &ScenarioRunReport,
    entry: &MatrixEntry,
    scenario_display_name_map: &BTreeMap<(String, String), String>,
) -> String {
    let mut md = String::with_capacity(4096);
    let scenario_display_name =
        resolve_scenario_display_name(scenario_display_name_map, &entry.suite, &entry.scenario);
    md.push_str(&format!(
        "# Connector `{}` / Suite `{}` / Scenario `{}`

",
        entry.connector, entry.suite, scenario_display_name
    ));
    md.push_str(&format!(
        "- Service: `{}`
",
        suite_service_name(&entry.suite)
    ));
    md.push_str(&format!("- Scenario Key: `{}`\n", entry.scenario));
    md.push_str(&format!("- PM / PMT: `{}` / `{}`\n", entry.pm, entry.pmt));
    md.push_str(&format!("- Result: `{}`\n", entry.result));
    if let Some(status) = &entry.response_status {
        md.push_str(&format!("- Response Status: `{status}`\n"));
    }
    md.push('\n');
    if let Some(error) = &entry.error {
        md.push_str("**Error**\n\n");
        md.push_str("```text\n");
        md.push_str(error);
        md.push_str("\n```\n\n");
    }

    md.push_str("**Pre Requisites Executed**\n\n");
    if entry.dependency.is_empty() {
        md.push_str("- None\n");
    } else {
        for (index, dependency_label) in entry.dependency.iter().enumerate() {
            if let Some(dep_entry) = latest_dependency_entry_before(report, entry, dependency_label)
            {
                md.push_str("<details>\n");
                md.push_str(&format!(
                    "<summary>{}. {} — {}</summary>\n\n",
                    index + 1,
                    dependency_label,
                    dep_entry.assertion_result
                ));
                if let Some(dep_error) = dep_entry.error.as_deref() {
                    md.push_str("**Dependency Error**\n\n");
                    md.push_str("```text\n");
                    md.push_str(dep_error);
                    md.push_str("\n```\n\n");
                }

                let dep_entry_req_body = dep_entry
                    .req_body
                    .as_ref()
                    .and_then(|value| serde_json::to_string_pretty(value).ok());
                let dep_entry_res_body = dep_entry
                    .res_body
                    .as_ref()
                    .and_then(|value| serde_json::to_string_pretty(value).ok());

                push_collapsible_code_block(
                    &mut md,
                    "Show Dependency Request (masked)",
                    if dep_entry.grpc_request.is_some() {
                        "bash"
                    } else {
                        "json"
                    },
                    dep_entry
                        .grpc_request
                        .as_deref()
                        .or(dep_entry_req_body.as_deref()),
                    "_Request trace not available._",
                );
                push_collapsible_code_block(
                    &mut md,
                    "Show Dependency Response (masked)",
                    if dep_entry.grpc_response.is_some() {
                        "text"
                    } else {
                        "json"
                    },
                    dep_entry
                        .grpc_response
                        .as_deref()
                        .or(dep_entry_res_body.as_deref()),
                    "_Response trace not available._",
                );
                md.push_str("</details>\n");
            } else {
                md.push_str(&format!(
                    "- {}. {} — NOT_FOUND\n",
                    index + 1,
                    dependency_label
                ));
            }
        }
    }

    let entry_req_body = entry
        .req_body
        .as_ref()
        .and_then(|value| serde_json::to_string_pretty(value).ok());
    let entry_res_body = entry
        .res_body
        .as_ref()
        .and_then(|value| serde_json::to_string_pretty(value).ok());

    push_collapsible_code_block(
        &mut md,
        "Show Request (masked)",
        if entry.grpc_request.is_some() {
            "bash"
        } else {
            "json"
        },
        entry.grpc_request.as_deref().or(entry_req_body.as_deref()),
        "_Request trace not available._",
    );
    push_collapsible_code_block(
        &mut md,
        "Show Response (masked)",
        if entry.grpc_response.is_some() {
            "text"
        } else {
            "json"
        },
        entry.grpc_response.as_deref().or(entry_res_body.as_deref()),
        "_Response trace not available._",
    );

    md.push_str(&format!(
        "\n[Back to Connector Suite](../{}.md) | [Back to Overview](../../../test_overview.md)\n",
        sanitize_anchor(&entry.suite)
    ));

    md
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }

    let safe_num = u32::try_from(numerator).unwrap_or(u32::MAX);
    let safe_den = u32::try_from(denominator).unwrap_or(u32::MAX);

    (f64::from(safe_num) / f64::from(safe_den)) * 100.0
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::harness::cred_masking::MASKED_VALUE;

    #[test]
    fn extract_pm_and_pmt_from_card_request() {
        let req = serde_json::json!({
            "payment_method": {
                "card": {
                    "card_type": "credit",
                    "card_number": {"value": "4111111111111111"}
                }
            }
        });
        let (pm, pmt) = extract_pm_and_pmt(Some(&req));
        assert_eq!(pm.as_deref(), Some("card"));
        assert_eq!(pmt.as_deref(), Some("credit"));
    }

    #[test]
    fn extract_pm_and_pmt_missing() {
        let req = serde_json::json!({"amount": 1000});
        let (pm, pmt) = extract_pm_and_pmt(Some(&req));
        assert!(pm.is_none());
        assert!(pmt.is_none());
    }

    #[test]
    fn suite_ordering_is_consistent() {
        // Core payment flows come first
        assert!(suite_sort_key("authorize") < suite_sort_key("capture"));
        assert!(suite_sort_key("capture") < suite_sort_key("refund"));
        assert!(suite_sort_key("refund") < suite_sort_key("get"));
        assert!(suite_sort_key("get") < suite_sort_key("refund_sync"));
        // Recurring flows come next
        assert!(suite_sort_key("refund_sync") < suite_sort_key("setup_recurring"));
        assert!(suite_sort_key("setup_recurring") < suite_sort_key("recurring_charge"));
        // Auxiliary flows come last
        assert!(suite_sort_key("recurring_charge") < suite_sort_key("server_authentication_token"));
        assert!(suite_sort_key("server_authentication_token") < suite_sort_key("create_customer"));
        assert!(suite_sort_key("create_customer") < suite_sort_key("pre_authenticate"));
        assert!(suite_sort_key("pre_authenticate") < suite_sort_key("authenticate"));
        assert!(suite_sort_key("authenticate") < suite_sort_key("post_authenticate"));
    }

    #[test]
    fn generated_markdown_uses_plain_status_without_badges() {
        let temp_root = std::env::temp_dir().join(format!("ucs-report-{}", now_epoch_ms()));
        fs::create_dir_all(&temp_root).expect("temp dir should be creatable");
        let json_path = temp_root.join("report.json");

        let report = ScenarioRunReport {
            runs: vec![
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "authorize".to_string(),
                    scenario: "no3ds_auto_capture_credit_card".to_string(),
                    scenario_display_name: None,
                    connector: "stripe".to_string(),
                    pm: Some("card".to_string()),
                    pmt: Some("credit".to_string()),
                    endpoint: "localhost:8000".to_string(),
                    is_dependency: false,
                    assertion_result: "PASS".to_string(),
                    response_status: None,
                    error: None,
                    dependency: vec![],
                    req_body: Some(serde_json::json!({"field": "value"})),
                    res_body: Some(serde_json::json!({"status": "CHARGED"})),
                    grpc_request: None,
                    grpc_response: None,
                },
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "create_customer".to_string(),
                    scenario: "create_customer".to_string(),
                    scenario_display_name: None,
                    connector: "paypal".to_string(),
                    pm: None,
                    pmt: None,
                    endpoint: "localhost:8000".to_string(),
                    is_dependency: true,
                    assertion_result: "PASS".to_string(),
                    response_status: None,
                    error: None,
                    dependency: vec![],
                    req_body: Some(serde_json::json!({"dep_req": "value"})),
                    res_body: Some(serde_json::json!({"dep_res": "ok"})),
                    grpc_request: None,
                    grpc_response: None,
                },
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "authorize".to_string(),
                    scenario: "no3ds_auto_capture_credit_card".to_string(),
                    scenario_display_name: None,
                    connector: "paypal".to_string(),
                    pm: Some("card".to_string()),
                    pmt: Some("credit".to_string()),
                    endpoint: "localhost:8000".to_string(),
                    is_dependency: false,
                    assertion_result: "FAIL".to_string(),
                    response_status: None,
                    error: Some("forced failure".to_string()),
                    dependency: vec!["create_customer(create_customer)".to_string()],
                    req_body: Some(serde_json::json!({"field": "value"})),
                    res_body: Some(serde_json::json!({"error": "forced failure"})),
                    grpc_request: None,
                    grpc_response: None,
                },
            ],
        };

        generate_md(&json_path, &report).expect("markdown generation should succeed");

        let overview_path = md_path(&json_path);
        let content =
            fs::read_to_string(&overview_path).expect("generated markdown should be readable");

        assert!(!content.contains("img.shields.io"));
        assert!(!content.contains("![Result]"));
        assert!(!content.contains("![Pass Rate]"));
        assert!(!content.contains("![Passed]"));
        assert!(!content.contains("![Failed]"));

        assert!(content.contains("## Connector Flow Matrix"));
        assert!(!content.contains("## Scenario Performance Matrix"));
        assert!(!content.contains("## Test Matrix"));
        assert!(!content.contains("## Summary"));
        assert!(!content.contains("## Scenario Details"));
        assert!(content.contains("[100.0%](./connectors/stripe/authorize.md)"));
        assert!(content.contains("[0.0%](./connectors/paypal/authorize.md)"));

        let stripe_suite_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("stripe")
            .join("authorize.md");
        let stripe_suite_detail_content = fs::read_to_string(&stripe_suite_detail)
            .expect("suite detail markdown should be readable");
        assert!(stripe_suite_detail_content.contains("# Connector `stripe` / Suite `authorize`"));
        assert!(stripe_suite_detail_content.contains(
            "[`Credit Card \\| No 3DS \\| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md)"
        ));

        let stripe_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("stripe")
            .join("authorize")
            .join("no3ds-auto-capture-credit-card.md");
        let stripe_detail_content =
            fs::read_to_string(&stripe_detail).expect("detail markdown should be readable");
        assert!(stripe_detail_content.contains(
            "# Connector `stripe` / Suite `authorize` / Scenario `Credit Card | No 3DS | Automatic Capture`"
        ));
        assert!(stripe_detail_content.contains("- Scenario Key: `no3ds_auto_capture_credit_card`"));
        assert!(stripe_detail_content.contains("<summary>Show Request (masked)</summary>"));
        assert!(stripe_detail_content.contains("<summary>Show Response (masked)</summary>"));
        assert!(!stripe_detail_content.contains("Show gRPC Request (masked)"));
        assert!(!stripe_detail_content.contains("Show gRPC Response (masked)"));
        assert!(stripe_detail_content.contains("\"field\": \"value\""));
        assert!(stripe_detail_content.contains("\"status\": \"CHARGED\""));
        assert!(stripe_detail_content.contains(
            "[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)"
        ));

        let paypal_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("paypal")
            .join("authorize")
            .join("no3ds-auto-capture-credit-card.md");
        let paypal_detail_content =
            fs::read_to_string(&paypal_detail).expect("paypal detail markdown should be readable");
        assert!(
            paypal_detail_content.contains("<summary>Show Dependency Request (masked)</summary>")
        );
        assert!(
            paypal_detail_content.contains("<summary>Show Dependency Response (masked)</summary>")
        );
        assert!(paypal_detail_content.contains("\"dep_req\": \"value\""));
        assert!(paypal_detail_content.contains("\"dep_res\": \"ok\""));

        let paypal_suite_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("paypal")
            .join("authorize.md");
        let paypal_suite_detail_content = fs::read_to_string(&paypal_suite_detail)
            .expect("paypal suite detail should be readable");
        assert!(paypal_suite_detail_content.contains("## Failed Scenarios"));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn sanitization_masks_sensitive_grpc_trace_and_json_fields() {
        let mut entry = ReportEntry {
            run_at_epoch_ms: now_epoch_ms(),
            suite: "authorize".to_string(),
            scenario: "no3ds_auto_capture_credit_card".to_string(),
            scenario_display_name: None,
            connector: "stripe".to_string(),
            pm: Some("card".to_string()),
            pmt: Some("credit".to_string()),
            endpoint: "localhost:50051".to_string(),
            is_dependency: false,
            assertion_result: "PASS".to_string(),
            response_status: None,
            error: Some("Authorization: Bearer token123".to_string()),
            dependency: vec![],
            req_body: Some(serde_json::json!({
                "api_key": "sk_test_123",
                "payment_method": {
                    "card": {
                        "card_number": {"value": "4111111111111111"},
                        "card_cvc": "123"
                    }
                }
            })),
            res_body: Some(serde_json::json!({
                "access_token": "access_token_value"
            })),
            grpc_request: Some(
                "grpcurl -plaintext \\\n+  -H \"x-api-key: sk_test_123\" \\\n+  -H \"authorization: Bearer token123\" \\\n+  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'"
                    .to_string(),
            ),
            grpc_response: Some(
                "Response headers received:\nauthorization: Bearer token123\nx-api-key: sk_test_123"
                    .to_string(),
            ),
        };

        sanitize_report_entry_in_place(&mut entry);

        let grpc_request = entry.grpc_request.expect("grpc request should exist");
        let grpc_response = entry.grpc_response.expect("grpc response should exist");
        let error = entry.error.expect("error should exist");

        assert!(!grpc_request.contains("sk_test_123"));
        assert!(!grpc_request.contains("token123"));
        assert!(!grpc_response.contains("sk_test_123"));
        assert!(!grpc_response.contains("token123"));
        assert!(!error.contains("token123"));
        assert!(grpc_request.contains(MASKED_VALUE));
        assert!(grpc_response.contains(MASKED_VALUE));
        assert!(error.contains(MASKED_VALUE));

        let req_body = entry.req_body.expect("request body should exist");
        let res_body = entry.res_body.expect("response body should exist");

        assert_eq!(
            req_body.get("api_key").and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
        assert_eq!(
            req_body
                .pointer("/payment_method/card/card_number")
                .and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
        assert_eq!(
            req_body
                .pointer("/payment_method/card/card_cvc")
                .and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
        assert_eq!(
            res_body.get("access_token").and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
    }

    #[test]
    fn bearer_masking_is_idempotent_and_masks_multiple_tokens() {
        let line = "authorization: Bearer abc123 Bearer ***MASKED*** Bearer def456";
        let masked_once = mask_sensitive_text(line);
        let masked_twice = mask_sensitive_text(&masked_once);

        assert_eq!(masked_once, masked_twice);
        assert!(!masked_once.contains("abc123"));
        assert!(!masked_once.contains("def456"));
    }
}
