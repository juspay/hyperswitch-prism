// Package: ffi.bindings
// File: uniffi.rs
//
// Overview:
//   UniFFI bridge implementation for the Connector Service.
//   Provides the top-level FFI entry points for request and response transformations.

#[cfg(feature = "uniffi")]
mod uniffi_bindings_inner {
    use bytes::Bytes;
    use common_utils::request::Request;
    use domain_types::router_response_types::Response;
    use grpc_api_types::payments::Environment;
    use grpc_api_types::payments::{
        ffi_result, ConnectorError, FfiConnectorHttpRequest, FfiConnectorHttpResponse, FfiResult,
        IntegrationError,
    };
    use prost::Message;
    use std::collections::HashMap;

    use crate::bindings::utils::{
        build_domain_response, build_ffi_request_bytes, parse_ffi_options_for_req,
        parse_ffi_options_for_res, parse_metadata, parse_webhook_metadata,
    };
    use crate::define_ffi_flow;

    // ── Generic transformer runners ───────────────────────────────────────────

    /// Decode `request_bytes` as `Req`, build `FfiRequestData`, call `handler`,
    /// and encode the resulting connector HTTP request as Result proto.
    /// If the handler returns an error, encode the IntegrationError in Result.
    pub fn run_req_transformer<Req>(
        request_bytes: Vec<u8>,
        options_bytes: Vec<u8>,
        handler: impl Fn(
            crate::types::FfiRequestData<Req>,
            Option<Environment>,
        ) -> Result<Option<Request>, IntegrationError>,
    ) -> Vec<u8>
    where
        Req: Message + Default,
    {
        let payload = match Req::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(IntegrationError {
                        error_message: format!("Request payload decode failed: {e}"),
                        error_code: "DECODE_FAILED".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_req(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        let result = match handler(request, environment) {
            Ok(r) => r,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let connector_request = match result {
            Some(r) => r,
            None => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(IntegrationError {
                        error_message: "Request encoding failed".to_string(),
                        error_code: "ENCODING_FAILED".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })),
                }
                .encode_to_vec();
            }
        };

        match build_ffi_request_bytes(&connector_request) {
            Ok(bytes) => match FfiConnectorHttpRequest::decode(Bytes::from(bytes)) {
                Ok(http_request) => FfiResult {
                    r#type: ffi_result::Type::HttpRequest.into(),
                    payload: Some(ffi_result::Payload::HttpRequest(http_request)),
                }
                .encode_to_vec(),
                Err(e) => FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(IntegrationError {
                        error_message: format!("Request re-decode failed: {e}"),
                        error_code: "RE_DECODE_FAILED".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })),
                }
                .encode_to_vec(),
            },
            Err(e) => FfiResult {
                r#type: ffi_result::Type::IntegrationError.into(),
                payload: Some(ffi_result::Payload::IntegrationError(e)),
            }
            .encode_to_vec(),
        }
    }

    /// Decode `response_bytes` as the domain `Response` and `request_bytes` as `Req`,
    /// call `handler`, and encode the result as Result proto.
    /// If the handler returns an error, encode the ConnectorError in Result.
    pub fn run_res_transformer<Req, Res>(
        response_bytes: Vec<u8>,
        request_bytes: Vec<u8>,
        options_bytes: Vec<u8>,
        handler: impl Fn(
            crate::types::FfiRequestData<Req>,
            Response,
            Option<Environment>,
        ) -> Result<Res, Box<ConnectorError>>,
    ) -> Vec<u8>
    where
        Req: Message + Default,
        Res: Message,
    {
        let domain_response = match build_domain_response(response_bytes) {
            Ok(r) => r,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(*e)),
                }
                .encode_to_vec()
            }
        };

        let payload = match Req::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                        error_message: format!("Request payload decode failed: {e}"),
                        error_code: "DECODE_FAILED".to_string(),
                        http_status_code: None,
                        error_info: None,
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(*e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                        error_message: e.error_message,
                        error_code: e.error_code,
                        http_status_code: None,
                        error_info: None,
                    })),
                }
                .encode_to_vec()
            }
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        // Extract headers and status code from domain_response before passing to handler
        let response_headers: HashMap<String, String> = domain_response
            .headers
            .as_ref()
            .map(|h| {
                h.iter()
                    .filter_map(|(k, v)| {
                        let key = k.to_string();
                        let value = v.to_str().ok()?.to_string();
                        Some((key, value))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let response_status_code = u32::from(domain_response.status_code);

        match handler(request, domain_response, environment) {
            Ok(proto_response) => {
                // Serialize the protobuf response and wrap it in FfiConnectorHttpResponse
                let response_bytes = proto_response.encode_to_vec();
                let http_response = FfiConnectorHttpResponse {
                    status_code: response_status_code,
                    headers: response_headers,
                    body: response_bytes,
                };
                FfiResult {
                    r#type: ffi_result::Type::HttpResponse.into(),
                    payload: Some(ffi_result::Payload::HttpResponse(http_response)),
                }
                .encode_to_vec()
            }
            Err(e) => FfiResult {
                r#type: ffi_result::Type::ConnectorError.into(),
                payload: Some(ffi_result::Payload::ConnectorError(*e)),
            }
            .encode_to_vec(),
        }
    }

    // ── Flow registrations (auto-generated) ──────────────────────────────────
    // To add a new flow: implement req_transformer!/res_transformer! in
    // services/payments.rs, then run `make generate` to regenerate this file.

    include!("_generated_ffi_flows.rs");

    // ── Hand-written exports (not auto-generated) ─────────────────────────────

    /// parse_event — stateless webhook event type and resource reference extraction.
    ///
    /// No secrets, no context. The caller passes raw `EventServiceParseRequest` proto bytes
    /// and receives encoded `EventServiceParseResponse` bytes directly.
    #[uniffi::export]
    pub fn parse_event_transformer(request_bytes: Vec<u8>, options_bytes: Vec<u8>) -> Vec<u8> {
        use grpc_api_types::payments::EventServiceParseRequest;
        use prost::Message as _;

        let payload = match EventServiceParseRequest::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(IntegrationError {
                        error_message: format!("EventServiceParseRequest decode failed: {e}"),
                        error_code: "DECODE_FAILED".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_req(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_webhook_metadata(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        match crate::handlers::payments::parse_event_handler(request, environment) {
            Ok(response) => FfiResult {
                r#type: ffi_result::Type::ProtoResponse.into(),
                payload: Some(ffi_result::Payload::ProtoResponse(response.encode_to_vec())),
            }
            .encode_to_vec(),
            Err(e) => FfiResult {
                r#type: ffi_result::Type::IntegrationError.into(),
                payload: Some(ffi_result::Payload::IntegrationError(e)),
            }
            .encode_to_vec(),
        }
    }

    /// handle_event — synchronous webhook processing (single-step, no outgoing HTTP).
    ///
    /// Unlike req/res flows there is no split: the caller passes raw
    /// `EventServiceHandleRequest` proto bytes and receives encoded
    /// `EventServiceHandleResponse` bytes directly.
    #[uniffi::export]
    pub fn handle_event_transformer(request_bytes: Vec<u8>, options_bytes: Vec<u8>) -> Vec<u8> {
        use grpc_api_types::payments::EventServiceHandleRequest;
        use prost::Message as _;

        let payload = match EventServiceHandleRequest::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(IntegrationError {
                        error_message: format!("EventServiceHandleRequest decode failed: {e}"),
                        error_code: "DECODE_FAILED".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_req(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_webhook_metadata(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::IntegrationError.into(),
                    payload: Some(ffi_result::Payload::IntegrationError(e)),
                }
                .encode_to_vec()
            }
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        match crate::handlers::payments::handle_event_handler(request, environment) {
            Ok(response) => FfiResult {
                r#type: ffi_result::Type::ProtoResponse.into(),
                payload: Some(ffi_result::Payload::ProtoResponse(response.encode_to_vec())),
            }
            .encode_to_vec(),
            Err(e) => FfiResult {
                r#type: ffi_result::Type::IntegrationError.into(),
                payload: Some(ffi_result::Payload::IntegrationError(e)),
            }
            .encode_to_vec(),
        }
    }

    /// verify_redirect_response — synchronous verification of redirect response (no outgoing HTTP call).
    ///
    /// Calls `decode_redirect_response_body`, `verify_redirect_response_source`, and
    /// `process_redirect_response` on the connector, mirroring what the gRPC server does.
    #[uniffi::export]
    pub fn verify_redirect_response_transformer(
        request_bytes: Vec<u8>,
        options_bytes: Vec<u8>,
    ) -> Vec<u8> {
        use grpc_api_types::payments::PaymentServiceVerifyRedirectResponseRequest;
        use prost::Message as _;

        let payload =
            match PaymentServiceVerifyRedirectResponseRequest::decode(Bytes::from(request_bytes)) {
                Ok(p) => p,
                Err(e) => {
                    return FfiResult {
                        r#type: ffi_result::Type::ConnectorError.into(),
                        payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                            error_message: format!(
                                "PaymentServiceVerifyRedirectResponseRequest decode failed: {e}"
                            ),
                            error_code: "DECODE_FAILED".to_string(),
                            http_status_code: None,
                            error_info: None,
                        })),
                    }
                    .encode_to_vec();
                }
            };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(*e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                        error_message: e.error_message,
                        error_code: e.error_code,
                        http_status_code: None,
                        error_info: None,
                    })),
                }
                .encode_to_vec()
            }
        };

        let connector = ffi_metadata.connector;
        let connector_config = match ffi_metadata.connector_config {
            Some(config) => config,
            None => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                        error_message: "Missing connector config".to_string(),
                        error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                        http_status_code: None,
                        error_info: None,
                    })),
                }
                .encode_to_vec()
            }
        };
        let metadata = &common_utils::metadata::MaskedMetadata::default();

        let config = match ucs_env::configs::Config::new() {
            Ok(cfg) => std::sync::Arc::new(cfg),
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ConnectorError.into(),
                    payload: Some(ffi_result::Payload::ConnectorError(ConnectorError {
                        error_message: format!("Failed to load config: {e}"),
                        error_code: "CONFIG_LOAD_FAILED".to_string(),
                        http_status_code: None,
                        error_info: None,
                    })),
                }
                .encode_to_vec()
            }
        };

        match crate::services::payments::verify_redirect_response_transformer(
            payload,
            &config,
            connector,
            connector_config,
            metadata,
        ) {
            Ok(response) => {
                let response_bytes = response.encode_to_vec();
                let http_response = FfiConnectorHttpResponse {
                    status_code: 200,
                    headers: HashMap::new(),
                    body: response_bytes,
                };
                FfiResult {
                    r#type: ffi_result::Type::HttpResponse.into(),
                    payload: Some(ffi_result::Payload::HttpResponse(http_response)),
                }
                .encode_to_vec()
            }
            Err(e) => FfiResult {
                r#type: ffi_result::Type::ConnectorError.into(),
                payload: Some(ffi_result::Payload::ConnectorError(*e)),
            }
            .encode_to_vec(),
        }
    }
}

#[cfg(feature = "uniffi")]
pub use uniffi_bindings_inner::*;
