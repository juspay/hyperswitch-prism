use std::fmt::Debug;

use grpc_api_types::payments::IntegrityCheck as ProtoIntegrityCheck;

use crate::request::RequestData;
use crate::utils::{self, get_config_from_request, grpc_logging_wrapper_with_parser};
use common_enums;
use common_utils::events::FlowName;
use connector_integration::types::{
    ConnectorData, ConnectorDataProvider, FrmConnectorData, SurchargeConnectorData,
};
use domain_types::{
    connector_flow::{
        FrmChargebackReceived, FrmPaymentOutcome, FrmRefundProcessed, SurchargePaymentSucceeded,
        SurchargeRefundSucceeded, VerifyWebhookSource,
    },
    connector_types::{VerifyWebhookSourceFlowData, WebhookIntegrityCheck},
    errors::WebhookError,
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse,
    },
    frm::types::{
        generate_frm_chargeback_received_response, generate_frm_payment_outcome_response,
        generate_frm_refund_processed_response,
    },
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
    utils::{ForeignFrom, ForeignTryFrom},
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
    error::{ReportExtGrpcError, ResultExtGrpc, ResultExtGrpcError},
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
            error_response_details = tracing::field::Empty,
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
        let config = get_config_from_request(&request).into_grpc_status()?;
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
                                .to_grpc_error()?,
                        )
                        .to_grpc_error()?;

                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::from_connector_variant(&metadata_payload.connector)
                            .ok_or_else(|| {
                                ucs_env::error::GrpcError::from(
                                    domain_types::errors::IntegrationError::InvalidDataFormat {
                                        field_name: "connector",
                                        context: domain_types::errors::IntegrationErrorContext {
                                            suggested_action: None,
                                            doc_url: None,
                                            additional_context: Some(
                                                metadata_payload.connector.get_connector_name(),
                                            ),
                                        },
                                    },
                                )
                            })?;

                    let response = connector_integration::webhook_utils::parse_webhook_event(
                        connector_data,
                        request_details,
                    )
                    .to_grpc_error()?;

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
            error_response_details = tracing::field::Empty,
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
        let config = get_config_from_request(&request).into_grpc_status()?;
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
                        ucs_env::error::GrpcError::from(
                            domain_types::errors::IntegrationError::FlowNotSupported {
                                flow: FlowName::IncomingWebhook.to_string(),
                                connector: metadata_payload.connector.get_connector_name(),
                                context: domain_types::errors::IntegrationErrorContext {
                                    suggested_action: Some(
                                        "Check connector rollout/configuration and call only flows implemented for this connector"
                                            .to_string(),
                                    ),
                                    doc_url: None,
                                    additional_context: None,
                                },
                            },
                        )
                    })?;
                    let _request_id = &metadata_payload.request_id;
                    let connector_config = &metadata_payload.connector_config;
                    let request_details = payload
                        .request_details
                        .ok_or_else(|| error_stack::report!(WebhookError::WebhookMissingRequiredField { field: "request_details" }))
                        .to_grpc_error()
                        .and_then(|rd| {
                            domain_types::connector_types::RequestDetails::foreign_try_from(rd)
                                .to_grpc_error()
                        })?;
                    let webhook_secrets = payload
                        .webhook_secrets
                        .clone()
                        .map(|details| {
                            domain_types::connector_types::ConnectorWebhookSecrets::foreign_try_from(
                                details,
                            )
                            .map_err(|e: error_stack::Report<WebhookError>| {
                                e.to_grpc_error()
                            })
                        })
                        .transpose()?;
                    let event_context = payload
                        .event_context
                        .map(domain_types::connector_types::EventContext::foreign_try_from)
                        .transpose()
                        .map_err(|e: error_stack::Report<WebhookError>| {
                            e.to_grpc_error()
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

                    let supported_integrity_checks: Vec<WebhookIntegrityCheck> = connector_data
                        .connector
                        .get_webhook_integrity_checks();

                    let mut response = connector_integration::webhook_utils::process_webhook_event(
                        connector_data,
                        request_details,
                        webhook_secrets,
                        Some(connector_config.clone()),
                        source_verified,
                        payload.merchant_event_id,
                        event_context,
                    )
                    .to_grpc_error()?;

                    response.supported_integrity_checks = supported_integrity_checks
                        .into_iter()
                        .map(|c| i32::from(ProtoIntegrityCheck::foreign_from(c)))
                        .collect();

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
            error_response_details = tracing::field::Empty,
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
        let config = get_config_from_request(&request).into_grpc_status()?;
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
                        error_stack::Report::new(ucs_env::error::GrpcError::from(
                            domain_types::errors::IntegrationError::InvalidDataFormat {
                                field_name: "event_type",
                                context: domain_types::errors::IntegrationErrorContext {
                                    suggested_action: None,
                                    doc_url: None,
                                    additional_context: Some(error.to_string()),
                                },
                            },
                        ))
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
                        grpc_api_types::payments::NotifyEventType::FrmPaymentSucceeded
                        | grpc_api_types::payments::NotifyEventType::FrmPaymentFailure => {
                            Self::handle_frm_payment_outcome_notify(
                                request_data,
                                &service_name,
                                config,
                            )
                            .await
                        }
                        grpc_api_types::payments::NotifyEventType::FrmRefundProcessed => {
                            Self::handle_frm_refund_processed_notify(
                                request_data,
                                &service_name,
                                config,
                            )
                            .await
                        }
                        grpc_api_types::payments::NotifyEventType::FrmChargebackReceived => {
                            Self::handle_frm_chargeback_received_notify(
                                request_data,
                                &service_name,
                                config,
                            )
                            .await
                        }
                        grpc_api_types::payments::NotifyEventType::Unspecified => {
                            Err(error_stack::Report::new(ucs_env::error::GrpcError::from(
                                domain_types::errors::IntegrationError::InvalidDataFormat {
                                    field_name: "event_type",
                                    context: domain_types::errors::IntegrationErrorContext {
                                        suggested_action: Some(
                                            "Set event_type to one of SURCHARGE_PAYMENT_SUCCEEDED, \
                                             SURCHARGE_REFUND_SUCCEEDED, FRM_PAYMENT_SUCCEEDED, \
                                             FRM_PAYMENT_FAILURE, FRM_REFUND_PROCESSED or \
                                             FRM_CHARGEBACK_RECEIVED"
                                                .to_string(),
                                        ),
                                        doc_url: None,
                                        additional_context: None,
                                    },
                                },
                            )))
                        }
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
    ) -> Result<
        tonic::Response<NotifyConnectorResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    > {
        tracing::info!("SURCHARGE_PAYMENT_SUCCEEDED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let masked_metadata = request_data.masked_metadata;
        let req = request_data.payload;

        let connector_data: SurchargeConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| {
            ucs_env::error::GrpcError::from(
                domain_types::errors::IntegrationError::FlowNotSupported {
                    flow: "SurchargePaymentSucceeded".to_string(),
                    connector: metadata_payload.connector.get_connector_name(),
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Check connector rollout/configuration and call only flows implemented for this connector"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                },
            )
        })?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SurchargePaymentSucceeded,
            SurchargeFlowData,
            SurchargePaymentSucceededRequest,
            SurchargePaymentSucceededResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            SurchargePaymentSucceededRequest::foreign_try_from(req.clone()).to_grpc_error()?;

        let common_flow_data = SurchargeFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &masked_metadata,
        ))
        .to_grpc_error()?;

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
            proxy_name: metadata_payload.proxy_name.as_deref(),
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
            connector_latency: metadata_payload.connector_latency.clone(),
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
        .to_grpc_error()?;

        let final_response =
            domain_types::surcharge::types::generate_surcharge_payment_succeeded_response(
                response_result,
            )
            .to_grpc_error()?;

        Ok(tonic::Response::new(final_response))
    }

    async fn handle_refund_surcharge_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<
        tonic::Response<NotifyConnectorResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    > {
        tracing::info!("SURCHARGE_REFUND_SUCCEEDED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let masked_metadata = request_data.masked_metadata;
        let req = request_data.payload;

        let connector_data: SurchargeConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| {
            ucs_env::error::GrpcError::from(
                domain_types::errors::IntegrationError::FlowNotSupported {
                    flow: "SurchargeRefundSucceeded".to_string(),
                    connector: metadata_payload.connector.get_connector_name(),
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Check connector rollout/configuration and call only flows implemented for this connector"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                },
            )
        })?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SurchargeRefundSucceeded,
            SurchargeFlowData,
            SurchargeRefundSucceededRequest,
            SurchargeRefundSucceededResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            SurchargeRefundSucceededRequest::foreign_try_from(req.clone()).to_grpc_error()?;

        let common_flow_data = SurchargeFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &masked_metadata,
        ))
        .to_grpc_error()?;

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
            proxy_name: metadata_payload.proxy_name.as_deref(),
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
            connector_latency: metadata_payload.connector_latency.clone(),
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
        .to_grpc_error()?;

        let final_response =
            domain_types::surcharge::types::generate_surcharge_refund_succeeded_response(
                response_result,
            )
            .to_grpc_error()?;

        Ok(tonic::Response::new(final_response))
    }

    async fn handle_frm_payment_outcome_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<
        tonic::Response<NotifyConnectorResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    > {
        tracing::info!("FRM_PAYMENT_OUTCOME_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let masked_metadata = request_data.masked_metadata;
        let req = request_data.payload;

        let connector_data: FrmConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| {
            ucs_env::error::GrpcError::from(
                domain_types::errors::IntegrationError::FlowNotSupported {
                    flow: "FrmPaymentOutcome".to_string(),
                    connector: metadata_payload.connector.get_connector_name(),
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Check connector rollout/configuration and call only flows implemented for this connector"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                },
            )
        })?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            FrmPaymentOutcome,
            FrmFlowData,
            FrmPaymentOutcomeRequest,
            FrmPaymentOutcomeResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            FrmPaymentOutcomeRequest::foreign_try_from(req.clone()).to_grpc_error()?;

        let common_flow_data = FrmFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &masked_metadata,
        ))
        .to_grpc_error()?;

        let router_data = RouterDataV2::<
            FrmPaymentOutcome,
            FrmFlowData,
            FrmPaymentOutcomeRequest,
            FrmPaymentOutcomeResponse,
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
            proxy_name: metadata_payload.proxy_name.as_deref(),
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
            connector_latency: metadata_payload.connector_latency.clone(),
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
        .to_grpc_error()?;

        let final_response =
            generate_frm_payment_outcome_response(response_result).to_grpc_error()?;

        Ok(tonic::Response::new(final_response))
    }

    async fn handle_frm_refund_processed_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<
        tonic::Response<NotifyConnectorResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    > {
        tracing::info!("FRM_REFUND_PROCESSED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let masked_metadata = request_data.masked_metadata;
        let req = request_data.payload;

        let connector_data: FrmConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| {
            ucs_env::error::GrpcError::from(
                domain_types::errors::IntegrationError::FlowNotSupported {
                    flow: "FrmRefundProcessed".to_string(),
                    connector: metadata_payload.connector.get_connector_name(),
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Check connector rollout/configuration and call only flows implemented for this connector"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                },
            )
        })?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            FrmRefundProcessed,
            FrmFlowData,
            FrmRefundProcessedRequest,
            FrmRefundProcessedResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            FrmRefundProcessedRequest::foreign_try_from(req.clone()).to_grpc_error()?;

        let common_flow_data = FrmFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &masked_metadata,
        ))
        .to_grpc_error()?;

        let router_data = RouterDataV2::<
            FrmRefundProcessed,
            FrmFlowData,
            FrmRefundProcessedRequest,
            FrmRefundProcessedResponse,
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
            proxy_name: metadata_payload.proxy_name.as_deref(),
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
            connector_latency: metadata_payload.connector_latency.clone(),
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
        .to_grpc_error()?;

        let final_response =
            generate_frm_refund_processed_response(response_result).to_grpc_error()?;

        Ok(tonic::Response::new(final_response))
    }

    async fn handle_frm_chargeback_received_notify(
        request_data: RequestData<NotifyConnectorRequest>,
        service_name: &str,
        config: std::sync::Arc<Config>,
    ) -> Result<
        tonic::Response<NotifyConnectorResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    > {
        tracing::info!("FRM_CHARGEBACK_RECEIVED_FLOW: initiated");

        let metadata_payload = request_data.extracted_metadata;
        let masked_metadata = request_data.masked_metadata;
        let req = request_data.payload;

        let connector_data: FrmConnectorData = ConnectorDataProvider::from_connector_variant(
            &metadata_payload.connector,
        )
        .ok_or_else(|| {
            ucs_env::error::GrpcError::from(
                domain_types::errors::IntegrationError::FlowNotSupported {
                    flow: "FrmChargebackReceived".to_string(),
                    connector: metadata_payload.connector.get_connector_name(),
                    context: domain_types::errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Check connector rollout/configuration and call only flows implemented for this connector"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: None,
                    },
                },
            )
        })?;

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            FrmChargebackReceived,
            FrmFlowData,
            FrmChargebackReceivedRequest,
            FrmChargebackReceivedResponse,
        > = connector_data.connector.get_connector_integration_v2();

        let request_data =
            FrmChargebackReceivedRequest::foreign_try_from(req.clone()).to_grpc_error()?;

        let common_flow_data = FrmFlowData::foreign_try_from((
            req.clone(),
            config.connectors.clone(),
            &masked_metadata,
        ))
        .to_grpc_error()?;

        let router_data = RouterDataV2::<
            FrmChargebackReceived,
            FrmFlowData,
            FrmChargebackReceivedRequest,
            FrmChargebackReceivedResponse,
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
            proxy_name: metadata_payload.proxy_name.as_deref(),
            tenant_id: &metadata_payload.tenant_id,
            merchant_id: metadata_payload.merchant_id.as_str(),
            return_raw_connector_data: config.common.return_raw_connector_data,
            connector_latency: metadata_payload.connector_latency.clone(),
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
        .to_grpc_error()?;

        let final_response =
            generate_frm_chargeback_received_response(response_result).to_grpc_error()?;

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
) -> Result<bool, error_stack::Report<ucs_env::error::GrpcError>> {
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
        proxy_name: metadata_payload.proxy_name.as_deref(),
        tenant_id: &metadata_payload.tenant_id,
        merchant_id: metadata_payload.merchant_id.as_str(),
        return_raw_connector_data: config.common.return_raw_connector_data,
        connector_latency: metadata_payload.connector_latency.clone(),
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
