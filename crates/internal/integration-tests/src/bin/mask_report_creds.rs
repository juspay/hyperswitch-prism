#![allow(clippy::print_stderr, clippy::print_stdout, clippy::expect_used)]

//! Retroactively masks credentials in existing test report markdown files and
//! the report.json file.
//!
//! Usage:
//!   cargo run -p integration-tests --bin mask_report_creds
//!   cargo run -p integration-tests --bin mask_report_creds -- --path <report_dir>
//!   cargo run -p integration-tests --bin mask_report_creds -- --dry-run
//!
//! This walks all `.md` files under `test_report/` and applies
//! `mask_sensitive_text()` line-by-line.  It also processes `report.json`
//! entries.  Files that are already fully masked are left untouched.

use std::fs;
use std::path::{Path, PathBuf};

use integration_tests::harness::cred_masking::{mask_json_value, mask_sensitive_text};
use integration_tests::harness::report::{report_path, ScenarioRunReport};

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            print_usage();
            std::process::exit(2);
        }
    };

    if args.help {
        print_usage();
        return;
    }

    let json_path = args.path.clone().unwrap_or_else(report_path);
    let report_dir = json_path
        .parent()
        .expect("report.json should have a parent directory")
        .join("test_report");

    let dry_run = args.dry_run;

    // ---- Phase 1: Mask markdown files ----
    let mut md_files_changed = 0u64;
    let mut md_files_checked = 0u64;

    if report_dir.is_dir() {
        walk_md_files(&report_dir, &mut |path| {
            md_files_checked += 1;
            if mask_md_file(path, dry_run) {
                md_files_changed += 1;
            }
        });
    } else {
        eprintln!(
            "warning: report directory does not exist: {}",
            report_dir.display()
        );
    }

    // ---- Phase 2: Mask report.json ----
    let json_changed = if json_path.is_file() {
        mask_report_json(&json_path, dry_run)
    } else {
        eprintln!("warning: report.json not found: {}", json_path.display());
        false
    };

    // ---- Summary ----
    let prefix = if dry_run { "[dry-run] " } else { "" };
    println!(
        "{prefix}mask_report_creds: checked {md_files_checked} markdown files, \
         {md_files_changed} needed masking."
    );
    if json_changed {
        println!("{prefix}report.json was updated.");
    } else {
        println!("{prefix}report.json: no changes needed.");
    }
}

/// Applies `mask_sensitive_text()` to every line in an `.md` file.
/// Returns `true` if the file was changed.
fn mask_md_file(path: &Path, dry_run: bool) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: could not read {}: {e}", path.display());
            return false;
        }
    };

    let masked = mask_sensitive_text(&content);

    if masked == content {
        return false;
    }

    if dry_run {
        println!("  would mask: {}", path.display());
    } else {
        if let Err(e) = fs::write(path, &masked) {
            eprintln!("error: could not write {}: {e}", path.display());
            return false;
        }
        println!("  masked: {}", path.display());
    }

    true
}

/// Masks sensitive fields inside `report.json` entries.
/// Returns `true` if the file was changed.
fn mask_report_json(json_path: &Path, dry_run: bool) -> bool {
    let content = match fs::read_to_string(json_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: could not read {}: {e}", json_path.display());
            return false;
        }
    };

    let mut report: ScenarioRunReport = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: could not parse {}: {e}", json_path.display());
            return false;
        }
    };

    let mut changed = false;
    for entry in &mut report.runs {
        let snapshot_error = entry.error.clone();
        let snapshot_grpc_req = entry.grpc_request.clone();
        let snapshot_grpc_res = entry.grpc_response.clone();

        if let Some(error) = entry.error.as_mut() {
            *error = mask_sensitive_text(error);
        }
        if let Some(grpc_request) = entry.grpc_request.as_mut() {
            *grpc_request = mask_sensitive_text(grpc_request);
        }
        if let Some(grpc_response) = entry.grpc_response.as_mut() {
            *grpc_response = mask_sensitive_text(grpc_response);
        }
        if let Some(req_body) = entry.req_body.as_mut() {
            mask_json_value(req_body);
        }
        if let Some(res_body) = entry.res_body.as_mut() {
            mask_json_value(res_body);
        }

        if entry.error != snapshot_error
            || entry.grpc_request != snapshot_grpc_req
            || entry.grpc_response != snapshot_grpc_res
        {
            changed = true;
        }
    }

    // Also check req_body/res_body changes by re-serialising and comparing
    if !changed {
        if let Ok(re_serialized) = serde_json::to_string_pretty(&report) {
            if re_serialized != content {
                changed = true;
            }
        }
    }

    if !changed {
        return false;
    }

    if dry_run {
        println!("  would mask: {}", json_path.display());
    } else {
        match serde_json::to_string_pretty(&report) {
            Ok(serialized) => {
                if let Err(e) = fs::write(json_path, &serialized) {
                    eprintln!("error: could not write {}: {e}", json_path.display());
                    return false;
                }
                println!("  masked: {}", json_path.display());
            }
            Err(e) => {
                eprintln!("error: could not serialize report: {e}");
                return false;
            }
        }
    }

    true
}

fn walk_md_files(dir: &Path, visitor: &mut dyn FnMut(&Path)) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_md_files(&path, visitor);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            visitor(&path);
        }
    }
}

// ---- CLI parsing ----

#[derive(Debug, Default)]
struct CliArgs {
    help: bool,
    dry_run: bool,
    path: Option<PathBuf>,
}

fn parse_args<I>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut cli = CliArgs::default();
    let mut iter = args.into_iter().map(Into::into).peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => cli.help = true,
            "--dry-run" | "-n" => cli.dry_run = true,
            "--path" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "missing value for --path".to_string())?;
                cli.path = Some(PathBuf::from(value));
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    Ok(cli)
}

fn print_usage() {
    let default_path = report_path();
    eprintln!(
        "\
Usage:
  cargo run -p integration-tests --bin mask_report_creds
  cargo run -p integration-tests --bin mask_report_creds -- --path <report.json>
  cargo run -p integration-tests --bin mask_report_creds -- --dry-run

Retroactively masks credentials in existing test report markdown files
and in report.json entries.

Options:
  --path <report.json>  Path to report.json (default: {})
  --dry-run, -n         Show what would be changed without writing

Files that are already fully masked are left untouched.",
        default_path.display()
    );
}
