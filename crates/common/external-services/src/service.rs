use std::{collections::HashMap, str::FromStr, sync::RwLock, time::Duration};

use base64::Engine;
use common_enums::ApiClientError;
#[cfg(feature = "injector-client")]
use common_utils::{
    consts::{X_API_TAG, X_API_URL, X_SESSION_ID},
    events::{EventStage, MaskedSerdeValue},
};
use common_utils::{
    ext_traits::AsyncExt,
    lineage,
    request::{Method, Request, RequestContent},
};
use domain_types::{
    connector_types::{ConnectorResponseHeaders, RawConnectorRequestResponse},
    errors::ApiErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Proxy,
    ConnectorError,
};
#[cfg(feature = "injector-client")]
use domain_types::{
    errors::{
        report_common_api_client_to_flow, report_connector_request_to_flow,
        report_connector_response_to_flow, ConnectorFlowError, ResponseTransformationErrorContext,
    },
    IntegrationError,
};
use hyperswitch_masking::{ExposeInterface, Secret};
#[cfg(feature = "injector-client")]
use injector;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Test context for mock server integration
#[derive(Debug, Clone)]
pub struct TestContext {
    pub session_id: String,
    pub mock_server_url: String,
}

pub trait ConnectorRequestReference {
    fn get_connector_request_reference_id(&self) -> &str;
}

pub trait AdditionalHeaders {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>>;
}

impl ConnectorRequestReference for domain_types::connector_types::PaymentFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl ConnectorRequestReference for domain_types::connector_types::VerifyWebhookSourceFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders for domain_types::connector_types::VerifyWebhookSourceFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        None
    }
}

impl AdditionalHeaders for domain_types::connector_types::PaymentFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        self.vault_headers.as_ref()
    }
}

impl ConnectorRequestReference for domain_types::connector_types::RefundFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders for domain_types::connector_types::RefundFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        // RefundFlowData might not have vault_headers, so return None
        None
    }
}

impl ConnectorRequestReference for domain_types::connector_types::DisputeFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders for domain_types::connector_types::DisputeFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        // DisputeFlowData might not have vault_headers, so return None
        None
    }
}

impl ConnectorRequestReference for domain_types::payouts::payouts_types::PayoutFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders for domain_types::payouts::payouts_types::PayoutFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        None
    }
}
use common_utils::events::{Event, EventConfig, FlowName};
#[cfg(feature = "injector-client")]
// TokenData is now imported from hyperswitch_injector
use common_utils::{consts, emit_event_with_config};
use error_stack::{report, ResultExt};
#[cfg(feature = "injector-client")]
use hyperswitch_masking::ErasedMaskSerialize;
use hyperswitch_masking::Maskable;
#[cfg(feature = "injector-client")]
use injector::{injector_core, HttpMethod, TokenData};
use interfaces::connector_integration_v2::BoxedConnectorIntegrationV2;
#[cfg(feature = "injector-client")]
use interfaces::integrity::{CheckIntegrity, FlowIntegrity, GetIntegrityObject};
use once_cell::sync::OnceCell;
use reqwest::Client;
use serde_json::json;
#[cfg(feature = "injector-client")]
use tracing::field::Empty;

use crate::shared_metrics as metrics;
pub type Headers = std::collections::HashSet<(String, Maskable<String>)>;

/// Handles the connector response, processing both successful and error responses
#[allow(clippy::too_many_arguments)]
pub fn handle_connector_response<F, ResourceCommonData, Req, Resp>(
    response: CustomResult<Result<Response, Response>, ConnectorError>,
    mut updated_router_data: RouterDataV2<F, ResourceCommonData, Req, Resp>,
    connector: &BoxedConnectorIntegrationV2<'static, F, ResourceCommonData, Req, Resp>,
    mut event: Option<&mut Event>,
    all_keys_required: Option<bool>,
    method: Method,
    url: String,
    event_params: Option<&EventProcessingParams<'_>>,
) -> CustomResult<RouterDataV2<F, ResourceCommonData, Req, Resp>, ConnectorError>
where
    F: Clone + 'static,
    Req: Clone + 'static + std::fmt::Debug,
    Resp: Clone + 'static + std::fmt::Debug,
    ResourceCommonData: Clone + RawConnectorRequestResponse + ConnectorResponseHeaders,
{
    match response {
        Ok(body) => {
            let response = match body {
                Ok(body) => {
                    let status_code = body.status_code;
                    tracing::Span::current()
                        .record("status_code", tracing::field::display(status_code));

                    if all_keys_required.unwrap_or(true) {
                        let raw_response_string = strip_bom_and_convert_to_string(&body.response);
                        updated_router_data
                            .resource_common_data
                            .set_raw_connector_response(raw_response_string.map(Into::into));

                        // Set response headers if available
                        updated_router_data
                            .resource_common_data
                            .set_connector_response_headers(body.headers.clone());
                    }

                    let handle_response_result = connector.handle_response_v2(
                        &updated_router_data,
                        event.as_deref_mut(),
                        body.clone(),
                    );

                    // Log response body and headers using properly masked data from connector
                    if let Some(evt) = event.as_deref_mut() {
                        if let Some(response_data) = &evt.response_data {
                            tracing::Span::current().record(
                                "response.body",
                                tracing::field::display(response_data.inner()),
                            );
                        }

                        // Log response headers from event (already masked)
                        tracing::Span::current()
                            .record("response.headers", tracing::field::debug(&evt.headers));
                    }

                    handle_response_result?
                }
                Err(body) => {
                    // Record metrics only if event_params is provided
                    if let Some(params) = event_params {
                        metrics::EXTERNAL_SERVICE_API_CALLS_ERRORS
                            .with_label_values(&[
                                &method.to_string(),
                                params.service_name,
                                params.connector_name,
                                body.status_code.to_string().as_str(),
                            ])
                            .inc();
                    }

                    if all_keys_required.unwrap_or(true) {
                        let raw_response_string = strip_bom_and_convert_to_string(&body.response);
                        updated_router_data
                            .resource_common_data
                            .set_raw_connector_response(raw_response_string.map(Into::into));
                        updated_router_data
                            .resource_common_data
                            .set_connector_response_headers(body.headers.clone());
                    }

                    let error_response = match body.status_code {
                        500..=511 => {
                            connector.get_5xx_error_response(body.clone(), event.as_deref_mut())?
                        }
                        _ => connector.get_error_response_v2(body.clone(), event.as_deref_mut())?,
                    };
                    if let Some(evt) = event {
                        evt.set_error_response(&error_response);
                    }
                    tracing::Span::current().record(
                        "response.error_message",
                        tracing::field::display(&error_response.message),
                    );
                    tracing::Span::current().record(
                        "response.status_code",
                        tracing::field::display(error_response.status_code),
                    );
                    Err(error_stack::report!(
                        ConnectorError::ConnectorErrorResponse(error_response)
                    ))?
                }
            };
            Ok(response)
        }
        Err(err) => {
            tracing::Span::current().record("url", tracing::field::display(url));
            Err(err)
        }
    }
}

