#![allow(clippy::print_stderr, clippy::print_stdout)]

//! Generates style-A `display_name` values in suite `scenario.json` files.
//!
//! Usage:
//!   cargo run -p integration-tests --bin generate_scenario_display_names
//!   cargo run -p integration-tests --bin generate_scenario_display_names -- --suite authorize
//!   cargo run -p integration-tests --bin generate_scenario_display_names -- --check
//!   cargo run -p integration-tests --bin generate_scenario_display_names -- --render-markdown

use std::{collections::BTreeSet, fs, path::PathBuf};

use integration_tests::harness::report::{
    regenerate_markdown_from_disk, regenerate_markdown_from_path,
};
use integration_tests::harness::scenario_display_name::generate_style_a_display_name;
use integration_tests::harness::scenario_loader::{
    scenario_file_path, scenario_root, suite_dir_name_to_suite_name,
};
use serde_json::Value;

#[derive(Debug, Default)]
struct CliArgs {
    suite: Option<String>,
    check: bool,
    render_markdown: bool,
    report_path: Option<PathBuf>,
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {err}");
            print_usage();
            std::process::exit(2);
        }
    };

    if let Err(err) = run(args) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(args: CliArgs) -> Result<(), String> {
    if args.check && args.render_markdown {
        return Err("--check cannot be combined with --render-markdown".to_string());
    }

    let suites = discover_suites(args.suite.as_deref())?;
    if suites.is_empty() {
        return Err("no suites found to process".to_string());
    }

    let mut touched_files = 0usize;
    let mut updated_display_names = 0usize;

    for suite in suites {
        let path = scenario_file_path(&suite);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
        let mut parsed: Value = serde_json::from_str(&content)
            .map_err(|e| format!("failed to parse '{}': {e}", path.display()))?;

        let scenarios = parsed
            .as_object_mut()
            .ok_or_else(|| format!("expected top-level object in '{}'", path.display()))?;

        let mut file_updates = 0usize;
        for (scenario_name, scenario_def) in scenarios.iter_mut() {
            let scenario_obj = scenario_def.as_object_mut().ok_or_else(|| {
                format!(
                    "expected scenario '{}' in '{}' to be an object",
                    scenario_name,
                    path.display()
                )
            })?;

            let generated_display_name = generate_style_a_display_name(&suite, scenario_name);
            let current_display_name = scenario_obj.get("display_name").and_then(Value::as_str);

            if current_display_name != Some(generated_display_name.as_str()) {
                scenario_obj.insert(
                    "display_name".to_string(),
                    Value::String(generated_display_name),
                );
                file_updates += 1;
            }
        }

        if file_updates > 0 {
            touched_files += 1;
            updated_display_names += file_updates;

            if args.check {
                println!(
                    "[check] {} -> {} scenario display name(s) would be updated",
                    path.display(),
                    file_updates
                );
            } else {
                let serialized = serde_json::to_string_pretty(&parsed)
                    .map_err(|e| format!("failed to serialize '{}': {e}", path.display()))?;
                fs::write(&path, format!("{serialized}\n"))
                    .map_err(|e| format!("failed to write '{}': {e}", path.display()))?;
                println!(
                    "[write] {} -> updated {} scenario display name(s)",
                    path.display(),
                    file_updates
                );
            }
        }
    }

    if touched_files == 0 {
        println!("No changes needed. All scenario display names are up-to-date.");
    } else if args.check {
        println!(
            "Check complete: {} file(s) need updates ({} scenario display name(s)).",
            touched_files, updated_display_names
        );
    } else {
        println!(
            "Done: updated {} file(s) with {} scenario display name(s).",
            touched_files, updated_display_names
        );
    }

    if args.render_markdown {
        let overview = if let Some(path) = args.report_path.as_deref() {
            regenerate_markdown_from_path(path)
        } else {
            regenerate_markdown_from_disk()
        }
        .map_err(|e| format!("failed to regenerate markdown report: {e}"))?;
        println!("Markdown report regenerated: {}", overview.display());
    }

    Ok(())
}

fn discover_suites(single_suite: Option<&str>) -> Result<Vec<String>, String> {
    if let Some(suite) = single_suite {
        let scenario_path = scenario_file_path(suite);
        if !scenario_path.is_file() {
            return Err(format!(
                "suite '{}' does not exist or is missing scenario.json ({})",
                suite,
                scenario_path.display()
            ));
        }
        return Ok(vec![suite.to_string()]);
    }

    let root = scenario_root();
    let entries = fs::read_dir(&root)
        .map_err(|e| format!("failed to read scenario root '{}': {e}", root.display()))?;

    let mut suites = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read scenario root entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(dir_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if path.join("scenario.json").is_file() {
            if let Some(suite_name) = suite_dir_name_to_suite_name(dir_name) {
                suites.insert(suite_name);
            }
        }
    }

    Ok(suites.into_iter().collect())
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
            "--suite" | "-s" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--suite requires a value".to_string())?;
                cli.suite = Some(value);
            }
            "--check" => {
                cli.check = true;
            }
            "--render-markdown" | "--regen-md" => {
                cli.render_markdown = true;
            }
            "--report-path" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--report-path requires a value".to_string())?;
                cli.report_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            unknown => {
                return Err(format!("unknown argument '{unknown}'"));
            }
        }
    }

    Ok(cli)
}

fn print_usage() {
    let root: PathBuf = scenario_root();
    println!(
        "Usage:\n  cargo run -p integration-tests --bin generate_scenario_display_names -- [--suite <suite>] [--check] [--render-markdown] [--report-path <report.json>]\n\nOptions:\n  --suite, -s <suite>      Update one suite only\n  --check                  Preview files that would change without writing\n  --render-markdown        Regenerate test_report/ markdown from report.json after updates\n  --regen-md               Alias for --render-markdown\n  --report-path <path>     Custom report.json path used with --render-markdown\n\nScenario root:\n  {}",
        root.display()
    );
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::Path;

    #[test]
    fn parses_render_markdown_flags() {
        assert!(matches!(
            parse_args(vec!["--render-markdown".to_string()]),
            Ok(super::CliArgs {
                render_markdown: true,
                ..
            })
        ));
    }

    #[test]
    fn parses_regen_md_alias_and_report_path() {
        assert!(matches!(
            parse_args(vec![
                "--regen-md".to_string(),
                "--report-path".to_string(),
                "./my-report.json".to_string(),
            ]),
            Ok(super::CliArgs {
                render_markdown: true,
                report_path: Some(path),
                ..
            }) if path == Path::new("./my-report.json")
        ));
    }
}
