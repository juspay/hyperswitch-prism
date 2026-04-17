use common_enums::KafkaClientError;
use domain_types::errors::{
    ApiClientError, ConnectorError, ConnectorFlowError, IntegrationError, WebhookError,
};
use tonic::Status;

use crate::logger;

/// Allows [error_stack::Report] to change between error contexts
/// using the dependent [ErrorSwitch] trait to define relations & mappings between traits
pub trait ReportSwitchExt<T, U> {
    /// Switch to the intended report by calling switch
    /// requires error switch to be already implemented on the error type
    fn switch(self) -> Result<T, error_stack::Report<U>>;
}

impl<T, U, V> ReportSwitchExt<T, U> for Result<T, error_stack::Report<V>>
where
    V: ErrorSwitch<U> + error_stack::Context,
    U: error_stack::Context,
{
    #[track_caller]
    fn switch(self) -> Result<T, error_stack::Report<U>> {
        match self {
            Ok(i) => Ok(i),
            Err(er) => {
                let new_c = er.current_context().switch();
                Err(er.change_context(new_c))
            }
        }
    }
}

/// Allow [error_stack::Report] to convert between error types
/// This auto-implements [ReportSwitchExt] for the corresponding errors
pub trait ErrorSwitch<T> {
    /// Get the next error type that the source error can be escalated into
    /// This does not consume the source error since we need to keep it in context
    fn switch(&self) -> T;
}

/// Allow [error_stack::Report] to convert between error types
/// This serves as an alternative to [ErrorSwitch]
pub trait ErrorSwitchFrom<T> {
    /// Convert to an error type that the source can be escalated into
    /// This does not consume the source error since we need to keep it in context
    fn switch_from(error: &T) -> Self;
}

impl<T, S> ErrorSwitch<T> for S
where
    T: ErrorSwitchFrom<Self>,
{
    fn switch(&self) -> T {
        T::switch_from(self)
    }
}

pub trait IntoGrpcStatus {
    fn into_grpc_status(self) -> Status;
}

pub trait ResultExtGrpc<T> {
    #[allow(clippy::result_large_err)]
    fn into_grpc_status(self) -> Result<T, Status>;
}

impl<T, E> ResultExtGrpc<T> for error_stack::Result<T, E>
where
    error_stack::Report<E>: IntoGrpcStatus,
{
    fn into_grpc_status(self) -> Result<T, Status> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => Err(err.into_grpc_status()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("Invalid host for socket: {0}")]
    AddressError(#[from] std::net::AddrParseError),
    #[error("Failed while building grpc reflection service: {0}")]
    GrpcReflectionServiceError(#[from] tonic_reflection::server::Error),
    #[error("Error while creating metrics server")]
    MetricsServerError,
    #[error("Error while creating the server: {0}")]
    ServerError(#[from] tonic::transport::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Direct gRPC status mapping for `IntegrationError` (request phase).
///
/// `invalid_argument` — caller sent a missing or invalid field in this request (UCS is stateless;
///   every required ID/field must be supplied by the caller on every call).
/// `failed_precondition` — connector/merchant configuration problem; not a client credential failure.
/// `unauthenticated` — credential / auth resolution failure.
/// `internal` — UCS machinery failure (encoding, URL building, serialization); caller cannot fix.
impl IntoGrpcStatus for error_stack::Report<IntegrationError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let msg = self.current_context().to_string();
        match self.current_context() {
            IntegrationError::MissingRequiredField { .. }
            | IntegrationError::MissingRequiredFields { .. }
            | IntegrationError::InvalidDataFormat { .. }
            | IntegrationError::MismatchedPaymentData { .. }
            | IntegrationError::InvalidWallet { .. }
            | IntegrationError::InvalidWalletToken { .. }
            | IntegrationError::MissingPaymentMethodType { .. }
            | IntegrationError::CurrencyNotSupported { .. }
            | IntegrationError::AmountConversionFailed { .. }
            | IntegrationError::MandatePaymentDataMismatch { .. }
            | IntegrationError::MissingApplePayTokenData { .. }
            // UCS is stateless — the caller must supply these IDs on every request.
            | IntegrationError::MissingConnectorTransactionID { .. }
            | IntegrationError::MissingConnectorRefundID { .. }
            | IntegrationError::MissingConnectorMandateID { .. }
            | IntegrationError::MissingConnectorMandateMetadata { .. }
            | IntegrationError::MissingConnectorRelatedTransactionID { .. }
            // Caller supplied a field value that exceeds the connector's length limit.
            | IntegrationError::MaxFieldLengthViolated { .. } => Status::invalid_argument(msg),
            IntegrationError::FlowNotSupported { .. }
            | IntegrationError::NotSupported { .. }
            | IntegrationError::CaptureMethodNotSupported { .. }
            | IntegrationError::NotImplemented(..)
            | IntegrationError::InvalidConnectorConfig { .. }
            | IntegrationError::ConfigurationError { .. }
            | IntegrationError::NoConnectorMetaData { .. } => Status::failed_precondition(msg),
            IntegrationError::FailedToObtainAuthType { .. } => Status::unauthenticated(msg),
            IntegrationError::SourceVerificationFailed { .. } => Status::unauthenticated(msg),
            IntegrationError::FailedToObtainIntegrationUrl { .. }
            | IntegrationError::RequestEncodingFailed { .. }
            | IntegrationError::HeaderMapConstructionFailed { .. }
            | IntegrationError::BodySerializationFailed { .. }
            | IntegrationError::UrlParsingFailed { .. }
            | IntegrationError::UrlEncodingFailed { .. } => Status::internal(msg),
        }
    }
}

/// Direct gRPC status mapping for `ConnectorError` (response phase).
///
/// - `ConnectorErrorResponse`: connector returned a 4xx/5xx; mapped per HTTP status code
///   following the standard HTTP → gRPC status code translation.
/// - All UCS-side transformation failures → `internal` (UCS machinery failed).
impl IntoGrpcStatus for error_stack::Report<ConnectorError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let msg = self.current_context().to_string();
        match self.current_context() {
            ConnectorError::ConnectorErrorResponse(error_response) => {
                match error_response.status_code {
                    400 => Status::invalid_argument(msg),
                    401 => Status::unauthenticated(msg),
                    403 => Status::permission_denied(msg),
                    404 => Status::not_found(msg),
                    429 => Status::resource_exhausted(msg),
                    500 => Status::internal(msg),
                    501 => Status::unimplemented(msg),
                    503 => Status::unavailable(msg),
                    504 => Status::deadline_exceeded(msg),
                    _ => Status::unknown(msg),
                }
            }
            ConnectorError::ResponseDeserializationFailed { .. }
            | ConnectorError::ResponseHandlingFailed { .. }
            | ConnectorError::UnexpectedResponseError { .. }
            | ConnectorError::IntegrityCheckFailed { .. } => Status::internal(msg),
        }
    }
}

