#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::too_many_arguments,
    clippy::indexing_slicing
)]

//! Interactive + non-interactive UCS test runner.
//!
//! ## Non-interactive mode (flags)
//!
//! ```
//! cargo run -p integration-tests --bin test_ucs -- \
//!     --connector stripe \
//!     --suite authorize \
//!     --scenario no3ds_auto_capture_credit_card \
//!     --interface grpc \
//!     --report
//! ```
//!
//! | Flag | Description |
//! |------|-------------|
//! | `--connector <name>` | Run for a single connector (omit for all) |
//! | `--all-connectors`   | Explicitly run all configured connectors |
//! | `--suite <name>`     | Run a single suite (omit for all) |
//! | `--scenario <name>`  | Run a single scenario within the selected suite |
//! | `--interface grpc\|sdk` | Execution interface (default: grpc) |
//! | `--endpoint <addr>`  | Override gRPC endpoint |
//! | `--report`           | Write report.json + markdown test reports |
//! | `--interactive`      | Open the step-by-step searchable TUI wizard |

use std::{collections::BTreeSet, fs, path::PathBuf};

use inquire::{list_option::ListOption, validator::Validation, Confirm, MultiSelect, Select, Text};
use integration_tests::harness::{
    credentials::load_connector_config,
    report::{append_report_batch_best_effort, extract_pm_and_pmt, now_epoch_ms, ReportEntry},
    scenario_api::{
        get_the_grpc_req_for_connector, run_all_suites_with_options,
        run_scenario_test_with_options, run_suite_test_with_options, ExecutionBackend,
        SuiteRunOptions, SuiteRunSummary, DEFAULT_ENDPOINT,
    },
    scenario_loader::{
        configured_all_connectors, discover_all_connectors, is_suite_supported_for_connector,
        load_suite_scenarios, load_supported_suites_for_connector,
    },
    scenario_types::ScenarioError,
};
use serde::Deserialize;
use serde_json::Value;

// ── Entrypoint ─────────────────────────────────────────────────────────────────

fn main() {
    if let Err(error) = run() {
        eprintln!("[test_ucs] {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // --interactive is now an explicit flag.
    // Any other recognised non-interactive flag → non-interactive mode.
    let wants_interactive = args.iter().any(|a| a == "--interactive");
    let has_non_interactive_flag = args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--connector"
                | "--all-connectors"
                | "--suite"
                | "--scenario"
                | "--interface"
                | "--endpoint"
        )
    });

    // --help is handled inside both modes; check it up-front too.
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return Ok(());
    }

    if wants_interactive {
        run_interactive(&args)
    } else if has_non_interactive_flag {
        run_non_interactive(&args)
    } else {
        // No flags at all → run all connectors / all suites (non-interactive batch)
        run_non_interactive(&args)
    }
}

// ── Non-interactive mode ───────────────────────────────────────────────────────

fn run_non_interactive(args: &[String]) -> Result<(), String> {
    let mut connector: Option<String> = None;
    let mut all_connectors = false;
    let mut suite: Option<String> = None;
    let mut scenario: Option<String> = None;
    let mut interface_str: Option<String> = None;
    let mut endpoint_flag: Option<String> = None;
    let mut report = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            "--interactive" => {
                // already handled in run(); ignore here
            }
            "--connector" => {
                connector = Some(iter.next().ok_or("--connector requires a value")?.clone());
            }
            "--all-connectors" => {
                all_connectors = true;
            }
            "--suite" => {
                suite = Some(iter.next().ok_or("--suite requires a value")?.clone());
            }
            "--scenario" => {
                scenario = Some(iter.next().ok_or("--scenario requires a value")?.clone());
            }
            "--interface" => {
                interface_str = Some(
                    iter.next()
                        .ok_or("--interface requires a value (grpc or sdk)")?
                        .clone(),
                );
            }
            "--endpoint" => {
                endpoint_flag = Some(iter.next().ok_or("--endpoint requires a value")?.clone());
            }
            "--report" => {
                report = true;
            }
            other => {
                return Err(format!(
                    "unknown flag '{other}'. run with --help for usage."
                ));
            }
        }
    }

    if scenario.is_some() && suite.is_none() {
        return Err("--scenario requires --suite to be specified".to_string());
    }

    let backend = parse_backend(interface_str.as_deref().unwrap_or("grpc"))?;
    let defaults = load_defaults();
    let endpoint = endpoint_flag
        .or_else(|| std::env::var("UCS_ENDPOINT").ok())
        .or_else(|| defaults.endpoint.clone())
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());

    apply_creds_from_env_or_defaults(&defaults);

    let options = SuiteRunOptions {
        endpoint: Some(&endpoint),
        merchant_id: None,
        tenant_id: None,
        plaintext: true,
        backend,
        report,
    };

    let connector_selection = if let Some(name) = connector.filter(|_| !all_connectors) {
        ConnectorSelection::Specific(vec![name])
    } else {
        ConnectorSelection::All(runnable_configured_connectors()?)
    };

    let connector_list = match &connector_selection {
        ConnectorSelection::All(list) | ConnectorSelection::Specific(list) => list,
    };
    if connector_list.is_empty() {
        return Err("no runnable connectors found".to_string());
    }

    let suite_selection = match suite {
        Some(s) => SuiteSelection::Specific(vec![s]),
        None => SuiteSelection::All,
    };

    let scenario_selection = match scenario {
        Some(s) => ScenarioSelection::Specific(s),
        None => ScenarioSelection::All,
    };

    println!("[test_ucs] non-interactive run starting...");

    let (passed, failed, skipped) = execute_plan(
        &connector_selection,
        &suite_selection,
        &scenario_selection,
        options,
        &endpoint,
    )
    .map_err(|e| e.to_string())?;

    println!("\n[test_ucs] grand total: passed={passed} failed={failed} skipped={skipped}");
    if failed > 0 {
        return Err("one or more scenarios failed".to_string());
    }

    Ok(())
}

