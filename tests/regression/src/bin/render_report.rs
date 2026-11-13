#![allow(clippy::print_stderr, clippy::print_stdout)]

//! Regenerates markdown report artifacts from an existing `report.json`.

use std::path::PathBuf;

use integration_tests::harness::report::{
    regenerate_markdown_from_disk, regenerate_markdown_from_path, report_path,
};

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

    let result = if let Some(path) = args.path.as_deref() {
        regenerate_markdown_from_path(path)
    } else {
        regenerate_markdown_from_disk()
    };

    match result {
        Ok(overview_path) => {
            println!("report markdown generated: {}", overview_path.display());
        }
        Err(error) => {
            eprintln!("render_report failed: {error}");
            std::process::exit(1);
        }
    }
}

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
    let default_path = report_path();
    eprintln!(
        "Usage:\n  cargo run -p integration-tests --bin render_report\n  cargo run -p integration-tests --bin render_report -- --path <report.json>\n\nBehavior:\n  - Reads an existing report.json\n  - Regenerates test_report/ markdown files only\n  - Does not execute tests\n\nDefault report path:\n  {}\n  ($UCS_RUN_TEST_REPORT_PATH can override this path)",
        default_path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::Path;

    #[test]
    fn parses_help_flag() {
        assert!(matches!(
            parse_args(["--help"]),
            Ok(super::CliArgs {
                help: true,
                path: None
            })
        ));
    }

    #[test]
    fn parses_path_flag() {
        assert!(matches!(
            parse_args(["--path", "./custom-report.json"]),
            Ok(super::CliArgs {
                help: false,
                path: Some(path)
            }) if path == Path::new("./custom-report.json")
        ));
    }

    #[test]
    fn errors_on_unknown_flag() {
        assert!(matches!(
            parse_args(["--nope"]),
            Err(err) if err.contains("unknown argument")
        ));
    }
}
