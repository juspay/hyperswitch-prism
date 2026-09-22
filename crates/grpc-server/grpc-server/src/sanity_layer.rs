//! Connector-specific request sanity logic.
//!
//! The typed service wrappers are generated in `grpc-api-types` from the
//! protobuf descriptor set. This module only supplies the runtime sanitizer:
//! 1. resolve which connector this request is for (reusing the same
//!    resolution every other flow already relies on), then
//! 2. dispatch to that connector's own sanity step, if one is registered.

#[cfg(feature = "connector-sanity-layer")]
use common_utils::SecretSerdeValue;
#[cfg(feature = "connector-sanity-layer")]
use connector_integration::sanity::sanity_for;
#[cfg(feature = "connector-sanity-layer")]
use domain_types::connector_types::ConnectorVariant;
#[cfg(feature = "connector-sanity-layer")]
use grpc_api_types::auto_populate::{
    PopulateOsBasedReturnUrl, RequestSanitizer, SanityLayer as GeneratedSanityLayer,
};
#[cfg(feature = "connector-sanity-layer")]
use hyperswitch_masking::PeekInterface;
#[cfg(feature = "connector-sanity-layer")]
use tonic::metadata::MetadataMap;
#[cfg(feature = "connector-sanity-layer")]
use ucs_interface_common::auth::connector_and_config_from_metadata;
#[cfg(feature = "connector-sanity-layer")]
use ucs_interface_common::metadata::connector_variant_from_metadata;

#[cfg(feature = "connector-sanity-layer")]
#[derive(Clone, Copy, Debug, Default)]
pub struct ConnectorSanitizer;

#[cfg(feature = "connector-sanity-layer")]
pub type SanityLayer<S> = GeneratedSanityLayer<S, ConnectorSanitizer>;

#[cfg(not(feature = "connector-sanity-layer"))]
pub type SanityLayer<S> = S;

#[cfg(feature = "connector-sanity-layer")]
pub fn wrap<S>(inner: S) -> SanityLayer<S> {
    GeneratedSanityLayer::new(inner, ConnectorSanitizer)
}

#[cfg(not(feature = "connector-sanity-layer"))]
pub fn wrap<S>(inner: S) -> SanityLayer<S> {
    inner
}

#[cfg(feature = "connector-sanity-layer")]
impl RequestSanitizer for ConnectorSanitizer {
    fn sanitize<T: PopulateOsBasedReturnUrl>(&self, metadata: &MetadataMap, req: &mut T) {
        let Some(connector) = connector_from_metadata(metadata) else {
            return;
        };
        match sanity_for(&connector) {
            Some(sanitizer) => {
                let raw_config = raw_connector_config(metadata, &connector.get_connector_name());
                sanitizer.apply(raw_config, req);
            }
            None => tracing::debug!(
                connector = %connector.get_connector_name(),
                "no connector sanity registered"
            ),
        }
    }
}

/// Primary resolution matches the real request pipeline exactly
/// (`x-connector-config` first, legacy headers as its own fallback — see
/// `ucs_interface_common::metadata::get_metadata_payload`), so the sanity
/// layer can never disagree with the handler about which connector a
/// *processable* request is for.
///
/// The bare `connector_variant_from_metadata` fallback only fires when that
/// primary resolution fails outright (e.g. a legacy connector-name header
/// present without enough legacy auth headers to build a full config) — at
/// which point the real handler would reject the request anyway, so there is
/// no live disagreement risk: neither path leads to the request actually
/// being processed as some connector. It only restores best-effort dispatch
/// for connectors that do have something to do before that rejection.
#[cfg(feature = "connector-sanity-layer")]
fn connector_from_metadata(metadata: &MetadataMap) -> Option<ConnectorVariant> {
    connector_and_config_from_metadata(metadata)
        .ok()
        .map(|(connector, _config)| connector)
        .or_else(|| connector_variant_from_metadata(metadata).ok())
}

#[cfg(feature = "connector-sanity-layer")]
pub(super) fn raw_connector_config(
    metadata: &MetadataMap,
    connector_name: &str,
) -> Option<SecretSerdeValue> {
    let header_value = metadata
        .get(common_utils::consts::X_CONNECTOR_CONFIG)?
        .to_str()
        .ok()?;
    let json = SecretSerdeValue::new(serde_json::from_str(header_value).ok()?);
    let config = json.peek().get("config")?.as_object()?;
    config
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(connector_name))
        .map(|(_, value)| value.clone())
        .map(SecretSerdeValue::new)
}
