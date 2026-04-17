// Re-export shared interface logic from ucs_interface_common
pub use ucs_interface_common::auth::*;
pub use ucs_interface_common::config::*;
pub use ucs_interface_common::flow::*;
pub use ucs_interface_common::metadata::*;

use common_utils::{
    consts::{self, Env},
    errors::CustomResult,
    events::{Event, EventStage, FlowName, MaskedSerdeValue},
    lineage::LineageIds,
    superposition_config::{get_connector_urls, ConnectorUrls, SuperpositionConfig},
};
use domain_types::{
    connector_types, errors::IntegrationError, router_data::ConnectorSpecificConfig,
};
use error_stack::Report;
use http::request::Request;
use hyperswitch_masking;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
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
    T: serde::Serialize,
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
    current_span.record("service_name", service_name);
    current_span.record("request_body", req_body_json);
    current_span.record("gateway", connector.to_string());
    current_span.record("merchant_id", merchant_id);
    current_span.record("tenant_id", tenant_id);
    current_span.record("request_id", request_id);
    tracing::info!("Golden Log Line (incoming)");
    Ok(())
}

pub fn log_after_initialization<T>(result: &Result<tonic::Response<T>, tonic::Status>)
where
    T: serde::Serialize + std::fmt::Debug,
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
    tracing::info!("Golden Log Line (incoming)");
}

pub async fn grpc_logging_wrapper<T, F, Fut, R>(
    request: tonic::Request<T>,
    service_name: &str,
    config: Arc<configs::Config>,
    flow_name: FlowName,
    handler: F,
) -> Result<tonic::Response<R>, tonic::Status>
where
    T: serde::Serialize
        + std::fmt::Debug
        + Send
        + 'static
        + hyperswitch_masking::ErasedMaskSerialize,
    F: FnOnce(RequestData<T>) -> Fut + Send,
    Fut: std::future::Future<Output = Result<tonic::Response<R>, tonic::Status>> + Send,
    R: serde::Serialize + std::fmt::Debug + hyperswitch_masking::ErasedMaskSerialize,
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

        let result = handler(request_data).await;

        let duration = start_time.elapsed().as_millis();
        current_span.record("response_time", duration);
        log_after_initialization(&result);
        result
    }
    .await;

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
    R: serde::Serialize,
{
    let mut grpc_event = Event {
        request_id: metadata_payload.map_or("unknown".to_string(), |md| md.request_id.clone()),
        timestamp: chrono::Utc::now().timestamp().into(),
        flow_type: flow_name,
        connector: metadata_payload.map_or("unknown".to_string(), |md| md.connector.to_string()),
        url: None,
        method: None,
        stage: EventStage::GrpcRequest,
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

    match grpc_response {
        Ok(response) => grpc_event.set_grpc_success_response(response.get_ref()),
        Err(error) => grpc_event.set_grpc_error_response(error),
    }

    common_utils::emit_event_with_config(grpc_event, &config.events);
}

#[allow(clippy::result_large_err)]
pub fn get_config_from_request<T>(
    request: &tonic::Request<T>,
) -> Result<Arc<configs::Config>, tonic::Status>
where
    T: serde::Serialize,
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
        all_keys_required: $all_keys_required:expr
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
                extensions: _  // unused in macro
            } = request;

            let (connector, request_id, connector_config) = (metadata_payload.connector, metadata_payload.request_id, metadata_payload.connector_config);

            // Get connector data
            let connector_data: ConnectorData<domain_types::payment_method_data::DefaultPCIHolder> = connector_integration::types::ConnectorData::get_connector_by_name(&connector);

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


            let connectors = $crate::utils::get_resolved_connectors(
                &config,
                &metadata_payload.connector,
                &connector_config,
                metadata_payload.environment.as_deref(),
            )
            .into_grpc_status()?;

            // Create common request data
            let common_flow_data = $common_flow_data_constructor((payload.clone(), connectors, &masked_metadata))
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

            // Get API tag for the current flow
            // Note: Flows with payment_method_type should implement manually (e.g., authorize, psync)
            let api_tag = config
                .api_tags
                .get_tag(flow_name, None);

            // Create test context if test mode is enabled
            let test_context = config.test.create_test_context(&request_id).map_err(|e| {
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
            };
            let response_result = external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                router_data,
                $all_keys_required,
                event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
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
