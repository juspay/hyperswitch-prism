//! Core probe engine that tests connector flows iteratively.
//!
//! The probe engine implements a retry loop that:
//! 1. Calls the connector's request transformer with the current request
//! 2. If successful: records the result as "supported"
//! 3. If error: classifies the error and either:
//!    - Stops with "not_implemented" or "not_supported" status
//!    - Attempts to patch the missing field and retries
//!    - Stops with "error" if field cannot be patched
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────────┐     ┌─────────────┐
//! │ Base Request │────▶│ Connector        │────▶│ Success?    │
//! │ (minimal)    │     │ Transformer      │     └──────┬──────┘
//! └──────────────┘     └──────────────────┘            │
//!                                                      │
//!                            ┌──────────Yes───────────┘
//!                            │
//!                            ▼
//!                     ┌──────────────┐
//!                     │ Mark as      │
//!                     │ "supported"  │
//!                     └──────────────┘
//!                            │
//!                            │ No
//!                            ▼
//!                     ┌──────────────┐
//!                     │ Classify     │
//!                     │ Error        │
//!                     └──────┬───────┘
//!                            │
//!              ┌─────────────┼─────────────┐
//!              │             │             │
//!              ▼             ▼             ▼
//!       ┌──────────┐  ┌──────────┐  ┌──────────┐
//!       │ NotImpl  │  │ NotSupp  │  │ Missing  │
//!       │ → Stop   │  │ → Stop   │  │ → Patch  │
//!       └──────────┘  └──────────┘  └────┬─────┘
//!                                        │
//!                                        ▼
//!                              ┌──────────────────┐
//!                              │ Retry with       │
//!                              │ patched request  │
//!                              └──────────────────┘
//! ```

use std::collections::HashSet;

use serde::Serialize;

use crate::config::max_iterations;
use crate::error_parsing::{
    is_not_implemented, is_not_supported, parse_missing_field, parse_missing_field_alt,
};
use crate::json_utils::{clean_proto_request, convert_rust_to_proto_json};
use crate::normalizer::extract_sample;
use crate::status::FlowStatus;
use crate::types::FlowResult;

pub(crate) type PciFfi = domain_types::payment_method_data::DefaultPCIHolder;

/// Result of a single probe attempt.
#[derive(Debug)]
enum ProbeAttemptResult {
    /// Successfully generated a connector request.
    Success(common_utils::request::Request),
    /// Connector returned None (flow not implemented).
    NotImplemented,
    /// Connector returned an error.
    Error(String),
}

/// Classification of what to do next after an error.
#[derive(Debug)]
enum ErrorAction {
    /// Stop probing and return this status.
    Stop(FlowStatus),
    /// Patch the field and retry.
    PatchAndRetry(String),
}

/// Runs a probe for a single connector flow.
///
/// This is the main entry point for probing. It takes a base request and
/// repeatedly calls the connector transformer, patching missing fields
/// until either success or max iterations is reached.
///
/// # Type Parameters
/// * `Req` - The request type (e.g., PaymentServiceAuthorizeRequest)
/// * `F` - The transformer function type
///
/// # Arguments
/// * `req` - The initial request to probe with
/// * `call` - The connector transformer function
/// * `patch` - Function to patch a missing field into the request
///
/// # Returns
/// A FlowResult containing the final status and any collected data
pub(crate) fn run_probe<Req, F>(
    flow_name: &str,
    mut req: Req,
    mut call: F,
    mut patch: impl FnMut(&mut Req, &str),
) -> FlowResult
where
    Req: Clone + Serialize,
    F: FnMut(
        Req,
    ) -> Result<
        Option<common_utils::request::Request>,
        grpc_api_types::payments::IntegrationError,
    >,
{
    let mut required_fields: Vec<String> = Vec::new();
    let mut seen_fields: HashSet<String> = HashSet::new();

    for _iteration in 0..max_iterations() {
        match attempt_probe(flow_name, &req, &mut call) {
            ProbeAttemptResult::Success(connector_req) => {
                return handle_success(req, connector_req, required_fields);
            }
            ProbeAttemptResult::NotImplemented => {
                return handle_not_implemented(required_fields);
            }
            ProbeAttemptResult::Error(msg) => match classify_error_action(&msg) {
                ErrorAction::Stop(status) => {
                    return handle_error_status(status, required_fields, msg);
                }
                ErrorAction::PatchAndRetry(field) => {
                    if !handle_patch_attempt(
                        &field,
                        &mut seen_fields,
                        &mut required_fields,
                        &mut patch,
                        &mut req,
                    ) {
                        return handle_stuck_field(&field, &msg, required_fields);
                    }
                }
            },
        }
    }

    handle_max_iterations_reached(required_fields)
}

