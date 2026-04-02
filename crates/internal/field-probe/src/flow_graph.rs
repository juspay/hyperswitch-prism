//! Flow graph generation for connector dependency visualization.
//!
//! This module builds a machine-readable flow graph that captures:
//! - Which flows a connector supports
//! - What order they must be called in
//! - What fields flow between flows
//! - Prerequisites (access tokens, orders, customers)

use crate::types::{
    CompactConnectorResult, FieldProvider, FieldRequirement, FlowEdge, FlowGraph, FlowNode,
    FlowNodeType,
};
use std::collections::{BTreeMap, HashSet};

/// Known provider fields that flows can provide to other flows
/// Format: (field_name, (description, example_path))
const KNOWN_PROVIDERS: &[(&str, (&str, &str))] = &[
    (
        "connector_transaction_id",
        ("Transaction ID from authorization", "authorize"),
    ),
    (
        "connector_customer_id",
        ("Customer ID from create_customer", "create_customer"),
    ),
    (
        "connector_order_id",
        ("Order ID from create_order", "create_order"),
    ),
    (
        "access_token",
        ("OAuth2 access token", "create_access_token"),
    ),
    (
        "mandate_reference",
        ("Mandate reference for recurring", "setup_recurring"),
    ),
    (
        "payment_method_token",
        ("Tokenized payment method", "tokenize"),
    ),
    (
        "connector_refund_id",
        ("Refund ID from refund flow", "refund"),
    ),
];

/// Flows that are prerequisites (must run before entry points)
const PREREQUISITE_FLOWS: &[&str] = &["create_access_token", "create_customer", "create_order"];

/// Flows that are entry points (start payment lifecycle)
const ENTRY_POINT_FLOWS: &[&str] = &[
    "authorize",
    "setup_recurring",
    "tokenize",
    "pre_authenticate",
];

/// Build a flow graph for a connector from its probe results.
///
/// This is a placeholder implementation that uses hardcoded relationships
/// based on common connector patterns. A future enhancement would detect
/// these automatically from proto schemas and request/response inspection.
pub(crate) fn build_flow_graph(
    connector_name: &str,
    connector_result: &CompactConnectorResult,
) -> Option<FlowGraph> {
    let mut nodes: BTreeMap<String, FlowNode> = BTreeMap::new();
    let mut edges: Vec<FlowEdge> = Vec::new();

    // Get list of supported flows
    let supported_flows: HashSet<String> = connector_result.flows.keys().cloned().collect();

    if supported_flows.is_empty() {
        return None;
    }

    // Build nodes for each supported flow
    for flow_name in &supported_flows {
        let node = build_flow_node(flow_name, &supported_flows, connector_name);
        nodes.insert(flow_name.clone(), node);
    }

    // Build edges from requires relationships
    for (flow_name, node) in &nodes {
        for (_field_key, req) in &node.requires {
            if supported_flows.contains(&req.from_flow) {
                edges.push(FlowEdge {
                    from: req.from_flow.clone(),
                    to: flow_name.clone(),
                });
            }
        }
    }

    // Deduplicate edges
    edges.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    edges.dedup_by(|a, b| a.from == b.from && a.to == b.to);

    Some(FlowGraph { nodes, edges })
}

