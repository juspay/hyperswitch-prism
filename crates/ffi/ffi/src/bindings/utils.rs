//! Shared utility functions for UniFFI bindings.
//!
//! Provides helper functions for parsing metadata, building requests/responses,
//! and handling FFI option decoding.

use bytes::Bytes;
use domain_types::connector_types::ConnectorEnum;
use domain_types::router_data::ConnectorSpecificConfig;
use domain_types::router_response_types::Response;
use domain_types::utils::ForeignTryFrom;
use error_stack::Report;
use grpc_api_types::payments::{
    ConnectorError, FfiConnectorHttpRequest, FfiConnectorHttpResponse, FfiOptions, IntegrationError,
};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use prost::Message;

/// Helper to convert internal Request to Protobuf FfiConnectorHttpRequest bytes.
pub fn build_ffi_request_bytes(
    request: &common_utils::request::Request,
) -> Result<Vec<u8>, IntegrationError> {
    let mut headers = request.get_headers_map();
    let (body, boundary) = request
        .body
        .as_ref()
        .map(|b| b.get_body_bytes())
        .transpose()
        .map_err(|e| IntegrationError {
            error_message: format!("Body encoding failed: {e}"),
            error_code: "BODY_ENCODING_FAILED".to_string(),
            suggested_action: None,
            doc_url: None,
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
pub fn build_domain_response(response_bytes: Vec<u8>) -> Result<Response, ConnectorError> {
    let response = FfiConnectorHttpResponse::decode(Bytes::from(response_bytes)).map_err(|e| {
        ConnectorError {
            error_message: format!("ConnectorHttpResponse decode failed: {e}"),
            error_code: "DECODE_FAILED".to_string(),
            http_status_code: None,
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
        status_code: response
            .status_code
            .try_into()
            .map_err(|e| ConnectorError {
                error_message: format!("Invalid HTTP status code: {e}"),
                error_code: "INVALID_STATUS_CODE".to_string(),
                http_status_code: None,
            })?,
    })
}

/// refactor later
/// Parse FfiOptions from optional bytes (for request path).
pub fn parse_ffi_options_for_req(options_bytes: Vec<u8>) -> Result<FfiOptions, IntegrationError> {
    if options_bytes.is_empty() {
        return Err(IntegrationError {
            error_message: "Empty options bytes".to_string(),
            error_code: "EMPTY_OPTIONS".to_string(),
            suggested_action: None,
            doc_url: None,
        });
    }
    FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| IntegrationError {
        error_message: format!("Options decode failed: {e}"),
        error_code: "DECODE_FAILED".to_string(),
        suggested_action: None,
        doc_url: None,
    })
}

/// refactor later
/// Parse FfiOptions from optional bytes (for response path).
pub fn parse_ffi_options_for_res(options_bytes: Vec<u8>) -> Result<FfiOptions, ConnectorError> {
    if options_bytes.is_empty() {
        return Err(ConnectorError {
            error_message: "Empty options bytes".to_string(),
            error_code: "EMPTY_OPTIONS".to_string(),
            http_status_code: None,
        });
    }
    FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| ConnectorError {
        error_message: format!("Options decode failed: {e}"),
        error_code: "DECODE_FAILED".to_string(),
        http_status_code: None,
    })
}

/// refactor later
/// Build FfiMetadataPayload from FfiOptions.
/// The connector identity is inferred from which ConnectorSpecificConfig variant is set.
pub fn parse_metadata(
    options: &FfiOptions,
) -> Result<crate::types::FfiMetadataPayload, IntegrationError> {
    // 1. Resolve ConnectorSpecificConfig from FfiOptions
    let proto_config = options
        .connector_config
        .as_ref()
        .ok_or_else(|| IntegrationError {
            error_message: "Missing connector_config".to_string(),
            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;

    // 2. Infer connector from which oneof variant is set
    let config_variant = proto_config
        .config
        .as_ref()
        .ok_or_else(|| IntegrationError {
            error_message: "Missing connector_config.config".to_string(),
            error_code: "MISSING_CONNECTOR_CONFIG_VARIANT".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;

    let connector = ConnectorEnum::foreign_try_from(config_variant.clone()).map_err(
        |e: Report<domain_types::errors::IntegrationError>| {
            common_utils::errors::ErrorSwitch::switch(e.current_context())
        },
    )?;

    // 3. Convert proto config to domain ConnectorSpecificConfig
    let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
        .map_err(|e: Report<domain_types::errors::IntegrationError>| {
            common_utils::errors::ErrorSwitch::switch(e.current_context())
        })?;

    Ok(crate::types::FfiMetadataPayload {
        connector,
        connector_config: Some(connector_config),
    })
}

/// Resolve connector identity for direct webhook flows.
///
/// Unlike the main req/res flow path, webhook direct flows only need the
/// connector identity up front. Full connector config is optional and is
/// passed through when it can be converted successfully.
pub fn parse_webhook_metadata(
    options: &FfiOptions,
) -> Result<crate::types::FfiMetadataPayload, IntegrationError> {
    let proto_config = options
        .connector_config
        .as_ref()
        .ok_or_else(|| IntegrationError {
            error_message: "Missing connector_config".to_string(),
            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;

    let config_variant = proto_config
        .config
        .as_ref()
        .ok_or_else(|| IntegrationError {
            error_message:
                "Missing connector_config.config. Webhook flows require connector identity, but full connector credentials are optional."
                    .to_string(),
            error_code: "MISSING_CONNECTOR_CONFIG_VARIANT".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;

    // Extract connector identity from the oneof variant.
    // This does NOT parse auth fields, just maps variant name to ConnectorEnum.
    let connector = ConnectorEnum::foreign_try_from(config_variant.clone()).map_err(
        |e: Report<domain_types::errors::IntegrationError>| IntegrationError {
            error_message: e.current_context().to_string(),
            error_code: "INVALID_CONNECTOR_CONFIG_VARIANT".to_string(),
            suggested_action: None,
            doc_url: None,
        },
    )?;

    // For webhook flows, connector config is optional.
    let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone()).ok();

    Ok(crate::types::FfiMetadataPayload {
        connector,
        connector_config,
    })
}
