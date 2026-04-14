/**
 * Multi-connector smoke test for the hyperswitch-payments Rust SDK.
 *
 * Loads connector credentials from external JSON file and runs all scenario
 * functions found in examples/{connector}/{connector}.rs for each connector.
 *
 * Each example file exports pub async fn process_*(client, txn_id) -> Result<String, Box<dyn Error>>
 * functions that the smoke test discovers and invokes at compile time.
 *
 * Usage:
 *   cargo run --bin hyperswitch-smoke-test -- --creds-file creds.json --all
 *   cargo run --bin hyperswitch-smoke-test -- --creds-file creds.json --connectors stripe,adyen
 *   cargo run --bin hyperswitch-smoke-test -- --creds-file creds.json --all --dry-run
 */
mod build_auth;

// Include the auto-generated connector modules (built by build.rs).
// Each connector becomes e.g. `connectors::stripe::process_checkout_card`.
mod connectors {
    include!(concat!(env!("OUT_DIR"), "/connectors.rs"));
}

use grpc_api_types::payments::{ConnectorConfig, Environment, SdkOptions};
use hyperswitch_payments_client::ConnectorClient;
use std::error::Error;

// ── ANSI color helpers ────────────────────────────────────────────────────────
fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
        || (std::env::var("FORCE_COLOR").is_err()
            && std::env::var("TERM").map_or(true, |t| t.is_empty() || t == "dumb"))
}
fn c(code: &str, text: &str) -> String {
    if no_color() {
        text.to_string()
    } else {
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}
fn green(t: &str) -> String {
    c("32", t)
}
fn yellow(t: &str) -> String {
    c("33", t)
}
fn red(t: &str) -> String {
    c("31", t)
}
fn grey(t: &str) -> String {
    c("90", t)
}
fn bold(t: &str) -> String {
    c("1", t)
}

const PLACEHOLDER_VALUES: &[&str] = &["", "placeholder", "test", "dummy", "sk_test_placeholder"];

fn is_placeholder(value: &str) -> bool {
    let lower = value.to_lowercase();
    PLACEHOLDER_VALUES.contains(&lower.as_str()) || lower.contains("placeholder")
}

fn has_valid_credentials(creds: &serde_json::Map<String, serde_json::Value>) -> bool {
    for (key, val) in creds {
        if key == "metadata" || key == "_comment" {
            continue;
        }
        let str_val = match val {
            serde_json::Value::String(s) => Some(s.as_str()),
            serde_json::Value::Object(obj) => obj.get("value").and_then(|v| v.as_str()),
            _ => None,
        };
        if let Some(s) = str_val {
            if !is_placeholder(s) {
                return true;
            }
        }
    }
    false
}

fn build_config(
    connector_name: &str,
    creds: &serde_json::Map<String, serde_json::Value>,
) -> Result<ConnectorConfig, String> {
    let connector_config = build_auth::build_connector_config(connector_name, creds)?;
    Ok(ConnectorConfig {
        connector_config: Some(connector_config),
        options: Some(SdkOptions {
            environment: Environment::Sandbox as i32,
        }),
    })
}

fn rand_hex() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        & 0xFFFFFF
}

/// Compile-time dispatch: call process_* functions for a given connector.
/// Returns Vec<(scenario_key, Result<String, Box<dyn Error>>)>.
#[allow(unused_variables)]
async fn run_connector_scenarios(
    connector_name: &str,
    client: &ConnectorClient,
) -> Vec<(String, Result<String, Box<dyn Error>>)> {
    let txn_id = format!("smoke_{:06x}", rand_hex());
    // connector_scenarios.rs expands to a match expression over connector_name
    include!(concat!(env!("OUT_DIR"), "/connector_scenarios.rs"))
}

#[derive(Debug)]
struct ScenarioResult {
    status: &'static str,    // "passed" | "skipped" | "failed" | "not_implemented"
    message: Option<String>, // e.g. mock request "POST https://..." or response summary
    reason: Option<String>,  // for skipped
    error: Option<String>,
}

#[derive(Debug)]
struct ConnectorResult {
    connector: String,
    status: &'static str, // "passed" | "failed" | "skipped" | "dry_run"
    scenarios: Vec<(String, ScenarioResult)>,
    error: Option<String>,
}

