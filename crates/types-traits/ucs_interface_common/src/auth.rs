use common_utils::{
    consts::{X_API_KEY, X_API_SECRET, X_AUTH, X_AUTH_KEY_MAP, X_CONNECTOR_CONFIG, X_KEY1, X_KEY2},
    errors::CustomResult,
};
use domain_types::{
    connector_types,
    errors::{ApiError, ApplicationErrorResponse},
    router_data::{ConnectorAuthType, ConnectorSpecificConfig},
    utils::ForeignTryFrom,
};
use error_stack::{Report, ResultExt};
use std::collections::HashMap;
use tonic::metadata;
use ucs_env::logger;

use crate::metadata::{connector_from_metadata, parse_metadata};

fn parse_connector_config_from_typed_header(
    header_value: &metadata::MetadataValue<metadata::Ascii>,
) -> CustomResult<(connector_types::ConnectorEnum, ConnectorSpecificConfig), ApplicationErrorResponse>
{
    let typed_config: grpc_api_types::payments::ConnectorSpecificConfig = header_value
        .to_str()
        .change_context(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "INVALID_CONNECTOR_CONFIG_HEADER".to_string(),
            error_identifier: 400,
            error_message: "X-Connector-Config header contains non-ASCII characters".to_string(),
            error_object: None,
        }))
        .and_then(|header_str| {
            serde_json::from_str(header_str).change_context(ApplicationErrorResponse::BadRequest(
                ApiError {
                    sub_code: "INVALID_CONNECTOR_CONFIG_JSON".to_string(),
                    error_identifier: 400,
                    error_message:
                        "Failed to parse X-Connector-Config JSON into ConnectorSpecificConfig"
                            .to_string(),
                    error_object: None,
                },
            ))
        })?;

    let config = typed_config.config.as_ref().ok_or_else(|| {
        Report::new(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "INVALID_CONNECTOR_CONFIG_FORMAT".to_string(),
            error_identifier: 400,
            error_message: "X-Connector-Config header missing config".to_string(),
            error_object: None,
        }))
    })?;

    let connector = connector_types::ConnectorEnum::foreign_try_from(config.clone())?;

    let connector_config = ConnectorSpecificConfig::foreign_try_from(typed_config).change_context(
        ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "AUTH_CONVERSION_FAILED".to_string(),
            error_identifier: 400,
            error_message: "Failed to convert connector config from X-Connector-Config header"
                .to_string(),
            error_object: None,
        }),
    )?;

    logger::debug!(
        "Connector config successfully resolved from X-Connector-Config header for connector: {}",
        connector
    );

    Ok((connector, connector_config))
}

/// Parses the deprecated `x-connector-auth` header.
///
/// The old format used `{"auth_type":{"Stripe":{...}}}` while the new format uses
/// `{"config":{"Stripe":{...}}}`. This rewrites the JSON key before parsing.
fn parse_connector_config_from_deprecated_header(
    header_value: &metadata::MetadataValue<metadata::Ascii>,
) -> CustomResult<(connector_types::ConnectorEnum, ConnectorSpecificConfig), ApplicationErrorResponse>
{
    let header_str = header_value
        .to_str()
        .change_context(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "INVALID_CONNECTOR_CONFIG_HEADER".to_string(),
            error_identifier: 400,
            error_message: "x-connector-auth header contains non-ASCII characters".to_string(),
            error_object: None,
        }))?;

    // Rewrite old field name to new: "auth_type" → "config"
    let rewritten = header_str.replace("\"auth_type\"", "\"config\"");

    let typed_config: grpc_api_types::payments::ConnectorSpecificConfig = serde_json::from_str(
        &rewritten,
    )
    .change_context(ApplicationErrorResponse::BadRequest(ApiError {
        sub_code: "INVALID_CONNECTOR_CONFIG_JSON".to_string(),
        error_identifier: 400,
        error_message: "Failed to parse x-connector-auth JSON into ConnectorSpecificConfig. \
                     Migrate to x-connector-config with {\"config\":{...}} format."
            .to_string(),
        error_object: None,
    }))?;

    let config = typed_config.config.as_ref().ok_or_else(|| {
        Report::new(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "INVALID_CONNECTOR_CONFIG_FORMAT".to_string(),
            error_identifier: 400,
            error_message: "x-connector-auth header missing config variant".to_string(),
            error_object: None,
        }))
    })?;

    let connector = connector_types::ConnectorEnum::foreign_try_from(config.clone())?;

    let connector_config = ConnectorSpecificConfig::foreign_try_from(typed_config).change_context(
        ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "AUTH_CONVERSION_FAILED".to_string(),
            error_identifier: 400,
            error_message: "Failed to convert config from deprecated x-connector-auth header"
                .to_string(),
            error_object: None,
        }),
    )?;

    Ok((connector, connector_config))
}

