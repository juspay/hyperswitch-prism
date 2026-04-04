use crate::utils::{self, get_config_from_request};
use crate::{
    implement_connector_operation,
    request::RequestData,
    utils::{grpc_logging_wrapper, MetadataPayload},
};
use connector_integration::types::ConnectorData;
use domain_types::{
    connector_flow::{Accept, DefendDispute, FlowName, SubmitEvidence},
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        SubmitEvidenceData,
    },
    payment_method_data::DefaultPCIHolder,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    types::{
        generate_accept_dispute_response, generate_defend_dispute_response,
        generate_submit_evidence_response,
    },
    utils::ForeignTryFrom,
};
use grpc_api_types::payments::{
    dispute_service_server::DisputeService, DisputeResponse, DisputeServiceAcceptRequest,
    DisputeServiceAcceptResponse, DisputeServiceDefendRequest, DisputeServiceDefendResponse,
    DisputeServiceGetRequest, DisputeServiceSubmitEvidenceRequest,
    DisputeServiceSubmitEvidenceResponse,
};
use interfaces::connector_integration_v2::BoxedConnectorIntegrationV2;
use tracing::info;
use ucs_env::error::{IntoGrpcStatus, ResultExtGrpc};

// Helper trait for dispute operations
trait DisputeOperationsInternal {
    async fn internal_defend(
        &self,
        request: RequestData<DisputeServiceDefendRequest>,
    ) -> Result<tonic::Response<DisputeServiceDefendResponse>, tonic::Status>;
}

#[derive(Clone)]
pub struct Disputes;

impl DisputeOperationsInternal for Disputes {
    implement_connector_operation!(
        fn_name: internal_defend,
        log_prefix: "DEFEND_DISPUTE",
        request_type: DisputeServiceDefendRequest,
        response_type: DisputeServiceDefendResponse,
        flow_marker: DefendDispute,
        resource_common_data_type: DisputeFlowData,
        request_data_type: DisputeDefendData,
        response_data_type: DisputeResponseData,
        request_data_constructor: DisputeDefendData::foreign_try_from,
        common_flow_data_constructor: DisputeFlowData::foreign_try_from,
        generate_response_fn: generate_defend_dispute_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl DisputeService for Disputes {
    #[tracing::instrument(
        name = "dispute_submit_evidence",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::SubmitEvidence.to_string(),
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
            flow = FlowName::SubmitEvidence.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn submit_evidence(
        &self,
        request: tonic::Request<DisputeServiceSubmitEvidenceRequest>,
    ) -> Result<tonic::Response<DisputeServiceSubmitEvidenceResponse>, tonic::Status> {
        info!("DISPUTE_FLOW: initiated");
        let config = get_config_from_request(&request)?;
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "DisputeService".to_string());
        Box::pin(grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::SubmitEvidence,
            |request_data| {
                let service_name = service_name.clone();
                async move {
                    let payload = request_data.payload;
                    let MetadataPayload {
                        connector,
                        request_id,
                        lineage_ids,
                        connector_config,
                        reference_id,
                        resource_id,
                        shadow_mode,
                        ..
                    } = request_data.extracted_metadata;
                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        SubmitEvidence,
                        DisputeFlowData,
                        SubmitEvidenceData,
                        DisputeResponseData,
                    > = connector_data.connector.get_connector_integration_v2();

                    let dispute_data = SubmitEvidenceData::foreign_try_from(payload.clone())
                        .map_err(|e| e.into_grpc_status())?;

                    let connectors = utils::connectors_with_connector_config_overrides(
                        &connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    let dispute_flow_data =
                        DisputeFlowData::foreign_try_from((payload.clone(), connectors))
                            .map_err(|e| e.into_grpc_status())?;

                    let router_data: RouterDataV2<
                        SubmitEvidence,
                        DisputeFlowData,
                        SubmitEvidenceData,
                        DisputeResponseData,
                    > = RouterDataV2 {
                        flow: std::marker::PhantomData,
                        resource_common_data: dispute_flow_data,
                        connector_config,
                        request: dispute_data,
                        response: Err(ErrorResponse::default()),
                    };
                    let event_params = external_services::service::EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: common_utils::events::FlowName::SubmitEvidence,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &reference_id,
                        resource_id: &resource_id,
                        shadow_mode,
                    };

                    let response = Box::pin(
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

                    let dispute_response = generate_submit_evidence_response(response)
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(dispute_response))
                }
            },
        ))
        .await
    }

