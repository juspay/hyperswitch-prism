//! Build script - dynamically discovers flows from FFI source
//!
//! This script:
//! 1. Parses crates/ffi/ffi/src/services/payments.rs for req_transformer! macros
//! 2. Extracts function_name and request_type
//! 3. Maps request_type to service/rpc by naming convention
//! 4. Generates flow_runners.rs with all probe functions

// Build scripts are allowed to use expect/unwrap for simplicity
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../../ffi/ffi/src/services/payments.rs");
    println!("cargo:rerun-if-changed=../../types-traits/grpc-api-types/proto/services.proto");

    let flows = discover_flows_from_ffi();
    generate_flow_runners(&flows);

    println!("cargo:info=Discovered {} flows from FFI", flows.len());
}

#[derive(Debug, Clone)]
struct FlowInfo {
    key: String,
    service: String,
    rpc: String,
    request_type: String,
    transformer_fn: String,
    needs_oauth: bool,
    needs_feature_data: bool,
    /// Whether this flow has a `connector_transaction_id` field that may need overriding.
    needs_transaction_id: bool,
}

fn discover_flows_from_ffi() -> Vec<FlowInfo> {
    let ffi_path = Path::new("../../ffi/ffi/src/services/payments.rs");
    let content = fs::read_to_string(ffi_path).expect("Failed to read FFI payments.rs");

    let mut flows = Vec::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim().starts_with("req_transformer!(") {
            let mut fn_name = None;
            let mut request_type = None;

            for line in lines.by_ref() {
                let trimmed = line.trim();

                if trimmed.starts_with("fn_name:") {
                    if let Some(name) = trimmed.split(':').nth(1) {
                        fn_name = Some(name.trim().trim_end_matches(',').to_string());
                    }
                } else if trimmed.starts_with("request_type:") {
                    if let Some(req) = trimmed.split(':').nth(1) {
                        request_type = Some(req.trim().trim_end_matches(',').to_string());
                    }
                } else if trimmed.starts_with(");") || trimmed == ");" {
                    break;
                }
            }

            if let (Some(fn_name), Some(request_type)) = (fn_name, request_type) {
                // Skip authorize - it's handled specially with payment methods
                if fn_name == "authorize_req_transformer" {
                    continue;
                }
                if let Some(flow) = parse_flow_info(&fn_name, &request_type) {
                    flows.push(flow);
                }
            }
        }
    }

    flows
}

fn parse_flow_info(transformer_fn: &str, request_type: &str) -> Option<FlowInfo> {
    // Skip webhooks and events
    if request_type.contains("Webhook") || request_type.contains("Event") {
        return None;
    }

    if !request_type.ends_with("Request") {
        return None;
    }

    let base = &request_type[..request_type.len() - 7];

    // Find service boundary
    let service_end = if let Some(pos) = base.find("PaymentService") {
        if pos == 0 {
            "PaymentService".len()
        } else {
            pos + "PaymentService".len()
        }
    } else if let Some(pos) = base.find("MethodService") {
        pos + "MethodService".len()
    } else if let Some(pos) = base.rfind("Service") {
        pos + 7
    } else {
        return None;
    };

    let service = &base[..service_end];
    let rpc = &base[service_end..];

    let key = transformer_fn
        .trim_end_matches("_req_transformer")
        .to_string();

    let (final_key, final_rpc) = match key.as_str() {
        "get" => ("get".to_string(), "Get".to_string()),
        "create" => ("create_customer".to_string(), "Create".to_string()),
        "charge" => ("recurring_charge".to_string(), "Charge".to_string()),
        "accept" => ("dispute_accept".to_string(), "Accept".to_string()),
        "defend" => ("dispute_defend".to_string(), "Defend".to_string()),
        "submit_evidence" => (
            "dispute_submit_evidence".to_string(),
            "SubmitEvidence".to_string(),
        ),
        _ => (key, rpc.to_string()),
    };

    let needs_oauth = matches!(
        final_key.as_str(),
        "authorize"
            | "capture"
            | "void"
            | "get"
            | "refund"
            | "create_order"
            | "setup_recurring"
            | "recurring_charge"
            | "proxy_authorize"
            | "token_authorize"
            | "proxy_setup_recurring"
            | "token_setup_recurring"
    );

    // Some flows don't have a connector_feature_data field in their request type
    let needs_feature_data = !matches!(
        final_key.as_str(),
        "server_authentication_token"
            | "server_session_authentication_token"
            | "dispute_accept"
            | "dispute_submit_evidence"
            | "dispute_defend"
            | "proxy_setup_recurring"
    );

    // Flows whose base request has a connector_transaction_id that some connectors
    // need to be a numeric string (e.g. Zift/Placetopay parse it as i64/u64).
    let needs_transaction_id = matches!(
        final_key.as_str(),
        "capture" | "void" | "get" | "refund" | "reverse"
    );

    Some(FlowInfo {
        key: final_key,
        service: service.to_string(),
        rpc: final_rpc,
        request_type: request_type.to_string(),
        transformer_fn: transformer_fn.to_string(),
        needs_oauth,
        needs_feature_data,
        needs_transaction_id,
    })
}

