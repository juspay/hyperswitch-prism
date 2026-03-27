use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Type alias for gRPC module tuple to avoid complex type warning
/// (connector_name, Vec<(flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth)>)
type GrpcModule<'a> = (
    String,
    Vec<(&'a str, &'a str, &'a str, &'a str, bool, bool)>,
);

/// Extracts the content of a JSON object block starting at the first `{` in `text`.
/// Returns (block_content_without_outer_braces, length_consumed).
fn extract_brace_block(text: &str) -> Option<(&str, usize)> {
    let brace_pos = text.find('{')?;
    let block = &text[brace_pos..];
    let mut depth = 0usize;
    for (i, ch) in block.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&block[1..i], brace_pos + i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Reads data/field_probe/{connector}.json and returns the set of flow keys
/// that have at least one variant with status == "supported". Returns None if no probe file.
///
/// Uses a brace-counting JSON scanner (no external JSON dependency) to stay
/// within the "flows" section and avoid false matches inside proto_request payloads.
fn load_supported_flows(examples_dir: &Path, connector: &str) -> Option<HashSet<String>> {
    let probe_file = examples_dir
        .join("..")
        .join("data")
        .join("field_probe")
        .join(format!("{connector}.json"));
    if !probe_file.exists() {
        return None;
    }
    println!("cargo:rerun-if-changed={}", probe_file.display());
    let text = fs::read_to_string(&probe_file).ok()?;

    // Locate the "flows": { ... } section.
    let flows_marker = "\"flows\":";
    let flows_start = text.find(flows_marker)?;
    let (flows_block, _) = extract_brace_block(&text[flows_start + flows_marker.len()..])?;

    // Within the flows block, each top-level key is a flow name.
    // Scan by finding `"flow_key":` patterns and extracting the variants block.
    let all_flow_keys = [
        "authorize",
        "capture",
        "void",
        "get",
        "refund",
        "reverse",
        "create_customer",
        "tokenize",
        "setup_recurring",
        "recurring_charge",
        "pre_authenticate",
        "authenticate",
        "post_authenticate",
        "handle_event",
        "create_access_token",
        "create_session_token",
        "create_sdk_session_token",
    ];
    let mut supported = HashSet::new();
    for flow_key in all_flow_keys {
        // Match `"flow_key":` as a JSON key within the flows block.
        let key_pattern = format!("\"{}\":", flow_key);
        if let Some(pos) = flows_block.find(&key_pattern) {
            let after_colon = &flows_block[pos + key_pattern.len()..];
            // Extract the variants block { "Ach": { ... }, ... }
            if let Some((variants_block, _)) = extract_brace_block(after_colon) {
                if variants_block.contains("\"supported\"") {
                    supported.insert(flow_key.to_string());
                }
            }
        }
    }
    Some(supported)
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let examples_dir = Path::new(&manifest_dir).join("../../../examples");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let connectors_path = format!("{out_dir}/connectors.rs");
    let scenarios_path = format!("{out_dir}/connector_scenarios.rs");
    let grpc_scenarios_path = format!("{out_dir}/grpc_scenarios.rs");
    let grpc_helpers_path = format!("{out_dir}/grpc_helpers.rs");

    // Known FFI scenario function names (subset may be present in any connector).
    let all_scenarios: &[(&str, &str)] = &[
        ("checkout_autocapture", "process_checkout_autocapture"),
        ("checkout_card", "process_checkout_card"),
        ("checkout_wallet", "process_checkout_wallet"),
        ("checkout_bank", "process_checkout_bank"),
        ("refund", "process_refund"),
        ("recurring", "process_recurring"),
        ("void_payment", "process_void_payment"),
        ("get_payment", "process_get_payment"),
        ("create_customer", "process_create_customer"),
        ("tokenize", "process_tokenize"),
        ("authentication", "process_authentication"),
    ];

    // Flow metadata for gRPC dispatch via builder functions.
    // Fields: (flow_key, grpc_field, grpc_method, builder_fn, needs_txn_id, self_auth)
    //   needs_txn_id : receives connector_transaction_id from the shared AUTOMATIC authorize pre-run
    //   self_auth    : must run its own MANUAL authorize inline (capture, void)
    let flow_meta: &[(&str, &str, &str, &str, bool, bool)] = &[
        (
            "authorize",
            "payment",
            "authorize",
            "build_authorize_request",
            false,
            false,
        ),
        (
            "capture",
            "payment",
            "capture",
            "build_capture_request",
            false,
            true,
        ),
        ("void", "payment", "void", "build_void_request", false, true),
        ("get", "payment", "get", "build_get_request", true, false),
        (
            "refund",
            "payment",
            "refund",
            "build_refund_request",
            true,
            false,
        ),
        (
            "reverse",
            "payment",
            "reverse",
            "build_reverse_request",
            true,
            false,
        ),
        (
            "create_customer",
            "customer",
            "create",
            "build_create_customer_request",
            false,
            false,
        ),
        (
            "tokenize",
            "payment_method",
            "tokenize",
            "build_tokenize_request",
            false,
            false,
        ),
        (
            "setup_recurring",
            "payment",
            "setup_recurring",
            "build_setup_recurring_request",
            false,
            false,
        ),
        (
            "recurring_charge",
            "recurring_payment",
            "charge",
            "build_recurring_charge_request",
            false,
            false,
        ),
        (
            "pre_authenticate",
            "payment",
            "pre_authenticate",
            "build_pre_authenticate_request",
            false,
            false,
        ),
        (
            "authenticate",
            "payment",
            "authenticate",
            "build_authenticate_request",
            false,
            false,
        ),
        (
            "post_authenticate",
            "payment",
            "post_authenticate",
            "build_post_authenticate_request",
            false,
            false,
        ),
        (
            "handle_event",
            "payment",
            "handle_event",
            "build_handle_event_request",
            false,
            false,
        ),
        (
            "create_access_token",
            "payment",
            "create_access_token",
            "build_create_access_token_request",
            false,
            false,
        ),
        (
            "create_session_token",
            "payment",
            "create_session_token",
            "build_create_session_token_request",
            false,
            false,
        ),
        (
            "create_sdk_session_token",
            "payment",
            "create_sdk_session_token",
            "build_create_sdk_session_token_request",
            false,
            false,
        ),
    ];

    let allowed_connectors: Vec<String> = std::env::var("CONNECTORS")
        .ok()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| vec!["stripe".to_string()]);

    let mut modules: Vec<(String, Vec<(&'static str, &'static str)>)> = Vec::new();
    // grpc_modules: connector_name -> list of present (flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth)
    let mut grpc_modules: Vec<GrpcModule> = Vec::new();

    for connector_name in &allowed_connectors {
        let rs_file = examples_dir
            .join(connector_name)
            .join("rust")
            .join(format!("{connector_name}.rs"));
        if !rs_file.exists() {
            continue;
        }

        println!("cargo:rerun-if-changed={}", rs_file.display());
        let content = fs::read_to_string(&rs_file).unwrap_or_default();

        // Load field_probe supported flows — used as authoritative filter (same as JS smoke test).
        let field_probe_supported = load_supported_flows(&examples_dir, connector_name);
        if let Some(ref supported) = field_probe_supported {
            println!(
                "cargo:warning=field_probe[{connector_name}]: {} supported flows",
                supported.len()
            );
        }

        // Discover FFI process_* functions.
        let mut present = Vec::new();
        for (key, fn_name) in all_scenarios {
            if content.contains(&format!("pub async fn {fn_name}(")) {
                present.push((*key, *fn_name));
            }
        }
        if !present.is_empty() {
            modules.push((connector_name.clone(), present));
        }

        // Discover gRPC flows: builder function present AND flow supported in field_probe.
        // This mirrors the JS smoke test which filters by field_probe.supportedFlows.
        let mut grpc_present: Vec<(&str, &str, &str, &str, bool, bool)> = Vec::new();
        for &(flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth) in flow_meta {
            // Skip if not supported in field_probe (when probe file exists).
            if let Some(ref supported) = field_probe_supported {
                if !supported.contains(flow_key) {
                    continue;
                }
            }
            if content.contains(&format!("fn {builder_fn}(")) {
                grpc_present.push((
                    flow_key,
                    grpc_field,
                    grpc_method,
                    builder_fn,
                    needs_txn,
                    self_auth,
                ));
            }
        }
        if !grpc_present.is_empty() {
            grpc_modules.push((connector_name.clone(), grpc_present));
        }
    }

    // ── connectors.rs ─────────────────────────────────────────────────────────
    {
        let mut code = String::new();
        code.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n");
        code.push_str("// Lists all connector modules with generated process_* / build_*_request functions.\n\n");

        let mut all_names: Vec<&String> = modules.iter().map(|(n, _)| n).collect();
        for (n, _) in &grpc_modules {
            if !all_names.contains(&n) {
                all_names.push(n);
            }
        }

        for name in &all_names {
            let path = examples_dir
                .join(name.as_str())
                .join("rust")
                .join(format!("{name}.rs"));
            let canonical = path.canonicalize().unwrap_or(path.clone());
            code.push_str(&format!(
                "pub mod {name} {{\n    include!(r\"{}\");\n}}\n",
                canonical.display()
            ));
        }

        code.push_str("\n#[allow(dead_code)]\npub const ALL_CONNECTORS: &[&str] = &[\n");
        for name in &all_names {
            code.push_str(&format!("    \"{name}\",\n"));
        }
        code.push_str("];\n");
        fs::write(&connectors_path, code).unwrap();
    }

    // ── connector_scenarios.rs (FFI smoke test) ────────────────────────────────
    {
        let mut code = String::new();
        code.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n\n");
        code.push_str("match connector_name {\n");
        for (name, scenarios) in &modules {
            code.push_str(&format!("    \"{name}\" => {{\n"));
            code.push_str("        let mut results = vec![];\n");
            for (key, fn_name) in scenarios {
                code.push_str(&format!(
                    "        results.push((\"{key}\".to_string(), connectors::{name}::{fn_name}(client, &txn_id).await));\n"
                ));
            }
            code.push_str("        results\n    }\n");
        }
        code.push_str("    _ => vec![],\n}\n");
        fs::write(&scenarios_path, code).unwrap();
    }

    // ── grpc_helpers.rs (module-level async helpers for self-auth flows) ─────────
    // Included at module level in grpc_smoke_test.rs so async fn definitions are valid.
    {
        let mut helpers = String::new();
        helpers.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n\n");

        for (name, flows) in &grpc_modules {
            for &(flow_key, grpc_field, grpc_method, builder_fn, _needs_txn, self_auth) in flows {
                if !self_auth {
                    continue;
                }
                let ret = match flow_key {
                    "capture" | "void" | "reverse" =>
                        "format!(\"txn_id: {}, status_code: {}\", response.connector_transaction_id, response.status_code)".to_string(),
                    _ => "format!(\"status_code: {}\", response.status_code)".to_string(),
                };
                helpers.push_str("#[allow(dead_code)]\n");
                helpers.push_str(&format!("async fn _run_grpc_{flow_key}_{name}(\n"));
                helpers.push_str("    client: &hyperswitch_payments_client::GrpcClient,\n");
                helpers.push_str(") -> Result<String, Box<dyn std::error::Error>> {\n");
                helpers.push_str(&format!("    let auth = client.{grpc_field}.authorize(\n"));
                helpers.push_str(&format!(
                    "        connectors::{name}::build_authorize_request(\"MANUAL\")\n"
                ));
                helpers.push_str("    ).await.map_err(|e| e.to_string())?;\n");
                helpers.push_str("    if auth.status_code >= 400 {\n");
                helpers.push_str("        return Err(hyperswitch_payments_client::grpc_response_err(auth.status_code, &auth.error));\n");
                helpers.push_str("    }\n");
                helpers.push_str("    let conn_txn = auth.connector_transaction_id.as_deref().unwrap_or(\"\");\n");
                helpers.push_str(&format!(
                    "    let response = client.{grpc_field}.{grpc_method}(\n"
                ));
                helpers.push_str(&format!(
                    "        connectors::{name}::{builder_fn}(conn_txn)\n"
                ));
                helpers.push_str("    ).await.map_err(|e| e.to_string())?;\n");
                helpers.push_str("    if response.status_code >= 400 {\n");
                helpers.push_str("        return Err(hyperswitch_payments_client::grpc_response_err(response.status_code, &response.error));\n");
                helpers.push_str("    }\n");
                helpers.push_str(&format!("    Ok({ret})\n"));
                helpers.push_str("}\n\n");
            }
        }

        fs::write(&grpc_helpers_path, helpers).unwrap();
    }

    // ── grpc_scenarios.rs (match expression included inside run_grpc_scenarios) ──
    // Included in expression position — must contain only the match expression.
    {
        let mut code = String::new();
        code.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n");
        code.push_str("// Calls connector build_*_request() builders directly — no grpc_* wrappers needed.\n\n");

        code.push_str("match connector_name {\n");

        for (name, flows) in &grpc_modules {
            let has_authorize = flows.iter().any(|&(k, ..)| k == "authorize");
            let has_dependents = flows.iter().any(|&(_, _, _, _, needs_txn, _)| needs_txn);

            code.push_str(&format!("    \"{name}\" => {{\n"));
            code.push_str("        let mut results = vec![];\n");

            // Pre-run AUTOMATIC authorize to obtain the connector txn_id for dependent flows.
            if has_authorize && has_dependents {
                code.push_str(&format!(
                    "        let pre_auth_res = client.payment.authorize(connectors::{name}::build_authorize_request(\"AUTOMATIC\")).await;\n"
                ));
                code.push_str("        let authorize_txn_id = pre_auth_res.as_ref().ok()\n");
                code.push_str("            .and_then(|r| r.connector_transaction_id.as_deref())\n");
                code.push_str("            .unwrap_or(\"probe_connector_txn_001\").to_string();\n");
                // Check if authorize returned an error status code
                code.push_str("        let auth_result = match &pre_auth_res {\n");
                code.push_str("            Ok(r) if r.status_code >= 400 => {\n");
                code.push_str("                let err_msg = r.error.as_ref()\n");
                code.push_str("                    .and_then(|e| {\n");
                code.push_str("                        // Try unified_details first, then connector_details, then issuer_details\n");
                code.push_str("                        e.unified_details.as_ref()\n");
                code.push_str("                            .and_then(|u| u.message.as_ref())\n");
                code.push_str("                            .or_else(|| e.connector_details.as_ref().and_then(|c| c.message.as_ref()))\n");
                code.push_str("                            .or_else(|| e.issuer_details.as_ref().and_then(|i| i.message.as_ref()))\n");
                code.push_str("                            .map(|s| s.as_str())\n");
                code.push_str("                    })\n");
                code.push_str("                    .unwrap_or(\"no error details available\");\n");
                code.push_str(
                    "                Err(format!(\"HTTP {}: {}\", r.status_code, err_msg))\n",
                );
                code.push_str("            },\n");
                code.push_str("            Ok(r) => {\n");
                code.push_str("                Ok(format!(\"txn_id: {}, status_code: {}\",\n");
                code.push_str("                    r.connector_transaction_id.as_deref().unwrap_or(\"-\"), r.status_code))\n");
                code.push_str("            },\n");
                code.push_str("            Err(e) => Err(e.to_string()),\n");
                code.push_str("        };\n");
                code.push_str("        results.push((\"authorize\".to_string(), auth_result.map_err(|e| e.into())));\n");
            }

            for &(flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth) in flows {
                // Skip authorize — already handled in the pre-run block above.
                if flow_key == "authorize" && has_authorize && has_dependents {
                    continue;
                }

                if self_auth {
                    code.push_str(&format!(
                        "        results.push((\"{flow_key}\".to_string(), _run_grpc_{flow_key}_{name}(client).await));\n"
                    ));
                } else {
                    // Flows with a dynamic string param: authorize (capture_method),
                    // or flows that receive a connector txn_id from an upstream step.
                    // All other flows have no-param builders.
                    let builder_arg = if flow_key == "authorize" {
                        "\"AUTOMATIC\"".to_string()
                    } else if needs_txn {
                        "&authorize_txn_id".to_string()
                    } else {
                        String::new() // no-param builder
                    };

                    let ret = match flow_key {
                    "authorize" =>
                        "format!(\"txn_id: {}, status_code: {}, error: {}\", r.connector_transaction_id.as_deref().unwrap_or(\"-\"), r.status_code, r.error.as_deref().unwrap_or(\"-\"))",
                        "get" | "reverse" =>
                            "format!(\"txn_id: {}, status_code: {}\", r.connector_transaction_id, r.status_code)",
                        "refund" =>
                            "format!(\"refund_id: {}, status_code: {}\", r.connector_refund_id, r.status_code)",
                        "tokenize" =>
                            "format!(\"token: {}\", r.payment_method_token)",
                        "create_customer" =>
                            "format!(\"customer_id: {}, status_code: {}\", r.connector_customer_id, r.status_code)",
                        "setup_recurring" =>
                            "format!(\"recurring_id: {}, status_code: {}\", r.connector_recurring_payment_id.as_deref().unwrap_or(\"-\"), r.status_code)",
                        "recurring_charge" =>
                            "format!(\"txn_id: {}, status_code: {}\", r.connector_transaction_id.as_deref().unwrap_or(\"-\"), r.status_code)",
                        _ => "format!(\"status_code: {}\", r.status_code)",
                    };

                    let has_status = ret.contains("status_code");

                    code.push_str(&format!("        results.push((\"{flow_key}\".to_string(), match client.{grpc_field}.{grpc_method}(connectors::{name}::{builder_fn}({builder_arg})).await {{\n"));
                    if has_status {
                        code.push_str("            Ok(r) if r.status_code >= 400 =>\n");
                        code.push_str("                Err(hyperswitch_payments_client::grpc_response_err(r.status_code, &r.error)),\n");
                    }
                    code.push_str(&format!("            Ok(r) => Ok({ret}),\n"));
                    code.push_str("            Err(e) => Err(e.to_string().into()),\n");
                    code.push_str("        }));\n");
                }
            }

            code.push_str("        results\n    }\n");
        }

        code.push_str("    _ => vec![],\n}\n");
        fs::write(&grpc_scenarios_path, code).unwrap();
    }
}