// ── Interactive mode ───────────────────────────────────────────────────────────

fn run_interactive(args: &[String]) -> Result<(), String> {
    // Accepted pre-flags: --interface, --report, --endpoint
    // (interface selection is also offered in the wizard, but CLI can override)
    let mut report_flag: Option<bool> = None;
    let mut endpoint_flag: Option<String> = None;
    let mut interface_flag: Option<String> = None;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--interactive" | "-h" | "--help" => {}
            "--report" => report_flag = Some(true),
            "--endpoint" => {
                endpoint_flag = Some(iter.next().ok_or("--endpoint requires a value")?.clone());
            }
            "--interface" => {
                interface_flag = Some(iter.next().ok_or("--interface requires a value")?.clone());
            }
            other => {
                return Err(format!(
                    "unknown argument '{other}' in interactive mode. run with --help for usage."
                ));
            }
        }
    }

    let defaults = load_defaults();
    apply_creds_from_env_or_defaults(&defaults);

    println!("\n=== UCS Connector Test Runner ===");
    println!("Use arrow keys to navigate, type to filter, Enter to select.\n");

    // ── Step 1: Connector scope ────────────────────────────────────────────────
    let discovered = discover_all_connectors().map_err(|e| e.to_string())?;
    if discovered.is_empty() {
        return Err("no connectors found under connector_specs/".to_string());
    }

    let connector_scope = Select::new(
        "1. Connector scope:",
        vec![
            "All connectors",
            "One connector",
            "Multiple connectors (multi-select)",
        ],
    )
    .prompt()
    .map_err(|e| format!("connector scope: {e}"))?;

    let connector_selection = match connector_scope {
        "All connectors" => ConnectorSelection::All(runnable_configured_connectors()?),
        "One connector" => {
            let runnable = runnable_configured_connectors()?;
            let name = Select::new("   Pick a connector:", runnable)
                .prompt()
                .map_err(|e| format!("connector pick: {e}"))?;
            ConnectorSelection::Specific(vec![name])
        }
        _ => {
            // Multi-select
            let runnable = runnable_configured_connectors()?;
            let chosen = MultiSelect::new(
                "   Pick connectors (space to select, Enter when done):",
                runnable,
            )
            .with_validator(|choices: &[ListOption<&String>]| {
                if choices.is_empty() {
                    Ok(Validation::Invalid("Select at least one connector.".into()))
                } else {
                    Ok(Validation::Valid)
                }
            })
            .prompt()
            .map_err(|e| format!("connector multi-select: {e}"))?;
            ConnectorSelection::Specific(chosen)
        }
    };

    let selected_connectors = match &connector_selection {
        ConnectorSelection::All(list) | ConnectorSelection::Specific(list) => list.clone(),
    };
    if selected_connectors.is_empty() {
        return Err("no runnable connectors available (check credentials)".to_string());
    }

    // ── Step 2: Suite scope ────────────────────────────────────────────────────
    let available_suites = suites_for_connectors(&selected_connectors)?;

    let suite_scope = Select::new(
        "2. Suite scope:",
        vec!["All suites", "One suite", "Multiple suites (multi-select)"],
    )
    .prompt()
    .map_err(|e| format!("suite scope: {e}"))?;

    let suite_selection = match suite_scope {
        "All suites" => SuiteSelection::All,
        "One suite" => {
            let name = Select::new("   Pick a suite:", available_suites.clone())
                .prompt()
                .map_err(|e| format!("suite pick: {e}"))?;
            SuiteSelection::Specific(vec![name])
        }
        _ => {
            let chosen = MultiSelect::new(
                "   Pick suites (space to select, Enter when done):",
                available_suites.clone(),
            )
            .with_validator(|choices: &[ListOption<&String>]| {
                if choices.is_empty() {
                    Ok(Validation::Invalid("Select at least one suite.".into()))
                } else {
                    Ok(Validation::Valid)
                }
            })
            .prompt()
            .map_err(|e| format!("suite multi-select: {e}"))?;
            SuiteSelection::Specific(chosen)
        }
    };

    // ── Step 3: Scenario scope ─────────────────────────────────────────────────
    let scenario_selection = match &suite_selection {
        SuiteSelection::Specific(suites) if suites.len() == 1 => {
            let suite_name = &suites[0];
            let all_scenarios = scenario_names_for_suite(suite_name).map_err(|e| e.to_string())?;

            let scenario_scope = Select::new(
                "3. Scenario scope:",
                vec![
                    "All scenarios",
                    "One scenario",
                    "Multiple scenarios (multi-select)",
                ],
            )
            .prompt()
            .map_err(|e| format!("scenario scope: {e}"))?;

            match scenario_scope {
                "All scenarios" => ScenarioSelection::All,
                "One scenario" => {
                    let name = Select::new("   Pick a scenario:", all_scenarios)
                        .prompt()
                        .map_err(|e| format!("scenario pick: {e}"))?;
                    ScenarioSelection::Specific(name)
                }
                _ => {
                    let chosen = MultiSelect::new(
                        "   Pick scenarios (space to select, Enter when done):",
                        all_scenarios,
                    )
                    .with_validator(|choices: &[ListOption<&String>]| {
                        if choices.is_empty() {
                            Ok(Validation::Invalid("Select at least one scenario.".into()))
                        } else {
                            Ok(Validation::Valid)
                        }
                    })
                    .prompt()
                    .map_err(|e| format!("scenario multi-select: {e}"))?;
                    ScenarioSelection::Multiple(chosen)
                }
            }
        }
        _ => {
            // All suites or multiple suites → run all scenarios
            ScenarioSelection::All
        }
    };

    // ── Step 4: Interface ──────────────────────────────────────────────────────
    let (interface_label, backend) = if let Some(ref iface) = interface_flag {
        // pre-selected via --interface flag, skip the wizard step
        let iface = iface.as_str();
        let label = if iface == "grpc" { "gRPC" } else { "SDK" };
        let be = parse_backend(iface)?;
        (label, be)
    } else {
        let label = Select::new("4. Interface:", vec!["gRPC", "SDK"])
            .prompt()
            .map_err(|e| format!("interface: {e}"))?;
        let be = if label == "gRPC" {
            ExecutionBackend::Grpcurl
        } else {
            ExecutionBackend::SdkFfi
        };
        (label, be)
    };

    // ── Step 5: Endpoint ───────────────────────────────────────────────────────
    let default_endpoint = endpoint_flag
        .or_else(|| std::env::var("UCS_ENDPOINT").ok())
        .or_else(|| defaults.endpoint.clone())
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());

    let endpoint = Text::new("5. gRPC endpoint:")
        .with_default(&default_endpoint)
        .prompt()
        .map_err(|e| format!("endpoint: {e}"))?;

    // ── Step 6: Report ─────────────────────────────────────────────────────────
    let report = if let Some(r) = report_flag {
        r
    } else {
        Confirm::new("6. Generate test report?")
            .with_default(true)
            .prompt()
            .map_err(|e| format!("report: {e}"))?
    };

    // ── Step 7: Show equivalent command ───────────────────────────────────────
    let equivalent_cmd = build_equivalent_command(
        &connector_selection,
        &suite_selection,
        &scenario_selection,
        interface_label,
        &endpoint,
        report,
    );
    println!("\nEquivalent command:");
    println!("  {equivalent_cmd}");

    let confirmed = Confirm::new("\nRun the tests?")
        .with_default(true)
        .prompt()
        .map_err(|e| format!("confirm: {e}"))?;

    if !confirmed {
        println!("[test_ucs] aborted.");
        return Ok(());
    }

    // ── Execute ────────────────────────────────────────────────────────────────
    let options = SuiteRunOptions {
        endpoint: Some(&endpoint),
        merchant_id: None,
        tenant_id: None,
        plaintext: true,
        backend,
        report,
    };

    println!("\n[test_ucs] starting run...");

    let (passed, failed, skipped) = execute_plan(
        &connector_selection,
        &suite_selection,
        &scenario_selection,
        options,
        &endpoint,
    )
    .map_err(|e| e.to_string())?;

    println!("\n[test_ucs] grand total: passed={passed} failed={failed} skipped={skipped}");
    if failed > 0 {
        return Err("one or more scenarios failed".to_string());
    }

    Ok(())
}

