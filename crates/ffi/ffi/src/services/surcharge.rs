use grpc_api_types::surcharge::{
    SurchargeServiceCalculateRequest, SurchargeServiceCalculateResponse,
    SurchargeServiceSurchargeRefundCompleteRequest,
    SurchargeServiceSurchargeRefundCompleteResponse, SurchargeServiceSurchargeRefundInitRequest,
    SurchargeServiceSurchargeRefundInitResponse,
};

use crate::macros::{surcharge_req_transformer, surcharge_res_transformer};

use domain_types::{
    connector_flow::{SurchargeCalculate, SurchargeRefundComplete, SurchargeRefundInit},
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargeRefundCompleteRequest, SurchargeRefundCompleteResponse,
        SurchargeRefundInitRequest, SurchargeRefundInitResponse,
    },
};

// surcharge calculate request transformer
surcharge_req_transformer!(
    fn_name: surcharge_calculate_req_transformer,
    request_type: SurchargeServiceCalculateRequest,
    flow_marker: SurchargeCalculate,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeCalculateRequest,
    response_data_type: SurchargeCalculateResponse,
);

surcharge_req_transformer!(
    fn_name: surcharge_refund_init_req_transformer,
    request_type: SurchargeServiceSurchargeRefundInitRequest,
    flow_marker: SurchargeRefundInit,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeRefundInitRequest,
    response_data_type: SurchargeRefundInitResponse,
);
surcharge_res_transformer!(
    fn_name: surcharge_refund_init_res_transformer,
    request_type: SurchargeServiceSurchargeRefundInitRequest,
    response_type: SurchargeServiceSurchargeRefundInitResponse,
    flow_marker: SurchargeRefundInit,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeRefundInitRequest,
    response_data_type: SurchargeRefundInitResponse,
    generate_response_fn: generate_surcharge_refund_init_response,
);
surcharge_req_transformer!(
    fn_name: surcharge_refund_complete_req_transformer,
    request_type: SurchargeServiceSurchargeRefundCompleteRequest,
    flow_marker: SurchargeRefundComplete,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeRefundCompleteRequest,
    response_data_type: SurchargeRefundCompleteResponse,
);
surcharge_res_transformer!(
    fn_name: surcharge_refund_complete_res_transformer,
    request_type: SurchargeServiceSurchargeRefundCompleteRequest,
    response_type: SurchargeServiceSurchargeRefundCompleteResponse,
    flow_marker: SurchargeRefundComplete,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeRefundCompleteRequest,
    response_data_type: SurchargeRefundCompleteResponse,
    generate_response_fn: generate_surcharge_refund_complete_response,
);

// surcharge calculate response transformer
surcharge_res_transformer!(
    fn_name: surcharge_calculate_res_transformer,
    request_type: SurchargeServiceCalculateRequest,
    response_type: SurchargeServiceCalculateResponse,
    flow_marker: SurchargeCalculate,
    resource_common_data_type: SurchargeFlowData,
    request_data_type: SurchargeCalculateRequest,
    response_data_type: SurchargeCalculateResponse,
    generate_response_fn: generate_surcharge_calculate_response,
);
