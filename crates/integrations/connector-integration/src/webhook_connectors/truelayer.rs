pub mod transformers;

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{RequestDetails, WebhookParseResponse},
    errors::WebhookError,
};
use error_stack::ResultExt;
use interfaces::webhook_common::WebhookCommon;
use transformers as truelayer;

#[derive(Debug, Clone, Default)]
pub struct TruelayerWebhook;

impl TruelayerWebhook {
    pub const fn new() -> Self {
        Self
    }
}

impl WebhookCommon for TruelayerWebhook {
    fn parse_event(
        &self,
        request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
        transformers::parse_event(request)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let webhook_body: truelayer::TruelayerWebhookEventTypeBody = request
            .body
            .parse_struct("TruelayerPayoutsWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(truelayer::get_webhook_event(webhook_body._type))
    }
}
