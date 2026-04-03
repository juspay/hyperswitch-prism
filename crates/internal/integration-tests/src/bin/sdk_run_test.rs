#![allow(clippy::print_stderr, clippy::print_stdout, clippy::too_many_arguments)]

//! Suite runner (SDK/FFI backend).
//!
//! Mirrors `suite_run_test` behavior, but executes requests through SDK
//! transformers and direct HTTP connector calls instead of grpcurl.

use std::{fs, path::PathBuf};

use integration_tests::harness::{
    report::{append_report_best_effort, extract_pm_and_pmt, now_epoch_ms, ReportEntry},
    scenario_api::{
        get_the_grpc_req_for_connector, run_all_connectors_with_options,
        run_all_suites_with_options, run_suite_test_with_options, ExecutionBackend,
        SuiteRunOptions, SuiteRunSummary, DEFAULT_CONNECTOR, DEFAULT_ENDPOINT,
    },
    scenario_loader::load_suite_scenarios,
    sdk_executor::sdk_coverage_report,
};
use serde::Deserialize;
use serde_json::Value;

/// CLI entrypoint for suite-level SDK execution.
fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            std::process::exit(2);
        }
    };

    if args.help {
        print_usage();
        return;
    }

    if args.all_connectors && (args.suite.is_some() || args.all || args.connector.is_some()) {
        eprintln!("cannot combine --all-connectors with --suite, --all, or --connector");
        print_usage();
        std::process::exit(2);
    }

    if args.all && args.suite.is_some() {
        eprintln!("cannot combine --all with --suite or positional suite");
        print_usage();
        std::process::exit(2);
    }

    // Print SDK interface coverage relative to the full proto service suite list.
    print_sdk_interface_coverage();

    let suite = args.suite.as_deref();

    let connector = args
        .connector
        .clone()
        .unwrap_or_else(|| DEFAULT_CONNECTOR.to_string());

    let defaults = load_defaults();

    let endpoint = args
        .endpoint
        .as_deref()
        .map(ToString::to_string)
        .or(defaults.endpoint)
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());

    let creds_file = args
        .creds_file
        .as_deref()
        .map(ToString::to_string)
        .or_else(|| std::env::var("CONNECTOR_AUTH_FILE_PATH").ok())
        .or_else(|| std::env::var("UCS_CREDS_PATH").ok())
        .or(defaults.creds_file);

    if let Some(creds_file) = creds_file.as_deref() {
        std::env::set_var("CONNECTOR_AUTH_FILE_PATH", creds_file);
    }

    let options = SuiteRunOptions {
        endpoint: Some(&endpoint),
        merchant_id: args.merchant_id.as_deref(),
        tenant_id: args.tenant_id.as_deref(),
        plaintext: args.plaintext,
        backend: ExecutionBackend::SdkFfi,
        report: args.report,
    };

    if args.all_connectors {
        let summary = match run_all_connectors_with_options(options) {
            Ok(summary) => summary,
            Err(error) => {
                eprintln!("[sdk_run_test] failed to run all connectors: {error}");
                std::process::exit(1);
            }
        };

        for connector_summary in &summary.connectors {
            println!("\n--- Connector: {} ---", connector_summary.connector);
            for suite_summary in &connector_summary.suites {
                print_suite_results(suite_summary, &endpoint, args.report);
            }
        }

        println!(
            "\n[sdk_run_test] grand total: connectors={} passed={} failed={}",
            summary.connectors.len(),
            summary.passed,
            summary.failed
        );

        if summary.failed > 0 {
            std::process::exit(1);
        }
        return;
    }

    if args.all {
        let summary = match run_all_suites_with_options(Some(&connector), options) {
            Ok(summary) => summary,
            Err(error) => {
                eprintln!("[sdk_run_test] failed to run all suites for '{connector}': {error}");
                std::process::exit(1);
            }
        };

        for suite_summary in &summary.suites {
            print_suite_results(suite_summary, &endpoint, args.report);
        }

        println!(
            "\n[sdk_run_test] summary mode=all connector={} suites={} passed={} failed={}",
            summary.connector,
            summary.suites.len(),
            summary.passed,
            summary.failed
        );

        if summary.failed > 0 {
            std::process::exit(1);
        }
        return;
    }

    let Some(suite) = suite else {
        eprintln!("missing required argument: --suite <suite> (or use --all / --all-connectors)");
        print_usage();
        std::process::exit(2);
    };

    let summary = match run_suite_test_with_options(suite, Some(&connector), options) {
        Ok(summary) => summary,
        Err(error) => {
            eprintln!("[sdk_run_test] failed to run suite '{suite}': {error}");
            std::process::exit(1);
        }
    };

    print_suite_results(&summary, &endpoint, args.report);

    if summary.failed > 0 {
        std::process::exit(1);
    }
}

/// Prints suite results and appends entries to report output.
fn print_suite_results(summary: &SuiteRunSummary, endpoint: &str, report: bool) {
    for result in &summary.results {
        let template_req =
            get_the_grpc_req_for_connector(&result.suite, &result.scenario, &summary.connector)
                .ok();
        let req_for_report = result.req_body.as_ref().or(template_req.as_ref());
        let (pm, pmt) = extract_pm_and_pmt(req_for_report);
        if report {
            let scenario_display_name = load_suite_scenarios(&result.suite)
                .ok()
                .and_then(|scenarios| scenarios.get(&result.scenario).cloned())
                .and_then(|scenario_def| scenario_def.display_name);
            write_report_entry(
                &result.suite,
                &result.scenario,
                scenario_display_name,
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
                "[sdk_run_test] assertion result for '{}': PASS",
                result.scenario
            );
        } else {
            println!(
                "[sdk_run_test] assertion result for '{}': FAIL ({})",
                result.scenario,
                compact_error_for_console(result.error.as_deref())
            );
        }
    }

    println!(
        "\n[sdk_run_test] summary suite={} connector={} passed={} failed={}",
        summary.suite, summary.connector, summary.passed, summary.failed
    );

    let failed_scenarios = summary
        .results
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.scenario.clone())
        .collect::<Vec<_>>();
    if !failed_scenarios.is_empty() {
        println!(
            "[sdk_run_test] failed_scenarios={}",
            failed_scenarios.join(", ")
        );
    }
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

