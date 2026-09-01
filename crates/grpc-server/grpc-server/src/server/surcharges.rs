use std::fmt::Debug;

use connector_integration::types::SurchargeConnectorData;
use domain_types::{
    connector_flow::{FlowName as DomainFlowName, SurchargeCalculate},
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
    },
    surcharge::types::generate_surcharge_calculate_response,
    utils::ForeignTryFrom,
};
use grpc_api_types::surcharge::{
    surcharge_service_server::SurchargeService, SurchargeServiceCalculateRequest,
    SurchargeServiceCalculateResponse,
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
}

#[tonic::async_trait]
impl SurchargeService for Surcharges {
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
