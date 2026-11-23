pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Hyperswitch<T> {
    _phantom: PhantomData<T>,
}

impl<T> Hyperswitch<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Hyperswitch<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Hyperswitch<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = transformers::get_webhook_object_from_body(&request.body)?;
        Ok(transformers::map_webhook_event_type(&body.event_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body = transformers::get_webhook_object_from_body(&request.body)?;
        transformers::get_webhook_reference(&body)
    }
}