/// Direct gRPC status mapping for `ApiClientError` (network/transport phase).
impl IntoGrpcStatus for error_stack::Report<ApiClientError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let msg = self.current_context().to_string();
        match self.current_context() {
            ApiClientError::RequestTimeoutReceived | ApiClientError::GatewayTimeoutReceived => {
                Status::deadline_exceeded(msg)
            }
            ApiClientError::ServiceUnavailableReceived => Status::unavailable(msg),
            _ => Status::internal(msg),
        }
    }
}

impl IntoGrpcStatus for error_stack::Report<KafkaClientError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let msg = self.current_context().to_string();
        Status::internal(msg)
    }
}

/// Direct gRPC status mapping for `ConnectorFlowError` (unified gRPC path wrapper).
impl IntoGrpcStatus for error_stack::Report<ConnectorFlowError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            ConnectorFlowError::Request(e) => {
                error_stack::Report::new(e.clone()).into_grpc_status()
            }
            ConnectorFlowError::Client(e) => error_stack::Report::new(e.clone()).into_grpc_status(),
            ConnectorFlowError::KafkaClient(e) => {
                error_stack::Report::new(e.clone()).into_grpc_status()
            }
            ConnectorFlowError::Response(e) => {
                error_stack::Report::new(e.clone()).into_grpc_status()
            }
        }
    }
}

/// Direct gRPC status mapping for `WebhookError`.
impl IntoGrpcStatus for error_stack::Report<WebhookError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let msg = self.current_context().to_string();
        match self.current_context() {
            WebhookError::WebhooksNotImplemented { .. } => Status::unimplemented(msg),
            WebhookError::WebhookEventTypeNotFound
            | WebhookError::WebhookSignatureNotFound
            | WebhookError::WebhookReferenceIdNotFound
            | WebhookError::WebhookResourceObjectNotFound
            | WebhookError::WebhookVerificationSecretNotFound => Status::not_found(msg),
            // Bad body from the webhook sender — genuinely bad argument.
            WebhookError::WebhookBodyDecodingFailed => Status::invalid_argument(msg),
            // Signature mismatch or configured secret is wrong — authentication failure.
            WebhookError::WebhookSourceVerificationFailed
            | WebhookError::WebhookVerificationSecretInvalid => Status::unauthenticated(msg),
            WebhookError::WebhookProcessingFailed
            | WebhookError::WebhookAmountConversionFailed { .. }
            | WebhookError::WebhookResponseEncodingFailed => Status::internal(msg),
        }
    }
}
