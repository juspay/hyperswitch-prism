use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Type alias for gRPC module tuple to avoid complex type warning
type GrpcModule<'a> = (
    String,
    Vec<(&'a str, &'a str, &'a str, &'a str, bool, bool)>,
);

/// Flow manifest data containing flows list and flow-to-example-fn mapping
struct FlowManifest {
    flows: Vec<String>,
    flow_to_example_fn: HashMap<String, Option<String>>,
}

/// Load flows.json manifest and return both flows list and flow-to-example-fn mapping.
/// Returns empty data if file doesn't exist (allows CI builds without generated files).
fn load_flow_manifest(repo_root: &Path) -> FlowManifest {
    let manifest_path = repo_root.join("sdk").join("generated").join("flows.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => {
            println!(
                "cargo:warning=flows.json not found at {}. Skipping smoke test generation.",
                manifest_path.display()
            );
            return FlowManifest {
                flows: Vec::new(),
                flow_to_example_fn: HashMap::new(),
            };
        }
    };

    // Parse flows array
    let flows_start = text
        .find("\"flows\"")
        .expect("flows key not found in flows.json");
    let bracket_start = text[flows_start..]
        .find('[')
        .expect("flows array not found")
        + flows_start;
    let bracket_end = text[bracket_start..]
        .find(']')
        .expect("flows array end not found")
        + bracket_start;
    let array_content = &text[bracket_start + 1..bracket_end];

    let flows: Vec<String> = array_content
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Parse flow_to_example_fn mapping if present
    let mut flow_to_example_fn: HashMap<String, Option<String>> = HashMap::new();
    if let Some(mapping_start) = text.find("\"flow_to_example_fn\"") {
        if let Some(brace_start) = text[mapping_start..].find('{') {
            let brace_start = brace_start + mapping_start;
            if let Some((block, _)) = extract_brace_block(&text[brace_start..]) {
                // Parse "flow": "example_fn" or "flow": null pairs
                for line in block.split(',') {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    // Extract key and value
                    if let Some(colon_pos) = line.find(':') {
                        let key = line[..colon_pos].trim().trim_matches('"').to_string();
                        let value = line[colon_pos + 1..].trim();
                        if value == "null" {
                            flow_to_example_fn.insert(key, None);
                        } else {
                            let val = value.trim_matches('"').to_string();
                            flow_to_example_fn.insert(key, Some(val));
                        }
                    }
                }
            }
        }
    }

    FlowManifest {
        flows,
        flow_to_example_fn,
    }
}

/// Read SUPPORTED_FLOWS from example .rs file content.
fn load_supported_flows_from_example(content: &str) -> Option<Vec<String>> {
    // Match: pub const SUPPORTED_FLOWS: &[&str] = &["flow1", "flow2", ...];
    let marker = "pub const SUPPORTED_FLOWS: &[&str] = &[";
    let start = content.find(marker)?;
    let after = &content[start + marker.len()..];
    let end = after
        .find("];")
        .unwrap_or_else(|| after.find(']').expect("SUPPORTED_FLOWS array not closed"));
    let array_content = &after[..end];

    let flows: Vec<String> = array_content
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(flows)
}

