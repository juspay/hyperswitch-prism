#![allow(clippy::print_stderr, clippy::print_stdout, clippy::expect_used)]

//! Pre-push safety check: scans all test report markdown files for unmasked
//! credentials and exits with a non-zero status if any are found.
//!
//! Usage:
//!   cargo run -p integration-tests --bin check_report_creds
//!   cargo run -p integration-tests --bin check_report_creds -- --path <report_dir>
//!
//! If unmasked credentials are detected the binary prints a summary and exits
//! with status 1, instructing the user to run `mask_report_creds` first.

use std::fs;
use std::path::{Path, PathBuf};

use integration_tests::harness::cred_masking::detect_unmasked_cred;
use integration_tests::harness::report::report_path;

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

    let report_dir = args.path.unwrap_or_else(|| {
        report_path()
            .parent()
            .expect("report.json should have a parent directory")
            .join("test_report")
    });

    if !report_dir.is_dir() {
        eprintln!("report directory does not exist: {}", report_dir.display());
        std::process::exit(2);
    }

    let mut violations: Vec<(PathBuf, usize, String)> = Vec::new();
    let mut files_checked = 0u64;

    walk_md_files(&report_dir, &mut |path| {
        files_checked += 1;
        check_file(path, &mut violations);
    });

    if violations.is_empty() {
        println!(
            "check_report_creds: OK — {files_checked} files checked, no unmasked credentials found."
        );
    } else {
        eprintln!(
            "check_report_creds: FAILED — found {} unmasked credential(s) in {} file(s):",
            violations.len(),
            {
                let mut files: Vec<_> = violations.iter().map(|(p, _, _)| p.clone()).collect();
                files.sort();
                files.dedup();
                files.len()
            }
        );
        for (path, line_no, reason) in &violations {
            eprintln!("  {}:{}: {}", path.display(), line_no, reason);
        }
        eprintln!();
        eprintln!("Run the following command to fix:");
        eprintln!("  cargo run -p integration-tests --bin mask_report_creds");
        std::process::exit(1);
    }
}

fn check_file(path: &Path, violations: &mut Vec<(PathBuf, usize, String)>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: could not read {}: {e}", path.display());
            return;
        }
    };

    for (line_no, line) in content.lines().enumerate() {
        if let Some(reason) = detect_unmasked_cred(line) {
            violations.push((path.to_path_buf(), line_no + 1, reason));
        }
    }
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
    let default_dir = report_path()
        .parent()
        .map(|p| p.join("test_report"))
        .unwrap_or_else(|| PathBuf::from("test_report"));
    eprintln!(
        "\
Usage:
  cargo run -p integration-tests --bin check_report_creds
  cargo run -p integration-tests --bin check_report_creds -- --path <report_dir>

Scans all .md files in the test report directory for unmasked credentials.
Exits with status 1 if any are found.

Default report directory:
  {}",
        default_dir.display()
    );
}
