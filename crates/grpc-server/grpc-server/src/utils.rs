// Re-export shared interface logic from ucs_interface_common
pub use ucs_interface_common::auth::*;
pub use ucs_interface_common::config::*;
pub use ucs_interface_common::flow::*;
pub use ucs_interface_common::metadata::*;

use art_recorder::{
    effects as art_effects,
    flush::{recording_rows_from_runtime, RecEntryTransform},
    runtime::{ArtMode, ArtRuntime, ArtRuntimeSettings, SessionContext},
    schema::{CsvRecording, IncomingApiEntry, IncomingApiRequestEntry, IncomingApiResponseEntry},
};
use common_utils::{
    consts::{self, Env},
    errors::CustomResult,
    events::{Event, EventStage, FlowName, MaskedSerdeValue},
    lineage::LineageIds,
    superposition_config::{get_connector_urls, ConnectorUrls, SuperpositionConfig},
    types::ExecutionMode,
};
use domain_types::{
    connector_types, errors::IntegrationError, router_data::ConnectorSpecificConfig,
};
use error_stack::Report;
use http::request::Request;
use hyperswitch_masking;
use prost::Message;
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, future::Future, mem::size_of_val, sync::Arc};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use ucs_env::{configs, error::ResultExtGrpc};

use crate::request::RequestData;