#[cfg(feature = "injector-client")]
trait ToHttpMethod {
    fn to_http_method(&self) -> HttpMethod;
}

#[cfg(feature = "injector-client")]
impl ToHttpMethod for Method {
    fn to_http_method(&self) -> HttpMethod {
        match self {
            Self::Get => HttpMethod::GET,
            Self::Post => HttpMethod::POST,
            Self::Put => HttpMethod::PUT,
            Self::Patch => HttpMethod::PATCH,
            Self::Delete => HttpMethod::DELETE,
        }
    }
}

#[derive(Debug)]
pub struct EventProcessingParams<'a> {
    pub connector_name: &'a str,
    pub service_name: &'a str,
    pub service_type: &'a str,
    pub flow_name: FlowName,
    pub event_config: &'a EventConfig,
    pub request_id: &'a str,
    pub lineage_ids: &'a lineage::LineageIds<'a>,
    pub reference_id: &'a Option<String>,
    pub resource_id: &'a Option<String>,
    pub shadow_mode: bool,
}

#[cfg(feature = "injector-client")]
#[tracing::instrument(
    name = "execute_connector_processing_step",
    skip_all,
    fields(
        request.headers = Empty,
        request.body = Empty,
        request.url = Empty,
        request.method = Empty,
        response.body = Empty,
        response.headers = Empty,
        response.error_message = Empty,
        response.status_code = Empty,
        message_ = "Golden Log Line (outgoing)",
        latency = Empty,
    )
)]
#[allow(clippy::too_many_arguments)]
pub async fn execute_connector_processing_step<T, F, ResourceCommonData, Req, Resp>(
    proxy: &Proxy,
    connector: BoxedConnectorIntegrationV2<'static, F, ResourceCommonData, Req, Resp>,
    router_data: RouterDataV2<F, ResourceCommonData, Req, Resp>,
    all_keys_required: Option<bool>,
    event_params: EventProcessingParams<'_>,
    token_data: Option<TokenData>,
    call_connector_action: common_enums::CallConnectorAction,
    test_context: Option<TestContext>,
    api_tag: Option<String>,
) -> CustomResult<RouterDataV2<F, ResourceCommonData, Req, Resp>, ConnectorFlowError>
where
    F: Clone + 'static,
    T: FlowIntegrity,
    Req: Clone + 'static + std::fmt::Debug + GetIntegrityObject<T> + CheckIntegrity<Req, T>,
    Resp: Clone + 'static + std::fmt::Debug,
    ResourceCommonData: Clone
        + 'static
        + RawConnectorRequestResponse
        + ConnectorResponseHeaders
        + ConnectorRequestReference
        + AdditionalHeaders,
{
    let start = tokio::time::Instant::now();
    let result = match call_connector_action {
        common_enums::CallConnectorAction::HandleResponse(res) => {
            let body = Response {
                headers: None,
                response: res.into(),
                status_code: 200,
            };

            let status_code = body.status_code;
            tracing::Span::current().record("status_code", tracing::field::display(status_code));
            if let Ok(response) = parse_json_with_bom_handling(&body.response) {
                tracing::Span::current().record(
                    "response.body",
                    tracing::field::display(response.masked_serialize().unwrap_or(
                        json!({ "error": "failed to mask serialize connector response"}),
                    )),
                );
            }

            // Set raw_connector_response BEFORE calling the transformer
            let mut updated_router_data = router_data.clone();
            if all_keys_required.unwrap_or(true) {
                let raw_response_string = strip_bom_and_convert_to_string(&body.response);
                updated_router_data
                    .resource_common_data
                    .set_raw_connector_response(raw_response_string.map(Into::into));
            }

            let handle_response_result =
                connector.handle_response_v2(&updated_router_data, None, body.clone());

            let response = match handle_response_result {
                Ok(data) => {
                    tracing::info!("Transformer completed successfully");
                    Ok(data)
                }
                Err(err) => Err(err),
            }
            .map_err(report_connector_response_to_flow)?;

            Ok(response)
        }
        common_enums::CallConnectorAction::Trigger => {
            let mut connector_request = connector
                .build_request_v2(&router_data.clone())
                .map_err(report_connector_request_to_flow)?;

            let mut updated_router_data = router_data.clone();
            updated_router_data = match &connector_request {
                Some(request) => {
                    updated_router_data
                        .resource_common_data
                        .set_raw_connector_request(Some(
                            extract_raw_connector_request(request).into(),
                        ));
                    updated_router_data
                }
                None => updated_router_data,
            };
            connector_request = connector_request.map(|mut req| {
                if event_params.shadow_mode {
                    req.add_header(
                        consts::X_REQUEST_ID,
                        Maskable::Masked(Secret::new(event_params.request_id.to_string())),
                    );
                    req.add_header(
                        consts::X_SOURCE_NAME,
                        Maskable::Masked(Secret::new(consts::X_CONNECTOR_SERVICE.to_string())),
                    );
                    req.add_header(
                        consts::X_FLOW_NAME,
                        Maskable::Masked(Secret::new(event_params.flow_name.to_string())),
                    );

                    req.add_header(
                        consts::X_CONNECTOR_NAME,
                        Maskable::Masked(Secret::new(event_params.connector_name.to_string())),
                    );
                }
                req
            });

            // Apply test environment modifications if test context is provided
            connector_request = connector_request.map(|mut req| {
                test_context.as_ref().map(|test_ctx| {
                    // Store original URL for x-api-url header
                    let original_url = req.url.clone();

                    // Replace URL with mock server URL
                    req.url = test_ctx.mock_server_url.clone();

                    // Add test headers
                    req.add_header(X_API_URL, original_url.clone().into());
                    req.add_header(X_SESSION_ID, test_ctx.session_id.clone().into());

                    // Add API tag if provided
                    api_tag.as_ref().map(|tag| {
                        req.add_header(X_API_TAG, tag.clone().into());
                    });

                    tracing::info!(
                        "Test mode enabled: redirected {} to {}",
                        original_url,
                        test_ctx.mock_server_url
                    );
                });
                req
            });

            let headers = connector_request
                .as_ref()
                .map(|connector_request| connector_request.headers.clone())
                .unwrap_or_default();
            tracing::info!(?headers, "headers of connector request");

            let event_headers: HashMap<String, String> = headers
                .iter()
                .map(|(k, v)| (k.clone(), format!("{v:?}")))
                .collect();

            let masked_headers = headers
                .iter()
                .fold(serde_json::Map::new(), |mut acc, (k, v)| {
                    let value = match v {
                        Maskable::Masked(_) => {
                            serde_json::Value::String("*** alloc::string::String ***".to_string())
                        }
                        Maskable::Normal(iv) => serde_json::Value::String(iv.to_owned()),
                    };
                    acc.insert(k.clone(), value);
                    acc
                });
            let headers = serde_json::Value::Object(masked_headers);
            tracing::Span::current().record("request.headers", tracing::field::display(&headers));

            let req = connector_request.as_ref().map(|connector_request| {
                let masked_request = match connector_request.body.as_ref() {
                    Some(request) => match request {
                        RequestContent::Json(i)
                        | RequestContent::FormUrlEncoded(i)
                        | RequestContent::Xml(i) => (**i).masked_serialize().unwrap_or(
                            json!({ "error": "failed to mask serialize connector request"}),
                        ),
                        RequestContent::FormData(_) => json!({"request_type": "FORM_DATA"}),
                        RequestContent::RawBytes(_) => json!({"request_type": "RAW_BYTES"}),
                    },
                    None => serde_json::Value::Null,
                };
                tracing::info!(request=?masked_request, "request of connector");
                tracing::Span::current()
                    .record("request.body", tracing::field::display(&masked_request));

                masked_request
            });

            match connector_request {
                Some(request) => {
                    let url = request.url.clone();
                    let method = request.method;
                    metrics::EXTERNAL_SERVICE_TOTAL_API_CALLS
                        .with_label_values(&[
                            &method.to_string(),
                            event_params.service_name,
                            event_params.connector_name,
                        ])
                        .inc();
                    let external_service_start_latency = tokio::time::Instant::now();
                    tracing::Span::current().record("request.url", tracing::field::display(&url));
                    tracing::Span::current()
                        .record("request.method", tracing::field::display(method));
                    let request_id = event_params.request_id.to_string();

                    let response = if let Some(token_data) = token_data {
                        tracing::debug!(
                            "Creating injector request with token data using unified API"
                        );

                        // Extract template and combine headers
                        let template = request
                            .body
                            .as_ref()
                            .ok_or(ConnectorFlowError::from(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            ))?
                            .get_inner_value()
                            .expose()
                            .to_string();

                        let headers = request
                            .headers
                            .iter()
                            .map(|(key, value)| {
                                (
                                    key.clone(),
                                    Secret::new(match value {
                                        Maskable::Normal(val) => val.clone(),
                                        Maskable::Masked(val) => val.clone().expose().to_string(),
                                    }),
                                )
                            })
                            .chain(
                                updated_router_data
                                    .resource_common_data
                                    .get_vault_headers()
                                    .map(|headers| {
                                        headers.iter().map(|(k, v)| (k.clone(), v.clone()))
                                    })
                                    .into_iter()
                                    .flatten(),
                            )
                            .collect();

                        // Create injector request
                        let injector_request = injector::InjectorRequest::new(
                            request.url.clone(),
                            request.method.to_http_method(),
                            template,
                            token_data,
                            Some(headers),
                            proxy
                                .https_url
                                .as_ref()
                                .or(proxy.http_url.as_ref())
                                .map(|url| Secret::new(url.clone())),
                            None,
                            None,
                            None,
                        );

                        // New injector handles HTTP request internally and returns enhanced response
                        let injector_response =
                            injector_core(injector_request).await.change_context(
                                ConnectorFlowError::from(IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                }),
                            )?;

                        // Convert injector response to connector service Response format
                        let response_bytes = serde_json::to_vec(&injector_response.response)
                            .map_err(|_| {
                                ConnectorFlowError::from(
                                    ConnectorError::response_handling_failed_http_status_unknown(),
                                )
                            })?;

                        // Convert headers from HashMap<String, String> to reqwest::HeaderMap if present
                        let headers = injector_response.headers.map(|h| {
                            let mut header_map = reqwest::header::HeaderMap::new();
                            for (key, value) in h {
                                if let (Ok(header_name), Ok(header_value)) = (
                                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                                    reqwest::header::HeaderValue::from_str(&value),
                                ) {
                                    header_map.insert(header_name, header_value);
                                }
                            }
                            header_map
                        });

                        Ok(Ok(Response {
                            headers,
                            response: response_bytes.into(),
                            status_code: injector_response.status_code, // Use actual status code from connector
                        }))
                    } else {
                        let test_mode = test_context.is_some();
                        call_connector_api(
                            proxy,
                            request,
                            "execute_connector_processing_step",
                            test_mode,
                        )
                        .await
                        .map_err(report_common_api_client_to_flow)
                        .inspect_err(|err| {
                            info_log(
                                "NETWORK_ERROR",
                                &json!(format!(
                                    "Failed getting response from connector. Error: {:?}",
                                    err
                                )),
                            );
                        })
                    };
                    let external_service_elapsed = external_service_start_latency.elapsed();
                    metrics::EXTERNAL_SERVICE_API_CALLS_LATENCY
                        .with_label_values(&[
                            &method.to_string(),
                            event_params.service_name,
                            event_params.connector_name,
                        ])
                        .observe(external_service_elapsed.as_secs_f64());
                    // Extract status code BEFORE creating event - one liner
                    let status_code = response.as_ref().ok().map(|result| match result {
                        Ok(body) | Err(body) => i32::from(body.status_code),
                    });

                    // Construct masked request for event
                    let masked_request_data = req.as_ref().and_then(|r| {
                        MaskedSerdeValue::from_masked_optional(r, "connector_request")
                    });

                    let latency =
                        u64::try_from(external_service_elapsed.as_millis()).unwrap_or(u64::MAX);

                    // Create single event (response_data will be set by connector)
                    let mut event = Event {
                        request_id: request_id.to_string(),
                        timestamp: chrono::Utc::now().timestamp().into(),
                        flow_type: event_params.flow_name,
                        connector: event_params.connector_name.to_string(),
                        url: Some(url.clone()),
                        stage: EventStage::ConnectorCall,
                        latency_ms: Some(latency),
                        status_code,
                        request_data: masked_request_data,
                        response_data: None, // Will be set by connector via set_response_body
                        error: None,         // Will be set on error responses
                        headers: event_headers,
                        additional_fields: HashMap::new(),
                        lineage_ids: event_params.lineage_ids.to_owned(),
                    };
                    event.add_reference_id(event_params.reference_id.as_deref());
                    event.add_resource_id(event_params.resource_id.as_deref());
                    event.add_service_type(event_params.service_type);
                    event.add_service_name(event_params.service_name);

                    let result = handle_connector_response(
                        response.change_context(
                            ConnectorError::response_handling_failed_http_status_unknown(),
                        ),
                        updated_router_data,
                        &connector,
                        Some(&mut event),
                        all_keys_required,
                        method,
                        url.clone(),
                        Some(&event_params),
                    )
                    .map_err(report_connector_response_to_flow);

                    emit_event_with_config(event, event_params.event_config);
                    result
                }
                None => Ok(router_data),
            }
        }
    };

    let result_with_integrity_check = match result {
        Ok(data) => {
            data.request
                .check_integrity(&data.request.clone(), None)
                .map_err(|err| {
                    report_connector_response_to_flow(error_stack::report!(
                        ConnectorError::IntegrityCheckFailed {
                            context: ResponseTransformationErrorContext {
                                http_status_code: None,
                                additional_context: None,
                            },
                            field_names: err.field_names,
                            connector_transaction_id: err.connector_transaction_id,
                        }
                    ))
                })?;
            Ok(data)
        }
        Err(err) => Err(err),
    };

    let elapsed = start.elapsed().as_millis();
    tracing::Span::current().record("latency", elapsed);
    tracing::info!(tag = ?Tag::OutgoingApi, log_type = "api", "Outgoing Request completed");
    result_with_integrity_check
}

