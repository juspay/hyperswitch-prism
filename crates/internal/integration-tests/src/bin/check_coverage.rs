//! Check which gRPC proto services/methods have test suite coverage.
//!
//! Run with: cargo run --bin check_coverage

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use regex::Regex;

const IGNORE_SERVICES: &[&str] = &["PayoutService", "DisputeService"];

#[derive(Debug, Clone)]
struct ProtoMethod {
    service: String,
    method: String,
    full_name: String,
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let proto_file = root.join("crates/types-traits/grpc-api-types/proto/services.proto");
    let suites_dir = root.join("crates/internal/integration-tests/src/global_suites");
    let scenario_api = root.join("crates/internal/integration-tests/src/harness/scenario_api.rs");

    println!("{}", "=".repeat(80));
    println!("gRPC PROTO SERVICE COVERAGE ANALYSIS");
    println!("{}", "=".repeat(80));
    println!();

    let proto_methods = extract_proto_methods(&proto_file);
    let suite_mappings = extract_suite_mappings(&scenario_api);
    let available_suites = get_available_suites(&suites_dir);

    analyze_coverage(&proto_methods, &suite_mappings, &available_suites);
}

fn extract_proto_methods(proto_file: &PathBuf) -> Vec<ProtoMethod> {
    let content = fs::read_to_string(proto_file).expect("Failed to read proto file");
    let mut methods = Vec::new();
    let mut current_service = None;

    let service_re = Regex::new(r"^\s*service\s+(\w+)").unwrap();
    let rpc_re = Regex::new(r"^\s*rpc\s+(\w+)\s*\(").unwrap();

    for line in content.lines() {
        if let Some(caps) = service_re.captures(line) {
            current_service = Some(caps[1].to_string());
            continue;
        }

        if let Some(caps) = rpc_re.captures(line) {
            if let Some(ref service) = current_service {
                let method = caps[1].to_string();
                methods.push(ProtoMethod {
                    service: service.clone(),
                    method: method.clone(),
                    full_name: format!("{}/{}", service, method),
                });
            }
        }
    }

    methods
}

fn extract_suite_mappings(scenario_api: &PathBuf) -> HashMap<String, String> {
    let content = fs::read_to_string(scenario_api).expect("Failed to read scenario_api.rs");
    let mut mappings = HashMap::new();

    // Find the grpc_method_for_suite function
    let func_re =
        Regex::new(r"fn grpc_method_for_suite.*?let method = match suite \{(.*?)\};").unwrap();

    if let Some(caps) = func_re.captures(&content) {
        let match_block = &caps[1];

        // Handle both single-line and multi-line formats
        let normalized = match_block
            .replace("{\n", "{ ")
            .replace("\n        }", " }");

        let case_re = Regex::new(r#""([^"]+)"\s*=>\s*\{?\s*"?types\.(\w+)/(\w+)"?\s*\}?"#).unwrap();

        for line in normalized.lines() {
            if let Some(caps) = case_re.captures(line) {
                let suite = caps[1].to_string();
                let service = caps[2].to_string();
                let method = caps[3].to_string();
                mappings.insert(suite, format!("{}/{}", service, method));
            }
        }
    }

    mappings
}

fn get_available_suites(suites_dir: &PathBuf) -> HashSet<String> {
    let mut suites = HashSet::new();

    if let Ok(entries) = fs::read_dir(suites_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let suite_spec = entry.path().join("suite_spec.json");
                if suite_spec.exists() {
                    if let Some(name) = entry.file_name().to_str() {
                        // Remove _suite suffix
                        let suite_name = name.trim_end_matches("_suite");
                        suites.insert(suite_name.to_string());
                    }
                }
            }
        }
    }

    suites
}

