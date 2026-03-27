use common_utils::metadata::MaskedMetadata;
use std::sync::Arc;

use crate::metadata::{get_metadata_payload, MetadataPayload};
use ucs_env::{configs, error::ResultExtGrpc};

/// Structured request data with secure metadata access.
/// Used by both gRPC and FFI interfaces.
#[derive(Debug)]
pub struct InterfaceRequestData<T> {
    pub payload: T,
    pub extracted_metadata: MetadataPayload,
    pub masked_metadata: MaskedMetadata,
    /// gRPC extensions (present for gRPC/HTTP, absent for FFI).
    pub extensions: Option<tonic::Extensions>,
}

impl<T> InterfaceRequestData<T> {
    /// Construct from a gRPC request, extracting metadata and masking config.
    #[allow(clippy::result_large_err)]
    pub fn from_grpc_request(
        request: tonic::Request<T>,
        config: Arc<configs::Config>,
    ) -> Result<Self, tonic::Status> {
        let (metadata, extensions, payload) = request.into_parts();

        let metadata_payload =
            get_metadata_payload(&metadata, config.clone()).into_grpc_status()?;

        let masked_metadata = MaskedMetadata::new(metadata, config.unmasked_headers.clone());

        Ok(Self {
            payload,
            extracted_metadata: metadata_payload,
            masked_metadata,
            extensions: Some(extensions),
        })
    }
}
