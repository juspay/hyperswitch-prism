pub const EMBEDDED_DEVELOPMENT_CONFIG: &str =
    include_str!("../../../../../config/development.toml");
pub const EMBEDDED_PROD_CONFIG: &str = include_str!("../../../../../config/production.toml");

use crate::types::FfiRequestData;
use domain_types::payment_method_data::DefaultPCIHolder;

use grpc_api_types::payments::{ConnectorError, Environment, IntegrationError};

/// Runtime config override for in-process Rust embedders of this crate (e.g.
/// Hyperswitch's UCS "library mode"). Unset by default, in which case
/// `get_config` falls back to the two embedded sandbox/production defaults
/// shipped with this crate, as before. See [`set_runtime_config`].
static RUNTIME_CONFIG_OVERRIDE: std::sync::OnceLock<std::sync::Arc<ucs_env::configs::Config>> =
    std::sync::OnceLock::new();

/// Lets an in-process Rust caller supply its own [`ucs_env::configs::Config`]
/// (e.g. built from its own toml via `Config::new_with_config_path`) instead of
/// this crate's embedded sandbox/production defaults. Every flow's handler
/// (built through [`impl_flow_handlers`]) reads config exclusively through
/// [`get_config`], so setting this once is sufficient — no other call site
/// needs to know an override exists.
///
/// Must be called before the first request is handled; the override is a
/// process-wide, set-once value (first call wins). Returns the value back as
/// `Err` if a config was already set, so a caller can detect a duplicate call
/// rather than silently keeping the first one.
///
/// Connector base URLs and other server-side settings are unaffected by this:
/// they always come from whichever `Config` is in effect (the embedded default,
/// or this override), never overridden separately.
pub fn set_runtime_config(
    config: std::sync::Arc<ucs_env::configs::Config>,
) -> Result<(), std::sync::Arc<ucs_env::configs::Config>> {
    RUNTIME_CONFIG_OVERRIDE.set(config)
}

fn get_config(
    environment: Option<Environment>,
) -> Result<std::sync::Arc<ucs_env::configs::Config>, IntegrationError> {
    if let Some(config) = RUNTIME_CONFIG_OVERRIDE.get() {
        return Ok(config.clone());
    }
    let config_str = if environment == Some(Environment::Production) {
        EMBEDDED_PROD_CONFIG
    } else {
        EMBEDDED_DEVELOPMENT_CONFIG
    };
    crate::utils::load_config(config_str).map_err(|e| common_utils::errors::ErrorSwitch::switch(&e))
}

