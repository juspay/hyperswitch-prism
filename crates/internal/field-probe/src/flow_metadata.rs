//! Flow metadata extraction from services.proto
//!
//! This module parses the gRPC service definitions from services.proto and
//! generates flow metadata that maps probe flow keys to their canonical
//! gRPC service.rpc names and human-readable descriptions.
//!
//! NO HARDCODED DATA - All metadata is parsed from services.proto at runtime.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Flow metadata extracted from services.proto
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FlowMetadata {
    /// Flow key used in probe (e.g., "authorize", "capture")
    pub flow_key: String,
    /// Full gRPC service.rpc name (e.g., "PaymentService.Authorize")
    pub service_rpc: String,
    /// Human-readable description from proto comments
    pub description: String,
    /// Service name (e.g., "PaymentService")
    pub service_name: String,
    /// RPC/method name (e.g., "Authorize")
    pub rpc_name: String,
    /// Category for grouping in documentation (e.g., "Payments", "Refunds")
    pub category: String,
    /// gRPC request message name (e.g., "PaymentServiceAuthorizeRequest")
    pub grpc_request: String,
    /// gRPC response message name (e.g., "PaymentServiceAuthorizeResponse")
    pub grpc_response: String,
}

impl FlowMetadata {
    /// Create a new flow metadata entry
    pub fn new(
        flow_key: String,
        service_name: String,
        rpc_name: String,
        description: String,
        grpc_request: String,
        grpc_response: String,
    ) -> Self {
        let category = get_category_for_service(&service_name);
        Self {
            flow_key,
            service_rpc: format!("{}.{}", service_name, rpc_name),
            description,
            service_name,
            rpc_name,
            category,
            grpc_request,
            grpc_response,
        }
    }
}

/// Get category for a service name
fn get_category_for_service(service_name: &str) -> String {
    match service_name {
        "PaymentService" => "Payments".to_string(),
        "RecurringPaymentService" => "Mandates".to_string(),
        "RefundService" => "Refunds".to_string(),
        "CustomerService" => "Customers".to_string(),
        "PaymentMethodService" => "Payments".to_string(),
        "MerchantAuthenticationService" => "Authentication".to_string(),
        "PaymentMethodAuthenticationService" => "Authentication".to_string(),
        "TokenizedPaymentService" => "Non-PCI Payments".to_string(),
        "ProxiedPaymentService" => "Non-PCI Payments".to_string(),
        "DisputeService" => "Disputes".to_string(),
        "EventService" => "Events".to_string(),
        _ => "Other".to_string(),
    }
}

