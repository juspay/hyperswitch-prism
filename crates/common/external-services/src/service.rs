use std::{collections::HashMap, str::FromStr, sync::RwLock, time::Duration};

#[cfg(feature = "injector-client")]
use art_recorder::{
    effects as art_effects,
    schema::{CallApiEntry, Either, HttpRequestEntry, HttpResponseEntry, RecordingEntry},
};
use base64::Engine;
use common_enums::ApiClientError;
#[cfg(feature = "injector-client")]
use common_utils::{
    consts::{X_API_TAG, X_API_URL, X_SESSION_ID},
    events::{EventStage, MaskedSerdeValue},
    request::TransportType,
};
use common_utils::{
    ext_traits::AsyncExt,
    lineage,
    request::{Method, Request, RequestContent},
    request_metrics::ConnectorLatencyTracker,
};
use domain_types::{
    connector_types::{ConnectorResponseHeaders, RawConnectorRequestResponse},
    errors::ApiErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{Proxy, ProxyConfig},
    ConnectorError,
};
#[cfg(feature = "injector-client")]
use domain_types::{
    errors::{
        report_common_api_client_to_flow, report_connector_request_to_flow,
        report_connector_response_to_flow, report_kafka_client_to_flow, ConnectorFlowError,
        ResponseTransformationErrorContext,
    },
    IntegrationError,
};
use hyperswitch_masking::{ExposeInterface, Secret};
#[cfg(feature = "injector-client")]
use injector;
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
use url::Url;

/// Test context for mock server integration
#[derive(Debug, Clone)]
pub struct TestContext {
    pub session_id: String,
    pub mock_server_url: String,
    pub protocol: TestMockServerProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMockServerProtocol {
    RawHttp,
    ArtUpload,
}

#[cfg(feature = "injector-client")]
fn apply_test_context_to_request(
    mut req: Request,
    test_context: Option<&TestContext>,
    api_tag: Option<&str>,
) -> Request {
    if let Some(test_ctx) = test_context {
        let original_url = req.url.clone();

        req.url = test_ctx.mock_server_url.clone();
        req.add_header(X_API_URL, original_url.clone().into());
        req.add_header(X_SESSION_ID, test_ctx.session_id.clone().into());

        if let Some(tag) = api_tag {
            req.add_header(X_API_TAG, tag.to_string().into());
        }

        tracing::info!(
            "Test mode enabled: redirected {} to {}",
            original_url,
            test_ctx.mock_server_url
        );
    }

    req
}

#[cfg(feature = "injector-client")]
fn apply_test_context_to_request_with_protocol(
    req: Request,
    test_context: Option<&TestContext>,
    api_tag: Option<&str>,
) -> Result<Request, String> {
    match test_context {
        Some(test_context) if test_context.protocol == TestMockServerProtocol::ArtUpload => {
            build_art_upload_replay_request(&req, test_context, api_tag)
        }
        _ => Ok(apply_test_context_to_request(req, test_context, api_tag)),
    }
}

#[cfg(feature = "injector-client")]
fn build_art_upload_replay_request(
    original_request: &Request,
    test_context: &TestContext,
    api_tag: Option<&str>,
) -> Result<Request, String> {
    let json_request = build_art_http_request_entry(original_request)?;
    let placeholder_response = HttpResponseEntry {
        get_response_body: String::new(),
        get_response_code: 0,
        get_response_headers: HashMap::new(),
        get_response_status: String::new(),
    };
    let call_api_entry =
        CallApiEntry {
            json_request,
            json_result: Either::Right(serde_json::to_value(placeholder_response).map_err(
                |error| format!("failed to serialize ART placeholder response: {error}"),
            )?),
            api_tag: api_tag.unwrap_or("UNKNOWN").to_string(),
        };
    let replay_entry = RecordingEntry::CallApi(call_api_entry);
    let payload = serde_json::to_vec(&replay_entry)
        .map_err(|error| format!("failed to serialize ART replay request: {error}"))?;
    let replay_url = art_upload_mock_url(&test_context.mock_server_url, &test_context.session_id)?;

    let mut request = Request::new(Method::Post, &replay_url);
    request.add_header("content-type", "application/json".to_string().into());
    request.add_header(X_API_URL, original_request.url.clone().into());
    request.add_header(X_SESSION_ID, test_context.session_id.clone().into());
    if let Some(tag) = api_tag {
        request.add_header(X_API_TAG, tag.to_string().into());
    }
    request.set_body(RequestContent::RawBytes(payload));

    tracing::info!(
        "ART upload replay enabled: redirected {} to {}",
        original_request.url,
        replay_url
    );

    Ok(request)
}

#[cfg(feature = "injector-client")]
fn art_upload_mock_url(mock_server_url: &str, session_id: &str) -> Result<String, String> {
    let mut url = Url::parse(mock_server_url)
        .map_err(|error| format!("invalid ART upload mock_server_url: {error}"))?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/mock");
    }

    let query_pairs = url
        .query_pairs()
        .filter(|(key, _value)| key != "guuid")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_pairs {
            query.append_pair(&key, &value);
        }
        query.append_pair("guuid", session_id);
    }

    Ok(url.to_string())
}

#[cfg(feature = "injector-client")]
fn decode_art_upload_replay_result(
    response_result: Result<Response, Response>,
) -> Result<Result<Response, Response>, String> {
    match response_result {
        Ok(response) => decode_art_upload_replay_response(response).map(Ok),
        Err(response) => decode_art_upload_replay_response(response).map(Err),
    }
}

#[cfg(feature = "injector-client")]
fn decode_art_upload_replay_response(response: Response) -> Result<Response, String> {
    let art_response = serde_json::from_slice::<HttpResponseEntry>(&response.response)
        .map_err(|error| format!("failed to decode ART upload replay response: {error}"))?;
    let response_body = BASE64_ENGINE
        .decode(art_response.get_response_body)
        .map_err(|error| format!("failed to decode ART replay response body: {error}"))?;
    let status_code = u16::try_from(art_response.get_response_code).map_err(|_| {
        format!(
            "invalid ART replay status code {}",
            art_response.get_response_code
        )
    })?;

    let mut response_headers = reqwest::header::HeaderMap::new();
    for (header_name, header_value) in art_response.get_response_headers {
        match (
            reqwest::header::HeaderName::from_bytes(header_name.as_bytes()),
            reqwest::header::HeaderValue::from_str(&header_value),
        ) {
            (Ok(header_name), Ok(header_value)) => {
                response_headers.insert(header_name, header_value);
            }
            _ => {
                tracing::warn!(header_name, "skipping invalid ART replay response header");
            }
        }
    }

    Ok(Response {
        headers: Some(response_headers),
        response: response_body.into(),
        status_code,
    })
}

