//! Macros for generating request and response transformer functions
//!
//! These macros eliminate duplicate code between authorize, capture, and other flow transformers.
//!
//! # Design: Single-arm macros with caller-provided extraction logic
//!
//! Both `req_transformer!` and `res_transformer!` have exactly **one** macro arm.
//! The caller provides two key parameters that control how payment data is built:
//!
//! - **`connector_data_type`**: The type parameter for `ConnectorData<_>`. Use `T` (the generic)
//!   for flows that don't need PMD, or a concrete type like `DefaultPCIHolder` for PMD flows.
//!
//! - **`request_data_fn`**: An expression `|payload: &$request_type| -> Result<$request_data_type, ...>`
//!   that constructs `payment_request_data` from the payload. This lets each call site define
//!   its own extraction strategy (none, required PMD, optional PMD, pre-convert + PMD, etc.)
//!   without the macro needing multiple arms.
//!
//! # Helper functions
//!
//! To keep call sites concise, helper functions are provided in `domain_types::types`:
//!
//! - **`build_request_data_with_required_pmd`**: Extracts `PaymentMethodData<DefaultPCIHolder>`
//!   from a payload's `payment_method` field, then calls
//!   `ForeignTryFrom::foreign_try_from((ftf_input, pmd))`.

/// Single-arm macro to generate request transformer functions.
///
/// # Parameters
/// - `fn_name`: Name of the generated function
/// - `request_type`: The gRPC request type
/// - `flow_marker`: The connector flow type (Authorize, Capture, etc.)
/// - `resource_common_data_type`: The flow data type (PaymentFlowData, RefundFlowData, etc.)
/// - `request_data_type`: The domain request data type
/// - `response_data_type`: The domain response data type
/// - `connector_data_type`: Type for `ConnectorData<_>` — use `T` or a concrete type
/// - `request_data_fn`: Expression `(|payload: &$request_type| -> Result<$request_data_type, Report<ApplicationErrorResponse>>)`
macro_rules! req_transformer {
    (
        fn_name: $fn_name:ident,
        request_type: $request_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty,
        connector_data_type: $connector_data_type:ty,
        request_data_fn: $request_data_fn:expr $(,)?
    ) => {
        pub fn $fn_name<
            T: domain_types::payment_method_data::PaymentMethodDataTypes
                + Default
                + Eq
                + std::fmt::Debug
                + Send
                + Sync
                + Clone
                + serde::Serialize
                + serde::de::DeserializeOwned
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
        ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {

            let connector_data: connector_integration::types::ConnectorData<$connector_data_type> =
                connector_integration::types::ConnectorData::get_connector_by_name(&connector);

            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            let connectors = ucs_interface_common::config::connectors_with_connector_config_overrides(
                &connector_config,
                config,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                common_utils::errors::ErrorSwitch::switch(e.current_context())
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            let build_request_data = $request_data_fn;
            let payment_request_data: $request_data_type = build_request_data(&payload)
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            let router_data = domain_types::router_data_v2::RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: flow_data,
                connector_config,
                request: payment_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };

            let connector_request = connector_integration
                .build_request_v2(&router_data)
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            Ok(connector_request)
        }
    };
}

/// Single-arm macro to generate response transformer functions.
///
/// # Parameters
/// Same as `req_transformer!` plus:
/// - `response_type`: The gRPC response type
/// - `generate_response_fn`: Name of the function in `domain_types::types` to produce the response
macro_rules! res_transformer {
    (
        fn_name: $fn_name:ident,
        request_type: $request_type:ty,
        response_type: $response_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty,
        generate_response_fn: $generate_response_fn:ident,
        connector_data_type: $connector_data_type:ty,
        request_data_fn: $request_data_fn:expr $(,)?
    ) => {
        pub fn $fn_name<
            T: domain_types::payment_method_data::PaymentMethodDataTypes
                + Default
                + Eq
                + std::fmt::Debug
                + Send
                + serde::Serialize
                + serde::de::DeserializeOwned
                + Clone
                + Sync
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
            response: domain_types::router_response_types::Response,
        ) -> Result<$response_type, Box<grpc_api_types::payments::ConnectorError>> {
            let connector_data: connector_integration::types::ConnectorData<$connector_data_type> =
                connector_integration::types::ConnectorData::get_connector_by_name(&connector);

            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            let connectors = ucs_interface_common::config::connectors_with_connector_config_overrides(
                &connector_config,
                config,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                let ctx = e.current_context();
                Box::new(grpc_api_types::payments::ConnectorError {
                    error_message: ctx.to_string(),
                    error_code: ctx.error_code().to_string(),
                    http_status_code: None,
                    error_info: None,
                })
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    Box::new(grpc_api_types::payments::ConnectorError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                        error_info: None,
                    })
                })?;

            let build_request_data = $request_data_fn;
            let payment_request_data: $request_data_type = build_request_data(&payload)
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    Box::new(grpc_api_types::payments::ConnectorError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                        error_info: None,
                    })
                })?;

            let router_data = domain_types::router_data_v2::RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: flow_data,
                connector_config,
                request: payment_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };

            let classified_response = match response.status_code {
                200..=399 => Ok(response),
                _ => Err(response),
            };
            let response = external_services::service::handle_connector_response(
                Ok(classified_response),
                router_data,
                &connector_integration,
                None,
                None,
                &common_utils::Method::Post.to_string(),
                "".to_string(),
                None,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::ConnectorError>| {
                Box::new(common_utils::errors::ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(e.current_context()))
            })?;

            domain_types::types::$generate_response_fn(response)
                .map_err(|e: error_stack::Report<domain_types::errors::ConnectorError>| {
                    Box::new(common_utils::errors::ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(e.current_context()))
                })
        }
    };
}

