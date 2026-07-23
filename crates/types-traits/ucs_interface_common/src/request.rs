use common_utils::metadata::MaskedMetadata;
use std::sync::Arc;

use crate::metadata::{get_metadata_payload, MetadataPayload};
use ucs_env::configs;

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
    pub fn from_grpc_request(
        request: tonic::Request<T>,
        config: Arc<configs::Config>,
    ) -> common_utils::errors::CustomResult<Self, domain_types::errors::IntegrationError> {
        let (metadata, extensions, payload) = request.into_parts();

        let metadata_payload = get_metadata_payload(&metadata, config.clone())?;

        let masked_metadata = MaskedMetadata::new(metadata, config.unmasked_headers.clone());

        Ok(Self {
            payload,
            extracted_metadata: metadata_payload,
            masked_metadata,
            extensions: Some(extensions),
        })
    }
}
