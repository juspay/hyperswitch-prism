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

impl InternalError {
    /// Machine-readable error code (SCREAMING_SNAKE_CASE from variant name).
    pub fn error_code(&self) -> &str {
        self.as_ref()
    }
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

/// gRPC status mapping for `IntegrationError`.
///
/// `invalid_argument` — caller sent a missing or invalid field in this request (UCS is stateless;
///   every required ID/field must be supplied by the caller on every call).
/// `unimplemented` — the flow or payment method is not implemented, or not enabled for this connector.
/// `failed_precondition` — connector/merchant configuration problem; not a client credential failure.
/// `unauthenticated` — credential / auth resolution failure.
/// `internal` — UCS machinery failure (encoding, URL building, serialization); caller cannot fix.
fn status_for_integration_error(context: &IntegrationError) -> Status {
    let integration_error: grpc_api_types::payments::IntegrationError =
        ErrorSwitch::switch(context);
    let msg = integration_error.error_message.clone();

    // Serialize the IntegrationError proto to bytes
    let mut buf = Vec::new();
    // SAFETY: IntegrationError only contains String fields with valid UTF-8
    // and prost encoding cannot fail for these controlled types
    let _ = integration_error.encode(&mut buf);

    match context {
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
        | IntegrationError::MaxFieldLengthViolated { .. } => Status::with_details(tonic::Code::InvalidArgument, msg, buf.into()),
        IntegrationError::FlowNotSupported { .. }
        | IntegrationError::NotSupported { .. }
        | IntegrationError::CaptureMethodNotSupported { .. }
        | IntegrationError::NotImplemented(..) => Status::with_details(tonic::Code::Unimplemented, msg, buf.into()),
        IntegrationError::InvalidConnectorConfig { .. }
        | IntegrationError::ConfigurationError { .. }
        | IntegrationError::NoConnectorMetaData { .. } => Status::with_details(tonic::Code::FailedPrecondition, msg, buf.into()),
        IntegrationError::FailedToObtainAuthType { .. } => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
        IntegrationError::SourceVerificationFailed { .. } => Status::with_details(tonic::Code::Unauthenticated, msg, buf.into()),
        IntegrationError::FailedToObtainIntegrationUrl { .. }
        | IntegrationError::RequestEncodingFailed { .. }
        | IntegrationError::HeaderMapConstructionFailed { .. }
        | IntegrationError::BodySerializationFailed { .. }
        | IntegrationError::UrlParsingFailed { .. }
        | IntegrationError::UrlEncodingFailed { .. } => Status::with_details(tonic::Code::Internal, msg, buf.into()),
    }
}

/// gRPC status mapping for `ConnectorError` (response phase).
///
/// - `ConnectorErrorResponse`: connector returned a 4xx/5xx; mapped per HTTP status code
///   following the standard HTTP → gRPC status code translation.
/// - All UCS-side transformation failures → `internal` (UCS machinery failed).
fn status_for_connector_error(context: &ConnectorError) -> Status {
    let connector_error: grpc_api_types::payments::ConnectorError =
        ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(context);
    let msg = connector_error.error_message.clone();

    // Serialize the ConnectorError proto to bytes
    let mut buf = Vec::new();
    // SAFETY: ConnectorError only contains String fields with valid UTF-8
    // and prost encoding cannot fail for these controlled types
    let _ = connector_error.encode(&mut buf);

    match context {
        ConnectorError::ConnectorErrorResponse(error_response) => {
            match error_response.status_code {
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
            }
        }
        ConnectorError::ResponseDeserializationFailed { .. }
        | ConnectorError::ResponseHandlingFailed { .. }
        | ConnectorError::UnexpectedResponseError { .. }
        | ConnectorError::IntegrityCheckFailed { .. } => {
            Status::with_details(tonic::Code::Internal, msg, buf.into())
        }
    }
}

/// gRPC status mapping for `ApiClientError` (network/transport phase).
fn status_for_api_client_error(context: &ApiClientError) -> Status {
    let msg = context.to_string();
    match context {
        ApiClientError::RequestTimeoutReceived | ApiClientError::GatewayTimeoutReceived => {
            Status::deadline_exceeded(msg)
        }
        ApiClientError::ServiceUnavailableReceived => Status::unavailable(msg),
        _ => Status::internal(msg),
    }
}

/// gRPC status mapping for `KafkaClientError`.
fn status_for_kafka_client_error(context: &KafkaClientError) -> Status {
    Status::internal(context.to_string())
}

/// gRPC status mapping for `WebhookError`.
fn status_for_webhook_error(context: &WebhookError) -> Status {
    let msg = context.to_string();
    match context {
        WebhookError::WebhooksNotImplemented { .. } => Status::unimplemented(msg),
        WebhookError::WebhookEventTypeNotFound
        | WebhookError::WebhookSignatureNotFound
        | WebhookError::WebhookReferenceIdNotFound
        | WebhookError::WebhookResourceObjectNotFound
        | WebhookError::WebhookVerificationSecretNotFound => Status::not_found(msg),
        // Caller omitted a required field — bad request from SDK user.
        WebhookError::WebhookMissingRequiredField { .. } => Status::invalid_argument(msg),
        // Bad body from the webhook sender — genuinely bad argument.
        WebhookError::WebhookBodyDecodingFailed => Status::invalid_argument(msg),
        // Caller did not supply required business context (e.g. capture_method).
        WebhookError::WebhookMissingRequiredContext { .. } => Status::invalid_argument(msg),
        // Signature mismatch or configured secret is wrong — authentication failure.
        WebhookError::WebhookSourceVerificationFailed
        | WebhookError::WebhookVerificationSecretInvalid => Status::unauthenticated(msg),
        WebhookError::WebhookProcessingFailed
        | WebhookError::WebhookAmountConversionFailed { .. }
        | WebhookError::WebhookResponseEncodingFailed => Status::internal(msg),
    }
}

/// Machine-readable code for the error a `ConnectorFlowError` wraps.
fn connector_flow_error_code(context: &ConnectorFlowError) -> &str {
    match context {
        ConnectorFlowError::Request(e) => e.error_code(),
        ConnectorFlowError::Response(e) => e.error_code(),
        ConnectorFlowError::Client(e) => e.as_ref(),
        ConnectorFlowError::KafkaClient(e) => e.as_ref(),
    }
}

impl IntoGrpcStatus for Report<IntegrationError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %self.current_context().error_code(),
        );
        status_for_integration_error(self.current_context())
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
        status_for_connector_error(self.current_context())
    }
}