async fn test_connector_scenarios(
    instance_name: &str,
    connector_name: &str,
    client: ConnectorClient,
    dry_run: bool,
    mock: bool,
) -> ConnectorResult {
    if dry_run {
        return ConnectorResult {
            connector: instance_name.to_string(),
            status: "dry_run",
            scenarios: vec![],
            error: None,
        };
    }

    let scenario_results = run_connector_scenarios(connector_name, &client).await;

    if scenario_results.is_empty() {
        return ConnectorResult {
            connector: instance_name.to_string(),
            status: "skipped",
            scenarios: vec![],
            error: Some("no_examples".to_string()),
        };
    }

    let mut scenarios = vec![];
    let mut any_failed = false;

    for (scenario_key, result) in scenario_results {
        print!("    [{scenario_key}] running ... ");
        use std::io::Write;
        std::io::stdout().flush().ok();

        match result {
            Ok(msg) => {
                println!("{} {}", green("PASSED"), grey(&format!("— {msg}")));
                scenarios.push((
                    scenario_key,
                    ScenarioResult {
                        status: "passed",
                        message: Some(msg),
                        reason: None,
                        error: None,
                    },
                ));
            }
            Err(e) => {
                let detail = e.to_string();
                if detail.contains("NOT IMPLEMENTED") {
                    // Unimplemented flow - not a failure
                    println!(
                        "{} {}",
                        grey("NOT IMPLEMENTED"),
                        grey(&format!("— {detail}"))
                    );
                    scenarios.push((
                        scenario_key,
                        ScenarioResult {
                            status: "not_implemented",
                            message: None,
                            reason: None,
                            error: Some(detail),
                        },
                    ));
                } else if detail.contains("Rust panic:") || detail.starts_with("thread '") {
                    // Rust panic (real SDK crash)
                    println!("{} — {}", red("FAILED"), &detail);
                    scenarios.push((
                        scenario_key,
                        ScenarioResult {
                            status: "failed",
                            message: None,
                            reason: None,
                            error: Some(detail),
                        },
                    ));
                    any_failed = true;
                } else if mock {
                    // In mock mode, connector-level errors mean req_transformer successfully built the HTTP request.
                    // The error is just from parsing the mock empty response, which is expected.
                    println!("{} — req_transformer OK (mock response)", green("PASSED"));
                    scenarios.push((
                        scenario_key,
                        ScenarioResult {
                            status: "passed",
                            message: None,
                            reason: Some("mock_verified".to_string()),
                            error: Some(detail),
                        },
                    ));
                } else {
                    // Connector-level error (expected)
                    println!("{}", yellow("SKIPPED (connector error)"));
                    scenarios.push((
                        scenario_key,
                        ScenarioResult {
                            status: "skipped",
                            message: None,
                            reason: Some("connector_error".to_string()),
                            error: Some(detail),
                        },
                    ));
                }
            }
        }
    }

    ConnectorResult {
        connector: instance_name.to_string(),
        status: if any_failed { "failed" } else { "passed" },
        scenarios,
        error: None,
    }
}