fn generate_flow_runners(flows: &[FlowInfo]) {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("flow_runners_generated.rs");
    let mut f = fs::File::create(&dest_path).expect("Failed to create generated file");

    writeln!(
        f,
        "// ============================================================================"
    )
    .unwrap();
    writeln!(f, "// GENERATED FILE - DO NOT EDIT MANUALLY").unwrap();
    writeln!(
        f,
        "// Generated by build.rs from FFI req_transformer! macros"
    )
    .unwrap();
    writeln!(
        f,
        "// ============================================================================"
    )
    .unwrap();
    writeln!(f).unwrap();

    // Start module
    writeln!(f, "pub mod generated {{").unwrap();
    writeln!(f, "    #![allow(clippy::all)]").unwrap();
    writeln!(f).unwrap();

    // Imports inside module
    writeln!(f, "    use std::sync::Arc;").unwrap();
    writeln!(f, "    use common_utils::metadata::MaskedMetadata;").unwrap();
    writeln!(f, "    use domain_types::{{connector_types::ConnectorEnum, router_data::ConnectorSpecificConfig}};").unwrap();
    writeln!(f, "    use grpc_api_types::payments::PaymentMethod;").unwrap();
    writeln!(f, "    use hyperswitch_masking::Secret;").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "    use crate::config::*;").unwrap();
    writeln!(f, "    use crate::error_parsing::*;").unwrap();
    writeln!(f, "    use crate::probe_engine::*;").unwrap();
    writeln!(f, "    use crate::registry::mock_connector_state;").unwrap();
    writeln!(f, "    use crate::requests::*;").unwrap();
    writeln!(f, "    use crate::types::*;").unwrap();
    writeln!(f, "    use crate::patcher::smart_patch;").unwrap();
    writeln!(f).unwrap();

    // Generate probe functions
    for flow in flows {
        generate_probe_function(&mut f, flow);
    }

    // Generate authorize with PM
    generate_authorize_probe(&mut f);

    // Generate FLOW_DEFINITIONS
    generate_flow_definitions(&mut f, flows);

    // Generate dispatcher
    generate_dispatcher(&mut f, flows);

    // Close module
    writeln!(f, "}}").unwrap();
}

