//! Boot contract for the `[superposition]` table: **Superposition is never a reason to
//! refuse boot.** A remote source whose settings cannot describe a workspace is
//! reported and set back to the file — the process serves policy from the baked file — the
//! same fail-open posture as a déjà record misconfiguration. A complete block loads
//! as written. Both go through the production loader, not a hand-built struct.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use common_utils::superposition_config::SuperpositionSource;
use hyperswitch_masking::PeekInterface;

fn dev_config_toml() -> String {
    let path = ucs_env::configs::workspace_path()
        .join("config")
        .join("development.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Replace the `[superposition]` table with `block` (which may declare its own table).
fn with_superposition_table(toml: &str, block: &str) -> String {
    let mut out = String::new();
    let mut in_table = false;
    for line in toml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            let name = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            in_table = name == "superposition";
        }
        if !in_table {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(block);
    out
}

fn load(tag: &str, toml: &str) -> ucs_env::configs::Config {
    let path = std::env::temp_dir().join(format!(
        "ucs_superposition_load_{}_{}.toml",
        std::process::id(),
        tag
    ));
    std::fs::write(&path, toml).expect("write temp config");
    let cfg = ucs_env::configs::Config::new_with_config_path(Some(path.clone()))
        .unwrap_or_else(|e| panic!("load {tag} config: {e}"));
    let _ = std::fs::remove_file(&path);
    cfg
}

/// `source = "remote"` with nothing else set: the loader must NOT fail. The source is
/// set back to the file and the baked file serves policy.
#[test]
fn undescribed_remote_source_loads_and_falls_back_to_the_file() {
    let toml = with_superposition_table(
        &dev_config_toml(),
        "\n[superposition]\nsource = \"remote\"\n",
    );
    let cfg = load("undescribed", &toml);
    assert_eq!(
        cfg.superposition.source,
        SuperpositionSource::File,
        "a remote source with no endpoint/token/org/workspace must fall back to the file, not abort boot"
    );
}

/// A complete block loads as written, secret included.
#[test]
fn complete_remote_block_loads_enabled() {
    let block = r#"
[superposition]
source = "remote"
endpoint = "http://superposition:8080"
token = "sp_test_token"
org_id = "hyperswitch"
workspace_id = "prism"
polling_interval = 7
"#;
    let cfg = load(
        "complete",
        &with_superposition_table(&dev_config_toml(), block),
    );
    assert_eq!(cfg.superposition.source, SuperpositionSource::Remote);
    assert_eq!(cfg.superposition.endpoint, "http://superposition:8080");
    assert_eq!(cfg.superposition.token.peek(), "sp_test_token");
    assert_eq!(cfg.superposition.workspace_id, "prism");
    assert_eq!(cfg.superposition.polling_interval, 7);
}

/// The shipped development config is file-first.
#[test]
fn shipped_development_config_is_file_first() {
    let cfg = load("shipped", &dev_config_toml());
    assert_eq!(cfg.superposition.source, SuperpositionSource::File);
    assert_eq!(cfg.superposition.polling_interval, 15);
}
