use std::fs;
use std::path::{Path, PathBuf};

fn root_connector_files() -> Vec<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let connectors_dir = manifest_dir.join("src/connectors");

    let mut files: Vec<PathBuf> = fs::read_dir(&connectors_dir)
        .expect("connector directory should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();

    files.push(manifest_dir.join("src/default_implementations.rs"));
    files.sort();
    files
}

fn has_empty_connector_integration_impl(text: &str) -> bool {
    let mut cursor = 0;

    while let Some(relative_idx) = text[cursor..].find("ConnectorIntegrationV2<") {
        let idx = cursor + relative_idx;
        let Some(open_brace_idx) = text[idx..].find('{').map(|offset| idx + offset) else {
            break;
        };

        if text[..idx].rfind("impl").is_none() {
            cursor = idx + "ConnectorIntegrationV2<".len();
            continue;
        }

        if text[open_brace_idx + 1..].trim_start().starts_with('}') {
            return true;
        }

        cursor = open_brace_idx + 1;
    }

    false
}

#[test]
fn connector_integration_v2_impls_must_not_be_empty() {
    let offenders: Vec<String> = root_connector_files()
        .into_iter()
        .filter_map(|path| {
            let text = fs::read_to_string(&path).expect("source file should be readable");
            has_empty_connector_integration_impl(&text).then(|| path.display().to_string())
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "empty ConnectorIntegrationV2 impls are not allowed: {offenders:?}"
    );
}

#[test]
fn root_connectors_must_use_shared_flow_status_helpers() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let connectors_dir = manifest_dir.join("src/connectors");

    let offenders: Vec<String> = fs::read_dir(&connectors_dir)
        .expect("connector directory should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .filter_map(|path| {
            let text = fs::read_to_string(&path).expect("source file should be readable");
            text.lines()
                .map(str::trim_start)
                .any(|line| {
                    line.starts_with("fn ")
                        && (line.contains("_not_implemented(")
                            || line.contains("_flow_not_supported("))
                })
                .then(|| path.display().to_string())
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "connector-local flow status helpers should be removed: {offenders:?}"
    );
}
