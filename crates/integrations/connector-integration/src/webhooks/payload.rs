pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::get_event_type_from_trigger;
use crate::connectors::payload::transformers::PayloadWebhookEvent;
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Payload<T> {
    _phantom: PhantomData<T>,
}

impl<T> Payload<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Payload<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Payload<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let webhook_body: PayloadWebhookEvent = request
            .body
            .parse_struct("PayloadWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(get_event_type_from_trigger(webhook_body.trigger))
    }
}
