//!
//! Logger-specific config.
//!

use std::collections::HashMap;

use common_utils::config_patch::Patch;
pub use common_utils::events::LogFieldEntry;
use serde::{Deserialize, Serialize};

/// Unified log fields for golden log lines, split by direction.
///
/// Each map key is a target path (dotted paths like `"api_details.url"` create nested JSON).
/// Each value is a [`LogFieldEntry`] — either a literal or a source field reference.
///
/// Patchable via `x-config-override` header with **merge** semantics:
/// override entries are added to / replace matching base entries; unmentioned entries stay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LogFields {
    /// Runtime toggle: when `false`, compiled log fields are not applied
    /// even though the `log-transformations` feature is compiled in.
    /// Defaults to `false` — must be explicitly enabled in config.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub incoming: HashMap<String, LogFieldEntry>,
    #[serde(default)]
    pub outgoing: HashMap<String, LogFieldEntry>,
}

/// Patch type for [`LogFields`] — manually defined for merge semantics.
///
/// When applied, each direction's map is **extended** (not replaced):
/// new keys are added, existing keys are overwritten, absent keys are kept.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogFieldsPatch {
    pub enabled: Option<bool>,
    pub incoming: Option<HashMap<String, LogFieldEntry>>,
    pub outgoing: Option<HashMap<String, LogFieldEntry>>,
}

impl Patch<LogFieldsPatch> for LogFields {
    fn apply(&mut self, patch: LogFieldsPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(incoming) = patch.incoming {
            self.incoming.extend(incoming);
        }
        if let Some(outgoing) = patch.outgoing {
            self.outgoing.extend(outgoing);
        }
    }
}

/// Log config settings.
#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct Log {
    /// Logging to a console.
    pub console: LogConsole,
    /// Logging to Kafka (optional).
    #[serde(default)]
    pub kafka: Option<LogKafka>,
    /// Unified log fields for golden log lines.
    /// Each entry is either a literal value or a source field reference.
    /// Dotted target paths create nested JSON objects.
    /// Patchable per-request via `x-config-override` with merge semantics.
    #[serde(default)]
    #[patch(patch_type = LogFieldsPatch)]
    pub fields: LogFields,
}

/// Logging to a console.
#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq, config_patch_derive::Patch)]
pub struct LogConsole {
    /// Whether you want to see log in your terminal.
    pub enabled: bool,
    /// What you see in your terminal.
    pub level: Level,
    /// Log format
    pub log_format: LogFormat,
    /// Directive which sets the log level for one or more crates/modules.
    pub filtering_directive: Option<String>,
}

/// Describes the level of verbosity of a span or event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, config_patch_derive::Patch)]
pub struct Level(pub(super) tracing::Level);

impl Serialize for Level {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl Level {
    /// Returns the most verbose [`tracing::Level`]
    pub fn into_level(&self) -> tracing::Level {
        self.0
    }
}

impl Default for Level {
    fn default() -> Self {
        Self(tracing::Level::INFO)
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::str::FromStr as _;

        let s = String::deserialize(deserializer)?;
        tracing::Level::from_str(&s)
            .map(Level)
            .map_err(serde::de::Error::custom)
    }
}

/// Telemetry / tracing.
#[derive(
    Default, Debug, Deserialize, Serialize, Clone, PartialEq, Eq, config_patch_derive::Patch,
)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Default pretty log format
    Default,
    /// JSON based structured logging
    #[default]
    Json,
}

/// Logging to Kafka.
#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Default, config_patch_derive::Patch)]
pub struct LogKafka {
    /// Whether Kafka logging is enabled.
    pub enabled: bool,
    /// Minimum log level for Kafka logging.
    pub level: Level,
    /// Directive which sets the log level for one or more crates/modules.
    pub filtering_directive: Option<String>,
    /// Kafka broker addresses.
    pub brokers: Vec<String>,
    /// Topic name for logs.
    pub topic: String,
    /// Batch size for Kafka messages (optional, defaults to Kafka default).
    #[serde(default)]
    pub batch_size: Option<usize>,
    /// Flush interval in milliseconds (optional, defaults to Kafka default).
    #[serde(default)]
    pub flush_interval_ms: Option<u64>,
    /// Buffer limit for Kafka messages (optional, defaults to Kafka default).
    #[serde(default)]
    pub buffer_limit: Option<usize>,
}