/// Generates a `{flow}_req_handler` and `{flow}_res_handler` function pair.
///
/// Both functions load the appropriate config via `get_config(environment)` and
/// delegate directly to the supplied service-layer transformer functions.
///
/// # Arguments
/// - `$flow`      — identifier prefix for the generated function names
/// - `$req_type`  — protobuf request type (e.g. `PaymentServiceAuthorizeRequest`)
/// - `$res_type`  — protobuf response type (e.g. `PaymentServiceAuthorizeResponse`)
/// - `$req_svc`   — service function for building the connector HTTP request
/// - `$res_svc`   — service function for parsing the connector HTTP response
/// - `$connector_enum_ty` — connector enum type (ConnectorEnum for payment flows, SurchargeConnectorEnum for surcharge)
macro_rules! impl_flow_handlers {
    // Arm 1: ConnectorEnum → payment flows with hardcoded DefaultPCIHolder type
    ($flow:ident, $req_type:ty, $res_type:ty, $req_svc:ident, $res_svc:ident, domain_types::connector_types::ConnectorEnum) => {
        paste::paste! {
            pub fn [<$flow _req_handler>](
                request: FfiRequestData<$req_type>,
                environment: Option<Environment>,
            ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {
                let config = get_config(environment)?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_payment()
                    .ok_or_else(|| IntegrationError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })?;
                $req_svc::<DefaultPCIHolder>(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(IntegrationError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            suggested_action: None,
                            doc_url: None,
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                )
            }

            pub fn [<$flow _res_handler>](
                request: FfiRequestData<$req_type>,
                response: domain_types::router_response_types::Response,
                environment: Option<Environment>,
            ) -> Result<$res_type, Box<grpc_api_types::payments::ConnectorError>> {
                let config = get_config(environment).map_err(|e| ConnectorError {
                    error_message: e.error_message,
                    error_code: e.error_code,
                    http_status_code: None,
                    error_info: None,
                ..Default::default()
                })?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_payment()
                    .ok_or_else(|| ConnectorError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        http_status_code: None,
                        error_info: None,
                    ..Default::default()
                    })?;
                $res_svc::<DefaultPCIHolder>(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(ConnectorError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            http_status_code: None,
                            error_info: None,
                        ..Default::default()
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                    response,
                )
            }
        }
    };
    // Arm 2: SurchargeConnectorEnum → surcharge flows (concrete)
    ($flow:ident, $req_type:ty, $res_type:ty, $req_svc:ident, $res_svc:ident, domain_types::connector_types::SurchargeConnectorEnum) => {
        paste::paste! {
            pub fn [<$flow _req_handler>](
                request: FfiRequestData<$req_type>,
                environment: Option<Environment>,
            ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {
                let config = get_config(environment)?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_surcharge()
                    .ok_or_else(|| IntegrationError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })?;
                $req_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(IntegrationError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            suggested_action: None,
                            doc_url: None,
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                )
            }

            pub fn [<$flow _res_handler>](
                request: FfiRequestData<$req_type>,
                response: domain_types::router_response_types::Response,
                environment: Option<Environment>,
            ) -> Result<$res_type, Box<grpc_api_types::payments::ConnectorError>> {
                let config = get_config(environment).map_err(|e| ConnectorError {
                    error_message: e.error_message,
                    error_code: e.error_code,
                    http_status_code: None,
                    error_info: None,
                ..Default::default()
                })?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_surcharge()
                    .ok_or_else(|| ConnectorError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        http_status_code: None,
                        error_info: None,
                    ..Default::default()
                    })?;
                $res_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(ConnectorError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            http_status_code: None,
                            error_info: None,
                        ..Default::default()
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                    response,
                )
            }
        }
    };
    // Arm 3: FrmConnectorEnum → FRM flows (concrete, no generic T)
    ($flow:ident, $req_type:ty, $res_type:ty, $req_svc:ident, $res_svc:ident, domain_types::connector_types::FrmConnectorEnum) => {
        paste::paste! {
            pub fn [<$flow _req_handler>](
                request: FfiRequestData<$req_type>,
                environment: Option<Environment>,
            ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {
                let config = get_config(environment)?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_frm()
                    .ok_or_else(|| IntegrationError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })?;
                $req_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(IntegrationError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            suggested_action: None,
                            doc_url: None,
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                )
            }

            pub fn [<$flow _res_handler>](
                request: FfiRequestData<$req_type>,
                response: domain_types::router_response_types::Response,
                environment: Option<Environment>,
            ) -> Result<$res_type, Box<grpc_api_types::payments::ConnectorError>> {
                let config = get_config(environment).map_err(|e| ConnectorError {
                    error_message: e.error_message,
                    error_code: e.error_code,
                    http_status_code: None,
                    error_info: None,
                ..Default::default()
                })?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_frm()
                    .ok_or_else(|| ConnectorError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE".to_string(),
                        http_status_code: None,
                        error_info: None,
                    ..Default::default()
                    })?;
                $res_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(ConnectorError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            http_status_code: None,
                            error_info: None,
                        ..Default::default()
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                    response,
                )
            }
        }
    };
    // Arm 4: PayoutConnectorEnum → payout flows (concrete, no generic T)
    ($flow:ident, $req_type:ty, $res_type:ty, $req_svc:ident, $res_svc:ident, domain_types::connector_types::PayoutConnectorEnum) => {
        paste::paste! {
            pub fn [<$flow _req_handler>](
                request: FfiRequestData<$req_type>,
                environment: Option<Environment>,
            ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {
                let config = get_config(environment)?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_payout()
                    .ok_or_else(|| IntegrationError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE_FOR_PAYOUT".to_string(),
                        suggested_action: None,
                        doc_url: None,
                    })?;
                $req_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(IntegrationError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            suggested_action: None,
                            doc_url: None,
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                )
            }

            pub fn [<$flow _res_handler>](
                request: FfiRequestData<$req_type>,
                response: domain_types::router_response_types::Response,
                environment: Option<Environment>,
            ) -> Result<$res_type, Box<grpc_api_types::payments::ConnectorError>> {
                let config = get_config(environment).map_err(|e| ConnectorError {
                    error_message: e.error_message,
                    error_code: e.error_code,
                    http_status_code: None,
                    error_info: None,
                ..Default::default()
                })?;
                let connector = request
                    .extracted_metadata
                    .connector
                    .as_payout()
                    .ok_or_else(|| ConnectorError {
                        error_message: "Invalid connector type for this flow".to_string(),
                        error_code: "INVALID_CONNECTOR_TYPE_FOR_PAYOUT".to_string(),
                        http_status_code: None,
                        error_info: None,
                    ..Default::default()
                    })?;
                $res_svc(
                    request.payload,
                    &config,
                    connector,
                    request
                        .extracted_metadata
                        .connector_config
                        .ok_or(ConnectorError {
                            error_message: "Missing connector config".to_string(),
                            error_code: "MISSING_CONNECTOR_CONFIG".to_string(),
                            http_status_code: None,
                            error_info: None,
                        ..Default::default()
                        })?,
                    &request.masked_metadata.unwrap_or_default(),
                    response,
                )
            }
        }
    };
}
// To add a new flow: implement req_transformer!/res_transformer! in
// services/payments.rs, then run `make generate` to regenerate this file.

