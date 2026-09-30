use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;
use ucs_env::configs::Config;

use crate::http::{
    error::HttpError, handlers::macros::http_handler, http_headers_to_grpc_metadata,
    state::AppState, transfer_config_to_grpc_request, utils::ValidatedJson,
};
use grpc_api_types::payments::{
    composite_payment_method_service_server::CompositePaymentMethodService,
    CompositePaymentMethodCreateRequest, CompositePaymentMethodCreateResponse,
    CompositePaymentMethodEligibilityRequest, CompositePaymentMethodEligibilityResponse,
    CompositePaymentMethodGetRequest, CompositePaymentMethodGetResponse,
    CompositePaymentMethodRechargeRequest, CompositePaymentMethodRechargeResponse,
};

http_handler!(
    create,
    CompositePaymentMethodCreateRequest,
    CompositePaymentMethodCreateResponse,
    create,
    composite_payment_method_service
);

http_handler!(
    get,
    CompositePaymentMethodGetRequest,
    CompositePaymentMethodGetResponse,
    get,
    composite_payment_method_service
);

http_handler!(
    recharge,
    CompositePaymentMethodRechargeRequest,
    CompositePaymentMethodRechargeResponse,
    recharge,
    composite_payment_method_service
);

http_handler!(
    eligibility,
    CompositePaymentMethodEligibilityRequest,
    CompositePaymentMethodEligibilityResponse,
    eligibility,
    composite_payment_method_service
);