// ── Equivalent command builder ─────────────────────────────────────────────────

fn build_equivalent_command(
    connector_selection: &ConnectorSelection,
    suite_selection: &SuiteSelection,
    scenario_selection: &ScenarioSelection,
    interface: &str,
    endpoint: &str,
    report: bool,
) -> String {
    let mut parts =
        vec!["make cargo ARGS=\"run -p integration-tests --bin test_ucs --".to_string()];

    match connector_selection {
        ConnectorSelection::All(_) => parts.push("--all-connectors".to_string()),
        ConnectorSelection::Specific(list) if list.len() == 1 => {
            parts.push(format!("--connector {}", list[0]));
        }
        ConnectorSelection::Specific(list) => {
            // Multiple connectors: show first + note
            parts.push(format!("--connector {}", list[0]));
            parts.push(format!("# (+ {} more)", list.len() - 1));
        }
    }

    match suite_selection {
        SuiteSelection::All => {}
        SuiteSelection::Specific(suites) if suites.len() == 1 => {
            parts.push(format!("--suite {}", suites[0]));
        }
        SuiteSelection::Specific(suites) => {
            parts.push(format!("--suite {}", suites[0]));
            parts.push(format!("# (+ {} more)", suites.len() - 1));
        }
    }

    match scenario_selection {
        ScenarioSelection::All => {}
        ScenarioSelection::Specific(s) => {
            parts.push(format!("--scenario {s}"));
        }
        ScenarioSelection::Multiple(list) => {
            parts.push(format!("--scenario {}", list[0]));
            parts.push(format!("# (+ {} more)", list.len() - 1));
        }
    }

    let interface_flag = if interface == "gRPC" { "grpc" } else { "sdk" };
    parts.push(format!("--interface {interface_flag}"));
    parts.push(format!("--endpoint {endpoint}"));

    if report {
        parts.push("--report".to_string());
    }

    format!("{}\"", parts.join(" \\\n    "))
}