include!("_generated_flow_registrations.rs");

// ── Hand-written handlers (not auto-generated) ───────────────────────────────

/// parse_event handler — stateless webhook event type and resource reference extraction.
///
/// No secrets, no context. Returns the event type and resource IDs
/// extracted purely from the raw webhook payload.
pub fn parse_event_handler(
    request: FfiRequestData<grpc_api_types::payments::EventServiceParseRequest>,
    environment: Option<Environment>,
) -> Result<grpc_api_types::payments::EventServiceParseResponse, IntegrationError> {
    let config = get_config(environment)?;
    let connector = request
        .extracted_metadata
        .connector
        .as_payment()
        .ok_or_else(|| IntegrationError {
            error_message: "Invalid connector type for this flow".to_string(),
            error_code: "INVALID_CONNECTOR_TYPE".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;
    crate::services::payments::parse_event_transformer(
        request.payload,
        &config,
        connector,
        request.extracted_metadata.connector_config,
        &request.masked_metadata.unwrap_or_default(),
    )
}

/// handle_event handler — synchronous webhook processing (single-step, no outgoing HTTP).
///
/// Unlike all other handlers there is no req/res split: the caller provides
/// the raw webhook payload and receives a fully-structured response directly.
/// No outgoing HTTP request is built or sent.
pub fn handle_event_handler(
    request: FfiRequestData<grpc_api_types::payments::EventServiceHandleRequest>,
    environment: Option<Environment>,
) -> Result<grpc_api_types::payments::EventServiceHandleResponse, IntegrationError> {
    let config = get_config(environment)?;
    let connector = request
        .extracted_metadata
        .connector
        .as_payment()
        .ok_or_else(|| IntegrationError {
            error_message: "Invalid connector type for this flow".to_string(),
            error_code: "INVALID_CONNECTOR_TYPE".to_string(),
            suggested_action: None,
            doc_url: None,
        })?;
    crate::services::payments::handle_event_transformer(
        request.payload,
        &config,
        connector,
        request.extracted_metadata.connector_config,
        &request.masked_metadata.unwrap_or_default(),
    )
}