/// Build a single flow node with its metadata
fn build_flow_node(
    flow_name: &str,
    supported_flows: &HashSet<String>,
    _connector_name: &str,
) -> FlowNode {
    let node_type = classify_flow_type(flow_name);
    let description = get_flow_description(flow_name);

    let mut provides = BTreeMap::new();
    let mut requires = BTreeMap::new();

    // Define what this flow provides
    match flow_name {
        "authorize" => {
            provides.insert(
                "connector_transaction_id".to_string(),
                FieldProvider {
                    response_path: "connector_transaction_id".to_string(),
                    description: "Transaction ID for subsequent operations".to_string(),
                },
            );
        }
        "create_customer" => {
            provides.insert(
                "connector_customer_id".to_string(),
                FieldProvider {
                    response_path: "connector_customer_id".to_string(),
                    description: "Customer ID for recurring payments".to_string(),
                },
            );
        }
        "create_order" => {
            provides.insert(
                "order_id".to_string(),
                FieldProvider {
                    response_path: "connector_order_id".to_string(),
                    description: "Order ID for payment authorization".to_string(),
                },
            );
        }
        "create_access_token" => {
            provides.insert(
                "access_token".to_string(),
                FieldProvider {
                    response_path: "state.access_token.token.value".to_string(),
                    description: "OAuth2 access token for API calls".to_string(),
                },
            );
        }
        "setup_recurring" => {
            provides.insert(
                "mandate_reference".to_string(),
                FieldProvider {
                    response_path: "mandate_reference.connector_mandate_id.connector_mandate_id"
                        .to_string(),
                    description: "Mandate reference for recurring charges".to_string(),
                },
            );
        }
        "tokenize" => {
            provides.insert(
                "payment_method_token".to_string(),
                FieldProvider {
                    response_path: "payment_method_token".to_string(),
                    description: "Tokenized payment method".to_string(),
                },
            );
        }
        "refund" => {
            provides.insert(
                "refund_id".to_string(),
                FieldProvider {
                    response_path: "connector_refund_id".to_string(),
                    description: "Refund ID for tracking".to_string(),
                },
            );
        }
        _ => {}
    }

    // Define what this flow requires
    match flow_name {
        "capture" | "refund" | "void" | "get" => {
            // These flows require a prior authorize
            if supported_flows.contains("authorize") {
                requires.insert(
                    "connector_transaction_id".to_string(),
                    FieldRequirement {
                        from_flow: "authorize".to_string(),
                        from_field: "connector_transaction_id".to_string(),
                        request_path: "connector_transaction_id".to_string(),
                    },
                );
            }
        }
        "recurring_charge" => {
            // Requires setup_recurring for mandate
            if supported_flows.contains("setup_recurring") {
                requires.insert(
                    "connector_recurring_payment_id".to_string(),
                    FieldRequirement {
                        from_flow: "setup_recurring".to_string(),
                        from_field: "mandate_reference".to_string(),
                        request_path: "connector_recurring_payment_id.connector_mandate_id.connector_mandate_id".to_string(),
                    },
                );
            }
            // May also require customer
            if supported_flows.contains("create_customer") {
                requires.insert(
                    "connector_customer_id".to_string(),
                    FieldRequirement {
                        from_flow: "create_customer".to_string(),
                        from_field: "connector_customer_id".to_string(),
                        request_path: "connector_customer_id".to_string(),
                    },
                );
            }
        }
        "authorize" => {
            // Check if connector has prerequisites
            for prereq in PREREQUISITE_FLOWS {
                if supported_flows.contains(*prereq) {
                    match *prereq {
                        "create_access_token" => {
                            requires.insert(
                                "access_token".to_string(),
                                FieldRequirement {
                                    from_flow: "create_access_token".to_string(),
                                    from_field: "access_token".to_string(),
                                    request_path: "state.access_token.token.value".to_string(),
                                },
                            );
                        }
                        "create_order" => {
                            requires.insert(
                                "order_id".to_string(),
                                FieldRequirement {
                                    from_flow: "create_order".to_string(),
                                    from_field: "order_id".to_string(),
                                    request_path: "merchant_order_id".to_string(),
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }

    FlowNode {
        node_type,
        description,
        provides,
        requires,
    }
}

/// Classify a flow into its node type
fn classify_flow_type(flow_name: &str) -> FlowNodeType {
    if PREREQUISITE_FLOWS.contains(&flow_name) {
        FlowNodeType::Prerequisite
    } else if ENTRY_POINT_FLOWS.contains(&flow_name) {
        FlowNodeType::EntryPoint
    } else {
        FlowNodeType::Dependent
    }
}

/// Get human-readable description for a flow
fn get_flow_description(flow_name: &str) -> String {
    match flow_name {
        "create_access_token" => "Obtain OAuth2 access token for API authentication".to_string(),
        "create_customer" => "Create a customer profile for recurring payments".to_string(),
        "create_order" => "Create an order before payment authorization".to_string(),
        "authorize" => "Authorize a payment, reserving funds".to_string(),
        "capture" => "Capture previously authorized funds".to_string(),
        "refund" => "Refund a completed payment".to_string(),
        "void" => "Cancel an uncaptured authorization".to_string(),
        "get" => "Retrieve payment status and details".to_string(),
        "setup_recurring" => "Set up a mandate for recurring charges".to_string(),
        "recurring_charge" => "Charge against an existing mandate".to_string(),
        "tokenize" => "Tokenize a payment method for later use".to_string(),
        "pre_authenticate" => "Pre-authenticate before capture".to_string(),
        _ => format!("{}", flow_name),
    }
}
