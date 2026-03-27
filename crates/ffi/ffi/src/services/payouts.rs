use grpc_api_types::payouts::{
    PayoutServiceCreateLinkRequest, PayoutServiceCreateLinkResponse,
    PayoutServiceCreateRecipientRequest, PayoutServiceCreateRecipientResponse,
    PayoutServiceCreateRequest, PayoutServiceCreateResponse,
    PayoutServiceEnrollDisburseAccountRequest, PayoutServiceEnrollDisburseAccountResponse,
    PayoutServiceGetRequest, PayoutServiceGetResponse, PayoutServiceStageRequest,
    PayoutServiceStageResponse, PayoutServiceTransferRequest, PayoutServiceTransferResponse,
    PayoutServiceVoidRequest, PayoutServiceVoidResponse,
};

use crate::macros::{payout_req_transformer, payout_res_transformer};

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
};

// payout create request transformer
payout_req_transformer!(
    fn_name: payout_create_req_transformer,
    request_type: PayoutServiceCreateRequest,
    flow_marker: PayoutCreate,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateRequest,
    response_data_type: PayoutCreateResponse,
);

// payout create response transformer
payout_res_transformer!(
    fn_name: payout_create_res_transformer,
    request_type: PayoutServiceCreateRequest,
    response_type: PayoutServiceCreateResponse,
    flow_marker: PayoutCreate,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateRequest,
    response_data_type: PayoutCreateResponse,
    generate_response_fn: generate_payout_create_response,
);

// payout transfer request transformer
payout_req_transformer!(
    fn_name: payout_transfer_req_transformer,
    request_type: PayoutServiceTransferRequest,
    flow_marker: PayoutTransfer,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutTransferRequest,
    response_data_type: PayoutTransferResponse,
);

// payout transfer response transformer
payout_res_transformer!(
    fn_name: payout_transfer_res_transformer,
    request_type: PayoutServiceTransferRequest,
    response_type: PayoutServiceTransferResponse,
    flow_marker: PayoutTransfer,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutTransferRequest,
    response_data_type: PayoutTransferResponse,
    generate_response_fn: generate_payout_transfer_response,
);

// payout get request transformer
payout_req_transformer!(
    fn_name: payout_get_req_transformer,
    request_type: PayoutServiceGetRequest,
    flow_marker: PayoutGet,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutGetRequest,
    response_data_type: PayoutGetResponse,
);

// payout get response transformer
payout_res_transformer!(
    fn_name: payout_get_res_transformer,
    request_type: PayoutServiceGetRequest,
    response_type: PayoutServiceGetResponse,
    flow_marker: PayoutGet,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutGetRequest,
    response_data_type: PayoutGetResponse,
    generate_response_fn: generate_payout_get_response,
);

// payout void request transformer
payout_req_transformer!(
    fn_name: payout_void_req_transformer,
    request_type: PayoutServiceVoidRequest,
    flow_marker: PayoutVoid,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutVoidRequest,
    response_data_type: PayoutVoidResponse,
);

// payout void response transformer
payout_res_transformer!(
    fn_name: payout_void_res_transformer,
    request_type: PayoutServiceVoidRequest,
    response_type: PayoutServiceVoidResponse,
    flow_marker: PayoutVoid,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutVoidRequest,
    response_data_type: PayoutVoidResponse,
    generate_response_fn: generate_payout_void_response,
);

// payout stage request transformer
payout_req_transformer!(
    fn_name: payout_stage_req_transformer,
    request_type: PayoutServiceStageRequest,
    flow_marker: PayoutStage,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutStageRequest,
    response_data_type: PayoutStageResponse,
);

// payout stage response transformer
payout_res_transformer!(
    fn_name: payout_stage_res_transformer,
    request_type: PayoutServiceStageRequest,
    response_type: PayoutServiceStageResponse,
    flow_marker: PayoutStage,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutStageRequest,
    response_data_type: PayoutStageResponse,
    generate_response_fn: generate_payout_stage_response,
);

// payout create_link request transformer
payout_req_transformer!(
    fn_name: payout_create_link_req_transformer,
    request_type: PayoutServiceCreateLinkRequest,
    flow_marker: PayoutCreateLink,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateLinkRequest,
    response_data_type: PayoutCreateLinkResponse,
);

// payout create_link response transformer
payout_res_transformer!(
    fn_name: payout_create_link_res_transformer,
    request_type: PayoutServiceCreateLinkRequest,
    response_type: PayoutServiceCreateLinkResponse,
    flow_marker: PayoutCreateLink,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateLinkRequest,
    response_data_type: PayoutCreateLinkResponse,
    generate_response_fn: generate_payout_create_link_response,
);

// payout create_recipient request transformer
payout_req_transformer!(
    fn_name: payout_create_recipient_req_transformer,
    request_type: PayoutServiceCreateRecipientRequest,
    flow_marker: PayoutCreateRecipient,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateRecipientRequest,
    response_data_type: PayoutCreateRecipientResponse,
);

// payout create_recipient response transformer
payout_res_transformer!(
    fn_name: payout_create_recipient_res_transformer,
    request_type: PayoutServiceCreateRecipientRequest,
    response_type: PayoutServiceCreateRecipientResponse,
    flow_marker: PayoutCreateRecipient,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutCreateRecipientRequest,
    response_data_type: PayoutCreateRecipientResponse,
    generate_response_fn: generate_payout_create_recipient_response,
);

// payout enroll_disburse_account request transformer
payout_req_transformer!(
    fn_name: payout_enroll_disburse_account_req_transformer,
    request_type: PayoutServiceEnrollDisburseAccountRequest,
    flow_marker: PayoutEnrollDisburseAccount,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutEnrollDisburseAccountRequest,
    response_data_type: PayoutEnrollDisburseAccountResponse,
);

// payout enroll_disburse_account response transformer
payout_res_transformer!(
    fn_name: payout_enroll_disburse_account_res_transformer,
    request_type: PayoutServiceEnrollDisburseAccountRequest,
    response_type: PayoutServiceEnrollDisburseAccountResponse,
    flow_marker: PayoutEnrollDisburseAccount,
    resource_common_data_type: PayoutFlowData,
    request_data_type: PayoutEnrollDisburseAccountRequest,
    response_data_type: PayoutEnrollDisburseAccountResponse,
    generate_response_fn: generate_payout_enroll_disburse_account_response,
);