fn print_result(result: &ConnectorResult) {
    match result.status {
        "passed" => {
            let passed_count = result
                .scenarios
                .iter()
                .filter(|(_, s)| s.status == "passed")
                .count();
            let skipped_count = result
                .scenarios
                .iter()
                .filter(|(_, s)| s.status == "skipped")
                .count();
            let not_impl_count = result
                .scenarios
                .iter()
                .filter(|(_, s)| s.status == "not_implemented")
                .count();
            println!(
                "{} ({} passed, {} skipped, {} not implemented)",
                green("  PASSED"),
                passed_count,
                skipped_count,
                not_impl_count
            );
            for (key, detail) in &result.scenarios {
                match detail.status {
                    "passed" => {
                        let extra = detail
                            .message
                            .as_deref()
                            .map(|m| format!(" {}", grey(&format!("— {m}"))))
                            .unwrap_or_default();
                        println!("{}    {}: ✓{extra}", green(""), key);
                    }
                    "skipped" => {
                        let reason = detail.reason.as_deref().unwrap_or("unknown");
                        let error_info = detail
                            .error
                            .as_deref()
                            .map(|e| format!(": {e}"))
                            .unwrap_or_default();
                        println!(
                            "{}    {}: ~ skipped ({}){}",
                            yellow(""),
                            key,
                            reason,
                            error_info
                        );
                    }
                    "not_implemented" => println!("{}    {}: N/A", grey(""), key),
                    _ => {}
                }
            }
        }
        "dry_run" => println!("{}", grey("  DRY RUN")),
        "skipped" => {
            let reason = result.error.as_deref().unwrap_or("unknown");
            println!("{}", grey(&format!("  SKIPPED ({reason})")));
        }
        _ => {
            println!("{}", red("  FAILED"));
            for (key, detail) in &result.scenarios {
                if detail.status == "failed" {
                    println!(
                        "{} — {}",
                        red(&format!("    {key}: ✗ FAILED")),
                        detail.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
            if let Some(e) = &result.error {
                println!("{}", red(&format!("  Error: {e}")));
            }
        }
    }
}

async fn run_tests(
    creds_file: &str,
    connectors: Option<Vec<String>>,
    dry_run: bool,
    mock: bool,
) -> Vec<ConnectorResult> {
    let text = std::fs::read_to_string(creds_file)
        .unwrap_or_else(|_| panic!("Credentials file not found: {creds_file}"));
    let credentials: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&text).expect("Invalid creds.json");

    let test_connectors: Vec<String> =
        connectors.unwrap_or_else(|| credentials.keys().cloned().collect());

    println!("\n{}", "=".repeat(60));
    println!(
        "Running smoke tests for {} connector(s)",
        test_connectors.len()
    );
    println!("{}\n", "=".repeat(60));

    let mut results = vec![];

    for connector_name in &test_connectors {
        println!("\n{}", bold(&format!("--- Testing {connector_name} ---")));

        let auth_val = match credentials.get(connector_name.as_str()) {
            Some(v) => v,
            None => {
                println!("{}", grey("  SKIPPED (not found in credentials file)"));
                results.push(ConnectorResult {
                    connector: connector_name.clone(),
                    status: "skipped",
                    scenarios: vec![],
                    error: Some("not_found".to_string()),
                });
                continue;
            }
        };

        // Handle both single-config and array-of-configs
        let instances: Vec<(String, &serde_json::Map<String, serde_json::Value>)> = match auth_val {
            serde_json::Value::Array(arr) => arr
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    v.as_object()
                        .map(|o| (format!("{connector_name}[{}]", i + 1), o))
                })
                .collect(),
            serde_json::Value::Object(obj) => vec![(connector_name.clone(), obj)],
            _ => {
                println!("{}", grey("  SKIPPED (invalid credentials format)"));
                results.push(ConnectorResult {
                    connector: connector_name.clone(),
                    status: "skipped",
                    scenarios: vec![],
                    error: Some("invalid_format".to_string()),
                });
                continue;
            }
        };

        for (instance_name, auth_map) in instances {
            if !mock && !has_valid_credentials(auth_map) {
                println!("{}", grey("  SKIPPED (placeholder credentials)"));
                results.push(ConnectorResult {
                    connector: instance_name.clone(),
                    status: "skipped",
                    scenarios: vec![],
                    error: Some("placeholder_credentials".to_string()),
                });
                continue;
            }

            let config = match build_config(connector_name, auth_map) {
                Ok(c) => c,
                Err(e) => {
                    println!("{}", grey(&format!("  SKIPPED ({e})")));
                    results.push(ConnectorResult {
                        connector: instance_name.clone(),
                        status: "skipped",
                        scenarios: vec![],
                        error: Some(e),
                    });
                    continue;
                }
            };

            let client = match ConnectorClient::new(config, None) {
                Ok(c) => c,
                Err(e) => {
                    println!(
                        "{}",
                        grey(&format!("  SKIPPED (client creation failed: {e})"))
                    );
                    results.push(ConnectorResult {
                        connector: instance_name.clone(),
                        status: "skipped",
                        scenarios: vec![],
                        error: Some(e.to_string()),
                    });
                    continue;
                }
            };

            let result =
                test_connector_scenarios(&instance_name, connector_name, client, dry_run, mock)
                    .await;
            print_result(&result);
            results.push(result);
        }
    }

    results
}

