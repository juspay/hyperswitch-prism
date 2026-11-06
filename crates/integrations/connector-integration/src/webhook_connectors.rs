//! Provider-level parsing, local verification, and acknowledgement operations.
//!
//! Future providers implement `interfaces::webhook_common::WebhookCommon` in
//! `<connector>.rs`, with payload types and mappings in `<connector>/transformers.rs`.
//! Register implementations below when a provider is ready for migration.

use domain_types::connector_types::ConnectorVariant;
use interfaces::webhook_common::BoxedWebhookConnector;

/// Register standalone webhook implementations here as providers migrate.
/// The migration resolver rejects explicit or configured selection when unregistered.
/// No providers are migrated by this framework change.
pub fn get_connector(_variant: &ConnectorVariant) -> Option<BoxedWebhookConnector> {
    None
}
