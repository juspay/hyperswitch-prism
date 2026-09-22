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

fn configured_run_id(config: &DejaConfig, pod_name: Option<&str>) -> String {
    config
        .effective_run_id()
        .map(str::to_owned)
        .unwrap_or_else(|| fallback_run_id(config, pod_name))
}

/// The id a recording is known by when nothing configured one, in the shape
/// hyperswitch mints: `rec-<short sha>-<MMDDhhmm>-<instance>`, e.g.
/// `rec-11d5b8d-09011234-a3` — revision, when, which instance. Without a
/// usable revision it falls back to the bare-timestamp form (with a
/// pre-logger stderr note) rather than claiming a provenance it does not
/// have: `rec-unknown-…` would be worse than an opaque id.
#[allow(clippy::print_stderr)] // The logger is not initialized yet at install time.
fn fallback_run_id(config: &DejaConfig, pod_name: Option<&str>) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs();
    match resolved_code_sha(config) {
        Some(sha) => format!(
            "rec-{}-{}-{}",
            short_revision(&sha),
            recording_stamp(now_secs),
            instance_discriminator(&resolved_instance_id(config, pod_name)),
        ),
        None => {
            eprintln!(
                "deja: recording without a code revision — its id will carry no provenance. \
                 Set deja.identity.code_sha, or build with VERGEN_GIT_SHA."
            );
            format!("run-{}", now_ns())
        }
    }
}