fn analyze_coverage(
    proto_methods: &[ProtoMethod],
    suite_mappings: &HashMap<String, String>,
    available_suites: &HashSet<String>,
) {
    // Reverse mapping: proto method -> suites
    let mut proto_to_suites: HashMap<String, Vec<String>> = HashMap::new();
    for (suite, proto_method) in suite_mappings {
        proto_to_suites
            .entry(proto_method.clone())
            .or_default()
            .push(suite.clone());
    }

    // Group by service
    let mut by_service: HashMap<String, Vec<ProtoMethod>> = HashMap::new();
    for method in proto_methods {
        by_service
            .entry(method.service.clone())
            .or_default()
            .push(method.clone());
    }

    let mut covered_methods = Vec::new();
    let mut uncovered_methods = Vec::new();

    let mut services: Vec<_> = by_service.keys().collect();
    services.sort();

    for service_name in services {
        // Skip ignored services
        if IGNORE_SERVICES.contains(&service_name.as_str()) {
            println!("\n{} (IGNORED)", service_name);
            println!("{}", "-".repeat(service_name.len() + 10));
            println!("  ⊘ Service ignored per configuration");
            continue;
        }

        let methods = &by_service[service_name];
        println!("\n{}", service_name);
        println!("{}", "-".repeat(service_name.len()));

        for method in methods {
            let full_name = &method.full_name;
            let suites = proto_to_suites.get(full_name).cloned().unwrap_or_default();

            // Check which suites actually exist
            let existing_suites: Vec<_> = suites
                .iter()
                .filter(|s| available_suites.contains(*s))
                .collect();
            let missing_suites: Vec<_> = suites
                .iter()
                .filter(|s| !available_suites.contains(*s))
                .collect();

            if !existing_suites.is_empty() {
                covered_methods.push(method.clone());
                let suite_info = existing_suites
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let extra = if !missing_suites.is_empty() {
                    format!(
                        " (mapped but missing: {})",
                        missing_suites
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    String::new()
                };
                println!(
                    "  {:12} {:40} by: {}{}",
                    "✓ COVERED", method.method, suite_info, extra
                );
            } else if !missing_suites.is_empty() {
                uncovered_methods.push(method.clone());
                let suite_info = missing_suites
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "  {:12} {:40} mapped by {} but suite dirs missing",
                    "⚠ PARTIAL", method.method, suite_info
                );
            } else {
                uncovered_methods.push(method.clone());
                println!("  {:12} {:40} NO SUITE", "✗ MISSING", method.method);
            }
        }
    }

    // Filter out ignored services from totals
    let total_methods: usize = proto_methods
        .iter()
        .filter(|m| !IGNORE_SERVICES.contains(&m.service.as_str()))
        .count();
    let covered_count: usize = covered_methods
        .iter()
        .filter(|m| !IGNORE_SERVICES.contains(&m.service.as_str()))
        .count();
    let uncovered_count: usize = uncovered_methods
        .iter()
        .filter(|m| !IGNORE_SERVICES.contains(&m.service.as_str()))
        .count();

    // Summary
    println!();
    println!("{}", "=".repeat(80));
    println!("SUMMARY");
    println!("{}", "=".repeat(80));
    println!("Total proto RPC methods:     {}", total_methods);
    println!("Covered with suites:         {}", covered_count);
    println!("Missing coverage:            {}", uncovered_count);
    println!(
        "Coverage percentage:         {:.1}%",
        (covered_count as f64 / total_methods as f64) * 100.0
    );

    if !uncovered_methods.is_empty() {
        println!();
        println!("{}", "=".repeat(80));
        println!("MISSING COVERAGE - METHODS WITHOUT SUITES");
        println!("{}", "=".repeat(80));
        for method in &uncovered_methods {
            if !IGNORE_SERVICES.contains(&method.service.as_str()) {
                println!("  - {}", method.full_name);
            }
        }
    }

    // Check for suites without proto mapping
    let mapped_suites: HashSet<_> = suite_mappings.keys().cloned().collect();
    let unmapped: Vec<_> = available_suites
        .iter()
        .filter(|s| !mapped_suites.contains(*s))
        .collect();

    if !unmapped.is_empty() {
        println!();
        println!("{}", "=".repeat(80));
        println!("SUITES WITHOUT PROTO MAPPING");
        println!("{}", "=".repeat(80));
        for suite in unmapped {
            println!("  - {}", suite);
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!("NOTE: Ignored services: {}", IGNORE_SERVICES.join(", "));
    println!("{}", "=".repeat(80));
}
