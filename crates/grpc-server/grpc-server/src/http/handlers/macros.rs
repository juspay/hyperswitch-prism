// All imports are used within the macro expansion
#[allow(unused_imports)]
use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    Json,
};
#[allow(unused_imports)]
use tonic;

#[allow(unused_imports)]
use crate::http::{
    error::HttpError, http_headers_to_grpc_metadata, state::AppState,
    transfer_config_to_grpc_request, utils::ValidatedJson,
};
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use ucs_env::configs::Config;

macro_rules! http_handler {
    ($fn_name:ident, $req_type:ty, $resp_type:ty, $service_method:ident, $service_field:ident) => {
        pub async fn $fn_name(
            Extension(config): Extension<Arc<Config>>,
            State(state): State<AppState>,
            headers: HeaderMap,
            ValidatedJson(payload): ValidatedJson<$req_type>,
        ) -> Result<Json<$resp_type>, HttpError> {
            let mut grpc_request = tonic::Request::new(payload);
            transfer_config_to_grpc_request(&config, &mut grpc_request);
            let grpc_metadata =
                http_headers_to_grpc_metadata(&headers).map_err(|status| HttpError {
                    status: StatusCode::BAD_REQUEST,
                    message: status.message().to_string(),
                })?;
            *grpc_request.metadata_mut() = grpc_metadata;
            let grpc_response = state.$service_field.$service_method(grpc_request).await?;
            Ok(Json(grpc_response.into_inner()))
        }
    };
}

pub(crate) use http_handler;
