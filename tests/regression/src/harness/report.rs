//! Report persistence and markdown rendering for harness runs.
//!
//! This module appends `ReportEntry` rows into `report.json` and regenerates
//! a markdown report directory (`test_report/`) after each write so the latest
//! execution state is always available in both machine-readable and
//! human-readable formats.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::scenario_display_name::generate_style_a_display_name;
use crate::harness::scenario_loader::{
    discover_all_connectors, load_connector_spec, load_suite_scenarios, load_suite_spec,
};

const REPORT_MODE_ENV: &str = "UCS_RUN_TEST_REPORT_MODE";
const AUDIT_LOG_ENV: &str = "UCS_AUDIT_LOG_PATH";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportMode {
    Summary,
    LocalDebug,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntry {
    /// Execution timestamp in epoch milliseconds.
    pub run_at_epoch_ms: u128,
    /// Suite name (e.g. `PaymentService/Authorize`).
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
    /// Transient effective request payload; always discarded before persistence.
    pub req_body: Option<Value>,
    /// Transient parsed response payload; always discarded before persistence.
    pub res_body: Option<Value>,
    /// Transient grpcurl command used only to obtain the invocation request ID.
    #[serde(default)]
    pub grpc_request: Option<String>,
    /// Transient verbose gRPC response; always discarded before persistence.
    #[serde(default)]
    pub grpc_response: Option<String>,
    /// Typed-and-masked UCS events captured for this exact invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audit_events: Vec<Value>,
    /// Safe explanation when local audit evidence could not be collected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_error: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScenarioRunReport {
    /// Chronological list of all run entries in current report file.
    pub runs: Vec<ReportEntry>,
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

fn report_mode() -> ReportMode {
    match std::env::var(REPORT_MODE_ENV).as_deref() {
        Ok("local-debug") => ReportMode::LocalDebug,
        _ => ReportMode::Summary,
    }
}

/// Selects the local-only audit report. Call during CLI initialization, before
/// execution starts or any worker threads are created.
pub fn enable_local_debug_report() {
    std::env::set_var(REPORT_MODE_ENV, "local-debug");
}

fn report_mode_for_path(path: &Path) -> ReportMode {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains("local-debug-report"))
    {
        ReportMode::LocalDebug
    } else {
        report_mode()
    }
}

fn report_directory_name(path: &Path) -> &'static str {
    if report_mode_for_path(path) == ReportMode::LocalDebug {
        "local_debug_report"
    } else {
        "test_report"
    }
}

pub fn report_path() -> PathBuf {
    if report_mode() == ReportMode::LocalDebug {
        if let Ok(path) = std::env::var("UCS_LOCAL_DEBUG_REPORT_PATH") {
            return PathBuf::from(path);
        }
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("local-debug-report.json");
    }
    if let Ok(path) = std::env::var("UCS_RUN_TEST_REPORT_PATH") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("report.json")
}

fn md_path(json_path: &Path) -> PathBuf {
    json_path
        .with_file_name(report_directory_name(json_path))
        .join("test_overview.md")
}

fn report_dir_path(json_path: &Path) -> PathBuf {
    json_path.with_file_name(report_directory_name(json_path))
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
    let _ = write_report_json(&path, "{\"runs\":[]}", report_mode());

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

    let mode = report_mode();
    // Strip legacy raw payloads every time a report is touched. Existing audit
    // evidence is preserved only in the explicitly local report.
    for entry in &mut report.runs {
        prepare_report_entry(entry, mode, false);
    }
    report.runs.extend(entries.into_iter().map(|mut entry| {
        prepare_report_entry(&mut entry, mode, true);
        entry
    }));

    let serialized = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("failed to serialize report: {e}"))?;
    write_report_json(&path, &serialized, mode)?;

    // Auto-generate markdown after every write
    if let Err(e) = generate_md(&path, &report) {
        tracing::warn!(%e, "failed to generate markdown report");
    }
    harden_local_debug_artifacts(&path, mode)?;

    Ok(())
}

