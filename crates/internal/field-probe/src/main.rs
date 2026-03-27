//! Field-probe binary — discovers required fields and sample payloads for every
//! connector × flow × payment-method combination WITHOUT making any HTTP calls.
//!
//! Strategy:
//!   For each (connector, flow, pm_type):
//!     1. Build a maximally-populated proto request with all standard fields set.
//!     2. Call the ffi req_transformer directly (no HTTP).
//!     3. Ok(Some(req))  → supported; record (url, method, headers, body).
//!     4. Ok(None)       → connector skips this flow/pm (returns None intentionally).
//!     5. Err(e)         → parse error, patch proto request, retry up to MAX_ITERS.
//!
//! Output: JSON written to stdout (pipe to file as needed).
//!
//! Configuration: See probe-config.toml for OAuth connectors, payment methods,
//! and connector-specific metadata.

// This is a build-time tool, not production code. Allow certain patterns that would
// be problematic in production but are acceptable here.
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::panic)] // Panics are acceptable in build tools
#![allow(clippy::unwrap_used)] // unwrap is fine in build tools
#![allow(clippy::expect_used)] // expect is fine in build tools
#![allow(clippy::as_conversions)] // as conversions are needed for proto enums
#![allow(clippy::type_complexity)] // Complex types are fine
#![allow(clippy::clone_on_copy)] // clone on Copy types is harmless
#![allow(clippy::indexing_slicing)] // Byte-parsing loops have explicit bounds checks
#![allow(unused_variables)]
#![allow(unused_assignments)]

extern crate connector_service_ffi as ffi;

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use rayon::prelude::*;

mod auth;
mod config;
mod error_parsing;
mod flow_metadata;
mod flow_registry;
mod json_utils;
mod normalizer;
mod orchestrator;
mod patcher;
mod probe_engine;
mod registry;
mod requests;
mod sample_data;
mod status;
mod types;

use config::get_config;
use flow_metadata::{parse_message_schemas, parse_services_proto};
use orchestrator::probe_connector;
use registry::all_connectors;
use types::{CompactConnectorResult, CompactFlowResult, ErrorStats, ProbeManifest};

