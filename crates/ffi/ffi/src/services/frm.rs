use grpc_api_types::frm::{
    FrmServicePreRiskCheckRequest, FrmServicePreRiskCheckResponse,
    FrmServicePostRiskCheckRequest, FrmServicePostRiskCheckResponse,
};

use crate::macros::{frm_req_transformer, frm_res_transformer};

use domain_types::{
    connector_flow::{PreRiskCheck, PostRiskCheck},
    frm::frm_types::{
        FrmFlowData, PreRiskCheckRequest, PreRiskCheckResponse,
        PostRiskCheckRequest, PostRiskCheckResponse,
    },
};

// PreRiskCheck request transformer
frm_req_transformer!(
    fn_name: pre_risk_check_req_transformer,
    request_type: FrmServicePreRiskCheckRequest,
    flow_marker: PreRiskCheck,
    resource_common_data_type: FrmFlowData,
    request_data_type: PreRiskCheckRequest,
    response_data_type: PreRiskCheckResponse,
);

// PreRiskCheck response transformer
frm_res_transformer!(
    fn_name: pre_risk_check_res_transformer,
    request_type: FrmServicePreRiskCheckRequest,
    response_type: FrmServicePreRiskCheckResponse,
    flow_marker: PreRiskCheck,
    resource_common_data_type: FrmFlowData,
    request_data_type: PreRiskCheckRequest,
    response_data_type: PreRiskCheckResponse,
    generate_response_fn: generate_pre_risk_check_response,
);

// PostRiskCheck request transformer
frm_req_transformer!(
    fn_name: post_risk_check_req_transformer,
    request_type: FrmServicePostRiskCheckRequest,
    flow_marker: PostRiskCheck,
    resource_common_data_type: FrmFlowData,
    request_data_type: PostRiskCheckRequest,
    response_data_type: PostRiskCheckResponse,
);

// PostRiskCheck response transformer
frm_res_transformer!(
    fn_name: post_risk_check_res_transformer,
    request_type: FrmServicePostRiskCheckRequest,
    response_type: FrmServicePostRiskCheckResponse,
    flow_marker: PostRiskCheck,
    resource_common_data_type: FrmFlowData,
    request_data_type: PostRiskCheckRequest,
    response_data_type: PostRiskCheckResponse,
    generate_response_fn: generate_post_risk_check_response,
);
