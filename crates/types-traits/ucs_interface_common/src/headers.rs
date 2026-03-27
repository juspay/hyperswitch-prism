use common_utils::consts;
use common_utils::metadata::{HeaderMaskingConfig, MaskedMetadata};
use std::collections::HashMap;
use tonic::metadata::{Ascii, MetadataMap, MetadataValue};

use crate::error::InterfaceError;

/// Abstraction over different header container types.
/// Allows unified header-to-metadata conversion for FFI (`HashMap`),
/// HTTP (`http::HeaderMap`), and any future transport.
pub trait HeaderSource {
    fn get_header(&self, key: &str) -> Option<&str>;
}

impl HeaderSource for HashMap<String, String> {
    fn get_header(&self, key: &str) -> Option<&str> {
        self.get(key).map(|s| s.as_str())
    }
}

impl HeaderSource for http::HeaderMap {
    fn get_header(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.to_str().ok())
    }
}

/// All recognized headers. Validation and defaults are applied
/// downstream in metadata extraction, not at the transport layer.
const HEADERS: &[&str] = &[
    consts::X_CONNECTOR_NAME,
    consts::X_MERCHANT_ID,
    consts::X_REQUEST_ID,
    consts::X_TENANT_ID,
    consts::X_AUTH,
    consts::X_REFERENCE_ID,
    consts::X_API_KEY,
    consts::X_API_SECRET,
    consts::X_KEY1,
    consts::X_KEY2,
    consts::X_AUTH_KEY_MAP,
    consts::X_SHADOW_MODE,
    consts::X_CONNECTOR_CONFIG,
    consts::X_RESOURCE_ID,
    consts::X_ENVIRONMENT,
];

fn to_metadata_value(key: &str, value: &str) -> Result<MetadataValue<Ascii>, InterfaceError> {
    MetadataValue::try_from(value).map_err(|e| InterfaceError::InvalidHeaderValue {
        key: key.to_string(),
        reason: e.to_string(),
    })
}

/// Converts headers from any `HeaderSource` into a gRPC `MetadataMap`.
/// All headers are optional at this layer; downstream metadata extraction
/// applies context-aware defaults for any missing values.
pub fn headers_to_metadata<H: HeaderSource>(headers: &H) -> Result<MetadataMap, InterfaceError> {
    let mut metadata = MetadataMap::new();

    for header_name in HEADERS {
        if let Some(value) = headers.get_header(header_name) {
            let metadata_value = to_metadata_value(header_name, value)?;
            metadata.insert(*header_name, metadata_value);
        }
    }

    Ok(metadata)
}

/// Converts headers from any `HeaderSource` into a `MaskedMetadata`,
/// which wraps a `MetadataMap` with masking configuration for sensitive values.
pub fn headers_to_masked_metadata<H: HeaderSource>(
    headers: &H,
    masking_config: HeaderMaskingConfig,
) -> Result<MaskedMetadata, InterfaceError> {
    let metadata = headers_to_metadata(headers)?;
    Ok(MaskedMetadata::new(metadata, masking_config))
}