/// Record the header's fields in request's trace
pub fn record_fields_from_header<B: hyper::body::Body>(request: &Request<B>) -> tracing::Span {
    let url_path = request.uri().path();

    let span = tracing::debug_span!(
        "request",
        uri = %url_path,
        version = ?request.version(),
        tenant_id = tracing::field::Empty,
        request_id = tracing::field::Empty,
        execution_mode = tracing::field::Empty,
    );
    request
        .headers()
        .get(consts::X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(|tenant_id| span.record("tenant_id", tenant_id));

    request
        .headers()
        .get(consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(|request_id| span.record("request_id", request_id));

    // On the request span so every log line of the request carries primary/shadow.
    let shadow = request
        .headers()
        .get(consts::X_SHADOW_MODE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    span.record(
        "execution_mode",
        ExecutionMode::from_shadow_flag(shadow).as_str(),
    );

    span
}

pub fn validate_environment(environment: &str) -> Result<Env, String> {
    let environment_lower = environment.to_lowercase();
    serde::Deserialize::deserialize(
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&environment_lower),
    )
    .map_err(|_| {
        format!(
            "Invalid environment '{}'. Valid values are: development, sandbox, production",
            environment
        )
    })
}

/// Resolves connector configuration with optional superposition URL patching.
///
/// This function handles the complete flow for connector configuration:
/// 1. If environment header is provided, validate and try to resolve URLs from superposition
/// 2. If URLs are resolved, patch the connector config with them
/// 3. Apply connector-specific config overrides
/// 4. Fall back to static config if no environment or superposition resolution fails
pub fn get_resolved_connectors(
    config: &configs::Config,
    connector: &connector_types::ConnectorEnum,
    connector_config: &ConnectorSpecificConfig,
    environment: Option<&str>,
) -> CustomResult<domain_types::types::Connectors, IntegrationError> {
    use domain_types::errors::IntegrationErrorContext;
    match environment {
        Some(env) => {
            validate_environment(env).map_err(|e| {
                Report::new(IntegrationError::InvalidDataFormat {
                    field_name: "x-environment",
                    context: IntegrationErrorContext {
                        additional_context: Some(e),
                        ..Default::default()
                    },
                })
            })?;

            match resolve_connector_urls(
                config.superposition_config.as_ref().map(|arc| arc.as_ref()),
                connector,
                env,
            ) {
                Some(urls) => {
                    tracing::info!("resolved URLs from superposition for environment: {}", env);
                    let patched_connectors = config
                        .connectors
                        .patch_connector_urls(connector, &urls)
                        .map_err(|e| {
                            Report::new(IntegrationError::ConfigurationError {
                                code: "URL_PATCHING_FAILED".to_string(),
                                message: format!("URL patching failed: {e}"),
                                context: IntegrationErrorContext::default(),
                            })
                        })?;
                    connectors_with_connector_config_overrides_on_connectors(
                        connector_config,
                        patched_connectors,
                    )
                }
                None => {
                    tracing::info!(
                        "superposition resolution failed, using static config with overrides"
                    );
                    connectors_with_connector_config_overrides(connector_config, config)
                }
            }
        }
        None => {
            tracing::info!("no x-environment header, using static config with overrides");
            connectors_with_connector_config_overrides(connector_config, config)
        }
    }
}

/// Resolve connector URLs from superposition configuration.
///
/// This function attempts to resolve connector URLs dynamically based on the
/// connector name and environment dimensions.
///
/// # Arguments
/// * `superposition_config` - Optional reference to the loaded superposition configuration
/// * `connector` - The connector enum (e.g., "stripe", "adyen")
/// * `environment` - The environment dimension (must be one of: "production", "sandbox", "development")
///
/// # Returns
/// * `Some(ConnectorUrls)` - Successfully resolved URLs from superposition (dynamic config)
/// * `None` - Superposition not configured or resolution failed (caller should fallback to static config)
///
/// # Static vs Dynamic Config
/// - **Static config**: Connector URLs defined in TOML files (development.toml, sandbox.toml, production.toml)
///   that are loaded at application startup and remain constant for the deployment environment.
/// - **Dynamic config**: URLs resolved at runtime from the Superposition service, which can vary per-request
///   based on the `x-environment` header, allowing different URLs for the same connector across requests.
///
/// # Note
/// This function does NOT validate the environment. Call `validate_environment()` first if you need
/// to reject invalid environment values with an error.
///
/// # Example
/// ```ignore
/// // First validate if you want to reject invalid environments
/// validate_environment(environment)?;
///
/// let urls = resolve_connector_urls(
///     config.superposition_config.as_ref(),
///     &metadata_payload.connector,
///     environment,
/// );
/// ```
pub fn resolve_connector_urls(
    superposition_config: Option<&SuperpositionConfig>,
    connector: &connector_types::ConnectorEnum,
    environment: &str,
) -> Option<ConnectorUrls> {
    let config = superposition_config?;

    let environment_lower = environment.to_lowercase();
    let connector_str = connector.to_string().to_lowercase();

    match config.resolve(&connector_str, &environment_lower) {
        Ok(resolved) => {
            let urls = get_connector_urls(&resolved);
            if urls.base_url.is_none() {
                tracing::warn!(
                    connector = %connector_str,
                    environment = %environment_lower,
                    "Superposition resolved but no base_url found, falling back to static config"
                );
                return None;
            }
            tracing::info!(
                connector = %connector_str,
                environment = %environment_lower,
                base_url = ?urls.base_url,
                "Resolved connector URLs from superposition"
            );
            Some(urls)
        }
        Err(e) => {
            tracing::warn!(
                connector = %connector_str,
                environment = %environment_lower,
                error = %e,
                "Failed to resolve connector URLs from superposition, falling back to static config"
            );
            None
        }
    }
}

pub fn merge_configs(override_val: &Value, base_val: &Value) -> Value {
    match (base_val, override_val) {
        (Value::Object(base_map), Value::Object(override_map)) => {
            let mut merged = base_map.clone();
            for (key, override_value) in override_map {
                let base_value = base_map.get(key).unwrap_or(&Value::Null);
                merged.insert(key.clone(), merge_configs(override_value, base_value));
            }
            Value::Object(merged)
        }
        // override replaces base for primitive, null, or array
        (_, override_val) => override_val.clone(),
    }
}

pub fn log_before_initialization<T>(
    request_data: &RequestData<T>,
    service_name: &str,
) -> CustomResult<(), IntegrationError>
where
    T: Serialize,
{
    let metadata_payload = &request_data.extracted_metadata;
    let MetadataPayload {
        connector,
        merchant_id,
        tenant_id,
        request_id,
        ..
    } = metadata_payload;
    let current_span = tracing::Span::current();
    let req_body_json = match hyperswitch_masking::masked_serialize(&request_data.payload) {
        Ok(masked_value) => masked_value.to_string(),
        Err(e) => {
            tracing::error!("Masked serialization error: {:?}", e);
            "<masked serialization error>".to_string()
        }
    };
    let connector_name = connector.get_connector_name();
    current_span.record("service_name", service_name);
    current_span.record("request_body", req_body_json);
    current_span.record("gateway", connector_name);
    current_span.record("merchant_id", merchant_id);
    current_span.record("tenant_id", tenant_id);
    current_span.record("request_id", request_id);
    tracing::info!("Golden Log Line (incoming - request)");
    Ok(())
}

pub fn log_after_initialization<T>(result: &Result<tonic::Response<T>, tonic::Status>)
where
    T: Serialize + std::fmt::Debug,
{
    let current_span = tracing::Span::current();

    match &result {
        Ok(response) => {
            current_span.record("response_body", tracing::field::debug(response.get_ref()));

            let res_ref = response.get_ref();

            // Try converting to JSON Value
            if let Ok(Value::Object(map)) = serde_json::to_value(res_ref) {
                if let Some(status_val) = map.get("status") {
                    let status_num_opt = status_val.as_number();
                    let status_u32_opt: Option<u32> = status_num_opt
                        .and_then(|n| n.as_u64())
                        .and_then(|n| u32::try_from(n).ok());
                    let status_str = if let Some(s) = status_u32_opt {
                        common_enums::AttemptStatus::try_from(s)
                            .unwrap_or(common_enums::AttemptStatus::Unknown)
                            .to_string()
                    } else {
                        common_enums::AttemptStatus::Unknown.to_string()
                    };
                    current_span.record("flow_specific_fields.status", status_str);
                }
            } else {
                tracing::warn!("Could not serialize response to JSON to extract status");
            }
        }
        Err(status) => {
            current_span.record("error_message", status.message());
            current_span.record("status_code", status.code().to_string());
        }
    }
    tracing::info!("Golden Log Line (incoming - response)");
}

fn create_art_runtime_for_request(
    config: &configs::Config,
    metadata_payload: &MetadataPayload,
    flow_name: FlowName,
    service_name: &str,
) -> ArtRuntime {
    let session = SessionContext {
        request_id: metadata_payload.request_id.clone(),
        merchant_id: metadata_payload.merchant_id.clone(),
        connector: metadata_payload.connector.get_connector_name(),
        flow: flow_name.as_str().to_string(),
        hostname: service_name.to_string(),
    };

    match config.art_feature(metadata_payload.art_recording_enabled) {
        configs::ArtFeature::Replay => ArtRuntime::replay(session, Vec::new()),
        configs::ArtFeature::Record => ArtRuntime::recording_with_settings(
            session,
            Some(config.art_recording.max_entries_per_session),
            ArtRuntimeSettings {
                record_incoming_api: config.art_recording.record_incoming_api,
                record_outgoing_http: config.art_recording.record_outgoing_http,
                record_effects: config.art_recording.record_effects,
            },
        ),
        configs::ArtFeature::Disabled => ArtRuntime::disabled(),
    }
}

fn art_order_id_from_request<T: Serialize>(
    payload: &T,
    metadata_payload: &MetadataPayload,
) -> String {
    art_order_id_from_payload(payload)
        .or_else(|| metadata_payload.reference_id.clone())
        .unwrap_or_default()
}

fn art_order_id_from_payload<T: Serialize>(payload: &T) -> Option<String> {
    let payload = serde_json::to_value(payload).ok()?;
    extract_art_order_id_from_payload_value(&payload)
}

fn extract_art_order_id_from_payload_value(payload: &Value) -> Option<String> {
    payload
        .get("metadata")
        .and_then(extract_order_id_from_metadata_value)
        .or_else(|| non_empty_string(payload.get("merchant_order_id")))
}

fn extract_order_id_from_metadata_value(metadata: &Value) -> Option<String> {
    extract_order_id_from_metadata_json(metadata).or_else(|| {
        let metadata_value = match metadata {
            Value::Object(map) => map.get("value"),
            Value::String(_) => Some(metadata),
            _ => None,
        }?;
        let metadata_str = metadata_value.as_str()?.trim();
        if metadata_str.is_empty() {
            return None;
        }
        serde_json::from_str::<Value>(metadata_str)
            .ok()
            .and_then(|metadata_json| extract_order_id_from_metadata_json(&metadata_json))
    })
}

fn extract_order_id_from_metadata_json(metadata: &Value) -> Option<String> {
    ["metadata[order_id]", "order_id"]
        .into_iter()
        .find_map(|key| non_empty_string(metadata.get(key)))
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn art_metadata_entry_from_request(
    metadata_payload: &MetadataPayload,
    flow_name: FlowName,
    service_name: &str,
) -> Value {
    serde_json::json!({
        "request_id": metadata_payload.request_id.as_str(),
        "merchant_id": metadata_payload.merchant_id.as_str(),
        "reference_id": metadata_payload.reference_id.as_deref(),
        "resource_id": metadata_payload.resource_id.as_deref(),
        "tenant_id": metadata_payload.tenant_id.as_str(),
        "connector": metadata_payload.connector.get_connector_name(),
        "service_name": service_name,
        "flow": flow_name.as_str(),
        "execution_mode": ExecutionMode::from_shadow_flag(metadata_payload.shadow_mode).as_str(),
    })
}

pub fn resolve_api_tag(
    config: &configs::Config,
    _metadata_payload: &MetadataPayload,
    flow_name: FlowName,
    payment_method_type: Option<common_enums::PaymentMethodType>,
) -> Option<String> {
    config.api_tags.get_tag(flow_name, payment_method_type)
}

fn rec_entry_transform_from_config(
    config: &configs::ArtRecordingConfig,
) -> Result<RecEntryTransform<'_>, String> {
    if !config.encrypt_entries {
        return Ok(RecEntryTransform::Plain);
    }

    let key = config.aes_key.as_deref().ok_or_else(|| {
        "art_recording.encrypt_entries=true requires art_recording.aes_key".to_string()
    })?;
    let iv = config.aes_iv.as_deref().ok_or_else(|| {
        "art_recording.encrypt_entries=true requires art_recording.aes_iv".to_string()
    })?;

    Ok(RecEntryTransform::Aes256Cbc { key, iv })
}

fn art_recording_rows_fit_buffer_limit(rows: &[CsvRecording], max_buffer_size_mb: u64) -> bool {
    let Some(max_buffer_size_bytes) = max_buffer_size_mb
        .checked_mul(1024 * 1024)
        .and_then(|bytes| usize::try_from(bytes).ok())
    else {
        return true;
    };

    art_recording_rows_size_bytes(rows) <= max_buffer_size_bytes
}

fn art_recording_rows_size_bytes(rows: &[CsvRecording]) -> usize {
    rows.iter()
        .map(|row| {
            row.sess_id.len()
                + row.merch_id.len()
                + row.ord_id.len()
                + row.val_type.len()
                + row.rec_entry.len()
                + size_of_val(&row.counter)
        })
        .sum()
}

fn flush_art_recording(runtime: &ArtRuntime, config: &configs::ArtRecordingConfig, order_id: &str) {
    if runtime.mode() != ArtMode::Record || !config.enabled {
        return;
    }

    let transform = match rec_entry_transform_from_config(config) {
        Ok(transform) => transform,
        Err(error) => {
            tracing::error!(
                error = %error,
                "Failed to prepare ART recording transform; recording rows dropped"
            );
            return;
        }
    };

    let rows = match recording_rows_from_runtime(runtime, Some(order_id), transform) {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(
                error = ?error,
                "Failed to build ART recording rows; recording rows dropped"
            );
            return;
        }
    };

    if !art_recording_rows_fit_buffer_limit(&rows, config.max_buffer_size_mb) {
        tracing::error!(
            row_count = rows.len(),
            max_buffer_size_mb = config.max_buffer_size_mb,
            "ART recording rows exceeded configured buffer limit; recording rows dropped"
        );
        return;
    }

    if config.flush_async {
        let config = config.clone();
        tokio::spawn(async move {
            crate::art_recording::publish_art_recording_rows(&rows, &config);
        });
    } else {
        crate::art_recording::publish_art_recording_rows(&rows, config);
    }
}

async fn run_art_scoped_handler<T, F, Fut, R>(
    request_data: RequestData<T>,
    mut art_runtime: ArtRuntime,
    art_recording_config: &configs::ArtRecordingConfig,
    art_order_id: String,
    should_record_incoming_api: bool,
    flow_name: FlowName,
    service_name: &str,
    handler: F,
) -> Result<tonic::Response<R>, tonic::Status>
where
    T: Serialize,
    F: FnOnce(RequestData<T>) -> Fut,
    Fut: Future<Output = Result<tonic::Response<R>, tonic::Status>>,
    R: Serialize,
{
    if art_runtime.mode() == ArtMode::Record {
        if let Err(error) = art_effects::record_metadata_with_runtime(
            &mut art_runtime,
            "PRISM_ART_CONTEXT",
            art_metadata_entry_from_request(
                &request_data.extracted_metadata,
                flow_name,
                service_name,
            ),
        ) {
            tracing::error!("failed to record ART metadata entry: {error}");
        }
    }

    let incoming_request = (art_runtime.mode() == ArtMode::Record && should_record_incoming_api)
        .then(|| build_incoming_grpc_api_request(&request_data, flow_name, service_name));
    let start_time = incoming_request.as_ref().map(|_| current_art_timestamp());

    let (result, mut art_runtime) =
        art_recorder::runtime::scope(art_runtime, handler(request_data)).await;

    if let (Some(incoming_request), Some(start_time)) = (incoming_request, start_time) {
        let end_time = current_art_timestamp();
        if let Err(error) = record_incoming_grpc_api_entry(
            &mut art_runtime,
            incoming_request,
            &result,
            flow_name,
            service_name,
            start_time,
            end_time,
        ) {
            tracing::error!("failed to record ART incoming gRPC API entry: {error}");
        }
    }

    flush_art_recording(&art_runtime, art_recording_config, &art_order_id);

    result
}

fn current_art_timestamp() -> Value {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map(Value::String)
        .unwrap_or_else(|error| {
            serde_json::json!({
                "error": "timestamp_formatting_failed",
                "message": error.to_string()
            })
        })
}

#[cfg(test)]
fn record_incoming_grpc_api<T, R>(
    runtime: &mut ArtRuntime,
    request_data: &RequestData<T>,
    grpc_response: &Result<tonic::Response<R>, tonic::Status>,
    flow_name: FlowName,
    service_name: &str,
    start_time: Value,
    end_time: Value,
) -> Result<(), art_recorder::runtime::ArtError>
where
    T: Serialize,
    R: Serialize,
{
    let incoming_request = build_incoming_grpc_api_request(request_data, flow_name, service_name);
    record_incoming_grpc_api_entry(
        runtime,
        incoming_request,
        grpc_response,
        flow_name,
        service_name,
        start_time,
        end_time,
    )
}

fn build_incoming_grpc_api_request<T>(
    request_data: &RequestData<T>,
    flow_name: FlowName,
    service_name: &str,
) -> IncomingApiRequestEntry
where
    T: Serialize,
{
    IncomingApiRequestEntry {
        api_req_body: to_json_value(&request_data.payload, "grpc_request"),
        api_req_url: format!("grpc://{}/{}", service_name, flow_name.as_str()),
        api_req_method: "GRPC".to_string(),
        api_req_headers: request_data.masked_metadata.get_all_masked(),
        api_req_query_params: HashMap::new(),
        api_req_route_params: HashMap::new(),
    }
}

fn record_incoming_grpc_api_entry<R>(
    runtime: &mut ArtRuntime,
    incoming_request: IncomingApiRequestEntry,
    grpc_response: &Result<tonic::Response<R>, tonic::Status>,
    flow_name: FlowName,
    service_name: &str,
    start_time: Value,
    end_time: Value,
) -> Result<(), art_recorder::runtime::ArtError>
where
    R: Serialize,
{
    let (response_body, response_headers, response_code) = match grpc_response {
        Ok(response) => (
            to_json_value(response.get_ref(), "grpc_response"),
            metadata_to_headers(response.metadata()),
            i32::from(tonic::Code::Ok),
        ),
        Err(status) => (
            build_error_detail(status),
            HashMap::new(),
            i32::from(status.code()),
        ),
    };

    art_effects::record_incoming_api_with_runtime(
        runtime,
        IncomingApiEntry {
            api_request: incoming_request,
            api_response: IncomingApiResponseEntry {
                api_res_body: response_body,
                api_res_headers: response_headers,
                api_res_code: response_code,
            },
            api_tag: flow_name.as_str().to_string(),
            hostname: service_name.to_string(),
            start_time,
            end_time,
        },
    )
}

fn metadata_to_headers(metadata: &tonic::metadata::MetadataMap) -> HashMap<String, String> {
    metadata
        .iter()
        .filter_map(|entry| match entry {
            tonic::metadata::KeyAndValueRef::Ascii(key, value) => value
                .to_str()
                .ok()
                .map(|value| (key.as_str().to_string(), value.to_string())),
            tonic::metadata::KeyAndValueRef::Binary(key, value) => Some((
                key.as_str().to_string(),
                String::from_utf8_lossy(value.as_encoded_bytes()).to_string(),
            )),
        })
        .collect()
}

fn to_json_value<T>(value: &T, field: &'static str) -> Value
where
    T: Serialize,
{
    serde_json::to_value(value).unwrap_or_else(|error| {
        serde_json::json!({
            "error": "serialization_failed",
            "field": field,
            "message": error.to_string()
        })
    })
}

/// Generic gRPC logging wrapper that accepts a custom parser function.
/// This allows different parsing strategies for different flow types
/// (e.g., authenticated flows vs unauthenticated webhook flows).
pub async fn grpc_logging_wrapper_with_parser<T, P, F, R>(
    request: tonic::Request<T>,
    service_name: &str,
    config: Arc<configs::Config>,
    flow_name: FlowName,
    parser: P,
    handler: F,
) -> Result<tonic::Response<R>, tonic::Status>
where
    T: Serialize + std::fmt::Debug + Send + 'static + hyperswitch_masking::ErasedMaskSerialize,
    P: FnOnce(tonic::Request<T>, Arc<configs::Config>) -> Result<RequestData<T>, tonic::Status>,
    F: FnOnce(
        RequestData<T>,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = Result<tonic::Response<R>, tonic::Status>> + Send>,
    >,
    R: Serialize + std::fmt::Debug + hyperswitch_masking::ErasedMaskSerialize,
{
    let current_span = tracing::Span::current();
    let start_time = tokio::time::Instant::now();
    let masked_request_data =
        MaskedSerdeValue::from_masked_optional(request.get_ref(), "grpc_request");
    let mut event_metadata_payload = None;
    let mut event_headers = HashMap::new();

    let grpc_response = async {
        let request_data = parser(request, config.clone())?;
        log_before_initialization(&request_data, service_name).into_grpc_status()?;
        event_headers = request_data.masked_metadata.get_all_masked();
        event_metadata_payload = Some(request_data.extracted_metadata.clone());

        let art_runtime = create_art_runtime_for_request(
            &config,
            &request_data.extracted_metadata,
            flow_name,
            service_name,
        );
        let art_order_id =
            art_order_id_from_request(&request_data.payload, &request_data.extracted_metadata);
        let result = run_art_scoped_handler(
            request_data,
            art_runtime,
            &config.art_recording,
            art_order_id,
            config.art_recording.record_incoming_api,
            flow_name,
            service_name,
            handler,
        )
        .await;

        let duration = start_time.elapsed().as_millis();
        current_span.record("response_time", duration);
        log_after_initialization(&result);
        result
    }
    .await;

    #[cfg(feature = "otel")]
    observe_internal_latency(
        start_time,
        flow_name,
        service_name,
        event_metadata_payload.as_ref(),
    );
    create_and_emit_grpc_event(
        masked_request_data,
        &grpc_response,
        start_time,
        flow_name,
        service_name,
        &config,
        event_metadata_payload.as_ref(),
        event_headers,
    );

    grpc_response
}

/// Original gRPC logging wrapper for authenticated flows.
/// Maintains backward compatibility with existing code.
pub async fn grpc_logging_wrapper<T, F, Fut, R>(
    request: tonic::Request<T>,
    service_name: &str,
    config: Arc<configs::Config>,
    flow_name: FlowName,
    handler: F,
) -> Result<tonic::Response<R>, tonic::Status>
where
    T: Serialize + std::fmt::Debug + Send + 'static + hyperswitch_masking::ErasedMaskSerialize,
    F: FnOnce(RequestData<T>) -> Fut + Send,
    Fut: Future<Output = Result<tonic::Response<R>, tonic::Status>> + Send,
    R: Serialize + std::fmt::Debug + hyperswitch_masking::ErasedMaskSerialize,
{
    let current_span = tracing::Span::current();
    let start_time = tokio::time::Instant::now();
    let masked_request_data =
        MaskedSerdeValue::from_masked_optional(request.get_ref(), "grpc_request");
    let mut event_metadata_payload = None;
    let mut event_headers = HashMap::new();

    let grpc_response = async {
        let request_data = RequestData::from_grpc_request(request, config.clone())?;
        log_before_initialization(&request_data, service_name).into_grpc_status()?;
        event_headers = request_data.masked_metadata.get_all_masked();
        event_metadata_payload = Some(request_data.extracted_metadata.clone());

        let art_runtime = create_art_runtime_for_request(
            &config,
            &request_data.extracted_metadata,
            flow_name,
            service_name,
        );
        let art_order_id =
            art_order_id_from_request(&request_data.payload, &request_data.extracted_metadata);
        let result = run_art_scoped_handler(
            request_data,
            art_runtime,
            &config.art_recording,
            art_order_id,
            config.art_recording.record_incoming_api,
            flow_name,
            service_name,
            handler,
        )
        .await;

        let duration = start_time.elapsed().as_millis();
        current_span.record("response_time", duration);
        log_after_initialization(&result);
        result
    }
    .await;

    #[cfg(feature = "otel")]
    observe_internal_latency(
        start_time,
        flow_name,
        service_name,
        event_metadata_payload.as_ref(),
    );
    create_and_emit_grpc_event(
        masked_request_data,
        &grpc_response,
        start_time,
        flow_name,
        service_name,
        &config,
        event_metadata_payload.as_ref(),
        event_headers,
    );

    grpc_response
}

#[cfg(feature = "otel")]
fn observe_internal_latency(
    start_time: tokio::time::Instant,
    flow_name: FlowName,
    service_name: &str,
    metadata_payload: Option<&MetadataPayload>,
) {
    let connector_time = metadata_payload
        .map(|metadata| metadata.connector_latency.connector_time())
        .unwrap_or_default();
    let internal = start_time.elapsed().saturating_sub(connector_time);
    let connector = metadata_payload
        .map(|md| md.connector.get_connector_name())
        .unwrap_or_else(|| "unknown".to_string());
    let mode =
        ExecutionMode::from_shadow_flag(metadata_payload.map(|md| md.shadow_mode).unwrap_or(false));
    external_services::otel_metrics::record_internal_latency(
        &flow_name.to_string(),
        service_name,
        &connector,
        mode.as_str(),
        internal.as_secs_f64(),
    );
}

#[allow(clippy::too_many_arguments)]
fn create_and_emit_grpc_event<R>(
    masked_request_data: Option<MaskedSerdeValue>,
    grpc_response: &Result<tonic::Response<R>, tonic::Status>,
    start_time: tokio::time::Instant,
    flow_name: FlowName,
    service_name: &str,
    config: &configs::Config,
    metadata_payload: Option<&MetadataPayload>,
    masked_headers: HashMap<String, String>,
) where
    R: Serialize,
{
    let connector = metadata_payload
        .map(|md| md.connector.get_connector_name())
        .unwrap_or_else(|| "unknown".to_string());

    let mut grpc_event = Event {
        request_id: metadata_payload.map_or("unknown".to_string(), |md| md.request_id.clone()),
        timestamp: chrono::Utc::now().timestamp_millis().into(),
        flow_type: flow_name,
        connector,
        url: None,
        method: None,
        stage: EventStage::GrpcRequest,
        execution_mode: ExecutionMode::from_shadow_flag(
            metadata_payload.map(|md| md.shadow_mode).unwrap_or(false),
        ),
        latency_ms: Some(u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX)),
        status_code: None,
        request_data: masked_request_data,
        response_data: None,
        error: None,
        headers: masked_headers,
        additional_fields: HashMap::new(),
        lineage_ids: metadata_payload
            .map_or_else(|| LineageIds::empty(""), |md| md.lineage_ids.clone()),
    };

    grpc_event
        .add_reference_id(metadata_payload.and_then(|metadata| metadata.reference_id.as_deref()));
    grpc_event
        .add_resource_id(metadata_payload.and_then(|metadata| metadata.resource_id.as_deref()));
    grpc_event.add_service_type(service_type_str(&config.server.type_));
    grpc_event.add_service_name(service_name);
    grpc_event.add_tenant_id(
        metadata_payload
            .map(|md| md.tenant_id.as_str())
            .unwrap_or("public"),
    );

    match grpc_response {
        Ok(response) => grpc_event.set_grpc_success_response(response.get_ref()),
        Err(error) => {
            grpc_event.set_grpc_error_response(error);
            grpc_event.set_error_response(&build_error_detail(error));
        }
    }

    common_utils::emit_event_with_config(grpc_event, &config.events);
}

fn build_error_detail(status: &tonic::Status) -> Value {
    use grpc_api_types::payments::{ConnectorError, IntegrationError};

    let details = status.details();
    let (error_code, http_status_code) = if details.is_empty() {
        (None, None)
    } else {
        IntegrationError::decode(details)
            .map(|e| (Some(e.error_code), None))
            .or_else(|_| {
                ConnectorError::decode(details).map(|e| (Some(e.error_code), e.http_status_code))
            })
            .unwrap_or((None, None))
    };
    serde_json::json!({
        "grpc_code": i32::from(status.code()),
        "grpc_code_name": format!("{:?}", status.code()),
        "error_code": error_code,
        "http_status_code": http_status_code,
        "error_message": status.message(),
    })
}

#[allow(clippy::result_large_err)]
pub fn get_config_from_request<T>(
    request: &tonic::Request<T>,
) -> Result<Arc<configs::Config>, tonic::Status>
where
    T: Serialize,
{
    match request.extensions().get::<Arc<configs::Config>>() {
        Some(config) => {
            tracing::info!("Using config from request extensions");
            Ok(config.clone())
        }
        None => {
            tracing::info!("Configuration not found in request extensions, using default config.");
            Err(tonic::Status::internal(
                "Configuration not found in request extensions",
            ))
        }
    }
}

#[macro_export]
macro_rules! implement_connector_operation {
    // Pattern with payment method data processing and action matching
    (
        fn_name: $fn_name:ident,
        log_prefix: $log_prefix:literal,
        request_type: $request_type:ty,
        response_type: $response_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty,
        request_data_constructor: $request_data_constructor:path,
        common_flow_data_constructor: $common_flow_data_constructor:path,
        generate_response_fn: $generate_response_fn:path,
        connector_data_type: $connector_data_type:ty,
        all_keys_required: $all_keys_required:expr,
        has_payment_method_data: true
    ) => {
        async fn $fn_name(
            &self,
            request: $crate::request::RequestData<$request_type>,
        ) -> Result<tonic::Response<$response_type>, tonic::Status> {
            #[allow(unused_imports)]
            use ucs_env::error::IntoGrpcStatus;
            tracing::info!(concat!($log_prefix, "_FLOW: initiated"));
            let config = request
                .extensions
                .get::<std::sync::Arc<ucs_env::configs::Config>>()
                .cloned()
                .ok_or_else(|| tonic::Status::internal("Configuration not found in request extensions"))?;
            let service_name = request
                .extensions
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown_service".to_string());
            let result = Box::pin(async{
            let $crate::request::RequestData {
                payload,
                extracted_metadata: metadata_payload,
                masked_metadata,
                extensions: _
            } = request;

            let request_id = metadata_payload.request_id.clone();
            let connector_config = metadata_payload.connector_config.clone();

            // Get connector data using ConnectorDataProvider trait
            let connector_data: $connector_data_type =
                connector_integration::types::ConnectorDataProvider::from_connector_variant(&metadata_payload.connector)
                    .ok_or_else(|| tonic::Status::unimplemented("Invalid connector type for this flow"))?;

            // Get connector integration
            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            // Create common request data
            let common_flow_data = $common_flow_data_constructor((payload.clone(), config.connectors.clone(), &masked_metadata))
                .into_grpc_status()?;

            // Process payment method data
            let payment_method_data_action = domain_types::types::PaymentMethodDataAction::get_payment_method_data_action(
                payload.payment_method.clone()
                    .ok_or_else(|| tonic::Status::invalid_argument("missing payment_method in the payload"))?
            )
            .map_err(|err| {
                tracing::error!(concat!($log_prefix, "_FLOW: failed to get payment method data action - error: {:?}"), err);
                tonic::Status::invalid_argument("Invalid payment method data")
            })?;

            let payment_method_data = match payment_method_data_action {
                domain_types::types::PaymentMethodDataAction::Card(card_details) => {
                    tracing::info!(concat!($log_prefix, "_FLOW: Processing regular payment with card"));
                    let card = domain_types::payment_method_data::Card::<domain_types::payment_method_data::DefaultPCIHolder>::foreign_try_from(card_details)
                        .map_err(|err| {
                            tracing::error!(concat!($log_prefix, "_FLOW: failed to convert card details - error: {:?}"), err);
                            tonic::Status::invalid_argument("Invalid card details")
                        })?;
                    Ok(domain_types::payment_method_data::PaymentMethodData::Card(card))
                }
                domain_types::types::PaymentMethodDataAction::Default => {
                    let pm_data = domain_types::payment_method_data::PaymentMethodData::convert_to_domain_model_for_non_card_payment_methods(
                        payload.payment_method.clone()
                            .ok_or_else(|| tonic::Status::invalid_argument("missing payment_method in the payload"))?
                    )
                    .map_err(|err| {
                        tracing::error!("Failed to convert payment method data: {:?}", err);
                        tonic::Status::invalid_argument("Invalid payment method data")
                    })?;
                    Ok(pm_data)
                }
                domain_types::types::PaymentMethodDataAction::CardProxy(_) => {
                    Err(tonic::Status::invalid_argument("CardProxy not supported in this flow"))
                }
            }?;

            // Create connector request data with payment method data
            let specific_request_data = $request_data_constructor((payload.clone(), payment_method_data))
                .into_grpc_status()?;

            // Create router data
            let router_data = domain_types::router_data_v2::RouterDataV2::<
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > {
                flow: std::marker::PhantomData,
                resource_common_data: common_flow_data,
                connector_config,
                request: specific_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };

            // Calculate flow name for dynamic flow-specific configurations
            let flow_name = $crate::utils::flow_marker_to_flow_name::<$flow_marker>();

            // Get API tag for the current flow with payment method type
            let api_tag = $crate::utils::resolve_api_tag(
                &config,
                &metadata_payload,
                flow_name,
                router_data.request.payment_method_type,
            );

            // Create ART replay context when replay mode is enabled.
            let test_context = config.create_art_replay_context(&request_id).map_err(|e| {
                tonic::Status::internal(format!("Test mode configuration error: {e}"))
            })?;

            // Execute connector processing
            let event_params = external_services::service::EventProcessingParams {
                connector_name: &connector.to_string(),
                service_name: &service_name,
                service_type: $crate::utils::service_type_str(&config.server.type_),
                flow_name,
                event_config: &config.events,
                request_id: &request_id,
                lineage_ids: &metadata_payload.lineage_ids,
                reference_id: &metadata_payload.reference_id,
                resource_id: &metadata_payload.resource_id,
                shadow_mode: metadata_payload.shadow_mode,
                proxy_name: metadata_payload.proxy_name.as_deref(),
                tenant_id: &metadata_payload.tenant_id,
                merchant_id: metadata_payload.merchant_id.as_str(),
                return_raw_connector_data: config.common.return_raw_connector_data,
                connector_latency: metadata_payload.connector_latency.clone(),
            };
            let call_connector_action = connector_integration.get_call_connector_action();
            let response_result = external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                router_data,
                $all_keys_required,
                event_params,
                None,
                call_connector_action,
                test_context,
                api_tag,
            )
            .await
            .switch()
            .into_grpc_status()?;

            // Generate response
            let final_response = $generate_response_fn(response_result)
                .into_grpc_status()?;

            Ok(tonic::Response::new(final_response))
        }).await;
        result
    }
};

    // Pattern with Option<PaymentMethodData> for flows that need it but don't do action processing
    (
        fn_name: $fn_name:ident,
        log_prefix: $log_prefix:literal,
        request_type: $request_type:ty,
        response_type: $response_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        // Base type constructors (macro applies `<T>`); a single flow has exactly one
        // payment-method-data holder (`DefaultPCIHolder` or `VaultTokenHolder`) at runtime,
        // so the proxy and non-proxy paths differ only in that generic `T`.
        request_data_type: $request_data_type:ident,
        response_data_type: $response_data_type:ty,
        request_data_constructor: $request_data_constructor:path,
        common_flow_data_constructor: $common_flow_data_constructor:path,
        generate_response_fn: $generate_response_fn:path,
        connector_data: $connector_data:ident,
        all_keys_required: $all_keys_required:expr,
        has_payment_method_data: option
    ) => {
        async fn $fn_name(
            &self,
            request: $crate::request::RequestData<$request_type>,
        ) -> Result<tonic::Response<$response_type>, tonic::Status> {
            tracing::info!(concat!($log_prefix, "_FLOW: initiated"));
            let config = request
                .extensions

                .get::<std::sync::Arc<ucs_env::configs::Config>>()
                .cloned()
                .ok_or_else(|| tonic::Status::internal("Configuration not found in request extensions"))?;
            let service_name = request
                .extensions
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown_service".to_string());
            let result = Box::pin(async{
            let $crate::request::RequestData {
                payload,
                extracted_metadata: metadata_payload,
                masked_metadata,
                extensions: _
            } = request;

            let request_id = metadata_payload.request_id.clone();
            let connector_config = metadata_payload.connector_config.clone();

            // Inspect the payment method up front so the vault-aliased card-proxy
            // (VGS / Basis Theory / Spreedly) flows can build `Some(token_data)` and
            // route through the external-services injector, exactly as the payment
            // Authorize flow does. A non-proxy request keeps the existing direct
            // connector call with `token_data = None`.
            let payment_method_data_action = match payload.payment_method.clone() {
                Some(pm) => Some(
                    domain_types::types::PaymentMethodDataAction::get_payment_method_data_action(pm)
                        .into_grpc_status()?,
                ),
                None => None,
            };

            // Create common request data (shared by both the direct and proxy paths;
            // it already carries the parsed `x-external-vault-metadata` vault headers).
            let common_flow_data = $common_flow_data_constructor((payload.clone(), config.connectors.clone(), &masked_metadata))
                .into_grpc_status()?;

            // Calculate flow name for dynamic flow-specific configurations
            let flow_name = $crate::utils::flow_marker_to_flow_name::<$flow_marker>();

            // Get API tag for the current flow
            let api_tag =
                $crate::utils::resolve_api_tag(&config, &metadata_payload, flow_name, None);

            // Create ART replay context when replay mode is enabled.
            let test_context = config.create_art_replay_context(&request_id).map_err(|e| {
                tonic::Status::internal(format!("Test mode configuration error: {e}"))
            })?;

            // Execute connector processing
            let event_params = external_services::service::EventProcessingParams {
                connector_name: &metadata_payload.connector.get_connector_name(),
                service_name: &service_name,
                service_type: $crate::utils::service_type_str(&config.server.type_),
                flow_name,
                event_config: &config.events,
                request_id: &request_id,
                lineage_ids: &metadata_payload.lineage_ids,
                reference_id: &metadata_payload.reference_id,
                resource_id: &metadata_payload.resource_id,
                shadow_mode: metadata_payload.shadow_mode,
                proxy_name: metadata_payload.proxy_name.as_deref(),
                tenant_id: &metadata_payload.tenant_id,
                merchant_id: metadata_payload.merchant_id.as_str(),
                return_raw_connector_data: config.common.return_raw_connector_data,
                connector_latency: metadata_payload.connector_latency.clone(),
            };

            // The connector round-trip is identical for both holders → written once,
            // generic over `T`. Each match arm only builds the holder-specific request
            // (+ optional injector token) and picks the monomorphisation; the two
            // `RouterDataV2` types never need to share a binding.
            #[allow(clippy::too_many_arguments)]
            async fn run_holder_flow<
                T: domain_types::payment_method_data::PaymentMethodDataTypes
                    + std::fmt::Debug
                    + Default
                    + Send
                    + Sync
                    + 'static
                    + serde::Serialize,
            >(
                connector: &domain_types::connector_types::ConnectorVariant,
                request: $request_data_type<T>,
                common_flow_data: $resource_common_data_type,
                connector_config: domain_types::router_data::ConnectorSpecificConfig,
                token_data: Option<injector::TokenData>,
                proxy: &domain_types::types::ProxyConfig,
                all_keys_required: Option<bool>,
                event_params: external_services::service::EventProcessingParams<'_>,
                test_context: Option<external_services::service::TestContext>,
                api_tag: Option<String>,
            ) -> Result<$response_type, tonic::Status>
            where
                $connector_data<T>: connector_integration::types::ConnectorDataProvider,
            {
                let connector_data: $connector_data<T> =
                    connector_integration::types::ConnectorDataProvider::from_connector_variant(connector)
                        .ok_or_else(|| tonic::Status::unimplemented("Invalid connector type for this flow"))?;

                let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                    '_,
                    $flow_marker,
                    $resource_common_data_type,
                    $request_data_type<T>,
                    $response_data_type,
                > = connector_data.connector.get_connector_integration_v2();

                let router_data = domain_types::router_data_v2::RouterDataV2::<
                    $flow_marker,
                    $resource_common_data_type,
                    $request_data_type<T>,
                    $response_data_type,
                > {
                    flow: std::marker::PhantomData,
                    resource_common_data: common_flow_data,
                    connector_config,
                    request,
                    response: Err(domain_types::router_data::ErrorResponse::default()),
                };

                let call_connector_action = connector_integration.get_call_connector_action();
                let response_result = external_services::service::execute_connector_processing_step(
                    proxy,
                    connector_integration,
                    router_data,
                    all_keys_required,
                    event_params,
                    token_data,
                    call_connector_action,
                    test_context,
                    api_tag,
                )
                .await
                .into_grpc_status()?;

                $generate_response_fn(response_result).into_grpc_status()
            }

            // Exhaustive dispatch (no `_`/`other`): a new `PaymentMethodDataAction`
            // variant or holder breaks compilation here until its routing is decided.
            let final_response = match payment_method_data_action {
                // ── Vault-aliased card proxy → VaultTokenHolder + injector ───────────
                Some(domain_types::types::PaymentMethodDataAction::CardProxy(proxy_card_details)) => {
                    tracing::info!(concat!($log_prefix, "_FLOW: INJECTOR: processing card-proxy request through injector"));

                    let token_data = <$crate::types::InjectorTokenData as domain_types::utils::ForeignTryFrom<&grpc_api_types::payments::ProxyCardDetails>>::foreign_try_from(&proxy_card_details)
                        .into_grpc_status()?
                        .0;

                    let payment_method_data = domain_types::payment_method_data::PaymentMethodData::Card(
                        <domain_types::payment_method_data::Card<domain_types::payment_method_data::VaultTokenHolder> as domain_types::utils::ForeignTryFrom<grpc_api_types::payments::ProxyCardDetails>>::foreign_try_from(proxy_card_details)
                            .into_grpc_status()?,
                    );

                    let request = $request_data_constructor((payload.clone(), Some(payment_method_data)))
                        .into_grpc_status()?;

                    run_holder_flow::<domain_types::payment_method_data::VaultTokenHolder>(
                        &metadata_payload.connector,
                        request,
                        common_flow_data,
                        connector_config,
                        Some(token_data),
                        &config.proxy,
                        $all_keys_required,
                        event_params,
                        test_context,
                        api_tag,
                    )
                    .await?
                }
                // ── Regular card → DefaultPCIHolder, direct connector call ───────────
                Some(domain_types::types::PaymentMethodDataAction::Card(card_details)) => {
                    let payment_method_data = domain_types::payment_method_data::PaymentMethodData::Card(
                        domain_types::payment_method_data::Card::<domain_types::payment_method_data::DefaultPCIHolder>::foreign_try_from(card_details)
                            .into_grpc_status()?,
                    );

                    let request = $request_data_constructor((payload.clone(), Some(payment_method_data)))
                        .into_grpc_status()?;

                    run_holder_flow::<domain_types::payment_method_data::DefaultPCIHolder>(
                        &metadata_payload.connector,
                        request,
                        common_flow_data,
                        connector_config,
                        None,
                        &config.proxy,
                        $all_keys_required,
                        event_params,
                        test_context,
                        api_tag,
                    )
                    .await?
                }
                // ── Non-card (Default) → DefaultPCIHolder, direct connector call ─────
                Some(domain_types::types::PaymentMethodDataAction::Default) => {
                    let payment_method_data: Option<domain_types::payment_method_data::PaymentMethodData<domain_types::payment_method_data::DefaultPCIHolder>> =
                        match payload.payment_method.clone() {
                            Some(pm) => Some(domain_types::payment_method_data::PaymentMethodData::convert_to_domain_model_for_non_card_payment_methods(pm).into_grpc_status()?),
                            None => None,
                        };

                    let request = $request_data_constructor((payload.clone(), payment_method_data))
                        .into_grpc_status()?;

                    run_holder_flow::<domain_types::payment_method_data::DefaultPCIHolder>(
                        &metadata_payload.connector,
                        request,
                        common_flow_data,
                        connector_config,
                        None,
                        &config.proxy,
                        $all_keys_required,
                        event_params,
                        test_context,
                        api_tag,
                    )
                    .await?
                }
                // ── No payment method data → DefaultPCIHolder, direct connector call ─
                None => {
                    let request = $request_data_constructor((payload.clone(), None))
                        .into_grpc_status()?;

                    run_holder_flow::<domain_types::payment_method_data::DefaultPCIHolder>(
                        &metadata_payload.connector,
                        request,
                        common_flow_data,
                        connector_config,
                        None,
                        &config.proxy,
                        $all_keys_required,
                        event_params,
                        test_context,
                        api_tag,
                    )
                    .await?
                }
            };

            Ok(tonic::Response::new(final_response))
        }).await;
        result
    }
};

    // Pattern without payment method data processing (original behavior)
    (
        fn_name: $fn_name:ident,
        log_prefix: $log_prefix:literal,
        request_type: $request_type:ty,
        response_type: $response_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty,
        request_data_constructor: $request_data_constructor:path,
        common_flow_data_constructor: $common_flow_data_constructor:path,
        generate_response_fn: $generate_response_fn:path,
        connector_data_type: $connector_data_type:ty,
        all_keys_required: $all_keys_required:expr
    ) => {
        async fn $fn_name(
            &self,
            request: $crate::request::RequestData<$request_type>,
        ) -> Result<tonic::Response<$response_type>, tonic::Status> {
            tracing::info!(concat!($log_prefix, "_FLOW: initiated"));
            let config = request
                .extensions
                .get::<std::sync::Arc<ucs_env::configs::Config>>()
                .cloned()
                .ok_or_else(|| tonic::Status::internal("Configuration not found in request extensions"))?;
            let service_name = request
                .extensions
                .get::<String>()
                .cloned()
                .unwrap_or_else(|| "unknown_service".to_string());
            let result = Box::pin(async{
            let $crate::request::RequestData {
                payload,
                extracted_metadata: metadata_payload,
                masked_metadata,
                extensions: _
            } = request;

            let request_id = metadata_payload.request_id.clone();
            let connector_config = metadata_payload.connector_config.clone();

            // Get connector data using ConnectorDataProvider trait
            let connector_data: $connector_data_type =
                connector_integration::types::ConnectorDataProvider::from_connector_variant(&metadata_payload.connector)
                    .ok_or_else(|| tonic::Status::unimplemented("Invalid connector type for this flow"))?;

            // Get connector integration
            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            // Create connector request data
            let specific_request_data = $request_data_constructor(payload.clone())
                .into_grpc_status()?;

            // Create common request data
            let common_flow_data = $common_flow_data_constructor((payload.clone(), config.connectors.clone(), &masked_metadata))
                .into_grpc_status()?;

            // Create router data
            let router_data = domain_types::router_data_v2::RouterDataV2::<
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > {
                flow: std::marker::PhantomData,
                resource_common_data: common_flow_data,
                connector_config,
                request: specific_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };
            let flow_name = $crate::utils::flow_marker_to_flow_name::<$flow_marker>();

            // Get API tag for the current flow

            let api_tag =
                $crate::utils::resolve_api_tag(&config, &metadata_payload, flow_name, None);

            // Create ART replay context when replay mode is enabled.
            let test_context = config.create_art_replay_context(&request_id).map_err(|e| {
                tonic::Status::internal(format!("Test mode configuration error: {e}"))
            })?;

            // Execute connector processing
            let event_params = external_services::service::EventProcessingParams {
                connector_name: &metadata_payload.connector.get_connector_name(),
                service_name: &service_name,
                service_type: $crate::utils::service_type_str(&config.server.type_),
                flow_name,
                event_config: &config.events,
                request_id: &request_id,
                lineage_ids: &metadata_payload.lineage_ids,
                reference_id: &metadata_payload.reference_id,
                resource_id: &metadata_payload.resource_id,
                shadow_mode: metadata_payload.shadow_mode,
                proxy_name: metadata_payload.proxy_name.as_deref(),
                tenant_id: &metadata_payload.tenant_id,
                merchant_id: metadata_payload.merchant_id.as_str(),
                return_raw_connector_data: config.common.return_raw_connector_data,
                connector_latency: metadata_payload.connector_latency.clone(),
            };
            let call_connector_action = connector_integration.get_call_connector_action();
            let response_result = external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                router_data,
                $all_keys_required,
                event_params,
                None,
                call_connector_action,
                test_context,
                api_tag,
            )
            .await
            .into_grpc_status()?;

            // Generate response
            let final_response = $generate_response_fn(response_result)
                .into_grpc_status()?;

            Ok(tonic::Response::new(final_response))
        }).await;
        result
    }
};
}