/// A git sha shortened to the length git itself uses for a short sha.
fn short_revision(sha: &str) -> String {
    sha.chars()
        .filter(char::is_ascii_alphanumeric)
        .take(7)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// `MMDDhhmm` UTC; the instance discriminator separates recorders that start
/// in the same minute.
fn recording_stamp(unix_secs: u64) -> String {
    let (month, day) = civil_month_day(unix_secs / 86_400);
    let today = unix_secs % 86_400;
    format!(
        "{month:02}{day:02}{:02}{:02}",
        today / 3600,
        (today % 3600) / 60
    )
}

/// Two characters standing for the instance, so two pods that start in the
/// same minute do not share a recording id.
fn instance_discriminator(instance_id: &str) -> String {
    // FNV-1a, so the same pod is always the same two characters.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in instance_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    // `n % 36` always indexes the 36-byte alphabet; spelled out instead of
    // asserted because a boot-time naming helper must not panic.
    let pick = |n: u64| {
        usize::try_from(n % 36)
            .ok()
            .and_then(|i| ALPHABET.get(i))
            .map_or('0', |byte| char::from(*byte))
    };
    let a = pick(hash);
    let b = pick(hash / 36);
    format!("{a}{b}")
}

/// Days since the epoch to (month, day), civil-from-days.
fn civil_month_day(days_since_epoch: u64) -> (i64, i64) {
    // An impossible clock degrades to a valid date instead of panicking.
    let Some(z) = i64::try_from(days_since_epoch)
        .ok()
        .and_then(|days| days.checked_add(719_468))
    else {
        return (1, 1);
    };
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (m, d)
}

fn configured_value(value: Option<&str>) -> Option<String> {
    non_empty(value).map(str::to_owned)
}

fn env_value_named(name: &str) -> Option<String> {
    let name = non_empty(Some(name))?;
    configured_value(std::env::var(name).ok().as_deref())
}

/// config → deployment pod name (Downward API via runtime_metadata) → pod-name env var →
/// hostname → `pi-{pid}-{now_ns}`.
///
/// The hostname step is what makes identity work with ZERO infra injection:
/// Kubernetes sets a pod's hostname to its pod name, and the deployment naming
/// convention already packs application, release tag, replicaset hash, and pod
/// suffix into it (e.g. `connector-service-grpc-2026o07o17o0ohotfix1-544b64f4cd-lr966`)
/// — self-identifying as ONE opaque string, deliberately never parsed into
/// parts (the segments are a naming convention, not an API).
fn resolved_instance_id(config: &DejaConfig, pod_name: Option<&str>) -> String {
    configured_value(config.identity.instance_id.as_deref())
        .or_else(|| configured_value(pod_name))
        .or_else(|| env_value_named(&config.identity.pod_name_env))
        .or_else(|| {
            let hostname = gethostname::gethostname();
            configured_value(hostname.to_str())
        })
        .unwrap_or_else(|| format!("pi-{}-{}", std::process::id(), now_ns()))
}

fn resolved_code_sha(config: &DejaConfig) -> Option<String> {
    configured_value(config.identity.code_sha.as_deref())
        .or_else(|| env_value_named(&config.identity.git_sha_env))
        .or_else(|| option_env!("VERGEN_GIT_SHA").map(str::to_owned))
    // No "unknown" placeholder: absence is a fact worth being able to observe.
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

    let run_id = configured_run_id(config, pod_name);
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

    // A directory source = a raw-tape artifact dir (`semantic-events.jsonl` of tagged
    // DejaRecord lines): replay via the in-process event cascade — no pre-rendered
    // lookup table (and no orchestrator) needed. A file source = a rendered
    // LookupTable JSONL, the orchestrator's output.
    if lookup_path.is_dir() {
        let hook = deja::ReplayHook::from_artifact_dir(&lookup_path).map_err(|error| {
            format!(
                "failed to load replay artifacts '{}': {error}",
                lookup_path.display()
            )
        })?;
        return try_install_hook(
            deja::RuntimeHook::Replay(hook),
            InstallReport {
                mode: "replay",
                run_id: config.effective_run_id().map(str::to_owned),
                detail: Some(format!("raw-tape artifacts '{}'", lookup_path.display())),
            },
        )
        .map_err(|error| format!("failed to install replay runtime hook: {error}"));
    }

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

#[cfg(test)]
mod tests {
    // Fixture assertions index freely — a panic IS the test failing.
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// The stamp is `MMDDhhmm` UTC — checked at the epoch, at a current-era
    /// date, and on a leap day (the civil-from-days math has to earn that one).
    #[test]
    fn recording_stamp_is_month_day_hour_minute_utc() {
        assert_eq!(recording_stamp(0), "01010000"); // 1970-01-01 00:00
        assert_eq!(recording_stamp(1_788_266_040), "09011234"); // 2026-09-01 12:34
        assert_eq!(recording_stamp(1_709_251_140), "02292359"); // 2024-02-29 23:59
    }

    /// Same instance → same two characters, always from the base-36 alphabet;
    /// distinct pods that boot in the same minute must not share an id.
    #[test]
    fn instance_discriminator_is_stable_and_two_base36_chars() {
        let a = instance_discriminator("connector-service-7d9f8b6c4-x2vlq");
        assert_eq!(
            a,
            instance_discriminator("connector-service-7d9f8b6c4-x2vlq")
        );
        assert_eq!(a.len(), 2);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_ne!(
            a,
            instance_discriminator("connector-service-7d9f8b6c4-9k3mp")
        );
    }

    /// Non-alphanumerics are filtered, casing is normalized, and git's own
    /// short-sha length is what survives.
    #[test]
    fn short_revision_takes_seven_lowercase_alphanumerics() {
        assert_eq!(short_revision("11d5b8dbd"), "11d5b8d");
        assert_eq!(short_revision("G1-1D5B8dbd"), "g11d5b8");
    }

    /// This build carries VERGEN_GIT_SHA (the same baked source the envelopes'
    /// `code.sha` uses), so a default config must mint the full hyperswitch
    /// shape — never the bare-timestamp fallback and never `rec-unknown-…`.
    #[test]
    fn fallback_run_id_has_the_hyperswitch_shape() {
        let id = fallback_run_id(&DejaConfig::default(), Some("pod-a"));
        let parts: Vec<&str> = id.splitn(4, '-').collect();
        assert_eq!(
            parts.len(),
            4,
            "expected rec-<sha>-<stamp>-<inst>, got {id}"
        );
        assert_eq!(parts[0], "rec");
        assert_ne!(parts[1], "unknown");
        assert!(
            (1..=7).contains(&parts[1].len())
                && parts[1].chars().all(|c| c.is_ascii_alphanumeric()),
            "revision part malformed in {id}"
        );
        assert!(
            parts[2].len() == 8 && parts[2].chars().all(|c| c.is_ascii_digit()),
            "stamp part malformed in {id}"
        );
        assert_eq!(parts[3].len(), 2, "instance part malformed in {id}");
    }
}
