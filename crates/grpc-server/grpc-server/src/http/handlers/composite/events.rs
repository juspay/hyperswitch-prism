use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use grpc_api_types::payments::{
    composite_event_service_server::CompositeEventService, CompositeEventHandleRequest,
    CompositeEventHandleResponse,
};
use std::sync::Arc;

use crate::http::handlers::macros::http_handler;
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
use ucs_env::configs::Config;

http_handler!(
    handle_event,
    CompositeEventHandleRequest,
    CompositeEventHandleResponse,
    handle_event,
    composite_event_service
);
