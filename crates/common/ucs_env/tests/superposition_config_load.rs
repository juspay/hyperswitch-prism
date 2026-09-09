//! Boot contract for the `[superposition]` table: **Superposition is never a reason to
//! refuse boot.** An enabled remote source whose settings cannot describe a workspace
//! is reported and switched off — the process serves policy from the baked file — the
//! same fail-open posture as a déjà record misconfiguration. A complete block loads
//! as written. Both go through the production loader, not a hand-built struct.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

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

/// `enabled = true` with nothing else set: the loader must NOT fail. The remote
/// source is switched off and the baked file serves policy.
#[test]
fn enabled_but_undescribed_remote_source_loads_and_falls_back_to_the_file() {
    let toml = with_superposition_table(&dev_config_toml(), "\n[superposition]\nenabled = true\n");
    let cfg = load("undescribed", &toml);
    assert!(
        !cfg.superposition.enabled,
        "an enabled remote source with no endpoint/token/org/workspace must be switched off, not abort boot"
    );
}

/// A complete block loads as written, secret included.
#[test]
fn complete_remote_block_loads_enabled() {
    let block = r#"
[superposition]
enabled = true
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
    assert!(cfg.superposition.enabled);
    assert_eq!(cfg.superposition.endpoint, "http://superposition:8080");
    assert_eq!(cfg.superposition.token.peek(), "sp_test_token");
    assert_eq!(cfg.superposition.workspace_id, "prism");
    assert_eq!(cfg.superposition.polling_interval, 7);
}

/// The shipped development config keeps the remote source off.
#[test]
fn shipped_development_config_is_file_first() {
    let cfg = load("shipped", &dev_config_toml());
    assert!(!cfg.superposition.enabled);
    assert_eq!(cfg.superposition.polling_interval, 15);
}
