use common_utils::{
    consts::{self, X_ENVIRONMENT, X_SHADOW_MODE},
    errors::CustomResult,
    fp_utils,
    lineage::LineageIds,
};
use domain_types::{
    connector_types,
    errors::{IntegrationError, IntegrationErrorContext},
    router_data::ConnectorSpecificConfig,
};
use error_stack::Report;
use std::{str::FromStr, sync::Arc};
use tonic::metadata;
use ucs_env::configs;

use crate::auth::connector_and_config_from_metadata;

/// Struct to hold extracted metadata payload.
///
/// SECURITY WARNING: This struct should only contain non-sensitive business metadata.
/// For any sensitive data (API keys, tokens, credentials, etc.), always:
/// 1. Wrap in hyperswitch_masking::Secret<T>
/// 2. Extract via MaskedMetadata methods instead of adding here
#[derive(Clone, Debug)]
pub struct MetadataPayload {
    pub tenant_id: String,
    pub request_id: String,
    pub merchant_id: String,
    pub connector: connector_types::ConnectorEnum,
    pub lineage_ids: LineageIds<'static>,
    /// Typed connector integration config extracted from request metadata.
    ///
    /// Any URL overrides here are only inputs to config patching; connectors should read effective
    /// URLs from the merged runtime config instead.
    pub connector_config: ConnectorSpecificConfig,
    pub reference_id: Option<String>,
    pub shadow_mode: bool,
    pub resource_id: Option<String>,
    /// Environment dimension for superposition config resolution (e.g., "production", "sandbox")
    pub environment: Option<String>,
}

pub fn get_metadata_payload(
    metadata: &metadata::MetadataMap,
    server_config: Arc<configs::Config>,
) -> CustomResult<MetadataPayload, IntegrationError> {
    // Resolve connector and config: try x-connector-config header first,
    // then fall back to legacy x-connector and x-auth headers.
    let (connector, connector_config) = connector_and_config_from_metadata(metadata)?;

    let merchant_id = merchant_id_from_metadata(metadata)?;
    let tenant_id = tenant_id_from_metadata(metadata)?;
    let request_id = request_id_from_metadata(metadata)?;
    let lineage_ids = extract_lineage_fields_from_metadata(metadata, &server_config.lineage);
    let reference_id = reference_id_from_metadata(metadata)?;
    let resource_id = resource_id_from_metadata(metadata)?;
    let shadow_mode = shadow_mode_from_metadata(metadata);
    let environment = environment_from_metadata(metadata);

    Ok(MetadataPayload {
        tenant_id,
        request_id,
        merchant_id,
        connector,
        lineage_ids,
        connector_config,
        reference_id,
        shadow_mode,
        resource_id,
        environment,
    })
}

/// Extract lineage fields from header
pub fn extract_lineage_fields_from_metadata(
    metadata: &metadata::MetadataMap,
    config: &configs::LineageConfig,
) -> LineageIds<'static> {
    if !config.enabled {
        return LineageIds::empty(&config.field_prefix).to_owned();
    }
    metadata
        .get(&config.header_name)
        .and_then(|value| value.to_str().ok())
        .map(|header_value| LineageIds::new(&config.field_prefix, header_value))
        .transpose()
        .inspect(|value| {
            tracing::info!(
                parsed_fields = ?value,
                "Successfully parsed lineage header"
            )
        })
        .inspect_err(|err| {
            tracing::warn!(
                error = %err,
                "Failed to parse lineage header, continuing without lineage fields"
            )
        })
        .ok()
        .flatten()
        .unwrap_or_else(|| LineageIds::empty(&config.field_prefix))
        .to_owned()
}

pub fn connector_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<connector_types::ConnectorEnum, IntegrationError> {
    parse_metadata(metadata, consts::X_CONNECTOR_NAME).and_then(|inner| {
        connector_types::ConnectorEnum::from_str(inner).map_err(|e| {
            Report::new(IntegrationError::InvalidDataFormat {
                field_name: "x-connector",
                context: IntegrationErrorContext {
                    additional_context: Some(format!("Invalid connector: {e}")),
                    ..Default::default()
                },
            })
        })
    })
}

pub fn merchant_id_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<String, IntegrationError> {
    Ok(common_utils::metadata::merchant_id_or_default(
        metadata
            .get(consts::X_MERCHANT_ID)
            .and_then(|value| value.to_str().ok()),
    ))
}