/// Resolves connector and config from the typed `X-Connector-Config` header first,
/// parsing both the connector enum and the specific config in one go. If it is
/// not present, it falls back to legacy `x-connector` and `x-auth` (+ keys) headers.
/// Deprecated header name kept for backward compatibility.
const X_CONNECTOR_AUTH_DEPRECATED: &str = "x-connector-auth";

pub fn connector_and_config_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<(connector_types::ConnectorEnum, ConnectorSpecificConfig), ApplicationErrorResponse>
{
    if let Some(header_value) = metadata.get(X_CONNECTOR_CONFIG) {
        return parse_connector_config_from_typed_header(header_value);
    }

    // Backward compat: accept the old header name with old JSON format
    if let Some(header_value) = metadata.get(X_CONNECTOR_AUTH_DEPRECATED) {
        logger::warn!(
            "x-connector-auth header is deprecated and will be removed in a future release. \
             Use x-connector-config with {{\"config\":{{...}}}} format instead."
        );
        return parse_connector_config_from_deprecated_header(header_value);
    }

    logger::debug!("Typed connector config headers not found, falling back to legacy headers");

    let connector = connector_from_metadata(metadata)?;
    let connector_config = legacy_connector_config_from_metadata(metadata, &connector)?;

    Ok((connector, connector_config))
}

/// Builds `ConnectorSpecificConfig` from legacy auth headers.
///
/// This only exists for backward compatibility with `x-auth` and related key headers.
pub fn legacy_connector_config_from_metadata(
    metadata: &metadata::MetadataMap,
    connector: &connector_types::ConnectorEnum,
) -> CustomResult<ConnectorSpecificConfig, ApplicationErrorResponse> {
    let generic_auth = generic_auth_from_metadata(metadata)?;
    ConnectorSpecificConfig::foreign_try_from((&generic_auth, connector)).map_err(|_| {
        Report::new(ApplicationErrorResponse::BadRequest(ApiError {
            sub_code: "AUTH_CONVERSION_FAILED".to_string(),
            error_identifier: 400,
            error_message: format!("Failed to convert legacy auth for connector: {}", connector),
            error_object: None,
        }))
    })
}

