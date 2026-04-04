use domain_types::errors::{
    ApiClientError, ConnectorFlowError, ConnectorResponseTransformationError, IntegrationError,
    WebhookError,
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
/// Missing/invalid input → invalid_argument (400)
/// Flow/feature unsupported → failed_precondition (400)
/// Auth/config failures → unauthenticated or internal (401/500)
/// Internal build/encode failures → internal (500)
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
            | IntegrationError::MissingApplePayTokenData { .. } => Status::invalid_argument(msg),
            IntegrationError::FlowNotSupported { .. }
            | IntegrationError::NotSupported { .. }
            | IntegrationError::CaptureMethodNotSupported { .. }
            | IntegrationError::NotImplemented(..) => Status::failed_precondition(msg),
            IntegrationError::FailedToObtainAuthType { .. }
            | IntegrationError::InvalidConnectorConfig { .. }
            | IntegrationError::ConfigurationError { .. }
            | IntegrationError::NoConnectorMetaData { .. } => Status::unauthenticated(msg),
            IntegrationError::SourceVerificationFailed { .. } => Status::unauthenticated(msg),
            IntegrationError::MissingConnectorTransactionID { .. }
            | IntegrationError::MissingConnectorRefundID { .. }
            | IntegrationError::MissingConnectorMandateID { .. }
            | IntegrationError::MissingConnectorMandateMetadata { .. }
            | IntegrationError::MissingConnectorRelatedTransactionID { .. }
            | IntegrationError::MaxFieldLengthViolated { .. }
            | IntegrationError::FailedToObtainIntegrationUrl { .. }
            | IntegrationError::RequestEncodingFailed { .. }
            | IntegrationError::HeaderMapConstructionFailed { .. }
            | IntegrationError::BodySerializationFailed { .. }
            | IntegrationError::UrlParsingFailed { .. }
            | IntegrationError::UrlEncodingFailed { .. } => Status::internal(msg),
        }
    }
}

/// Direct gRPC status mapping for `ConnectorResponseTransformationError` (response phase).
/// All response-phase failures are internal errors — the request reached the connector
/// but we failed to process its response.
impl IntoGrpcStatus for error_stack::Report<ConnectorResponseTransformationError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        Status::internal(self.current_context().to_string())
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

/// Direct gRPC status mapping for `ConnectorFlowError` (unified gRPC path wrapper).
impl IntoGrpcStatus for error_stack::Report<ConnectorFlowError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            ConnectorFlowError::Request(e) => {
                error_stack::Report::new(e.clone()).into_grpc_status()
            }
            ConnectorFlowError::Client(e) => error_stack::Report::new(e.clone()).into_grpc_status(),
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
            WebhookError::WebhookBodyDecodingFailed
            | WebhookError::WebhookSourceVerificationFailed
            | WebhookError::WebhookVerificationSecretInvalid => Status::invalid_argument(msg),
            WebhookError::WebhookProcessingFailed
            | WebhookError::WebhookAmountConversionFailed { .. }
            | WebhookError::WebhookResponseEncodingFailed => Status::internal(msg),
        }
    }
}