/// Mapping from (service_name, rpc_name) to probe flow key
/// This is the only configuration needed - everything else comes from proto
fn get_flow_key_mapping() -> HashMap<(&'static str, &'static str), &'static str> {
    [
        // PaymentService
        (("PaymentService", "Authorize"), "authorize"),
        (("PaymentService", "Capture"), "capture"),
        (("PaymentService", "Get"), "get"),
        (("PaymentService", "Void"), "void"),
        (("PaymentService", "Reverse"), "reverse"),
        (("PaymentService", "Refund"), "refund"),
        (("PaymentService", "CreateOrder"), "create_order"),
        (("PaymentService", "SetupRecurring"), "setup_recurring"),
        (
            ("PaymentService", "IncrementalAuthorization"),
            "incremental_auth",
        ),
        (
            ("PaymentService", "VerifyRedirectResponse"),
            "verify_redirect",
        ),
        // RecurringPaymentService
        (("RecurringPaymentService", "Charge"), "recurring_charge"),
        (("RecurringPaymentService", "Revoke"), "mandate_revoke"),
        // RefundService
        (("RefundService", "Get"), "rsync"),
        // CustomerService
        (("CustomerService", "Create"), "create_customer"),
        // PaymentMethodService
        (("PaymentMethodService", "Tokenize"), "tokenize"),
        // MerchantAuthenticationService
        (
            ("MerchantAuthenticationService", "CreateAccessToken"),
            "create_access_token",
        ),
        (
            ("MerchantAuthenticationService", "CreateSessionToken"),
            "create_session_token",
        ),
        (
            ("MerchantAuthenticationService", "CreateSdkSessionToken"),
            "sdk_session_token",
        ),
        // PaymentMethodAuthenticationService
        (
            ("PaymentMethodAuthenticationService", "PreAuthenticate"),
            "pre_authenticate",
        ),
        (
            ("PaymentMethodAuthenticationService", "Authenticate"),
            "authenticate",
        ),
        (
            ("PaymentMethodAuthenticationService", "PostAuthenticate"),
            "post_authenticate",
        ),
        // DisputeService
        (
            ("DisputeService", "SubmitEvidence"),
            "dispute_submit_evidence",
        ),
        (("DisputeService", "Get"), "dispute_get"),
        (("DisputeService", "Defend"), "dispute_defend"),
        (("DisputeService", "Accept"), "dispute_accept"),
        // TokenizedPaymentService
        (
            ("TokenizedPaymentService", "Authorize"),
            "tokenized_authorize",
        ),
        (
            ("TokenizedPaymentService", "SetupRecurring"),
            "tokenized_setup_recurring",
        ),
        // ProxiedPaymentService
        (("ProxiedPaymentService", "Authorize"), "proxied_authorize"),
        (
            ("ProxiedPaymentService", "SetupRecurring"),
            "proxied_setup_recurring",
        ),
        // EventService
        (("EventService", "HandleEvent"), "handle_event"),
    ]
    .iter()
    .cloned()
    .collect()
}

/// Parse services.proto and extract flow metadata.
/// Maps probe flow keys to their gRPC service.rpc names and descriptions.
///
/// Returns empty vec if services.proto cannot be found or parsed (with error logged).
pub fn parse_services_proto() -> Vec<FlowMetadata> {
    // Try to find services.proto
    let proto_paths = [
        "crates/types-traits/grpc-api-types/proto/services.proto",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../types-traits/grpc-api-types/proto/services.proto"
        ),
    ];

    let proto_content = proto_paths
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok());

    match proto_content {
        Some(content) => parse_proto_content(&content),
        None => {
            eprintln!(
                "ERROR: Could not find services.proto. Searched paths:\n  {}",
                proto_paths.join("\n  ")
            );
            Vec::new()
        }
    }
}

/// Parse proto file content and extract service/RPC definitions with comments
fn parse_proto_content(content: &str) -> Vec<FlowMetadata> {
    let mut metadata: Vec<FlowMetadata> = Vec::new();
    let mut current_service: Option<String> = None;
    let mut pending_comment: String = String::new();

    let flow_key_mapping = get_flow_key_mapping();

    for line in content.lines() {
        let trimmed = line.trim();

        // Track service declarations
        if trimmed.starts_with("service ") && trimmed.ends_with("{") {
            let service_name = trimmed
                .strip_prefix("service ")
                .unwrap_or("")
                .trim()
                .trim_end_matches('{')
                .trim()
                .to_string();
            current_service = Some(service_name);
            pending_comment.clear();
            continue;
        }

        // Track closing braces (end of service)
        if trimmed == "}" {
            current_service = None;
            pending_comment.clear();
            continue;
        }

        // Collect comment lines (description) - must be directly above the RPC
        if trimmed.starts_with("//") {
            let comment = trimmed.trim_start_matches('/').trim();
            if !comment.is_empty() {
                // Check if this is a standalone line comment (not inline)
                if !line.starts_with("rpc") {
                    if !pending_comment.is_empty() {
                        pending_comment.push(' ');
                    }
                    pending_comment.push_str(comment);
                }
            }
            continue;
        }

        // Parse RPC definitions
        if let Some(ref service_name) = current_service {
            if trimmed.starts_with("rpc ") {
                // Extract RPC name
                let rpc_part = trimmed.strip_prefix("rpc ").unwrap_or("").trim();
                let rpc_name = rpc_part.split('(').next().unwrap_or("").trim().to_string();

                // Extract request and response message names
                let (grpc_request, grpc_response) = extract_message_names(trimmed);

                // Look up flow key using (service, rpc) tuple
                if let Some(&flow_key) =
                    flow_key_mapping.get(&(service_name.as_str(), rpc_name.as_str()))
                {
                    let description = clean_description(&pending_comment);
                    metadata.push(FlowMetadata::new(
                        flow_key.to_string(),
                        service_name.clone(),
                        rpc_name,
                        description,
                        grpc_request,
                        grpc_response,
                    ));
                }

                // Clear pending comment after processing an RPC
                pending_comment.clear();
            }
        }
    }

    if metadata.is_empty() {
        eprintln!(
            "WARNING: No flows extracted from services.proto. Check that the proto file contains RPC definitions."
        );
    } else {
        eprintln!(
            "Extracted {} flow metadata entries from services.proto",
            metadata.len()
        );
    }

    metadata
}

