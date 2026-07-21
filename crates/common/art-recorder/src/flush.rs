use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine};
use cbc::Encryptor;
use cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use thiserror::Error;

use crate::{
    runtime::ArtRuntime,
    schema::{CsvRecording, RecordingEntry},
};

type Aes256CbcEncryptor = Encryptor<Aes256>;

const AES_256_CBC_PREFIX: &str = "aes256cbc:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecEntryTransform<'a> {
    Plain,
    Aes256Cbc { key: &'a str, iv: &'a str },
}

pub fn recording_rows_from_runtime(
    runtime: &ArtRuntime,
    order_id: Option<&str>,
    transform: RecEntryTransform<'_>,
) -> Result<Vec<CsvRecording>, FlushError> {
    let Some(session) = runtime.session() else {
        return Ok(Vec::new());
    };

    runtime
        .recorded_entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            Ok(CsvRecording {
                sess_id: session.session_id().to_string(),
                merch_id: session.merchant_id.clone(),
                ord_id: order_id.unwrap_or_default().to_string(),
                counter: i32::try_from(index + 1).map_err(|_| FlushError::CounterOverflow)?,
                val_type: recording_entry_val_type(entry).to_string(),
                rec_entry: transform_rec_entry(entry, transform)?,
            })
        })
        .collect()
}

fn transform_rec_entry(
    entry: &RecordingEntry,
    transform: RecEntryTransform<'_>,
) -> Result<String, FlushError> {
    let rec_entry =
        serde_json::to_string(entry).map_err(|source| FlushError::SerializeEntry { source })?;

    match transform {
        RecEntryTransform::Plain => Ok(BASE64_ENGINE.encode(rec_entry.as_bytes())),
        RecEntryTransform::Aes256Cbc { key, iv } => {
            encrypt_aes_256_cbc(&rec_entry, key, iv).map(|encrypted| {
                let mut prefixed =
                    String::with_capacity(AES_256_CBC_PREFIX.len() + encrypted.len());
                prefixed.push_str(AES_256_CBC_PREFIX);
                prefixed.push_str(&encrypted);
                prefixed
            })
        }
    }
}

fn encrypt_aes_256_cbc(plaintext: &str, key: &str, iv: &str) -> Result<String, FlushError> {
    let key_bytes = key.as_bytes();
    let iv_bytes = iv.as_bytes();

    if key_bytes.len() != 32 {
        return Err(FlushError::InvalidEncryptionConfig {
            field: "aes_key",
            expected: "32 bytes for AES-256-CBC",
            actual: key_bytes.len(),
        });
    }

    if iv_bytes.len() != 16 {
        return Err(FlushError::InvalidEncryptionConfig {
            field: "aes_iv",
            expected: "16 bytes for AES-256-CBC",
            actual: iv_bytes.len(),
        });
    }

    let encryptor = Aes256CbcEncryptor::new_from_slices(key_bytes, iv_bytes).map_err(|_| {
        FlushError::Encryption {
            message: "failed to initialize AES-256-CBC encryptor".to_string(),
        }
    })?;
    let mut buffer = plaintext.as_bytes().to_vec();
    let plaintext_len = buffer.len();
    buffer.resize(plaintext_len + 16, 0);

    let ciphertext = encryptor
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext_len)
        .map_err(|_| FlushError::Encryption {
            message: "failed to encrypt recEntry".to_string(),
        })?;

    Ok(BASE64_ENGINE.encode(ciphertext))
}

fn recording_entry_val_type(entry: &RecordingEntry) -> &'static str {
    match entry {
        RecordingEntry::Metadata(_) => "METADATA",
        RecordingEntry::Timestamp(_) => "TIMESTAMP",
        RecordingEntry::Uuid(_) => "UUID",
        RecordingEntry::RandomRio(_) => "RANDOM_RIO",
        RecordingEntry::RandomBytes(_) => "RANDOM_BYTES",
        RecordingEntry::CallApi(_) | RecordingEntry::CallApiPii(_) => "OUTGOING_API",
        RecordingEntry::IncomingApi(_) => "INCOMING_API",
    }
}

#[derive(Debug, Error)]
pub enum FlushError {
    #[error("failed to serialize ART recording entry")]
    SerializeEntry { source: serde_json::Error },
    #[error("ART recording counter overflowed i32")]
    CounterOverflow,
    #[error("invalid ART recording encryption config for {field}: expected {expected}, got {actual} bytes")]
    InvalidEncryptionConfig {
        field: &'static str,
        expected: &'static str,
        actual: usize,
    },
    #[error("failed to encrypt ART recording entry: {message}")]
    Encryption { message: String },
}
