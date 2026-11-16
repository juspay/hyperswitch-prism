//! Migration selection for common webhook operations, shared by transports.

use crate::types::{
    ConnectorData, ConnectorDataProvider, ParseEventConnectorData, WebhookConnectorData,
};
use common_utils::{consts::X_WEBHOOK_CONNECTOR_NAME, errors::CustomResult};
use domain_types::{
    connector_types::{ConnectorVariant, WebhookProvider},
    errors::{IntegrationError, IntegrationErrorContext},
};
use error_stack::Report;

// TODO(webhook-migration): remove legacy selection and the rollout gate once all
// supported connectors use Webhook and their compatibility checks pass. The
// x-webhook-connector header remains permanent provider identity.
/// Resolve routing policy once; callers only match the returned connector data.
/// An explicitly selected standalone implementation never falls back to payments.
pub fn get_webhook_connector_data(
    connector: &ConnectorVariant,
    migrated: bool,
    explicit_name: Option<&str>,
) -> CustomResult<ParseEventConnectorData, IntegrationError> {
    let explicit_provider = explicit_name
        .map(|name| {
            WebhookProvider::new(name)
                .ok_or_else(|| invalid_provider(X_WEBHOOK_CONNECTOR_NAME, name))
        })
        .transpose()?;
    if let Some(provider) = explicit_provider.as_ref() {
        if provider.as_str() != connector.get_connector_name() {
            return Err(invalid_provider(
                X_WEBHOOK_CONNECTOR_NAME,
                "Webhook connector must match the typed connector configuration",
            ));
        }
    }

    let connector_data = if explicit_provider.is_some()
        || matches!(connector, ConnectorVariant::Webhook(_))
        || migrated
    {
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
pub fn validate_webhook_migration_config(
    names: Option<&std::collections::HashSet<String>>,
) -> CustomResult<(), IntegrationError> {
    if let Some(names) = names {
        for name in names {
            let provider = WebhookProvider::new(name)
                .ok_or_else(|| invalid_provider("webhook_parse.migrated_connectors", name))?;
            if crate::webhook_connectors::get_connector(&provider).is_none() {
                return Err(invalid_provider(
                    "webhook_parse.migrated_connectors",
                    &format!("No webhook implementation registered for {name}"),
                ));
            }
        }
    }
    Ok(())
}

fn invalid_provider(field_name: &'static str, message: &str) -> Report<IntegrationError> {
    Report::new(IntegrationError::InvalidDataFormat {
        field_name,
        context: IntegrationErrorContext {
            additional_context: Some(message.to_owned()),
            ..Default::default()
        },
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain_types::connector_types::ConnectorEnum;

    #[test]
    fn empty_rollout_preserves_legacy_and_unregistered_rollout_is_rejected() {
        assert!(validate_webhook_migration_config(None).is_ok());
        let connector = ConnectorVariant::Payment(ConnectorEnum::Checkout);
        assert!(matches!(
            get_webhook_connector_data(&connector, false, None).unwrap(),
            ParseEventConnectorData::Payment(_)
        ));
        let names = ["checkout".to_string()].into();
        assert!(validate_webhook_migration_config(Some(&names)).is_err());
        assert!(get_webhook_connector_data(&connector, true, None).is_err());
    }

    #[test]
    fn explicit_provider_rejects_missing_registration_and_mismatch_without_fallback() {
        let connector = ConnectorVariant::Payment(ConnectorEnum::Checkout);
        assert!(get_webhook_connector_data(&connector, false, Some("checkout")).is_err());
        let error = get_webhook_connector_data(&connector, false, Some("trustly"))
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
