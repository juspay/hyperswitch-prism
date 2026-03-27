use std::collections::HashSet;

use crate::config_patch::Patch;
use bytes::Bytes;
use hyperswitch_masking::{Maskable, Secret};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};

/// Configuration for header masking in gRPC metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderMaskingConfig {
    unmasked_keys: HashSet<String>,
}

impl HeaderMaskingConfig {
    pub fn new(unmasked_keys: HashSet<String>) -> Self {
        Self { unmasked_keys }
    }

    pub fn should_unmask(&self, key: &str) -> bool {
        self.unmasked_keys.contains(&key.to_lowercase())
    }
}

impl Serialize for HeaderMaskingConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("HeaderMaskingConfig", 1)?;
        let keys: Vec<String> = self.unmasked_keys.iter().cloned().collect();
        state.serialize_field("keys", &keys)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for HeaderMaskingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Config {
            keys: Vec<String>,
        }

        Config::deserialize(deserializer).map(|config| Self {
            unmasked_keys: config
                .keys
                .into_iter()
                .map(|key| key.to_lowercase())
                .collect(),
        })
    }
}

impl Default for HeaderMaskingConfig {
    fn default() -> Self {
        Self {
            unmasked_keys: ["content-type", "content-length", "user-agent"]
                .iter()
                .map(|&key| key.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HeaderMaskingConfigPatch {
    #[serde(rename = "keys")]
    pub keys: Option<Vec<String>>,
}

impl Patch<HeaderMaskingConfigPatch> for HeaderMaskingConfig {
    fn apply(&mut self, patch: HeaderMaskingConfigPatch) {
        if let Some(keys) = patch.keys {
            let set: HashSet<String> = keys.into_iter().map(|key| key.to_lowercase()).collect();
            *self = Self::new(set);
        }
    }
}

/// Secure wrapper for gRPC metadata with configurable masking.
/// ASCII headers:
/// - get(key) -> Secret<String> - Forces explicit .expose() call
/// - get_raw(key) -> String - Raw access
/// - get_maskable(key) -> Maskable<String> - For logging/observability
///
/// Binary headers:
/// - get_bin(key) -> Secret<Bytes> - Forces explicit .expose() call
/// - get_bin_raw(key) -> Bytes - Raw access
/// - get_bin_maskable(key) -> Maskable<String> - Base64 encoded for logging
/// - get_all_masked() -> HashMap<String, String> - Safe for logging
#[derive(Clone)]
pub struct MaskedMetadata {
    raw_metadata: tonic::metadata::MetadataMap,
    masking_config: HeaderMaskingConfig,
}

impl std::fmt::Debug for MaskedMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaskedMetadata")
            .field("masked_headers", &self.get_all_masked())
            .field("masking_config", &self.masking_config)
            .finish()
    }
}

impl Default for MaskedMetadata {
    fn default() -> Self {
        Self {
            raw_metadata: tonic::metadata::MetadataMap::new(),
            masking_config: HeaderMaskingConfig::default(),
        }
    }
}

impl MaskedMetadata {
    pub fn new(
        raw_metadata: tonic::metadata::MetadataMap,
        masking_config: HeaderMaskingConfig,
    ) -> Self {
        Self {
            raw_metadata,
            masking_config,
        }
    }

    /// Always returns Secret - business logic must call .expose() explicitly
    pub fn get(&self, key: &str) -> Option<Secret<String>> {
        self.raw_metadata
            .get(key)
            .and_then(|value| value.to_str().ok())
            .map(|s| Secret::new(s.to_string()))
    }

    /// Returns raw string value regardless of config
    pub fn get_raw(&self, key: &str) -> Option<String> {
        self.raw_metadata
            .get(key)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Returns Maskable with enum variants for logging (masked/unmasked)
    pub fn get_maskable(&self, key: &str) -> Option<Maskable<String>> {
        self.raw_metadata
            .get(key)
            .and_then(|value| value.to_str().ok())
            .map(|s| {
                if self.masking_config.should_unmask(key) {
                    Maskable::new_normal(s.to_string())
                } else {
                    Maskable::new_masked(Secret::new(s.to_string()))
                }
            })
    }

    /// Always returns Secret<Bytes> - business logic must call .expose() explicitly
    pub fn get_bin(&self, key: &str) -> Option<Secret<Bytes>> {
        self.raw_metadata
            .get_bin(key)
            .and_then(|value| value.to_bytes().ok())
            .map(Secret::new)
    }

    /// Returns raw Bytes value regardless of config
    pub fn get_bin_raw(&self, key: &str) -> Option<Bytes> {
        self.raw_metadata
            .get_bin(key)
            .and_then(|value| value.to_bytes().ok())
    }

    /// Returns Maskable<String> with base64 encoding for binary headers
    pub fn get_bin_maskable(&self, key: &str) -> Option<Maskable<String>> {
        self.raw_metadata.get_bin(key).map(|value| {
            let encoded = String::from_utf8_lossy(value.as_encoded_bytes()).to_string();
            if self.masking_config.should_unmask(key) {
                Maskable::new_normal(encoded)
            } else {
                Maskable::new_masked(Secret::new(encoded))
            }
        })
    }

    /// Get all metadata as HashMap with masking for logging
    pub fn get_all_masked(&self) -> std::collections::HashMap<String, String> {
        self.raw_metadata
            .iter()
            .filter_map(|entry| {
                let key_name = match entry {
                    tonic::metadata::KeyAndValueRef::Ascii(key, _) => key.as_str(),
                    tonic::metadata::KeyAndValueRef::Binary(key, _) => key.as_str(),
                };

                let masked_value = match entry {
                    tonic::metadata::KeyAndValueRef::Ascii(_, _) => self
                        .get_maskable(key_name)
                        .map(|maskable| format!("{maskable:?}")),
                    tonic::metadata::KeyAndValueRef::Binary(_, _) => self
                        .get_bin_maskable(key_name)
                        .map(|maskable| format!("{maskable:?}")),
                };

                masked_value.map(|value| (key_name.to_string(), value))
            })
            .collect()
    }
}

/// Return the merchant ID if present, or generate a default.
///
/// Shared fallback logic used by both the gRPC path (raw `MetadataMap`)
/// and the FFI path (`MaskedMetadata`).
pub fn merchant_id_or_default(value: Option<&str>) -> String {
    value.map(|s| s.to_string()).unwrap_or_else(|| {
        tracing::warn!("x-merchant-id header missing, using default merchant ID");
        "DefaultMerchantId".to_string()
    })
}
