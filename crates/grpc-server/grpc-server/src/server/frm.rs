use connector_integration::types::FrmConnectorData;

use std::fmt::Debug;

use domain_types::{
    connector_flow::{FlowName as DomainFlowName, PostRiskCheck, PreRiskCheck},
    frm::frm_types::{
        FrmFlowData, PostRiskCheckRequest, PostRiskCheckResponse, PreRiskCheckRequest,
        PreRiskCheckResponse,
    },
    frm::types::{generate_post_risk_check_response, generate_pre_risk_check_response},
    utils::ForeignTryFrom,
};
use grpc_api_types::frm::{
    fraud_and_risk_management_service_server::FraudAndRiskManagementService,
    FrmServicePostRiskCheckRequest, FrmServicePostRiskCheckResponse, FrmServicePreRiskCheckRequest,
    FrmServicePreRiskCheckResponse,
};

use common_utils::consts::FRM_SERVICE_NAME;
use ucs_env::error::ResultExtGrpc;

use crate::{implement_connector_operation, request::RequestData, utils};

// Helper trait for FRM operations
trait FrmOperationsInternal {
    async fn internal_pre_risk_check(
        &self,
        request: RequestData<FrmServicePreRiskCheckRequest>,
    ) -> Result<tonic::Response<FrmServicePreRiskCheckResponse>, tonic::Status>;

    async fn internal_post_risk_check(
        &self,
        request: RequestData<FrmServicePostRiskCheckRequest>,
    ) -> Result<tonic::Response<FrmServicePostRiskCheckResponse>, tonic::Status>;
}

#[derive(Debug, Clone)]
pub struct FraudAndRiskManagement;

impl FrmOperationsInternal for FraudAndRiskManagement {
    implement_connector_operation!(
        fn_name: internal_pre_risk_check,
        log_prefix: "PRE_RISK_CHECK",
        request_type: FrmServicePreRiskCheckRequest,
        response_type: FrmServicePreRiskCheckResponse,
        flow_marker: PreRiskCheck,
        resource_common_data_type: FrmFlowData,
        request_data_type: PreRiskCheckRequest,
        response_data_type: PreRiskCheckResponse,
        request_data_constructor: PreRiskCheckRequest::foreign_try_from,
        common_flow_data_constructor: FrmFlowData::foreign_try_from,
        generate_response_fn: generate_pre_risk_check_response,
        connector_data_type: FrmConnectorData,
    );

    implement_connector_operation!(
        fn_name: internal_post_risk_check,
        log_prefix: "POST_RISK_CHECK",
        request_type: FrmServicePostRiskCheckRequest,
        response_type: FrmServicePostRiskCheckResponse,
        flow_marker: PostRiskCheck,
        resource_common_data_type: FrmFlowData,
        request_data_type: PostRiskCheckRequest,
        response_data_type: PostRiskCheckResponse,
        request_data_constructor: PostRiskCheckRequest::foreign_try_from,
        common_flow_data_constructor: FrmFlowData::foreign_try_from,
        generate_response_fn: generate_post_risk_check_response,
        connector_data_type: FrmConnectorData,
    );
}

#[tonic::async_trait]
impl FraudAndRiskManagementService for FraudAndRiskManagement {
    #[tracing::instrument(
        name = "pre_risk_check",
        fields(
            name = common_utils::consts::NAME,
            service_name = FRM_SERVICE_NAME,
            service_method = DomainFlowName::PreRiskCheck.to_string(),
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
            flow = DomainFlowName::PreRiskCheck.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn pre_risk_check(
        &self,
        request: tonic::Request<FrmServicePreRiskCheckRequest>,
    ) -> Result<tonic::Response<FrmServicePreRiskCheckResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "FraudAndRiskManagementService".to_string());
        let config = utils::get_config_from_request(&request)?;
        Box::pin(utils::grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::PreRiskCheck,
            |request_data| async move { self.internal_pre_risk_check(request_data).await },
        ))
        .await
    }

    #[tracing::instrument(
        name = "post_risk_check",
        fields(
            name = common_utils::consts::NAME,
            service_name = FRM_SERVICE_NAME,
            service_method = DomainFlowName::PostRiskCheck.to_string(),
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
            flow = DomainFlowName::PostRiskCheck.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn post_risk_check(
        &self,
        request: tonic::Request<FrmServicePostRiskCheckRequest>,
    ) -> Result<tonic::Response<FrmServicePostRiskCheckResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "FraudAndRiskManagementService".to_string());
        let config = utils::get_config_from_request(&request)?;
        Box::pin(utils::grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::PostRiskCheck,
            |request_data| async move { self.internal_post_risk_check(request_data).await },
        ))
        .await
    }
}