/// Extracts generic auth type from metadata headers.
/// This is the legacy format that uses key1, key2, etc.
pub fn generic_auth_from_metadata(
    metadata: &metadata::MetadataMap,
) -> CustomResult<ConnectorAuthType, ApplicationErrorResponse> {
    let auth = parse_metadata(metadata, X_AUTH)?;

    #[allow(clippy::wildcard_in_or_patterns)]
    match auth {
        "header-key" => Ok(ConnectorAuthType::HeaderKey {
            api_key: parse_metadata(metadata, X_API_KEY)?.to_string().into(),
        }),
        "body-key" => Ok(ConnectorAuthType::BodyKey {
            api_key: parse_metadata(metadata, X_API_KEY)?.to_string().into(),
            key1: parse_metadata(metadata, X_KEY1)?.to_string().into(),
        }),
        "signature-key" => Ok(ConnectorAuthType::SignatureKey {
            api_key: parse_metadata(metadata, X_API_KEY)?.to_string().into(),
            key1: parse_metadata(metadata, X_KEY1)?.to_string().into(),
            api_secret: parse_metadata(metadata, X_API_SECRET)?.to_string().into(),
        }),
        "multi-auth-key" => Ok(ConnectorAuthType::MultiAuthKey {
            api_key: parse_metadata(metadata, X_API_KEY)?.to_string().into(),
            key1: parse_metadata(metadata, X_KEY1)?.to_string().into(),
            key2: parse_metadata(metadata, X_KEY2)?.to_string().into(),
            api_secret: parse_metadata(metadata, X_API_SECRET)?.to_string().into(),
        }),
        "no-key" => Ok(ConnectorAuthType::NoKey),
        "temporary-auth" => Ok(ConnectorAuthType::TemporaryAuth),
        "currency-auth-key" => {
            let auth_key_map_str = parse_metadata(metadata, X_AUTH_KEY_MAP)?;
            let auth_key_map: HashMap<
                common_enums::enums::Currency,
                common_utils::pii::SecretSerdeValue,
            > = serde_json::from_str(auth_key_map_str).change_context(
                ApplicationErrorResponse::BadRequest(ApiError {
                    sub_code: "INVALID_AUTH_KEY_MAP".to_string(),
                    error_identifier: 400,
                    error_message: "Invalid auth-key-map format".to_string(),
                    error_object: None,
                }),
            )?;
            Ok(ConnectorAuthType::CurrencyAuthKey { auth_key_map })
        }
        "certificate-auth" | _ => Err(Report::new(ApplicationErrorResponse::BadRequest(
            ApiError {
                sub_code: "INVALID_AUTH_TYPE".to_string(),
                error_identifier: 400,
                error_message: format!("Invalid auth type: {auth}"),
                error_object: None,
            },
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::connector_and_config_from_metadata;
    use common_utils::consts;
    use domain_types::{connector_types, router_data::ConnectorSpecificConfig};
    use hyperswitch_masking::ExposeInterface;
    use tonic::metadata::MetadataMap;

    /// Build JSON for a Stripe ConnectorSpecificConfig header value.
    fn stripe_config_json(api_key: &str) -> String {
        format!(r#"{{"config":{{"Stripe":{{"api_key":"{}"}}}}}}"#, api_key)
    }

    /// Build a MetadataMap with a typed `X-Connector-Config` JSON header for Stripe.
    fn metadata_with_typed_config(api_key: &str) -> MetadataMap {
        let mut metadata = MetadataMap::new();
        let json = stripe_config_json(api_key);
        metadata.insert(
            consts::X_CONNECTOR_CONFIG,
            json.parse().expect("valid x-connector-config header"),
        );
        metadata
    }

    /// Build a MetadataMap with legacy `x-auth` / `x-api-key` headers and `x-connector` header.
    fn metadata_with_legacy_auth(api_key: &str) -> MetadataMap {
        let mut metadata = MetadataMap::new();
        metadata.insert(
            consts::X_AUTH,
            "header-key".parse().expect("valid x-auth header"),
        );
        metadata.insert(
            consts::X_API_KEY,
            api_key.parse().expect("valid x-api-key header"),
        );
        metadata.insert(
            consts::X_CONNECTOR_NAME,
            "stripe".parse().expect("valid x-connector header"),
        );
        metadata
    }

    #[test]
    fn connector_config_resolves_from_typed_config_header() {
        let metadata = metadata_with_typed_config("typed-key-value");

        let (connector, config) = connector_and_config_from_metadata(&metadata)
            .expect("typed header config should resolve");

        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
        match config {
            ConnectorSpecificConfig::Stripe { api_key, .. } => {
                assert_eq!(api_key.expose(), "typed-key-value");
            }
            _ => panic!("expected stripe config"),
        }
    }

    #[test]
    fn connector_config_falls_back_to_legacy_headers() {
        let metadata = metadata_with_legacy_auth("legacy-key-value");

        let (connector, config) = connector_and_config_from_metadata(&metadata)
            .expect("legacy header config should resolve");

        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
        match config {
            ConnectorSpecificConfig::Stripe { api_key, .. } => {
                assert_eq!(api_key.expose(), "legacy-key-value");
            }
            _ => panic!("expected stripe config"),
        }
    }

    #[test]
    fn connector_config_prefers_typed_config_header_over_legacy() {
        let mut metadata = metadata_with_legacy_auth("legacy-key-value");
        let json = stripe_config_json("typed-key-value");
        metadata.insert(
            consts::X_CONNECTOR_CONFIG,
            json.parse().expect("valid x-connector-config header"),
        );

        let (connector, config) = connector_and_config_from_metadata(&metadata)
            .expect("typed header should take precedence");

        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
        match config {
            ConnectorSpecificConfig::Stripe { api_key, .. } => {
                assert_eq!(api_key.expose(), "typed-key-value");
            }
            _ => panic!("expected stripe config"),
        }
    }

    #[test]
    fn connector_config_fails_when_no_auth_present() {
        let metadata = MetadataMap::new();

        let result = connector_and_config_from_metadata(&metadata);

        assert!(result.is_err());
    }

    #[test]
    fn deprecated_x_connector_auth_header_still_works() {
        // Old format: {"auth_type":{"Stripe":{"api_key":"..."}}}
        let old_json = format!(
            r#"{{"auth_type":{{"Stripe":{{"api_key":"{}"}}}}}}"#,
            "deprecated-key-value"
        );
        let mut metadata = MetadataMap::new();
        metadata.insert(
            super::X_CONNECTOR_AUTH_DEPRECATED,
            old_json.parse().expect("valid header"),
        );

        let (connector, config) = connector_and_config_from_metadata(&metadata)
            .expect("deprecated x-connector-auth should still resolve");

        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
        match config {
            ConnectorSpecificConfig::Stripe { api_key, .. } => {
                assert_eq!(api_key.expose(), "deprecated-key-value");
            }
            _ => panic!("expected stripe config"),
        }
    }

    #[test]
    fn new_header_takes_precedence_over_deprecated() {
        let new_json = stripe_config_json("new-key");
        let old_json = r#"{"auth_type":{"Stripe":{"api_key":"old-key"}}}"#.to_string();
        let mut metadata = MetadataMap::new();
        metadata.insert(
            consts::X_CONNECTOR_CONFIG,
            new_json.parse().expect("valid header"),
        );
        metadata.insert(
            super::X_CONNECTOR_AUTH_DEPRECATED,
            old_json.parse().expect("valid header"),
        );

        let (connector, config) = connector_and_config_from_metadata(&metadata)
            .expect("new header should take precedence");

        assert_eq!(connector, connector_types::ConnectorEnum::Stripe);
        match config {
            ConnectorSpecificConfig::Stripe { api_key, .. } => {
                assert_eq!(api_key.expose(), "new-key");
            }
            _ => panic!("expected stripe config"),
        }
    }
}