// ── Execution ──────────────────────────────────────────────────────────────────

fn execute_plan(
    connector_selection: &ConnectorSelection,
    suite_selection: &SuiteSelection,
    scenario_selection: &ScenarioSelection,
    options: SuiteRunOptions<'_>,
    endpoint: &str,
) -> Result<(usize, usize, usize), ScenarioError> {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    let connectors = match connector_selection {
        ConnectorSelection::All(list) | ConnectorSelection::Specific(list) => list.clone(),
    };

    let suites_to_run: Option<Vec<String>> = match suite_selection {
        SuiteSelection::All => None,
        SuiteSelection::Specific(list) => Some(list.clone()),
    };

    for connector in connectors {
        println!("\n--- Connector: {connector} ---");

        match &suites_to_run {
            None => {
                // All suites
                let summary = run_all_suites_with_options(Some(&connector), options)?;
                for suite_summary in &summary.suites {
                    print_suite_results(suite_summary, endpoint, options.report);
                }
                passed += summary.passed;
                failed += summary.failed;
                skipped += summary.skipped;
            }
            Some(suites) => {
                for suite in suites {
                    if !is_suite_supported_for_connector(&connector, suite)? {
                        println!(
                            "[test_ucs] skipping unsupported suite '{suite}' for connector '{connector}'."
                        );
                        continue;
                    }

                    let summary = match scenario_selection {
                        ScenarioSelection::All => {
                            run_suite_test_with_options(suite, Some(&connector), options)?
                        }
                        ScenarioSelection::Specific(scenario) => run_scenario_test_with_options(
                            suite,
                            scenario,
                            Some(&connector),
                            options,
                        )?,
                        ScenarioSelection::Multiple(scenarios) => {
                            // Run each scenario in the suite individually and aggregate
                            let mut agg = run_scenario_test_with_options(
                                suite,
                                &scenarios[0],
                                Some(&connector),
                                options,
                            )?;
                            for scenario in &scenarios[1..] {
                                let s = run_scenario_test_with_options(
                                    suite,
                                    scenario,
                                    Some(&connector),
                                    options,
                                )?;
                                agg.results.extend(s.results);
                                agg.passed += s.passed;
                                agg.failed += s.failed;
                                agg.skipped += s.skipped;
                            }
                            agg
                        }
                    };

                    print_suite_results(&summary, endpoint, options.report);
                    passed += summary.passed;
                    failed += summary.failed;
                    skipped += summary.skipped;
                }
            }
        }
    }

    Ok((passed, failed, skipped))
}