#[cfg(test)]
mod art_lifecycle_tests {
    use art_recorder::{
        runtime::{ArtMode, ArtRuntime},
        schema::{CsvRecording, RecordingEntry},
    };
    use common_utils::{
        events::FlowName, metadata::MaskedMetadata, request_metrics::ConnectorLatencyTracker,
    };
    use domain_types::{
        connector_types::{ConnectorEnum, ConnectorVariant},
        router_data::ConnectorSpecificConfig,
    };
    use serde::Serialize;
    use serde_json::json;
    use tonic::metadata::MetadataMap;
    use ucs_env::configs;

    use super::{
        art_order_id_from_request, art_recording_rows_fit_buffer_limit,
        create_art_runtime_for_request, record_incoming_grpc_api, resolve_api_tag, MetadataPayload,
    };
    use crate::request::RequestData;

    fn metadata_payload() -> MetadataPayload {
        MetadataPayload {
            tenant_id: "tenant_123".to_string(),
            request_id: "req_phase_5".to_string(),
            merchant_id: "merchant_123".to_string(),
            connector: ConnectorVariant::Payment(ConnectorEnum::Stripe),
            lineage_ids: common_utils::lineage::LineageIds::empty(""),
            connector_config: ConnectorSpecificConfig::NoKey,
            reference_id: None,
            art_recording_enabled: false,
            shadow_mode: false,
            resource_id: None,
            environment: None,
            proxy_name: None,
            connector_latency: ConnectorLatencyTracker::default(),
        }
    }