pub enum ApplicationResponse<R> {
    Json(R),
}

pub type CustomResult<T, E> = error_stack::Result<T, E>;
pub type RouterResult<T> = CustomResult<T, ApiErrorResponse>;
pub type RouterResponse<T> = CustomResult<ApplicationResponse<T>, ApiErrorResponse>;

pub async fn call_connector_api(
    proxy: &Proxy,
    request: Request,
    _flow_name: &str,
    test_mode: bool,
) -> CustomResult<Result<Response, Response>, ApiClientError> {
    let url =
        reqwest::Url::parse(&request.url).change_context(ApiClientError::UrlEncodingFailed)?;

    let should_bypass_proxy = proxy.bypass_proxy_urls.contains(&url.to_string());

    let client = create_client(
        proxy,
        should_bypass_proxy,
        request.certificate,
        request.certificate_key,
        test_mode,
    )?;

    let headers = request.headers.construct_header_map()?;

    // Process and log the request body based on content type
    let request = {
        match request.method {
            Method::Get => client.get(url),
            Method::Post => {
                let client = client.post(url);
                match request.body {
                    Some(RequestContent::Json(payload)) => client.json(&payload),
                    Some(RequestContent::FormUrlEncoded(payload)) => client.form(&payload),
                    Some(RequestContent::Xml(payload)) => {
                        // For XML content, we need to extract the XML string properly
                        // The payload implements a custom Serialize that generates XML content
                        let body = serde_json::to_string(&payload)
                            .change_context(ApiClientError::UrlEncodingFailed)?;

                        // Properly deserialize the JSON string to extract clean XML
                        let xml_body = if body.starts_with('"') && body.ends_with('"') {
                            // This is a JSON-encoded string, deserialize it properly
                            serde_json::from_str::<String>(&body)
                                .change_context(ApiClientError::UrlEncodingFailed)?
                        } else {
                            // This is already the raw body content
                            body
                        };
                        client.body(xml_body).header("Content-Type", "text/xml")
                    }
                    Some(RequestContent::FormData(data)) => {
                        let (bytes, boundary) = data
                            .render_as_bytes()
                            .change_context(ApiClientError::BodySerializationFailed)?;
                        client.body(bytes).header(
                            "Content-Type",
                            format!("multipart/form-data; boundary={}", boundary),
                        )
                    }
                    Some(RequestContent::RawBytes(payload)) => client.body(payload),
                    _ => client,
                }
            }
            Method::Put => {
                let client = client.put(url);
                match request.body {
                    Some(RequestContent::Json(payload)) => client.json(&payload),
                    Some(RequestContent::FormUrlEncoded(payload)) => client.form(&payload),
                    Some(RequestContent::Xml(payload)) => {
                        let body = serde_json::to_string(&payload)
                            .change_context(ApiClientError::UrlEncodingFailed)?;
                        let xml_body = if body.starts_with('"') && body.ends_with('"') {
                            serde_json::from_str::<String>(&body)
                                .change_context(ApiClientError::UrlEncodingFailed)?
                        } else {
                            body
                        };
                        client.body(xml_body).header("Content-Type", "text/xml")
                    }
                    Some(RequestContent::FormData(data)) => {
                        let (bytes, boundary) = data
                            .render_as_bytes()
                            .change_context(ApiClientError::BodySerializationFailed)?;
                        client.body(bytes).header(
                            "Content-Type",
                            format!("multipart/form-data; boundary={}", boundary),
                        )
                    }
                    Some(RequestContent::RawBytes(payload)) => client.body(payload),
                    _ => client,
                }
            }
            Method::Patch => {
                let client = client.patch(url);
                match request.body {
                    Some(RequestContent::Json(payload)) => client.json(&payload),
                    Some(RequestContent::FormUrlEncoded(payload)) => client.form(&payload),
                    Some(RequestContent::Xml(payload)) => {
                        let body = serde_json::to_string(&payload)
                            .change_context(ApiClientError::UrlEncodingFailed)?;
                        let xml_body = if body.starts_with('"') && body.ends_with('"') {
                            serde_json::from_str::<String>(&body)
                                .change_context(ApiClientError::UrlEncodingFailed)?
                        } else {
                            body
                        };
                        client.body(xml_body).header("Content-Type", "text/xml")
                    }
                    Some(RequestContent::FormData(data)) => {
                        let (bytes, boundary) = data
                            .render_as_bytes()
                            .change_context(ApiClientError::BodySerializationFailed)?;
                        client.body(bytes).header(
                            "Content-Type",
                            format!("multipart/form-data; boundary={}", boundary),
                        )
                    }
                    Some(RequestContent::RawBytes(payload)) => client.body(payload),
                    _ => client,
                }
            }
            Method::Delete => {
                let client = client.delete(url);
                match request.body {
                    Some(RequestContent::Json(payload)) => client.json(&payload),
                    Some(RequestContent::FormUrlEncoded(payload)) => client.form(&payload),
                    Some(RequestContent::Xml(payload)) => {
                        let body = serde_json::to_string(&payload)
                            .change_context(ApiClientError::UrlEncodingFailed)?;
                        let xml_body = if body.starts_with('"') && body.ends_with('"') {
                            serde_json::from_str::<String>(&body)
                                .change_context(ApiClientError::UrlEncodingFailed)?
                        } else {
                            body
                        };
                        client.body(xml_body).header("Content-Type", "text/xml")
                    }
                    Some(RequestContent::FormData(data)) => {
                        let (bytes, boundary) = data
                            .render_as_bytes()
                            .change_context(ApiClientError::BodySerializationFailed)?;
                        client.body(bytes).header(
                            "Content-Type",
                            format!("multipart/form-data; boundary={}", boundary),
                        )
                    }
                    Some(RequestContent::RawBytes(payload)) => client.body(payload),
                    _ => client,
                }
            }
        }
        .add_headers(headers)
    };
    let send_request = async {
        request.send().await.map_err(|error| {
            let api_error = match error {
                error if error.is_timeout() => ApiClientError::RequestTimeoutReceived,
                _ => ApiClientError::RequestNotSent(error.to_string()),
            };
            info_log(
                "REQUEST_FAILURE",
                &json!(format!("Unable to send request to connector.",)),
            );
            report!(api_error)
        })
    };

    let response = send_request.await;

    handle_response(response).await
}

