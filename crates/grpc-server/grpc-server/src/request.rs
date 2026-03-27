use std::sync::Arc;

use common_utils::metadata::MaskedMetadata;

use crate::utils::MetadataPayload;
use ucs_env::configs;

/// Structured request data with secure metadata access.
/// This is the gRPC-specific wrapper around `InterfaceRequestData` that
/// provides non-optional extensions for backward compatibility.
#[derive(Debug)]
pub struct RequestData<T> {
    pub payload: T,
    pub extracted_metadata: MetadataPayload,
    pub masked_metadata: MaskedMetadata,
    pub extensions: tonic::Extensions,
}

impl<T> RequestData<T> {
    #[allow(clippy::result_large_err)]
    pub fn from_grpc_request(
        request: tonic::Request<T>,
        config: Arc<configs::Config>,
    ) -> Result<Self, tonic::Status> {
        let interface_data =
            ucs_interface_common::request::InterfaceRequestData::from_grpc_request(
                request, config,
            )?;

        Ok(Self {
            payload: interface_data.payload,
            extracted_metadata: interface_data.extracted_metadata,
            masked_metadata: interface_data.masked_metadata,
            extensions: interface_data
                .extensions
                .ok_or_else(|| tonic::Status::internal("Extensions missing from gRPC request"))?,
        })
    }
}