/// Regenerates markdown artifacts from the report JSON at `json_path`.
pub fn regenerate_markdown_from_path(json_path: &Path) -> Result<PathBuf, String> {
    let content = fs::read_to_string(json_path)
        .map_err(|e| format!("failed to read report '{}': {e}", json_path.display()))?;
    let mut report = serde_json::from_str::<ScenarioRunReport>(&content)
        .map_err(|e| format!("failed to parse report '{}': {e}", json_path.display()))?;

    let mode = report_mode_for_path(json_path);
    for run in &mut report.runs {
        prepare_report_entry(run, mode, false);
    }

    generate_md(json_path, &report)?;
    harden_local_debug_artifacts(json_path, mode)?;
    Ok(md_path(json_path))
}

fn write_report_json(path: &Path, content: &str, mode: ReportMode) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    if mode == ReportMode::LocalDebug {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to write report '{}': {error}", path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("failed to write report '{}': {error}", path.display()))?;

    #[cfg(unix)]
    if mode == ReportMode::LocalDebug {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!(
                "failed to make local debug report private '{}': {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn harden_local_debug_artifacts(path: &Path, mode: ReportMode) -> Result<(), String> {
    if mode != ReportMode::LocalDebug {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let report_dir = report_dir_path(path);
        if report_dir.exists() {
            harden_directory_tree(&report_dir)?;
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            format!(
                "failed to make local debug report private '{}': {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn harden_directory_tree(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    for entry in fs::read_dir(path)
        .map_err(|error| format!("failed to inspect '{}': {error}", path.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to inspect report entry: {error}"))?;
        let child = entry.path();
        if child.is_dir() {
            harden_directory_tree(&child)?;
            fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).map_err(|error| {
                format!("failed to make '{}' private: {error}", child.display())
            })?;
        } else {
            fs::set_permissions(&child, fs::Permissions::from_mode(0o600)).map_err(|error| {
                format!("failed to make '{}' private: {error}", child.display())
            })?;
        }
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to make '{}' private: {error}", path.display()))
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
    "PaymentService/Authorize",
    "PaymentService/Capture",
    "PaymentService/Void",
    "PaymentService/Refund",
    "PaymentService/Get",
    "RefundService/Get",
    "PaymentService/CompleteAuthorize",
    // Recurring / mandate flows
    "PaymentService/SetupRecurring",
    "RecurringPaymentService/Charge",
    "RecurringPaymentService/Revoke",
    // Auxiliary / setup flows
    "MerchantAuthenticationService/CreateServerAuthenticationToken",
    "CustomerService/Create",
    "PaymentMethodAuthenticationService/PreAuthenticate",
    "PaymentMethodAuthenticationService/Authenticate",
    "PaymentMethodAuthenticationService/PostAuthenticate",
];

/// Human-readable display names for suite columns.
fn suite_display_name(suite: &str) -> String {
    let name = match suite {
        "PaymentService/Authorize" => "Authorize",
        "PaymentService/Capture" => "Capture",
        "PaymentService/Void" => "Void",
        "PaymentService/Refund" => "Refund",
        "PaymentService/Get" => "Payment Sync",
        "RefundService/Get" => "Refund Sync",
        "PaymentService/CompleteAuthorize" => "Complete Auth",
        "PaymentService/SetupRecurring" => "Setup Mandate",
        "RecurringPaymentService/Charge" => "Mandate Pay",
        "RecurringPaymentService/Revoke" => "Revoke Mandate",
        "MerchantAuthenticationService/CreateServerAuthenticationToken" => "Create Token",
        "CustomerService/Create" => "Customer",
        "PaymentMethodAuthenticationService/PreAuthenticate" => "Pre Auth",
        "PaymentMethodAuthenticationService/Authenticate" => "Auth",
        "PaymentMethodAuthenticationService/PostAuthenticate" => "Post Auth",
        "MerchantAuthenticationService/CreateServerSessionAuthenticationToken" => "Session Token",
        "MerchantAuthenticationService/CreateClientAuthenticationToken" => "SDK Session",
        "PaymentMethodService/Tokenize" => "Tokenize PM",
        "PaymentService/IncrementalAuthorization" => "Incremental Auth",
        "PaymentService/CreateOrder" => "Create Order",
        other => {
            // Suite identifiers are "ServiceName/MethodName". Extract the method
            // part and split CamelCase words for a readable fallback label.
            let method = other.split('/').nth(1).unwrap_or(other);
            return method.chars().fold(String::new(), |mut acc, c| {
                if c.is_uppercase() && !acc.is_empty() {
                    acc.push(' ');
                }
                acc.push(c);
                acc
            });
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
        "MerchantAuthenticationService/CreateServerAuthenticationToken".to_string(),
        "MerchantAuthenticationService/CreateServerAuthenticationToken".to_string(),
    );
    cache.insert(
        "CustomerService/Create".to_string(),
        "CustomerService/Create".to_string(),
    );
    cache.insert(
        "PaymentMethodAuthenticationService/PreAuthenticate".to_string(),
        "PaymentMethodAuthenticationService/PreAuthenticate".to_string(),
    );
    cache.insert(
        "PaymentMethodAuthenticationService/Authenticate".to_string(),
        "PaymentMethodAuthenticationService/Authenticate".to_string(),
    );
    cache.insert(
        "PaymentMethodAuthenticationService/PostAuthenticate".to_string(),
        "PaymentMethodAuthenticationService/PostAuthenticate".to_string(),
    );
    cache.insert(
        "PaymentService/Authorize".to_string(),
        "PaymentService/Authorize".to_string(),
    );
    cache.insert(
        "PaymentService/CompleteAuthorize".to_string(),
        "PaymentService/Authorize".to_string(),
    );
    cache.insert(
        "PaymentService/Capture".to_string(),
        "PaymentService/Capture".to_string(),
    );
    cache.insert(
        "PaymentService/Refund".to_string(),
        "PaymentService/Refund".to_string(),
    );
    cache.insert(
        "PaymentService/Void".to_string(),
        "PaymentService/Void".to_string(),
    );
    cache.insert(
        "PaymentService/Get".to_string(),
        "PaymentService/Get".to_string(),
    );
    cache.insert(
        "RefundService/Get".to_string(),
        "RefundService/Get".to_string(),
    );
    cache.insert(
        "PaymentService/SetupRecurring".to_string(),
        "PaymentService/SetupRecurring".to_string(),
    );
    cache.insert(
        "RecurringPaymentService/Charge".to_string(),
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
    audit_events: Vec<Value>,
    audit_error: Option<String>,
}

fn prepare_report_entry(entry: &mut ReportEntry, mode: ReportMode, collect_audit: bool) {
    let audit_request_id = entry
        .grpc_request
        .as_deref()
        .and_then(request_id_from_grpcurl_command);
    // Raw grpcurl material has no persistence path. It remains in memory only
    // long enough for assertions and console-safe error extraction.
    entry.req_body = None;
    entry.res_body = None;
    entry.grpc_request = None;
    entry.grpc_response = None;
    entry.error = safe_error_category(entry);

    match mode {
        ReportMode::Summary => {
            entry.audit_events.clear();
            entry.audit_error = None;
        }
        ReportMode::LocalDebug if collect_audit => match audit_request_id {
            Some(request_id) => match collect_audit_events(entry, &request_id) {
                Ok(events) => {
                    entry.audit_events = events;
                    entry.audit_error = None;
                }
                Err(reason) => {
                    entry.audit_events.clear();
                    entry.audit_error = Some(reason);
                }
            },
            None => {
                entry.audit_events.clear();
                entry.audit_error = Some(
                    "masked UCS audit events unavailable: invocation request ID was not captured"
                        .to_string(),
                );
            }
        },
        ReportMode::LocalDebug => {}
    }
}

fn safe_error_category(entry: &ReportEntry) -> Option<String> {
    match entry.assertion_result.as_str() {
        "PASS" => None,
        "SKIP" => Some("scenario_skipped".to_string()),
        _ => Some("scenario_failed".to_string()),
    }
}

fn request_id_from_grpcurl_command(command: &str) -> Option<String> {
    command.lines().find_map(|line| {
        let (_, value) = line.split_once("x-request-id:")?;
        let value = value.trim().trim_end_matches('\\').trim().trim_matches('"');
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn collect_audit_events(entry: &ReportEntry, request_id: &str) -> Result<Vec<Value>, String> {
    let path = std::env::var(AUDIT_LOG_ENV)
        .map_err(|_| format!("masked UCS audit events unavailable: {AUDIT_LOG_ENV} is not set"))?;
    let mut last_error = "masked UCS audit events unavailable for this invocation".to_string();
    for _ in 0..10 {
        match collect_audit_events_once(entry, request_id, Path::new(&path)) {
            Ok(events) => return Ok(events),
            Err(reason) => last_error = reason,
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Err(last_error)
}

fn collect_audit_events_once(
    entry: &ReportEntry,
    request_id: &str,
    path: &Path,
) -> Result<Vec<Value>, String> {
    let lines = fs::read_to_string(path)
        .map_err(|_| "masked UCS audit events unavailable: audit log cannot be read".to_string())?;
    let mut events = lines
        .lines()
        .filter_map(extract_processed_event)
        .filter(|event| {
            event.get("request_id").and_then(Value::as_str) == Some(request_id)
                && event.get("connector").and_then(Value::as_str) == Some(entry.connector.as_str())
                && matches!(
                    event.get("stage").and_then(Value::as_str),
                    Some("GrpcRequest" | "ConnectorCall")
                )
        })
        .collect::<Vec<_>>();
    events.sort_by_key(event_timestamp);

    let grpc_event_indexes = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (event.get("stage").and_then(Value::as_str) == Some("GrpcRequest")).then_some(index)
        })
        .collect::<Vec<_>>();
    let Some(&last_grpc_index) = grpc_event_indexes.last() else {
        return Err("masked UCS audit events unavailable for this invocation".to_string());
    };
    let invocation_start = grpc_event_indexes
        .iter()
        .rev()
        .nth(1)
        .map_or(0, |index| index + 1);
    let invocation = events
        .into_iter()
        .skip(invocation_start)
        .take(last_grpc_index.saturating_sub(invocation_start) + 1)
        .collect::<Vec<_>>();

    if invocation.is_empty() {
        Err("masked UCS audit events unavailable for this invocation".to_string())
    } else {
        Ok(invocation)
    }
}

fn event_timestamp(event: &Value) -> i64 {
    event.get("timestamp").and_then(Value::as_i64).unwrap_or(0)
}

fn extract_processed_event(line: &str) -> Option<Value> {
    let record: Value = serde_json::from_str(line).ok()?;
    let payload = if let Some(payload) = record.get("ucs_event").and_then(Value::as_str) {
        payload
    } else {
        if record.get("target").and_then(Value::as_str) != Some("ucs_common_utils::events") {
            return None;
        }
        let message = record.get("message").and_then(Value::as_str)?;
        let marker = " - Event JSON: ";
        message.split_once(marker)?.1
    };
    serde_json::from_str(payload).ok()
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
            audit_events: entry.audit_events.clone(),
            audit_error: entry.audit_error.clone(),
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

                let dependency_audit = serde_json::to_string_pretty(&dep_entry.audit_events).ok();
                push_collapsible_code_block(
                    &mut md,
                    "Show Dependency UCS Audit Events (typed and masked)",
                    "json",
                    dependency_audit
                        .as_deref()
                        .filter(|_| !dep_entry.audit_events.is_empty()),
                    dep_entry.audit_error.as_deref().unwrap_or(
                        "_Masked audit evidence was not requested for this summary report._",
                    ),
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

    let audit_events = serde_json::to_string_pretty(&entry.audit_events).ok();
    push_collapsible_code_block(
        &mut md,
        "Show UCS Audit Events (typed and masked)",
        "json",
        audit_events
            .as_deref()
            .filter(|_| !entry.audit_events.is_empty()),
        entry
            .audit_error
            .as_deref()
            .unwrap_or("_Masked audit evidence was not requested for this summary report._"),
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
        assert!(
            suite_sort_key("PaymentService/Authorize") < suite_sort_key("PaymentService/Capture")
        );
        assert!(suite_sort_key("PaymentService/Capture") < suite_sort_key("PaymentService/Refund"));
        assert!(suite_sort_key("PaymentService/Refund") < suite_sort_key("PaymentService/Get"));
        assert!(suite_sort_key("PaymentService/Get") < suite_sort_key("RefundService/Get"));
        // Recurring flows come next
        assert!(
            suite_sort_key("RefundService/Get") < suite_sort_key("PaymentService/SetupRecurring")
        );
        assert!(
            suite_sort_key("PaymentService/SetupRecurring")
                < suite_sort_key("RecurringPaymentService/Charge")
        );
        // Auxiliary flows come last
        assert!(
            suite_sort_key("RecurringPaymentService/Charge")
                < suite_sort_key("MerchantAuthenticationService/CreateServerAuthenticationToken")
        );
        assert!(
            suite_sort_key("MerchantAuthenticationService/CreateServerAuthenticationToken")
                < suite_sort_key("CustomerService/Create")
        );
        assert!(
            suite_sort_key("CustomerService/Create")
                < suite_sort_key("PaymentMethodAuthenticationService/PreAuthenticate")
        );
        assert!(
            suite_sort_key("PaymentMethodAuthenticationService/PreAuthenticate")
                < suite_sort_key("PaymentMethodAuthenticationService/Authenticate")
        );
        assert!(
            suite_sort_key("PaymentMethodAuthenticationService/Authenticate")
                < suite_sort_key("PaymentMethodAuthenticationService/PostAuthenticate")
        );
    }

    #[test]
    fn generated_markdown_uses_plain_status_without_badges() {
        let temp_root = std::env::temp_dir().join(format!("ucs-report-{}", now_epoch_ms()));
        fs::create_dir_all(&temp_root).expect("temp dir should be creatable");
        let json_path = temp_root.join("report.json");

        let mut report = ScenarioRunReport {
            runs: vec![
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "PaymentService/Authorize".to_string(),
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
                    audit_events: Vec::new(),
                    audit_error: None,
                },
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "CustomerService/Create".to_string(),
                    scenario: "CustomerService/Create".to_string(),
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
                    audit_events: Vec::new(),
                    audit_error: None,
                },
                ReportEntry {
                    run_at_epoch_ms: now_epoch_ms(),
                    suite: "PaymentService/Authorize".to_string(),
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
                    dependency: vec!["CustomerService/Create(CustomerService/Create)".to_string()],
                    req_body: Some(serde_json::json!({"field": "value"})),
                    res_body: Some(serde_json::json!({"error": "forced failure"})),
                    grpc_request: None,
                    grpc_response: None,
                    audit_events: Vec::new(),
                    audit_error: None,
                },
            ],
        };

        for entry in &mut report.runs {
            prepare_report_entry(entry, ReportMode::Summary, false);
        }
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
        assert!(content.contains("[100.0%](./connectors/stripe/paymentservice-authorize.md)"));
        assert!(content.contains("[0.0%](./connectors/paypal/paymentservice-authorize.md)"));

        let stripe_suite_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("stripe")
            .join("paymentservice-authorize.md");
        let stripe_suite_detail_content = fs::read_to_string(&stripe_suite_detail)
            .expect("suite detail markdown should be readable");
        assert!(stripe_suite_detail_content
            .contains("# Connector `stripe` / Suite `PaymentService/Authorize`"));
        assert!(stripe_suite_detail_content.contains(
            "[`Credit Card \\| No 3DS \\| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-credit-card.md)"
        ));

        let stripe_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("stripe")
            .join("paymentservice-authorize")
            .join("no3ds-auto-capture-credit-card.md");
        let stripe_detail_content =
            fs::read_to_string(&stripe_detail).expect("detail markdown should be readable");
        assert!(stripe_detail_content.contains(
            "# Connector `stripe` / Suite `PaymentService/Authorize` / Scenario `Credit Card | No 3DS | Automatic Capture`"
        ));
        assert!(stripe_detail_content.contains("- Scenario Key: `no3ds_auto_capture_credit_card`"));
        assert!(stripe_detail_content
            .contains("<summary>Show UCS Audit Events (typed and masked)</summary>"));
        assert!(!stripe_detail_content.contains("\"field\": \"value\""));
        assert!(!stripe_detail_content.contains("\"status\": \"CHARGED\""));
        assert!(stripe_detail_content.contains(
            "[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)"
        ));

        let paypal_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("paypal")
            .join("paymentservice-authorize")
            .join("no3ds-auto-capture-credit-card.md");
        let paypal_detail_content =
            fs::read_to_string(&paypal_detail).expect("paypal detail markdown should be readable");
        assert!(paypal_detail_content
            .contains("<summary>Show Dependency UCS Audit Events (typed and masked)</summary>"));
        assert!(!paypal_detail_content.contains("\"dep_req\": \"value\""));
        assert!(!paypal_detail_content.contains("\"dep_res\": \"ok\""));

        let paypal_suite_detail = temp_root
            .join("test_report")
            .join("connectors")
            .join("paypal")
            .join("paymentservice-authorize.md");
        let paypal_suite_detail_content = fs::read_to_string(&paypal_suite_detail)
            .expect("paypal suite detail should be readable");
        assert!(paypal_suite_detail_content.contains("## Failed Scenarios"));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn summary_discards_all_raw_diagnostics() {
        let mut entry = ReportEntry {
            run_at_epoch_ms: now_epoch_ms(),
            suite: "PaymentService/Authorize".to_string(),
            scenario: "no3ds_auto_capture_credit_card".to_string(),
            scenario_display_name: None,
            connector: "stripe".to_string(),
            pm: Some("card".to_string()),
            pmt: Some("credit".to_string()),
            endpoint: "localhost:8000".to_string(),
            is_dependency: false,
            assertion_result: "FAIL".to_string(),
            response_status: None,
            error: Some("assertion included raw response material".to_string()),
            dependency: vec![],
            req_body: Some(serde_json::json!({
                "state": {
                    "accessToken": {
                        "token": {"value": "request-secret"},
                        "tokenType": {"value": "Bearer"},
                        "expiresInSeconds": 60
                    }
                },
                "description": "visible"
            })),
            res_body: Some(serde_json::json!({
                "rawConnectorRequest": {"value": "raw connector request"},
                "rawConnectorResponse": {"value": "raw connector response"},
                "responseHeaders": {
                    "content-type": "application/json",
                    "request-id": "response-secret"
                }
            })),
            grpc_request: Some(
                "grpcurl -plaintext \\\n+  -H \"x-api-key: sk_test_123\" \\\n+  -H \"authorization: Bearer token123\" \\\n+  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'"
                    .to_string(),
            ),
            grpc_response: Some(
                "Response headers received:\nauthorization: Bearer token123\nx-api-key: sk_test_123"
                    .to_string(),
            ),
            audit_events: Vec::new(),
            audit_error: None,
        };

        prepare_report_entry(&mut entry, ReportMode::Summary, false);

        let error = entry.error.expect("error should exist");
        assert_eq!(error, "scenario_failed");
        assert!(entry.req_body.is_none());
        assert!(entry.res_body.is_none());
        assert!(entry.grpc_request.is_none());
        assert!(entry.grpc_response.is_none());
        assert!(entry.audit_events.is_empty());
    }

    #[test]
    fn extracts_only_the_processed_event_payload() {
        let event = serde_json::json!({
            "request_id": "PaymentService/Authorize_case_req",
            "connector": "stripe",
            "stage": "GrpcRequest",
            "timestamp": 42,
            "request_data": {"secret": "***MASKED***"}
        });
        let line = serde_json::json!({
            "target": "ucs_common_utils::events",
            "message": format!(
                "Event processed (Kafka publishing: disabled) - Event JSON: {}",
                event
            )
        })
        .to_string();
        assert_eq!(extract_processed_event(&line), Some(event));
        assert!(extract_processed_event("not json").is_none());
    }

    #[test]
    fn extracts_invocation_id_without_retaining_the_command() {
        let command = concat!(
            "grpcurl -plaintext \\\n",
            "  -H \"x-connector-config: raw-secret\" \\\n",
            "  -H \"x-request-id: unique-request-id\" \\\n",
            "  -d @ localhost:8000 types.PaymentService/Authorize"
        );
        assert_eq!(
            request_id_from_grpcurl_command(command).as_deref(),
            Some("unique-request-id")
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_debug_artifacts_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temporary directory should be available");
        let report = temp.path().join("local-debug-report.json");
        write_report_json(&report, "{\"runs\":[]}", ReportMode::LocalDebug)
            .expect("local report should be writable");
        let markdown_dir = report_dir_path(&report);
        fs::create_dir_all(markdown_dir.join("connectors"))
            .expect("markdown directory should be creatable");
        let markdown = markdown_dir.join("test_overview.md");
        fs::write(&markdown, "local evidence").expect("markdown should be writable");
        harden_local_debug_artifacts(&report, ReportMode::LocalDebug)
            .expect("local artifacts should become private");

        assert_eq!(
            fs::metadata(&report)
                .expect("report metadata should be readable")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&markdown)
                .expect("markdown metadata should be readable")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&markdown_dir)
                .expect("markdown directory metadata should be readable")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