pub fn create_client(
    proxy_config: &Proxy,
    should_bypass_proxy: bool,
    client_certificate: Option<Secret<String>>,
    client_certificate_key: Option<Secret<String>>,
    test_mode: bool,
) -> CustomResult<Client, ApiClientError> {
    match (client_certificate.clone(), client_certificate_key.clone()) {
        (Some(encoded_certificate), Some(encoded_certificate_key)) => {
            let client_builder = get_client_builder(proxy_config, should_bypass_proxy, test_mode)?;

            let identity = create_identity_from_certificate_and_key(
                encoded_certificate.clone(),
                encoded_certificate_key,
            )?;
            let certificate_list = create_certificate(encoded_certificate)?;
            let client_builder = certificate_list
                .into_iter()
                .fold(client_builder, |client_builder, certificate| {
                    client_builder.add_root_certificate(certificate)
                });
            client_builder
                .identity(identity)
                .use_rustls_tls()
                .build()
                .change_context(ApiClientError::ClientConstructionFailed)
                .attach_printable("Failed to construct client with certificate and certificate key")
        }
        _ => get_base_client(proxy_config, should_bypass_proxy, test_mode),
    }
}

static DEFAULT_CLIENT: OnceCell<Client> = OnceCell::new();
static PROXY_CLIENT_CACHE: OnceCell<RwLock<HashMap<Proxy, Client>>> = OnceCell::new();

