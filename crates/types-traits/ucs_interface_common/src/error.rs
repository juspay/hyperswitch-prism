use prost::Message;
use tonic::{Code, Status};

/// Shared error type for interface-level operations (header parsing, metadata extraction).
/// Each transport layer can convert this into its own error type.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    #[error("Missing required header: {key}")]
    MissingRequiredHeader { key: String },
    #[error("Invalid header value for '{key}': {reason}")]
    InvalidHeaderValue { key: String, reason: String },
}

impl From<InterfaceError> for Status {
    fn from(err: InterfaceError) -> Self {
        let integration_error = match err {
            InterfaceError::MissingRequiredHeader { key } => {
                grpc_api_types::payments::IntegrationError {
                    error_message: format!("Missing required header: {}", key),
                    error_code: "MISSING_REQUIRED_HEADER".to_string(),
                    suggested_action: Some(format!("Add the '{}' header", key)),
                    doc_url: None,
                }
            }
            InterfaceError::InvalidHeaderValue { key, reason } => {
                grpc_api_types::payments::IntegrationError {
                    error_message: format!("Invalid header value for '{}': {}", key, reason),
                    error_code: "INVALID_HEADER_VALUE".to_string(),
                    suggested_action: Some(format!("Fix the '{}' header value", key)),
                    doc_url: None,
                }
            }
        };

        let msg = integration_error.error_message.clone();
        let mut buf = Vec::new();
        if let Err(e) = integration_error.encode(&mut buf) {
            return Self::with_details(
                Code::Internal,
                format!("Failed to encode error: {}", e),
                Vec::new().into(),
            );
        }

        Self::with_details(Code::InvalidArgument, msg, buf.into())
    }
}
