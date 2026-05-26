use std::fmt::Debug;

use crate::request::RequestData;
use crate::utils::{self, get_config_from_request, grpc_logging_wrapper_with_parser};
use common_enums;
use common_utils::events::FlowName;
use connector_integration::types::{ConnectorData, ConnectorDataProvider, SurchargeConnectorData};
use domain_types::{
    connector_flow::{SurchargePaymentSucceeded, SurchargeRefundSucceeded, VerifyWebhookSource},
    connector_types::VerifyWebhookSourceFlowData,
    errors::WebhookError,
    payment_method_data::DefaultPCIHolder,
    router_data::ConnectorSpecificConfig,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::{VerifyWebhookSourceResponseData, VerifyWebhookStatus},
    surcharge::surcharge_types::{
        SurchargeFlowData, SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundSucceededRequest, SurchargeRefundSucceededResponse,
    },
    utils::ForeignTryFrom,
};
use external_services::service::EventProcessingParams;
use grpc_api_types::payments::{
    event_service_server::EventService, EventServiceHandleRequest, EventServiceHandleResponse,
    EventServiceParseRequest, EventServiceParseResponse, NotifyConnectorRequest,
    NotifyConnectorResponse,
};
use interfaces::connector_integration_v2::BoxedConnectorIntegrationV2;
use ucs_env::{
    configs::Config,
    error::{IntoGrpcStatus, ResultExtGrpc},
};

#[derive(Debug, Clone)]
pub struct EventServiceImpl;

#[tonic::async_trait]
impl EventService for EventServiceImpl {
    #[tracing::instrument(
        name = "EventService::parse_event",
        skip(self, request),
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = "ParseEvent",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::IncomingWebhook.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
    )]
    async fn parse_event(
        &self,
        request: tonic::Request<EventServiceParseRequest>,
    ) -> Result<tonic::Response<EventServiceParseResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "EventService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper_with_parser(
            request,
            &service_name,
            config,
            FlowName::IncomingWebhook,
            RequestData::from_grpc_request_unauthenticated,
            |request_data| {
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let request_details =
                        domain_types::connector_types::RequestDetails::foreign_try_from(
                            payload
                                .request_details
                                .ok_or_else(|| {
                                    error_stack::report!(
                                        WebhookError::WebhookMissingRequiredField {
                                            field: "request_details"
                                        }
                                    )
                                })
                                .into_grpc_status()?,
                        )
                        .into_grpc_status()?;

                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::from_connector_variant(&metadata_payload.connector)
                            .ok_or_else(|| {
                                tonic::Status::invalid_argument("Invalid Connector Received")
                            })?;

                    let response = connector_integration::webhook_utils::parse_webhook_event(
                        connector_data,
                        request_details,
                    )
                    .into_grpc_status()?;

                    Ok(tonic::Response::new(response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "EventService::handle_event",
        skip(self, request),
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::IncomingWebhook.to_string(),
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::IncomingWebhook.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
    )]
    async fn handle_event(
        &self,
        request: tonic::Request<EventServiceHandleRequest>,
    ) -> Result<tonic::Response<EventServiceHandleResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "EventService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper_with_parser(
            request,
            &service_name,
            config.clone(),
            FlowName::IncomingWebhook,
            RequestData::from_grpc_request_unauthenticated,
            |request_data| {
                let service_name_clone = service_name.clone();
                Box::pin(async move {
                    let payload = request_data.payload;
                    let metadata_payload = request_data.extracted_metadata;
                    let connector = metadata_payload.connector.clone().as_payment().ok_or_else(|| {
                        tonic::Status::unimplemented("Surcharge connectors not supported for webhook events")
                    })?;
                    let _request_id = &metadata_payload.request_id;
                    let connector_config = &metadata_payload.connector_config;
                    let request_details = payload
                        .request_details
                        .ok_or_else(|| error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "request_details" }))
                        .into_grpc_status()
                        .and_then(|rd| {
                            domain_types::connector_types::RequestDetails::foreign_try_from(rd)
                                .into_grpc_status()
                        })?;
                    let webhook_secrets = payload
                        .webhook_secrets
                        .clone()
                        .map(|details| {
                            domain_types::connector_types::ConnectorWebhookSecrets::foreign_try_from(
                                details,
                            )
                            .map_err(|e: error_stack::Report<WebhookError>| {
                                e.into_grpc_status()
                            })
                        })
                        .transpose()?;
                    let event_context = payload
                        .event_context
                        .map(domain_types::connector_types::EventContext::foreign_try_from)
                        .transpose()
                        .map_err(|e: error_stack::Report<WebhookError>| {
                            e.into_grpc_status()
                        })?;
                    //get connector data
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    let requires_external_verification = connector_data
                        .connector
                        .requires_external_webhook_verification(config
                            .webhook_source_verification_call
                            .connectors_with_webhook_source_verification_call
                            .as_ref());

                    let source_verified = if requires_external_verification {
                        verify_webhook_source_external(
                            config.as_ref(),
                            &connector_data,
                            &request_details,
                            webhook_secrets.clone(),
                            connector_config,
                            &metadata_payload,
                            &service_name_clone,
                        )
                        .await?
                     } else {
                        match connector_data
                            .connector
                            .verify_webhook_source(
                                request_details.clone(),
                                webhook_secrets.clone(),
                                Some(connector_config.clone()),
                            )
                        {
                            Ok(result) => result,
                            Err(err) => {
                                tracing::warn!(
                                    target: "webhook",
                                    error = ?err
                                );
                                false
                            }
                        }
                    };

                    let response = connector_integration::webhook_utils::process_webhook_event(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_config.clone()),
                        source_verified,
                        payload.merchant_event_id,
                        event_context,
                    )
                    .into_grpc_status()?;

                    Ok(tonic::Response::new(response))
                })
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "EventService::notify_connector",
        skip(self, request),
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = "NotifyConnector",
            request_body = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error_message = tracing::field::Empty,
            merchant_id = tracing::field::Empty,
            gateway = tracing::field::Empty,
            request_id = tracing::field::Empty,
            status_code = tracing::field::Empty,
            message_ = "Golden Log Line (incoming)",
            response_time = tracing::field::Empty,
            tenant_id = tracing::field::Empty,
            flow = FlowName::NotifyConnector.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
    )]
    async fn notify_connector(
        &self,
        request: tonic::Request<NotifyConnectorRequest>,
    ) -> Result<tonic::Response<NotifyConnectorResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "EventService".to_string());
        let config = get_config_from_request(&request)?;
        let service_name_for_closure = service_name.clone();

        grpc_logging_wrapper_with_parser(
            request,
            &service_name,
            config.clone(),
            FlowName::NotifyConnector,
            RequestData::from_grpc_request,
            move |request_data| {
                let service_name = service_name_for_closure;
                Box::pin(async move {
                    let event_type_enum = grpc_api_types::payments::NotifyEventType::try_from(
                        request_data.payload.event_type,
                    )
                    .map_err(|error| {
                        tonic::Status::invalid_argument(format!("Invalid event type: {}", error))
                    })?;

                    match event_type_enum {
                        grpc_api_types::payments::NotifyEventType::SurchargePaymentSucceeded => {
                            Self::handle_payment_surcharge_notify(
                                request_data,
                                &service_name,
                                config,
                            )
                            .await
                        }
                        grpc_api_types::payments::NotifyEventType::SurchargeRefundSucceeded => {
                            Self::handle_refund_surcharge_notify(
                                request_data,
                                &service_name,
                                config,
                            )
                            .await
                        }
                        other_event_type => Err(tonic::Status::invalid_argument(format!(
                            "Unsupported event type: {}",
                            other_event_type.as_str_name()
                        ))),
                    }
                })
            },
        )
        .await
    }
}

