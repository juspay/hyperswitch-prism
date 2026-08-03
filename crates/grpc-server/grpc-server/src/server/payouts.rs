use common_utils::events::FlowName;
use connector_integration::types::PayoutConnectorData;
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount,
        PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
    },
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse, PayoutFlowData,
        PayoutGetRequest, PayoutGetResponse, PayoutStageRequest, PayoutStageResponse,
        PayoutTransferRequest, PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    payouts::types::{
        generate_payout_create_link_response, generate_payout_create_recipient_response,
        generate_payout_create_response, generate_payout_enroll_disburse_account_response,
        generate_payout_get_response, generate_payout_stage_response,
        generate_payout_transfer_response, generate_payout_void_response,
    },
    utils::ForeignTryFrom,
};
use grpc_api_types::payouts::{
    payout_service_server::PayoutService, PayoutMethodEligibilityRequest,
    PayoutMethodEligibilityResponse, PayoutServiceCreateLinkRequest,
    PayoutServiceCreateLinkResponse, PayoutServiceCreateRecipientRequest,
    PayoutServiceCreateRecipientResponse, PayoutServiceCreateRequest, PayoutServiceCreateResponse,
    PayoutServiceEnrollDisburseAccountRequest, PayoutServiceEnrollDisburseAccountResponse,
    PayoutServiceGetRequest, PayoutServiceGetResponse, PayoutServiceStageRequest,
    PayoutServiceStageResponse, PayoutServiceTransferRequest, PayoutServiceTransferResponse,
    PayoutServiceVoidRequest, PayoutServiceVoidResponse,
};
use ucs_env::error::ResultExtGrpc;

use crate::{
    implement_connector_operation,
    request::RequestData,
    utils::{get_config_from_request, grpc_logging_wrapper},
};

pub struct Payouts;

impl Payouts {
    /// Extract common request metadata (config and service_name) from gRPC request
    fn extract_request_metadata<T>(
        &self,
        request: &tonic::Request<T>,
    ) -> Result<(std::sync::Arc<ucs_env::configs::Config>, String), tonic::Status>
    where
        T: serde::Serialize,
    {
        let config = get_config_from_request(request).into_grpc_status()?;
        let service_name = request
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_else(|| "PayoutService".to_string());
        Ok((config, service_name))
    }
}

#[tonic::async_trait]
impl PayoutService for Payouts {
    #[tracing::instrument(
        name = "payout_create",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutCreate.as_str(),
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
            flow = FlowName::PayoutCreate.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create(
        &self,
        request: tonic::Request<PayoutServiceCreateRequest>,
    ) -> Result<tonic::Response<PayoutServiceCreateResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutCreate,
            |request_data| self.internal_payout_create(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_transfer",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutTransfer.as_str(),
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
            flow = FlowName::PayoutTransfer.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn transfer(
        &self,
        request: tonic::Request<PayoutServiceTransferRequest>,
    ) -> Result<tonic::Response<PayoutServiceTransferResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutTransfer,
            |request_data| self.internal_payout_transfer(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_get",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutGet.as_str(),
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
            flow = FlowName::PayoutGet.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn get(
        &self,
        request: tonic::Request<PayoutServiceGetRequest>,
    ) -> Result<tonic::Response<PayoutServiceGetResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutGet,
            |request_data| self.internal_payout_get(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_void",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutVoid.as_str(),
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
            flow = FlowName::PayoutVoid.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn void(
        &self,
        request: tonic::Request<PayoutServiceVoidRequest>,
    ) -> Result<tonic::Response<PayoutServiceVoidResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutVoid,
            |request_data| self.internal_payout_void(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_stage",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutStage.as_str(),
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
            flow = FlowName::PayoutStage.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn stage(
        &self,
        request: tonic::Request<PayoutServiceStageRequest>,
    ) -> Result<tonic::Response<PayoutServiceStageResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutStage,
            |request_data| self.internal_payout_stage(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_create_link",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutCreateLink.as_str(),
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
            flow = FlowName::PayoutCreateLink.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_link(
        &self,
        request: tonic::Request<PayoutServiceCreateLinkRequest>,
    ) -> Result<tonic::Response<PayoutServiceCreateLinkResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutCreateLink,
            |request_data| self.internal_payout_create_link(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_create_recipient",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutCreateRecipient.as_str(),
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
            flow = FlowName::PayoutCreateRecipient.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn create_recipient(
        &self,
        request: tonic::Request<PayoutServiceCreateRecipientRequest>,
    ) -> Result<tonic::Response<PayoutServiceCreateRecipientResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutCreateRecipient,
            |request_data| self.internal_payout_create_recipient(request_data),
        )
        .await
    }

    #[tracing::instrument(
        name = "payout_enroll_disburse_account",
        fields(
            name = common_utils::consts::NAME,
            service_name = tracing::field::Empty,
            service_method = FlowName::PayoutEnrollDisburseAccount.as_str(),
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
            flow = FlowName::PayoutEnrollDisburseAccount.as_str(),
            flow_specific_fields.status = tracing::field::Empty,
        )
        skip(self, request)
    )]
    async fn enroll_disburse_account(
        &self,
        request: tonic::Request<PayoutServiceEnrollDisburseAccountRequest>,
    ) -> Result<tonic::Response<PayoutServiceEnrollDisburseAccountResponse>, tonic::Status> {
        let (config, service_name) = self.extract_request_metadata(&request)?;
        grpc_logging_wrapper(
            request,
            &service_name,
            config,
            FlowName::PayoutEnrollDisburseAccount,
            |request_data| self.internal_payout_enroll_disburse_account(request_data),
        )
        .await
    }

    async fn eligibility(
        &self,
        _request: tonic::Request<PayoutMethodEligibilityRequest>,
    ) -> Result<tonic::Response<PayoutMethodEligibilityResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Eligibility check not implemented yet",
        ))
    }
}