fn generate_probe_function(f: &mut fs::File, flow: &FlowInfo) {
    // Map flow keys to request builder function names
    let base_builder = match flow.key.as_str() {
        "dispute_accept" => "base_accept_dispute_request".to_string(),
        "dispute_submit_evidence" => "base_submit_evidence_request".to_string(),
        "dispute_defend" => "base_defend_dispute_request".to_string(),
        "recurring_charge" => "base_recurring_charge_request".to_string(),
        // Tokenized payment service flows
        "token_authorize" => "base_tokenized_authorize_request".to_string(),
        "token_setup_recurring" => "base_tokenized_setup_recurring_request".to_string(),
        // Proxied payment service flows
        "proxy_authorize" => "base_proxied_authorize_request".to_string(),
        "proxy_setup_recurring" => "base_proxied_setup_recurring_request".to_string(),
        _ => format!("base_{}_request", flow.key),
    };

    writeln!(
        f,
        "    /// Probe flow: {} ({}::{})",
        flow.key, flow.service, flow.rpc
    )
    .unwrap();
    writeln!(f, "    pub fn probe_{}(", flow.key).unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();

    // Only use mut if we need to modify the request
    let needs_mut = flow.needs_oauth || flow.needs_feature_data || flow.needs_transaction_id;
    if needs_mut {
        writeln!(f, "        let mut req = {}();", base_builder).unwrap();
    } else {
        writeln!(f, "        let req = {}();", base_builder).unwrap();
    }

    if flow.needs_transaction_id {
        writeln!(
            f,
            "        if let Some(txn_id) = connector_transaction_id_override(connector) {{"
        )
        .unwrap();
        writeln!(f, "            req.connector_transaction_id = txn_id;").unwrap();
        writeln!(f, "        }}").unwrap();
    }

    if flow.needs_oauth {
        writeln!(f, "        if is_oauth_connector(connector) {{").unwrap();
        writeln!(
            f,
            "            req.state = Some(mock_connector_state(Some(connector)));"
        )
        .unwrap();
        writeln!(f, "        }}").unwrap();
    }

    if flow.needs_feature_data {
        writeln!(
            f,
            "        if let Some(meta) = connector_feature_data_json(connector) {{"
        )
        .unwrap();
        writeln!(
            f,
            "            req.connector_feature_data = Some(Secret::new(meta));"
        )
        .unwrap();
        writeln!(f, "        }}").unwrap();
    }

    writeln!(f, "        run_probe(").unwrap();
    writeln!(f, "            \"{}\",", flow.key).unwrap();
    writeln!(f, "            req,").unwrap();
    writeln!(f, "            |req| {{").unwrap();
    writeln!(
        f,
        "                ffi::services::payments::{}::<PciFfi>(",
        flow.transformer_fn
    )
    .unwrap();
    writeln!(f, "                    req,").unwrap();
    writeln!(f, "                    config,").unwrap();
    writeln!(f, "                    connector.clone(),").unwrap();
    writeln!(f, "                    auth.clone(),").unwrap();
    writeln!(f, "                    metadata,").unwrap();
    writeln!(f, "                )").unwrap();
    writeln!(f, "            }},").unwrap();
    writeln!(f, "            |req, field| {{").unwrap();
    writeln!(
        f,
        "                smart_patch(req, \"{}\", field);",
        flow.key
    )
    .unwrap();
    writeln!(f, "            }},").unwrap();
    writeln!(f, "        )").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();
}

