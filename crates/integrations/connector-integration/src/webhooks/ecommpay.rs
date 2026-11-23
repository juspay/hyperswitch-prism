pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Ecommpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Ecommpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Ecommpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Ecommpay<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body = transformers::parse_webhook_body(&request.body)?;
        Ok(transformers::webhook_event_type(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body = transformers::parse_webhook_body(&request.body)?;
        transformers::webhook_reference(&body)
    }
}
