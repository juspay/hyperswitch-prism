#![allow(clippy::print_stderr, clippy::print_stdout, clippy::too_many_arguments)]

//! Single-scenario runner.
//!
//! This binary executes exactly one `(suite, scenario, connector)` path and
//! optionally appends a structured report entry.

use std::{fs, path::PathBuf};

use integration_tests::harness::{
    auto_gen::resolve_auto_generate,
    report::{append_report_best_effort, extract_pm_and_pmt, now_epoch_ms, ReportEntry},
    scenario_api::{
        build_grpcurl_request_from_payload, do_assertion,
        execute_grpcurl_request_from_payload_with_trace, get_the_assertion_for_connector,
        get_the_grpc_req_for_connector, run_test, DEFAULT_CONNECTOR, DEFAULT_ENDPOINT,
        DEFAULT_SCENARIO, DEFAULT_SUITE,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// CLI entrypoint for one-scenario execution.
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

    let mut defaults = load_defaults();

    if args.set_defaults {
        if args.endpoint.is_none() && args.creds_file.is_none() {
            eprintln!("set-defaults failed: provide at least one of --endpoint or --creds-file");
            std::process::exit(2);
        }

        if let Some(endpoint) = args.endpoint.clone() {
            defaults.endpoint = Some(endpoint);
        }
        if let Some(creds_file) = args.creds_file.clone() {
            defaults.creds_file = Some(creds_file);
        }

        if let Err(error) = save_defaults(&defaults) {
            eprintln!("set-defaults failed: {error}");
            std::process::exit(1);
        }

        println!("saved defaults in {}", defaults_path().display());
        println!(
            "current defaults: endpoint={} creds_file={}",
            defaults.endpoint.as_deref().unwrap_or("<none>"),
            defaults.creds_file.as_deref().unwrap_or("<none>")
        );
        return;
    }

    if args.show_defaults {
        println!("defaults file: {}", defaults_path().display());
        println!(
            "endpoint={} creds_file={}",
            defaults.endpoint.as_deref().unwrap_or("<none>"),
            defaults.creds_file.as_deref().unwrap_or("<none>")
        );
        return;
    }

    let endpoint = args.endpoint.clone().or(defaults.endpoint.clone());
    let endpoint_value = endpoint
        .clone()
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
    let creds_file = args
        .creds_file
        .clone()
        .or_else(|| std::env::var("CONNECTOR_AUTH_FILE_PATH").ok())
        .or_else(|| std::env::var("UCS_CREDS_PATH").ok())
        .or(defaults.creds_file.clone());

    if let Some(creds_file) = creds_file.as_deref() {
        std::env::set_var("CONNECTOR_AUTH_FILE_PATH", creds_file);
    }

    let suite = args.suite.as_deref().unwrap_or(DEFAULT_SUITE);
    let scenario = args.scenario.as_deref().unwrap_or(DEFAULT_SCENARIO);
    let connector = args.connector.as_deref().unwrap_or(DEFAULT_CONNECTOR);
    let mut grpc_req = match get_the_grpc_req_for_connector(suite, scenario, connector) {
        Ok(req) => req,
        Err(error) => {
            write_report_entry(
                args.report,
                suite,
                scenario,
                connector,
                &endpoint_value,
                None,
                None,
                "FAIL",
                None,
                Some(format!("failed to load grpc request: {error}")),
                vec![],
                None,
                None,
                None,
                None,
            );
            eprintln!("run_test failed: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = resolve_auto_generate(&mut grpc_req) {
        write_report_entry(
            args.report,
            suite,
            scenario,
            connector,
            &endpoint_value,
            None,
            None,
            "FAIL",
            None,
            Some(format!("failed to resolve auto-generated fields: {error}")),
            vec![],
            Some(grpc_req.clone()),
            None,
            None,
            None,
        );
        eprintln!("run_test failed: {error}");
        std::process::exit(1);
    }

    let (pm, pmt) = extract_pm_and_pmt(Some(&grpc_req));

    if let Err(error) = run_test(Some(suite), Some(scenario), Some(connector)) {
        write_report_entry(
            args.report,
            suite,
            scenario,
            connector,
            &endpoint_value,
            pm.as_deref(),
            pmt.as_deref(),
            "FAIL",
            None,
            Some(error.to_string()),
            vec![],
            Some(grpc_req.clone()),
            None,
            None,
            None,
        );
        eprintln!("run_test failed: {error}");
        std::process::exit(1);
    }

    let prebuilt_grpc_request = match build_grpcurl_request_from_payload(
        suite,
        scenario,
        &grpc_req,
        endpoint.as_deref(),
        Some(connector),
        args.merchant_id.as_deref(),
        args.tenant_id.as_deref(),
        args.plaintext,
        false,
    ) {
        Ok(request) => Some(request.to_command_string()),
        Err(error) => {
            write_report_entry(
                args.report,
                suite,
                scenario,
                connector,
                &endpoint_value,
                pm.as_deref(),
                pmt.as_deref(),
                "FAIL",
                None,
                Some(error.to_string()),
                vec![],
                Some(grpc_req.clone()),
                None,
                None,
                None,
            );
            eprintln!("grpcurl generation failed: {error}");
            std::process::exit(1);
        }
    };

    match execute_grpcurl_request_from_payload_with_trace(
        suite,
        scenario,
        &grpc_req,
        endpoint.as_deref(),
        Some(connector),
        args.merchant_id.as_deref(),
        args.tenant_id.as_deref(),
        args.plaintext,
    ) {
        Ok(trace) => {
            let response = trace.response_body;
            let grpc_request = Some(trace.request_command);
            let grpc_response = Some(trace.response_output);
            let response_json: Value = match serde_json::from_str(&response) {
                Ok(parsed) => parsed,
                Err(error) => {
                    write_report_entry(
                        args.report,
                        suite,
                        scenario,
                        connector,
                        &endpoint_value,
                        pm.as_deref(),
                        pmt.as_deref(),
                        "FAIL",
                        None,
                        Some(format!(
                            "failed to parse grpc response JSON before assertions: {error}"
                        )),
                        vec![],
                        Some(grpc_req.clone()),
                        None,
                        grpc_request.clone(),
                        grpc_response.clone(),
                    );
                    eprintln!("[run_test] assertion result: FAIL");
                    eprintln!(
                        "[run_test] failed to parse grpc response JSON before assertions: {error}"
                    );
                    std::process::exit(1);
                }
            };

            let assertions = match get_the_assertion_for_connector(suite, scenario, connector) {
                Ok(assertions) => assertions,
                Err(error) => {
                    write_report_entry(
                        args.report,
                        suite,
                        scenario,
                        connector,
                        &endpoint_value,
                        pm.as_deref(),
                        pmt.as_deref(),
                        "FAIL",
                        extract_response_status(&response_json),
                        Some(format!("failed to load assertion rules: {error}")),
                        vec![],
                        Some(grpc_req.clone()),
                        Some(response_json.clone()),
                        grpc_request.clone(),
                        grpc_response.clone(),
                    );
                    eprintln!("[run_test] assertion result: FAIL");
                    eprintln!("[run_test] failed to load assertion rules: {error}");
                    std::process::exit(1);
                }
            };

            match do_assertion(&assertions, &response_json, &grpc_req) {
                Ok(()) => {
                    println!("[run_test] assertion result: PASS");
                    write_report_entry(
                        args.report,
                        suite,
                        scenario,
                        connector,
                        &endpoint_value,
                        pm.as_deref(),
                        pmt.as_deref(),
                        "PASS",
                        extract_response_status(&response_json),
                        None,
                        vec![],
                        Some(grpc_req.clone()),
                        Some(response_json.clone()),
                        grpc_request.clone(),
                        grpc_response.clone(),
                    );
                }
                Err(error) => {
                    write_report_entry(
                        args.report,
                        suite,
                        scenario,
                        connector,
                        &endpoint_value,
                        pm.as_deref(),
                        pmt.as_deref(),
                        "FAIL",
                        extract_response_status(&response_json),
                        Some(error.to_string()),
                        vec![],
                        Some(grpc_req.clone()),
                        Some(response_json.clone()),
                        grpc_request.clone(),
                        grpc_response.clone(),
                    );
                    eprintln!("[run_test] assertion result: FAIL");
                    eprintln!("[run_test] assertion failure: {error}");
                    std::process::exit(1);
                }
            }
        }
        Err(error) => {
            write_report_entry(
                args.report,
                suite,
                scenario,
                connector,
                &endpoint_value,
                pm.as_deref(),
                pmt.as_deref(),
                "FAIL",
                None,
                Some(error.to_string()),
                vec![],
                Some(grpc_req.clone()),
                None,
                prebuilt_grpc_request,
                None,
            );
            eprintln!("grpc execution failed: {error}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Default)]
struct CliArgs {
    suite: Option<String>,
    scenario: Option<String>,
    connector: Option<String>,
    endpoint: Option<String>,
    creds_file: Option<String>,
    merchant_id: Option<String>,
    tenant_id: Option<String>,
    set_defaults: bool,
    show_defaults: bool,
    report: bool,
    plaintext: bool,
    help: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct StoredDefaults {
    endpoint: Option<String>,
    creds_file: Option<String>,
}

/// Returns location of persisted CLI defaults for endpoint/credentials.
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

/// Loads saved defaults; returns empty defaults when file is absent/invalid.
fn load_defaults() -> StoredDefaults {
    let path = defaults_path();
    let Ok(content) = fs::read_to_string(path) else {
        return StoredDefaults::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Persists CLI defaults to disk in pretty JSON form.
fn save_defaults(defaults: &StoredDefaults) -> Result<(), String> {
    let path = defaults_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create defaults directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    let serialized = serde_json::to_string_pretty(defaults)
        .map_err(|error| format!("failed to serialize defaults: {error}"))?;
    fs::write(&path, serialized).map_err(|error| {
        format!(
            "failed to write defaults file '{}': {error}",
            path.display()
        )
    })
}

/// Appends one run result into `report.json` / `test_report/` markdown outputs.
fn write_report_entry(
    report: bool,
    suite: &str,
    scenario: &str,
    connector: &str,
    endpoint: &str,
    pm: Option<&str>,
    pmt: Option<&str>,
    assertion_result: &str,
    response_status: Option<String>,
    error: Option<String>,
    dependency: Vec<String>,
    req_body: Option<Value>,
    res_body: Option<Value>,
    grpc_request: Option<String>,
    grpc_response: Option<String>,
) {
    if !report {
        return;
    }

    append_report_best_effort(ReportEntry {
        run_at_epoch_ms: now_epoch_ms(),
        suite: suite.to_string(),
        scenario: scenario.to_string(),
        connector: connector.to_string(),
        pm: pm.map(ToString::to_string),
        pmt: pmt.map(ToString::to_string),
        endpoint: endpoint.to_string(),
        is_dependency: false,
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

/// Extracts normalized status text from response payload for reporting.
fn extract_response_status(response_json: &Value) -> Option<String> {
    response_json
        .get("status")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

/// Parses CLI flags/positionals with backward-compatible positional support.
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
            "-s" | "--suite" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --suite".to_string())?;
                cli.suite = Some(value);
            }
            "-c" | "--scenario" => {
                let value = it
                    .next()
                    .ok_or_else(|| "missing value for --scenario".to_string())?;
                cli.scenario = Some(value);
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
            "--set-defaults" => cli.set_defaults = true,
            "--show-defaults" => cli.show_defaults = true,
            "--report" => cli.report = true,
            "--tls" => cli.plaintext = false,
            _ if arg.starts_with('-') => {
                return Err(format!("unknown argument '{arg}'"));
            }
            _ => positionals.push(arg),
        }
    }

    if !positionals.is_empty() {
        if cli.suite.is_some() || cli.scenario.is_some() || cli.connector.is_some() {
            return Err(
                "cannot mix positional args with --suite/--scenario/--connector flags".to_string(),
            );
        }

        cli.suite = positionals.first().cloned();
        cli.scenario = positionals.get(1).cloned();
        cli.connector = positionals.get(2).cloned();
        if positionals.len() > 3 {
            return Err(
                "too many positional arguments; expected: [suite] [scenario] [connector]"
                    .to_string(),
            );
        }
    }

    Ok(cli)
}

/// Prints CLI usage/help text.
fn print_usage() {
    eprintln!(
        "Usage:\n  cargo run -p integration-tests --bin run_test -- [--suite <suite>] [--scenario <scenario>] [--connector <name>] [--endpoint <host:port>] [--creds-file <path>] [--merchant-id <id>] [--tenant-id <id>] [--report] [--tls]\n  cargo run -p integration-tests --bin run_test -- [suite] [scenario] [connector]\n\nDefault mode behavior:\n  - Loads scenario request JSON\n  - Executes grpcurl\n  - Runs assertions and prints PASS/FAIL\n\nOptional report output:\n  - Pass --report to clear previous report.json at start\n  - Pass --report to write run details into report.json and auto-generate test_report/ markdown files\n\nSave once and reuse auth/endpoint:\n  cargo run -p integration-tests --bin run_test -- --set-defaults --endpoint <host:port> --creds-file <path>\n  cargo run -p integration-tests --bin run_test -- --show-defaults\n\nDefaults:\n  suite: {DEFAULT_SUITE}\n  scenario: {DEFAULT_SCENARIO}\n  connector: {DEFAULT_CONNECTOR}\n  endpoint: {DEFAULT_ENDPOINT}\n  merchant-id: test_merchant\n  tenant-id: default\n  transport: plaintext\n\nConfig path:\n  $UCS_RUN_TEST_DEFAULTS_PATH or ~/.config/integration-tests/run_test_defaults.json\nReport path:\n  $UCS_RUN_TEST_REPORT_PATH or crates/internal/integration-tests/report.json"
    );
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::parse_args;

    #[test]
    fn parses_named_flags() {
        let args = vec![
            "--suite",
            "authorize",
            "--scenario",
            "no3ds_auto_capture_credit_card",
            "--connector",
            "stripe",
            "--creds-file",
            "/tmp/creds.json",
        ]
        .into_iter()
        .map(str::to_string);

        let parsed = parse_args(args).expect("args should parse");
        assert_eq!(parsed.suite.as_deref(), Some("authorize"));
        assert_eq!(
            parsed.scenario.as_deref(),
            Some("no3ds_auto_capture_credit_card")
        );
        assert_eq!(parsed.connector.as_deref(), Some("stripe"));
        assert_eq!(parsed.creds_file.as_deref(), Some("/tmp/creds.json"));
    }

    #[test]
    fn parses_positionals() {
        let args = vec!["authorize", "no3ds_manual_capture_credit_card", "adyen"]
            .into_iter()
            .map(str::to_string);

        let parsed = parse_args(args).expect("args should parse");
        assert_eq!(parsed.suite.as_deref(), Some("authorize"));
        assert_eq!(
            parsed.scenario.as_deref(),
            Some("no3ds_manual_capture_credit_card")
        );
        assert_eq!(parsed.connector.as_deref(), Some("adyen"));
    }

    #[test]
    fn parses_tls_and_endpoint_flags() {
        let args = vec![
            "--suite",
            "authorize",
            "--scenario",
            "no3ds_auto_capture_credit_card",
            "--connector",
            "stripe",
            "--endpoint",
            "localhost:50051",
            "--merchant-id",
            "test_merchant",
            "--tenant-id",
            "default",
            "--tls",
        ]
        .into_iter()
        .map(str::to_string);

        let parsed = parse_args(args).expect("args should parse");
        assert!(!parsed.plaintext);
        assert_eq!(parsed.connector.as_deref(), Some("stripe"));
        assert_eq!(parsed.endpoint.as_deref(), Some("localhost:50051"));
    }

    #[test]
    fn parses_defaults_flags() {
        let args = vec![
            "--set-defaults",
            "--endpoint",
            "localhost:8000",
            "--creds-file",
            "/tmp/connector_creds.json",
        ]
        .into_iter()
        .map(str::to_string);

        let parsed = parse_args(args).expect("defaults args should parse");
        assert!(parsed.set_defaults);
        assert_eq!(parsed.endpoint.as_deref(), Some("localhost:8000"));
        assert_eq!(
            parsed.creds_file.as_deref(),
            Some("/tmp/connector_creds.json")
        );
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
