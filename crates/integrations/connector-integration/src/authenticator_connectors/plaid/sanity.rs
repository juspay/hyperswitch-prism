use std::collections::HashMap;

use common_utils::SecretSerdeValue;
use grpc_api_types::payments::{ClientPlatform, OsBasedReturnUrl};
use hyperswitch_masking::PeekInterface;

use crate::sanity::{ConnectorSanity, ConnectorSanityRequest};

pub struct PlaidSanity;

const OS_BASED_RETURN_URL_CONFIG_KEYS: &[(&str, &str)] = &[
    ("ios", "ios_redirect_uri"),
    ("android", "android_package_name"),
    ("web", "web_redirect_uri"),
];

pub static PLAID_SANITY: PlaidSanity = PlaidSanity;

impl ConnectorSanity for PlaidSanity {
    fn populate_os_based_return_url(
        &self,
        raw_config: Option<&SecretSerdeValue>,
        req: &mut dyn ConnectorSanityRequest,
    ) {
        let Some(plaid_config) = raw_config else {
            return;
        };

        plaid_os_based_return_url(plaid_config, req);
    }
}

fn plaid_os_based_return_url(
    plaid_config: &SecretSerdeValue,
    req: &mut dyn ConnectorSanityRequest,
) {
    let return_url_map = OS_BASED_RETURN_URL_CONFIG_KEYS
        .iter()
        .filter_map(|(os, config_key)| {
            plaid_config
                .peek()
                .get(*config_key)
                .and_then(serde_json::Value::as_str)
                .map(|value| ((*os).to_string(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();

    if return_url_map.is_empty() {
        return;
    }

    tracing::debug!(
        map_keys = ?return_url_map.keys().collect::<Vec<_>>(),
        "populating os_based_return_url map extracted from raw Plaid config"
    );

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Unspecified),
        return_url_map,
    });
}
