use std::fmt::Debug;

use connector_integration::types::ConnectorData;
use domain_types::{
    connector_flow::{FlowName as DomainFlowName, RSync},
    connector_types::{RefundFlowData, RefundSyncData, RefundsResponseData},
    utils::ForeignTryFrom,
};
use grpc_api_types::payments::{
    refund_service_server::RefundService, RefundResponse, RefundServiceGetRequest,
};

use ucs_env::error::ResultExtGrpc;

use crate::{implement_connector_operation, request::RequestData, utils};
// Helper trait for refund operations
trait RefundOperationsInternal {
    async fn internal_get(
        &self,
        request: RequestData<RefundServiceGetRequest>,
    ) -> Result<tonic::Response<RefundResponse>, tonic::Status>;
}

#[derive(Debug, Clone)]
pub struct Refunds;

impl RefundOperationsInternal for Refunds {
    implement_connector_operation!(
        fn_name: internal_get,
        log_prefix: "REFUND_SYNC",
        request_type: RefundServiceGetRequest,
        response_type: RefundResponse,
        flow_marker: RSync,
        resource_common_data_type: RefundFlowData,
        request_data_type: RefundSyncData,
        response_data_type: RefundsResponseData,
        request_data_constructor: RefundSyncData::foreign_try_from,
        common_flow_data_constructor: RefundFlowData::foreign_try_from,
        generate_response_fn: domain_types::types::generate_refund_sync_response,
        all_keys_required: None
    );
}

#[tonic::async_trait]
impl RefundService for Refunds {
    #[tracing::instrument(
        name = "refunds_sync",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = DomainFlowName::Rsync.to_string(),
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
            flow = DomainFlowName::Rsync.to_string(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn get(
        &self,
        request: tonic::Request<RefundServiceGetRequest>,
    ) -> Result<tonic::Response<RefundResponse>, tonic::Status> {
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "RefundService".to_string());
        let config = utils::get_config_from_request(&request)?;
        Box::pin(utils::grpc_logging_wrapper(
            request,
            &service_name,
            config.clone(),
            common_utils::events::FlowName::Rsync,
            |request_data| async move { self.internal_get(request_data).await },
        ))
        .await
    }
}