/// Macro to generate payout request transformer functions
///
/// # Example
/// payout_req_transformer!(
///     fn_name: payout_create_payout_req_transformer,
///     request_type: PayoutServiceCreateRequest,
///     flow_marker: PayoutCreate,
///     resource_common_data_type: PayoutFlowData,
///     request_data_type: PayoutCreateRequest,
///     response_data_type: PayoutCreateResponse,
/// );
/// ```
macro_rules! payout_req_transformer {
    (
        fn_name: $fn_name:ident,
        request_type: $request_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty $(,)?
    ) => {
        pub fn $fn_name<
            T: domain_types::payment_method_data::PaymentMethodDataTypes
                + Default
                + Eq
                + std::fmt::Debug
                + Send
                + Sync
                + Clone
                + serde::Serialize
                + serde::de::DeserializeOwned
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
        ) -> Result<Option<common_utils::request::Request>, grpc_api_types::payments::IntegrationError> {

            let connector_data: connector_integration::types::ConnectorData<T> =
                connector_integration::types::ConnectorData::get_connector_by_name(&connector);

            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            let connectors = ucs_interface_common::config::connectors_with_connector_config_overrides(
                &connector_config,
                config,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                common_utils::errors::ErrorSwitch::switch(e.current_context())
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            let payment_request_data: $request_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from(payload.clone())
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            let router_data = domain_types::router_data_v2::RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: flow_data,
                connector_config,
                request: payment_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };

            let connector_request = connector_integration
                .build_request_v2(&router_data)
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })?;

            Ok(connector_request)
        }
    };
}

/// Macro to generate payout response transformer functions
///
/// # Example
/// payout_res_transformer!(
///     fn_name: payout_create_payout_res_transformer,
///     request_type: PayoutServiceCreateRequest,
///     response_type: PayoutServiceCreateResponse,
///     flow_marker: PayoutCreate,
///     resource_common_data_type: PayoutFlowData,
///     request_data_type: PayoutCreateRequest,
///     response_data_type: PayoutCreateResponse,
///     generate_response_fn: generate_payout_create_response,
/// );
/// ```
macro_rules! payout_res_transformer {
    (
        fn_name: $fn_name:ident,
        request_type: $request_type:ty,
        response_type: $response_type:ty,
        flow_marker: $flow_marker:ty,
        resource_common_data_type: $resource_common_data_type:ty,
        request_data_type: $request_data_type:ty,
        response_data_type: $response_data_type:ty,
        generate_response_fn: $generate_response_fn:ident,
    ) => {
        pub fn $fn_name<
            T: domain_types::payment_method_data::PaymentMethodDataTypes
                + Default
                + Eq
                + std::fmt::Debug
                + Send
                + serde::Serialize
                + serde::de::DeserializeOwned
                + Clone
                + Sync
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
            response: domain_types::router_response_types::Response,
        ) -> Result<$response_type, Box<grpc_api_types::payments::ConnectorError>> {
            let connector_data: connector_integration::types::ConnectorData<T> =
                connector_integration::types::ConnectorData::get_connector_by_name(&connector);

            let connector_integration: interfaces::connector_integration_v2::BoxedConnectorIntegrationV2<
                '_,
                $flow_marker,
                $resource_common_data_type,
                $request_data_type,
                $response_data_type,
            > = connector_data.connector.get_connector_integration_v2();

            let connectors = ucs_interface_common::config::connectors_with_connector_config_overrides(
                &connector_config,
                config,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                let ctx = e.current_context();
                Box::new(grpc_api_types::payments::ConnectorError {
                    error_message: ctx.to_string(),
                    error_code: ctx.error_code().to_string(),
                    http_status_code: None,
                    error_info: None,
                })
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    Box::new(grpc_api_types::payments::ConnectorError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                        error_info: None,
                    })
                })?;

            let payment_request_data: $request_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from(payload.clone())
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    Box::new(grpc_api_types::payments::ConnectorError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                        error_info: None,
                    })
                })?;

            let router_data = domain_types::router_data_v2::RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: flow_data,
                connector_config,
                request: payment_request_data,
                response: Err(domain_types::router_data::ErrorResponse::default()),
            };

            // transform connector response type to common response type
            // Classify response based on status code: 2xx/3xx = success, 4xx/5xx = error
            let classified_response = match response.status_code {
                200..=399 => Ok(response),
                _ => Err(response),
            };
            let response = external_services::service::handle_connector_response(
                Ok(classified_response),
                router_data,
                &connector_integration,
                None,
                None,
                &common_utils::Method::Post.to_string(),
                "".to_string(),
                None,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::ConnectorError>| {
                Box::new(common_utils::errors::ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(e.current_context()))
            })?;

            domain_types::payouts::types::$generate_response_fn(response)
                .map_err(|e: error_stack::Report<domain_types::errors::ConnectorError>| {
                    Box::new(common_utils::errors::ErrorSwitch::<grpc_api_types::payments::ConnectorError>::switch(e.current_context()))
                })
        }
    };
}

pub(crate) use payout_req_transformer;
pub(crate) use payout_res_transformer;
pub(crate) use req_transformer;
pub(crate) use res_transformer;
