use std::sync::Arc;

use common_utils::{consts, metadata::MaskedMetadata, request_metrics::ConnectorLatencyTracker};
use domain_types::{errors::IntegrationError, router_data::ConnectorSpecificConfig};
use error_stack::Report;
use tonic::metadata;
use ucs_env::{
    configs,
    error::{GrpcError, InternalError, ResultExtGrpcError},
};

use crate::utils::{
    connector_and_config_from_metadata, connector_variant_from_metadata, MetadataPayload,
};
use ucs_interface_common::metadata::{
    merchant_id_from_metadata, proxy_name_from_metadata, request_id_from_metadata,
    tenant_id_from_metadata,
};

/// Deprecated header carrying the typed connector config (kept for backward compatibility).
const X_CONNECTOR_AUTH_DEPRECATED: &str = "x-connector-auth";

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
    pub fn from_grpc_request(
        request: tonic::Request<T>,
        config: Arc<configs::Config>,
    ) -> Result<Self, Report<GrpcError>> {
        let interface_data =
            ucs_interface_common::request::InterfaceRequestData::from_grpc_request(request, config)
                .to_grpc_error()?;

        Ok(Self {
            payload: interface_data.payload,
            extracted_metadata: interface_data.extracted_metadata,
            masked_metadata: interface_data.masked_metadata,
            extensions: interface_data.extensions.ok_or_else(|| {
                Report::new(GrpcError::from(InternalError::MissingRequestExtensions))
            })?,
        })
    }

    /// Parse request for webhook flows that only need routing metadata.
    /// This does not require connector authentication credentials.
    pub fn from_grpc_request_unauthenticated(
        request: tonic::Request<T>,
        config: Arc<configs::Config>,
    ) -> Result<Self, Report<GrpcError>> {
        let (metadata, extensions, payload) = request.into_parts();

        // Extract routing metadata only (connector, request_id, etc.)
        // without requiring connector_config/auth credentials
        let routing_metadata =
            extract_routing_metadata_only(&metadata, config.clone()).to_grpc_error()?;

        let masked_metadata = MaskedMetadata::new(metadata, config.unmasked_headers.clone());

        Ok(Self {
            payload,
            extracted_metadata: routing_metadata,
            masked_metadata,
            extensions,
        })
    }
}

/// Extract only routing metadata without requiring authentication credentials.
/// Used for webhook flows where connector_config is not needed for initial processing.
fn extract_routing_metadata_only(
    metadata: &metadata::MetadataMap,
    _config: Arc<configs::Config>,
) -> Result<MetadataPayload, Report<IntegrationError>> {
    // Extract other routing fields - use defaults where appropriate
    let merchant_id = merchant_id_from_metadata(metadata).unwrap_or_default();
    let tenant_id = tenant_id_from_metadata(metadata).unwrap_or_default();
    let request_id = request_id_from_metadata(metadata).unwrap_or_default();

    // For webhooks, connector credentials are optional. Resolve the config from the
    // typed `x-connector-config` header, falling back to the deprecated
    // `x-connector-auth` header. When neither is present, use NoKey so the webhook can
    // still be routed and processed. The connector name header is always required.
    let (connector, connector_config) = match metadata
        .get(consts::X_CONNECTOR_CONFIG)
        .or_else(|| metadata.get(X_CONNECTOR_AUTH_DEPRECATED))
    {
        Some(_) => connector_and_config_from_metadata(metadata)?,
        None => (
            connector_variant_from_metadata(metadata)?,
            ConnectorSpecificConfig::NoKey,
        ),
    };

    // Extract optional fields
    let reference_id = metadata
        .get(consts::X_REFERENCE_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let resource_id = metadata
        .get(consts::X_RESOURCE_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let shadow_mode = metadata
        .get(consts::X_SHADOW_MODE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false);

    let environment = metadata
        .get(consts::X_ENVIRONMENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let proxy_name = proxy_name_from_metadata(metadata);

    Ok(MetadataPayload {
        tenant_id,
        request_id,
        merchant_id,
        connector,
        lineage_ids: common_utils::lineage::LineageIds::empty(""),
        connector_config,
        reference_id,
        shadow_mode,
        resource_id,
        environment,
        proxy_name,
        connector_latency: ConnectorLatencyTracker::default(),
    })
}
