use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use grpc_api_types::payments::{
    composite_payment_service_server::CompositePaymentService, CompositeAuthorizeRequest,
    CompositeAuthorizeResponse, CompositeCaptureRequest, CompositeCaptureResponse,
    CompositeGetRequest, CompositeGetResponse, CompositePreAuthenticateRequest,
    CompositePreAuthenticateResponse, CompositeRefundRequest, CompositeRefundResponse,
    CompositeVerifyRedirectResponseRequest, CompositeVerifyRedirectResponseResponse,
    CompositeVoidRequest, CompositeVoidResponse,
};
use std::sync::Arc;

use crate::http::handlers::macros::http_handler;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
use ucs_env::configs::Config;

http_handler!(
    authorize,
    CompositeAuthorizeRequest,
    CompositeAuthorizeResponse,
    authorize,
    composite_payments_service
);

http_handler!(
    pre_authenticate,
    CompositePreAuthenticateRequest,
    CompositePreAuthenticateResponse,
    pre_authenticate,
    composite_payments_service
);

http_handler!(
    get,
    CompositeGetRequest,
    CompositeGetResponse,
    get,
    composite_payments_service
);

http_handler!(
    refund,
    CompositeRefundRequest,
    CompositeRefundResponse,
    refund,
    composite_payments_service
);

http_handler!(
    void,
    CompositeVoidRequest,
    CompositeVoidResponse,
    void,
    composite_payments_service
);

http_handler!(
    capture,
    CompositeCaptureRequest,
    CompositeCaptureResponse,
    capture,
    composite_payments_service
);

http_handler!(
    verify_redirect_response,
    CompositeVerifyRedirectResponseRequest,
    CompositeVerifyRedirectResponseResponse,
    verify_redirect_response,
    composite_payments_service
);
