// This module owns the ART effect boundaries, so direct system time, UUID, and
// random calls are expected here and replayed/recorded by the wrapper APIs.
#![allow(clippy::disallowed_methods)]

use std::ops::RangeInclusive;

use rand::{Rng, RngCore};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    runtime::{self, ArtError, ArtMode, ArtRuntime},
    schema::{
        CallApiEntry, IncomingApiEntry, RandomBytesEntry, RandomRioEntry, RecordingEntry,
        TimestampEntry, UuidEntry,
    },
};

pub fn now(tag: impl Into<String>) -> Result<serde_json::Value, ArtError> {
    let tag = tag.into();
    runtime::try_with_current(|runtime| now_with_runtime(runtime, tag.clone())).unwrap_or_else(
        || {
            let mut runtime = ArtRuntime::disabled();
            now_with_runtime(&mut runtime, tag)
        },
    )
}

pub fn now_with_runtime(
    runtime: &mut ArtRuntime,
    tag: impl Into<String>,
) -> Result<serde_json::Value, ArtError> {
    let tag = tag.into();
    if runtime.mode() == ArtMode::Replay {
        return replay_timestamp(runtime);
    }

    let timestamp = serde_json::Value::String(OffsetDateTime::now_utc().format(&Rfc3339).map_err(
        |error| ArtError::TimestampFormatting {
            message: error.to_string(),
        },
    )?);

    runtime.record_entry(RecordingEntry::Timestamp(TimestampEntry::new(
        timestamp.clone(),
        tag,
    )))?;

    Ok(timestamp)
}

pub fn uuid_v4(tag: impl Into<String>) -> Result<String, ArtError> {
    let tag = tag.into();
    runtime::try_with_current(|runtime| uuid_v4_with_runtime(runtime, tag.clone())).unwrap_or_else(
        || {
            let mut runtime = ArtRuntime::disabled();
            uuid_v4_with_runtime(&mut runtime, tag)
        },
    )
}

pub fn uuid_v4_with_runtime(
    runtime: &mut ArtRuntime,
    tag: impl Into<String>,
) -> Result<String, ArtError> {
    uuid_effect(runtime, "uuidV4", tag, || uuid::Uuid::new_v4().to_string())
}

pub fn uuid_v7(tag: impl Into<String>) -> Result<String, ArtError> {
    let tag = tag.into();
    runtime::try_with_current(|runtime| uuid_v7_with_runtime(runtime, tag.clone())).unwrap_or_else(
        || {
            let mut runtime = ArtRuntime::disabled();
            uuid_v7_with_runtime(&mut runtime, tag)
        },
    )
}

pub fn uuid_v7_with_runtime(
    runtime: &mut ArtRuntime,
    tag: impl Into<String>,
) -> Result<String, ArtError> {
    uuid_effect(runtime, "uuidV7", tag, || uuid::Uuid::now_v7().to_string())
}

fn uuid_effect(
    runtime: &mut ArtRuntime,
    function_name: &'static str,
    tag: impl Into<String>,
    generate: impl FnOnce() -> String,
) -> Result<String, ArtError> {
    let tag = tag.into();
    if runtime.mode() == ArtMode::Replay {
        return replay_uuid(runtime, function_name);
    }

    let uuid = generate();
    runtime.record_entry(RecordingEntry::Uuid(UuidEntry::new(
        function_name,
        uuid.clone(),
        tag,
    )))?;

    Ok(uuid)
}

pub fn random_i64_range(
    range: RangeInclusive<i64>,
    tag: impl Into<String>,
) -> Result<i64, ArtError> {
    let tag = tag.into();
    runtime::try_with_current(|runtime| {
        random_i64_range_with_runtime(runtime, range.clone(), tag.clone())
    })
    .unwrap_or_else(|| {
        let mut runtime = ArtRuntime::disabled();
        random_i64_range_with_runtime(&mut runtime, range, tag)
    })
}

pub fn random_i64_range_with_runtime(
    runtime: &mut ArtRuntime,
    range: RangeInclusive<i64>,
    tag: impl Into<String>,
) -> Result<i64, ArtError> {
    let start = *range.start();
    let end = *range.end();
    let tag = tag.into();

    if start > end {
        return Err(ArtError::InvalidRandomRange { start, end });
    }

    if runtime.mode() == ArtMode::Replay {
        return replay_random_i64(runtime);
    }

    let value = rand::thread_rng().gen_range(range);
    runtime.record_entry(RecordingEntry::RandomRio(RandomRioEntry::new(
        json!([start, end]),
        json!(value),
        tag,
    )))?;

    Ok(value)
}