/// Extracts the content of a JSON object block starting at the first `{` in `text`.
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
fn load_supported_flows_from_probe(
    examples_dir: &Path,
    connector: &str,
) -> Option<HashSet<String>> {
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

    let flows_marker = "\"flows\":";
    let flows_start = text.find(flows_marker)?;
    let (flows_block, _) = extract_brace_block(&text[flows_start + flows_marker.len()..])?;

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
        "tokenized_authorize",
        "tokenized_setup_recurring",
        "proxied_authorize",
        "proxied_setup_recurring",
    ];

    let mut supported = HashSet::new();
    for flow_key in all_flow_keys {
        let key_pattern = format!("\"{}\":", flow_key);
        if let Some(pos) = flows_block.find(&key_pattern) {
            let after_colon = &flows_block[pos + key_pattern.len()..];
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
    // Declare env-var dependencies so cargo reruns this script when they change.
    println!("cargo:rerun-if-env-changed=CONNECTORS");
    println!("cargo:rerun-if-env-changed=HARNESS_DIR");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = Path::new(&manifest_dir).join("../../..");
    // Allow overriding examples directory via HARNESS_DIR env var (used in mock mode)
    let examples_dir = std::env::var("HARNESS_DIR")
        .map(|p| Path::new(&p).to_path_buf())
        .unwrap_or_else(|_| repo_root.join("examples"));
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let connectors_path = format!("{out_dir}/connectors.rs");
    let scenarios_path = format!("{out_dir}/connector_scenarios.rs");
    let grpc_scenarios_path = format!("{out_dir}/grpc_scenarios.rs");
    let grpc_helpers_path = format!("{out_dir}/grpc_helpers.rs");

    // Load canonical flow manifest from flows.json
    let manifest_data = load_flow_manifest(&repo_root);
    let manifest = &manifest_data.flows;
    let _flow_to_example_fn = &manifest_data.flow_to_example_fn; // Prefixed with underscore as it's no longer used in legacy mode
    println!(
        "cargo:warning=Loaded {} flows from flows.json",
        manifest.len()
    );

    // Flow metadata for gRPC dispatch via builder functions.
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
            true,
            false,
        ),
        ("void", "payment", "void", "build_void_request", true, false),
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
            "payment_method_authentication",
            "pre_authenticate",
            "build_pre_authenticate_request",
            false,
            false,
        ),
        (
            "authenticate",
            "payment_method_authentication",
            "authenticate",
            "build_authenticate_request",
            false,
            false,
        ),
        (
            "post_authenticate",
            "payment_method_authentication",
            "post_authenticate",
            "build_post_authenticate_request",
            false,
            false,
        ),
        (
            "handle_event",
            "event",
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
        (
            "tokenized_authorize",
            "tokenized_payment",
            "tokenized_authorize",
            "build_tokenized_authorize_request",
            false,
            false,
        ),
        (
            "tokenized_setup_recurring",
            "tokenized_payment",
            "tokenized_setup_recurring",
            "build_tokenized_setup_recurring_request",
            false,
            false,
        ),
        (
            "proxied_authorize",
            "proxy_payment",
            "proxied_authorize",
            "build_proxied_authorize_request",
            false,
            false,
        ),
        (
            "proxied_setup_recurring",
            "proxy_payment",
            "proxied_setup_recurring",
            "build_proxied_setup_recurring_request",
            false,
            false,
        ),
    ];

    let allowed_connectors: Vec<String> = std::env::var("CONNECTORS")
        .ok()
        .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| vec!["stripe".to_string()]);

    // Store scenario data as (connector_name, Vec<(flow_key, fn_name)>)
    // fn_name is stored as String to avoid lifetime issues
    let mut modules: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut grpc_modules: Vec<GrpcModule> = Vec::new();

    for connector_name in &allowed_connectors {
        let rs_file = examples_dir
            .join(connector_name)
            .join(format!("{connector_name}.rs"));
        if !rs_file.exists() {
            continue;
        }

        println!("cargo:rerun-if-changed={}", rs_file.display());
        let content = fs::read_to_string(&rs_file).unwrap_or_default();

        // Load SUPPORTED_FLOWS from example file (new approach)
        let declared_flows = load_supported_flows_from_example(&content);

        // Load field_probe supported flows (legacy support)
        let field_probe_supported = load_supported_flows_from_probe(&examples_dir, connector_name);

        // Discover FFI process_* functions using SUPPORTED_FLOWS with 3-check validation
        let mut present: Vec<(String, String)> = Vec::new();

        // Skip connectors without SUPPORTED_FLOWS (they need to add it)
        let declared = match declared_flows {
            Some(flows) => flows,
            None => {
                println!("cargo:warning=Skipping connector '{}': No SUPPORTED_FLOWS found. Add: pub const SUPPORTED_FLOWS: &[&str] = &[...];", connector_name);
                continue;
            }
        };

        let manifest_set: HashSet<_> = manifest.iter().cloned().collect();

        // CHECK 1: Verify all declared flows have implementations
        let mut missing = Vec::new();
        for flow in &declared {
            let fn_name = format!("process_{}", flow);
            if !content.contains(&format!("pub async fn {}(", fn_name)) {
                missing.push(flow.clone());
            }
        }
        if !missing.is_empty() {
            panic!(
                "COVERAGE ERROR [{}]: SUPPORTED_FLOWS declares {:?} but no process_* function found for them.",
                connector_name, missing
            );
        }

        // CHECK 2: Verify no undeclared *flow* process_* functions exist.
        // Scenario functions (e.g. process_checkout_autocapture) whose base name is not
        // a known flow in flows.json are allowed — they cover multi-step scenarios.
        for line in content.lines() {
            if let Some(pos) = line.find("pub async fn process_") {
                let after_process = &line[pos + 21..]; // after "pub async fn process_" (21 chars)
                if let Some(paren_pos) = after_process.find('(') {
                    let flow_name = &after_process[..paren_pos];
                    // Only error if this is a known flow but not declared in SUPPORTED_FLOWS
                    if manifest_set.contains(flow_name) && !declared.iter().any(|d| d == flow_name)
                    {
                        panic!(
                            "COVERAGE ERROR [{}]: process_{} exists for a known flow but '{}' not in SUPPORTED_FLOWS.",
                            connector_name, flow_name, flow_name
                        );
                    }
                }
            }
        }

        // CHECK 3: Verify all declared flows exist in manifest
        // Note: We only warn about stale flows instead of panicking, since scenarios
        // (like checkout_card, void_payment) are valid SUPPORTED_FLOWS entries but
        // are not in the flow manifest - they represent composite scenarios rather
        // than individual protocol flows.
        let stale: Vec<_> = declared
            .iter()
            .filter(|flow| !manifest_set.contains(*flow))
            .cloned()
            .collect();
        if !stale.is_empty() {
            println!("cargo:warning=SUPPORTED_FLOWS for '{}' contains entries not in flows.json (these are scenario names): {:?}", connector_name, stale);
        }

        // Build declared set before consuming declared
        let declared_set: HashSet<String> = declared.iter().cloned().collect();

        // Add validated flows
        for flow in declared {
            present.push((flow.clone(), format!("process_{}", flow)));
        }

        if !present.is_empty() {
            modules.push((connector_name.clone(), present));
        }

        // Discover gRPC flows
        let mut grpc_present: Vec<(&str, &str, &str, &str, bool, bool)> = Vec::new();
        for &(flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth) in flow_meta {
            // Only include flows declared in SUPPORTED_FLOWS
            if !declared_set.contains(flow_key) {
                continue;
            }
            if let Some(ref supported) = field_probe_supported {
                if !supported.contains(flow_key) {
                    continue;
                }
            }
            if content.contains(&format!("fn {}(", builder_fn)) {
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
            let path = examples_dir.join(name.as_str()).join(format!("{name}.rs"));
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
        if modules.is_empty() {
            code.push_str("{\n");
            code.push_str("    let _ = connector_name;\n");
            code.push_str("    vec![]\n");
            code.push_str("}\n");
        } else {
            code.push_str("match connector_name {\n");
            for (name, scenarios) in &modules {
                // Build a map of flow -> example_fn_name for this connector
                let implemented: std::collections::HashMap<_, _> = scenarios
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                code.push_str(&format!("    \"{name}\" => {{\n"));
                code.push_str("        #[allow(unused_mut)]\n");
                code.push_str("        let mut results = vec![];\n");
                // Include ALL flows from manifest, using flow_to_example_fn mapping
                for flow in manifest {
                    if let Some(example_fn) = implemented.get(flow.as_str()) {
                        // Flow has an implementation - call the example function
                        // Use the actual result message for normal mode, mock request info for mock mode
                        code.push_str(&format!(
                            "        match connectors::{}::{}(client, &txn_id).await {{\n",
                            name, example_fn
                        ));
                        code.push_str("            Ok(msg) => results.push((\"");
                        code.push_str(flow);
                        code.push_str("\".to_string(), Ok(msg))),\n");
                        code.push_str("            Err(e) => results.push((\"");
                        code.push_str(flow);
                        code.push_str("\".to_string(), Err(e))),\n");
                        code.push_str("        }\n");
                    } else {
                        // Flow not implemented
                        code.push_str(&format!(
                            "        results.push((\"{}\".to_string(), Err(\"NOT IMPLEMENTED — No example function for flow '{}'\".to_string().into())));\n",
                            flow, flow
                        ));
                    }
                }
                code.push_str("        results\n    }\n");
            }
            code.push_str("    _ => vec![],\n}\n");
        }
        fs::write(&scenarios_path, code).unwrap();
    }

    // ── grpc_helpers.rs ───────────────────────────────────────────────────────
    {
        let mut helpers = String::new();
        helpers.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n\n");

        for (name, flows) in &grpc_modules {
            let has_authorize = flows.iter().any(|&(k, ..)| k == "authorize");
            for &(flow_key, grpc_field, grpc_method, builder_fn, _needs_txn, self_auth) in flows {
                if !self_auth {
                    continue;
                }
                // Skip helpers that require authorize if this connector has no build_authorize_request
                if !has_authorize {
                    continue;
                }
                let ret = match flow_key {
                    "capture" | "void" | "reverse" =>
                        "format!(\"txn_id: {}, status_code: {}\", response.connector_transaction_id, response.status_code)".to_string(),
                    _ => "format!(\"status_code: {}\", response.status_code)".to_string(),
                };
                helpers.push_str("#[allow(dead_code)]\n");
                helpers.push_str(&format!("async fn _run_grpc_{}_{}(\n", flow_key, name));
                helpers.push_str("    client: &hyperswitch_payments_client::GrpcClient,\n");
                helpers.push_str(") -> Result<String, Box<dyn std::error::Error>> {\n");
                helpers.push_str(&format!(
                    "    let auth = client.{}.authorize(\n",
                    grpc_field
                ));
                helpers.push_str(&format!(
                    "        connectors::{}::build_authorize_request(\"MANUAL\")\n",
                    name
                ));
                helpers.push_str("    ).await.map_err(|e| e.to_string())?;\n");
                helpers.push_str("    if auth.status_code >= 400 {\n");
                helpers.push_str("        return Err(hyperswitch_payments_client::grpc_response_err(auth.status_code, &auth.error));\n");
                helpers.push_str("    }\n");
                helpers.push_str("    let conn_txn = auth.connector_transaction_id.as_deref().unwrap_or(\"\");\n");
                helpers.push_str(&format!(
                    "    let response = client.{}.{}(\n",
                    grpc_field, grpc_method
                ));
                helpers.push_str(&format!(
                    "        connectors::{}::{}(conn_txn)\n",
                    name, builder_fn
                ));
                helpers.push_str("    ).await.map_err(|e| e.to_string())?;\n");
                helpers.push_str("    if response.status_code >= 400 {\n");
                helpers.push_str("        return Err(hyperswitch_payments_client::grpc_response_err(response.status_code, &response.error));\n");
                helpers.push_str("    }\n");
                helpers.push_str(&format!("    Ok({})\n", ret));
                helpers.push_str("}\n\n");
            }
        }

        fs::write(&grpc_helpers_path, helpers).unwrap();
    }

    // ── grpc_scenarios.rs ─────────────────────────────────────────────────────
    {
        let mut code = String::new();
        code.push_str("// AUTO-GENERATED by build.rs — do not edit manually.\n");
        code.push_str("// Calls connector build_*_request() builders directly.\n\n");

        if grpc_modules.is_empty() {
            code.push_str("{\n");
            code.push_str("    let _ = connector_name;\n");
            code.push_str("    vec![]\n");
            code.push_str("}\n");
        } else {
            code.push_str("match connector_name {\n");

            for (name, flows) in &grpc_modules {
                let has_authorize = flows.iter().any(|&(k, ..)| k == "authorize");
                let has_dependents = flows.iter().any(|&(_, _, _, _, needs_txn, _)| needs_txn);

                code.push_str(&format!("    \"{name}\" => {{\n"));
                code.push_str("        #[allow(unused_mut)]\n");
                code.push_str("        let mut results = vec![];\n");

                // If connector has needs_txn flows but no authorize, emit a placeholder txn_id
                if !has_authorize && has_dependents {
                    code.push_str(
                        "        let authorize_txn_id = \"probe_connector_txn_001\".to_string();\n",
                    );
                }

                if has_authorize && has_dependents {
                    code.push_str(&format!(
                    "        let pre_auth_res = client.payment.authorize(connectors::{name}::build_authorize_request(\"AUTOMATIC\")).await;\n"
                ));
                    code.push_str("        let authorize_txn_id = pre_auth_res.as_ref().ok()\n");
                    code.push_str(
                        "            .and_then(|r| r.connector_transaction_id.as_deref())\n",
                    );
                    code.push_str(
                        "            .unwrap_or(\"probe_connector_txn_001\").to_string();\n",
                    );
                    code.push_str("        let auth_result = match &pre_auth_res {\n");
                    code.push_str("            Ok(r) if r.status_code >= 400 => {\n");
                    code.push_str("                let err_msg = r.error.as_ref()\n");
                    code.push_str("                    .and_then(|e| {\n");
                    code.push_str("                        e.unified_details.as_ref()\n");
                    code.push_str(
                        "                            .and_then(|u| u.message.as_ref())\n",
                    );
                    code.push_str("                            .or_else(|| e.connector_details.as_ref().and_then(|c| c.message.as_ref()))\n");
                    code.push_str("                            .or_else(|| e.issuer_details.as_ref().and_then(|i| i.message.as_ref()))\n");
                    code.push_str("                            .map(|s| s.as_str())\n");
                    code.push_str("                    })\n");
                    code.push_str(
                        "                    .unwrap_or(\"no error details available\");\n",
                    );
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

                for &(flow_key, grpc_field, grpc_method, builder_fn, needs_txn, self_auth) in flows
                {
                    if flow_key == "authorize" && has_authorize && has_dependents {
                        continue;
                    }

                    if self_auth {
                        // self_auth helpers require build_authorize_request — skip if not available
                        if !has_authorize {
                            continue;
                        }
                        code.push_str(&format!(
                        "        results.push((\"{}\".to_string(), _run_grpc_{}_{}(client).await));\n",
                        flow_key, flow_key, name
                    ));
                    } else {
                        let builder_arg = if flow_key == "authorize" {
                            "\"AUTOMATIC\"".to_string()
                        } else if needs_txn {
                            "&authorize_txn_id".to_string()
                        } else {
                            String::new()
                        };

                        let ret = match flow_key {
                        "authorize" | "tokenized_authorize" | "proxied_authorize" =>
                            "format!(\"txn_id: {}, status_code: {}\", r.connector_transaction_id.as_deref().unwrap_or(\"-\"), r.status_code)",
                        "get" | "reverse" =>
                            "format!(\"txn_id: {}, status_code: {}\", r.connector_transaction_id, r.status_code)",
                        "refund" =>
                            "format!(\"refund_id: {}, status_code: {}\", r.connector_refund_id, r.status_code)",
                        "tokenize" =>
                            "format!(\"token: {}\", r.payment_method_token)",
                        "create_customer" =>
                            "format!(\"customer_id: {}, status_code: {}\", r.connector_customer_id, r.status_code)",
                        "setup_recurring" | "tokenized_setup_recurring" | "proxied_setup_recurring" =>
                            "format!(\"recurring_id: {}, status_code: {}\", r.connector_recurring_payment_id.as_deref().unwrap_or(\"-\"), r.status_code)",
                        "recurring_charge" =>
                            "format!(\"txn_id: {}, status_code: {}\", r.connector_transaction_id.as_deref().unwrap_or(\"-\"), r.status_code)",
                        _ => "format!(\"status_code: {}\", r.status_code)",
                    };

                        let has_status = ret.contains("status_code");

                        code.push_str(&format!("        results.push((\"{}\".to_string(), match client.{}.{}(connectors::{}::{}({})).await {{\n", flow_key, grpc_field, grpc_method, name, builder_fn, builder_arg));
                        if has_status {
                            code.push_str("            Ok(r) if r.status_code >= 400 =>\n");
                            code.push_str("                Err(hyperswitch_payments_client::grpc_response_err(r.status_code, &r.error)),\n");
                        }
                        code.push_str(&format!("            Ok(r) => Ok({}),\n", ret));
                        code.push_str("            Err(e) => Err(e.to_string().into()),\n");
                        code.push_str("        }));\n");
                    }
                }

                code.push_str("        results\n    }\n");
            }

            code.push_str("    _ => vec![],\n}\n");
        }
        fs::write(&grpc_scenarios_path, code).unwrap();
    }
}
