pub mod transformers;

use domain_types::{
    connector_types::{RequestDetails, WebhookParseResponse},
    errors::WebhookError,
};
use interfaces::webhook_parse::WebhookParse;

#[derive(Debug, Clone, Default)]
pub struct TrustlyWebhook;

impl TrustlyWebhook {
    pub const fn new() -> Self {
        Self
    }
}

impl WebhookParse for TrustlyWebhook {
    fn parse_event(
        &self,
        request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
        transformers::parse_event(request)
    }
}
