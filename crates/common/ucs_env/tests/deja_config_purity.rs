//! Contract for the whole déjà stack: feature-off, the `[deja]` config surface is
//! provably inert. We load a real config twice through the production loader — once
//! with every `[deja]` table stripped, once with an aggressively-populated `[deja]`
//! block injected — and assert the resulting `Config` is byte-identical. If serde had
//! deposited the injected tables anywhere, the two would differ.
//!
//! The purity test runs only in the default (feature-off) build. A feature-on
//! companion asserts the same surface actually parses when the feature is enabled.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

fn dev_config_toml() -> String {
    let path = ucs_env::configs::workspace_path()
        .join("config")
        .join("development.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Remove every `[deja]` / `[deja.*]` table from a TOML config string.
fn strip_deja_tables(toml: &str) -> String {
    let mut out = String::new();
    let mut in_deja = false;
    for line in toml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            let name = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            in_deja = name == "deja" || name.starts_with("deja.");
        }
        if !in_deja {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

const INJECTED_DEJA_TABLES: &str = r#"
[deja]
mode = "record"
run_id = "purity-injected-run"

[deja.recording.kafka]
topic = "aggressive-injected-topic"
brokers = ["injected-a:9092", "injected-b:9092"]
acks = "all"
idempotence = true
queue_buffering_max_messages = 12345

[deja.replay]
source = "/tmp/injected-tape.jsonl"

[deja.sampler]
record_key = "deja_record"
fail_closed = false
"#;

fn write_temp(tag: &str, toml: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ucs_deja_purity_{}_{}.toml",
        std::process::id(),
        tag
    ));
    std::fs::write(&path, toml).expect("write temp config");
    path
}

fn load(tag: &str, toml: &str) -> ucs_env::configs::Config {
    let path = write_temp(tag, toml);
    let cfg = ucs_env::configs::Config::new_with_config_path(Some(path.clone()))
        .unwrap_or_else(|e| panic!("load {tag} config: {e}"));
    let _ = std::fs::remove_file(&path);
    cfg
}

/// Canonical string with object keys and array elements sorted, so HashMap/HashSet
/// iteration order (nondeterministic across constructions) cannot cause a false diff.
#[cfg(not(feature = "deja"))]
fn canonical(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{k:?}:{}", canonical(&map[k])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let mut inner: Vec<String> = arr.iter().map(canonical).collect();
            inner.sort();
            format!("[{}]", inner.join(","))
        }
        other => other.to_string(),
    }
}

#[cfg(not(feature = "deja"))]
fn canonical_config(cfg: &ucs_env::configs::Config) -> String {
    let value = serde_json::to_value(cfg).expect("serialize Config");
    canonical(&value)
}

#[cfg(not(feature = "deja"))]
#[test]
fn deja_config_surface_is_inert_when_feature_off() {
    let base = dev_config_toml();
    let stripped = strip_deja_tables(&base);
    let injected = format!("{stripped}\n{INJECTED_DEJA_TABLES}");

    let without = load("stripped", &stripped);
    let with = load("injected", &injected);

    assert_eq!(
        canonical_config(&without),
        canonical_config(&with),
        "feature-off: the injected [deja] tables must not affect the loaded Config"
    );
}

#[cfg(feature = "deja")]
#[test]
fn deja_config_parses_when_feature_on() {
    use ucs_env::deja_config::DejaMode;

    // The committed development.toml carries an inert `mode = "disabled"` block.
    let base = dev_config_toml();
    let default_cfg = load("default", &base);
    assert_eq!(default_cfg.deja.mode, DejaMode::Disabled);

    // Injected record block round-trips into the typed surface.
    let injected = format!("{}\n{INJECTED_DEJA_TABLES}", strip_deja_tables(&base));
    let record_cfg = load("record", &injected);
    assert_eq!(record_cfg.deja.mode, DejaMode::Record);
    assert_eq!(
        record_cfg.deja.recording.kafka.brokers,
        vec!["injected-a:9092".to_string(), "injected-b:9092".to_string()]
    );
    assert!(!record_cfg.deja.sampler.fail_closed);
}
