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
}
