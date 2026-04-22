use common_enums::KafkaClientError;
use common_utils::errors::ErrorSwitch;
use domain_types::errors::{
    ApiClientError, ConnectorError, ConnectorFlowError, IntegrationError, WebhookError,
};
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
        let integration_error: grpc_api_types::payments::IntegrationError =
            ErrorSwitch::switch(self.current_context());
        let msg = integration_error.error_message.clone();

        // Serialize the IntegrationError proto to bytes
        let mut buf = Vec::new();
        let _ = integration_error.encode(&mut buf);

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
            | IntegrationError::MaxFieldLengthViolated { .. } => Status::with_details(tonic::Code::InvalidArgument, msg, buf.into()),
            IntegrationError::FlowNotSupported { .. }
            | IntegrationError::NotSupported { .. }
            | IntegrationError::CaptureMethodNotSupported { .. }
            | IntegrationError::NotImplemented(..)
            | IntegrationError::InvalidConnectorConfig { .. }
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
}

/// Direct gRPC status mapping for `ConnectorError` (response phase).
///
/// - `ConnectorErrorResponse`: connector returned a 4xx/5xx; mapped per HTTP status code
///   following the standard HTTP → gRPC status code translation.
/// - All UCS-side transformation failures → `internal` (UCS machinery failed).
impl IntoGrpcStatus for error_stack::Report<ConnectorError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        let connector_error: grpc_api_types::payments::ConnectorError =
            ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(self.current_context());
        let msg = connector_error.error_message.clone();

        // Serialize the ConnectorError proto to bytes
        let mut buf = Vec::new();
        let _ = connector_error.encode(&mut buf);

        match self.current_context() {
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
}