fn get_or_create_proxy_client(
    cache: &RwLock<HashMap<Proxy, Client>>,
    cache_key: Proxy,
    proxy_config: &Proxy,
    should_bypass_proxy: bool,
    test_mode: bool,
) -> CustomResult<Client, ApiClientError> {
    let read_result = cache
        .read()
        .ok()
        .and_then(|read_lock| read_lock.get(&cache_key).cloned());

    let client = match read_result {
        Some(cached_client) => {
            tracing::debug!("Retrieved cached proxy client for config: {:?}", cache_key);
            cached_client
        }
        None => {
            let mut write_lock = cache
                .try_write()
                .map_err(|_| ApiClientError::ClientConstructionFailed)?;

            match write_lock.get(&cache_key) {
                Some(cached_client) => {
                    tracing::debug!(
                        "Retrieved cached proxy client after write lock for config: {:?}",
                        cache_key
                    );
                    cached_client.clone()
                }
                None => {
                    tracing::info!("Creating new proxy client for config: {:?}", cache_key);

                    let new_client =
                        get_client_builder(proxy_config, should_bypass_proxy, test_mode)?
                            .build()
                            .change_context(ApiClientError::ClientConstructionFailed)
                            .attach_printable("Failed to construct proxy client")?;

                    write_lock.insert(cache_key.clone(), new_client.clone());
                    tracing::debug!("Cached new proxy client for config: {:?}", cache_key);
                    new_client
                }
            }
        }
    };

    Ok(client)
}