    fn base_config() -> configs::Config {
        let mut config = configs::Config::new().expect("default config should load");
        config.test.enabled = false;
        config
    }

    #[test]
    fn art_runtime_uses_replay_mode_when_test_config_is_enabled() {
        let mut config = base_config();
        config.test.enabled = true;
        config.test.mock_server_url = Some("http://localhost:3000/mockGateway".to_string());
        config.art_recording.enabled = true;

        let runtime = create_art_runtime_for_request(
            &config,
            &metadata_payload(),
            FlowName::Authorize,
            "PaymentService",
        );

        assert_eq!(runtime.mode(), ArtMode::Replay);
        let session = runtime
            .session()
            .expect("replay mode should create session");
        assert_eq!(session.session_id(), "req_phase_5");
        assert_eq!(session.merchant_id, "merchant_123");
        assert_eq!(session.connector, "stripe");
        assert_eq!(session.flow, "Authorize");
        assert_eq!(session.hostname, "PaymentService");
    }

    #[test]
    fn art_runtime_records_when_recording_config_is_enabled() {
        let mut config = base_config();
        config.art_recording.enabled = true;

        let runtime = create_art_runtime_for_request(
            &config,
            &metadata_payload(),
            FlowName::Authorize,
            "PaymentService",
        );

        assert_eq!(runtime.mode(), ArtMode::Record);
    }