    #[tracing::instrument(
        name = "dispute_sync",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::Dsync.to_string(),
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
            flow = FlowName::Dsync.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn get(
        &self,
        request: tonic::Request<DisputeServiceGetRequest>,
    ) -> Result<tonic::Response<DisputeResponse>, tonic::Status> {
        // For now, return a basic dispute response
        // This will need proper implementation based on domain logic
        let config = get_config_from_request(&request)?;
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "DisputeService".to_string());
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::Dsync,
            |request_data| async {
                let _payload = request_data.payload;
                let response = DisputeResponse {
                    ..Default::default()
                };
                Ok(tonic::Response::new(response))
            },
        )
        .await
    }

    #[tracing::instrument(
        name = "dispute_defend",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::DefendDispute.to_string(),
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
            flow = FlowName::DefendDispute.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn defend(
        &self,
        request: tonic::Request<DisputeServiceDefendRequest>,
    ) -> Result<tonic::Response<DisputeServiceDefendResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "DisputeService".to_string());
        let config = get_config_from_request(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::DefendDispute,
            |request_data| async move { self.internal_defend(request_data).await },
        )
        .await
    }

    #[tracing::instrument(
        name = "dispute_accept",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::AcceptDispute.to_string(),
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
            flow = FlowName::AcceptDispute.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn accept(
        &self,
        request: tonic::Request<DisputeServiceAcceptRequest>,
    ) -> Result<tonic::Response<DisputeServiceAcceptResponse>, tonic::Status> {
        info!("DISPUTE_FLOW: initiated");
        let config = get_config_from_request(&request)?;
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "DisputeService".to_string());
        Box::pin(grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::AcceptDispute,
            |request_data| {
                let service_name = service_name.clone();
                async move {
                    let payload = request_data.payload;
                    let MetadataPayload {
                        connector,
                        request_id,
                        lineage_ids,
                        connector_config,
                        reference_id,
                        resource_id,
                        shadow_mode,
                        ..
                    } = request_data.extracted_metadata;

                    let connector_data: ConnectorData<DefaultPCIHolder> =
                        ConnectorData::get_connector_by_name(&connector);

                    let connector_integration: BoxedConnectorIntegrationV2<
                        '_,
                        Accept,
                        DisputeFlowData,
                        AcceptDisputeData,
                        DisputeResponseData,
                    > = connector_data.connector.get_connector_integration_v2();

                    let dispute_data = AcceptDisputeData::foreign_try_from(payload.clone())
                        .map_err(|e| e.into_grpc_status())?;

                    let connectors = utils::connectors_with_connector_config_overrides(
                        &connector_config,
                        &config,
                    )
                    .into_grpc_status()?;

                    let dispute_flow_data =
                        DisputeFlowData::foreign_try_from((payload.clone(), connectors))
                            .map_err(|e| e.into_grpc_status())?;

                    let router_data: RouterDataV2<
                        Accept,
                        DisputeFlowData,
                        AcceptDisputeData,
                        DisputeResponseData,
                    > = RouterDataV2 {
                        flow: std::marker::PhantomData,
                        resource_common_data: dispute_flow_data,
                        connector_config,
                        request: dispute_data,
                        response: Err(ErrorResponse::default()),
                    };

                    let event_params = external_services::service::EventProcessingParams {
                        connector_name: &connector.to_string(),
                        service_name: &service_name,
                        service_type: utils::service_type_str(&config.server.type_),
                        flow_name: common_utils::events::FlowName::AcceptDispute,
                        event_config: &config.events,
                        request_id: &request_id,
                        lineage_ids: &lineage_ids,
                        reference_id: &reference_id,
                        resource_id: &resource_id,
                        shadow_mode,
                    };

                    let response = Box::pin(
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

                    let dispute_response = generate_accept_dispute_response(response)
                        .map_err(|e| e.into_grpc_status())?;

                    Ok(tonic::Response::new(dispute_response))
                }
            },
        ))
        .await
    }
}
