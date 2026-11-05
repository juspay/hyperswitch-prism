//! Provider implementations for the stateless `ParseEvent` phase.
//!
//! Each provider has a `<connector>.rs` module implementing
//! `interfaces::webhook_parse::WebhookParse`, and a
//! `<connector>/transformers.rs` module containing its payload types and mappings.
//! Payment and payout events from the same provider share one parser.
//!
//! Export its module and connector type here, following `payout_connectors.rs`.
//! TrueLayer and Trustly use standalone parsers; other providers use the payment adapter.

pub mod truelayer;
pub mod trustly;
pub use self::{truelayer::TruelayerWebhook, trustly::TrustlyWebhook};

use domain_types::connector_types::{ConnectorEnum, ConnectorVariant, PayoutConnectorEnum};
use interfaces::webhook_parse::BoxedWebhookConnector;

/// Registers providers migrated to the standalone webhook implementation.
///
/// Match the appropriate payment and payout variants for a shared provider.
/// Returning `None` permits the caller to resolve the existing payment parser.
/// A registered parser's errors are propagated without falling back to payments.
pub fn get_connector(variant: &ConnectorVariant) -> Option<BoxedWebhookConnector> {
    match variant {
        ConnectorVariant::Payment(ConnectorEnum::Truelayer)
        | ConnectorVariant::Payout(PayoutConnectorEnum::Truelayer) => {
            Some(Box::new(TruelayerWebhook::new()))
        }
        ConnectorVariant::Payment(ConnectorEnum::Trustly)
        | ConnectorVariant::Payout(PayoutConnectorEnum::Trustly) => {
            Some(Box::new(TrustlyWebhook::new()))
        }
        _ => None,
    }
}
