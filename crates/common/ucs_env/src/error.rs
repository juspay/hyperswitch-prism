use common_enums::KafkaClientError;
use common_utils::errors::ErrorSwitch;
use domain_types::errors::{
    ApiClientError, ConnectorError, ConnectorFlowError, IntegrationError, WebhookError,
};
use error_stack::Report;
use tonic::Status;

use crate::logger;
use prost::Message;

pub trait IntoGrpcStatus {
    fn into_grpc_status(self) -> Status;
}

pub trait ResultExtGrpc<T> {
    #[allow(clippy::result_large_err)]
    fn into_grpc_status(self) -> Result<T, Status>;
}

impl<T, E> ResultExtGrpc<T> for error_stack::Result<T, E>
where
    Report<E>: IntoGrpcStatus,
{
    fn into_grpc_status(self) -> Result<T, Status> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => Err(err.into_grpc_status()),
        }
    }
}

/// Failures in UCS's own gRPC plumbing, raised before any transformation runs.
#[derive(Debug, Clone, PartialEq, thiserror::Error, strum::AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalError {
    #[error("Extensions missing from gRPC request")]
    MissingRequestExtensions,
    #[error("Configuration not found in request extensions")]
    ConfigNotFound,
    #[error("Test mode configuration error: {reason}")]
    TestContextCreationFailed { reason: String },
}