#[cfg(feature = "injector-client")]
fn art_upload_replay_request_error(error: String) -> error_stack::Report<ConnectorFlowError> {
    report!(ConnectorFlowError::Request(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        }
    ))
    .attach_printable(error)
}

#[cfg(feature = "injector-client")]
fn art_upload_replay_response_error(error: String) -> error_stack::Report<ConnectorFlowError> {
    report!(ConnectorFlowError::Response(
        ConnectorError::response_handling_failed_http_status_unknown()
    ))
    .attach_printable(error)
}

#[cfg(all(test, feature = "injector-client"))]
fn build_art_call_api_entry(
    request: &Request,
    response: &Response,
    api_tag: Option<&str>,
) -> Result<CallApiEntry, String> {
    build_art_call_api_entry_from_request_entry(
        build_art_http_request_entry(request)?,
        response,
        api_tag,
    )
}

#[cfg(feature = "injector-client")]
fn build_art_call_api_entry_from_request_entry(
    json_request: HttpRequestEntry,
    response: &Response,
    api_tag: Option<&str>,
) -> Result<CallApiEntry, String> {
    let response_entry = HttpResponseEntry {
        get_response_body: BASE64_ENGINE.encode(&response.response),
        get_response_code: i32::from(response.status_code),
        get_response_headers: response_headers_to_art_headers(&response.headers),
        get_response_status: response_status_text(response.status_code),
    };

    Ok(CallApiEntry {
        json_request,
        json_result: Either::Right(
            serde_json::to_value(response_entry)
                .map_err(|error| format!("failed to serialize ART HTTP response: {error}"))?,
        ),
        api_tag: api_tag.unwrap_or("UNKNOWN").to_string(),
    })
}

#[cfg(feature = "injector-client")]
fn record_art_outgoing_http(
    json_request: HttpRequestEntry,
    response_result: &Result<Response, Response>,
    api_tag: Option<&str>,
) {
    let response = match response_result {
        Ok(response) | Err(response) => response,
    };

    match build_art_call_api_entry_from_request_entry(json_request, response, api_tag) {
        Ok(entry) => {
            if let Err(error) = art_effects::record_outgoing_http(entry) {
                tracing::error!("failed to record ART outgoing HTTP entry: {error}");
            }
        }
        Err(error) => {
            tracing::error!("failed to build ART outgoing HTTP entry: {error}");
        }
    }
}

#[cfg(feature = "injector-client")]
fn build_art_http_request_entry(request: &Request) -> Result<HttpRequestEntry, String> {
    Ok(HttpRequestEntry {
        get_request_method: art_http_method_name(request.method).to_string(),
        get_request_headers: request.get_headers_map(),
        get_request_body: request_body_to_base64(request.body.as_ref())?,
        get_request_url: request.url.clone(),
        get_request_timeout: None,
        get_request_redirects: None,
    })
}

#[cfg(feature = "injector-client")]
fn art_http_method_name(method: Method) -> &'static str {
    match method {
        Method::Get => "Get",
        Method::Post => "Post",
        Method::Put => "Put",
        Method::Delete => "Delete",
        Method::Patch => "Patch",
    }
}

#[cfg(feature = "injector-client")]
fn request_body_to_base64(body: Option<&RequestContent>) -> Result<Option<String>, String> {
    body.map(|body| {
        body.get_body_bytes()
            .map_err(|error| format!("failed to render ART request body: {error}"))
            .map(|(bytes, _content_type)| bytes.map(|bytes| BASE64_ENGINE.encode(bytes)))
    })
    .transpose()
    .map(Option::flatten)
}

#[cfg(feature = "injector-client")]
fn response_headers_to_art_headers(
    headers: &Option<reqwest::header::HeaderMap>,
) -> HashMap<String, String> {
    headers
        .as_ref()
        .map(|headers| {
            headers
                .iter()
                .map(|(key, value)| {
                    let value = value
                        .to_str()
                        .map(str::to_string)
                        .unwrap_or_else(|_| String::from_utf8_lossy(value.as_bytes()).to_string());
                    (key.as_str().to_lowercase(), value)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(feature = "injector-client")]
fn response_status_text(status_code: u16) -> String {
    reqwest::StatusCode::from_u16(status_code)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_default()
}

/// Type of the vault connector
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultConnectorType {
    /// Proxy vault - forwards requests through a proxy (e.g., VGS forward proxy)
    Proxy,
    /// Transformation vault - transforms/tokenizes data (e.g., HyperswitchVault)
    Transformation,
}

/// Authentication credentials for vault connectors
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct VaultConnectorAuth {
    /// API key for authenticating with the vault connector
    pub api_key: Secret<String>,
    /// profile ID for authenticating with the vault connector
    pub profile_id: Secret<String>,
}

/// External Vault Proxy Related Metadata
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum ExternalVaultProxyMetadata {
    /// VGS proxy data variant
    VgsMetadata(VgsMetadata),
    /// HyperswitchVault data variant
    HyperswitchVaultMetadata(HyperswitchVaultMetadata),
}

/// Complete external vault proxy configuration to be serialized and sent to UCS
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ExternalVaultProxyConfig {
    /// Type of the vault connector (e.g., Proxy or Transformation)
    pub vault_connector_type: VaultConnectorType,
    /// Name/ID of the vault connector (e.g., "vgs", "hyperswitch_vault")
    pub vault_connector_id: Option<String>,
    /// Metadata specific to the vault connector type
    pub metadata: ExternalVaultProxyMetadata,
}

/// VGS proxy data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct VgsMetadata {
    /// External vault url
    pub proxy_url: Url,
    /// CA certificates to verify the vault server
    pub certificate: Secret<String>,
}

/// HyperswitchVault proxy data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HyperswitchVaultMetadata {
    /// External vault url
    pub vault_endpoint: Url,
    /// Authentication data for the vault connector
    pub vault_auth_data: VaultConnectorAuth,
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

impl ConnectorRequestReference for domain_types::surcharge::surcharge_types::SurchargeFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders for domain_types::surcharge::surcharge_types::SurchargeFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        None
    }
}

impl ConnectorRequestReference
    for domain_types::merchant_authentication_flow_data::MerchantAuthenticationFlowData
{
    fn get_connector_request_reference_id(&self) -> &str {
        &self.connector_request_reference_id
    }
}

impl AdditionalHeaders
    for domain_types::merchant_authentication_flow_data::MerchantAuthenticationFlowData
{
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        None
    }
}
// `ConnectorRequestReference` is a compile-time bound on `execute_connector_processing_step` but `get_connector_request_reference_id`
// is never called at runtime for FRM flows — the empty string satisfies the trait without affecting behaviour.
impl ConnectorRequestReference for domain_types::frm::frm_types::FrmFlowData {
    fn get_connector_request_reference_id(&self) -> &str {
        ""
    }
}
impl AdditionalHeaders for domain_types::frm::frm_types::FrmFlowData {
    fn get_vault_headers(&self) -> Option<&HashMap<String, Secret<String>>> {
        None
    }
}
use common_utils::events::{Event, EventConfig, FlowName};
#[cfg(feature = "injector-client")]
use common_utils::types::ExecutionMode;
#[cfg(feature = "injector-client")]
// TokenData is now imported from hyperswitch_injector
use common_utils::{consts, emit_event_with_config};
use error_stack::{report, ResultExt};
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

