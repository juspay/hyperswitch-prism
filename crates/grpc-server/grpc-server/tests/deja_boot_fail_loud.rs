//! Boot-contract tests for the déjà hook installer.
//!
//! Pins the failure-policy asymmetry: **record misconfiguration fails open** (a disabled
//! hook is installed and boot continues) while **replay misconfiguration fails loud**
//! (an `Err` aborts boot before anything serves traffic).
//!
//! Process note: `set_global_runtime_hook` latches once per process. The fail-loud replay
//! cases return `Err` *before* any install, so any number of them can run in one test
//! binary; exactly one fail-open case (which installs a disabled hook) is safe alongside.
#![cfg(feature = "deja")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use grpc_server::deja::boot;
use ucs_env::deja_config::{DejaConfig, DejaMode};

fn config_with_mode(mode: DejaMode) -> DejaConfig {
    DejaConfig {
        mode,
        ..DejaConfig::default()
    }
}

#[test]
fn replay_without_source_or_lookup_dir_fails_loud() {
    let config = config_with_mode(DejaMode::Replay);
    let error = boot::install(&config, None, None).expect_err("replay misconfig must abort boot");
    assert!(
        error.contains("deja.replay.source") && error.contains("deja.replay.lookup_dir"),
        "error should name both knobs: {error}"
    );
}

#[test]
fn replay_with_relative_source_and_no_lookup_dir_fails_loud() {
    let mut config = config_with_mode(DejaMode::Replay);
    config.replay.source = Some("tape.jsonl".to_owned());
    let error = boot::install(&config, None, None).expect_err("relative source needs lookup_dir");
    assert!(
        error.contains("tape.jsonl") && error.contains("lookup_dir"),
        "error should name the source and the missing knob: {error}"
    );
}

#[test]
fn replay_with_missing_lookup_file_fails_loud() {
    let mut config = config_with_mode(DejaMode::Replay);
    config.replay.source = Some("/nonexistent/deja-purity/tape.jsonl".to_owned());
    let error = boot::install(&config, None, None).expect_err("missing tape must abort boot");
    assert!(
        error.contains("/nonexistent/deja-purity/tape.jsonl"),
        "error should carry the resolved path: {error}"
    );
}

/// Record misconfiguration (no topic) fails OPEN: install succeeds, the hook is disabled,
/// boot would continue, and the report names the missing knob. This is the one installing
/// test in this binary (the hook cell latches).
#[test]
fn record_without_topic_fails_open_with_disabled_hook() {
    let mut config = config_with_mode(DejaMode::Record);
    config.recording.kafka.topic = String::new();
    let report =
        boot::install(&config, None, None).expect("record misconfig must never abort boot");
    assert_eq!(report.mode, "disabled");
    assert!(report.run_id.is_none());
    assert!(
        report
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("deja.recording.kafka.topic")),
        "report should name the missing knob: {:?}",
        report.detail
    );
}
