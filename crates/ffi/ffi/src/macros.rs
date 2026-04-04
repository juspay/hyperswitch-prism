//! Macros for generating request and response transformer functions
//!
//! These macros eliminate duplicate code between authorize, capture, and other flow transformers.

/// Macro to generate request transformer functions
///
/// # Example
/// ```ignore
/// req_transformer! {
///     fn_name: authorize_req_transformer,
///     request_type: PaymentServiceAuthorizeRequest,
///     flow_marker: Authorize,
///     request_data_type: PaymentsAuthorizeData<T>,
/// }
/// ```
macro_rules! req_transformer {
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
                + domain_types::types::CardConversionHelper<T>
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

/// Macro to generate response transformer functions
///
/// # Example
/// res_transformer! {
///     fn_name: authorize_res_transformer,
///     request_type: PaymentServiceAuthorizeRequest,
///     response_type: PaymentServiceAuthorizeResponse,
///     flow_marker: Authorize,
///     request_data_type: PaymentsAuthorizeData<T>,
///     generate_response_fn: generate_payment_authorize_response,
/// }
/// ```
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
                + domain_types::types::CardConversionHelper<T>
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
            response: domain_types::router_response_types::Response,
        ) -> Result<$response_type, grpc_api_types::payments::ConnectorResponseTransformationError> {
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
                grpc_api_types::payments::ConnectorResponseTransformationError {
                    error_message: ctx.to_string(),
                    error_code: ctx.error_code().to_string(),
                    http_status_code: None,
                }
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    grpc_api_types::payments::ConnectorResponseTransformationError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                    }
                })?;

            let payment_request_data: $request_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from(payload.clone())
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    grpc_api_types::payments::ConnectorResponseTransformationError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                    }
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
                common_utils::Method::Post,
                "".to_string(),
                None,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::ConnectorResponseTransformationError>| {
                common_utils::errors::ErrorSwitch::switch(e.current_context())
            })?;

            domain_types::types::$generate_response_fn(response)
                .map_err(|e: error_stack::Report<domain_types::errors::ConnectorResponseTransformationError>| {
                    let ctx = e.current_context();
                    grpc_api_types::payments::ConnectorResponseTransformationError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: ctx.http_status_code().map(u32::from),
                    }
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
                + domain_types::types::CardConversionHelper<T>
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
                + domain_types::types::CardConversionHelper<T>
                + 'static,
        >(
            payload: $request_type,
            config: &std::sync::Arc<ucs_env::configs::Config>,
            connector: domain_types::connector_types::ConnectorEnum,
            connector_config: domain_types::router_data::ConnectorSpecificConfig,
            metadata: &common_utils::metadata::MaskedMetadata,
            response: domain_types::router_response_types::Response,
        ) -> Result<$response_type, grpc_api_types::payments::ConnectorResponseTransformationError> {
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
                grpc_api_types::payments::ConnectorResponseTransformationError {
                    error_message: ctx.to_string(),
                    error_code: ctx.error_code().to_string(),
                    http_status_code: None,
                }
            })?;

            let flow_data: $resource_common_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from((
                    payload.clone(),
                    connectors,
                    metadata,
                ))
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    grpc_api_types::payments::ConnectorResponseTransformationError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                    }
                })?;

            let payment_request_data: $request_data_type =
                domain_types::utils::ForeignTryFrom::foreign_try_from(payload.clone())
                .map_err(|e: error_stack::Report<domain_types::errors::IntegrationError>| {
                    let ctx = e.current_context();
                    grpc_api_types::payments::ConnectorResponseTransformationError {
                        error_message: ctx.to_string(),
                        error_code: ctx.error_code().to_string(),
                        http_status_code: None,
                    }
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
                common_utils::Method::Post,
                "".to_string(),
                None,
            )
            .map_err(|e: error_stack::Report<domain_types::errors::ConnectorResponseTransformationError>| {
                common_utils::errors::ErrorSwitch::switch(e.current_context())
            })?;

            domain_types::payouts::types::$generate_response_fn(response)
                .map_err(|e: error_stack::Report<domain_types::errors::ConnectorResponseTransformationError>| {
                    common_utils::errors::ErrorSwitch::switch(e.current_context())
                })
        }
    };
}

pub(crate) use payout_req_transformer;
pub(crate) use payout_res_transformer;
pub(crate) use req_transformer;
pub(crate) use res_transformer;
