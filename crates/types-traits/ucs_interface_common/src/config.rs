use base64::{engine::general_purpose, Engine as _};
use common_utils::errors::CustomResult;
use domain_types::{
    errors::{ApiError, ApplicationErrorResponse},
    router_data::ConnectorSpecificConfig,
    types::Connectors,
};
use error_stack::Report;
use serde_json::Value;
use std::sync::Arc;
use ucs_env::configs::{self, ConfigPatch};

use common_utils::config_patch::Patch;

pub fn merge_config_with_override(
    config_override: String,
    config: configs::Config,
) -> CustomResult<Arc<configs::Config>, ApplicationErrorResponse> {
    match config_override.trim().is_empty() {
        true => Ok(Arc::new(config)),
        false => {
            let mut override_patch: ConfigPatch = serde_json::from_str(config_override.trim())
                .map_err(|e| {
                    Report::new(ApplicationErrorResponse::BadRequest(ApiError {
                        sub_code: "CANNOT_CONVERT_TO_JSON".into(),
                        error_identifier: 400,
                        error_message: format!("Cannot convert override config to JSON: {e}"),
                        error_object: None,
                    }))
                })?;

            if let Some(proxy_patch) = override_patch.proxy.as_mut() {
                if let Some(cert_input) = proxy_patch
                    .mitm_ca_cert
                    .as_ref()
                    .and_then(|value| value.as_ref())
                {
                    let cert_trimmed = cert_input.trim();

                    let cert = if cert_trimmed.is_empty() {
                        Err(Report::new(ApplicationErrorResponse::BadRequest(
                            ApiError {
                                sub_code: "INVALID_MITM_CA_CERT_BASE64".into(),
                                error_identifier: 400,
                                error_message: "proxy.mitm_ca_cert must be base64-encoded"
                                    .to_string(),
                                error_object: None,
                            },
                        )))
                    } else {
                        let sanitized: String = cert_trimmed.split_whitespace().collect();
                        let decoded = general_purpose::STANDARD
                            .decode(sanitized.as_bytes())
                            .map_err(|e| {
                                Report::new(ApplicationErrorResponse::BadRequest(ApiError {
                                    sub_code: "INVALID_MITM_CA_CERT_BASE64".into(),
                                    error_identifier: 400,
                                    error_message: format!(
                                        "Invalid base64 for proxy.mitm_ca_cert: {e}"
                                    ),
                                    error_object: None,
                                }))
                            })?;

                        String::from_utf8(decoded).map_err(|e| {
                            Report::new(ApplicationErrorResponse::BadRequest(ApiError {
                                sub_code: "INVALID_MITM_CA_CERT_UTF8".into(),
                                error_identifier: 400,
                                error_message: format!(
                                    "Decoded proxy.mitm_ca_cert is not valid UTF-8: {e}"
                                ),
                                error_object: None,
                            }))
                        })
                    }?;

                    proxy_patch.mitm_ca_cert = Some(Some(cert));
                }
            }

            let mut merged_config = config;
            merged_config.apply(override_patch);

            tracing::info!("Config override applied successfully");

            Ok(Arc::new(merged_config))
        }
    }
}

pub fn connectors_with_connector_config_overrides(
    connector_config: &ConnectorSpecificConfig,
    base_config: &configs::Config,
) -> CustomResult<Connectors, ApplicationErrorResponse> {
    match connector_config.connector_config_override_patch() {
        Some(config_override) => {
            merge_config_with_override(config_override.to_string(), base_config.clone())
                .map(|config| config.connectors.clone())
        }
        None => Ok(base_config.connectors.clone()),
    }
}

/// Apply connector-specific config overrides on top of an existing Connectors struct.
/// This is useful when you want to apply overrides on a Connectors that has already been
/// modified (e.g., with superposition URL resolution).
pub fn connectors_with_connector_config_overrides_on_connectors(
    connector_config: &ConnectorSpecificConfig,
    base_connectors: Connectors,
) -> CustomResult<Connectors, ApplicationErrorResponse> {
    match connector_config.connector_config_override_patch() {
        Some(config_override) => {
            // Parse the override patch from JSON Value
            let override_patch: ConfigPatch = serde_json::from_value(config_override.clone())
                .map_err(|e| {
                    Report::new(ApplicationErrorResponse::BadRequest(ApiError {
                        sub_code: "CANNOT_CONVERT_TO_JSON".into(),
                        error_identifier: 400,
                        error_message: format!("Cannot convert override config to JSON: {e}"),
                        error_object: None,
                    }))
                })?;

            // If there's a connectors patch, apply it
            if let Some(connectors_patch) = override_patch.connectors {
                let mut merged_connectors = base_connectors;
                merged_connectors.apply(connectors_patch);
                Ok(merged_connectors)
            } else {
                Ok(base_connectors)
            }
        }
        None => Ok(base_connectors),
    }
}

pub fn merge_configs(override_val: &Value, base_val: &Value) -> Value {
    match (base_val, override_val) {
        (Value::Object(base_map), Value::Object(override_map)) => {
            let mut merged = base_map.clone();
            for (key, override_value) in override_map {
                let base_value = base_map.get(key).unwrap_or(&Value::Null);
                merged.insert(key.clone(), merge_configs(override_value, base_value));
            }
            Value::Object(merged)
        }
        // override replaces base for primitive, null, or array
        (_, override_val) => override_val.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::connectors_with_connector_config_overrides;
    use hyperswitch_masking::Secret;

    #[test]
    fn connector_config_overrides_patch_effective_runtime_urls() {
        let base_config = ucs_env::configs::Config::new().expect("default config should load");
        let connector_config = domain_types::router_data::ConnectorSpecificConfig::Adyen {
            api_key: Secret::new("api_key".to_string()),
            merchant_account: Secret::new("merchant_account".to_string()),
            review_key: None,
            base_url: Some("https://override.adyen.example".to_string()),
            dispute_base_url: Some("https://override-dispute.adyen.example".to_string()),
            endpoint_prefix: None,
        };

        let connectors =
            connectors_with_connector_config_overrides(&connector_config, &base_config)
                .expect("connector override should merge into effective config");

        assert_eq!(
            connectors.adyen.base_url.as_str(),
            "https://override.adyen.example"
        );
        assert_eq!(
            connectors.adyen.dispute_base_url.as_deref(),
            Some("https://override-dispute.adyen.example")
        );
    }
}
