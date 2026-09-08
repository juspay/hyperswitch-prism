//! Typed configuration for the déjà record/replay harness.
//!
//! This module is compiled **only** under `--features deja`. It contains plain
//! `serde` data types and references no `deja` crate symbols, so the `deja`
//! feature on this crate is dependency-free (a pure cfg flag).
//!
//! Defaults are fail-closed throughout: [`DejaMode::Disabled`], sampler
//! fail-closed, empty broker list. Nothing here has any runtime effect until the
//! boot hook is installed (a later change in `grpc-server`).

use common_utils::consts;

/// Root déjà configuration, surfaced on `Config` as the optional `[deja]` table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct DejaConfig {
    /// Operating mode. Defaults to [`DejaMode::Disabled`].
    pub mode: DejaMode,
    /// Explicit recording run id. Empty/absent => a `run-{now_ns}` id is minted at boot.
    pub run_id: Option<String>,
    pub recording: RecordingConfig,
    pub replay: ReplayConfig,
    pub sampler: SamplerConfig,
    pub identity: IdentityConfig,
    pub writer: WriterConfig,
}

impl Default for DejaConfig {
    fn default() -> Self {
        Self {
            mode: DejaMode::Disabled,
            run_id: None,
            recording: RecordingConfig::default(),
            replay: ReplayConfig::default(),
            sampler: SamplerConfig::default(),
            identity: IdentityConfig::default(),
            writer: WriterConfig::default(),
        }
    }
}

impl DejaConfig {
    /// Fail-loud validation, run at boot. Replay must never be reachable in
    /// production, and recording needs somewhere to send events.
    pub fn validate(&self, environment: &consts::Env) -> Result<(), config::ConfigError> {
        match self.mode {
            DejaMode::Disabled => Ok(()),
            DejaMode::Replay => match environment {
                consts::Env::Production => Err(config::ConfigError::Message(
                    "deja.mode = \"replay\" is not permitted in the production environment"
                        .to_string(),
                )),
                _ => Ok(()),
            },
            DejaMode::Record => {
                // Empty brokers are permitted here: the boot installer inherits the
                // `[events]` broker list when this is empty. A truly unresolved broker
                // set fails open at producer creation (never blocks a request).
                Ok(())
            }
        }
    }

    /// Whether the process should install any hook at all.
    pub fn is_observing(&self) -> bool {
        !matches!(self.mode, DejaMode::Disabled)
    }

    /// The configured run id, filtering empty strings.
    pub fn effective_run_id(&self) -> Option<&str> {
        self.run_id
            .as_deref()
            .map(str::trim)
            .filter(|run_id| !run_id.is_empty())
    }
}

/// Record / replay / off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DejaMode {
    /// No instrumentation is active (default, and byte-identical to today).
    #[default]
    Disabled,
    /// Capture boundary events onto the tape.
    Record,
    /// Substitute recorded values; never touch live systems.
    Replay,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RecordingConfig {
    pub kafka: RecordingKafkaConfig,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RecordingKafkaConfig {
    /// Topic every recording envelope is published to.
    pub topic: String,
    /// Broker list. Empty => inherit the `[events]` broker list at boot.
    pub brokers: Vec<String>,
    pub acks: String,
    pub idempotence: bool,
    pub client_id: Option<String>,
    pub compression: Option<String>,
    pub linger_ms: Option<u64>,
    pub message_timeout_ms: u64,
    /// Loss-budget knob: a full producer buffer surfaces as counted drops, never OOM.
    pub queue_buffering_max_messages: u64,
    pub queue_buffering_max_kbytes: u64,
}

impl Default for RecordingKafkaConfig {
    fn default() -> Self {
        Self {
            topic: "ucs-deja-recording-events".to_string(),
            brokers: Vec::new(),
            acks: "all".to_string(),
            idempotence: true,
            client_id: None,
            compression: None,
            linger_ms: None,
            message_timeout_ms: 30_000,
            queue_buffering_max_messages: 100_000,
            queue_buffering_max_kbytes: 262_144,
        }
    }
}

impl RecordingKafkaConfig {
    /// The configured topic, filtering empty strings.
    pub fn effective_topic(&self) -> Option<&str> {
        let topic = self.topic.trim();
        (!topic.is_empty()).then_some(topic)
    }
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ReplayConfig {
    /// Absolute tape path, or a path relative to `lookup_dir`.
    pub source: Option<String>,
    pub lookup_dir: Option<String>,
    /// Where the replay hook writes its observed-call stream; in-memory when unset.
    pub observed_sink: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SamplerConfig {
    /// Superposition boolean key consulted per request.
    pub record_key: String,
    /// On sampler error, default to not recording.
    pub fail_closed: bool,
    /// Honor an upstream recorder's own decision (`x-deja-record` on the incoming
    /// call) so both services tape the same flow — hyperswitch trickling its
    /// sampling decision down to UCS. OFF by default: a client-supplied header
    /// must not drive recording unless the deployment opts in.
    pub honor_upstream: bool,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            record_key: "deja_record".to_string(),
            fail_closed: true,
            honor_upstream: false,
        }
    }
}

/// Who this recorder IS — stamped onto every tape envelope so a recording can
/// be traced back to the build and pod that produced it.
///
/// Set like every other `[deja]` key: `[deja.identity]` in the config TOML or
/// `CS__DEJA__IDENTITY__*` environment overrides. Nothing needs to be set for
/// the defaults to work: `pod_name_env`/`git_sha_env` name ENVIRONMENT
/// VARIABLES to read at boot (defaults `POD_NAME` — the k8s Downward API
/// convention — and `VERGEN_GIT_SHA`), while `instance_id`/`code_sha` are
/// direct literal overrides that win over the env lookups. When everything is
/// absent, boot falls back to `pi-{pid}-{boot_nanos}` for the instance and the
/// compile-time `VERGEN_GIT_SHA` baked into the binary for the sha.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct IdentityConfig {
    /// Name of the env var carrying the pod name (default `POD_NAME`).
    pub pod_name_env: String,
    /// Name of the env var carrying the git sha (default `VERGEN_GIT_SHA`).
    pub git_sha_env: String,
    /// Literal instance-id override; wins over `pod_name_env`.
    pub instance_id: Option<String>,
    /// Literal code-sha override; wins over `git_sha_env` and the baked sha.
    pub code_sha: Option<String>,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            pod_name_env: "POD_NAME".to_string(),
            git_sha_env: "VERGEN_GIT_SHA".to_string(),
            instance_id: None,
            code_sha: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WriterConfig {
    pub queue_capacity: usize,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    /// Flush after this many records (0 disables the record-count trigger).
    pub flush_after_records: usize,
    /// Bounded drain budget at shutdown.
    pub shutdown_flush_ms: u64,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 8_192,
            batch_size: 500,
            flush_interval_ms: 1_000,
            flush_after_records: 500,
            shutdown_flush_ms: 5_000,
        }
    }
}