/// Extract request and response message names from RPC definition
/// Example: "rpc Authorize(PaymentServiceAuthorizeRequest) returns (PaymentServiceAuthorizeResponse);"
fn extract_message_names(rpc_line: &str) -> (String, String) {
    // Find the request message between first pair of parentheses
    let request = rpc_line
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Find the response message after "returns"
    let response = rpc_line
        .split("returns")
        .nth(1)
        .and_then(|s| s.split('(').nth(1))
        .and_then(|s| s.split(')').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    (request, response)
}

/// Clean up description text extracted from comments
fn clean_description(desc: &str) -> String {
    // Remove " // " patterns that might have been captured
    let cleaned = desc
        .replace(" // ", " ")
        .replace("  ", " ")
        .trim()
        .to_string();

    // Ensure it ends with a period for consistency
    if !cleaned.is_empty()
        && !cleaned.ends_with('.')
        && !cleaned.ends_with('!')
        && !cleaned.ends_with('?')
    {
        format!("{}.", cleaned)
    } else {
        cleaned
    }
}

// ============================================================================
// MESSAGE SCHEMA EXTRACTION
// Parses all *.proto files and extracts per-field comments and message-type
// information so that SDK doc snippets can annotate request payloads.
// ============================================================================

/// Schema for a single protobuf message type.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MessageSchema {
    /// Proto doc comments keyed by field name.
    /// e.g. {"minor_amount": "Amount in minor units (e.g., 1000 = $10.00)"}
    pub comments: BTreeMap<String, String>,
    /// Field name → declared message type, for non-scalar fields only.
    /// Used by the doc generator to recurse into nested JSON objects.
    /// e.g. {"amount": "Money", "payment_method": "PaymentMethod"}
    pub field_types: BTreeMap<String, String>,
}

/// Proto scalar / wrapper types that serialize as JSON scalars.
/// Fields of these types are not recursed into during annotation.
const SCALAR_TYPES: &[&str] = &[
    "string",
    "int32",
    "int64",
    "uint32",
    "uint64",
    "sint32",
    "sint64",
    "fixed32",
    "fixed64",
    "sfixed32",
    "sfixed64",
    "bool",
    "bytes",
    "double",
    "float",
    // UCS wrapper types that serialize as plain strings in JSON
    "SecretString",
    "CardNumberType",
    "NetworkTokenType",
];