/// Converts scenario result information into a report row.
fn write_report_entry(
    suite: &str,
    scenario: &str,
    scenario_display_name: Option<String>,
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
        scenario_display_name,
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

#[derive(Debug, Default)]
struct CliArgs {
    suite: Option<String>,
    all: bool,
    all_connectors: bool,
    connector: Option<String>,
    endpoint: Option<String>,
    creds_file: Option<String>,
    merchant_id: Option<String>,
    tenant_id: Option<String>,
    report: bool,
    plaintext: bool,
    help: bool,
}

/// Parses CLI arguments for SDK runner modes.
fn parse_args(args: impl Iterator<Item = String>) -> Result<CliArgs, String> {
    let mut cli = CliArgs {
        plaintext: true,
        ..CliArgs::default()
    };
    let mut positionals = Vec::new();
    let mut it = args.peekable();

    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => cli.help = true,
            "--all" => cli.all = true,
            "--all-connectors" => cli.all_connectors = true,
            "--suite" | "-s" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --suite".to_string())?;
                cli.suite = Some(value);
            }
            "--connector" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --connector".to_string())?;
                cli.connector = Some(value);
            }
            "--endpoint" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --endpoint".to_string())?;
                cli.endpoint = Some(value);
            }
            "--creds-file" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --creds-file".to_string())?;
                cli.creds_file = Some(value);
            }
            "--merchant-id" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --merchant-id".to_string())?;
                cli.merchant_id = Some(value);
            }
            "--tenant-id" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --tenant-id".to_string())?;
                cli.tenant_id = Some(value);
            }
            "--report" => cli.report = true,
            "--tls" => cli.plaintext = false,
            _ if arg.starts_with('-') => return Err(format!("unknown argument '{arg}'")),
            _ => positionals.push(arg),
        }
    }

    if !positionals.is_empty() {
        if cli.suite.is_some() {
            return Err("cannot mix positional suite with --suite".to_string());
        }
        if cli.all {
            return Err("cannot use positional suite with --all".to_string());
        }
        if cli.all_connectors {
            return Err("cannot use positional suite with --all-connectors".to_string());
        }
        cli.suite = positionals.first().cloned();
        if positionals.len() > 1 {
            return Err("too many positional arguments; expected: [suite]".to_string());
        }
    }

    Ok(cli)
}

#[derive(Debug, Default, Clone, Deserialize)]
struct StoredDefaults {
    endpoint: Option<String>,
    creds_file: Option<String>,
}

/// Returns persisted defaults path.
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

/// Loads persisted endpoint/credential defaults if available.
fn load_defaults() -> StoredDefaults {
    let path = defaults_path();
    let Ok(content) = fs::read_to_string(path) else {
        return StoredDefaults::default();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

/// Prints a one-time coverage summary showing which proto service suites are
/// supported by the SDK/FFI interface and which are not yet implemented.
fn print_sdk_interface_coverage() {
    let report = sdk_coverage_report();

    eprintln!(
        "[sdk_run_test] interface=SDK/FFI  proto suites={total}  supported={sup}  not yet supported={missing}",
        total = report.supported.len() + report.not_supported.len(),
        sup = report.supported.len(),
        missing = report.not_supported.len(),
    );
    eprintln!(
        "[sdk_run_test]   supported suites    : {}",
        report.supported.join(", ")
    );
    eprintln!(
        "[sdk_run_test]   not yet supported   : {}",
        report.not_supported.join(", ")
    );
}

/// Prints usage/help text for SDK suite runner.
fn print_usage() {
    eprintln!(
        "Usage:\n  cargo run -p integration-tests --bin sdk_run_test -- --suite <suite> [--connector <name>] [options]\n  cargo run -p integration-tests --bin sdk_run_test -- --all [--connector <name>] [options]\n  cargo run -p integration-tests --bin sdk_run_test -- --all-connectors [options]\n  cargo run -p integration-tests --bin sdk_run_test -- <suite>\n\nOptions:\n  --endpoint <host:port>   Reserved for parity with suite_run_test (not used by SDK path)\n  --creds-file <path>      Connector credentials file\n  --merchant-id <id>       Merchant ID\n  --tenant-id <id>         Tenant ID\n  --report                 Generate report.json and test_report/ markdown files\n  --tls                    Reserved for parity with suite_run_test\n\nBehavior:\n  - Uses SDK/FFI path only (no grpc fallback)\n  - Report files are generated only when --report is passed\n  - Fails with exit code 1 if any scenario fails"
    );
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::parse_args;

    #[test]
    fn parses_suite_and_connector() {
        let args = vec!["--suite", "authorize", "--connector", "stripe"]
            .into_iter()
            .map(str::to_string);

        let parsed = parse_args(args).expect("args should parse");
        assert_eq!(parsed.suite.as_deref(), Some("authorize"));
        assert_eq!(parsed.connector.as_deref(), Some("stripe"));
    }

    #[test]
    fn parses_report_flag() {
        let args = vec!["--suite", "authorize", "--report"]
            .into_iter()
            .map(str::to_string);

        let parsed = parse_args(args).expect("args should parse");
        assert!(parsed.report);
    }
}