impl IntoGrpcStatus for Report<ApiClientError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %self.current_context().as_ref(),
        );
        status_for_api_client_error(self.current_context())
    }
}

impl IntoGrpcStatus for Report<KafkaClientError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error = ?self, extra_error = ?self);
        status_for_kafka_client_error(self.current_context())
    }
}

/// gRPC status mapping for `ConnectorFlowError` (unified gRPC path wrapper).
impl IntoGrpcStatus for Report<ConnectorFlowError> {
    fn into_grpc_status(self) -> Status {
        let context = self.current_context();
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %connector_flow_error_code(context),
        );
        match context {
            ConnectorFlowError::Request(e) => status_for_integration_error(e),
            ConnectorFlowError::Client(e) => status_for_api_client_error(e),
            ConnectorFlowError::KafkaClient(e) => status_for_kafka_client_error(e),
            ConnectorFlowError::Response(e) => status_for_connector_error(e),
        }
    }
}

impl IntoGrpcStatus for Report<WebhookError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %self.current_context().as_ref(),
        );
        status_for_webhook_error(self.current_context())
    }
}

/// gRPC status mapping for `InternalError`.
fn status_for_internal_error(context: &InternalError) -> Status {
    Status::internal(context.to_string())
}

impl IntoGrpcStatus for Report<InternalError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(
            error = ?self,
            extra_error = ?self,
            error_code = %self.current_context().error_code(),
        );
        status_for_internal_error(self.current_context())
    }
}

/// Machine-readable code for the error a `GrpcError` wraps.
fn grpc_error_code(context: &GrpcError) -> &str {
    match context {
        GrpcError::Integration(e) => e.error_code(),
        GrpcError::Connector(e) => e.error_code(),
        GrpcError::ApiClient(e) => e.as_ref(),
        GrpcError::KafkaClient(e) => e.as_ref(),
        GrpcError::Webhook(e) => e.as_ref(),
        GrpcError::Internal(e) => e.error_code(),
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
            error_code = %grpc_error_code(context),
        );
        match context {
            GrpcError::Integration(e) => status_for_integration_error(e),
            GrpcError::Connector(e) => status_for_connector_error(e),
            GrpcError::ApiClient(e) => status_for_api_client_error(e),
            GrpcError::KafkaClient(e) => status_for_kafka_client_error(e),
            GrpcError::Webhook(e) => status_for_webhook_error(e),
            GrpcError::Internal(e) => status_for_internal_error(e),
        }
    }
}