#[cfg(not(feature = "connector-request-kafka"))]
use common_enums::KafkaClientError;
#[cfg(not(feature = "connector-request-kafka"))]
use common_utils::request::KafkaRecord;
#[cfg(feature = "connector-request-kafka")]
pub use connector_request_kafka::publish_to_kafka;
#[cfg(not(feature = "connector-request-kafka"))]
pub async fn publish_to_kafka(
    _kafka_record: KafkaRecord,
) -> CustomResult<Result<Response, Response>, KafkaClientError> {
    Err(KafkaClientError::NotEnabled)?
}

/// Exposes a flow's outcome as a unified [`FlowStatus`], so the generic connector
/// response handler can record the payment-outcome metric for any flow without
/// knowing the concrete status type. Flows without a payment-style status return
/// `None`.
pub trait GetFlowStatus {
    /// The flow's current outcome as a unified `FlowStatus`, if it has one.
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus>;
}

impl GetFlowStatus for domain_types::connector_types::PaymentFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        Some(domain_types::router_data::FlowStatus::Payment(self.status))
    }
}
impl GetFlowStatus for domain_types::connector_types::RefundFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        Some(domain_types::router_data::FlowStatus::Refund(self.status))
    }
}
impl GetFlowStatus for domain_types::connector_types::DisputeFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}
impl GetFlowStatus for domain_types::connector_types::VerifyWebhookSourceFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}
impl GetFlowStatus for domain_types::payouts::payouts_types::PayoutFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}
impl GetFlowStatus for domain_types::surcharge::surcharge_types::SurchargeFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}
impl GetFlowStatus
    for domain_types::merchant_authentication_flow_data::MerchantAuthenticationFlowData
{
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}

impl GetFlowStatus for domain_types::frm::frm_types::FrmFlowData {
    fn flow_status(&self) -> Option<domain_types::router_data::FlowStatus> {
        None
    }
}

/// Stringify a unified `FlowStatus` into a bounded metric label (e.g. `payment_charged`).
#[cfg(feature = "otel")]
fn flow_status_label(flow_status: &domain_types::router_data::FlowStatus) -> String {
    use domain_types::router_data::FlowStatus;
    match flow_status {
        FlowStatus::Payment(status) => format!("payment_{status}"),
        FlowStatus::Refund(status) => format!("refund_{status}"),
        FlowStatus::Dispute(status) => format!("dispute_{status}"),
    }
}