fn main() {
    // Load config first (initializes PROBE_CONFIG)
    let config = get_config();
    let skip_set: HashSet<String> = config
        .skip_connectors
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let connectors: Vec<_> = all_connectors()
        .into_iter()
        .filter(|c| {
            let name = format!("{c:?}").to_lowercase();
            !skip_set.contains(&name)
        })
        .collect();

    eprintln!(
        "Probing {} connectors ({} skipped)...",
        connectors.len(),
        skip_set.len()
    );

    // Generate flow metadata from services.proto
    eprintln!("Generating flow metadata from services.proto...");
    let flow_metadata = parse_services_proto();
    eprintln!("Generated {} flow metadata entries", flow_metadata.len());

    let results: Vec<_> = connectors
        .par_iter()
        .map(|c| {
            let name = format!("{c:?}");
            eprintln!("Probing {name}...");
            probe_connector(c)
        })
        .collect();

    // Determine output directory
    let output_dir = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("data/field_probe"))
            .unwrap_or_else(|| Path::new("data/field_probe").to_path_buf())
    } else {
        Path::new("data/field_probe").to_path_buf()
    };

    // Create output directory
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        eprintln!(
            "Error: Failed to create output directory {:?}: {e}",
            output_dir
        );
        std::process::exit(1);
    }

    // Convert to compact format and write per-connector files
    let mut connector_names: Vec<String> = Vec::new();
    let mut total_supported = 0;
    let mut total_not_supported = 0;
    #[allow(unused_mut)]
    let mut error_stats = ErrorStats::default();
    // (connector, flow, pm, full_error) for "Stuck on field:" errors
    let mut stuck_entries: Vec<(String, String, String, String)> = Vec::new();

    for result in results {
        let connector_name = result.connector.clone();
        connector_names.push(connector_name.clone());

        // Convert to compact format (omits not_supported entries and null fields)
        let mut compact_flows: BTreeMap<String, BTreeMap<String, CompactFlowResult>> =
            BTreeMap::new();
        let mut supported_count = 0;
        let mut not_supported_count = 0;

        for (flow_name, flow_data) in result.flows {
            let mut compact_flow_data: BTreeMap<String, CompactFlowResult> = BTreeMap::new();
            for (entry_name, flow_result) in flow_data {
                if flow_result.status == "not_supported" {
                    not_supported_count += 1;
                } else {
                    supported_count += 1;
                }
                // Collect stuck-field entries
                if let Some(ref error) = flow_result.error {
                    if error.starts_with("Stuck on field:") {
                        stuck_entries.push((
                            connector_name.clone(),
                            flow_name.clone(),
                            entry_name.clone(),
                            error.clone(),
                        ));
                    }
                }
                // Collect error statistics
                if let Some(ref error) = flow_result.error {
                    if error.contains("MissingRequiredField") {
                        error_stats.missing_field += 1;
                    } else if error.contains("NotSupported") {
                        error_stats.not_supported += 1;
                    } else if error.contains("NotImplemented") || error.contains("not implemented")
                    {
                        error_stats.not_implemented += 1;
                    } else if error.contains("InvalidConnectorConfig")
                        || error.contains("account_id")
                    {
                        error_stats.invalid_config += 1;
                    } else {
                        error_stats.other += 1;
                    }
                }
                if let Some(compact) = Option::<CompactFlowResult>::from(flow_result) {
                    compact_flow_data.insert(entry_name, compact);
                }
            }
            // Only include flows that have at least one supported/error entry
            if !compact_flow_data.is_empty() {
                compact_flows.insert(flow_name, compact_flow_data);
            }
        }

        total_supported += supported_count;
        total_not_supported += not_supported_count;

        let compact_result = CompactConnectorResult {
            connector: result.connector,
            flows: compact_flows,
        };

        // Write formatted JSON with proper indentation
        let connector_json = serde_json::to_string_pretty(&compact_result)
            .expect("Failed to serialize connector results");

        let connector_file = output_dir.join(format!("{}.json", connector_name));
        match std::fs::write(&connector_file, &connector_json) {
            Ok(()) => eprintln!(
                "  Wrote {:?} ({} supported, {} not_supported)",
                connector_file, supported_count, not_supported_count
            ),
            Err(e) => eprintln!("  Warning: Failed to write {:?}: {e}", connector_file),
        }
    }

    // Write manifest file with flow metadata and connector list.
    // Preserve any existing scenario_groups — they are maintained manually
    // and must survive field-probe regeneration.
    let manifest_path = output_dir.join("manifest.json");
    let existing_scenario_groups: Vec<serde_json::Value> = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("scenario_groups").and_then(|g| g.as_array()).cloned())
        .unwrap_or_default();

    let message_schemas = parse_message_schemas();
    let manifest = ProbeManifest {
        flow_metadata,
        connectors: connector_names,
        message_schemas,
        schema_version: "2.0.0".to_string(),
        scenario_groups: existing_scenario_groups,
    };

    let manifest_json =
        serde_json::to_string_pretty(&manifest).expect("Failed to serialize manifest");

    match std::fs::write(&manifest_path, &manifest_json) {
        Ok(()) => eprintln!("Wrote manifest to {:?}", manifest_path),
        Err(e) => eprintln!("Warning: Failed to write manifest: {e}"),
    }

    eprintln!(
        "\nSummary: {} connectors, {} supported entries, {} not_supported entries omitted",
        connectors.len(),
        total_supported,
        total_not_supported
    );

    if stuck_entries.is_empty() {
        eprintln!("\nNo stuck fields — all connectors resolved successfully.");
    } else {
        stuck_entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        eprintln!("\nStuck fields ({} total):", stuck_entries.len());
        for (connector, flow, pm, error) in &stuck_entries {
            eprintln!("  {connector} / {flow} / {pm}: {error}");
        }
    }
}