    #[test]
    fn art_runtime_uses_record_mode_when_recording_config_and_header_are_enabled() {
        let mut config = base_config();
        config.art_recording.enabled = true;
        config.art_recording.max_entries_per_session = 1;
        let mut metadata = metadata_payload();
        metadata.art_recording_enabled = true;

        let mut runtime = create_art_runtime_for_request(
            &config,
            &metadata,
            FlowName::Authorize,
            "PaymentService",
        );

        assert_eq!(runtime.mode(), ArtMode::Record);
        runtime
            .record_entry(RecordingEntry::Timestamp(
                art_recorder::schema::TimestampEntry::new(json!("now"), "first"),
            ))
            .expect("first entry should fit max_entries_per_session");
        let error = runtime
            .record_entry(RecordingEntry::Timestamp(
                art_recorder::schema::TimestampEntry::new(json!("later"), "second"),
            ))
            .expect_err("second entry should exceed max_entries_per_session");
        assert!(error
            .to_string()
            .contains("ART recorder reached max entries per session: 1"));
    }

    #[derive(Serialize)]
    struct ArtOrderPayload {
        metadata: Option<SecretPayload>,
        merchant_order_id: Option<String>,
    }

    #[derive(Serialize)]
    struct SecretPayload {
        value: String,
    }