/// Parse all relevant *.proto files and return a map of
/// `{MessageName → MessageSchema}`.
///
/// Searches for the proto directory using the same strategy as
/// `parse_services_proto` (repo-relative path first, then CARGO_MANIFEST_DIR).
pub fn parse_message_schemas() -> BTreeMap<String, MessageSchema> {
    let proto_dirs = [
        "crates/types-traits/grpc-api-types/proto",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../types-traits/grpc-api-types/proto"
        ),
    ];

    for dir in &proto_dirs {
        let path = Path::new(dir);
        if path.is_dir() {
            let schemas = parse_proto_dir(path);
            eprintln!(
                "Extracted field schemas for {} message types from {:?}",
                schemas.len(),
                path
            );
            return schemas;
        }
    }

    eprintln!("WARNING: Could not find proto directory for message schema extraction");
    BTreeMap::new()
}

fn parse_proto_dir(dir: &Path) -> BTreeMap<String, MessageSchema> {
    // Parse these files; order doesn't matter since types are merged into one map.
    let files = [
        "payment.proto",
        "payment_methods.proto",
        "services.proto",
        "sdk_config.proto",
    ];

    let mut all: BTreeMap<String, MessageSchema> = BTreeMap::new();
    for file in &files {
        let path = dir.join(file);
        if let Ok(content) = std::fs::read_to_string(&path) {
            all.extend(parse_proto_messages(&content));
        }
    }
    all
}

/// Line-by-line state-machine parser: extracts `{MessageName → MessageSchema}`
/// from a single proto file's content.
fn parse_proto_messages(content: &str) -> BTreeMap<String, MessageSchema> {
    let mut schemas: BTreeMap<String, MessageSchema> = BTreeMap::new();

    // Each entry: "message" | "oneof" | "enum"
    let mut context_stack: Vec<&'static str> = Vec::new();
    // Innermost message name at each nesting level
    let mut msg_path: Vec<String> = Vec::new();
    // Accumulated leading comment lines (reset after each field or blank line)
    let mut pending_comment = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // ── Closing brace ────────────────────────────────────────────────────
        if trimmed == "}" {
            if let Some(kind) = context_stack.pop() {
                if kind == "message" {
                    msg_path.pop();
                }
            }
            pending_comment.clear();
            continue;
        }

        // ── Message definition ───────────────────────────────────────────────
        if trimmed.starts_with("message ") && trimmed.ends_with('{') {
            let name = trimmed
                .strip_prefix("message ")
                .unwrap_or("")
                .trim_end_matches('{')
                .trim()
                .to_string();
            context_stack.push("message");
            msg_path.push(name);
            pending_comment.clear();
            continue;
        }

        // ── Oneof block ──────────────────────────────────────────────────────
        // Fields inside oneof belong to the enclosing message — don't push to msg_path.
        if trimmed.starts_with("oneof ") && trimmed.ends_with('{') {
            context_stack.push("oneof");
            pending_comment.clear();
            continue;
        }

        // ── Enum block ───────────────────────────────────────────────────────
        if trimmed.starts_with("enum ") && trimmed.ends_with('{') {
            context_stack.push("enum");
            pending_comment.clear();
            continue;
        }

        // ── Comment line ─────────────────────────────────────────────────────
        if trimmed.starts_with("//") {
            let text = trimmed.trim_start_matches('/').trim();
            // Skip decoration lines (====, ----, file-level banner rows)
            let is_decoration =
                text.is_empty() || text.chars().all(|c| matches!(c, '=' | '-' | ' '));
            if !is_decoration {
                if !pending_comment.is_empty() {
                    pending_comment.push(' ');
                }
                pending_comment.push_str(text);
            }
            continue;
        }

        // ── Field definition ─────────────────────────────────────────────────
        // Enum values match the field regex but live inside enum blocks — skip them.
        let in_enum = context_stack.last() == Some(&"enum");
        if !in_enum {
            if let Some((type_name, field_name, inline_comment)) = parse_field_line(trimmed) {
                if let Some(current_msg) = msg_path.last() {
                    let comment = if !inline_comment.is_empty() {
                        inline_comment
                    } else {
                        pending_comment.clone()
                    };

                    let schema = schemas.entry(current_msg.clone()).or_default();

                    if !comment.is_empty() {
                        schema.comments.insert(field_name.clone(), comment);
                    }

                    // Only record message-type fields (needed for recursive annotation)
                    let is_msg_type = type_name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                        && !SCALAR_TYPES.contains(&type_name.as_str())
                        && !type_name.starts_with("map<");

                    if is_msg_type {
                        schema.field_types.insert(field_name, type_name);
                    }
                }
                pending_comment.clear();
                continue;
            }
        }

        // ── Blank / other lines ──────────────────────────────────────────────
        if trimmed.is_empty() {
            pending_comment.clear();
        }
    }

    schemas
}

