// Re-export shared interface logic from ucs_interface_common
pub use ucs_interface_common::auth::*;
pub use ucs_interface_common::config::*;
pub use ucs_interface_common::flow::*;
pub use ucs_interface_common::metadata::*;

#[cfg(feature = "log-transformations")]
use common_utils::events::apply_log_fields;
use common_utils::{
    consts::{self, Env},
    errors::CustomResult,
    events::{CompiledLogFields, Event, EventStage, FlowName, MaskedSerdeValue},
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
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use ucs_env::{
    configs,
    error::{GrpcError, InternalError, ResultExtGrpc, ResultExtGrpcError},
};

use crate::request::RequestData;

/// Record the header's fields in request's trace
pub fn record_fields_from_header<B: hyper::body::Body>(request: &Request<B>) -> tracing::Span {
    let url_path = request.uri().path();

    let span = tracing::debug_span!(
        "request",
        uri = %url_path,
        version = ?request.version(),
        // euler's `action` = the real HTTP verb (GET/POST/…); euler maps it to the `_method`
        // column. gRPC-over-HTTP2 is always POST, the HTTP gateway carries the true verb.
        action = %request.method(),
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

/// Resolves the effective [`Connectors`] for a request, applying all overrides in priority order:
///
/// 1. **Superposition** — if `x-environment` is set, resolve URLs from the superposition config
///    (dynamic, per-environment) for all connector variants (payment, payout, FRM, surcharge).
/// 2. **Caller config override** — apply any `base_url` / URL fields set in `connector_config`
///    (e.g. from the `x-connector-config` header) on top of whatever came from step 1.
/// 3. **Static fallback** — if no environment is provided, superposition is unconfigured, or
///    resolution fails, use the static TOML config as the base.
///
/// This is the **single entry point** for connector URL resolution. All flows must call this
/// instead of `connectors_with_connector_config_overrides` directly so that both override
/// sources are always applied consistently.
pub fn apply_url_overrides(
    config: &configs::Config,
    connector: &connector_types::ConnectorVariant,
    connector_config: &ConnectorSpecificConfig,
    environment: Option<&str>,
) -> CustomResult<domain_types::types::Connectors, IntegrationError> {
    use domain_types::errors::IntegrationErrorContext;

    let connector_name = connector.get_connector_name();

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
                &connector_name,
                env,
            ) {
                Some(urls) => {
                    tracing::info!("resolved URLs from superposition for environment: {}", env);
                    let patch_result = match connector {
                        connector_types::ConnectorVariant::Payment(c) => {
                            config.connectors.patch_connector_urls(c, &urls)
                        }
                        connector_types::ConnectorVariant::Payout(c) => {
                            config.connectors.patch_payout_connector_urls(c, &urls)
                        }
                        connector_types::ConnectorVariant::Frm(c) => {
                            config.connectors.patch_frm_connector_urls(c, &urls)
                        }
                        connector_types::ConnectorVariant::Surcharge(c) => {
                            config.connectors.patch_surcharge_connector_urls(c, &urls)
                        }
                        connector_types::ConnectorVariant::Authenticator(c) => config
                            .connectors
                            .patch_authenticator_connector_urls(c, &urls),
                    };
                    let patched_connectors = patch_result.map_err(|e| {
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
/// * `connector_name` - The connector name string (e.g., "stripe", "adyen")
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
///     &connector.get_connector_name(),
///     environment,
/// );
/// ```
pub fn resolve_connector_urls(
    superposition_config: Option<&SuperpositionConfig>,
    connector_name: &str,
    environment: &str,
) -> Option<ConnectorUrls> {
    let config = superposition_config?;

    let environment_lower = environment.to_lowercase();
    let connector_str = connector_name.to_lowercase();

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
    let masked_body = hyperswitch_masking::masked_serialize(&request_data.payload)
        .map_err(|e| tracing::error!("Masked serialization error: {:?}", e))
        .ok();
    let req_body_json = masked_body
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<masked serialization error>".to_string());
    let connector_name = connector.get_connector_name();
    current_span.record("service_name", service_name);
    current_span.record("request_body", req_body_json);
    current_span.record("gateway", connector_name);
    current_span.record("merchant_id", merchant_id);
    current_span.record("tenant_id", tenant_id);
    current_span.record("request_id", request_id);

    // Euler secondary-key id sources (udf_order_id / udf_customer_id / udf_txn_uuid).
    // Recorded as flat span fields via the storage API because the log transform sources by flat
    // key (it cannot reach into `request_body`) and this avoids declaring the fields on every
    // handler's `#[instrument]`. (`category` is set per-direction via static values.)
    if let Some(ids) = masked_body.as_ref().map(|body| {
        [
            (
                "merchant_order_id",
                body.get("merchant_order_id").and_then(Value::as_str),
            ),
            (
                "customer_id",
                body.get("customer")
                    .and_then(|customer| customer.get("id"))
                    .and_then(Value::as_str),
            ),
            (
                "merchant_transaction_id",
                body.get("merchant_transaction_id").and_then(Value::as_str),
            ),
        ]
    }) {
        log_utils::Storage::with_current_span_mut(|storage| {
            for (key, value) in ids {
                if let Some(value) = value {
                    storage.record_value(key, Value::String(value.to_owned()));
                }
            }
        });
    }
    tracing::info!("Golden Log Line (incoming - request)");
    Ok(())
}

pub fn log_after_initialization<T>(
    result: &Result<tonic::Response<T>, tonic::Status>,
    log_fields_enabled: bool,
    log_fields: &CompiledLogFields,
) where
    T: serde::Serialize + std::fmt::Debug,
{
    let current_span = tracing::Span::current();

    match &result {
        Ok(response) => {
            // Success HTTP status; euler's `res_code` is present on every line.
            current_span.record("status_code", 200_i64);
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
            // Connector-aware HTTP status (e.g. 422) — matches the HTTP response the caller
            // receives, not the coarse gRPC code.
            let http_status = crate::http::error::http_status_for_status(status).as_u16();
            current_span.record("status_code", i64::from(http_status));
        }
    }
    // Apply unified log fields (transformations + static values) before emitting the golden log line
    #[cfg(feature = "log-transformations")]
    if log_fields_enabled {
        apply_log_fields(log_fields);
    }
    #[cfg(not(feature = "log-transformations"))]
    {
        let _ = log_fields;
        let _ = log_fields_enabled;
    }
    tracing::info!("Golden Log Line (incoming - response)");
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
    T: serde::Serialize
        + std::fmt::Debug
        + Send
        + 'static
        + hyperswitch_masking::ErasedMaskSerialize,
    P: FnOnce(tonic::Request<T>, Arc<configs::Config>) -> Result<RequestData<T>, Report<GrpcError>>,
    F: FnOnce(
        RequestData<T>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<tonic::Response<R>, Report<GrpcError>>> + Send>,
    >,
    R: serde::Serialize + std::fmt::Debug + hyperswitch_masking::ErasedMaskSerialize,
{
    let current_span = tracing::Span::current();
    let start_time = tokio::time::Instant::now();
    let masked_request_data =
        MaskedSerdeValue::from_masked_optional(request.get_ref(), "grpc_request");
    let mut event_metadata_payload = None;
    let mut event_headers = HashMap::new();

    let handler_result = async {
        let request_data = parser(request, config.clone())?;
        log_before_initialization(&request_data, service_name).to_grpc_error()?;
        event_headers = request_data.masked_metadata.get_all_masked();
        event_metadata_payload = Some(request_data.extracted_metadata.clone());

        let result = handler(request_data).await;

        let duration = start_time.elapsed().as_millis();
        current_span.record("response_time", duration);
        result
    }
    .await;

    let grpc_response = handler_result.into_grpc_status();
    log_after_initialization(
        &grpc_response,
        config.log_fields.enabled,
        &config.log_fields.incoming,
    );

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
    T: serde::Serialize
        + std::fmt::Debug
        + Send
        + 'static
        + hyperswitch_masking::ErasedMaskSerialize,
    F: FnOnce(RequestData<T>) -> Fut + Send,
    Fut: std::future::Future<Output = Result<tonic::Response<R>, Report<GrpcError>>> + Send,
    R: serde::Serialize + std::fmt::Debug + hyperswitch_masking::ErasedMaskSerialize,
{
    let current_span = tracing::Span::current();
    let start_time = tokio::time::Instant::now();
    let masked_request_data =
        MaskedSerdeValue::from_masked_optional(request.get_ref(), "grpc_request");
    let mut event_metadata_payload = None;
    let mut event_headers = HashMap::new();

    let handler_result = async {
        let request_data = RequestData::from_grpc_request(request, config.clone())?;
        log_before_initialization(&request_data, service_name).to_grpc_error()?;
        event_headers = request_data.masked_metadata.get_all_masked();
        event_metadata_payload = Some(request_data.extracted_metadata.clone());

        let result = handler(request_data).await;

        let duration = start_time.elapsed().as_millis();
        current_span.record("response_time", duration);
        result
    }
    .await;

    let grpc_response = handler_result.into_grpc_status();
    log_after_initialization(
        &grpc_response,
        config.log_fields.enabled,
        &config.log_fields.incoming,
    );

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
    R: serde::Serialize,
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
        runtime_metadata: config.runtime_metadata.clone(),
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

// TODO: fold this into `grpc_logging_wrapper` so it is a true catch-all; today a failure here
// is logged but skips `create_and_emit_grpc_event`.
#[allow(clippy::result_large_err)]
pub fn get_config_from_request<T>(
    request: &tonic::Request<T>,
) -> Result<Arc<configs::Config>, Report<GrpcError>>
where
    T: serde::Serialize,
{
    match request.extensions().get::<Arc<configs::Config>>() {
        Some(config) => {
            tracing::info!("Using config from request extensions");
            Ok(config.clone())
        }
        None => Err(Report::new(GrpcError::from(InternalError::ConfigNotFound))),
    }
}

#[macro_export]
macro_rules! implement_connector_operation {
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
        ) -> Result<tonic::Response<$response_type>, error_stack::Report<ucs_env::error::GrpcError>> {
            use ucs_env::error::ResultExtGrpcError;
            tracing::info!(concat!($log_prefix, "_FLOW: initiated"));
            let config = request
                .extensions

                .get::<std::sync::Arc<ucs_env::configs::Config>>()
                .cloned()
                .ok_or_else(|| {
                    error_stack::Report::new(ucs_env::error::GrpcError::from(
                        ucs_env::error::InternalError::ConfigNotFound,
                    ))
                })?;
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
                        .to_grpc_error()?,
                ),
                None => None,
            };

            // Resolve effective connector URLs — applies superposition (x-environment) first,
            // then any caller-supplied base_url override from x-connector-config on top.
            let overridden_connectors =
                $crate::utils::apply_url_overrides(
                    &config,
                    &metadata_payload.connector,
                    &connector_config,
                    metadata_payload.environment.as_deref(),
                )
                .to_grpc_error()?;

            // Create common request data (shared by both the direct and proxy paths;
            // it already carries the parsed `x-external-vault-metadata` vault headers).
            let common_flow_data = $common_flow_data_constructor((payload.clone(), overridden_connectors, &masked_metadata))
                .to_grpc_error()?;

            // Calculate flow name for dynamic flow-specific configurations
            let flow_name = $crate::utils::flow_marker_to_flow_name::<$flow_marker>();

            // Get API tag for the current flow
            let api_tag = config
                .api_tags
                .get_tag(flow_name, None);

            // Create test context if test mode is enabled
            let test_context = config.test.create_test_context(&request_id).map_err(|e| {
                error_stack::Report::new(ucs_env::error::GrpcError::from(
                    ucs_env::error::InternalError::TestContextCreationFailed {
                        reason: e.to_string(),
                    },
                ))
            })?;

            // Execute connector processing
            let event_params = external_services::service::EventProcessingParams {
                connector_name: &metadata_payload.connector.get_connector_name(),
                service_name: &service_name,
                service_type: $crate::utils::service_type_str(&config.server.type_),
                flow_name,
                event_config: &config.events,
                runtime_metadata: &config.runtime_metadata,
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
                log_fields_enabled: config.log_fields.enabled,
                log_fields: &config.log_fields.outgoing,
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
            ) -> Result<$response_type, error_stack::Report<ucs_env::error::GrpcError>>
            where
                $connector_data<T>: connector_integration::types::ConnectorDataProvider,
            {
                let connector_data: $connector_data<T> =
                    connector_integration::types::ConnectorDataProvider::from_connector_variant(connector)
                        .ok_or_else(|| {
                            error_stack::Report::new(ucs_env::error::GrpcError::from(
                                domain_types::errors::IntegrationError::NotSupported {
                                    message: "Invalid connector type for this flow".to_string(),
                                    connector: "N/A",
                                    context: domain_types::errors::IntegrationErrorContext {
                                        additional_context: None,
                                        suggested_action: Some("Check connector rollout/configuration and call only flows implemented for this connector".to_string()),
                                        doc_url: None,
                                    },
                                },
                            ))
                        })?;

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
                .to_grpc_error()?;

                $generate_response_fn(response_result).to_grpc_error()
            }

            // Exhaustive dispatch (no `_`/`other`): a new `PaymentMethodDataAction`
            // variant or holder breaks compilation here until its routing is decided.
            let final_response = match payment_method_data_action {
                // ── Vault-aliased card proxy → VaultTokenHolder + injector ───────────
                Some(domain_types::types::PaymentMethodDataAction::CardProxy(proxy_card_details)) => {
                    tracing::info!(concat!($log_prefix, "_FLOW: INJECTOR: processing card-proxy request through injector"));

                    let token_data = <$crate::types::InjectorTokenData as domain_types::utils::ForeignTryFrom<&grpc_api_types::payments::ProxyCardDetails>>::foreign_try_from(&proxy_card_details)
                        .to_grpc_error()?
                        .0;

                    let payment_method_data = domain_types::payment_method_data::PaymentMethodData::Card(
                        <domain_types::payment_method_data::Card<domain_types::payment_method_data::VaultTokenHolder> as domain_types::utils::ForeignTryFrom<grpc_api_types::payments::ProxyCardDetails>>::foreign_try_from(proxy_card_details)
                            .to_grpc_error()?,
                    );

                    let request = $request_data_constructor((payload.clone(), Some(payment_method_data)))
                        .to_grpc_error()?;

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
                            .to_grpc_error()?,
                    );

                    let request = $request_data_constructor((payload.clone(), Some(payment_method_data)))
                        .to_grpc_error()?;

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
                            Some(pm) => Some(domain_types::payment_method_data::PaymentMethodData::convert_to_domain_model_for_non_card_payment_methods(pm).to_grpc_error()?),
                            None => None,
                        };

                    let request = $request_data_constructor((payload.clone(), payment_method_data))
                        .to_grpc_error()?;

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
                Some(domain_types::types::PaymentMethodDataAction::CardWithNoCvc(card_details)) => {
                    let payment_method_data =
                        domain_types::payment_method_data::PaymentMethodData::CardWithNoCvc(
                            domain_types::payment_method_data::CardWithNoCvc::foreign_try_from(
                                card_details,
                            )
                            .to_grpc_error()?,
                        );

                    let request =
                        $request_data_constructor((payload.clone(), Some(payment_method_data)))
                            .to_grpc_error()?;

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
                        .to_grpc_error()?;

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
        ) -> Result<tonic::Response<$response_type>, error_stack::Report<ucs_env::error::GrpcError>> {
            use ucs_env::error::ResultExtGrpcError;
            tracing::info!(concat!($log_prefix, "_FLOW: initiated"));
            let config = request
                .extensions
                .get::<std::sync::Arc<ucs_env::configs::Config>>()
                .cloned()
                .ok_or_else(|| {
                    error_stack::Report::new(ucs_env::error::GrpcError::from(
                        ucs_env::error::InternalError::ConfigNotFound,
                    ))
                })?;
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
                    .ok_or_else(|| {
                        error_stack::Report::new(ucs_env::error::GrpcError::from(
                            domain_types::errors::IntegrationError::NotSupported {
                                message: "Invalid connector type for this flow".to_string(),
                                connector: "N/A",
                                context: domain_types::errors::IntegrationErrorContext {
                                    additional_context: None,
                                    suggested_action: Some("Check connector rollout/configuration and call only flows implemented for this connector".to_string()),
                                    doc_url: None,
                                },
                            },
                        ))
                    })?;

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
                .to_grpc_error()?;

            // Resolve effective connector URLs — applies superposition (x-environment) first,
            // then any caller-supplied base_url override from x-connector-config on top.
            let overridden_connectors =
                $crate::utils::apply_url_overrides(
                    &config,
                    &metadata_payload.connector,
                    &connector_config,
                    metadata_payload.environment.as_deref(),
                )
                .to_grpc_error()?;

            // Create common request data
            let common_flow_data = $common_flow_data_constructor((payload.clone(), overridden_connectors, &masked_metadata))
                .to_grpc_error()?;

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

            let api_tag = config
                .api_tags
                .get_tag(flow_name, None);

            // Create test context if test mode is enabled
            let test_context = config.test.create_test_context(&request_id).map_err(|e| {
                error_stack::Report::new(ucs_env::error::GrpcError::from(
                    ucs_env::error::InternalError::TestContextCreationFailed {
                        reason: e.to_string(),
                    },
                ))
            })?;

            // Execute connector processing
            let event_params = external_services::service::EventProcessingParams {
                connector_name: &metadata_payload.connector.get_connector_name(),
                service_name: &service_name,
                service_type: $crate::utils::service_type_str(&config.server.type_),
                flow_name,
                event_config: &config.events,
                runtime_metadata: &config.runtime_metadata,
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
                log_fields_enabled: config.log_fields.enabled,
                log_fields: &config.log_fields.outgoing,
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
            .to_grpc_error()?;

            // Generate response
            let final_response = $generate_response_fn(response_result)
                .to_grpc_error()?;

            Ok(tonic::Response::new(final_response))
        }).await;
        result
    }
};
}
