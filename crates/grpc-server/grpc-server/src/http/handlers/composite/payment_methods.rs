use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;
use ucs_env::configs::Config;

use crate::http::handlers::macros::http_handler;
use crate::http::state::AppState;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, transfer_config_to_grpc_request,
    utils::ValidatedJson,
};
use grpc_api_types::payments::{
    composite_payment_method_service_server::CompositePaymentMethodService,
    CompositePaymentMethodCreateRequest, CompositePaymentMethodCreateResponse,
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