    #[test]
    fn art_order_id_prefers_order_id_from_metadata_value() {
        let mut metadata = metadata_payload();
        metadata.reference_id = Some("txn_123".to_string());
        let payload = ArtOrderPayload {
            metadata: Some(SecretPayload {
                value: r#"{"metadata[order_id]":"J1784704882","order_id":"fallback_order"}"#
                    .to_string(),
            }),
            merchant_order_id: Some("merchant_order_fallback".to_string()),
        };

        assert_eq!(
            art_order_id_from_request(&payload, &metadata),
            "J1784704882"
        );
    }

    #[test]
    fn art_order_id_falls_back_to_merchant_order_id_then_reference_id() {
        let mut metadata = metadata_payload();
        metadata.reference_id = Some("txn_123".to_string());
        metadata.resource_id = Some("res_123".to_string());
        let payload = ArtOrderPayload {
            metadata: Some(SecretPayload {
                value: r#"{"some_key":"some_value"}"#.to_string(),
            }),
            merchant_order_id: Some("J1784704882".to_string()),
        };
        assert_eq!(
            art_order_id_from_request(&payload, &metadata),
            "J1784704882"
        );

        let payload = ArtOrderPayload {
            metadata: None,
            merchant_order_id: None,
        };
        assert_eq!(art_order_id_from_request(&payload, &metadata), "txn_123");

        metadata.reference_id = None;
        metadata.resource_id = None;
        assert_eq!(art_order_id_from_request(&payload, &metadata), "");
    }

