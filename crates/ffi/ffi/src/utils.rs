use common_utils::metadata::{HeaderMaskingConfig, MaskedMetadata};
use domain_types::errors::ConnectorError;
use grpc_api_types::payments::IntegrationError;
use std::collections::HashMap;
use std::sync::Arc;
use ucs_env::configs::Config;

/// Converts FFI headers (HashMap) to gRPC metadata with masking support.
/// Delegates to the shared `headers_to_masked_metadata` implementation.
pub fn ffi_headers_to_masked_metadata(
    headers: &HashMap<String, String>,
) -> Result<MaskedMetadata, IntegrationError> {
    ucs_interface_common::headers::headers_to_masked_metadata(
        headers,
        HeaderMaskingConfig::default(),
    )
    .map_err(|e| match e {
        ucs_interface_common::error::InterfaceError::MissingRequiredHeader { key } => {
            IntegrationError {
                error_message: format!("Missing required header: {}", key),
                error_code: "MISSING_REQUIRED_HEADER".to_string(),
                suggested_action: None,
                doc_url: None,
            }
        }
        ucs_interface_common::error::InterfaceError::InvalidHeaderValue { key, reason } => {
            IntegrationError {
                error_message: format!("{}: {}", key, reason),
                error_code: "INVALID_HEADER_VALUE".to_string(),
                suggested_action: None,
                doc_url: None,
            }
        }
    })
}

/// Load development config from the embedded config string.
/// This avoids runtime path lookup by embedding the config at build time.
pub fn load_config(embedded_config: &str) -> Result<Arc<Config>, ConnectorError> {
    toml::from_str(embedded_config)
        .map(Arc::new)
        .map_err(|e| ConnectorError::GenericError {
            error_message: e.to_string(),
            error_object: serde_json::Value::Null,
        })
}
