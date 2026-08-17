//! Boot-time composition of the process-wide déjà runtime hook.
//!
//! Typed `[deja]` config selects disabled, Kafka recording, or lookup-table replay, and
//! this module eagerly installs the hook **before** any boundary or logger layer can peek
//! (and latch) the default disabled state.
//!
//! Failure policy: **record misconfiguration fails open** — an invalid record config
//! installs a disabled hook with a pre-logger stderr note and boot continues (payments are
//! never blocked by instrumentation). **Replay misconfiguration fails loud** — the error
//! aborts boot (a replay rig must never silently run live).
//!
//! Identity fallbacks here use raw `SystemTime` / `process::id()` deliberately: the seamed
//! helpers would recurse into the hook this module is installing.

use std::{path::PathBuf, sync::Arc};

use ucs_env::deja_config::{DejaConfig, DejaMode, ReplayConfig};

use super::record_sink::{UcsKafkaRecordSink, UcsKafkaRecordSinkConfig};

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub mode: &'static str,
    pub run_id: Option<String>,
    pub detail: Option<String>,
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn now_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_nanos()
}

fn configured_run_id(config: &DejaConfig) -> String {
    config
        .effective_run_id()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("run-{}", now_ns()))
}

fn configured_value(value: Option<&str>) -> Option<String> {
    non_empty(value).map(str::to_owned)
}

fn env_value_named(name: &str) -> Option<String> {
    let name = non_empty(Some(name))?;
    configured_value(std::env::var(name).ok().as_deref())
}

/// config → deployment pod name (Downward API via runtime_metadata) → pod-name env var →
/// `pi-{pid}-{now_ns}`.
fn resolved_instance_id(config: &DejaConfig, pod_name: Option<&str>) -> String {
    configured_value(config.identity.instance_id.as_deref())
        .or_else(|| configured_value(pod_name))
        .or_else(|| env_value_named(&config.identity.pod_name_env))
        .unwrap_or_else(|| format!("pi-{}-{}", std::process::id(), now_ns()))
}

fn resolved_code_sha(config: &DejaConfig) -> Option<String> {
    configured_value(config.identity.code_sha.as_deref())
        .or_else(|| env_value_named(&config.identity.git_sha_env))
        .or_else(|| option_env!("VERGEN_GIT_SHA").map(str::to_owned))
        .or_else(|| Some("unknown".to_owned()))
}

fn writer_config(config: &DejaConfig) -> deja::WriterConfig {
    let writer = &config.writer;
    deja::WriterConfig {
        queue_capacity: writer.queue_capacity.max(1),
        batch_size: writer.batch_size.max(1),
        flush_interval: std::time::Duration::from_millis(writer.flush_interval_ms.max(1)),
        flush_timeout: std::time::Duration::from_millis(writer.shutdown_flush_ms.max(1)),
        flush_after_records: (writer.flush_after_records > 0).then_some(writer.flush_after_records),
        policy: deja::SinkPolicy::FailOpen,
    }
}

fn disabled_report(detail: Option<String>) -> InstallReport {
    InstallReport {
        mode: "disabled",
        run_id: None,
        detail,
    }
}

#[allow(clippy::print_stderr)] // The logger is not initialized yet at install time.
fn print_configuration_error(error: &str) {
    eprintln!("deja configuration error: {error}; runtime hook disabled");
}

fn try_install_hook(
    hook: deja::RuntimeHook,
    report: InstallReport,
) -> Result<InstallReport, String> {
    deja::set_global_runtime_hook(Some(hook))
        .map_err(|error| error.to_owned())
        .map(|()| report)
}

#[allow(clippy::print_stderr)] // The logger is not initialized yet at install time.
fn install_hook(hook: deja::RuntimeHook, report: InstallReport) -> InstallReport {
    match try_install_hook(hook, report) {
        Ok(report) => report,
        Err(error) => {
            eprintln!(
                "deja configuration error: {error}; requested runtime hook was not installed"
            );
            disabled_report(Some(error))
        }
    }
}

fn install_disabled(detail: Option<String>) -> InstallReport {
    if let Some(error) = detail.as_deref() {
        print_configuration_error(error);
    }
    install_hook(
        deja::RuntimeHook::Disabled(deja::DisabledHook),
        disabled_report(detail),
    )
}

