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

/// Failures in the gRPC plumbing itself, raised before request transformation.
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
/// Handlers return `Report<GrpcError>` rather than constructing a `tonic::Status`. No
/// `From<tonic::Status>` impl exists for this type: adding one would let a hand-built status
/// compile and bypass the logging in `IntoGrpcStatus`.
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
    /// Machine-readable code of the wrapped error.
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

    /// HTTP status code from the connector response, when the wrapped error carries one.
    pub fn http_status_code(&self) -> Option<u16> {
        match self {
            Self::Connector(e) => e.http_status_code(),
            _ => None,
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

/// Pure error-to-status mapping, without logging.
///
/// Private: `IntoGrpcStatus` is the only reachable path to a `Status`, and it logs the report
/// first. A public non-logging mapper would allow statuses that are never logged.
trait ToGrpcStatus {
    fn to_grpc_status_unlogged(&self) -> Status;
}

/// `invalid_argument` — caller sent a missing or invalid field in this request (UCS is stateless;
///   every required ID/field must be supplied by the caller on every call).
/// `unimplemented` — the flow or payment method is not implemented, or not enabled for this connector.
/// `failed_precondition` — connector/merchant configuration problem; not a client credential failure.
/// `unauthenticated` — credential / auth resolution failure.
/// `internal` — UCS machinery failure (encoding, URL building, serialization); caller cannot fix.
impl ToGrpcStatus for IntegrationError {
    fn to_grpc_status_unlogged(&self) -> Status {
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
impl ToGrpcStatus for ConnectorError {
    fn to_grpc_status_unlogged(&self) -> Status {
        let connector_error: grpc_api_types::payments::ConnectorError =
            ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(self);
        let msg = connector_error.error_message.clone();

        // Serialize the ConnectorError proto to bytes
        let mut buf = Vec::new();
        // SAFETY: ConnectorError only contains String fields with valid UTF-8
        // and prost encoding cannot fail for these controlled types
        let _ = connector_error.encode(&mut buf);

        match self {
            Self::ConnectorErrorResponse { error_response, .. } => match error_response.status_code
            {
                400 | 402 | 405 | 406 | 407 | 410..=428 | 431..=499 => {
                    Status::with_details(tonic::Code::InvalidArgument, msg, buf.into())
                }
                401 => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
                403 => Status::with_details(tonic::Code::PermissionDenied, msg, buf.into()),
                404 => Status::with_details(tonic::Code::NotFound, msg, buf.into()),
                408 | 504 => Status::with_details(tonic::Code::DeadlineExceeded, msg, buf.into()),
                409 => Status::with_details(tonic::Code::Aborted, msg, buf.into()),
                429 => Status::with_details(tonic::Code::ResourceExhausted, msg, buf.into()),
                500 | 502 | 505..=599 => {
                    Status::with_details(tonic::Code::Internal, msg, buf.into())
                }
                501 => Status::with_details(tonic::Code::Unimplemented, msg, buf.into()),
                503 => Status::with_details(tonic::Code::Unavailable, msg, buf.into()),
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

impl ToGrpcStatus for ApiClientError {
    fn to_grpc_status_unlogged(&self) -> Status {
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

impl ToGrpcStatus for KafkaClientError {
    fn to_grpc_status_unlogged(&self) -> Status {
        Status::internal(self.to_string())
    }
}

impl ToGrpcStatus for WebhookError {
    fn to_grpc_status_unlogged(&self) -> Status {
        let msg = self.to_string();
        match self {
            Self::WebhooksNotImplemented { .. } => Status::unimplemented(msg),
            Self::WebhookEventTypeNotFound
            | Self::WebhookSignatureNotFound
            | Self::WebhookReferenceIdNotFound
            | Self::WebhookResourceObjectNotFound
            | Self::WebhookVerificationSecretNotFound => Status::not_found(msg),
            Self::WebhookMissingRequiredField { .. } => Status::invalid_argument(msg),
            Self::WebhookBodyDecodingFailed => Status::invalid_argument(msg),
            Self::WebhookMissingRequiredContext { .. } => Status::invalid_argument(msg),
            Self::WebhookSourceVerificationFailed | Self::WebhookVerificationSecretInvalid => {
                Status::unauthenticated(msg)
            }
            Self::WebhookProcessingFailed
            | Self::WebhookAmountConversionFailed { .. }
            | Self::WebhookResponseEncodingFailed => Status::internal(msg),
        }
    }
}

/// Not actionable by the caller: bare `internal`, no proto details.
impl ToGrpcStatus for InternalError {
    fn to_grpc_status_unlogged(&self) -> Status {
        Status::internal(self.to_string())
    }
}

impl ToGrpcStatus for GrpcError {
    fn to_grpc_status_unlogged(&self) -> Status {
        match self {
            Self::Integration(e) => e.to_grpc_status_unlogged(),
            Self::Connector(e) => e.to_grpc_status_unlogged(),
            Self::ApiClient(e) => e.to_grpc_status_unlogged(),
            Self::KafkaClient(e) => e.to_grpc_status_unlogged(),
            Self::Webhook(e) => e.to_grpc_status_unlogged(),
            Self::Internal(e) => e.to_grpc_status_unlogged(),
        }
    }
}

/// gRPC status mapping for `GrpcError`, the error every gRPC handler returns.
///
/// The report is logged here, while its frames are still attached.
impl IntoGrpcStatus for Report<GrpcError> {
    fn into_grpc_status(self) -> Status {
        let context = self.current_context();
        let status = context.to_grpc_status_unlogged();
        logger::error!(
            error = ?self,
            error_code = %context.error_code(),
            http_status_code = ?context.http_status_code(),
            grpc_code_name = ?status.code(),
        );
        status
    }
}

impl IntoGrpcStatus for Report<IntegrationError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
    }
}

impl IntoGrpcStatus for Report<ConnectorError> {
    fn into_grpc_status(self) -> Status {
        self.to_grpc_error().into_grpc_status()
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
