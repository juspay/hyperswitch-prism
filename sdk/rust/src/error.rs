/// Top-level error type for all SDK operations.
///
/// Covers every flow — payments, refunds, payouts, webhooks, etc.
///
/// ## Variants
///
/// - [`SdkError::IntegrationError`] — request rejected **before** any HTTP call was made.
///   Caused by missing/invalid fields, bad auth config, or unsupported combinations.
///   The connector was never contacted.
///
/// - [`SdkError::ConnectorError`] — request was sent and the connector returned an
///   error (4xx / 5xx), or the SDK could not parse the connector's response.
///
/// - [`SdkError::NetworkError`] — transport-layer failure (timeout, connection refused,
///   TLS error, etc.). The connector's decision is unknown.
///
/// ## Pattern matching
///
/// This enum is `#[non_exhaustive]` so new variants can be added in future versions
/// without breaking existing code. Always include a `_ =>` fallback arm:
///
/// ```rust
/// use hyperswitch_payments_client::SdkError;
///
/// match err {
///     SdkError::IntegrationError { error_code, error_message, .. } => {
///         // Bad request — fix your inputs, never retryable
///     }
///     SdkError::ConnectorError { error_code, http_status_code, .. } => {
///         // Connector declined or errored — check http_status_code
///     }
///     SdkError::NetworkError { error_code, .. } => {
///         // Transport failure — may be retryable
///     }
///     _ => { /* forward-compatibility fallback */ }
/// }
/// ```
#[non_exhaustive]
#[derive(Debug)]
pub enum SdkError {
    /// Request was rejected before any HTTP call was made.
    ///
    /// Equivalent to `IntegrationError` in the Java / Python / TypeScript SDKs.
    IntegrationError {
        /// Machine-readable code (e.g. `"MISSING_REQUIRED_FIELD"`).
        error_code: String,
        /// Human-readable description with full context.
        error_message: String,
        /// Actionable guidance for developers, if available.
        suggested_action: Option<String>,
        /// Documentation URL for reference, if available.
        doc_url: Option<String>,
    },

    /// The connector returned a 4xx/5xx response, or the SDK could not parse
    /// the connector's response.
    ///
    /// Equivalent to `ConnectorError` in the Java / Python / TypeScript SDKs.
    ConnectorError {
        /// Machine-readable code (e.g. `"CONNECTOR_ERROR_RESPONSE"`).
        error_code: String,
        /// Human-readable description.
        error_message: String,
        /// HTTP status code returned by the connector, if known.
        http_status_code: Option<u16>,
    },

    /// Transport-layer failure. The connector was never reached, or the
    /// connection was dropped before a full response was received.
    NetworkError {
        /// Machine-readable code (e.g. `"CONNECT_TIMEOUT_EXCEEDED"`).
        error_code: String,
        /// Human-readable description.
        error_message: String,
    },
}

impl std::fmt::Display for SdkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SdkError::IntegrationError {
                error_code,
                error_message,
                ..
            } => write!(f, "IntegrationError({error_code}): {error_message}"),
            SdkError::ConnectorError {
                error_code,
                error_message,
                http_status_code,
            } => match http_status_code {
                Some(status) => write!(
                    f,
                    "ConnectorError({error_code}, http={status}): {error_message}"
                ),
                None => write!(f, "ConnectorError({error_code}): {error_message}"),
            },
            SdkError::NetworkError {
                error_code,
                error_message,
            } => write!(f, "NetworkError({error_code}): {error_message}"),
        }
    }
}

impl std::error::Error for SdkError {}

// ── Conversions from inner error types ────────────────────────────────────────

impl From<grpc_api_types::payments::IntegrationError> for SdkError {
    fn from(e: grpc_api_types::payments::IntegrationError) -> Self {
        SdkError::IntegrationError {
            error_code: e.error_code,
            error_message: e.error_message,
            suggested_action: e.suggested_action,
            doc_url: e.doc_url,
        }
    }
}

impl From<grpc_api_types::payments::ConnectorError> for SdkError {
    fn from(e: grpc_api_types::payments::ConnectorError) -> Self {
        SdkError::ConnectorError {
            error_code: e.error_code,
            error_message: e.error_message,
            http_status_code: e.http_status_code.map(|s| s as u16),
        }
    }
}

impl From<Box<grpc_api_types::payments::ConnectorError>> for SdkError {
    fn from(e: Box<grpc_api_types::payments::ConnectorError>) -> Self {
        SdkError::from(*e)
    }
}

impl From<crate::http_client::NetworkError> for SdkError {
    fn from(e: crate::http_client::NetworkError) -> Self {
        SdkError::NetworkError {
            error_code: e.error_code().to_string(),
            error_message: e.to_string(),
        }
    }
}
