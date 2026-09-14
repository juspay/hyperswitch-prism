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

/// Matches the SDK's SdkError enum structure with automatic "type" discriminator
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    IntegrationError {
        error_code: String,
        error_message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggested_action: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        doc_url: Option<String>,
    },
    ConnectorError {
        error_code: String,
        error_message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        http_status_code: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_info: Option<Box<ErrorInfo>>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    unified_details: Option<grpc_api_types::payments::UnifiedErrorDetails>,

    #[serde(skip_serializing_if = "Option::is_none")]
    issuer_details: Option<grpc_api_types::payments::IssuerErrorDetails>,

    #[serde(skip_serializing_if = "Option::is_none")]
    connector_details: Option<ConnectorErrorDetails>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,

    // Custom serializer for FlowStatus - outputs string enum names like "REFUND_FAILURE"
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_flow_status_option")]
    status: Option<grpc_api_types::payments::FlowStatus>,
}

/// Custom serializer for FlowStatus that converts oneof enum values to strings
fn serialize_flow_status_option<S>(
    flow_status: &Option<grpc_api_types::payments::FlowStatus>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use grpc_api_types::payments::{flow_status, DisputeStatus, PaymentStatus, RefundStatus};
    use serde::Serialize;

    match flow_status {
        Some(fs) => match &fs.status {
            Some(flow_status::Status::PaymentStatus(val)) => {
                let status_str = PaymentStatus::try_from(*val)
                    .map(|e| e.as_str_name())
                    .unwrap_or("PAYMENT_STATUS_UNSPECIFIED");

                let mut map = serde_json::Map::new();
                map.insert(
                    "payment_status".to_string(),
                    serde_json::Value::String(status_str.to_string()),
                );
                serde_json::Value::Object(map).serialize(serializer)
            }
            Some(flow_status::Status::RefundStatus(val)) => {
                let status_str = RefundStatus::try_from(*val)
                    .map(|e| e.as_str_name())
                    .unwrap_or("REFUND_STATUS_UNSPECIFIED");

                let mut map = serde_json::Map::new();
                map.insert(
                    "refund_status".to_string(),
                    serde_json::Value::String(status_str.to_string()),
                );
                serde_json::Value::Object(map).serialize(serializer)
            }
            Some(flow_status::Status::DisputeStatus(val)) => {
                let status_str = DisputeStatus::try_from(*val)
                    .map(|e| e.as_str_name())
                    .unwrap_or("DISPUTE_STATUS_UNSPECIFIED");

                let mut map = serde_json::Map::new();
                map.insert(
                    "dispute_status".to_string(),
                    serde_json::Value::String(status_str.to_string()),
                );
                serde_json::Value::Object(map).serialize(serializer)
            }
            None => serializer.serialize_none(),
        },
        None => serializer.serialize_none(),
    }
}

