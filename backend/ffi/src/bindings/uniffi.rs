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
        ffi_result, FfiConnectorHttpRequest, FfiConnectorHttpResponse, FfiResult, PaymentStatus,
        RequestError, ResponseError,
    };
    use prost::Message;
    use std::collections::HashMap;

    use crate::bindings::utils::{
        build_domain_response, build_ffi_request_bytes, parse_ffi_options_for_req,
        parse_ffi_options_for_res, parse_metadata_for_req, parse_metadata_for_res,
    };
    use crate::define_ffi_flow;

    // ── Generic transformer runners ───────────────────────────────────────────

    /// Decode `request_bytes` as `Req`, build `FfiRequestData`, call `handler`,
    /// and encode the resulting connector HTTP request as Result proto.
    /// If the handler returns an error, encode the RequestError in Result.
    pub fn run_req_transformer<Req>(
        request_bytes: Vec<u8>,
        options_bytes: Vec<u8>,
        handler: impl Fn(
            crate::types::FfiRequestData<Req>,
            Option<Environment>,
        ) -> Result<Option<Request>, RequestError>,
    ) -> Vec<u8>
    where
        Req: Message + Default,
    {
        let payload = match Req::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(RequestError {
                        status: PaymentStatus::Pending.into(),
                        error_message: Some(format!("Request payload decode failed: {e}")),
                        error_code: None,
                        status_code: Some(400),
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_req(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata_for_req(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(e)),
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
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(e)),
                }
                .encode_to_vec()
            }
        };

        let connector_request = match result {
            Some(r) => r,
            None => {
                return FfiResult {
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(RequestError {
                        status: PaymentStatus::Pending.into(),
                        error_message: Some("Request encoding failed".to_string()),
                        error_code: None,
                        status_code: Some(400),
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
                    r#type: ffi_result::Type::RequestError.into(),
                    payload: Some(ffi_result::Payload::RequestError(RequestError {
                        status: PaymentStatus::Pending.into(),
                        error_message: Some(format!("Request re-decode failed: {e}")),
                        error_code: None,
                        status_code: Some(500),
                    })),
                }
                .encode_to_vec(),
            },
            Err(e) => FfiResult {
                r#type: ffi_result::Type::RequestError.into(),
                payload: Some(ffi_result::Payload::RequestError(e)),
            }
            .encode_to_vec(),
        }
    }

    /// Decode `response_bytes` as the domain `Response` and `request_bytes` as `Req`,
    /// call `handler`, and encode the result as Result proto.
    /// If the handler returns an error, encode the ResponseError in Result.
    pub fn run_res_transformer<Req, Res>(
        response_bytes: Vec<u8>,
        request_bytes: Vec<u8>,
        options_bytes: Vec<u8>,
        handler: impl Fn(
            crate::types::FfiRequestData<Req>,
            Response,
            Option<Environment>,
        ) -> Result<Res, ResponseError>,
    ) -> Vec<u8>
    where
        Req: Message + Default,
        Res: Message,
    {
        let domain_response = match build_domain_response(response_bytes) {
            Ok(r) => r,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(e)),
                }
                .encode_to_vec()
            }
        };

        let payload = match Req::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(ResponseError {
                        status: PaymentStatus::Pending.into(),
                        error_message: Some(format!("Request payload decode failed: {e}")),
                        error_code: None,
                        status_code: Some(400),
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata_for_res(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(e)),
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
                r#type: ffi_result::Type::ResponseError.into(),
                payload: Some(ffi_result::Payload::ResponseError(e)),
            }
            .encode_to_vec(),
        }
    }

    // ── Flow registrations (auto-generated) ──────────────────────────────────
    // To add a new flow: implement req_transformer!/res_transformer! in
    // services/payments.rs, then run `make generate` to regenerate this file.

    include!("_generated_ffi_flows.rs");

    // ── Hand-written exports (not auto-generated) ─────────────────────────────

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
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(ResponseError {
                        status: PaymentStatus::Pending.into(),
                        error_message: Some(format!(
                            "EventServiceHandleRequest decode failed: {e}"
                        )),
                        error_code: None,
                        status_code: Some(400),
                    })),
                }
                .encode_to_vec();
            }
        };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(e)),
                }
                .encode_to_vec()
            }
        };

        let ffi_metadata = match parse_metadata_for_res(&ffi_options) {
            Ok(m) => m,
            Err(e) => {
                return FfiResult {
                    r#type: ffi_result::Type::ResponseError.into(),
                    payload: Some(ffi_result::Payload::ResponseError(e)),
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
            Ok(response) => {
                // Serialize the protobuf response and wrap it in FfiConnectorHttpResponse
                // Note: handle_event doesn't have connector response headers (webhook processing)
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
                r#type: ffi_result::Type::ResponseError.into(),
                payload: Some(ffi_result::Payload::ResponseError(e)),
            }
            .encode_to_vec(),
        }
    }
}

#[cfg(feature = "uniffi")]
pub use uniffi_bindings_inner::*;
