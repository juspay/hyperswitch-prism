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
    use domain_types::connector_types::ConnectorEnum;
    use domain_types::errors::{ConnectorError, ReportInto};
    use domain_types::router_data::ConnectorSpecificConfig;
    use domain_types::router_response_types::Response;
    use domain_types::utils::ForeignTryFrom;
    use error_stack::Report;
    use grpc_api_types::payments::{
        Environment, FfiConnectorHttpRequest, FfiConnectorHttpResponse, FfiOptions, RequestError,
        ResponseError,
    };
    use http::header::{HeaderMap, HeaderName, HeaderValue};
    use prost::Message;

    // ── Shared helpers ────────────────────────────────────────────────────────

    /// Build FfiMetadataPayload from FfiOptions.
    /// The connector identity is inferred from which ConnectorSpecificConfig variant is set.
    fn parse_metadata_for_req(
        options: &FfiOptions,
    ) -> Result<crate::types::FfiMetadataPayload, RequestError> {
        // 1. Resolve ConnectorSpecificConfig from FfiOptions
        let proto_config = options
            .connector_config
            .as_ref()
            .ok_or_else(|| RequestError {
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                error_message: Some("Missing connector_config".to_string()),
                error_code: None,
                status_code: Some(400),
            })?;

        // 2. Infer connector from which oneof variant is set
        let config_variant = proto_config.config.as_ref().ok_or_else(|| RequestError {
            status: grpc_api_types::payments::PaymentStatus::Pending.into(),
            error_message: Some("Missing connector_config.config".to_string()),
            error_code: None,
            status_code: Some(400),
        })?;

        let connector = ConnectorEnum::foreign_try_from(config_variant.clone()).map_err(|e| {
            <Report<domain_types::errors::ApplicationErrorResponse> as ReportInto<RequestError>>::
                report_into(e)
        })?;

        // 3. Convert proto config to domain ConnectorSpecificConfig
        let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
            .map_err(<Report<ConnectorError> as ReportInto<RequestError>>::report_into)?;

        Ok(crate::types::FfiMetadataPayload {
            connector,
            connector_config,
        })
    }

    /// Build FfiMetadataPayload from FfiOptions (for response path).
    fn parse_metadata_for_res(
        options: &FfiOptions,
    ) -> Result<crate::types::FfiMetadataPayload, ResponseError> {
        // 1. Resolve ConnectorSpecificConfig from FfiOptions
        let proto_config = options
            .connector_config
            .as_ref()
            .ok_or_else(|| ResponseError {
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                error_message: Some("Missing connector_config".to_string()),
                error_code: None,
                status_code: Some(400),
            })?;

        // 2. Infer connector from which oneof variant is set
        let config_variant = proto_config.config.as_ref().ok_or_else(|| ResponseError {
            status: grpc_api_types::payments::PaymentStatus::Pending.into(),
            error_message: Some("Missing connector_config.config".to_string()),
            error_code: None,
            status_code: Some(400),
        })?;

        let connector = ConnectorEnum::foreign_try_from(config_variant.clone()).map_err(|e| {
            <Report<domain_types::errors::ApplicationErrorResponse> as ReportInto<ResponseError>>::
                report_into(e)
        })?;

        // 3. Convert proto config to domain ConnectorSpecificConfig
        let connector_config = ConnectorSpecificConfig::foreign_try_from(proto_config.clone())
            .map_err(<Report<ConnectorError> as ReportInto<ResponseError>>::report_into)?;

        Ok(crate::types::FfiMetadataPayload {
            connector,
            connector_config,
        })
    }

    /// Helper to convert internal Request to Protobuf FfiConnectorHttpRequest bytes.
    fn build_ffi_request_bytes(request: &Request) -> Result<Vec<u8>, RequestError> {
        let mut headers = request.get_headers_map();
        let (body, boundary) = request
            .body
            .as_ref()
            .map(|b| b.get_body_bytes())
            .transpose()
            .map_err(|e| RequestError {
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
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
    fn build_domain_response(response_bytes: Vec<u8>) -> Result<Response, ResponseError> {
        let response =
            FfiConnectorHttpResponse::decode(Bytes::from(response_bytes)).map_err(|e| {
                ResponseError {
                    status: grpc_api_types::payments::PaymentStatus::Pending.into(),
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
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                error_message: Some(format!("Invalid HTTP status code: {e}")),
                error_code: None,
                status_code: Some(400),
            })?,
        })
    }

    /// Parse FfiOptions from optional bytes (for request path).
    fn parse_ffi_options_for_req(options_bytes: Vec<u8>) -> Result<FfiOptions, RequestError> {
        if options_bytes.is_empty() {
            return Err(RequestError {
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                error_message: Some("Empty options bytes".to_string()),
                error_code: None,
                status_code: Some(400),
            });
        }
        FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| RequestError {
            status: grpc_api_types::payments::PaymentStatus::Pending.into(),
            error_message: Some(format!("Options decode failed: {e}")),
            error_code: None,
            status_code: Some(400),
        })
    }

    /// Parse FfiOptions from optional bytes (for response path).
    fn parse_ffi_options_for_res(options_bytes: Vec<u8>) -> Result<FfiOptions, ResponseError> {
        if options_bytes.is_empty() {
            return Err(ResponseError {
                status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                error_message: Some("Empty options bytes".to_string()),
                error_code: None,
                status_code: Some(400),
            });
        }
        FfiOptions::decode(Bytes::from(options_bytes)).map_err(|e| ResponseError {
            status: grpc_api_types::payments::PaymentStatus::Pending.into(),
            error_message: Some(format!("Options decode failed: {e}")),
            error_code: None,
            status_code: Some(400),
        })
    }

    // ── Generic transformer runners ───────────────────────────────────────────

    /// Decode `request_bytes` as `Req`, build `FfiRequestData`, call `handler`,
    /// and encode the resulting connector HTTP request as protobuf bytes.
    /// If the handler returns an error, encode the RequestError to bytes.
    fn run_req_transformer<Req>(
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
                return RequestError {
                    status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                    error_message: Some(format!("Request payload decode failed: {e}")),
                    error_code: None,
                    status_code: Some(400),
                }
                .encode_to_vec()
            }
        };

        let ffi_options = match parse_ffi_options_for_req(options_bytes) {
            Ok(o) => o,
            Err(e) => return e.encode_to_vec(),
        };

        let ffi_metadata = match parse_metadata_for_req(&ffi_options) {
            Ok(m) => m,
            Err(e) => return e.encode_to_vec(),
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        let result = match handler(request, environment) {
            Ok(r) => r,
            Err(e) => return e.encode_to_vec(),
        };

        let connector_request = match result {
            Some(r) => r,
            None => {
                return RequestError {
                    status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                    error_message: Some("Request encoding failed".to_string()),
                    error_code: None,
                    status_code: Some(400),
                }
                .encode_to_vec()
            }
        };

        match build_ffi_request_bytes(&connector_request) {
            Ok(bytes) => bytes,
            Err(e) => e.encode_to_vec(),
        }
    }

    /// Decode `response_bytes` as the domain `Response` and `request_bytes` as `Req`,
    /// call `handler`, and encode the result as protobuf bytes.
    /// If the handler returns an error, encode the ResponseError to bytes.
    fn run_res_transformer<Req, Res>(
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
            Err(e) => return e.encode_to_vec(),
        };

        let payload = match Req::decode(Bytes::from(request_bytes)) {
            Ok(p) => p,
            Err(e) => {
                return ResponseError {
                    status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                    error_message: Some(format!("Request payload decode failed: {e}")),
                    error_code: None,
                    status_code: Some(400),
                }
                .encode_to_vec()
            }
        };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => return e.encode_to_vec(),
        };

        let ffi_metadata = match parse_metadata_for_res(&ffi_options) {
            Ok(m) => m,
            Err(e) => return e.encode_to_vec(),
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        match handler(request, domain_response, environment) {
            Ok(proto_response) => proto_response.encode_to_vec(),
            Err(e) => e.encode_to_vec(),
        }
    }

    // ── Flow macro ────────────────────────────────────────────────────────────

    /// Generates a `#[uniffi::export]` `{flow}_req_transformer` and
    /// `{flow}_res_transformer` function pair backed by the generic runners.
    ///
    /// # Arguments
    /// - `$flow`        — snake_case flow name (used as identifier prefix)
    /// - `$req_type`    — protobuf request type to decode from bytes
    /// - `$req_handler` — handler fn: `(FfiRequestData<Req>, Option<Environment>) -> Result<Option<Request>, RequestError>`
    /// - `$res_handler` — handler fn: `(FfiRequestData<Req>, Response, Option<Environment>) -> Result<Res, ResponseError>`
    macro_rules! define_ffi_flow {
        ($flow:ident, $req_type:ty, $req_handler:path, $res_handler:path) => {
            paste::paste! {
                #[uniffi::export]
                pub fn [<$flow _req_transformer>](
                    request_bytes: Vec<u8>,
                    options_bytes: Vec<u8>,
                ) -> Vec<u8> {
                    run_req_transformer::<$req_type>(
                        request_bytes,
                        options_bytes,
                        $req_handler,
                    )
                }

                #[uniffi::export]
                pub fn [<$flow _res_transformer>](
                    response_bytes: Vec<u8>,
                    request_bytes: Vec<u8>,
                    options_bytes: Vec<u8>,
                ) -> Vec<u8> {
                    run_res_transformer::<$req_type, _>(
                        response_bytes,
                        request_bytes,
                        options_bytes,
                        $res_handler,
                    )
                }
            }
        };
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
        use prost::Message as _;

        let payload = match grpc_api_types::payments::EventServiceHandleRequest::decode(
            Bytes::from(request_bytes),
        ) {
            Ok(p) => p,
            Err(e) => {
                return ResponseError {
                    status: grpc_api_types::payments::PaymentStatus::Pending.into(),
                    error_message: Some(format!("EventServiceHandleRequest decode failed: {e}")),
                    error_code: None,
                    status_code: Some(400),
                }
                .encode_to_vec()
            }
        };

        let ffi_options = match parse_ffi_options_for_res(options_bytes) {
            Ok(o) => o,
            Err(e) => return e.encode_to_vec(),
        };

        let ffi_metadata = match parse_metadata_for_res(&ffi_options) {
            Ok(m) => m,
            Err(e) => return e.encode_to_vec(),
        };

        let request = crate::types::FfiRequestData {
            payload,
            extracted_metadata: ffi_metadata,
            masked_metadata: None,
        };

        let environment = Some(ffi_options.environment());

        match crate::handlers::payments::handle_event_handler(request, environment) {
            Ok(response) => response.encode_to_vec(),
            Err(e) => e.encode_to_vec(),
        }
    }
}

#[cfg(feature = "uniffi")]
pub use uniffi_bindings_inner::*;