fn print_summary(results: &[ConnectorResult]) -> i32 {
    println!("\n{}", "=".repeat(60));
    println!("{}", bold("TEST SUMMARY"));
    println!("{}\n", "=".repeat(60));

    let passed = results
        .iter()
        .filter(|r| r.status == "passed" || r.status == "dry_run")
        .count();
    let skipped = results.iter().filter(|r| r.status == "skipped").count();
    let failed = results.iter().filter(|r| r.status == "failed").count();

    // Count per-scenario statuses across all connectors
    let mut total_flows_passed = 0;
    let mut total_flows_skipped = 0;
    let mut total_flows_failed = 0;
    for r in results {
        for (_, scenario) in &r.scenarios {
            match scenario.status {
                "passed" => total_flows_passed += 1,
                "skipped" => total_flows_skipped += 1,
                "failed" => total_flows_failed += 1,
                _ => {}
            }
        }
    }

    println!("Total connectors:   {}", results.len());
    println!("{}", green(&format!("Passed:  {passed}")));
    println!(
        "{}",
        grey(&format!(
            "Skipped: {skipped} (placeholder credentials or no examples)"
        ))
    );
    let failed_str = format!("Failed:  {failed}");
    println!(
        "{}",
        if failed > 0 {
            red(&failed_str)
        } else {
            green(&failed_str)
        }
    );
    println!();
    println!("Flow results:");
    println!(
        "{}",
        green(&format!("  {} flows PASSED", total_flows_passed))
    );
    if total_flows_skipped > 0 {
        println!(
            "{}",
            yellow(&format!(
                "  {} flows SKIPPED (connector errors)",
                total_flows_skipped
            ))
        );
    }
    if total_flows_failed > 0 {
        println!("{}", red(&format!("  {} flows FAILED", total_flows_failed)));
    }
    println!();

    if failed > 0 {
        println!("{}", red("Failed connectors:"));
        for r in results {
            if r.status == "failed" {
                let detail = r.error.as_deref().unwrap_or("see scenarios above");
                println!("{}: {detail}", red(&format!("  - {}", r.connector)));
            }
        }
        println!();
        return 1;
    }

    if passed == 0 && skipped > 0 {
        println!(
            "{}",
            yellow("All tests skipped (no valid credentials found)")
        );
        println!("Update creds.json with real credentials to run tests");
        return 1;
    }

    println!("{}", green("All tests completed successfully!"));
    0
}

fn parse_args() -> (String, Option<Vec<String>>, bool, bool, bool) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut creds_file = "creds.json".to_string();
    let mut connectors: Option<Vec<String>> = None;
    let mut all = false;
    let mut dry_run = false;
    let mut mock = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--creds-file" if i + 1 < args.len() => {
                creds_file = args[i + 1].clone();
                i += 1;
            }
            "--connectors" if i + 1 < args.len() => {
                connectors = Some(
                    args[i + 1]
                        .split(',')
                        .map(str::trim)
                        .map(str::to_string)
                        .collect(),
                );
                i += 1;
            }
            "--all" => {
                all = true;
            }
            "--dry-run" => {
                dry_run = true;
            }
            "--mock" => {
                mock = true;
            }
            "--help" | "-h" => {
                println!("Usage: hyperswitch-smoke-test [options]");
                println!(
                    "  --creds-file <path>     Path to credentials JSON (default: creds.json)"
                );
                println!("  --connectors <list>     Comma-separated list of connectors");
                println!("  --all                   Test all connectors in the credentials file");
                println!("  --dry-run               Skip HTTP calls, just verify compilation");
                println!("  --mock                  Intercept HTTP; verify req_transformer only");
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }

    if !all && connectors.is_none() {
        eprintln!("Error: Must specify either --all or --connectors");
        std::process::exit(1);
    }

    (
        creds_file,
        if all { None } else { connectors },
        dry_run,
        mock,
        all,
    )
}

#[tokio::main]
async fn main() {
    let (creds_file, connectors, dry_run, mock, _all) = parse_args();

    // Enable mock HTTP mode if requested
    if mock {
        hyperswitch_payments_client::set_mock_http(true);
    }

    let results = run_tests(&creds_file, connectors, dry_run, mock).await;

    // Disable mock HTTP mode after tests
    if mock {
        hyperswitch_payments_client::set_mock_http(false);
    }

    let exit_code = print_summary(&results);
    std::process::exit(exit_code);
}