fn get_base_client(
    proxy_config: &Proxy,
    should_bypass_proxy: bool,
    test_mode: bool,
) -> CustomResult<Client, ApiClientError> {
    // Check if proxy configuration is provided using cache_key extract_raw_connector_request
    if let Some(cache_key) = proxy_config.cache_key(should_bypass_proxy) {
        tracing::debug!(
            "Using proxy-specific client cache with key: {:?}",
            cache_key
        );

        let cache = PROXY_CLIENT_CACHE.get_or_init(|| RwLock::new(HashMap::new()));

        let client = get_or_create_proxy_client(
            cache,
            cache_key,
            proxy_config,
            should_bypass_proxy,
            test_mode,
        )?;

        Ok(client)
    } else {
        tracing::debug!("No proxy configuration detected, using DEFAULT_CLIENT");

        // Use DEFAULT_CLIENT for non-proxy scenarios
        let client = DEFAULT_CLIENT
            .get_or_try_init(|| {
                tracing::info!("Initializing DEFAULT_CLIENT (no proxy configuration)");
                get_client_builder(proxy_config, should_bypass_proxy, test_mode)?
                    .build()
                    .change_context(ApiClientError::ClientConstructionFailed)
                    .attach_printable("Failed to construct default client")
            })?
            .clone();

        Ok(client)
    }
}

