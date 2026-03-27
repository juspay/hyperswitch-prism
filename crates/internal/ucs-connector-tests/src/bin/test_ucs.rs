#![allow(clippy::print_stderr, clippy::print_stdout, clippy::too_many_arguments)]

//! Interactive UCS test runner.
//!
//! This binary guides users through connector/suite/scenario/backend selection
//! and executes the selected plan with optional report generation.

use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use serde::Deserialize;
use serde_json::Value;
use ucs_connector_tests::harness::{
    credentials::load_connector_auth,
    report::{
        append_report_best_effort, clear_report, extract_pm_and_pmt, now_epoch_ms, ReportEntry,
    },
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

/// CLI entrypoint for interactive flow.
fn main() {
    if let Err(error) = run_interactive() {
        eprintln!("[test_ucs] {error}");
        std::process::exit(1);
    }
}

/// Runs the end-to-end interactive prompt and execution sequence.
fn run_interactive() -> Result<(), String> {
    let mut report = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            "--report" => report = true,
            _ => {
                return Err(format!(
                    "unknown argument '{arg}'. this command is interactive."
                ));
            }
        }
    }

    let defaults = load_defaults();
    let endpoint = defaults
        .endpoint
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());

    let creds_file = std::env::var("CONNECTOR_AUTH_FILE_PATH")
        .ok()
        .or_else(|| std::env::var("UCS_CREDS_PATH").ok())
        .or(defaults.creds_file);

    if let Some(creds_file) = creds_file {
        std::env::set_var("CONNECTOR_AUTH_FILE_PATH", creds_file);
    }

    println!("\n=== Test UCS ===");
    println!("Interactive connector test runner");

    let discovered_connectors = discover_all_connectors().map_err(|error| error.to_string())?;
    let connector_selection = prompt_connector_selection(&discovered_connectors)?;
    let available_suites = suites_for_connector_selection(&connector_selection)?;
    let suite_selection = prompt_suite_selection(&available_suites)?;

    let scenario_selection = if let SuiteSelection::Specific(suite) = &suite_selection {
        let scenario_names = scenario_names_for_suite(suite).map_err(|error| error.to_string())?;
        prompt_scenario_selection(&scenario_names)?
    } else {
        ScenarioSelection::All
    };

    let backend = prompt_backend()?;
    let options = SuiteRunOptions {
        endpoint: Some(&endpoint),
        merchant_id: None,
        tenant_id: None,
        plaintext: true,
        backend,
        report,
    };

    println!("\n[test_ucs] starting run...");
    if report {
        clear_report();
    }

    let (passed, failed) = execute_plan(
        &connector_selection,
        &suite_selection,
        &scenario_selection,
        options,
        &endpoint,
    )
    .map_err(|error| error.to_string())?;

    println!("\n[test_ucs] grand total: passed={passed} failed={failed}");
    if failed > 0 {
        return Err("one or more scenarios failed".to_string());
    }

    Ok(())
}

/// Executes selected connectors/suites/scenarios and aggregates pass/fail counts.
fn execute_plan(
    connector_selection: &ConnectorSelection,
    suite_selection: &SuiteSelection,
    scenario_selection: &ScenarioSelection,
    options: SuiteRunOptions<'_>,
    endpoint: &str,
) -> Result<(usize, usize), ScenarioError> {
    let mut passed = 0usize;
    let mut failed = 0usize;

    let connectors = match connector_selection {
        ConnectorSelection::All(connectors) => connectors.clone(),
        ConnectorSelection::Specific(connector) => vec![connector.clone()],
    };

    for connector in connectors {
        println!("\n--- Connector: {connector} ---");

        match suite_selection {
            SuiteSelection::All => {
                let summary = run_all_suites_with_options(Some(&connector), options)?;
                for suite_summary in &summary.suites {
                    print_suite_results(suite_summary, endpoint, options.report);
                }
                passed += summary.passed;
                failed += summary.failed;
            }
            SuiteSelection::Specific(suite) => {
                if !is_suite_supported_for_connector(&connector, suite)? {
                    println!(
                        "[test_ucs] skipping unsupported suite '{}' for connector '{}'.",
                        suite, connector
                    );
                    continue;
                }

                let summary = match scenario_selection {
                    ScenarioSelection::All => {
                        run_suite_test_with_options(suite, Some(&connector), options)?
                    }
                    ScenarioSelection::Specific(scenario) => {
                        run_scenario_test_with_options(suite, scenario, Some(&connector), options)?
                    }
                };

                print_suite_results(&summary, endpoint, options.report);
                passed += summary.passed;
                failed += summary.failed;
            }
        }
    }

    Ok((passed, failed))
}