/// Convert proto ErrorInfo to wrapper
fn convert_error_info(ei: grpc_api_types::payments::ErrorInfo) -> ErrorInfo {
    ErrorInfo {
        unified_details: ei.unified_details,
        issuer_details: ei.issuer_details,
        connector_details: ei.connector_details.map(|cd| ConnectorErrorDetails {
            code: cd.code,
            message: cd.message,
            reason: cd.reason,
            status: cd.status, // FlowStatus - will use custom serializer
        }),
    }
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
fn extract_error_details_from_status(status: &tonic::Status) -> Option<ErrorDetails> {
    let details = status.details();

    // Try to decode IntegrationError from proto details
    if let Ok(err) = grpc_api_types::payments::IntegrationError::decode(details) {
        return Some(ErrorDetails::IntegrationError {
            error_code: err.error_code,
            error_message: err.error_message,
            suggested_action: err.suggested_action,
            doc_url: err.doc_url,
        });
    }

    // Try to decode ConnectorError from proto details
    if let Ok(err) = grpc_api_types::payments::ConnectorError::decode(details) {
        return Some(ErrorDetails::ConnectorError {
            error_code: err.error_code,
            error_message: err.error_message,
            http_status_code: err.http_status_code,
            error_info: err.error_info.map(convert_error_info).map(Box::new),
        });
    }

    None
}

/// Extract the connector's exact HTTP error status when the decoded details identify a
/// connector-originated 4xx/5xx response.
fn connector_http_status_from_error_details(details: Option<&ErrorDetails>) -> Option<StatusCode> {
    match details {
        Some(ErrorDetails::ConnectorError {
            error_code,
            http_status_code: Some(http_status_code),
            ..
        }) if error_code == "CONNECTOR_ERROR_RESPONSE" => u16::try_from(*http_status_code)
            .ok()
            .and_then(|status_code| StatusCode::from_u16(status_code).ok())
            .filter(|status_code| status_code.is_client_error() || status_code.is_server_error()),
        _ => None,
    }
}

fn grpc_code_to_http_status(code: tonic::Code) -> StatusCode {
    match code {
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
    }
}

/// The HTTP status the caller receives for a gRPC error: the connector's exact 4xx/5xx when the
/// details carry one, else the gRPC-to-HTTP fallback. Shared by the HTTP response and the golden
/// line so both report the same status.
pub fn http_status_for_status(status: &tonic::Status) -> StatusCode {
    let details = extract_error_details_from_status(status);
    connector_http_status_from_error_details(details.as_ref())
        .unwrap_or_else(|| grpc_code_to_http_status(status.code()))
}

// Convert tonic::Status to HTTP error
impl From<tonic::Status> for HttpError {
    fn from(status: tonic::Status) -> Self {
        let details = extract_error_details_from_status(&status);

        // Preserve the exact connector HTTP status when available; otherwise use the
        // canonical gRPC-to-HTTP fallback.
        let http_status = match connector_http_status_from_error_details(details.as_ref()) {
            Some(connector_status) => connector_status,
            None => grpc_code_to_http_status(status.code()),
        };

        let message = status.message().to_string();

        Self {
            status: http_status,
            message,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_with_connector_error(
        grpc_code: tonic::Code,
        error_code: &str,
        http_status_code: Option<u32>,
    ) -> tonic::Status {
        let connector_error = grpc_api_types::payments::ConnectorError {
            error_message: "connector error".to_string(),
            error_code: error_code.to_string(),
            http_status_code,
            error_info: None,
            raw_connector_request: None,
            raw_connector_response: None,
            typed_connector_request: None,
            typed_connector_response: None,
        };
        let encoded = connector_error.encode_to_vec();

        tonic::Status::with_details(grpc_code, "connector error", encoded.into())
    }

    #[test]
    fn connector_http_status_takes_precedence_over_grpc_mapping() {
        let cases = [
            (tonic::Code::Unknown, 409),
            (tonic::Code::Unknown, 422),
            (tonic::Code::DeadlineExceeded, 408),
            (tonic::Code::Unavailable, 502),
            (tonic::Code::Unknown, 599),
        ];

        for (grpc_code, connector_http_status) in cases {
            let status = status_with_connector_error(
                grpc_code,
                "CONNECTOR_ERROR_RESPONSE",
                Some(connector_http_status.into()),
            );

            let http_error = HttpError::from(status);

            assert_eq!(
                http_error.status.as_u16(),
                connector_http_status,
                "connector HTTP status should be preserved for {grpc_code:?}"
            );
        }
    }

    #[test]
    fn non_connector_response_errors_use_grpc_fallback() {
        let status = status_with_connector_error(
            tonic::Code::Internal,
            "RESPONSE_HANDLING_FAILED",
            Some(422),
        );

        let http_error = HttpError::from(status);

        assert_eq!(http_error.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn invalid_connector_http_status_uses_grpc_fallback() {
        for connector_http_status in [200, 600] {
            let status = status_with_connector_error(
                tonic::Code::Unknown,
                "CONNECTOR_ERROR_RESPONSE",
                Some(connector_http_status),
            );

            let http_error = HttpError::from(status);

            assert_eq!(http_error.status, StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    #[test]
    fn missing_details_use_grpc_fallback() {
        let http_error = HttpError::from(tonic::Status::invalid_argument("invalid request"));

        assert_eq!(http_error.status, StatusCode::BAD_REQUEST);
    }
}