    #[test]
    fn resolve_api_tag_uses_config_mapping() {
        let mut config = base_config();
        config
            .api_tags
            .tags
            .insert("psync".to_string(), "CONFIG_PSYNC".to_string());

        let metadata = metadata_payload();

        assert_eq!(
            resolve_api_tag(&config, &metadata, FlowName::Psync, None).as_deref(),
            Some("CONFIG_PSYNC")
        );
    }

    #[test]
    fn art_recording_rows_fit_configured_buffer_limit() {
        let rows = vec![CsvRecording {
            sess_id: "req_123".to_string(),
            merch_id: "merchant_123".to_string(),
            ord_id: "order_123".to_string(),
            counter: 1,
            val_type: "UUID".to_string(),
            rec_entry: "x".repeat(128),
        }];

        assert!(art_recording_rows_fit_buffer_limit(&rows, 1));
        assert!(!art_recording_rows_fit_buffer_limit(&rows, 0));
    }

    #[derive(Clone, Debug, Serialize)]
    struct TestPayload {
        amount: i64,
    }

    #[derive(Clone, Debug, Serialize)]
    struct TestResponse {
        status: &'static str,
    }

    #[test]
    fn record_incoming_grpc_api_appends_eulerhs_incoming_entry() {
        let mut metadata = MetadataMap::new();
        metadata.insert(
            "x-request-id",
            "req_phase_5".parse().expect("valid metadata"),
        );
        metadata.insert(
            "x-merchant-id",
            "merchant_123".parse().expect("valid metadata"),
        );

        let request_data = RequestData {
            payload: TestPayload { amount: 100 },
            extracted_metadata: metadata_payload(),
            masked_metadata: MaskedMetadata::new(metadata, Default::default()),
            extensions: tonic::Extensions::default(),
        };
        let response = Ok(tonic::Response::new(TestResponse { status: "ok" }));
        let mut runtime = ArtRuntime::recording(
            art_recorder::runtime::SessionContext {
                request_id: "req_phase_5".to_string(),
                merchant_id: "merchant_123".to_string(),
                connector: "stripe".to_string(),
                flow: "Authorize".to_string(),
                hostname: "PaymentService".to_string(),
            },
            Some(10),
        );

        record_incoming_grpc_api(
            &mut runtime,
            &request_data,
            &response,
            FlowName::Authorize,
            "PaymentService",
            json!("2026-07-07T13:00:00Z"),
            json!("2026-07-07T13:00:01Z"),
        )
        .expect("incoming API entry should record");

        assert!(matches!(
            runtime.recorded_entries(),
            [RecordingEntry::IncomingApi(entry)]
                if entry.api_tag == "Authorize"
                    && entry.hostname == "PaymentService"
                    && entry.api_request.api_req_method == "GRPC"
                    && entry.api_request.api_req_url == "grpc://PaymentService/Authorize"
                    && entry.api_request.api_req_body == json!({"amount": 100})
                    && entry.api_response.api_res_code == 0
                    && entry.api_response.api_res_body == json!({"status": "ok"})
                    && entry.start_time == json!("2026-07-07T13:00:00Z")
                    && entry.end_time == json!("2026-07-07T13:00:01Z")
        ));
    }
}