pub fn random_bytes(len: usize, tag: impl Into<String>) -> Result<Vec<u8>, ArtError> {
    let tag = tag.into();
    runtime::try_with_current(|runtime| random_bytes_with_runtime(runtime, len, tag.clone()))
        .unwrap_or_else(|| {
            let mut runtime = ArtRuntime::disabled();
            random_bytes_with_runtime(&mut runtime, len, tag)
        })
}

pub fn random_bytes_with_runtime(
    runtime: &mut ArtRuntime,
    len: usize,
    tag: impl Into<String>,
) -> Result<Vec<u8>, ArtError> {
    let tag = tag.into();
    if runtime.mode() == ArtMode::Replay {
        return replay_random_bytes(runtime);
    }

    let mut bytes = vec![0; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    runtime.record_entry(RecordingEntry::RandomBytes(RandomBytesEntry::from_bytes(
        bytes.clone(),
        tag,
    )))?;

    Ok(bytes)
}

pub fn record_outgoing_http(entry: CallApiEntry) -> Result<(), ArtError> {
    runtime::try_with_current(|runtime| record_outgoing_http_with_runtime(runtime, entry))
        .unwrap_or(Ok(()))
}

pub fn record_outgoing_http_with_runtime(
    runtime: &mut ArtRuntime,
    entry: CallApiEntry,
) -> Result<(), ArtError> {
    runtime.record_entry(RecordingEntry::CallApi(entry))
}

pub fn record_incoming_api(entry: IncomingApiEntry) -> Result<(), ArtError> {
    runtime::try_with_current(|runtime| record_incoming_api_with_runtime(runtime, entry))
        .unwrap_or(Ok(()))
}

pub fn record_incoming_api_with_runtime(
    runtime: &mut ArtRuntime,
    entry: IncomingApiEntry,
) -> Result<(), ArtError> {
    runtime.record_entry(RecordingEntry::IncomingApi(entry))
}

fn replay_timestamp(runtime: &mut ArtRuntime) -> Result<serde_json::Value, ArtError> {
    match runtime.pop_replay_entry()? {
        RecordingEntry::Timestamp(entry) => Ok(entry.timestamp),
        other => Err(type_mismatch("TimeStampEntryT", &other)),
    }
}

fn replay_uuid(
    runtime: &mut ArtRuntime,
    expected_function_name: &'static str,
) -> Result<String, ArtError> {
    match runtime.pop_replay_entry()? {
        RecordingEntry::Uuid(entry) if entry.function_name == expected_function_name => {
            Ok(entry.uuid)
        }
        RecordingEntry::Uuid(entry) => Err(ArtError::InvalidReplayValue {
            field: "functionName",
            message: format!(
                "expected {expected_function_name}, found {}",
                entry.function_name
            ),
        }),
        other => Err(type_mismatch("UuidEntryT", &other)),
    }
}

fn replay_random_i64(runtime: &mut ArtRuntime) -> Result<i64, ArtError> {
    match runtime.pop_replay_entry()? {
        RecordingEntry::RandomRio(entry) => {
            entry.value.as_i64().ok_or(ArtError::InvalidReplayValue {
                field: "value",
                message: "expected i64 randomRIO value".to_string(),
            })
        }
        other => Err(type_mismatch("RandomRIOEntryT", &other)),
    }
}

fn replay_random_bytes(runtime: &mut ArtRuntime) -> Result<Vec<u8>, ArtError> {
    match runtime.pop_replay_entry()? {
        RecordingEntry::RandomBytes(entry) => {
            let encoded = entry
                .value
                .as_str()
                .ok_or_else(|| ArtError::InvalidReplayValue {
                    field: "value",
                    message: "expected base64 string".to_string(),
                })?;
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).map_err(
                |error| ArtError::InvalidReplayValue {
                    field: "value",
                    message: error.to_string(),
                },
            )
        }
        other => Err(type_mismatch("RandomBytesEntryT", &other)),
    }
}

fn type_mismatch(expected: &'static str, entry: &RecordingEntry) -> ArtError {
    ArtError::ReplayEntryTypeMismatch {
        expected,
        actual: entry_type(entry),
    }
}

fn entry_type(entry: &RecordingEntry) -> &'static str {
    match entry {
        RecordingEntry::Timestamp(_) => "TimeStampEntryT",
        RecordingEntry::Uuid(_) => "UuidEntryT",
        RecordingEntry::RandomRio(_) => "RandomRIOEntryT",
        RecordingEntry::RandomBytes(_) => "RandomBytesEntryT",
        RecordingEntry::CallApi(_) => "CallAPIEntryT",
        RecordingEntry::CallApiPii(_) => "CallAPIEntryPIIT",
        RecordingEntry::IncomingApi(_) => "IncomingApiEntryT",
    }
}
