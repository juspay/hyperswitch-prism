pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::Report;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Braintree<T> {
    _phantom: PhantomData<T>,
}

impl<T> Braintree<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Braintree<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Braintree<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let notif = transformers::decode_from_request(&request)?;
        Ok(transformers::get_status(notif.kind.as_str()))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notif = transformers::decode_from_request(&request)?;
        transformers::get_webhook_reference(&notif)
    }
}
