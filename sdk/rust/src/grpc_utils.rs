use grpc_api_types::payments::ErrorInfo;

/// Format a gRPC response error into a `Box<dyn Error>`.
///
/// Extracts `connector_details.code` and `connector_details.message` from the
/// response `error` field when present.  Used by generated connector examples
/// to surface 4xx/5xx connector errors as `Err(...)`.
pub fn grpc_response_err(
    status_code: u32,
    error: &Option<ErrorInfo>,
) -> Box<dyn std::error::Error> {
    let code = error
        .as_ref()
        .and_then(|e| e.connector_details.as_ref())
        .and_then(|d| d.code.as_deref())
        .unwrap_or("-");
    let msg = error
        .as_ref()
        .and_then(|e| e.connector_details.as_ref())
        .and_then(|d| d.message.as_deref())
        .unwrap_or("-");
    format!("status_code: {status_code}, code: {code}, message: {msg}").into()
}
