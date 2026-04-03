pub const EMBEDDED_DEVELOPMENT_CONFIG: &str =
    include_str!("../../../../../config/development.toml");
pub const EMBEDDED_PROD_CONFIG: &str = include_str!("../../../../../config/production.toml");

use crate::types::FfiRequestData;
use domain_types::payment_method_data::DefaultPCIHolder;
use ucs_env::error::ErrorSwitch;

use grpc_api_types::payments::{
    ConnectorResponseTransformationError, Environment, IntegrationError,
};
fn get_config_for_req(
    environment: Option<Environment>,
) -> Result<std::sync::Arc<ucs_env::configs::Config>, IntegrationError> {
    let config_str = if environment == Some(Environment::Production) {
        EMBEDDED_PROD_CONFIG
    } else {
        EMBEDDED_DEVELOPMENT_CONFIG
    };
    crate::utils::load_config(config_str).map_err(|e| ErrorSwitch::switch(&e))
}

fn get_config_for_res(
    environment: Option<Environment>,
) -> Result<std::sync::Arc<ucs_env::configs::Config>, ConnectorResponseTransformationError> {
    let config_str = if environment == Some(Environment::Production) {
        EMBEDDED_PROD_CONFIG
    } else {
        EMBEDDED_DEVELOPMENT_CONFIG
    };
    crate::utils::load_config(config_str).map_err(|e| ErrorSwitch::switch(&e))
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
macro_rules! impl_flow_handlers {
    ($flow:ident, $req_type:ty, $res_type:ty, $req_svc:ident, $res_svc:ident) => {
        paste::paste! {
            pub fn [<$flow _req_handler>](
                request: FfiRequestData<$req_type>,
                environment: Option<Environment>,
            ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {
                let config = get_config_for_req(environment)?;
                $req_svc::<DefaultPCIHolder>(
                    request.payload,
                    &config,
                    request.extracted_metadata.connector,
                    request.extracted_metadata.connector_config,
                    &request.masked_metadata.unwrap_or_default(),
                )
            }

            pub fn [<$flow _res_handler>](
                request: FfiRequestData<$req_type>,
                response: domain_types::router_response_types::Response,
                environment: Option<Environment>,
            ) -> Result<$res_type, grpc_api_types::payments::ConnectorResponseTransformationError> {
                let config = get_config_for_res(environment)?;
                $res_svc::<DefaultPCIHolder>(
                    request.payload,
                    &config,
                    request.extracted_metadata.connector,
                    request.extracted_metadata.connector_config,
                    &request.masked_metadata.unwrap_or_default(),
                    response,
                )
            }
        }
    };
}

// ── Flow registrations (auto-generated) ──────────────────────────────────────
// To add a new flow: implement req_transformer!/res_transformer! in
// services/payments.rs, then run `make generate` to regenerate this file.

include!("_generated_flow_registrations.rs");

// ── Hand-written handlers (not auto-generated) ───────────────────────────────

/// handle_event handler — single-function webhook processing.
///
/// Unlike all other handlers there is no req/res split: the caller provides
/// the raw webhook payload and receives a fully-structured response directly.
/// No outgoing HTTP request is built or sent.
pub fn handle_event_handler(
    request: FfiRequestData<grpc_api_types::payments::EventServiceHandleRequest>,
    environment: Option<Environment>,
) -> Result<
    grpc_api_types::payments::EventServiceHandleResponse,
    ConnectorResponseTransformationError,
> {
    let config = get_config_for_res(environment)?;
    crate::services::payments::handle_event_transformer(
        request.payload,
        &config,
        request.extracted_metadata.connector,
        request.extracted_metadata.connector_config,
        &request.masked_metadata.unwrap_or_default(),
    )
}
