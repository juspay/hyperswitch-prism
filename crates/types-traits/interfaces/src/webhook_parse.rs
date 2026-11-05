//! Stateless webhook parsing shared across payment, payout, and other event families.

use domain_types::{
    connector_types::{RequestDetails, WebhookParseResponse},
    errors::WebhookError,
};

/// Classifies an incoming notification and extracts its resource reference.
///
/// Implementations parse the provider payload locally. Source verification,
/// credentials, business context, and resource status processing belong to the
/// subsequent handling phase. The returned identifiers are unverified input.
pub trait WebhookParse: Send + Sync {
    fn parse_event(
        &self,
        request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>>;
}

/// Owns either a provider parser or an adapter for an existing connector.
pub type BoxedWebhookConnector = Box<dyn WebhookParse>;