fn load_custom_ca_certificate_from_content(
    mut client_builder: reqwest::ClientBuilder,
    cert_content: &str,
) -> CustomResult<reqwest::ClientBuilder, ApiClientError> {
    let certificate = reqwest::Certificate::from_pem(cert_content.as_bytes())
        .change_context(ApiClientError::InvalidProxyConfiguration)
        .attach_printable("Failed to parse certificate PEM from provided content")?;
    client_builder = client_builder.add_root_certificate(certificate);
    Ok(client_builder)
}

fn get_client_builder(
    proxy_config: &Proxy,
    should_bypass_proxy: bool,
    test_mode: bool,
) -> CustomResult<reqwest::ClientBuilder, ApiClientError> {
    let mut client_builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .pool_idle_timeout(Duration::from_secs(
            proxy_config
                .idle_pool_connection_timeout
                .unwrap_or_default(),
        ));

    // Disable automatic gzip decompression in test mode
    // Mock server returns decompressed responses, so we need to bypass reqwest's gzip handling
    if test_mode {
        client_builder = client_builder.no_gzip();
    }

    if should_bypass_proxy {
        return Ok(client_builder);
    }

    // Attach MITM certificate if enabled
    if proxy_config.mitm_proxy_enabled {
        if let Some(cert_content) = &proxy_config.mitm_ca_cert {
            if !cert_content.trim().is_empty() {
                client_builder =
                    load_custom_ca_certificate_from_content(client_builder, cert_content.trim())?;
            }
        }
    }

    // Proxy all HTTPS traffic through the configured HTTPS proxy
    if let Some(url) = proxy_config.https_url.as_ref() {
        client_builder = client_builder.proxy(
            reqwest::Proxy::https(url)
                .change_context(ApiClientError::InvalidProxyConfiguration)
                .inspect_err(|err| {
                    info_log(
                        "PROXY_ERROR",
                        &json!(format!("HTTPS proxy configuration error. Error: {:?}", err)),
                    );
                })?,
        );
    }

    // Proxy all HTTP traffic through the configured HTTP proxy
    if let Some(url) = proxy_config.http_url.as_ref() {
        client_builder = client_builder.proxy(
            reqwest::Proxy::http(url)
                .change_context(ApiClientError::InvalidProxyConfiguration)
                .inspect_err(|err| {
                    info_log(
                        "PROXY_ERROR",
                        &json!(format!("HTTP proxy configuration error. Error: {:?}", err)),
                    );
                })?,
        );
    }

    Ok(client_builder)
}

pub fn create_identity_from_certificate_and_key(
    encoded_certificate: Secret<String>,
    encoded_certificate_key: Secret<String>,
) -> Result<reqwest::Identity, error_stack::Report<ApiClientError>> {
    let decoded_certificate = BASE64_ENGINE
        .decode(encoded_certificate.expose())
        .change_context(ApiClientError::CertificateDecodeFailed)?;

    let decoded_certificate_key = BASE64_ENGINE
        .decode(encoded_certificate_key.expose())
        .change_context(ApiClientError::CertificateDecodeFailed)?;

    let certificate = String::from_utf8(decoded_certificate)
        .change_context(ApiClientError::CertificateDecodeFailed)?;

    let certificate_key = String::from_utf8(decoded_certificate_key)
        .change_context(ApiClientError::CertificateDecodeFailed)?;

    let key_chain = format!("{}{}", certificate_key, certificate);
    reqwest::Identity::from_pem(key_chain.as_bytes())
        .change_context(ApiClientError::CertificateDecodeFailed)
}

pub fn create_certificate(
    encoded_certificate: Secret<String>,
) -> Result<Vec<reqwest::Certificate>, error_stack::Report<ApiClientError>> {
    let decoded_certificate = BASE64_ENGINE
        .decode(encoded_certificate.expose())
        .change_context(ApiClientError::CertificateDecodeFailed)?;

    let certificate = String::from_utf8(decoded_certificate)
        .change_context(ApiClientError::CertificateDecodeFailed)?;
    reqwest::Certificate::from_pem_bundle(certificate.as_bytes())
        .change_context(ApiClientError::CertificateDecodeFailed)
}

async fn handle_response(
    response: CustomResult<reqwest::Response, ApiClientError>,
) -> CustomResult<Result<Response, Response>, ApiClientError> {
    response
        .async_map(|resp| async {
            let status_code = resp.status().as_u16();
            let headers = Some(resp.headers().to_owned());
            match status_code {
                200..=202 | 302 | 204 => {
                    let response = resp
                        .bytes()
                        .await
                        .change_context(ApiClientError::ResponseDecodingFailed)?;
                    Ok(Ok(Response {
                        headers,
                        response,
                        status_code,
                    }))
                }
                500..=599 => {
                    let bytes = resp.bytes().await.map_err(|error| {
                        report!(error).change_context(ApiClientError::ResponseDecodingFailed)
                    })?;

                    Ok(Err(Response {
                        headers,
                        response: bytes,
                        status_code,
                    }))
                }

                400..=499 => {
                    let bytes = resp.bytes().await.map_err(|error| {
                        report!(error).change_context(ApiClientError::ResponseDecodingFailed)
                    })?;

                    Ok(Err(Response {
                        headers,
                        response: bytes,
                        status_code,
                    }))
                }
                _ => {
                    info_log(
                        "UNEXPECTED_RESPONSE",
                        &json!("Unexpected response from server."),
                    );
                    Err(report!(ApiClientError::UnexpectedServerResponse))
                }
            }
        })
        .await?
}

