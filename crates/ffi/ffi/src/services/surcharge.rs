use grpc_api_types::surcharge::{
    SurchargeServiceCalculateRequest, SurchargeServiceCalculateResponse,
};

use crate::macros::{surcharge_req_transformer, surcharge_res_transformer};

use domain_types::{
    connector_flow::SurchargeCalculate,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
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