fn generate_authorize_probe(f: &mut fs::File) {
    writeln!(f, "    /// Probe authorize with payment method").unwrap();
    writeln!(f, "    pub fn probe_authorize(").unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        pm_name: &str,").unwrap();
    writeln!(f, "        payment_method: PaymentMethod,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();
    writeln!(f, "        let _ = pm_name; // Unused, for API consistency").unwrap();
    writeln!(
        f,
        "        let connector_meta = connector_feature_data_json(connector);"
    )
    .unwrap();
    writeln!(f, "        let req = if is_oauth_connector(connector) {{").unwrap();
    writeln!(f, "            base_authorize_request_with_state(payment_method, connector_meta, mock_connector_state(Some(connector)))").unwrap();
    writeln!(f, "        }} else {{").unwrap();
    writeln!(
        f,
        "            base_authorize_request_with_meta(payment_method, connector_meta)"
    )
    .unwrap();
    writeln!(f, "        }};").unwrap();
    writeln!(f, "        run_probe(").unwrap();
    writeln!(f, "            \"authorize\",").unwrap();
    writeln!(f, "            req,").unwrap();
    writeln!(f, "            |req| {{").unwrap();
    writeln!(
        f,
        "                ffi::services::payments::authorize_req_transformer::<PciFfi>("
    )
    .unwrap();
    writeln!(f, "                    req,").unwrap();
    writeln!(f, "                    config,").unwrap();
    writeln!(f, "                    connector.clone(),").unwrap();
    writeln!(f, "                    auth.clone(),").unwrap();
    writeln!(f, "                    metadata,").unwrap();
    writeln!(f, "                )").unwrap();
    writeln!(f, "            }},").unwrap();
    writeln!(f, "            |req, field| {{").unwrap();
    writeln!(f, "                smart_patch(req, \"authorize\", field);").unwrap();
    writeln!(f, "            }},").unwrap();
    writeln!(f, "        )").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();
}

fn generate_flow_definitions(f: &mut fs::File, flows: &[FlowInfo]) {
    writeln!(f, "    /// Flow definition for registry").unwrap();
    writeln!(f, "    #[allow(dead_code)]").unwrap();
    writeln!(f, "    pub struct FlowDefinition {{").unwrap();
    writeln!(f, "        pub key: &'static str,").unwrap();
    writeln!(f, "        pub service: &'static str,").unwrap();
    writeln!(f, "        pub rpc: &'static str,").unwrap();
    writeln!(f, "        pub request_type: &'static str,").unwrap();
    writeln!(f, "        pub transformer_fn: &'static str,").unwrap();
    writeln!(f, "        pub has_payment_methods: bool,").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();

    writeln!(f, "    /// All discovered flows from FFI").unwrap();
    writeln!(f, "    pub const FLOW_DEFINITIONS: &[FlowDefinition] = &[").unwrap();

    // Authorize is special
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"authorize\",").unwrap();
    writeln!(f, "            service: \"PaymentService\",").unwrap();
    writeln!(f, "            rpc: \"Authorize\",").unwrap();
    writeln!(
        f,
        "            request_type: \"PaymentServiceAuthorizeRequest\","
    )
    .unwrap();
    writeln!(
        f,
        "            transformer_fn: \"authorize_req_transformer\","
    )
    .unwrap();
    writeln!(f, "            has_payment_methods: true,").unwrap();
    writeln!(f, "        }},").unwrap();

    for flow in flows {
        writeln!(f, "        FlowDefinition {{").unwrap();
        writeln!(f, "            key: \"{}\",", flow.key).unwrap();
        writeln!(f, "            service: \"{}\",", flow.service).unwrap();
        writeln!(f, "            rpc: \"{}\",", flow.rpc).unwrap();
        writeln!(f, "            request_type: \"{}\",", flow.request_type).unwrap();
        writeln!(
            f,
            "            transformer_fn: \"{}\",",
            flow.transformer_fn
        )
        .unwrap();
        writeln!(f, "            has_payment_methods: false,").unwrap();
        writeln!(f, "        }},").unwrap();
    }

    writeln!(f, "    ];").unwrap();
    writeln!(f).unwrap();
}

fn generate_dispatcher(f: &mut fs::File, flows: &[FlowInfo]) {
    writeln!(f, "    /// Dispatch to appropriate probe function").unwrap();
    writeln!(f, "    pub fn dispatch_probe(").unwrap();
    writeln!(f, "        key: &str,").unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> Option<FlowResult> {{").unwrap();
    writeln!(f, "        match key {{").unwrap();

    for flow in flows {
        writeln!(
            f,
            "            \"{}\" => Some(probe_{}(connector, config, auth, metadata)),",
            flow.key, flow.key
        )
        .unwrap();
    }

    writeln!(f, "            _ => None,").unwrap();
    writeln!(f, "        }}").unwrap();
    writeln!(f, "    }}").unwrap();
}