impl EventServiceImpl {
    async fn handle_payment_surcharge_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<tonic::Response<NotifyConnectorResponse>, tonic::Status> {
        tracing::info!("SURCHARGE_PAYMENT_SUCCEEDED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let req = request_data.payload;

        let connector_data: SurchargeConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| tonic::Status::unimplemented("Invalid connector type for this flow"))?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            SurchargePaymentSucceededRequest::foreign_try_from(req.clone()).into_grpc_status()?;

        let common_flow_data = SurchargeFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &common_utils::metadata::MaskedMetadata::default(),
        ))
        .into_grpc_status()?;

        let router_data = RouterDataV2::<
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: common_flow_data,
            connector_config: metadata_payload.connector_config,
            request: request_data,
            response: Err(ErrorResponse::default()),
        };

        let event_params = EventProcessingParams {
            connector_name: connector_data.connector.id(),
            service_name,
            service_type: utils::service_type_str(&config.server.type_),
            flow_name: FlowName::NotifyConnector,
            event_config: &config.events,
            request_id: &req.event_id,
            lineage_ids: &metadata_payload.lineage_ids,
            reference_id: &metadata_payload.reference_id,
            resource_id: &metadata_payload.resource_id,
            shadow_mode: metadata_payload.shadow_mode,
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
        };

        let response_result = Box::pin(
            external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                router_data,
                None,
                event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                None,
                None,
            ),
        )
        .await
        .into_grpc_status()?;

        let final_response =
            domain_types::surcharge::types::generate_surcharge_payment_succeeded_response(
                response_result,
            )
            .into_grpc_status()?;

        Ok(tonic::Response::new(final_response))
    }

    async fn handle_refund_surcharge_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<tonic::Response<NotifyConnectorResponse>, tonic::Status> {
        tracing::info!("SURCHARGE_REFUND_SUCCEEDED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let req = request_data.payload;

        let connector_data: SurchargeConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| tonic::Status::unimplemented("Invalid connector type for this flow"))?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            SurchargeRefundSucceededRequest::foreign_try_from(req.clone()).into_grpc_status()?;

        let common_flow_data = SurchargeFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &common_utils::metadata::MaskedMetadata::default(),
        ))
        .into_grpc_status()?;

        let router_data = RouterDataV2::<
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        > {
            flow: std::marker::PhantomData,
            resource_common_data: common_flow_data,
            connector_config: metadata_payload.connector_config,
            request: request_data,
            response: Err(ErrorResponse::default()),
        };

        let event_params = EventProcessingParams {
            connector_name: connector_data.connector.id(),
            service_name,
            service_type: utils::service_type_str(&config.server.type_),
            flow_name: FlowName::NotifyConnector,
            event_config: &config.events,
            request_id: &req.event_id,
            lineage_ids: &metadata_payload.lineage_ids,
            reference_id: &metadata_payload.reference_id,
            resource_id: &metadata_payload.resource_id,
            shadow_mode: metadata_payload.shadow_mode,
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
        };

        let response_result = Box::pin(
            external_services::service::execute_connector_processing_step(
                &config.proxy,
                connector_integration,
                router_data,
                None,
                event_params,
                None,
                common_enums::CallConnectorAction::Trigger,
                None,
                None,
            ),
        )
        .await
        .into_grpc_status()?;

        let final_response =
            domain_types::surcharge::types::generate_surcharge_refund_succeeded_response(
                response_result,
            )
            .into_grpc_status()?;

        Ok(tonic::Response::new(final_response))
    }
}

