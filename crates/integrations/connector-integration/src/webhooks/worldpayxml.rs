pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::errors::CustomResult;
use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Worldpayxml<T> {
    _phantom: PhantomData<T>,
}

impl<T> Worldpayxml<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Worldpayxml<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Worldpayxml<T>
{
    fn get_event_type(&self, request: RequestDetails) -> CustomResult<EventType, WebhookError> {
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }
        Ok(transformers::parse_webhook_body(&request.body)?.event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> CustomResult<Option<WebhookResourceReference>, WebhookError> {
        if request.body.is_empty() {
            return Ok(None);
        }
        transformers::parse_webhook_body(&request.body)?.event_reference()
    }
}