/// Attempts a single probe call.
fn attempt_probe<Req, F>(_flow_name: &str, req: &Req, call: &mut F) -> ProbeAttemptResult
where
    Req: Clone + Serialize,
    F: FnMut(
        Req,
    ) -> Result<
        Option<common_utils::request::Request>,
        grpc_api_types::payments::IntegrationError,
    >,
{
    match call(req.clone()) {
        Ok(Some(connector_req)) => {
            // If the connector returned a request with no URL, treat it as not_implemented.
            // This happens when ConnectorIntegrationV2 is implemented as an empty default
            // impl (no get_url override), so the default build_request_v2 produces a
            // Request with an empty URL string.
            if connector_req.url.is_empty() {
                ProbeAttemptResult::NotImplemented
            } else {
                ProbeAttemptResult::Success(connector_req)
            }
        }
        Ok(None) => ProbeAttemptResult::NotImplemented,
        Err(e) => {
            let msg = e.error_message;
            ProbeAttemptResult::Error(msg)
        }
    }
}

/// Handles a successful probe result.
fn handle_success<Req: Serialize>(
    req: Req,
    connector_req: common_utils::request::Request,
    required_fields: Vec<String>,
) -> FlowResult {
    let proto_json = serde_json::to_value(&req)
        .ok()
        .map(|v| convert_rust_to_proto_json(&v))
        .map(|v| clean_proto_request(&v));

    FlowResult {
        status: FlowStatus::Supported.to_string(),
        required_fields,
        proto_request: proto_json,
        sample: Some(extract_sample(&connector_req)),
        error: None,
        service_rpc: None,
        description: None,
    }
}

/// Handles a "not implemented" result.
fn handle_not_implemented(required_fields: Vec<String>) -> FlowResult {
    FlowResult {
        status: FlowStatus::NotImplemented.to_string(),
        required_fields,
        proto_request: None,
        sample: None,
        error: None,
        service_rpc: None,
        description: None,
    }
}

/// Classifies an error message and determines the next action.
fn classify_error_action(msg: &str) -> ErrorAction {
    if is_not_implemented(msg) {
        ErrorAction::Stop(FlowStatus::NotImplemented)
    } else if is_not_supported(msg) {
        ErrorAction::Stop(FlowStatus::NotSupported)
    } else if let Some(field) = parse_missing_field(msg).or_else(|| parse_missing_field_alt(msg)) {
        ErrorAction::PatchAndRetry(field)
    } else {
        ErrorAction::Stop(FlowStatus::Failed)
    }
}

/// Handles an error status that should stop probing.
fn handle_error_status(
    status: FlowStatus,
    required_fields: Vec<String>,
    msg: String,
) -> FlowResult {
    FlowResult {
        status: status.to_string(),
        required_fields,
        proto_request: None,
        sample: None,
        error: Some(msg),
        service_rpc: None,
        description: None,
    }
}

/// Attempts to patch a missing field.
///
/// Returns true if the field was successfully queued for patching,
/// false if we've seen this field before (indicating we're stuck).
fn handle_patch_attempt<Req>(
    field: &str,
    seen_fields: &mut HashSet<String>,
    required_fields: &mut Vec<String>,
    patch: &mut impl FnMut(&mut Req, &str),
    req: &mut Req,
) -> bool
where
    Req: Serialize,
{
    if seen_fields.contains(field) {
        return false;
    }

    seen_fields.insert(field.to_string());
    required_fields.push(field.to_string());
    patch(req, field);

    true
}

/// Handles the case where we're stuck on a field (already seen it).
fn handle_stuck_field(field: &str, msg: &str, required_fields: Vec<String>) -> FlowResult {
    FlowResult {
        status: FlowStatus::Failed.to_string(),
        required_fields,
        proto_request: None,
        sample: None,
        error: Some(format!("Stuck on field: {field} — {msg}")),
        service_rpc: None,
        description: None,
    }
}

/// Handles reaching the maximum number of iterations.
fn handle_max_iterations_reached(required_fields: Vec<String>) -> FlowResult {
    FlowResult {
        status: FlowStatus::Failed.to_string(),
        required_fields,
        proto_request: None,
        sample: None,
        error: Some("Max iterations reached".to_string()),
        service_rpc: None,
        description: None,
    }
}
