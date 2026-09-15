//! Connector-specific request sanity logic.
//!
//! The typed service wrappers are generated in `grpc-api-types` from the
//! protobuf descriptor set. This module only supplies the runtime sanitizer:
//! 1. resolve which connector this request is for (reusing the same
//!    resolution every other flow already relies on), then
//! 2. dispatch to that connector's own sanity step, if one is registered.

#[cfg(feature = "connector-sanity-layer")]
use domain_types::connector_types::{AuthenticatorConnectorEnum, ConnectorVariant};
#[cfg(feature = "connector-sanity-layer")]
use grpc_api_types::auto_populate::{
    PopulateOsBasedReturnUrl, RequestSanitizer, SanityLayer as GeneratedSanityLayer,
};
#[cfg(feature = "connector-sanity-layer")]
use grpc_api_types::payments::OsBasedReturnUrl;
#[cfg(feature = "connector-sanity-layer")]
use std::collections::HashMap;
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
        match sanity_connector(metadata) {
            Some(SanityConnector::Plaid) => PlaidSanity.apply(metadata, req),
            Some(SanityConnector::Other(connector)) => {
                tracing::debug!(
                    connector = %connector.get_connector_name(),
                    connector_variant = ?connector,
                    "no connector sanity registered"
                );
            }
            None => {}
        }
    }
}

#[cfg(feature = "connector-sanity-layer")]
trait ConnectorSanity {
    fn raw_config(&self, metadata: &MetadataMap) -> Option<serde_json::Value>;

    fn apply<T: PopulateOsBasedReturnUrl>(&self, metadata: &MetadataMap, req: &mut T);
}

#[cfg(feature = "connector-sanity-layer")]
struct PlaidSanity;

#[cfg(feature = "connector-sanity-layer")]
impl ConnectorSanity for PlaidSanity {
    fn raw_config(&self, metadata: &MetadataMap) -> Option<serde_json::Value> {
        raw_connector_config(metadata, &["Plaid", "plaid"])
    }

    fn apply<T: PopulateOsBasedReturnUrl>(&self, metadata: &MetadataMap, req: &mut T) {
        let Some(plaid_config) = self.raw_config(metadata) else {
            return;
        };

        if let Some(os_based_return_url) = plaid_os_based_return_url(plaid_config) {
            tracing::debug!(
                map_keys = ?os_based_return_url.return_url_map.keys().collect::<Vec<_>>(),
                "populating os_based_return_url map extracted from raw Plaid config"
            );
            req.populate_os_based_return_url(os_based_return_url);
        }
    }
}

#[cfg(feature = "connector-sanity-layer")]
enum SanityConnector {
    Plaid,
    Other(ConnectorVariant),
}

#[cfg(feature = "connector-sanity-layer")]
fn sanity_connector(metadata: &MetadataMap) -> Option<SanityConnector> {
    let connector = connector_variant_from_metadata(metadata).ok().or_else(|| {
        connector_and_config_from_metadata(metadata)
            .ok()
            .map(|(connector, _config)| connector)
    })?;

    match connector {
        ConnectorVariant::Authenticator(AuthenticatorConnectorEnum::Plaid) => {
            Some(SanityConnector::Plaid)
        }
        _ => Some(SanityConnector::Other(connector)),
    }
}

#[cfg(feature = "connector-sanity-layer")]
fn raw_connector_config(
    metadata: &MetadataMap,
    connector_keys: &[&str],
) -> Option<serde_json::Value> {
    let header_value = metadata
        .get(common_utils::consts::X_CONNECTOR_CONFIG)?
        .to_str()
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(header_value).ok()?;
    let config = json.get("config")?;
    connector_keys
        .iter()
        .find_map(|key| config.get(*key).cloned())
}

#[cfg(feature = "connector-sanity-layer")]
fn plaid_os_based_return_url(plaid_config: serde_json::Value) -> Option<OsBasedReturnUrl> {
    let return_url_map = [
        ("ios_redirect_uri", "ios_redirect_uri"),
        ("android_package_name", "android_package_name"),
        ("web_redirect_uri", "web_redirect_uri"),
    ]
    .into_iter()
    .filter_map(|(os, config_key)| {
        plaid_config
            .get(config_key)
            .and_then(serde_json::Value::as_str)
            .map(|value| (os.to_string(), value.to_string()))
    })
    .collect::<HashMap<_, _>>();

    (!return_url_map.is_empty()).then_some(OsBasedReturnUrl {
        os_type: String::new(),
        return_url_map,
    })
}
