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
                // Skip req_handler and res_handler variants - they're handled by the base transformer
                if fn_name.contains("_req_handler") || fn_name.contains("_res_handler") {
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
            | "incremental_authorization"
            | "refund_get"
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
            | "refund_get"
            | "recurring_revoke"
    );

    Some(FlowInfo {
        key: final_key,
        service: service.to_string(),
        rpc: final_rpc,
        request_type: request_type.to_string(),
        transformer_fn: transformer_fn.to_string(),
        needs_oauth,
        needs_feature_data,
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
    writeln!(
        f,
        "    use crate::patcher::{{smart_patch, apply_connector_flow_overrides}};"
    )
    .unwrap();
    writeln!(f).unwrap();

    // Generate probe functions
    for flow in flows {
        generate_probe_function(&mut f, flow);
    }

    // Generate authorize with PM
    generate_authorize_probe(&mut f);

    // Generate parse_event probe (custom signature, not a req_transformer!)
    generate_parse_event_probe(&mut f);

    // Generate handle_event probe (custom signature, not a req_transformer!)
    generate_handle_event_probe(&mut f);

    // Generate verify_redirect probe (bespoke, different return type)
    generate_verify_redirect_probe(&mut f);

    // Generate not_implemented probes
    generate_not_implemented_probe(&mut f, "dispute_get", "DisputeService", "Get");
    generate_not_implemented_probe(&mut f, "eligibility", "PaymentMethodService", "Eligibility");

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

    // Always mut: connector_flow_overrides may pre-patch any flow
    writeln!(f, "        let mut req = {}();", base_builder).unwrap();

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

    writeln!(
        f,
        "        apply_connector_flow_overrides(&mut req, connector, \"{}\");",
        flow.key
    )
    .unwrap();
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
    writeln!(
        f,
        "        let mut req = if is_oauth_connector(connector) {{"
    )
    .unwrap();
    writeln!(f, "            base_authorize_request_with_state(payment_method, connector_meta, mock_connector_state(Some(connector)))").unwrap();
    writeln!(f, "        }} else {{").unwrap();
    writeln!(
        f,
        "            base_authorize_request_with_meta(payment_method, connector_meta)"
    )
    .unwrap();
    writeln!(f, "        }};").unwrap();
    writeln!(
        f,
        "        apply_connector_flow_overrides(&mut req, connector, \"authorize\");"
    )
    .unwrap();
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

fn generate_parse_event_probe(f: &mut fs::File) {
    writeln!(f, "    /// Probe parse_event (EventService::ParseEvent).").unwrap();
    writeln!(f, "    /// Stateless — no auth required.").unwrap();
    writeln!(f, "    pub fn probe_parse_event(").unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();
    writeln!(f, "        let mut req = base_parse_event_request();").unwrap();
    writeln!(
        f,
        "        req.request_details = req.request_details.map(|mut rd| {{ rd.body = ffi::services::payments::get_webhook_sample_body(connector.clone()).to_vec(); rd }});"
    )
    .unwrap();
    writeln!(
        f,
        "        match ffi::services::payments::parse_event_transformer(req, config, connector.clone(), Some(auth), metadata) {{"
    )
    .unwrap();
    writeln!(
        f,
        "            Ok(_) => FlowResult {{ status: \"supported\".to_string(), ..Default::default() }},"
    )
    .unwrap();
    writeln!(f, "            Err(e) => {{").unwrap();
    writeln!(f, "                let msg = e.error_message;").unwrap();
    writeln!(f, "                if is_not_implemented(&msg) {{").unwrap();
    writeln!(
        f,
        "                    FlowResult {{ status: \"not_implemented\".to_string(), ..Default::default() }}"
    )
    .unwrap();
    writeln!(f, "                }} else {{").unwrap();
    writeln!(
        f,
        "                    // Non-NotImplemented error means connector has a handler but fake body failed — treat as supported."
    )
    .unwrap();
    writeln!(
        f,
        "                    FlowResult {{ status: \"supported\".to_string(), ..Default::default() }}"
    )
    .unwrap();
    writeln!(f, "                }}").unwrap();
    writeln!(f, "            }}").unwrap();
    writeln!(f, "        }}").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();
}

fn generate_handle_event_probe(f: &mut fs::File) {
    writeln!(f, "    /// Probe handle_event (EventService::HandleEvent).").unwrap();
    writeln!(
        f,
        "    /// Uses a bespoke approach: handle_event_transformer has a different signature"
    )
    .unwrap();
    writeln!(
        f,
        "    /// (returns EventServiceHandleResponse, not Option<Request>) so it cannot use run_probe."
    )
    .unwrap();
    writeln!(f, "    pub fn probe_handle_event(").unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();
    writeln!(f, "        let mut req = base_handle_event_request();").unwrap();
    writeln!(
        f,
        "        req.request_details = req.request_details.map(|mut rd| {{ rd.body = ffi::services::payments::get_webhook_sample_body(connector.clone()).to_vec(); rd }});"
    )
    .unwrap();
    writeln!(
        f,
        "        match ffi::services::payments::handle_event_transformer(req, config, connector.clone(), Some(auth), metadata) {{"
    )
    .unwrap();
    writeln!(
        f,
        "            Ok(_) => FlowResult {{ status: \"supported\".to_string(), ..Default::default() }},"
    )
    .unwrap();
    writeln!(f, "            Err(e) => {{").unwrap();
    writeln!(f, "                let msg = e.error_message;").unwrap();
    writeln!(f, "                if is_not_implemented(&msg) {{").unwrap();
    writeln!(
        f,
        "                    FlowResult {{ status: \"not_implemented\".to_string(), ..Default::default() }}"
    )
    .unwrap();
    writeln!(f, "                }} else {{").unwrap();
    writeln!(
        f,
        "                    // Any non-NotImplemented error means the connector HAS a webhook handler"
    )
    .unwrap();
    writeln!(
        f,
        "                    // but our fake body failed to parse — treat as supported."
    )
    .unwrap();
    writeln!(
        f,
        "                    FlowResult {{ status: \"supported\".to_string(), ..Default::default() }}"
    )
    .unwrap();
    writeln!(f, "                }}").unwrap();
    writeln!(f, "            }}").unwrap();
    writeln!(f, "        }}").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();
}

fn generate_verify_redirect_probe(f: &mut fs::File) {
    writeln!(
        f,
        "    /// Probe verify_redirect (PaymentService::VerifyRedirectResponse)."
    )
    .unwrap();
    writeln!(f, "    /// Bespoke: calls verify_redirect_response_transformer which has a different return type.").unwrap();
    writeln!(f, "    pub fn probe_verify_redirect(").unwrap();
    writeln!(f, "        connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();
    writeln!(f, "        let req = base_verify_redirect_request();").unwrap();
    writeln!(f, "        match ffi::services::payments::verify_redirect_response_transformer(req, config, connector.clone(), auth, metadata) {{").unwrap();
    writeln!(f, "            Ok(_) => FlowResult {{ status: \"supported\".to_string(), ..Default::default() }},").unwrap();
    writeln!(f, "            Err(e) => {{").unwrap();
    writeln!(f, "                let msg = e.error_message;").unwrap();
    writeln!(f, "                if is_not_implemented(&msg) {{").unwrap();
    writeln!(f, "                    FlowResult {{ status: \"not_implemented\".to_string(), ..Default::default() }}").unwrap();
    writeln!(f, "                }} else {{").unwrap();
    writeln!(f, "                    FlowResult {{ status: \"not_supported\".to_string(), error: Some(msg), ..Default::default() }}").unwrap();
    writeln!(f, "                }}").unwrap();
    writeln!(f, "            }}").unwrap();
    writeln!(f, "        }}").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f).unwrap();
}

fn generate_not_implemented_probe(f: &mut fs::File, key: &str, _service: &str, _rpc: &str) {
    writeln!(
        f,
        "    /// Probe {} — not yet implemented at the connector layer.",
        key
    )
    .unwrap();
    writeln!(f, "    pub fn probe_{}(", key).unwrap();
    writeln!(f, "        _connector: &ConnectorEnum,").unwrap();
    writeln!(f, "        _config: &Arc<ucs_env::configs::Config>,").unwrap();
    writeln!(f, "        _auth: ConnectorSpecificConfig,").unwrap();
    writeln!(f, "        _metadata: &MaskedMetadata,").unwrap();
    writeln!(f, "    ) -> FlowResult {{").unwrap();
    writeln!(
        f,
        "        FlowResult {{ status: \"not_implemented\".to_string(), ..Default::default() }}"
    )
    .unwrap();
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

    // parse_event is special — standalone function, not a req_transformer! macro
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"parse_event\",").unwrap();
    writeln!(f, "            service: \"EventService\",").unwrap();
    writeln!(f, "            rpc: \"ParseEvent\",").unwrap();
    writeln!(f, "            request_type: \"EventServiceParseRequest\",").unwrap();
    writeln!(
        f,
        "            transformer_fn: \"parse_event_transformer\","
    )
    .unwrap();
    writeln!(f, "            has_payment_methods: false,").unwrap();
    writeln!(f, "        }},").unwrap();

    // handle_event is special — standalone function, not a req_transformer! macro
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"handle_event\",").unwrap();
    writeln!(f, "            service: \"EventService\",").unwrap();
    writeln!(f, "            rpc: \"HandleEvent\",").unwrap();
    writeln!(
        f,
        "            request_type: \"EventServiceHandleRequest\","
    )
    .unwrap();
    writeln!(
        f,
        "            transformer_fn: \"handle_event_transformer\","
    )
    .unwrap();
    writeln!(f, "            has_payment_methods: false,").unwrap();
    writeln!(f, "        }},").unwrap();

    // verify_redirect
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"verify_redirect\",").unwrap();
    writeln!(f, "            service: \"PaymentService\",").unwrap();
    writeln!(f, "            rpc: \"VerifyRedirectResponse\",").unwrap();
    writeln!(
        f,
        "            request_type: \"PaymentServiceVerifyRedirectResponseRequest\","
    )
    .unwrap();
    writeln!(
        f,
        "            transformer_fn: \"verify_redirect_transformer\","
    )
    .unwrap();
    writeln!(f, "            has_payment_methods: false,").unwrap();
    writeln!(f, "        }},").unwrap();

    // dispute_get
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"dispute_get\",").unwrap();
    writeln!(f, "            service: \"DisputeService\",").unwrap();
    writeln!(f, "            rpc: \"Get\",").unwrap();
    writeln!(f, "            request_type: \"DisputeServiceGetRequest\",").unwrap();
    writeln!(f, "            transformer_fn: \"none\",").unwrap();
    writeln!(f, "            has_payment_methods: false,").unwrap();
    writeln!(f, "        }},").unwrap();

    // eligibility
    writeln!(f, "        FlowDefinition {{").unwrap();
    writeln!(f, "            key: \"eligibility\",").unwrap();
    writeln!(f, "            service: \"PaymentMethodService\",").unwrap();
    writeln!(f, "            rpc: \"Eligibility\",").unwrap();
    writeln!(
        f,
        "            request_type: \"PayoutMethodEligibilityRequest\","
    )
    .unwrap();
    writeln!(f, "            transformer_fn: \"none\",").unwrap();
    writeln!(f, "            has_payment_methods: false,").unwrap();
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

    // parse_event and handle_event use bespoke probes, not the standard req_transformer path
    writeln!(
        f,
        "            \"parse_event\" => Some(probe_parse_event(connector, config, auth, metadata)),"
    )
    .unwrap();
    writeln!(
        f,
        "            \"handle_event\" => Some(probe_handle_event(connector, config, auth, metadata)),"
    )
    .unwrap();

    writeln!(f, "            \"verify_redirect\" => Some(probe_verify_redirect(connector, config, auth, metadata)),").unwrap();
    writeln!(f, "            \"dispute_get\" => Some(probe_dispute_get(connector, config, auth, metadata)),").unwrap();
    writeln!(f, "            \"eligibility\" => Some(probe_eligibility(connector, config, auth, metadata)),").unwrap();

    writeln!(f, "            _ => None,").unwrap();
    writeln!(f, "        }}").unwrap();
    writeln!(f, "    }}").unwrap();
}
