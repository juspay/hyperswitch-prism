//! Migration selection for common webhook operations, shared by transports.

use common_utils::{
    consts::X_WEBHOOK_CONNECTOR_NAME, errors::CustomResult, metadata::MaskedMetadata,
};
use connector_integration::types::{
    ConnectorData, ConnectorDataProvider, ParseEventConnectorData, WebhookConnectorData,
};
use domain_types::{
    connector_types::ConnectorVariant,
    errors::{IntegrationError, IntegrationErrorContext},
};
use error_stack::Report;
use ucs_env::configs::Config;

// TODO(webhook-migration): remove legacy selection and the rollout gate once all
// supported connectors use Webhook and their compatibility checks pass. The
// x-webhook-connector header remains permanent provider identity.
/// Resolve routing policy once; callers only match the returned connector data.
/// An explicitly selected standalone implementation never falls back to payments.
pub fn get_webhook_connector_data(
    connector: &ConnectorVariant,
    config: &Config,
    metadata: &MaskedMetadata,
) -> CustomResult<ParseEventConnectorData, IntegrationError> {
    let explicit_name = metadata.get_raw(X_WEBHOOK_CONNECTOR_NAME);
    let explicit_connector = explicit_name
        .as_deref()
        .map(crate::metadata::webhook_connector_from_name)
        .transpose()?;
    let connector = match explicit_connector.as_ref() {
        Some(explicit) => {
            if explicit.get_connector_name() != connector.get_connector_name() {
                return Err(Report::new(IntegrationError::InvalidDataFormat {
                    field_name: X_WEBHOOK_CONNECTOR_NAME,
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Webhook connector must match the typed connector configuration"
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }));
            }
            explicit
        }
        None => connector,
    };

    let connector_data =
        if explicit_connector.is_some() || config.webhook_parse.is_migrated(connector) {
            WebhookConnectorData::from_connector_variant(connector)
                .map(ParseEventConnectorData::Webhook)
        } else {
            ConnectorData::from_connector_variant(connector).map(ParseEventConnectorData::Payment)
        };

    connector_data.ok_or_else(|| {
        Report::new(IntegrationError::InvalidDataFormat {
            field_name: "connector",
            context: IntegrationErrorContext {
                additional_context: Some(connector.get_connector_name()),
                ..Default::default()
            },
        })
    })
}

/// Compatibility entry point for existing ParseEvent callers.
pub use get_webhook_connector_data as get_parse_event_connector_data;

/// Reject invalid rollout entries while loading configuration, before serving requests.
pub fn validate_webhook_migration_config(config: &Config) -> CustomResult<(), IntegrationError> {
    if let Some(names) = config.webhook_parse.migrated_connectors.as_ref() {
        for name in names {
            let provider = crate::metadata::webhook_connector_from_name(name).map_err(|_| {
                Report::new(IntegrationError::InvalidDataFormat {
                    field_name: "webhook_parse.migrated_connectors",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!("Unknown webhook provider: {name}")),
                        ..Default::default()
                    },
                })
            })?;
            if WebhookConnectorData::from_connector_variant(&provider).is_none() {
                return Err(Report::new(IntegrationError::InvalidDataFormat {
                    field_name: "webhook_parse.migrated_connectors",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!(
                            "No webhook implementation registered for {name}"
                        )),
                        ..Default::default()
                    },
                }));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain_types::connector_types::{ConnectorEnum, ConnectorVariant};
    fn config() -> Config {
        Config::new_with_config_path(Some(std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../config/development.toml"
        ))))
        .unwrap()
    }
    #[test]
    fn empty_rollout_preserves_legacy_and_unregistered_rollout_is_rejected() {
        let mut config = config();
        config.webhook_parse.migrated_connectors = None;
        assert!(validate_webhook_migration_config(&config).is_ok());
        let provider = ConnectorVariant::Payment(ConnectorEnum::Checkout);
        assert!(matches!(
            get_webhook_connector_data(&provider, &config, &MaskedMetadata::default()).unwrap(),
            ParseEventConnectorData::Payment(_)
        ));
        config.webhook_parse.migrated_connectors = Some(["checkout".to_string()].into());
        assert!(validate_webhook_migration_config(&config).is_err());
        assert!(
            get_webhook_connector_data(&provider, &config, &MaskedMetadata::default()).is_err()
        );
    }
    #[test]
    fn explicit_provider_selection_rejects_missing_registration_without_fallback() {
        let mut raw = tonic::metadata::MetadataMap::new();
        raw.insert(X_WEBHOOK_CONNECTOR_NAME, "checkout".parse().unwrap());
        let metadata = MaskedMetadata::new(raw, Default::default());
        let mut config = config();
        config.webhook_parse.migrated_connectors = None;
        let provider = ConnectorVariant::Payment(ConnectorEnum::Checkout);
        assert!(get_webhook_connector_data(&provider, &config, &metadata).is_err());
        let mismatched = ConnectorVariant::Payment(ConnectorEnum::Trustly);
        let error = get_webhook_connector_data(&mismatched, &config, &metadata)
            .err()
            .unwrap();
        assert!(matches!(
            error.current_context(),
            IntegrationError::InvalidDataFormat {
                field_name: X_WEBHOOK_CONNECTOR_NAME,
                ..
            }
        ));
    }
}