pub fn request_id_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<String, IntegrationError> {
    parse_metadata(metadata, consts::X_REQUEST_ID)
        .map(|inner| inner.to_string())
        .or_else(|_| {
            let generated_id = fp_utils::generate_uuid_v7();
            tracing::debug!(
                request_id = %generated_id,
                "x-request-id header missing, auto-generated request ID"
            );
            Ok(generated_id)
        })
}

pub fn tenant_id_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<String, IntegrationError> {
    parse_metadata(metadata, consts::X_TENANT_ID)
        .map(|s| s.to_string())
        .or_else(|_| Ok("public".to_string()))
}

pub fn reference_id_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<Option<String>, IntegrationError> {
    parse_optional_metadata(metadata, consts::X_REFERENCE_ID).map(|s| s.map(|s| s.to_string()))
}

pub fn resource_id_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<Option<String>, IntegrationError> {
    parse_optional_metadata(metadata, consts::X_RESOURCE_ID).map(|s| s.map(|s| s.to_string()))
}

pub fn shadow_mode_from_metadata(metadata: &metadata::MetadataMap) -> bool {
    parse_optional_metadata(metadata, X_SHADOW_MODE)
        .ok()
        .flatten()
        .map(|value| value.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Extracts environment from the x-environment header for superposition config resolution.
pub fn environment_from_metadata(metadata: &metadata::MetadataMap) -> Option<String> {
    parse_optional_metadata(metadata, X_ENVIRONMENT)
        .ok()
        .flatten()
        .map(|s| s.to_string())
}

pub fn parse_metadata<'a>(
    metadata: &'a metadata::MetadataMap,
    key: &'static str,
) -> CustomResult<&'a str, IntegrationError> {
    metadata
        .get(key)
        .ok_or_else(|| {
            Report::new(IntegrationError::MissingRequiredField {
                field_name: key,
                context: IntegrationErrorContext::default(),
            })
        })
        .and_then(|value| {
            value.to_str().map_err(|e| {
                Report::new(IntegrationError::InvalidDataFormat {
                    field_name: key,
                    context: IntegrationErrorContext {
                        additional_context: Some(format!("Invalid {key} in request metadata: {e}")),
                        ..Default::default()
                    },
                })
            })
        })
}

pub fn parse_optional_metadata<'a>(
    metadata: &'a metadata::MetadataMap,
    key: &'static str,
) -> CustomResult<Option<&'a str>, IntegrationError> {
    metadata
        .get(key)
        .map(|value| value.to_str())
        .transpose()
        .map_err(|e| {
            Report::new(IntegrationError::InvalidDataFormat {
                field_name: key,
                context: IntegrationErrorContext {
                    additional_context: Some(format!("Invalid {key} in request metadata: {e}")),
                    ..Default::default()
                },
            })
        })
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataMap;

    #[test]
    fn merchant_id_defaults_when_missing() {
        let metadata = MetadataMap::new();
        let merchant_id = merchant_id_from_metadata(&metadata).expect("should not fail");
        assert_eq!(merchant_id, "DefaultMerchantId");
    }

    #[test]
    fn merchant_id_resolves_when_present() {
        let mut metadata = MetadataMap::new();
        metadata.insert(consts::X_MERCHANT_ID, "test-merchant".parse().unwrap());
        let merchant_id = merchant_id_from_metadata(&metadata).expect("should resolve");
        assert_eq!(merchant_id, "test-merchant");
    }

    #[test]
    fn request_id_generates_uuid_v7_when_missing() {
        let metadata = MetadataMap::new();
        let request_id = request_id_from_metadata(&metadata).expect("should not fail");
        // UUID v7 is 36 characters long
        assert_eq!(request_id.len(), 36);
        // Simple check for UUID format (hyphens at specific positions)
        assert_eq!(request_id.chars().nth(8), Some('-'));
        assert_eq!(request_id.chars().nth(13), Some('-'));
    }

    #[test]
    fn request_id_resolves_when_present() {
        let mut metadata = MetadataMap::new();
        metadata.insert(consts::X_REQUEST_ID, "specific-request-id".parse().unwrap());
        let request_id = request_id_from_metadata(&metadata).expect("should resolve");
        assert_eq!(request_id, "specific-request-id");
    }

    #[test]
    fn tenant_id_defaults_when_missing() {
        let metadata = MetadataMap::new();
        let tenant_id = tenant_id_from_metadata(&metadata).expect("should not fail");
        assert_eq!(tenant_id, "public");
    }

    #[test]
    fn connector_resolves_from_metadata() {
        let mut metadata = MetadataMap::new();
        metadata.insert(consts::X_CONNECTOR_NAME, "stripe".parse().unwrap());
        let connector = connector_from_metadata(&metadata).expect("should resolve");
        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
    }
}
