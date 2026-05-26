use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use prost::Message;
use serde::Serialize;

/// HTTP error with optional SDK error details
#[derive(Debug)]
pub struct HttpError {
    pub status: StatusCode,
    pub message: String,
    pub details: Option<ErrorDetails>,
}

/// Matches the SDK's SdkError enum structure
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    IntegrationError(grpc_api_types::payments::IntegrationError),
    ConnectorError(Box<grpc_api_types::payments::ConnectorError>),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    message: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails>,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: ErrorDetail {
                message: self.message.clone(),
                code: format!("{}", self.status.as_u16()),
                details: self.details,
            },
        });
        (self.status, body).into_response()
    }
}

/// Extract SDK error details from gRPC Status
fn extract_sdk_error_from_status(status: &tonic::Status) -> Option<ErrorDetails> {
    let details = status.details();
    // Try to decode IntegrationError from proto details
    if let Ok(integration_error) = grpc_api_types::payments::IntegrationError::decode(details) {
        return Some(ErrorDetails::IntegrationError(integration_error));
    }

    // Try to decode ConnectorError from proto details
    if let Ok(connector_error) = grpc_api_types::payments::ConnectorError::decode(details) {
        return Some(ErrorDetails::ConnectorError(Box::new(connector_error)));
    }
}


// Convert tonic::Status to HTTP error
impl From<tonic::Status> for HttpError {
    fn from(status: tonic::Status) -> Self {
        let http_status = match status.code() {
            tonic::Code::Ok => StatusCode::OK,
            tonic::Code::Cancelled => StatusCode::REQUEST_TIMEOUT,
            tonic::Code::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::InvalidArgument => StatusCode::BAD_REQUEST,
            tonic::Code::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
            tonic::Code::NotFound => StatusCode::NOT_FOUND,
            tonic::Code::AlreadyExists => StatusCode::CONFLICT,
            tonic::Code::PermissionDenied => StatusCode::FORBIDDEN,
            tonic::Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
            tonic::Code::FailedPrecondition => StatusCode::PRECONDITION_FAILED,
            tonic::Code::Aborted => StatusCode::CONFLICT,
            tonic::Code::OutOfRange => StatusCode::RANGE_NOT_SATISFIABLE,
            tonic::Code::Unimplemented => StatusCode::NOT_IMPLEMENTED,
            tonic::Code::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            tonic::Code::DataLoss => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unauthenticated => StatusCode::UNAUTHORIZED,
        };

        let message = status.message().to_string();

        // Extract SDK error details from Status
        let details = extract_sdk_error_from_status(&status);

        Self {
            status: http_status,
            message,
            details,
        }
    }
}