/// Prints suite-level results and appends each scenario to report output.
fn print_suite_results(summary: &SuiteRunSummary, endpoint: &str, report: bool) {
    if summary.results.is_empty() {
        println!(
            "[test_ucs] suite={} connector={} had no runnable scenarios",
            summary.suite, summary.connector
        );
        return;
    }

    for result in &summary.results {
        let template_req =
            get_the_grpc_req_for_connector(&result.suite, &result.scenario, &summary.connector)
                .ok();
        let req_for_report = result.req_body.as_ref().or(template_req.as_ref());
        let (pm, pmt) = extract_pm_and_pmt(req_for_report);
        if report {
            write_report_entry(
                &result.suite,
                &result.scenario,
                &summary.connector,
                endpoint,
                pm.as_deref(),
                pmt.as_deref(),
                result.is_dependency,
                if result.passed { "PASS" } else { "FAIL" },
                None,
                result.error.clone(),
                result.dependency.clone(),
                result.req_body.clone(),
                result.res_body.clone(),
                result.grpc_request.clone(),
                result.grpc_response.clone(),
            );
        }

        if result.passed {
            println!(
                "[test_ucs] assertion result for '{}': PASS",
                result.scenario
            );
        } else {
            println!(
                "[test_ucs] assertion result for '{}': FAIL ({})",
                result.scenario,
                compact_error_for_console(result.error.as_deref())
            );
        }
    }

    println!(
        "[test_ucs] summary suite={} connector={} passed={} failed={}",
        summary.suite, summary.connector, summary.passed, summary.failed
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

/// Serializes one scenario execution into `ReportEntry` and appends it.
fn write_report_entry(
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
) {
    append_report_best_effort(ReportEntry {
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
    });
}

/// Prompts user to run one connector or all configured connectors.
fn prompt_connector_selection(
    discovered_connectors: &[String],
) -> Result<ConnectorSelection, String> {
    println!("\n1) Connectors to test:");
    let mode = prompt_choice(
        "Select connector option",
        &[
            "Test all configured connectors",
            "Test one specific connector",
        ],
    )?;

    if mode == 0 {
        let connectors = runnable_configured_connectors()?;
        if connectors.is_empty() {
            return Err("no runnable connectors found from configured list".to_string());
        }
        println!("[test_ucs] selected connectors: {}", connectors.join(", "));
        return Ok(ConnectorSelection::All(connectors));
    }

    if discovered_connectors.is_empty() {
        return Err("no connectors discovered under connector_specs".to_string());
    }

    println!(
        "[test_ucs] available connectors: {}",
        discovered_connectors.join(", ")
    );

    loop {
        let input = prompt_input("Enter connector name")?;
        if !discovered_connectors
            .iter()
            .any(|connector| connector == &input)
        {
            println!("[test_ucs] invalid connector '{}'. try again.", input);
            continue;
        }

        match load_connector_auth(&input) {
            Ok(_) => return Ok(ConnectorSelection::Specific(input)),
            Err(error) => {
                println!(
                    "[test_ucs] connector '{}' credentials unavailable: {}. try another connector.",
                    input, error
                );
            }
        }
    }
}

/// Prompts user for suite mode (all or one specific suite).
fn prompt_suite_selection(available_suites: &[String]) -> Result<SuiteSelection, String> {
    println!("\n2) Suites to test:");
    let mode = prompt_choice("Select suite option", &["All suites", "One specific suite"])?;

    if mode == 0 {
        return Ok(SuiteSelection::All);
    }

    if available_suites.is_empty() {
        return Err("no suites available for the selected connector scope".to_string());
    }

    println!(
        "[test_ucs] available suites: {}",
        available_suites.join(", ")
    );

    loop {
        let suite = prompt_input("Enter suite name")?;
        if available_suites.iter().any(|candidate| candidate == &suite) {
            return Ok(SuiteSelection::Specific(suite));
        }
        println!("[test_ucs] invalid suite '{}'. try again.", suite);
    }
}

/// Prompts user for scenario mode (all or one specific scenario).
fn prompt_scenario_selection(available_scenarios: &[String]) -> Result<ScenarioSelection, String> {
    println!("\n3) Scenarios to test:");
    let mode = prompt_choice(
        "Select scenario option",
        &["All scenarios in selected suite", "One specific scenario"],
    )?;

    if mode == 0 {
        return Ok(ScenarioSelection::All);
    }

    if available_scenarios.is_empty() {
        return Err("no scenarios available for the selected suite".to_string());
    }

    println!(
        "[test_ucs] available scenarios: {}",
        available_scenarios.join(", ")
    );

    loop {
        let scenario = prompt_input("Enter scenario name")?;
        if available_scenarios
            .iter()
            .any(|candidate| candidate == &scenario)
        {
            return Ok(ScenarioSelection::Specific(scenario));
        }
        println!("[test_ucs] invalid scenario '{}'. try again.", scenario);
    }
}

/// Prompts execution backend selection (grpcurl or SDK/FFI).
fn prompt_backend() -> Result<ExecutionBackend, String> {
    println!("\n4) Interface:");
    let mode = prompt_choice("Select interface", &["gRPC", "SDK"])?;
    if mode == 0 {
        Ok(ExecutionBackend::Grpcurl)
    } else {
        Ok(ExecutionBackend::SdkFfi)
    }
}

/// Generic numbered-menu prompt helper.
fn prompt_choice(label: &str, options: &[&str]) -> Result<usize, String> {
    println!("{label}:");
    for (index, option) in options.iter().enumerate() {
        println!("  {}) {}", index + 1, option);
    }

    loop {
        print!("> ");
        io::stdout().flush().map_err(|error| error.to_string())?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| error.to_string())?;
        let trimmed = input.trim();

        if let Ok(choice) = trimmed.parse::<usize>() {
            if (1..=options.len()).contains(&choice) {
                return Ok(choice - 1);
            }
        }

        println!("Please enter a number between 1 and {}.", options.len());
    }
}

/// Reads one line of user input from stdin.
fn prompt_input(label: &str) -> Result<String, String> {
    loop {
        print!("{label}: ");
        io::stdout().flush().map_err(|error| error.to_string())?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| error.to_string())?;

        let trimmed = input.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }

        println!("Input cannot be empty.");
    }
}