/// Helper function to remove BOM from response bytes and convert to string
fn strip_bom_and_convert_to_string(response_bytes: &[u8]) -> Option<String> {
    String::from_utf8(response_bytes.to_vec()).ok().map(|s| {
        // Remove BOM if present (UTF-8 BOM is 0xEF, 0xBB, 0xBF)
        if s.starts_with('\u{FEFF}') {
            s.trim_start_matches('\u{FEFF}').to_string()
        } else {
            s
        }
    })
}

#[cfg(feature = "injector-client")]
fn extract_raw_connector_request(connector_request: &Request) -> String {
    // Extract actual body content
    let body_content = match connector_request.body.as_ref() {
        Some(request) => {
            match request {
                // For RawBytes (e.g., SOAP XML), use the string directly without JSON parsing
                RequestContent::RawBytes(_) => {
                    serde_json::Value::String(request.get_inner_value().expose())
                }
                // For other content types, try to parse as JSON
                RequestContent::Json(_)
                | RequestContent::FormUrlEncoded(_)
                | RequestContent::FormData(_)
                | RequestContent::Xml(_) => {
                    let exposed_value = request.get_inner_value().expose();
                    serde_json::from_str(&exposed_value).unwrap_or_else(|_| {
                        tracing::warn!("failed to parse body as JSON, treating as string in extract_raw_connector_request");
                        serde_json::Value::String(exposed_value)
                    })
                }
            }
        }
        None => serde_json::Value::Null,
    };
    // Extract unmasked headers
    let headers_content = connector_request
        .headers
        .iter()
        .map(|(k, v)| {
            let value = match v {
                Maskable::Normal(val) => val.clone(),
                Maskable::Masked(val) => val.clone().expose().to_string(),
            };
            (k.clone(), value)
        })
        .collect::<HashMap<_, _>>();

    // Create complete request with actual content
    json!({
        "url": connector_request.url,
        "method": connector_request.method.to_string(),
        "headers": headers_content,
        "body": body_content
    })
    .to_string()
}

#[cfg(feature = "injector-client")]
/// Helper function to parse JSON from response bytes with BOM handling
fn parse_json_with_bom_handling(
    response_bytes: &[u8],
) -> Result<serde_json::Value, serde_json::Error> {
    // Try direct parsing first (most common case)
    match serde_json::from_slice::<serde_json::Value>(response_bytes) {
        Ok(value) => Ok(value),
        Err(_) => {
            // If direct parsing fails, try after removing BOM
            let cleaned_response = if response_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                // UTF-8 BOM detected, remove it
                #[allow(clippy::indexing_slicing)]
                &response_bytes[3..]
            } else {
                response_bytes
            };
            serde_json::from_slice::<serde_json::Value>(cleaned_response)
        }
    }
}

pub(super) trait HeaderExt {
    fn construct_header_map(self) -> CustomResult<reqwest::header::HeaderMap, ApiClientError>;
}

impl HeaderExt for Headers {
    fn construct_header_map(self) -> CustomResult<reqwest::header::HeaderMap, ApiClientError> {
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

        self.into_iter().try_fold(
            HeaderMap::new(),
            |mut header_map, (header_name, header_value)| {
                let header_name = HeaderName::from_str(&header_name)
                    .change_context(ApiClientError::HeaderMapConstructionFailed)?;
                let header_value = header_value.into_inner();
                let header_value = HeaderValue::from_str(&header_value)
                    .change_context(ApiClientError::HeaderMapConstructionFailed)?;
                header_map.append(header_name, header_value);
                Ok(header_map)
            },
        )
    }
}

pub(super) trait RequestBuilderExt {
    fn add_headers(self, headers: reqwest::header::HeaderMap) -> Self;
}

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn add_headers(mut self, headers: reqwest::header::HeaderMap) -> Self {
        self = self.headers(headers);
        self
    }
}

#[derive(Debug, Default, serde::Deserialize, Clone, strum::EnumString)]
pub enum Tag {
    /// General.
    #[default]
    General,
    /// Redis: get.
    RedisGet,
    /// Redis: set.
    RedisSet,
    /// API: incoming web request.
    ApiIncomingRequest,
    /// API: outgoing web request body.
    ApiOutgoingRequestBody,
    /// API: outgoingh headers
    ApiOutgoingRequestHeaders,
    /// End Request
    EndRequest,
    /// Call initiated to connector.
    InitiatedToConnector,
    /// Incoming response
    IncomingApi,
    /// Api Outgoing Request
    OutgoingApi,
}

#[inline]
pub fn debug_log(action: &str, message: &serde_json::Value) {
    tracing::debug!(tags = %action, json_value= %message);
}

#[inline]
pub fn info_log(action: &str, message: &serde_json::Value) {
    tracing::info!(tags = %action, json_value= %message);
}

#[inline]
pub fn error_log(action: &str, message: &serde_json::Value) {
    tracing::error!(tags = %action, json_value= %message);
}

#[inline]
pub fn warn_log(action: &str, message: &serde_json::Value) {
    tracing::warn!(tags = %action, json_value= %message);
}
