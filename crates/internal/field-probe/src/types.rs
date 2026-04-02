use crate::flow_metadata::{FlowMetadata, MessageSchema};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Statistics for error types across all connectors
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct ErrorStats {
    pub(crate) missing_field: usize,
    pub(crate) not_implemented: usize,
    pub(crate) not_supported: usize,
    pub(crate) invalid_config: usize,
    pub(crate) other: usize,
}

impl ErrorStats {
    #[allow(dead_code)]
    pub(crate) fn total(&self) -> usize {
        self.missing_field
            + self.not_implemented
            + self.not_supported
            + self.invalid_config
            + self.other
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct SamplePayload {
    pub(crate) url: String,
    pub(crate) method: String,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct FlowResult {
    pub(crate) status: String, // "supported" | "not_supported" | "error"
    pub(crate) required_fields: Vec<String>,
    /// The proto JSON request that produced a successful transformer call.
    /// This is what the SDK user should send to UCS.
    pub(crate) proto_request: Option<serde_json::Value>,
    pub(crate) sample: Option<SamplePayload>,
    pub(crate) error: Option<String>,
    /// Full gRPC service.rpc name (e.g., "PaymentService.Authorize")
    pub(crate) service_rpc: Option<String>,
    /// Human-readable description from proto comments
    pub(crate) description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct ConnectorResult {
    pub(crate) connector: String,
    pub(crate) flows: BTreeMap<String, BTreeMap<String, FlowResult>>,
}

/// Top-level output structure for the manifest file
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ProbeManifest {
    /// Flow metadata for all probed flows (generated from services.proto)
    pub(crate) flow_metadata: Vec<FlowMetadata>,
    /// List of connector names that were probed
    pub(crate) connectors: Vec<String>,
    /// Proto message schemas: field comments and nested message types.
    /// Key is the message name (e.g. "PaymentServiceAuthorizeRequest").
    pub(crate) message_schemas: BTreeMap<String, MessageSchema>,
    /// Schema version for future compatibility
    pub(crate) schema_version: String,
    /// Scenario groups — manually maintained; preserved across field-probe runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) scenario_groups: Vec<serde_json::Value>,
}

/// Compact flow result that omits null fields and not_supported status
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct CompactFlowResult {
    pub(crate) status: String, // "supported" | "error" (not_supported is omitted entirely)
    /// The proto JSON request that produced a successful transformer call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) proto_request: Option<serde_json::Value>,
    /// Sample payload for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample: Option<SamplePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl From<FlowResult> for Option<CompactFlowResult> {
    fn from(result: FlowResult) -> Self {
        // Include not_supported entries so documentation generators can show x (not supported)
        // instead of ? (unknown) for payment methods that were probed but not supported
        Some(CompactFlowResult {
            status: result.status,
            proto_request: result.proto_request,
            sample: result.sample,
            error: result.error,
        })
    }
}

/// Compact connector result with omitted null fields
#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct CompactConnectorResult {
    pub(crate) connector: String,
    pub(crate) flows: BTreeMap<String, BTreeMap<String, CompactFlowResult>>,
}

// ============================================================================
// Flow Graph Types - Added for documentation generator refactoring
// ============================================================================

/// Node type classification in the flow graph
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FlowNodeType {
    /// Must run before entry points (create_access_token, create_customer, create_order)
    Prerequisite,
    /// Starts payment lifecycle (authorize, setup_recurring, tokenize)
    EntryPoint,
    /// Requires prior flow output (capture, refund, void, recurring_charge)
    Dependent,
}

/// A field that a flow provides in its response
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct FieldProvider {
    /// JSON path in the response (e.g., "connector_transaction_id")
    pub(crate) response_path: String,
    /// Human-readable description
    pub(crate) description: String,
}

/// A field that a flow requires from a prior flow's response
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct FieldRequirement {
    /// Source flow that provides this field
    pub(crate) from_flow: String,
    /// Key in the source flow's "provides" (e.g., "connector_transaction_id")
    pub(crate) from_field: String,
    /// Path in this flow's request where the value goes
    pub(crate) request_path: String,
}

/// One flow in the connector's flow graph
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct FlowNode {
    /// Type of node (prerequisite, entry_point, dependent)
    pub(crate) node_type: FlowNodeType,
    /// Human-readable description
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub(crate) description: String,
    /// Fields this flow provides in its response
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub(crate) provides: BTreeMap<String, FieldProvider>,
    /// Fields this flow requires from prior flows
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub(crate) requires: BTreeMap<String, FieldRequirement>,
}

impl Default for FlowNode {
    fn default() -> Self {
        Self {
            node_type: FlowNodeType::Dependent,
            description: String::new(),
            provides: BTreeMap::new(),
            requires: BTreeMap::new(),
        }
    }
}

/// Directed edge: from_flow must complete before to_flow can run
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct FlowEdge {
    pub(crate) from: String,
    pub(crate) to: String,
}

/// Connector-specific flow dependency graph
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct FlowGraph {
    /// All flows keyed by flow name
    pub(crate) nodes: BTreeMap<String, FlowNode>,
    /// Dependencies between flows
    pub(crate) edges: Vec<FlowEdge>,
}

/// Connector result with flow graph (extends CompactConnectorResult)
#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct ConnectorResultWithGraph {
    pub(crate) connector: String,
    pub(crate) flows: BTreeMap<String, BTreeMap<String, CompactFlowResult>>,
    /// Flow dependency graph for this connector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flow_graph: Option<FlowGraph>,
}
