use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use grpc_api_types::payments::{
    event_service_server::EventService, refund_service_server::RefundService,
    EventServiceHandleRequest, EventServiceHandleResponse, RefundResponse, RefundServiceGetRequest,
};
use std::sync::Arc;

use crate::http::handlers::macros::http_handler;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
use ucs_env::configs::Config;

http_handler!(
    get_refund,
    RefundServiceGetRequest,
    RefundResponse,
    get,
    refunds_service
);

http_handler!(
    transform_refund,
    EventServiceHandleRequest,
    EventServiceHandleResponse,
    handle_event,
    event_service
);
