use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;
use ucs_env::configs::Config;

use crate::http::handlers::macros::http_handler;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
use grpc_api_types::frm::{
    composite_fraud_and_risk_management_service_server::CompositeFraudAndRiskManagementService,
    CompositeFrmDeviceDataCollectionRequest, CompositeFrmDeviceDataCollectionResponse,
    CompositeFrmPostRiskCheckRequest, CompositeFrmPostRiskCheckResponse,
    CompositeFrmPreRiskCheckRequest, CompositeFrmPreRiskCheckResponse,
};

http_handler!(
    device_data_collection,
    CompositeFrmDeviceDataCollectionRequest,
    CompositeFrmDeviceDataCollectionResponse,
    device_data_collection,
    composite_frm_service
);

http_handler!(
    pre_risk_check,
    CompositeFrmPreRiskCheckRequest,
    CompositeFrmPreRiskCheckResponse,
    pre_risk_check,
    composite_frm_service
);

http_handler!(
    post_risk_check,
    CompositeFrmPostRiskCheckRequest,
    CompositeFrmPostRiskCheckResponse,
    post_risk_check,
    composite_frm_service
);