/// Extract `(type_name, field_name, inline_comment)` from a single proto field line.
///
/// Handles:
///   `[optional|repeated] TYPE FIELD_NAME = NUMBER [options] ;  [// comment]`
///   `map<K, V> FIELD_NAME = NUMBER ;  [// comment]`
///
/// Returns `None` for enum values, service/rpc lines, reserved statements, etc.
fn parse_field_line(trimmed: &str) -> Option<(String, String, String)> {
    // Strip optional / repeated modifier
    let s = trimmed
        .strip_prefix("optional ")
        .or_else(|| trimmed.strip_prefix("repeated "))
        .unwrap_or(trimmed);

    // Must contain a semicolon
    let semi_pos = s.find(';')?;
    let before_semi = s[..semi_pos].trim();
    let after_semi = s[semi_pos + 1..].trim();

    // Inline comment comes after the semicolon
    let inline_comment = after_semi
        .strip_prefix("//")
        .map(|c| c.trim().to_string())
        .unwrap_or_default();

    // before_semi: "TYPE FIELD_NAME = NUMBER [options]"
    // rfind('=') locates the field-number assignment
    let eq_pos = before_semi.rfind('=')?;

    // The digit right after '=' confirms this is a field-number assignment,
    // not some option expression that happens to contain '='.
    let after_eq = before_semi[eq_pos + 1..].trim();
    if !after_eq
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        return None;
    }

    let before_eq = before_semi[..eq_pos].trim();

    // The last whitespace-separated token is the field name
    let field_name = before_eq.split_whitespace().last()?.to_string();
    if !field_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    // Everything before the field name is the type
    let type_end = before_eq.len().saturating_sub(field_name.len());
    let type_name = before_eq[..type_end].trim().to_string();

    // Enum values have no leading type token — their "type_name" would be empty
    if type_name.is_empty() {
        return None;
    }

    Some((type_name, field_name, inline_comment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_message_names() {
        let rpc_line = "rpc Authorize(PaymentServiceAuthorizeRequest) returns (PaymentServiceAuthorizeResponse);";
        let (req, resp) = extract_message_names(rpc_line);
        assert_eq!(req, "PaymentServiceAuthorizeRequest");
        assert_eq!(resp, "PaymentServiceAuthorizeResponse");
    }

    #[test]
    fn test_clean_description() {
        assert_eq!(clean_description("Test description"), "Test description.");
        assert_eq!(clean_description("Test description."), "Test description.");
        assert_eq!(
            clean_description("Test // description"),
            "Test description."
        );
    }

    #[test]
    fn test_get_category_for_service() {
        assert_eq!(get_category_for_service("PaymentService"), "Payments");
        assert_eq!(get_category_for_service("RefundService"), "Refunds");
        assert_eq!(get_category_for_service("UnknownService"), "Other");
    }

    #[test]
    fn test_flow_key_mapping_exists() {
        let mapping = get_flow_key_mapping();
        // Verify essential mappings exist
        assert!(mapping.contains_key(&("PaymentService", "Authorize")));
        assert!(mapping.contains_key(&("PaymentService", "Capture")));
        assert!(mapping.contains_key(&("PaymentService", "Get")));
        assert!(mapping.contains_key(&("RefundService", "Get")));
    }
}
