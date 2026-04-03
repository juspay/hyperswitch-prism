//! Verify that every flow listed in a connector's `create_all_prerequisites!`
//! macro is accounted for in that connector's `connector_specs/<name>/specs.json`
//! (either in `supported_suites` or `unsupported_suites`).
//!
//! Run with:
//!   cargo run --bin check_connector_specs
//!
//! Exits non-zero if any connector has a flow whose mapped suite is absent
//! from both `supported_suites` and `unsupported_suites`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use regex::Regex;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Flow → suite mapping
// ---------------------------------------------------------------------------

/// Maps a Rust flow name (as it appears after `flow:` in `create_all_prerequisites!`)
/// to the suite name(s) it corresponds to in `connector_specs/<connector>/specs.json`.
///
/// Flows that have no corresponding integration-test suite (e.g. payout flows,
/// dispute flows, server-authentication flows, incremental auth) are mapped to
/// `None` and are silently skipped.
fn flow_to_suites(flow: &str) -> Option<&'static [&'static str]> {
    match flow {
        "Authorize" => Some(&["authorize"]),
        "PSync" => Some(&["get"]),
        "Capture" => Some(&["capture"]),
        "Void" => Some(&["void"]),
        "Refund" => Some(&["refund"]),
        "RSync" => Some(&["refund_sync"]),
        "SetupMandate" => Some(&["setup_recurring"]),
        "RepeatPayment" => Some(&["recurring_charge"]),
        "CreateConnectorCustomer" => Some(&["create_customer"]),
        "PaymentMethodToken" => Some(&["tokenize_payment_method"]),
        "MandateRevoke" => Some(&["revoke_mandate"]),
        "ClientAuthenticationToken" => Some(&["create_sdk_session_token"]),
        "ServerSessionAuthenticationToken" => Some(&["create_session_token"]),
        // Flows below have no dedicated integration-test suite — skip them.
        "ServerAuthenticationToken" => None,
        "PreAuthenticate" => None,
        "Authenticate" => None,
        "PostAuthenticate" => None,
        "CreateOrder" => None,
        "IncrementalAuthorization" => None,
        "Accept" => None,
        "DefendDispute" => None,
        "SubmitEvidence" => None,
        // Payout flows — out of scope.
        "PayoutCreate" => None,
        "PayoutGet" => None,
        "PayoutStage" => None,
        "PayoutTransfer" => None,
        "PayoutVoid" => None,
        "PayoutEnrollDisburseAccount" => None,
        "PayoutCreateRecipient" => None,
        "PayoutCreateLink" => None,
        // Unknown / macro-internal tokens — skip.
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Specs.json schema
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ConnectorSpecs {
    #[serde(default)]
    supported_suites: Vec<String>,
    #[serde(default)]
    unsupported_suites: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Extract all flow names from one connector source file.
///
/// Strategy: locate every `macros::create_all_prerequisites!(` block,
/// then collect every line that matches `flow:\s*<Ident>` inside it.
///
/// Brackets are balanced to find the block end so nested parens are handled.
fn extract_flows_from_source(src: &str) -> Vec<String> {
    let flow_re = Regex::new(r"^\s*flow:\s*([A-Za-z][A-Za-z0-9]*)").unwrap();
    let mut flows = Vec::new();

    let mut search_from = 0;
    while let Some(macro_pos) = src[search_from..].find("macros::create_all_prerequisites!(") {
        let abs_start = search_from + macro_pos;
        // Find the matching closing ')' by counting parens.
        let block_start = abs_start + "macros::create_all_prerequisites!(".len() - 1; // at '('
        let block = extract_balanced_parens(src, block_start);

        for line in block.lines() {
            if let Some(caps) = flow_re.captures(line) {
                let name = caps[1].to_string();
                if !flows.contains(&name) {
                    flows.push(name);
                }
            }
        }

        search_from = abs_start + 1;
    }

    flows
}

/// Return the slice of `src` starting at `start` (which should be the `(`)
/// up to and including the balanced closing `)`.
fn extract_balanced_parens(src: &str, start: usize) -> &str {
    let bytes = src.as_bytes();
    if bytes.get(start) != Some(&b'(') {
        return "";
    }
    let mut depth = 0usize;
    for (i, &b) in bytes[start..].iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &src[start..=start + i];
                }
            }
            _ => {}
        }
    }
    // Unbalanced — return the rest (should not happen in valid Rust source).
    &src[start..]
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR = .../crates/internal/integration-tests
    // workspace root is three levels up.
    let root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let connectors_src = root.join("crates/integrations/connector-integration/src/connectors");
    let specs_root = root.join("crates/internal/integration-tests/src/connector_specs");

    // Collect all connector names from connector_specs/ directory.
    let mut connectors: Vec<String> = fs::read_dir(&specs_root)
        .expect("failed to read connector_specs dir")
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    connectors.sort();

    println!("{}", "=".repeat(80));
    println!("CONNECTOR FLOW → SPECS.JSON COVERAGE CHECK");
    println!("{}", "=".repeat(80));
    println!();

    // connector_name → list of (flow, suite) pairs that are missing from specs
    let mut errors: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    // connector_name → list of (flow, suite) pairs that are covered
    let mut covered_summary: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    // connectors where no source .rs was found
    let mut no_source: Vec<String> = Vec::new();

    for connector in &connectors {
        let src_path = connectors_src.join(format!("{connector}.rs"));

        let src = match fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(_) => {
                no_source.push(connector.clone());
                continue;
            }
        };

        let flows = extract_flows_from_source(&src);
        if flows.is_empty() {
            no_source.push(connector.clone());
            continue;
        }

        let specs_path = specs_root.join(connector).join("specs.json");
        let specs: ConnectorSpecs = match fs::read_to_string(&specs_path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                eprintln!("WARN: failed to parse {}: {e}", specs_path.display());
                ConnectorSpecs::default()
            }),
            Err(_) => ConnectorSpecs::default(),
        };

        let supported: BTreeSet<&str> = specs.supported_suites.iter().map(String::as_str).collect();
        let unsupported: BTreeSet<&str> = specs
            .unsupported_suites
            .iter()
            .map(String::as_str)
            .collect();

        for flow in &flows {
            let Some(suites) = flow_to_suites(flow) else {
                continue; // flow has no suite mapping — skip
            };

            for &suite in suites {
                if supported.contains(suite) || unsupported.contains(suite) {
                    covered_summary
                        .entry(connector.clone())
                        .or_default()
                        .push((flow.clone(), suite.to_string()));
                } else {
                    errors
                        .entry(connector.clone())
                        .or_default()
                        .push((flow.clone(), suite.to_string()));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Print per-connector results
    // -----------------------------------------------------------------------
    for connector in &connectors {
        let has_errors = errors.contains_key(connector.as_str());
        let covered = covered_summary.get(connector.as_str());
        let missing = errors.get(connector.as_str());

        if no_source.contains(connector) {
            println!("[SKIP] {connector}  (no source .rs or no create_all_prerequisites!)");
            continue;
        }

        if has_errors {
            println!("[FAIL] {connector}");
            if let Some(covered_list) = covered {
                for (flow, suite) in covered_list {
                    println!("       OK      flow={flow:<35} suite={suite}");
                }
            }
            for (flow, suite) in missing.unwrap() {
                println!("       MISSING flow={flow:<35} suite={suite}  (not in supported_suites or unsupported_suites)");
            }
        } else {
            let n = covered.map(|v| v.len()).unwrap_or(0);
            println!("[OK]   {connector}  ({n} flows mapped)");
        }
        println!();
    }

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    println!("{}", "=".repeat(80));
    println!("SUMMARY");
    println!("{}", "=".repeat(80));
    let total = connectors.len() - no_source.len();
    let fail_count = errors.len();
    let ok_count = total - fail_count;
    println!("Connectors checked:   {total}");
    println!("All flows accounted:  {ok_count}");
    println!("With missing suites:  {fail_count}");
    println!("Skipped (no source):  {}", no_source.len());

    if !no_source.is_empty() {
        println!();
        println!("Skipped connectors: {}", no_source.join(", "));
    }

    if !errors.is_empty() {
        println!();
        println!("{}", "=".repeat(80));
        println!("ERRORS — flows whose suite is missing from specs.json");
        println!("{}", "=".repeat(80));
        for (connector, pairs) in &errors {
            for (flow, suite) in pairs {
                println!("  {connector:<30}  flow={flow:<35} suite={suite}");
            }
        }
        println!();
        eprintln!(
            "ERROR: {} connector(s) have flows not accounted for in specs.json",
            errors.len()
        );
        std::process::exit(1);
    } else {
        println!();
        println!("All connector flows are accounted for in specs.json. OK.");
    }
}

impl Default for ConnectorSpecs {
    fn default() -> Self {
        Self {
            supported_suites: Vec::new(),
            unsupported_suites: Vec::new(),
        }
    }
}