/// For connectors requiring external webhook source verification (e.g., PayPal).
/// Executes the VerifyWebhookSource flow via the connector integration.
async fn verify_webhook_source_external(
    config: &Config,
    connector_data: &ConnectorData<DefaultPCIHolder>,
    request_details: &domain_types::connector_types::RequestDetails,
    webhook_secrets: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
    connector_config: &ConnectorSpecificConfig,
    metadata_payload: &utils::MetadataPayload,
    service_name: &str,
) -> Result<bool, tonic::Status> {
    let verify_webhook_flow_data = VerifyWebhookSourceFlowData {
        connectors: config.connectors.clone(),
        connector_request_reference_id: format!("webhook_verify_{}", metadata_payload.request_id),
        raw_connector_response: None,
        raw_connector_request: None,
        connector_response_headers: None,
    };

    let merchant_secret =
        webhook_secrets.unwrap_or_else(|| domain_types::connector_types::ConnectorWebhookSecrets {
            secret: "default_secret".to_string().into_bytes(),
            additional_secret: None,
        });

    let verify_webhook_request = VerifyWebhookSourceRequestData {
        webhook_headers: request_details.headers.clone(),
        webhook_body: request_details.body.clone(),
        merchant_secret,
        webhook_uri: request_details.uri.clone(),
    };

    let verify_webhook_router_data = RouterDataV2::<
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > {
        flow: std::marker::PhantomData,
        resource_common_data: verify_webhook_flow_data,
        connector_config: connector_config.clone(),
        request: verify_webhook_request,
        response: Err(ErrorResponse::default()),
    };

    let connector_integration: BoxedConnectorIntegrationV2<
        '_,
        VerifyWebhookSource,
        VerifyWebhookSourceFlowData,
        VerifyWebhookSourceRequestData,
        VerifyWebhookSourceResponseData,
    > = connector_data.connector.get_connector_integration_v2();

    let event_params = EventProcessingParams {
        connector_name: connector_data.connector.id(),
        service_name,
        service_type: utils::service_type_str(&config.server.type_),
        flow_name: FlowName::IncomingWebhook,
        event_config: &config.events,
        request_id: &metadata_payload.request_id,
        lineage_ids: &metadata_payload.lineage_ids,
        reference_id: &metadata_payload.reference_id,
        resource_id: &metadata_payload.resource_id,
        shadow_mode: metadata_payload.shadow_mode,
        tenant_id: &metadata_payload.tenant_id,
        merchant_id: metadata_payload.merchant_id.as_str(),
        return_raw_connector_data: config.common.return_raw_connector_data,
    };

    match Box::pin(
        external_services::service::execute_connector_processing_step(
            &config.proxy,
            connector_integration,
            verify_webhook_router_data,
            None,
            event_params,
            None,
            common_enums::CallConnectorAction::Trigger,
            None,
            None,
        ),
    )
    .await
    {
        Ok(verify_result) => Ok(match verify_result.response {
            Ok(response_data) => {
                matches!(
                    response_data.verify_webhook_status,
                    VerifyWebhookStatus::SourceVerified
                )
            }
            Err(_) => {
                tracing::warn!(
                    target: "webhook",
                    "Webhook verification returned error response for connector {}",
                    connector_data.connector.id()
                );
                false
            }
        }),
        Err(e) => {
            tracing::warn!(
                target: "webhook",
                "Webhook verification failed for connector {}: {:?}. Setting source_verified=false",
                connector_data.connector.id(),
                e
            );
            Ok(false)
        }
    }
}