/// Handles the connector response, processing both successful and error responses
#[allow(clippy::too_many_arguments)]
pub fn handle_connector_response<F, ResourceCommonData, Req, Resp>(
    response: CustomResult<Result<Response, Response>, ConnectorError>,
    mut updated_router_data: RouterDataV2<F, ResourceCommonData, Req, Resp>,
    connector: &BoxedConnectorIntegrationV2<'static, F, ResourceCommonData, Req, Resp>,
    mut event: Option<&mut Event>,
    all_keys_required: Option<bool>,
    method: &str,
    url: String,
    event_params: Option<&EventProcessingParams<'_>>,
) -> CustomResult<RouterDataV2<F, ResourceCommonData, Req, Resp>, ConnectorError>
where
    F: Clone + 'static,
    Req: Clone + 'static + std::fmt::Debug,
    Resp: Clone + 'static + std::fmt::Debug,
    ResourceCommonData:
        Clone + RawConnectorRequestResponse + ConnectorResponseHeaders + GetFlowStatus,
{
    let return_raw = event_params.is_none_or(|p| p.return_raw_connector_data);
    match response {
        Ok(body) => {
            let response = match body {
                Ok(body) => {
                    let status_code = body.status_code;
                    tracing::Span::current()
                        .record("status_code", tracing::field::display(status_code));

                    if all_keys_required.unwrap_or(true) && return_raw {
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
                                method,
                                params.service_name,
                                params.connector_name,
                                body.status_code.to_string().as_str(),
                            ])
                            .inc();
                        #[cfg(feature = "otel")]
                        crate::otel_metrics::record_external_error(
                            method,
                            params.service_name,
                            params.connector_name,
                            if params.shadow_mode {
                                "shadow"
                            } else {
                                "primary"
                            },
                            body.status_code.to_string().as_str(),
                        );
                    }

                    if all_keys_required.unwrap_or(true) && return_raw {
                        let raw_response_string = strip_bom_and_convert_to_string(&body.response);
                        updated_router_data
                            .resource_common_data
                            .set_raw_connector_response(raw_response_string.map(Into::into));
                        updated_router_data
                            .resource_common_data
                            .set_connector_response_headers(body.headers.clone());
                    }

                    let error_response = match body.status_code {
                        500..=511 => connector.get_5xx_error_response(
                            body.clone(),
                            event.as_deref_mut(),
                            &updated_router_data.connector_config,
                        )?,
                        _ => connector.get_error_response_v2(
                            body.clone(),
                            event.as_deref_mut(),
                            &updated_router_data.connector_config,
                        )?,
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
                    // Additive: record the connector flow outcome (FlowStatus) so a
                    // decline is visible even though the gRPC call "succeeded".
                    #[cfg(feature = "otel")]
                    if let (Some(params), Some(flow_status)) =
                        (event_params, error_response.attempt_status.as_ref())
                    {
                        crate::otel_metrics::record_payment_status(
                            params.connector_name,
                            params.flow_name.as_str(),
                            if params.shadow_mode {
                                "shadow"
                            } else {
                                "primary"
                            },
                            &flow_status_label(flow_status),
                        );
                    }
                    Err(error_stack::report!(
                        ConnectorError::ConnectorErrorResponse(error_response)
                    ))?
                }
            };
            // Centralised success-path payment outcome: every connector flow returns
            // through here, so the final status is recorded once without per-handler
            // code. Additive, feature-gated.
            #[cfg(feature = "otel")]
            if let (Some(params), Some(flow_status)) =
                (event_params, response.resource_common_data.flow_status())
            {
                crate::otel_metrics::record_payment_status(
                    params.connector_name,
                    params.flow_name.as_str(),
                    if params.shadow_mode {
                        "shadow"
                    } else {
                        "primary"
                    },
                    &flow_status_label(&flow_status),
                );
            }
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
    /// Explicit proxy name from `x-proxy-name` header. If None, falls back to shadow_mode heuristic.
    pub proxy_name: Option<&'a str>,
    pub tenant_id: &'a str,
    pub merchant_id: &'a str,
    pub return_raw_connector_data: bool,
    pub connector_latency: ConnectorLatencyTracker,
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
    proxy: &ProxyConfig,
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
        + AdditionalHeaders
        + GetFlowStatus,
{
    let start = tokio::time::Instant::now();
    let proxy_name = event_params.proxy_name.unwrap_or("primary");
    let transport_type = connector.get_transport_type();
    let result = match (call_connector_action, transport_type) {
        (common_enums::CallConnectorAction::HandleResponseWithoutBuildRequest, _) => {
            let response = Response {
                headers: None,
                response: bytes::Bytes::new(),
                status_code: 200,
            };
            connector
                .handle_response_v2(&router_data, None, response)
                .map_err(report_connector_response_to_flow)
        }
        // handle_response removed from proto (PaymentServiceGetRequest field 5 reserved)
        (common_enums::CallConnectorAction::HandleResponse(_), _) => {
            return Err(error_stack::report!(ConnectorFlowError::from(
                IntegrationError::NotSupported {
                    message:
                        "The handle_response field has been removed from PaymentServiceGetRequest \
                              (proto field 5 reserved). This flow is no longer supported."
                            .into(),
                    connector: "N/A",
                    context: Default::default(),
                }
            )));
        }
        (common_enums::CallConnectorAction::Trigger, TransportType::Http) => {
            let mut connector_request = connector
                .build_request_v2(&router_data.clone())
                .map_err(report_connector_request_to_flow)?;

            let mut updated_router_data = router_data.clone();
            updated_router_data = match &connector_request {
                Some(request) if event_params.return_raw_connector_data => {
                    updated_router_data
                        .resource_common_data
                        .set_raw_connector_request(Some(
                            extract_raw_connector_request(request).into(),
                        ));
                    updated_router_data
                }
                _ => updated_router_data,
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
                    req.add_header(
                        consts::X_MERCHANT_ID,
                        Maskable::Masked(Secret::new(event_params.merchant_id.to_string())),
                    );
                }
                req
            });

            let art_upload_replay = test_context.as_ref().is_some_and(|test_context| {
                test_context.protocol == TestMockServerProtocol::ArtUpload
            });

            // Apply test environment modifications if test context is provided
            connector_request = connector_request
                .map(|req| {
                    apply_test_context_to_request_with_protocol(
                        req,
                        test_context.as_ref(),
                        api_tag.as_deref(),
                    )
                })
                .transpose()
                .map_err(art_upload_replay_request_error)?;

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
                    #[cfg(feature = "otel")]
                    crate::otel_metrics::record_external_call(
                        &method.to_string(),
                        event_params.service_name,
                        event_params.connector_name,
                        if event_params.shadow_mode {
                            "shadow"
                        } else {
                            "primary"
                        },
                    );
                    let external_service_start_latency = tokio::time::Instant::now();
                    tracing::Span::current().record("request.url", tracing::field::display(&url));
                    tracing::Span::current()
                        .record("request.method", tracing::field::display(method));

                    let masked_headers = request.headers.clone();
                    tracing::info!(headers=?masked_headers, "headers of connector request");
                    tracing::Span::current()
                        .record("request.headers", tracing::field::debug(&masked_headers));

                    let masked_request = mask_connector_request(&request.body);
                    tracing::info!(request=?masked_request, "request of connector");
                    tracing::Span::current()
                        .record("request.body", tracing::field::display(&masked_request));

                    let art_request = match build_art_http_request_entry(&request) {
                        Ok(request) => Some(request),
                        Err(error) => {
                            tracing::error!(
                                "failed to build ART outgoing HTTP request entry: {error}"
                            );
                            None
                        }
                    };

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

                        // Collect connector request headers (excluding vault metadata)
                        let headers: HashMap<String, Secret<String>> = request
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
                            .collect();

                        // Parse vault metadata and build injector request
                        let vault_headers =
                            updated_router_data.resource_common_data.get_vault_headers();
                        let injector_request = build_injector_request(
                            Url::parse(&request.url).change_context(ConnectorFlowError::from(
                                IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                },
                            ))?,
                            request.method.to_http_method(),
                            template,
                            token_data,
                            headers,
                            proxy
                                .effective_https_url(proxy_name)
                                .or(proxy.effective_http_url(proxy_name))
                                .map(|url| Secret::new(url.to_string())),
                            vault_headers,
                        );

                        // New injector handles HTTP request internally and returns enhanced response
                        let injector_response =
                            injector_core(injector_request).await.change_context(
                                ConnectorFlowError::from(IntegrationError::RequestEncodingFailed {
                                    context: Default::default(),
                                }),
                            )?;

                        // Convert injector response to connector service Response format
                        let actual_response = injector_response
                            .response
                            .get("response")
                            .cloned()
                            .unwrap_or(injector_response.response.clone());

                        let response_bytes =
                            serde_json::to_vec(&actual_response).map_err(|_| {
                                ConnectorFlowError::from(
                                    ConnectorError::response_handling_failed_http_status_unknown(),
                                )
                            })?;

                        // Extract the actual connector status_code from the wrapper if present,
                        // otherwise fall back to the injector-level status_code
                        let actual_status_code = injector_response
                            .response
                            .get("status_code")
                            .and_then(|v| v.as_u64())
                            .and_then(|v| u16::try_from(v).ok())
                            .unwrap_or(injector_response.status_code);

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
                            status_code: actual_status_code, // Use actual status code from connector
                        }))
                    } else {
                        let test_mode = test_context.is_some();
                        call_connector_api(
                            proxy,
                            request,
                            "execute_connector_processing_step",
                            test_mode,
                            event_params.proxy_name,
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
                    let response = if art_upload_replay {
                        response.and_then(|response| {
                            decode_art_upload_replay_result(response)
                                .map_err(art_upload_replay_response_error)
                        })
                    } else {
                        response
                    };
                    if let (Some(art_request), Ok(connector_response)) =
                        (art_request, response.as_ref())
                    {
                        record_art_outgoing_http(
                            art_request,
                            connector_response,
                            api_tag.as_deref(),
                        );
                    }
                    let external_service_elapsed = external_service_start_latency.elapsed();
                    event_params
                        .connector_latency
                        .add_connector_time(external_service_elapsed);
                    metrics::EXTERNAL_SERVICE_API_CALLS_LATENCY
                        .with_label_values(&[
                            &method.to_string(),
                            event_params.service_name,
                            event_params.connector_name,
                        ])
                        .observe(external_service_elapsed.as_secs_f64());
                    #[cfg(feature = "otel")]
                    crate::otel_metrics::record_external_latency(
                        &method.to_string(),
                        event_params.service_name,
                        event_params.connector_name,
                        if event_params.shadow_mode {
                            "shadow"
                        } else {
                            "primary"
                        },
                        external_service_elapsed.as_secs_f64(),
                    );
                    // Extract status code BEFORE creating event - one liner
                    let status_code = response.as_ref().ok().map(|result| match result {
                        Ok(body) | Err(body) => i32::from(body.status_code),
                    });

                    let latency =
                        u64::try_from(external_service_elapsed.as_millis()).unwrap_or(u64::MAX);

                    // Create single event (response_data will be set by connector)
                    let mut event = create_event(
                        &event_params,
                        Some(url.clone()),
                        Some(method.to_string()),
                        Some(latency),
                        &masked_headers,
                        status_code,
                        &masked_request,
                    );

                    let result = handle_connector_response(
                        response.change_context(
                            ConnectorError::response_handling_failed_http_status_unknown(),
                        ),
                        updated_router_data,
                        &connector,
                        Some(&mut event),
                        all_keys_required,
                        &method.to_string(),
                        url,
                        Some(&event_params),
                    )
                    .map_err(report_connector_response_to_flow);

                    emit_event_with_config(event, event_params.event_config);
                    result
                }
                None => Ok(router_data),
            }
        }
        (common_enums::CallConnectorAction::Trigger, TransportType::Kafka) => {
            let kafka_record = connector
                .build_kafka_record(&router_data.clone())
                .map_err(report_connector_request_to_flow)?;

            match kafka_record {
                Some(record) => {
                    metrics::EXTERNAL_SERVICE_TOTAL_API_CALLS
                        .with_label_values(&[
                            "PUBLISH",
                            event_params.service_name,
                            event_params.connector_name,
                        ])
                        .inc();
                    #[cfg(feature = "otel")]
                    crate::otel_metrics::record_external_call(
                        "PUBLISH",
                        event_params.service_name,
                        event_params.connector_name,
                        if event_params.shadow_mode {
                            "shadow"
                        } else {
                            "primary"
                        },
                    );
                    let external_service_start_latency = tokio::time::Instant::now();

                    let topic = record.topic.clone();
                    tracing::Span::current().record("request.url", tracing::field::display(&topic));

                    let masked_headers = record.headers.clone();
                    tracing::info!(headers=?masked_headers, "headers of connector request");
                    tracing::Span::current()
                        .record("request.headers", tracing::field::debug(&record.headers));

                    let masked_request = mask_connector_request(&record.payload);
                    tracing::info!(request=?masked_request, "request of connector");
                    tracing::Span::current()
                        .record("request.body", tracing::field::display(&masked_request));

                    let response = publish_to_kafka(record)
                        .await
                        .map_err(report_kafka_client_to_flow)
                        .inspect_err(|err| {
                            info_log(
                                "NETWORK_ERROR",
                                &json!(format!(
                                    "Failed to publish connector message to Kafka. Error: {:?}",
                                    err
                                )),
                            );
                        });

                    let external_service_elapsed = external_service_start_latency.elapsed();
                    event_params
                        .connector_latency
                        .add_connector_time(external_service_elapsed);
                    metrics::EXTERNAL_SERVICE_API_CALLS_LATENCY
                        .with_label_values(&[
                            "PUBLISH",
                            event_params.service_name,
                            event_params.connector_name,
                        ])
                        .observe(external_service_elapsed.as_secs_f64());
                    #[cfg(feature = "otel")]
                    crate::otel_metrics::record_external_latency(
                        "PUBLISH",
                        event_params.service_name,
                        event_params.connector_name,
                        if event_params.shadow_mode {
                            "shadow"
                        } else {
                            "primary"
                        },
                        external_service_elapsed.as_secs_f64(),
                    );
                    tracing::info!(?response, "response from connector");

                    // Extract status code BEFORE creating event - one liner
                    let status_code = response.as_ref().ok().map(|result| match result {
                        Ok(body) | Err(body) => i32::from(body.status_code),
                    });

                    let latency =
                        u64::try_from(external_service_elapsed.as_millis()).unwrap_or(u64::MAX);

                    // Create single event (response_data will be set by connector)
                    let mut event = create_event(
                        &event_params,
                        Some(topic.clone()),
                        None,
                        Some(latency),
                        &masked_headers,
                        status_code,
                        &masked_request,
                    );

                    let result = handle_connector_response(
                        response.change_context(
                            ConnectorError::response_handling_failed_http_status_unknown(),
                        ),
                        router_data,
                        &connector,
                        Some(&mut event),
                        all_keys_required,
                        "PUBLISH",
                        topic,
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

#[cfg(feature = "injector-client")]
fn mask_connector_request(request_content: &Option<RequestContent>) -> serde_json::Value {
    match request_content {
        Some(request) => match request {
            RequestContent::Json(i)
            | RequestContent::FormUrlEncoded(i)
            | RequestContent::Xml(i) => (**i)
                .masked_serialize()
                .unwrap_or(json!({ "error": "failed to mask serialize connector request"})),
            RequestContent::FormData(_) => json!({"request_type": "FORM_DATA"}),
            RequestContent::RawBytes(_) => json!({"request_type": "RAW_BYTES"}),
        },
        None => serde_json::Value::Null,
    }
}

#[cfg(feature = "injector-client")]
fn create_event(
    event_params: &EventProcessingParams<'_>,
    url: Option<String>,
    method: Option<String>,
    latency_ms: Option<u64>,
    headers: &Headers,
    status_code: Option<i32>,
    masked_request: &serde_json::Value,
) -> Event {
    let request_id = event_params.request_id.to_string();
    let event_headers = headers
        .iter()
        .map(|(k, v)| (k.clone(), format!("{v:?}")))
        .collect();

    let mut event = Event {
        request_id: request_id.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis().into(),
        flow_type: event_params.flow_name,
        connector: event_params.connector_name.to_string(),
        url,
        method,
        stage: EventStage::ConnectorCall,
        execution_mode: ExecutionMode::from_shadow_flag(event_params.shadow_mode),
        latency_ms,
        status_code,
        request_data: MaskedSerdeValue::from_masked_optional(masked_request, "connector_request"),
        response_data: None, // Will be set by connector via set_response_body
        error: None,
        headers: event_headers,
        additional_fields: HashMap::new(),
        lineage_ids: event_params.lineage_ids.to_owned(),
    };

    event.add_reference_id(event_params.reference_id.as_deref());
    event.add_resource_id(event_params.resource_id.as_deref());
    event.add_service_type(event_params.service_type);
    event.add_service_name(event_params.service_name);
    event.add_tenant_id(event_params.tenant_id);

    event
}

pub enum ApplicationResponse<R> {
    Json(R),
}

pub type CustomResult<T, E> = error_stack::Result<T, E>;
pub type RouterResult<T> = CustomResult<T, ApiErrorResponse>;
pub type RouterResponse<T> = CustomResult<ApplicationResponse<T>, ApiErrorResponse>;

pub async fn call_connector_api(
    proxy: &ProxyConfig,
    request: Request,
    _flow_name: &str,
    test_mode: bool,
    header_proxy_name: Option<&str>,
) -> CustomResult<Result<Response, Response>, ApiClientError> {
    let url = Url::parse(&request.url).change_context(ApiClientError::UrlEncodingFailed)?;

    let should_bypass_proxy = proxy.bypass_urls.contains(&url.to_string());

    let proxy_name = header_proxy_name.unwrap_or("primary");

    let client = create_client(
        proxy,
        should_bypass_proxy,
        proxy_name,
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
    proxy_config: &ProxyConfig,
    should_bypass_proxy: bool,
    proxy_name: &str,
    client_certificate: Option<Secret<String>>,
    client_certificate_key: Option<Secret<String>>,
    test_mode: bool,
) -> CustomResult<Client, ApiClientError> {
    match (client_certificate.clone(), client_certificate_key.clone()) {
        (Some(encoded_certificate), Some(encoded_certificate_key)) => {
            let client_builder =
                get_client_builder(proxy_config, should_bypass_proxy, proxy_name, test_mode)?;

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
        _ => get_base_client(proxy_config, should_bypass_proxy, proxy_name, test_mode),
    }
}

/// Default total timeout (seconds) for a single connector API call.
const DEFAULT_CONNECTOR_REQUEST_TIMEOUT_SECS: u64 = 30;

static DEFAULT_CLIENT: OnceCell<Client> = OnceCell::new();
static PROXY_CLIENT_CACHE: OnceCell<RwLock<HashMap<(Proxy, String), Client>>> = OnceCell::new();

fn get_or_create_proxy_client(
    cache: &RwLock<HashMap<(Proxy, String), Client>>,
    cache_key: (Proxy, String),
    proxy_config: &ProxyConfig,
    should_bypass_proxy: bool,
    proxy_name: &str,
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

                    let new_client = get_client_builder(
                        proxy_config,
                        should_bypass_proxy,
                        proxy_name,
                        test_mode,
                    )?
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
    proxy_config: &ProxyConfig,
    should_bypass_proxy: bool,
    proxy_name: &str,
    test_mode: bool,
) -> CustomResult<Client, ApiClientError> {
    if let Some(cache_key) = proxy_config.cache_key(should_bypass_proxy, proxy_name) {
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
            proxy_name,
            test_mode,
        )?;

        Ok(client)
    } else {
        tracing::debug!("No proxy configuration detected, using DEFAULT_CLIENT");

        let client = DEFAULT_CLIENT
            .get_or_try_init(|| {
                tracing::info!("Initializing DEFAULT_CLIENT (no proxy configuration)");
                get_client_builder(proxy_config, should_bypass_proxy, proxy_name, test_mode)?
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
    proxy_config: &ProxyConfig,
    should_bypass_proxy: bool,
    proxy_name: &str,
    test_mode: bool,
) -> CustomResult<reqwest::ClientBuilder, ApiClientError> {
    let mut client_builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .pool_idle_timeout(Duration::from_secs(
            proxy_config
                .idle_pool_connection_timeout
                .unwrap_or_default(),
        ))
        .timeout(Duration::from_secs(
            proxy_config
                .connector_request_timeout
                .unwrap_or(DEFAULT_CONNECTOR_REQUEST_TIMEOUT_SECS),
        ));

    // Disable automatic gzip decompression in test mode
    // Mock server returns decompressed responses, so we need to bypass reqwest's gzip handling
    if test_mode {
        client_builder = client_builder.no_gzip();
    }

    if should_bypass_proxy {
        return Ok(client_builder);
    }

    if !proxy_name.is_empty() && !proxy_config.proxies.contains_key(proxy_name) {
        tracing::warn!(
            proxy_name,
            "x-proxy-name header refers to unknown proxy — falling back to direct connection"
        );
    }

    if let Some(cert) = proxy_config
        .get(proxy_name)
        .and_then(|p| p.active_ca_cert())
    {
        client_builder = load_custom_ca_certificate_from_content(client_builder, cert)?;
    }

    if let Some(url) = proxy_config.effective_https_url(proxy_name) {
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

    if let Some(url) = proxy_config.effective_http_url(proxy_name) {
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

/// Parse the `x-external-vault-metadata` header from vault headers and return the config.
///
/// The header value is expected to be a **base64-encoded** JSON string representing
/// an [`ExternalVaultProxyConfig`]. This function decodes the base64 payload, converts
/// it to a UTF-8 string, and then deserializes the JSON.
#[cfg(feature = "injector-client")]
fn parse_external_vault_config(
    vault_headers: Option<&HashMap<String, Secret<String>>>,
) -> Option<ExternalVaultProxyConfig> {
    use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine};

    vault_headers
        .and_then(|vh| vh.get(consts::X_EXTERNAL_VAULT_METADATA))
        .and_then(|header_value| {
            let encoded = header_value.clone().expose();
            let decoded_bytes = BASE64_ENGINE
                .decode(&encoded)
                .inspect_err(|e| {
                    tracing::warn!("Failed to base64-decode external vault metadata: {:?}", e);
                })
                .ok()?;

            let json_str = String::from_utf8(decoded_bytes)
                .inspect_err(|e| {
                    tracing::warn!("External vault metadata is not valid UTF-8: {:?}", e);
                })
                .ok()?;

            serde_json::from_str::<ExternalVaultProxyConfig>(&json_str)
                .inspect_err(|e| {
                    tracing::warn!("Failed to parse external vault metadata JSON: {:?}", e);
                })
                .ok()
        })
}

#[cfg(feature = "injector-client")]
/// Apply parsed external vault proxy config to the injector request's connection config.
fn apply_vault_config_to_injector(
    injector_request: &mut injector::InjectorRequest,
    vault_cfg: ExternalVaultProxyConfig,
) {
    tracing::info!(
        vault_connector_type = ?vault_cfg.vault_connector_type,
        vault_connector_id = ?vault_cfg.vault_connector_id,
        "Applying external vault proxy config to injector request"
    );

    // Map local VaultConnectorType to injector's VaultConnectorType
    injector_request.connection_config.vault_connector_type =
        Some(match vault_cfg.vault_connector_type {
            VaultConnectorType::Proxy => injector::VaultConnectorType::Proxy,
            VaultConnectorType::Transformation => injector::VaultConnectorType::Transformation,
        });

    // Map vault_connector_id string to injector's VaultConnectors enum
    injector_request.connection_config.vault_connector_id = vault_cfg
        .vault_connector_id
        .as_deref()
        .and_then(|id| match id.to_lowercase().as_str() {
            "vgs" => Some(injector::VaultConnectors::VGS),
            "hyperswitch_vault" => Some(injector::VaultConnectors::HyperswitchVault),
            _ => {
                tracing::warn!("Unknown vault_connector_id: {}", id);
                None
            }
        });

    // Apply metadata-specific config (proxy_url/ca_cert or vault_endpoint/auth)
    match vault_cfg.metadata {
        ExternalVaultProxyMetadata::VgsMetadata(vgs) => {
            injector_request.connection_config.proxy_url =
                Some(Secret::new(vgs.proxy_url.to_string()));
            injector_request.connection_config.ca_cert = Some(vgs.certificate);
        }
        ExternalVaultProxyMetadata::HyperswitchVaultMetadata(hsv) => {
            injector_request.connection_config.vault_endpoint = Some(hsv.vault_endpoint);
            injector_request.connection_config.vault_auth_data =
                Some(injector::VaultConnectorAuth {
                    api_key: hsv.vault_auth_data.api_key,
                    profile_id: hsv.vault_auth_data.profile_id,
                });
        }
    }
}

/// Build an `InjectorRequest` from connector request components and vault metadata.
///
/// Constructs the base request and enriches the `connection_config` with external vault
/// proxy metadata (VGS proxy_url/ca_cert, or HyperswitchVault endpoint/auth) if the
/// `x-external-vault-metadata` header is present in vault headers.
#[cfg(feature = "injector-client")]
fn build_injector_request(
    endpoint: Url,
    http_method: HttpMethod,
    template: String,
    token_data: TokenData,
    headers: HashMap<String, Secret<String>>,
    backup_proxy_url: Option<Secret<String>>,
    vault_headers: Option<&HashMap<String, Secret<String>>>,
) -> injector::InjectorRequest {
    let mut injector_request = injector::InjectorRequest::new(
        endpoint,
        http_method,
        template,
        token_data,
        Some(headers),
        backup_proxy_url,
        None,
        None,
        None,
    );

    if let Some(vault_cfg) = parse_external_vault_config(vault_headers) {
        apply_vault_config_to_injector(&mut injector_request, vault_cfg);
    }

    injector_request
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

#[cfg(all(test, feature = "injector-client"))]
mod test_context_tests {
    use common_utils::{
        consts::{X_API_TAG, X_API_URL, X_SESSION_ID},
        request::{Method, Request, RequestContent},
    };
    use serde_json::json;

    use super::{
        apply_test_context_to_request, build_art_upload_replay_request, TestContext,
        TestMockServerProtocol,
    };

    #[test]
    fn apply_test_context_redirects_request_to_mock_server() {
        let request = Request::new(Method::Post, "https://connector.example.com/payments");
        let test_context = TestContext {
            session_id: "req_art_replay_123".to_string(),
            mock_server_url: "http://localhost:3000/mockGateway".to_string(),
            protocol: TestMockServerProtocol::RawHttp,
        };

        let request =
            apply_test_context_to_request(request, Some(&test_context), Some("AUTHORIZE"));
        let headers = request.get_headers_map();

        assert_eq!(request.url, "http://localhost:3000/mockGateway");
        assert_eq!(
            headers.get(X_API_URL).map(String::as_str),
            Some("https://connector.example.com/payments")
        );
        assert_eq!(
            headers.get(X_SESSION_ID).map(String::as_str),
            Some("req_art_replay_123")
        );
        assert_eq!(
            headers.get(X_API_TAG).map(String::as_str),
            Some("AUTHORIZE")
        );
    }

    #[test]
    fn apply_test_context_leaves_request_unchanged_without_context() {
        let mut request = Request::new(Method::Post, "https://connector.example.com/payments");
        request.add_header("existing-header", "existing-value".to_string().into());

        let request = apply_test_context_to_request(request, None, Some("AUTHORIZE"));
        let headers = request.get_headers_map();

        assert_eq!(request.url, "https://connector.example.com/payments");
        assert_eq!(
            headers.get("existing-header").map(String::as_str),
            Some("existing-value")
        );
        assert!(!headers.contains_key(X_API_URL));
        assert!(!headers.contains_key(X_SESSION_ID));
        assert!(!headers.contains_key(X_API_TAG));
    }

    #[test]
    fn build_art_upload_replay_request_uses_art_mock_route_and_body_shape() {
        let mut request = Request::new(Method::Post, "https://connector.example.com/payments");
        request.add_header("content-type", "application/json".to_string().into());
        request.set_body(RequestContent::RawBytes(br#"{"amount":100}"#.to_vec()));
        let test_context = TestContext {
            session_id: "req_art_replay_123".to_string(),
            mock_server_url: "http://localhost:8010".to_string(),
            protocol: TestMockServerProtocol::ArtUpload,
        };

        let replay_request =
            build_art_upload_replay_request(&request, &test_context, Some("GW_AUTHORIZE"))
                .expect("ART upload replay request should be built");
        let headers = replay_request.get_headers_map();

        assert_eq!(replay_request.method, Method::Post);
        assert_eq!(
            replay_request.url,
            "http://localhost:8010/mock?guuid=req_art_replay_123"
        );
        assert_eq!(
            headers.get("content-type").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            headers.get(X_API_URL).map(String::as_str),
            Some("https://connector.example.com/payments")
        );
        assert_eq!(
            headers.get(X_SESSION_ID).map(String::as_str),
            Some("req_art_replay_123")
        );
        assert_eq!(
            headers.get(X_API_TAG).map(String::as_str),
            Some("GW_AUTHORIZE")
        );

        let Some(RequestContent::RawBytes(payload)) = replay_request.body.as_ref() else {
            panic!("ART replay request should use a raw JSON body");
        };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(payload)
                .expect("ART replay payload should be JSON"),
            json!({
                "tag": "CallAPIEntryT",
                "contents": {
                    "jsonRequest": {
                        "getRequestMethod": "Post",
                        "getRequestHeaders": { "content-type": "application/json" },
                        "getRequestBody": "eyJhbW91bnQiOjEwMH0=",
                        "getRequestURL": "https://connector.example.com/payments",
                        "getRequestTimeout": null,
                        "getRequestRedirects": null
                    },
                    "jsonResult": {
                        "Right": {
                            "getResponseBody": "",
                            "getResponseCode": 0,
                            "getResponseHeaders": {},
                            "getResponseStatus": ""
                        }
                    },
                    "apiTag": "GW_AUTHORIZE"
                }
            })
        );
    }
}

#[cfg(all(test, feature = "injector-client"))]
mod art_outgoing_http_tests {
    use std::collections::HashMap;

    use art_recorder::schema::HttpResponseEntry;
    use base64::Engine;
    use bytes::Bytes;
    use common_utils::request::{Method, Request, RequestContent};
    use domain_types::router_response_types::Response;
    use reqwest::header::{HeaderMap, HeaderValue};
    use serde_json::json;

    use super::{build_art_call_api_entry, decode_art_upload_replay_response, BASE64_ENGINE};

    #[test]
    fn build_art_call_api_entry_records_connector_request_and_response() {
        let mut request = Request::new(Method::Post, "https://connector.example.com/payments");
        request.add_header("Content-Type", "application/json".to_string().into());
        request.set_body(RequestContent::RawBytes(br#"{"amount":100}"#.to_vec()));

        let mut response_headers = HeaderMap::new();
        response_headers.insert("x-request-id", HeaderValue::from_static("req_123"));
        let response = Response {
            headers: Some(response_headers),
            response: Bytes::from_static(br#"{"status":"ok"}"#),
            status_code: 201,
        };

        let entry = build_art_call_api_entry(&request, &response, Some("AUTHORIZE"))
            .expect("request and response should convert to ART entry");

        assert_eq!(entry.api_tag, "AUTHORIZE");
        assert_eq!(entry.json_request.get_request_method, "Post");
        assert_eq!(
            entry.json_request.get_request_url,
            "https://connector.example.com/payments"
        );
        assert_eq!(
            entry
                .json_request
                .get_request_headers
                .get("content-type")
                .map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            entry.json_request.get_request_body.as_deref(),
            Some("eyJhbW91bnQiOjEwMH0=")
        );

        assert_eq!(
            serde_json::to_value(entry.json_result).expect("jsonResult should serialize"),
            json!({
                "Right": {
                    "getResponseBody": "eyJzdGF0dXMiOiJvayJ9",
                    "getResponseCode": 201,
                    "getResponseHeaders": {
                        "x-request-id": "req_123"
                    },
                    "getResponseStatus": "Created"
                }
            })
        );
    }

    #[test]
    fn decode_art_upload_replay_response_restores_connector_http_response() {
        let mut response_headers = HashMap::new();
        response_headers.insert("content-type".to_string(), "application/json".to_string());
        let art_response = HttpResponseEntry {
            get_response_body: BASE64_ENGINE.encode(br#"{"replayed":true}"#),
            get_response_code: 202,
            get_response_headers: response_headers,
            get_response_status: "Accepted".to_string(),
        };
        let response = Response {
            headers: None,
            response: Bytes::from(
                serde_json::to_vec(&art_response).expect("ART response should serialize"),
            ),
            status_code: 200,
        };

        let decoded =
            decode_art_upload_replay_response(response).expect("ART upload response should decode");

        assert_eq!(decoded.status_code, 202);
        assert_eq!(
            decoded.response,
            Bytes::from_static(br#"{"replayed":true}"#)
        );
        assert_eq!(
            decoded
                .headers
                .as_ref()
                .and_then(|headers| headers.get("content-type"))
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
    }
}
