/// Shared error type for interface-level operations (header parsing, metadata extraction).
/// Each transport layer can convert this into its own error type.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    #[error("Missing required header: {key}")]
    MissingRequiredHeader { key: String },
    #[error("Invalid header value for '{key}': {reason}")]
    InvalidHeaderValue { key: String, reason: String },
}

impl From<InterfaceError> for tonic::Status {
    fn from(err: InterfaceError) -> Self {
        Self::invalid_argument(err.to_string())
    }
}
