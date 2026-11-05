//! Connector selection for ParseEvent, shared by transport implementations.

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

/// Resolve routing policy once; callers only match the returned connector data.
/// An explicitly selected standalone implementation never falls back to payments.
pub fn get_parse_event_connector_data(
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