/// Every error that can reach the gRPC boundary; one variant per leaf error type.
///
/// Handlers return `Report<GrpcError>` and never build a `tonic::Status`. There is no
/// `From<tonic::Status>` for this type, and there must not be one: without it a hand-built status
/// does not compile, so every error reaches the logging wrapper with its stack intact.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GrpcError {
    #[error("Integration error: {0}")]
    Integration(#[from] IntegrationError),
    #[error("Connector error: {0}")]
    Connector(#[from] ConnectorError),
    #[error("Client error: {0}")]
    ApiClient(#[from] ApiClientError),
    #[error("Kafka client error: {0}")]
    KafkaClient(#[from] KafkaClientError),
    #[error("Webhook error: {0}")]
    Webhook(#[from] WebhookError),
    #[error("Internal error: {0}")]
    Internal(#[from] InternalError),
}

impl GrpcError {
    /// Machine-readable code of the wrapped error; alerts group on this.
    pub fn error_code(&self) -> &str {
        match self {
            Self::Integration(e) => e.error_code(),
            Self::Connector(e) => e.error_code(),
            Self::ApiClient(e) => e.as_ref(),
            Self::KafkaClient(e) => e.as_ref(),
            Self::Webhook(e) => e.as_ref(),
            Self::Internal(e) => e.as_ref(),
        }
    }
}

impl From<ConnectorFlowError> for GrpcError {
    fn from(value: ConnectorFlowError) -> Self {
        match value {
            ConnectorFlowError::Request(e) => Self::Integration(e),
            ConnectorFlowError::Client(e) => Self::ApiClient(e),
            ConnectorFlowError::KafkaClient(e) => Self::KafkaClient(e),
            ConnectorFlowError::Response(e) => Self::Connector(e),
        }
    }
}

/// Lift a leaf report to the gRPC boundary, preserving the frame chain.
pub trait ReportExtGrpcError {
    fn to_grpc_error(self) -> Report<GrpcError>;
}

impl<E> ReportExtGrpcError for Report<E>
where
    E: Clone + error_stack::Context,
    GrpcError: From<E>,
{
    fn to_grpc_error(self) -> Report<GrpcError> {
        let context = GrpcError::from(self.current_context().clone());
        self.change_context(context)
    }
}

pub trait ResultExtGrpcError<T> {
    fn to_grpc_error(self) -> Result<T, Report<GrpcError>>;
}

impl<T, E> ResultExtGrpcError<T> for error_stack::Result<T, E>
where
    E: Clone + error_stack::Context,
    GrpcError: From<E>,
{
    fn to_grpc_error(self) -> Result<T, Report<GrpcError>> {
        self.map_err(ReportExtGrpcError::to_grpc_error)
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

/// Pure error-to-status mapping.
///
/// Private on purpose: the only public path to a `Status` is `IntoGrpcStatus`, which logs the
/// report first. A public non-logging mapper would let callers produce a `Status` that never
/// reaches the alerting pipeline.
trait ToStatus {
    fn to_status(&self) -> Status;
}

/// `invalid_argument` — caller sent a missing or invalid field in this request (UCS is stateless;
///   every required ID/field must be supplied by the caller on every call).
/// `unimplemented` — the flow or payment method is not implemented, or not enabled for this connector.
/// `failed_precondition` — connector/merchant configuration problem; not a client credential failure.
/// `unauthenticated` — credential / auth resolution failure.
/// `internal` — UCS machinery failure (encoding, URL building, serialization); caller cannot fix.
impl ToStatus for IntegrationError {
    fn to_status(&self) -> Status {
        let integration_error: grpc_api_types::payments::IntegrationError =
            ErrorSwitch::switch(self);
        let msg = integration_error.error_message.clone();

        // Serialize the IntegrationError proto to bytes
        let mut buf = Vec::new();
        // SAFETY: IntegrationError only contains String fields with valid UTF-8
        // and prost encoding cannot fail for these controlled types
        let _ = integration_error.encode(&mut buf);

        match self {
            Self::MissingRequiredField { .. }
            | Self::MissingRequiredFields { .. }
            | Self::InvalidDataFormat { .. }
            | Self::MismatchedPaymentData { .. }
            | Self::InvalidWallet { .. }
            | Self::InvalidWalletToken { .. }
            | Self::MissingPaymentMethodType { .. }
            | Self::CurrencyNotSupported { .. }
            | Self::AmountConversionFailed { .. }
            | Self::MandatePaymentDataMismatch { .. }
            | Self::MissingApplePayTokenData { .. }
            // UCS is stateless — the caller must supply these IDs on every request.
            | Self::MissingConnectorTransactionID { .. }
            | Self::MissingConnectorRefundID { .. }
            | Self::MissingConnectorMandateID { .. }
            | Self::MissingConnectorMandateMetadata { .. }
            | Self::MissingConnectorRelatedTransactionID { .. }
            // Caller supplied a field value that exceeds the connector's length limit.
            | Self::MaxFieldLengthViolated { .. } => Status::with_details(tonic::Code::InvalidArgument, msg, buf.into()),
            Self::FlowNotSupported { .. }
            | Self::NotSupported { .. }
            | Self::CaptureMethodNotSupported { .. }
            | Self::NotImplemented(..) => Status::with_details(tonic::Code::Unimplemented, msg, buf.into()),
            Self::InvalidConnectorConfig { .. }
            | Self::ConfigurationError { .. }
            | Self::NoConnectorMetaData { .. } => Status::with_details(tonic::Code::FailedPrecondition, msg, buf.into()),
            Self::FailedToObtainAuthType { .. } => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
            Self::SourceVerificationFailed { .. } => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
            Self::FailedToObtainIntegrationUrl { .. }
            | Self::RequestEncodingFailed { .. }
            | Self::HeaderMapConstructionFailed { .. }
            | Self::BodySerializationFailed { .. }
            | Self::UrlParsingFailed { .. }
            | Self::UrlEncodingFailed { .. } => Status::with_details(tonic::Code::Internal, msg, buf.into()),
        }
    }
}

/// - `ConnectorErrorResponse`: connector returned a 4xx/5xx; mapped per HTTP status code
///   following the standard HTTP → gRPC status code translation.
/// - All UCS-side transformation failures → `internal` (UCS machinery failed).
impl ToStatus for ConnectorError {
    fn to_status(&self) -> Status {
        let connector_error: grpc_api_types::payments::ConnectorError =
            ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(self);
        let msg = connector_error.error_message.clone();

        // Serialize the ConnectorError proto to bytes
        let mut buf = Vec::new();
        // SAFETY: ConnectorError only contains String fields with valid UTF-8
        // and prost encoding cannot fail for these controlled types
        let _ = connector_error.encode(&mut buf);

        match self {
            Self::ConnectorErrorResponse(error_response) => match error_response.status_code {
                400 => Status::with_details(tonic::Code::InvalidArgument, msg, buf.into()),
                401 => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
                403 => Status::with_details(tonic::Code::PermissionDenied, msg, buf.into()),
                404 => Status::with_details(tonic::Code::NotFound, msg, buf.into()),
                429 => Status::with_details(tonic::Code::ResourceExhausted, msg, buf.into()),
                500 => Status::with_details(tonic::Code::Internal, msg, buf.into()),
                501 => Status::with_details(tonic::Code::Unimplemented, msg, buf.into()),
                503 => Status::with_details(tonic::Code::Unavailable, msg, buf.into()),
                504 => Status::with_details(tonic::Code::DeadlineExceeded, msg, buf.into()),
                _ => Status::with_details(tonic::Code::Unknown, msg, buf.into()),
            },
            Self::ResponseDeserializationFailed { .. }
            | Self::ResponseHandlingFailed { .. }
            | Self::UnexpectedResponseError { .. }
            | Self::IntegrityCheckFailed { .. } => {
                Status::with_details(tonic::Code::Internal, msg, buf.into())
            }
        }
    }
}

impl ToStatus for ApiClientError {
    fn to_status(&self) -> Status {
        let msg = self.to_string();
        match self {
            Self::RequestTimeoutReceived | Self::GatewayTimeoutReceived => {
                Status::deadline_exceeded(msg)
            }
            Self::ServiceUnavailableReceived => Status::unavailable(msg),
            _ => Status::internal(msg),
        }
    }
}

impl ToStatus for KafkaClientError {
    fn to_status(&self) -> Status {
        Status::internal(self.to_string())
    }
}

impl ToStatus for WebhookError {
    fn to_status(&self) -> Status {
        let msg = self.to_string();
        match self {
            Self::WebhooksNotImplemented { .. } => Status::unimplemented(msg),
            Self::WebhookEventTypeNotFound
            | Self::WebhookSignatureNotFound
            | Self::WebhookReferenceIdNotFound
            | Self::WebhookResourceObjectNotFound
            | Self::WebhookVerificationSecretNotFound => Status::not_found(msg),
            // Caller omitted a required field — bad request from SDK user.
            Self::WebhookMissingRequiredField { .. } => Status::invalid_argument(msg),
            // Bad body from the webhook sender — genuinely bad argument.
            Self::WebhookBodyDecodingFailed => Status::invalid_argument(msg),
            // Caller did not supply required business context (e.g. capture_method).
            Self::WebhookMissingRequiredContext { .. } => Status::invalid_argument(msg),
            // Signature mismatch or configured secret is wrong — authentication failure.
            Self::WebhookSourceVerificationFailed | Self::WebhookVerificationSecretInvalid => {
                Status::unauthenticated(msg)
            }
            Self::WebhookProcessingFailed
            | Self::WebhookAmountConversionFailed { .. }
            | Self::WebhookResponseEncodingFailed => Status::internal(msg),
        }
    }
}

/// UCS plumbing failed and the caller cannot act on it: bare `internal`, no proto details.
impl ToStatus for InternalError {
    fn to_status(&self) -> Status {
        Status::internal(self.to_string())
    }
}

impl ToStatus for GrpcError {
    fn to_status(&self) -> Status {
        match self {
            Self::Integration(e) => e.to_status(),
            Self::Connector(e) => e.to_status(),
            Self::ApiClient(e) => e.to_status(),
            Self::KafkaClient(e) => e.to_status(),
            Self::Webhook(e) => e.to_status(),
            Self::Internal(e) => e.to_status(),
        }
    }
}

/// gRPC status mapping for `GrpcError`, the error every gRPC handler returns.
///
/// The report is logged here, once, while its frames are still attached.
impl IntoGrpcStatus for Report<GrpcError> {
    fn into_grpc_status(self) -> Status {
        let context = self.current_context();
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %context.error_code(),
        );
        context.to_status()
    }
}

impl IntoGrpcStatus for Report<IntegrationError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<ConnectorError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %self.current_context().error_code(),
            http_status_code = ?self.current_context().http_status_code(),
        );
        self.current_context().to_status()
    }
}

impl IntoGrpcStatus for Report<ApiClientError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<KafkaClientError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<WebhookError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<InternalError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<ConnectorFlowError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}
