//! Standalone provider webhook modules and registry.
//!
//! Implement `Webhook` in `webhook_connectors/<provider>.rs` and declare its module
//! here. Add one registration so both runtime dispatch and field-probe discover it.

use domain_types::connector_types::WebhookProvider;
use interfaces::webhook::BoxedWebhookConnector;

struct Registration {
    name: &'static str,
    build: fn() -> BoxedWebhookConnector,
}

// No production providers are migrated by this framework change.
const PROVIDERS: &[Registration] = &[];

/// Enumerate provider identities independently of payment and payout connector enums.
pub fn registered_providers() -> impl Iterator<Item = WebhookProvider> {
    PROVIDERS
        .iter()
        .filter_map(|entry| WebhookProvider::new(entry.name))
}

pub fn get_connector(provider: &WebhookProvider) -> Option<BoxedWebhookConnector> {
    PROVIDERS
        .iter()
        .find(|entry| entry.name == provider.as_str())
        .map(|entry| (entry.build)())
}