fn install_record(
    config: &DejaConfig,
    inherited_brokers: Option<&[String]>,
    pod_name: Option<&str>,
) -> InstallReport {
    let kafka = &config.recording.kafka;
    let Some(topic) = kafka.effective_topic() else {
        return install_disabled(Some(
            "record mode requires deja.recording.kafka.topic".to_owned(),
        ));
    };

    // Broker resolution: an explicit deja broker list wins; an empty list inherits the
    // deployment's `[events]` brokers — shared cluster provisioning, separate client.
    let brokers: &[String] = if kafka.brokers.is_empty() {
        inherited_brokers.unwrap_or_default()
    } else {
        kafka.brokers.as_slice()
    };
    if brokers.is_empty() || brokers.iter().any(|broker| broker.trim().is_empty()) {
        return install_disabled(Some(
            "record mode requires Kafka brokers: set deja.recording.kafka.brokers, or \
             configure [events] brokers for the recording sink to inherit"
                .to_owned(),
        ));
    }

    let run_id = configured_run_id(config);
    let sink = match UcsKafkaRecordSink::new(UcsKafkaRecordSinkConfig {
        brokers,
        topic,
        recording_run_id: &run_id,
        instance_id: resolved_instance_id(config, pod_name),
        code_sha: resolved_code_sha(config),
        client_id: kafka.client_id.as_deref(),
        acks: &kafka.acks,
        enable_idempotence: kafka.idempotence,
        compression: kafka.compression.as_deref(),
        linger_ms: kafka.linger_ms,
        message_timeout_ms: kafka.message_timeout_ms,
        queue_buffering_max_messages: kafka.queue_buffering_max_messages,
        queue_buffering_max_kbytes: kafka.queue_buffering_max_kbytes,
    }) {
        Ok(sink) => sink,
        Err(error) => {
            return install_disabled(Some(format!(
                "failed to create deja Kafka producer for topic '{topic}': {error}"
            )));
        }
    };

    let hook = Arc::new(deja::RecordingHook::with_sink(
        sink,
        run_id.clone(),
        writer_config(config),
    ));
    install_hook(
        deja::RuntimeHook::Recording(hook),
        InstallReport {
            mode: "record",
            run_id: Some(run_id),
            detail: Some(format!("Kafka topic '{topic}'")),
        },
    )
}

/// Resolve the lookup-table path from `deja.replay.{source, lookup_dir}` with ONE rule:
/// absolute `source` wins; relative `source` requires `lookup_dir`; `lookup_dir` alone
/// requires `run_id` (→ `<dir>/<run_id>.jsonl`). Anything else is a configuration error.
fn replay_lookup_path(config: &DejaConfig, replay: &ReplayConfig) -> Result<PathBuf, String> {
    let lookup_dir = non_empty(replay.lookup_dir.as_deref()).map(PathBuf::from);
    match (non_empty(replay.source.as_deref()), lookup_dir) {
        (Some(source), _) if PathBuf::from(source).is_absolute() => Ok(PathBuf::from(source)),
        (Some(source), Some(lookup_dir)) => Ok(lookup_dir.join(source)),
        (Some(source), None) => Err(format!(
            "deja.replay.source '{source}' is relative; set deja.replay.lookup_dir or make it absolute"
        )),
        (None, Some(lookup_dir)) => match config.effective_run_id() {
            Some(run_id) => Ok(lookup_dir.join(format!("{run_id}.jsonl"))),
            None => Err(
                "deja.replay.lookup_dir without deja.replay.source requires deja.run_id".to_owned(),
            ),
        },
        (None, None) => {
            Err("replay mode requires deja.replay.source or deja.replay.lookup_dir".to_owned())
        }
    }
}

fn install_replay(config: &DejaConfig) -> Result<InstallReport, String> {
    let lookup_path = replay_lookup_path(config, &config.replay)?;

    let hook = match non_empty(config.replay.observed_sink.as_deref()) {
        Some(path) => match deja::FileObservedSink::create(path) {
            Ok(sink) => deja::LookupTableHook::from_source(
                deja::LocalFileLookupSource::new(lookup_path.clone()),
                sink,
            ),
            Err(error) => {
                return Err(format!(
                    "failed to open replay observed sink '{path}': {error}"
                ));
            }
        },
        None => deja::LookupTableHook::from_source(
            deja::LocalFileLookupSource::new(lookup_path.clone()),
            deja::InMemoryObservedSink::new(),
        ),
    };

    let hook = hook.map_err(|error| {
        format!(
            "failed to load replay lookup table '{}': {error}",
            lookup_path.display()
        )
    })?;
    let entries = hook.entry_count();

    try_install_hook(
        deja::RuntimeHook::LookupReplay(hook),
        InstallReport {
            mode: "replay",
            run_id: config.effective_run_id().map(str::to_owned),
            detail: Some(format!(
                "lookup table '{}' with {entries} entries",
                lookup_path.display()
            )),
        },
    )
    .map_err(|error| format!("failed to install replay runtime hook: {error}"))
}

/// Compose and install the process-wide déjà runtime hook from typed config.
///
/// Must run before `logger::setup` and before any instrumented call: the hook cell
/// latches on first peek. `Err` aborts boot (replay misconfiguration only).
pub fn install(
    config: &DejaConfig,
    inherited_brokers: Option<&[String]>,
    pod_name: Option<&str>,
) -> Result<InstallReport, String> {
    match &config.mode {
        DejaMode::Disabled => Ok(install_disabled(None)),
        DejaMode::Record => Ok(install_record(config, inherited_brokers, pod_name)),
        DejaMode::Replay => install_replay(config),
    }
}
