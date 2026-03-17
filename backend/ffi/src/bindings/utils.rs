//! Shared utility functions for UniFFI bindings.
//!
//! Provides helper functions for parsing metadata, building requests/responses,
//! and handling FFI option decoding.

use bytes::Bytes;
use domain_types::connector_types::ConnectorEnum;
use domain_types::errors::{ApplicationErrorResponse, ConnectorError};
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::router_response_types::Response;
use domain_types::utils::ForeignTryFrom;
use error_stack::Report;
use grpc_api_types::payments::{
    FfiConnectorHttpRequest, FfiConnectorHttpResponse, FfiOptions, PaymentStatus, RequestError,
    ResponseError,
};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use prost::Message;
use ucs_env::error::ErrorSwitch;

/// Helper to convert internal Request to Protobuf FfiConnectorHttpRequest bytes.
pub fn build_ffi_request_bytes(
    request: &common_utils::request::Request,
) -> Result<Vec<u8>, RequestError> {
    let mut headers = request.get_headers_map();
    let (body, boundary) = request
        .body
        .as_ref()
        .map(|b| b.get_body_bytes())
        .transpose()
        .map_err(|e| RequestError {
            status: PaymentStatus::Pending.into(),
            error_message: Some(format!("Body encoding failed: {e}")),
            error_code: None,
            status_code: Some(400),
        })?
        .unwrap_or((None, None));

    if let Some(boundary) = boundary {
        headers.insert(
            "content-type".to_string(),
            format!("multipart/form-data; boundary={}", boundary),
        );
    }

    let proto = FfiConnectorHttpRequest {
        url: request.url.clone(),
        method: request.method.to_string(),
        headers,
        body,
    };

    Ok(proto.encode_to_vec())
}

/// Helper to convert Protobuf FfiConnectorHttpResponse bytes to internal Response.
pub fn build_domain_response(response_bytes: Vec<u8>) -> Result<Response, ResponseError> {
    let response = FfiConnectorHttpResponse::decode(Bytes::from(response_bytes)).map_err(|e| {
        ResponseError {
            status: PaymentStatus::Pending.into(),
            error_message: Some(format!("ConnectorHttpResponse decode failed: {e}")),
            error_code: None,
            status_code: Some(400),
        }
    })?;

    let mut header_map = HeaderMap::new();
    for (key, value) in &response.headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            header_map.insert(name, val);
        }
    }

    Ok(Response {
        headers: if header_map.is_empty() {
            None
        } else {
            Some(header_map)
        },
        response: Bytes::from(response.body),
        status_code: response.status_code.try_into().map_err(|e| ResponseError {
            status: PaymentStatus::Pending.into(),
            error_message: Some(format!("Invalid HTTP status code: {e}")),
            error_code: None,
            status_code: Some(400),
        })?,
    })
}

/// refactor later
/// Parse FfiOptions from optional bytes (for request path).
pub fn parse_ffi_options_for_req(options_bytes: Vec<u8>) -> Result<FfiOptions, RequestError> {
    if options_bytes.is_empty() {
        return Err(RequestError {
            status: PaymentStatus::Pending.into(),
            error_message: Some("Empty options bytes".to_string()),
            error_code: None,
            status_code: Some(400),
        });
    }
    FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| RequestError {
        status: PaymentStatus::Pending.into(),
        error_message: Some(format!("Options decode failed: {e}")),
        error_code: None,
        status_code: Some(400),
    })
}

/// refactor later
/// Parse FfiOptions from optional bytes (for response path).
pub fn parse_ffi_options_for_res(options_bytes: Vec<u8>) -> Result<FfiOptions, ResponseError> {
    if options_bytes.is_empty() {
        return Err(ResponseError {
            status: PaymentStatus::Pending.into(),
            error_message: Some("Empty options bytes".to_string()),
            error_code: None,
            status_code: Some(400),
        });
    }
    FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| ResponseError {
        status: PaymentStatus::Pending.into(),
        error_message: Some(format!("Options decode failed: {e}")),
        error_code: None,
        status_code: Some(400),
    })
}

/// refactor later
/// Build FfiMetadataPayload from FfiOptions.
/// The connector identity is inferred from which ConnectorSpecificConfig variant is set.
pub fn parse_metadata_for_req(
    options: &FfiOptions,
) -> Result<crate::types::FfiMetadataPayload, RequestError> {
    // 1. Resolve ConnectorSpecificConfig from FfiOptions
    let proto_config = options
        .connector_config
        .as_ref()
        .ok_or_else(|| RequestError {
            status: PaymentStatus::Pending.into(),
            error_message: Some("Missing connector_config".to_string()),
            error_code: None,
            status_code: Some(400),
        })?;

    // 2. Infer connector from which oneof variant is set
    let config_variant = proto_config.config.as_ref().ok_or_else(|| RequestError {
        status: PaymentStatus::Pending.into(),
        error_message: Some("Missing connector_config.config".to_string()),
        error_code: None,
        status_code: Some(400),
    })?;

    let connector = ConnectorEnum::foreign_try_from(config_variant.clone())
        .map_err(|e: Report<ApplicationErrorResponse>| e.current_context().switch())?;

    // 3. Convert proto config to domain ConnectorSpecificConfig
    let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
        .map_err(|e: Report<ConnectorError>| {
            let app_error: ApplicationErrorResponse = e.current_context().switch();
            app_error.switch()
        })?;

    Ok(crate::types::FfiMetadataPayload {
        connector,
        connector_config,
    })
}

/// refactor later
/// Build FfiMetadataPayload from FfiOptions (for response path).
pub fn parse_metadata_for_res(
    options: &FfiOptions,
) -> Result<crate::types::FfiMetadataPayload, ResponseError> {
    // 1. Resolve ConnectorSpecificConfig from FfiOptions
    let proto_config = options
        .connector_config
        .as_ref()
        .ok_or_else(|| ResponseError {
            status: PaymentStatus::Pending.into(),
            error_message: Some("Missing connector_config".to_string()),
            error_code: None,
            status_code: Some(400),
        })?;

    // 2. Infer connector from which oneof variant is set
    let config_variant = proto_config.config.as_ref().ok_or_else(|| ResponseError {
        status: PaymentStatus::Pending.into(),
        error_message: Some("Missing connector_config.config".to_string()),
        error_code: None,
        status_code: Some(400),
    })?;

    let connector = ConnectorEnum::foreign_try_from(config_variant.clone())
        .map_err(|e: Report<ApplicationErrorResponse>| e.current_context().switch())?;

    // 3. Convert proto config to domain ConnectorSpecificConfig
    let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
        .map_err(|e: Report<ConnectorError>| {
            let app_error: ApplicationErrorResponse = e.current_context().switch();
            app_error.switch()
        })?;

    Ok(crate::types::FfiMetadataPayload {
        connector,
        connector_config,
    })
}
