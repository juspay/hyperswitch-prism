//! Connector-specific request sanity logic.
//!
//! The typed service wrappers are generated in `grpc-api-types` from the
//! protobuf descriptor set. This module only supplies the runtime sanitizer:
//! 1. resolve which connector this request is for (reusing the same
//!    resolution every other flow already relies on), then
//! 2. dispatch to that connector's own sanity step, if one is registered.

#[cfg(feature = "connector-sanity-layer")]
use grpc_api_types::auto_populate::{
    PopulateOsBasedReturnUrl, RequestSanitizer, SanityLayer as GeneratedSanityLayer,
};
#[cfg(feature = "connector-sanity-layer")]
use grpc_api_types::payments::OsBasedReturnUrl;
#[cfg(feature = "connector-sanity-layer")]
use std::collections::HashMap;
#[cfg(feature = "connector-sanity-layer")]
use tonic::metadata::{MetadataMap, MetadataValue};

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
    fn sanitize<T: PopulateOsBasedReturnUrl>(&self, metadata: &mut MetadataMap, req: &mut T) {
        if is_plaid_request(metadata) {
            plaid_sanity(metadata, req);
        }
    }
}

/// Plaid's dashboard config (sent via `x-connector-config`) may carry
/// Euler-only redirect keys that UCS's typed `PlaidConfig` proto message
/// doesn't declare. Read those keys from the raw header JSON, populate the
/// request field, then strip them before downstream typed config parsing.
#[cfg(feature = "connector-sanity-layer")]
fn plaid_sanity<T: PopulateOsBasedReturnUrl>(metadata: &mut MetadataMap, req: &mut T) {
    if let Some(os_based_return_url) = plaid_os_based_return_url(metadata) {
        tracing::debug!(
            map_keys = ?os_based_return_url.return_url_map.keys().collect::<Vec<_>>(),
            "populating os_based_return_url map extracted from raw Plaid config"
        );
        req.populate_os_based_return_url(os_based_return_url);
    }

    clean_plaid_config_header(metadata);
}

#[cfg(feature = "connector-sanity-layer")]
fn is_plaid_request(metadata: &MetadataMap) -> bool {
    metadata
        .get(common_utils::consts::X_AUTHENTICATOR_CONNECTOR_NAME)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("plaid"))
        || raw_plaid_config(metadata).is_some()
}

#[cfg(feature = "connector-sanity-layer")]
fn raw_plaid_config(metadata: &MetadataMap) -> Option<serde_json::Value> {
    let header_value = metadata
        .get(common_utils::consts::X_CONNECTOR_CONFIG)?
        .to_str()
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(header_value).ok()?;
    let config = json.get("config")?;
    config.get("Plaid").or_else(|| config.get("plaid")).cloned()
}

#[cfg(feature = "connector-sanity-layer")]
fn clean_plaid_config_header(metadata: &mut MetadataMap) {
    let Some(header_str) = metadata
        .get(common_utils::consts::X_CONNECTOR_CONFIG)
        .and_then(|value| value.to_str().ok())
    else {
        return;
    };

    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(header_str) else {
        return;
    };

    let Some(config) = json
        .get_mut("config")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };

    let Some(mut plaid_config) = config.remove("Plaid").or_else(|| config.remove("plaid")) else {
        return;
    };

    if let Some(plaid_config) = plaid_config.as_object_mut() {
        plaid_config.remove("ios_redirect_uri");
        plaid_config.remove("android_package_name");
        plaid_config.remove("web_redirect_uri");
    }

    config.insert("Plaid".to_string(), plaid_config);

    let Ok(header_value) = serde_json::to_string(&json) else {
        return;
    };
    let Ok(header_value) = MetadataValue::try_from(header_value.as_str()) else {
        return;
    };

    metadata.insert(common_utils::consts::X_CONNECTOR_CONFIG, header_value);
}

#[cfg(feature = "connector-sanity-layer")]
fn plaid_os_based_return_url(metadata: &MetadataMap) -> Option<OsBasedReturnUrl> {
    let plaid_config = raw_plaid_config(metadata)?;
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
