use std::{cell::RefCell, collections::VecDeque, future::Future};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::schema::RecordingEntry;

tokio::task_local! {
    static CURRENT_RUNTIME: RefCell<ArtRuntime>;
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtMode {
    #[default]
    Disabled,
    Record,
    Replay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionContext {
    pub request_id: String,
    pub merchant_id: String,
    pub connector: String,
    pub flow: String,
    pub hostname: String,
}

impl SessionContext {
    pub fn session_id(&self) -> &str {
        &self.request_id
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArtRecorder {
    entries: Vec<RecordingEntry>,
    max_entries: Option<usize>,
}

impl ArtRecorder {
    pub fn new(max_entries: Option<usize>) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn entries(&self) -> &[RecordingEntry] {
        &self.entries
    }

    pub fn push(&mut self, entry: RecordingEntry) -> Result<(), ArtError> {
        if let Some(max_entries) = self.max_entries {
            if self.entries.len() >= max_entries {
                return Err(ArtError::MaxEntriesReached { max_entries });
            }
        }

        self.entries.push(entry);
        Ok(())
    }

    pub fn into_entries(self) -> Vec<RecordingEntry> {
        self.entries
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtRuntimeSettings {
    pub record_incoming_api: bool,
    pub record_outgoing_http: bool,
    pub record_effects: bool,
}

impl Default for ArtRuntimeSettings {
    fn default() -> Self {
        Self {
            record_incoming_api: true,
            record_outgoing_http: true,
            record_effects: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtRuntime {
    mode: ArtMode,
    session: Option<SessionContext>,
    recorder: ArtRecorder,
    replay_entries: VecDeque<RecordingEntry>,
    settings: ArtRuntimeSettings,
}

impl ArtRuntime {
    pub fn disabled() -> Self {
        Self {
            mode: ArtMode::Disabled,
            session: None,
            recorder: ArtRecorder::default(),
            replay_entries: VecDeque::new(),
            settings: ArtRuntimeSettings::default(),
        }
    }

    pub fn recording(session: SessionContext, max_entries: Option<usize>) -> Self {
        Self::recording_with_settings(session, max_entries, ArtRuntimeSettings::default())
    }

    pub fn recording_with_settings(
        session: SessionContext,
        max_entries: Option<usize>,
        settings: ArtRuntimeSettings,
    ) -> Self {
        Self {
            mode: ArtMode::Record,
            session: Some(session),
            recorder: ArtRecorder::new(max_entries),
            replay_entries: VecDeque::new(),
            settings,
        }
    }

    pub fn replay(session: SessionContext, entries: Vec<RecordingEntry>) -> Self {
        Self {
            mode: ArtMode::Replay,
            session: Some(session),
            recorder: ArtRecorder::default(),
            replay_entries: entries.into(),
            settings: ArtRuntimeSettings::default(),
        }
    }

    pub fn mode(&self) -> ArtMode {
        self.mode
    }

    pub fn session(&self) -> Option<&SessionContext> {
        self.session.as_ref()
    }

    pub fn recorded_entries(&self) -> &[RecordingEntry] {
        self.recorder.entries()
    }

    pub fn settings(&self) -> ArtRuntimeSettings {
        self.settings
    }

    pub fn record_entry(&mut self, entry: RecordingEntry) -> Result<(), ArtError> {
        match self.mode {
            ArtMode::Disabled | ArtMode::Replay => Ok(()),
            ArtMode::Record if self.should_record_entry(&entry) => self.recorder.push(entry),
            ArtMode::Record => Ok(()),
        }
    }

    pub fn pop_replay_entry(&mut self) -> Result<RecordingEntry, ArtError> {
        self.replay_entries
            .pop_front()
            .ok_or(ArtError::ReplayEntriesExhausted)
    }

    pub fn into_recorded_entries(self) -> Vec<RecordingEntry> {
        self.recorder.into_entries()
    }

    fn should_record_entry(&self, entry: &RecordingEntry) -> bool {
        match entry {
            RecordingEntry::CallApi(_) | RecordingEntry::CallApiPii(_) => {
                self.settings.record_outgoing_http
            }
            RecordingEntry::IncomingApi(_) => self.settings.record_incoming_api,
            RecordingEntry::Timestamp(_)
            | RecordingEntry::Uuid(_)
            | RecordingEntry::RandomRio(_)
            | RecordingEntry::RandomBytes(_) => self.settings.record_effects,
        }
    }
}

pub async fn scope<F>(runtime: ArtRuntime, future: F) -> (F::Output, ArtRuntime)
where
    F: Future,
{
    CURRENT_RUNTIME
        .scope(RefCell::new(runtime), async {
            let output = future.await;
            let runtime = CURRENT_RUNTIME.with(|runtime| runtime.borrow().clone());
            (output, runtime)
        })
        .await
}

pub fn try_with_current<R>(f: impl FnOnce(&mut ArtRuntime) -> R) -> Option<R> {
    CURRENT_RUNTIME
        .try_with(|runtime| f(&mut runtime.borrow_mut()))
        .ok()
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtError {
    #[error("ART recorder reached max entries per session: {max_entries}")]
    MaxEntriesReached { max_entries: usize },
    #[error("invalid random range: start {start} is greater than end {end}")]
    InvalidRandomRange { start: i64, end: i64 },
    #[error("failed to format ART timestamp: {message}")]
    TimestampFormatting { message: String },
    #[error("ART replay entries are exhausted")]
    ReplayEntriesExhausted,
    #[error("ART replay expected {expected} entry but found {actual}")]
    ReplayEntryTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    #[error("ART replay entry has invalid value for {field}: {message}")]
    InvalidReplayValue {
        field: &'static str,
        message: String,
    },
}
