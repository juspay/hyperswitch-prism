//! Lineage ID domain types for tracking request lineage across services

use std::collections::HashMap;

/// A domain type representing lineage IDs as key-value pairs(uses hashmap internally).
///
/// This type can deserialize only from URL-encoded format (e.g., "trace_id=123&span_id=456")
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineageIds<'a> {
    prefix: &'a str,
    inner: HashMap<String, String>,
}

impl<'a> LineageIds<'a> {
    /// Create a new LineageIds from prefix and URL-encoded string
    pub fn new(prefix: &'a str, url_encoded_string: &str) -> Result<Self, LineageParseError> {
        Ok(Self {
            prefix,
            inner: serde_urlencoded::from_str(url_encoded_string)
                .map_err(|e| LineageParseError::InvalidFormat(e.to_string()))?,
        })
    }

    /// Create a new empty LineageIds
    pub fn empty(prefix: &'a str) -> Self {
        Self {
            prefix,
            inner: HashMap::new(),
        }
    }

    /// Get the inner HashMap with prefixed keys
    pub fn inner(&self) -> HashMap<String, String> {
        self.inner
            .iter()
            .map(|(k, v)| (format!("{}{}", self.prefix, k), v.clone()))
            .collect()
    }

    /// Get the inner HashMap without prefix (raw keys)
    pub fn inner_raw(&self) -> &HashMap<String, String> {
        &self.inner
    }

    /// Convert to an owned LineageIds with 'static lifetime
    pub fn to_owned(&self) -> LineageIds<'static> {
        LineageIds {
            prefix: Box::leak(self.prefix.to_string().into_boxed_str()),
            inner: self.inner.clone(),
        }
    }
}

impl serde::Serialize for LineageIds<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let prefixed_map = self.inner();
        prefixed_map.serialize(serializer)
    }
}
/// Error type for lineage parsing operations
#[derive(Debug, thiserror::Error)]
pub enum LineageParseError {
    #[error("Invalid lineage header format: {0}")]
    InvalidFormat(String),
    #[error("URL decoding failed: {0}")]
    UrlDecoding(String),
}
