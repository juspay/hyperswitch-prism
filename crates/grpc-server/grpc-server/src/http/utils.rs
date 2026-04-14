use axum::{
    extract::{FromRequest, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use std::sync::Arc;

use super::error::HttpError;
use ucs_env::configs::Config;

/// Converts HTTP headers to gRPC metadata.
/// Delegates to the shared `headers_to_metadata` implementation.
pub fn http_headers_to_grpc_metadata(
    http_headers: &HeaderMap,
) -> Result<tonic::metadata::MetadataMap, Box<tonic::Status>> {
    ucs_interface_common::headers::headers_to_metadata(http_headers)
        .map_err(|e| Box::new(tonic::Status::from(e)))
}

/// Transfers config from Axum Extension to gRPC request
/// Copies the Arc<Config> from Axum Extension to gRPC request extensions
pub fn transfer_config_to_grpc_request<T>(
    config: &Arc<Config>,
    grpc_request: &mut tonic::Request<T>,
) {
    grpc_request.extensions_mut().insert(config.clone());
}

/// Custom JSON extractor that converts 422 errors to 400 with original error messages
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(HttpError {
                status: StatusCode::BAD_REQUEST,
                message: rejection.to_string(),
            }
            .into_response()),
        }
    }
}