// ── Output helpers ─────────────────────────────────────────────────────────────

fn print_suite_results(summary: &SuiteRunSummary, endpoint: &str, report: bool) {
    if summary.results.is_empty() {
        println!(
            "[test_ucs] suite={} connector={} had no runnable scenarios",
            summary.suite, summary.connector
        );
        return;
    }

    let mut batch: Vec<ReportEntry> = Vec::new();

    for result in &summary.results {
        let template_req =
            get_the_grpc_req_for_connector(&result.suite, &result.scenario, &summary.connector)
                .ok();
        let req_for_report = result.req_body.as_ref().or(template_req.as_ref());
        let (pm, pmt) = extract_pm_and_pmt(req_for_report);
        if report {
            batch.push(build_report_entry(
                &result.suite,
                &result.scenario,
                &summary.connector,
                endpoint,
                pm.as_deref(),
                pmt.as_deref(),
                result.is_dependency,
                if result.passed {
                    "PASS"
                } else if result.skipped {
                    "SKIP"
                } else {
                    "FAIL"
                },
                None,
                result.error.clone(),
                result.dependency.clone(),
                result.req_body.clone(),
                result.res_body.clone(),
                result.grpc_request.clone(),
                result.grpc_response.clone(),
            ));
        }

        if result.passed {
            println!(
                "[test_ucs] assertion result for '{}': PASS",
                result.scenario
            );
        } else if result.skipped {
            println!(
                "[test_ucs] assertion result for '{}': SKIP ({})",
                result.scenario,
                result.error.as_deref().unwrap_or("no reason given")
            );
        } else {
            println!(
                "[test_ucs] assertion result for '{}': FAIL ({})",
                result.scenario,
                compact_error_for_console(result.error.as_deref())
            );
        }
    }

    if !batch.is_empty() {
        append_report_batch_best_effort(batch);
    }

    println!(
        "[test_ucs] summary suite={} connector={} passed={} failed={} skipped={}",
        summary.suite, summary.connector, summary.passed, summary.failed, summary.skipped
    );
}

fn compact_error_for_console(error: Option<&str>) -> String {
    let Some(error) = error else {
        return "unknown error".to_string();
    };

    for line in error.lines() {
        let trimmed = line.trim();
        if let Some(message) = trimmed.strip_prefix("Message:") {
            let message = message.trim();
            if !message.is_empty() {
                return truncate_for_console(message, 220);
            }
        }
    }

    for line in error.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed == "ERROR:"
            || trimmed.starts_with("Resolved method descriptor:")
            || trimmed.starts_with("Request metadata to send:")
            || trimmed.starts_with("Response headers received:")
            || trimmed.starts_with("Response trailers received:")
            || trimmed.starts_with("Sent ")
        {
            continue;
        }
        return truncate_for_console(trimmed, 220);
    }

    truncate_for_console(error.trim(), 220)
}

