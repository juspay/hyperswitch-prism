use std::fmt::Debug;

use connector_integration::types::SurchargeConnectorData;
use domain_types::{
    connector_flow::{
        FlowName as DomainFlowName, SurchargeCalculate, SurchargeRefundComplete,
        SurchargeRefundInit,
    },
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargeRefundCompleteRequest, SurchargeRefundCompleteResponse,
        SurchargeRefundInitRequest, SurchargeRefundInitResponse,
    },
    surcharge::types::{
        generate_surcharge_calculate_response, generate_surcharge_refund_complete_response,
        generate_surcharge_refund_init_response,
    },
    utils::ForeignTryFrom,
};
use grpc_api_types::surcharge::{
    surcharge_service_server::SurchargeService, SurchargeServiceCalculateRequest,
    SurchargeServiceCalculateResponse, SurchargeServiceSurchargeRefundCompleteRequest,
    SurchargeServiceSurchargeRefundCompleteResponse, SurchargeServiceSurchargeRefundInitRequest,
    SurchargeServiceSurchargeRefundInitResponse,
};

use ucs_env::error::ResultExtGrpc;

use crate::{implement_connector_operation, request::RequestData, utils};

// Helper trait for surcharge operations
trait SurchargeOperationsInternal {
    async fn internal_calculate(
        &self,
        request: RequestData<SurchargeServiceCalculateRequest>,
    ) -> Result<
        tonic::Response<SurchargeServiceCalculateResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    >;
    async fn internal_surcharge_refund_init(
        &self,
        request: RequestData<SurchargeServiceSurchargeRefundInitRequest>,
    ) -> Result<
        tonic::Response<SurchargeServiceSurchargeRefundInitResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    >;
    async fn internal_surcharge_refund_complete(
        &self,
        request: RequestData<SurchargeServiceSurchargeRefundCompleteRequest>,
    ) -> Result<
        tonic::Response<SurchargeServiceSurchargeRefundCompleteResponse>,
        error_stack::Report<ucs_env::error::GrpcError>,
    >;
}

#[derive(Debug, Clone)]
pub struct Surcharges;

impl SurchargeOperationsInternal for Surcharges {
    implement_connector_operation!(
        fn_name: internal_calculate,
        log_prefix: "SURCHARGE_CALCULATE",
        request_type: SurchargeServiceCalculateRequest,
        response_type: SurchargeServiceCalculateResponse,
        flow_marker: SurchargeCalculate,
        resource_common_data_type: SurchargeFlowData,
        request_data_type: SurchargeCalculateRequest,
        response_data_type: SurchargeCalculateResponse,
        request_data_constructor: SurchargeCalculateRequest::foreign_try_from,
        common_flow_data_constructor: SurchargeFlowData::foreign_try_from,
        generate_response_fn: generate_surcharge_calculate_response,
        connector_data_types: [SurchargeConnectorData],
        all_keys_required: None
    );
    implement_connector_operation!(
        fn_name: internal_surcharge_refund_init,
        log_prefix: "SURCHARGE_REFUND_INIT",
        request_type: SurchargeServiceSurchargeRefundInitRequest,
        response_type: SurchargeServiceSurchargeRefundInitResponse,
        flow_marker: SurchargeRefundInit,
        resource_common_data_type: SurchargeFlowData,
        request_data_type: SurchargeRefundInitRequest,
        response_data_type: SurchargeRefundInitResponse,
        request_data_constructor: SurchargeRefundInitRequest::foreign_try_from,
        common_flow_data_constructor: SurchargeFlowData::foreign_try_from,
        generate_response_fn: generate_surcharge_refund_init_response,
        connector_data_types: [SurchargeConnectorData],
        all_keys_required: None
    );
    implement_connector_operation!(
        fn_name: internal_surcharge_refund_complete,
        log_prefix: "SURCHARGE_REFUND_COMPLETE",
        request_type: SurchargeServiceSurchargeRefundCompleteRequest,
        response_type: SurchargeServiceSurchargeRefundCompleteResponse,
        flow_marker: SurchargeRefundComplete,
        resource_common_data_type: SurchargeFlowData,
        request_data_type: SurchargeRefundCompleteRequest,
        response_data_type: SurchargeRefundCompleteResponse,
        request_data_constructor: SurchargeRefundCompleteRequest::foreign_try_from,
        common_flow_data_constructor: SurchargeFlowData::foreign_try_from,
        generate_response_fn: generate_surcharge_refund_complete_response,
        connector_data_types: [SurchargeConnectorData],
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl SurchargeService for Surcharges {
    #[tracing::instrument(name = "surcharge_refund_init", fields(service_method = DomainFlowName::SurchargeRefundInit.to_string(), flow = DomainFlowName::SurchargeRefundInit.to_string()), skip(self, request))]
    async fn surcharge_refund_init(
        &self,
        request: tonic::Request<SurchargeServiceSurchargeRefundInitRequest>,
    ) -> Result<tonic::Response<SurchargeServiceSurchargeRefundInitResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "SurchargeService".to_string());
        let config = utils::get_config_from_request(&request).into_grpc_status()?;
        Box::pin(utils::grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::SurchargeRefundInit,
            |request_data| async move { self.internal_surcharge_refund_init(request_data).await },
        ))
        .await
    }

    #[tracing::instrument(name = "surcharge_refund_complete", fields(service_method = DomainFlowName::SurchargeRefundComplete.to_string(), flow = DomainFlowName::SurchargeRefundComplete.to_string()), skip(self, request))]
    async fn surcharge_refund_complete(
        &self,
        request: tonic::Request<SurchargeServiceSurchargeRefundCompleteRequest>,
    ) -> Result<tonic::Response<SurchargeServiceSurchargeRefundCompleteResponse>, tonic::Status>
    {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "SurchargeService".to_string());
        let config = utils::get_config_from_request(&request).into_grpc_status()?;
        Box::pin(utils::grpc_logging_wrapper(request, &service_name, config.clone(), common_utils::events::FlowName::SurchargeRefundComplete, |request_data| async move { self.internal_surcharge_refund_complete(request_data).await })).await
    }
    #[tracing::instrument(
        name = "surcharge_calculate",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = DomainFlowName::SurchargeCalculate.to_string(),
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
            flow = DomainFlowName::SurchargeCalculate.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn calculate(
        &self,
        request: tonic::Request<SurchargeServiceCalculateRequest>,
    ) -> Result<tonic::Response<SurchargeServiceCalculateResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "SurchargeService".to_string());
        let config = utils::get_config_from_request(&request).into_grpc_status()?;
        Box::pin(utils::grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::SurchargeCalculate,
            |request_data| async move { self.internal_calculate(request_data).await },
        ))
        .await
    }
}