pub(crate) trait PayoutOperationsInternal {
    fn internal_payout_create(
        &self,
        request: RequestData<PayoutServiceCreateRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceCreateResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_transfer(
        &self,
        request: RequestData<PayoutServiceTransferRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceTransferResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_get(
        &self,
        request: RequestData<PayoutServiceGetRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceGetResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_void(
        &self,
        request: RequestData<PayoutServiceVoidRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceVoidResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_stage(
        &self,
        request: RequestData<PayoutServiceStageRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceStageResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_create_link(
        &self,
        request: RequestData<PayoutServiceCreateLinkRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceCreateLinkResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_create_recipient(
        &self,
        request: RequestData<PayoutServiceCreateRecipientRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceCreateRecipientResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;

    fn internal_payout_enroll_disburse_account(
        &self,
        request: RequestData<PayoutServiceEnrollDisburseAccountRequest>,
    ) -> impl std::future::Future<
        Output = Result<
            tonic::Response<PayoutServiceEnrollDisburseAccountResponse>,
            error_stack::Report<ucs_env::error::GrpcError>,
        >,
    > + Send;
}

impl PayoutOperationsInternal for Payouts {
    implement_connector_operation!(
        fn_name: internal_payout_create,
        log_prefix: "PAYOUT_CREATE",
        request_type: PayoutServiceCreateRequest,
        response_type: PayoutServiceCreateResponse,
        flow_marker: PayoutCreate,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutCreateRequest,
        response_data_type: PayoutCreateResponse,
        request_data_constructor: PayoutCreateRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_create_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_transfer,
        log_prefix: "PAYOUT_TRANSFER",
        request_type: PayoutServiceTransferRequest,
        response_type: PayoutServiceTransferResponse,
        flow_marker: PayoutTransfer,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutTransferRequest,
        response_data_type: PayoutTransferResponse,
        request_data_constructor: PayoutTransferRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_transfer_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_get,
        log_prefix: "PAYOUT_GET",
        request_type: PayoutServiceGetRequest,
        response_type: PayoutServiceGetResponse,
        flow_marker: PayoutGet,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutGetRequest,
        response_data_type: PayoutGetResponse,
        request_data_constructor: PayoutGetRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_get_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_void,
        log_prefix: "PAYOUT_VOID",
        request_type: PayoutServiceVoidRequest,
        response_type: PayoutServiceVoidResponse,
        flow_marker: PayoutVoid,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutVoidRequest,
        response_data_type: PayoutVoidResponse,
        request_data_constructor: PayoutVoidRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_void_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_stage,
        log_prefix: "PAYOUT_STAGE",
        request_type: PayoutServiceStageRequest,
        response_type: PayoutServiceStageResponse,
        flow_marker: PayoutStage,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutStageRequest,
        response_data_type: PayoutStageResponse,
        request_data_constructor: PayoutStageRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_stage_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_create_link,
        log_prefix: "PAYOUT_CREATE_LINK",
        request_type: PayoutServiceCreateLinkRequest,
        response_type: PayoutServiceCreateLinkResponse,
        flow_marker: PayoutCreateLink,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutCreateLinkRequest,
        response_data_type: PayoutCreateLinkResponse,
        request_data_constructor: PayoutCreateLinkRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_create_link_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_create_recipient,
        log_prefix: "PAYOUT_CREATE_RECIPIENT",
        request_type: PayoutServiceCreateRecipientRequest,
        response_type: PayoutServiceCreateRecipientResponse,
        flow_marker: PayoutCreateRecipient,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutCreateRecipientRequest,
        response_data_type: PayoutCreateRecipientResponse,
        request_data_constructor: PayoutCreateRecipientRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_create_recipient_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );

    implement_connector_operation!(
        fn_name: internal_payout_enroll_disburse_account,
        log_prefix: "PAYOUT_ENROLL_DISBURSE_ACCOUNT",
        request_type: PayoutServiceEnrollDisburseAccountRequest,
        response_type: PayoutServiceEnrollDisburseAccountResponse,
        flow_marker: PayoutEnrollDisburseAccount,
        resource_common_data_type: PayoutFlowData,
        request_data_type: PayoutEnrollDisburseAccountRequest,
        response_data_type: PayoutEnrollDisburseAccountResponse,
        request_data_constructor: PayoutEnrollDisburseAccountRequest::foreign_try_from,
        common_flow_data_constructor: PayoutFlowData::foreign_try_from,
        generate_response_fn: generate_payout_enroll_disburse_account_response,
        connector_data_type: PayoutConnectorData,
        all_keys_required: None
    );
}