fn truncate_for_console(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn build_report_entry(
    suite: &str,
    scenario: &str,
    connector: &str,
    endpoint: &str,
    pm: Option<&str>,
    pmt: Option<&str>,
    is_dependency: bool,
    assertion_result: &str,
    response_status: Option<String>,
    error: Option<String>,
    dependency: Vec<String>,
    req_body: Option<Value>,
    res_body: Option<Value>,
    grpc_request: Option<String>,
    grpc_response: Option<String>,
) -> ReportEntry {
    ReportEntry {
        run_at_epoch_ms: now_epoch_ms(),
        suite: suite.to_string(),
        scenario: scenario.to_string(),
        connector: connector.to_string(),
        pm: pm.map(ToString::to_string),
        pmt: pmt.map(ToString::to_string),
        endpoint: endpoint.to_string(),
        is_dependency,
        assertion_result: assertion_result.to_string(),
        response_status,
        error,
        dependency,
        req_body,
        res_body,
        grpc_request,
        grpc_response,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn parse_backend(s: &str) -> Result<ExecutionBackend, String> {
    match s {
        "grpc" => Ok(ExecutionBackend::Grpcurl),
        "sdk" => Ok(ExecutionBackend::SdkFfi),
        other => Err(format!("unknown interface '{other}': use 'grpc' or 'sdk'")),
    }
}

fn apply_creds_from_env_or_defaults(defaults: &StoredDefaults) {
    let creds_file = std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .ok()
        .or_else(|| std::env::var("UCS_CREDS_PATH").ok())
        .or_else(|| defaults.creds_file.clone());

    if let Some(creds_file) = creds_file {
        std::env::set_var("CONNECTOR_AUTH_FILE_PATH", creds_file);
    }
}

fn runnable_configured_connectors() -> Result<Vec<String>, String> {
    let connectors = configured_all_connectors();
    let mut runnable = Vec::new();

    for connector in connectors {
        match load_connector_config(&connector) {
            Ok(_) => runnable.push(connector),
            Err(err) => println!(
                "[test_ucs] skipping connector '{}' due to missing/invalid credentials: {}",
                connector, err
            ),
        }
    }

    Ok(runnable)
}

fn suites_for_connectors(connectors: &[String]) -> Result<Vec<String>, String> {
    let mut suites = BTreeSet::new();
    for connector in connectors {
        for suite in load_supported_suites_for_connector(connector).map_err(|e| e.to_string())? {
            suites.insert(suite);
        }
    }
    Ok(suites.into_iter().collect())
}

fn scenario_names_for_suite(suite: &str) -> Result<Vec<String>, ScenarioError> {
    Ok(load_suite_scenarios(suite)?.keys().cloned().collect())
}

// ── Persisted defaults ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Deserialize)]
struct StoredDefaults {
    endpoint: Option<String>,
    creds_file: Option<String>,
}

fn defaults_path() -> PathBuf {
    if let Ok(path) = std::env::var("UCS_RUN_TEST_DEFAULTS_PATH") {
        return PathBuf::from(path);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("integration-tests")
            .join("run_test_defaults.json");
    }
    PathBuf::from(".ucs_run_test_defaults.json")
}

fn load_defaults() -> StoredDefaults {
    let path = defaults_path();
    let Ok(content) = fs::read_to_string(path) else {
        return StoredDefaults::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

// ── Usage ──────────────────────────────────────────────────────────────────────

fn print_usage() {
    eprintln!(
        r#"Usage:
  test-prism [FLAGS]           ← preferred (installed by setup)
  cargo run -p integration-tests --bin test_ucs [FLAGS]

FLAGS
  --interactive          Open the step-by-step searchable TUI wizard
  --connector <name>     Run tests for a single connector
  --all-connectors       Run tests for all configured connectors
  --suite <name>         Run tests for a single suite
  --scenario <name>      Run a single scenario (requires --suite)
  --interface grpc|sdk   Execution interface (default: grpc)
  --endpoint <addr>      Override gRPC endpoint
  --report               Write report.json + markdown test_report/ files
  -h, --help             Print this help and exit

Default (no flags):
  Runs all connectors / all suites / all scenarios (non-interactive batch).

Interactive wizard (--interactive):
  Step-by-step searchable selection for connector → suite → scenario →
  interface → endpoint → report.  Shows equivalent cargo command before
  executing so you can copy it for future non-interactive runs.

Credential resolution order:
  1. CONNECTOR_AUTH_FILE_PATH environment variable
  2. UCS_CREDS_PATH environment variable
  3. ~/.config/integration-tests/run_test_defaults.json
  4. creds.json (repo default)

Examples:
  test-prism                                   # full batch run
  test-prism --interactive                     # TUI wizard
  test-prism --connector stripe                # all suites for stripe
  test-prism --interface sdk --connector stripe
  test-prism --connector stripe --suite authorize
  test-prism --connector stripe --suite authorize \
      --scenario no3ds_auto_capture_credit_card --report
"#
    );
}

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum ConnectorSelection {
    All(Vec<String>),
    Specific(Vec<String>),
}

#[derive(Debug, Clone)]
enum SuiteSelection {
    All,
    Specific(Vec<String>),
}

#[derive(Debug, Clone)]
enum ScenarioSelection {
    All,
    Specific(String),
    Multiple(Vec<String>),
}