/// Returns configured connectors that currently have valid credentials.
fn runnable_configured_connectors() -> Result<Vec<String>, String> {
    let connectors = configured_all_connectors();
    let mut runnable = Vec::new();

    for connector in connectors {
        match load_connector_auth(&connector) {
            Ok(_) => runnable.push(connector),
            Err(error) => println!(
                "[test_ucs] skipping connector '{}' due to missing/invalid credentials: {}",
                connector, error
            ),
        }
    }

    Ok(runnable)
}

/// Computes suite intersection/union based on connector selection mode.
fn suites_for_connector_selection(selection: &ConnectorSelection) -> Result<Vec<String>, String> {
    let connectors = match selection {
        ConnectorSelection::All(connectors) => connectors.clone(),
        ConnectorSelection::Specific(connector) => vec![connector.clone()],
    };

    let mut suites = BTreeSet::new();
    for connector in connectors {
        for suite in
            load_supported_suites_for_connector(&connector).map_err(|error| error.to_string())?
        {
            suites.insert(suite);
        }
    }

    Ok(suites.into_iter().collect())
}

/// Lists scenario names for one suite in stable sort order.
fn scenario_names_for_suite(suite: &str) -> Result<Vec<String>, ScenarioError> {
    Ok(load_suite_scenarios(suite)?.keys().cloned().collect())
}

#[derive(Debug, Default, Clone, Deserialize)]
struct StoredDefaults {
    endpoint: Option<String>,
    creds_file: Option<String>,
}

/// Returns persisted defaults path used by interactive runner.
fn defaults_path() -> PathBuf {
    if let Ok(path) = std::env::var("UCS_RUN_TEST_DEFAULTS_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("ucs-connector-tests")
            .join("run_test_defaults.json");
    }

    PathBuf::from(".ucs_run_test_defaults.json")
}

/// Loads persisted defaults (endpoint and creds file), if present.
fn load_defaults() -> StoredDefaults {
    let path = defaults_path();
    let Ok(content) = fs::read_to_string(path) else {
        return StoredDefaults::default();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

/// Prints usage/help text for interactive runner.
fn print_usage() {
    eprintln!(
        "Usage:\n  cargo run -p ucs-connector-tests --bin test_ucs [--report]\n\nInteractive flow:\n  1) Select connectors (all or specific)\n  2) Select suites (all or specific)\n  3) If suite is specific: select scenarios (all or specific)\n  4) Select interface (gRPC or SDK)\n\nNotes:\n  - Pass --report to generate report.json + test_report/ markdown files\n  - Without --report, only test execution output is printed\n  - Exits with non-zero when any scenario fails"
    );
}

#[derive(Debug, Clone)]
enum ConnectorSelection {
    All(Vec<String>),
    Specific(String),
}

#[derive(Debug, Clone)]
enum SuiteSelection {
    All,
    Specific(String),
}

#[derive(Debug, Clone)]
enum ScenarioSelection {
    All,
    Specific(String),
}
