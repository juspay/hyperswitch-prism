use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

/// Shared schema types used across scenario loading, execution, and assertions.
pub type ScenarioFile = BTreeMap<String, ScenarioDef>;

/// Mapping of target request paths to dependency source paths.
///
/// Key   = target path in the downstream request (dot-notation, e.g. `state.access_token.token.value`)
/// Value = source reference, prefixed with `res.` or `req.` (e.g. `res.access_token`)
///
/// If the prefix is omitted, `res.` is assumed.
pub type ContextMap = HashMap<String, String>;

#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioDef {
    /// Request payload template for the suite/scenario.
    pub grpc_req: Value,
    /// Assertion rules evaluated against response JSON.
    #[serde(rename = "assert")]
    pub assert_rules: BTreeMap<String, FieldAssert>,
    /// Marks exactly one scenario in a suite as the default target.
    #[serde(default)]
    pub is_default: bool,
    /// Optional human-friendly scenario name for docs and reports.
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuiteSpec {
    /// Suite name, expected to match `<suite>_suite` folder naming.
    pub suite: String,
    /// Human-readable suite classification (`independent`, `payment_flow`, etc.).
    pub suite_type: String,
    /// Upstream suites/scenarios that must run before this suite.
    pub depends_on: Vec<SuiteDependency>,
    /// Whether dependency failures should stop this suite immediately.
    pub strict_dependencies: bool,
    /// Whether dependencies run once per suite or once per scenario.
    #[serde(default)]
    pub dependency_scope: DependencyScope,
    /// gRPC method to invoke for this suite, e.g. `"types.PaymentService/Authorize"`.
    /// When present, overrides the built-in suite→method mapping so new suites
    /// can be added via data files without modifying core harness code.
    #[serde(default)]
    pub grpc_method: Option<String>,
    /// Canonical suite name this suite aliases for dispatch purposes.
    ///
    /// When set, the tonic execution dispatch and proto-shape validation will
    /// treat this suite as if it were the aliased suite. This allows data-defined
    /// aliases to reuse the proto request type of a standard suite (for example,
    /// aliasing a custom suite to `authorize`) without extra core harness logic.
    ///
    /// Example: `"alias_for": "authorize"` makes the suite dispatch via the
    /// `PaymentService/Authorize` proto path.
    #[serde(default)]
    pub alias_for: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectorBrowserAutomationSpec {
    /// Browser automation hooks configured for this connector.
    #[serde(default)]
    pub hooks: Vec<BrowserAutomationHook>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrowserAutomationHook {
    /// Suite where this browser automation hook should run.
    pub suite: String,
    /// Scenario names where this hook applies. Empty = all scenarios in suite.
    #[serde(default)]
    pub scenarios: Vec<String>,
    /// Hook phase in scenario execution lifecycle.
    #[serde(default)]
    pub phase: BrowserAutomationPhase,
    /// Optional dependency suite to source redirect data from.
    #[serde(default)]
    pub after_dependency_suite: Option<String>,
    /// Optional dependency scenario to source redirect data from.
    #[serde(default)]
    pub after_dependency_scenario: Option<String>,
    /// Dependency response path for redirect endpoint.
    #[serde(default = "default_browser_endpoint_path")]
    pub endpoint_path: String,
    /// Dependency response path for redirect method (GET/POST).
    #[serde(default = "default_browser_method_path")]
    pub method_path: String,
    /// Dependency response path for redirect query params object.
    #[serde(default = "default_browser_query_params_path")]
    pub query_params_path: String,
    /// Optional request path fallback for redirect uri interpolation.
    #[serde(default)]
    pub redirect_uri_fallback_request_path: Option<String>,
    /// Rules sent to browser automation engine.
    #[serde(default)]
    pub rules: Vec<Value>,
    /// Maps target request paths to final browser URL query param keys.
    #[serde(default)]
    pub final_url_query_param_map: BTreeMap<String, String>,
    /// Fallback map from target request paths to redirect form field keys.
    #[serde(default)]
    pub fallback_form_field_map: BTreeMap<String, String>,
    /// Map from target request paths to browser response data keys.
    #[serde(default)]
    pub browser_data_map: BTreeMap<String, String>,
    /// Configuration for the `cli_pre_request` phase.
    #[serde(default)]
    pub cli_pre_request: Option<CliPreRequestHookConfig>,
}

/// Configuration for the `cli_pre_request` browser automation phase.
///
/// The hook runs an arbitrary CLI command before the gRPC request is sent,
/// reads its JSON output from a temp file, and maps fields from that output
/// back into the effective request payload.  No connector-specific knowledge
/// lives in the core harness — everything is expressed in the connector's
/// `browser_automation_spec.json`.
///
/// ## Placeholder substitutions in `args` and `env` values
///
/// | Placeholder       | Resolved to                                              |
/// |-------------------|----------------------------------------------------------|
/// | `{{connector}}`   | The connector name (e.g. `"stripe"`)                     |
/// | `{{creds_path}}`  | Absolute path to the harness credentials file            |
/// | `{{output_file}}` | Absolute path to a temp file the CLI must write JSON to  |
///
/// ## `output_map`
///
/// Keys are dot-notation paths into the gRPC request to overwrite.
/// Values are dot-notation paths into the JSON the CLI wrote to `{{output_file}}`.
///
/// Example:
/// ```json
/// {
///   "payment_method.google_pay.tokenization_data.encrypted_data.token": "paymentData.paymentMethodData.tokenizationData.token"
/// }
/// ```
///
/// ## `required_env`
///
/// If any of the listed environment variable names are absent from the current
/// process environment, the hook (and the scenario) are **skipped** with a
/// warning rather than failing hard.  This lets CI environments without the
/// necessary setup continue running the rest of the suite.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CliPreRequestHookConfig {
    /// CLI executable to invoke (e.g. `"npm"`).
    pub command: String,
    /// Arguments list; supports `{{connector}}`, `{{creds_path}}`,
    /// `{{output_file}}` placeholders.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to pass to the CLI process.
    /// Values support `{{creds_path}}` placeholder.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// If any of these env var names are not set in the current process
    /// environment, the scenario is skipped with a warning instead of failing.
    #[serde(default)]
    pub required_env: Vec<String>,
    /// Maps gRPC request target paths (keys) to source paths in the CLI's
    /// JSON output file (values).
    pub output_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAutomationPhase {
    #[default]
    BeforeRequest,
    /// Run an arbitrary CLI tool before the gRPC request; inject its JSON
    /// output into the request payload via `output_map`.
    CliPreRequest,
}

fn default_browser_endpoint_path() -> String {
    "redirection_data.form.endpoint".to_string()
}

fn default_browser_method_path() -> String {
    "redirection_data.form.method".to_string()
}

fn default_browser_query_params_path() -> String {
    "redirection_data.form.form_fields".to_string()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyScope {
    #[default]
    Suite,
    Scenario,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SuiteDependency {
    SuiteName(String),
    SuiteWithScenario {
        suite: String,
        #[serde(default)]
        scenario: Option<String>,
        #[serde(default)]
        context_map: Option<ContextMap>,
    },
}

impl SuiteDependency {
    pub fn suite(&self) -> &str {
        match self {
            Self::SuiteName(suite) => suite,
            Self::SuiteWithScenario { suite, .. } => suite,
        }
    }

    pub fn scenario(&self) -> Option<&str> {
        match self {
            Self::SuiteName(_) => None,
            Self::SuiteWithScenario { scenario, .. } => scenario.as_deref(),
        }
    }

    pub fn context_map(&self) -> Option<&ContextMap> {
        match self {
            Self::SuiteName(_) => None,
            Self::SuiteWithScenario { context_map, .. } => context_map.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectorSuiteSpec {
    /// Connector name represented by this spec file.
    pub connector: String,
    /// Suites explicitly supported for this connector.
    pub supported_suites: Vec<String>,
    /// If set, the harness reads this request field as the connector request
    /// reference ID instead of generating the default `{suite}_{scenario}_ref`.
    /// Example: `"merchant_order_id"`.
    #[serde(default)]
    pub request_id_source_field: Option<String>,
    /// Prefix for auto-generated reference IDs when `request_id_source_field`
    /// is set but resolves to an empty value. Defaults to `""`.
    #[serde(default)]
    pub request_id_prefix: Option<String>,
    /// Max length for auto-generated reference IDs (default: no truncation).
    #[serde(default)]
    pub request_id_length: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FieldAssert {
    MustExist { must_exist: bool },
    MustNotExist { must_not_exist: bool },
    Equals { equals: Value },
    OneOf { one_of: Vec<Value> },
    Contains { contains: String },
    Echo { echo: String },
}

// Note: Cannot use #[serde(deny_unknown_fields)] on untagged enum variants.
// The untagged deserialization tries each variant in order, so extra fields
// will cause it to try the next variant rather than failing immediately.

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error("failed to read scenario file '{path}': {source}")]
    ScenarioFileRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse scenario file '{path}': {source}")]
    ScenarioFileParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("scenario '{scenario}' not found in suite '{suite}'")]
    ScenarioNotFound { suite: String, scenario: String },
    #[error("assertion rule for field '{field}' is invalid: {message}")]
    InvalidAssertionRule { field: String, message: String },
    #[error("assertion failed for field '{field}': {message}")]
    AssertionFailed { field: String, message: String },
    #[error("unsupported suite '{suite}' for grpcurl generation")]
    UnsupportedSuite { suite: String },
    #[error("failed to serialize JSON payload: {source}")]
    JsonSerialize { source: serde_json::Error },
    #[error("failed to load connector auth for '{connector}': {message}")]
    CredentialLoad { connector: String, message: String },
    #[error("grpcurl execution failed: {message}")]
    GrpcurlExecution { message: String },
    #[error("sdk call failed: {message}")]
    SdkExecution { message: String },
    #[error("failed to read suite spec '{path}': {source}")]
    SuiteSpecRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse suite spec '{path}': {source}")]
    SuiteSpecParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("suite spec '{path}' not found")]
    SuiteSpecMissing { path: PathBuf },
    #[error("no default scenario found in suite '{suite}'")]
    DefaultScenarioMissing { suite: String },
    #[error("multiple default scenarios found in suite '{suite}': {scenarios}")]
    MultipleDefaultScenarios { suite: String, scenarios: String },
    #[error("failed to read connector spec '{path}': {source}")]
    ConnectorSpecRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse connector spec '{path}': {source}")]
    ConnectorSpecParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to read connector override file '{path}': {source}")]
    ConnectorOverrideRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse connector override file '{path}': {source}")]
    ConnectorOverrideParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("scenario skipped: {reason}")]
    Skipped { reason: String },
}
