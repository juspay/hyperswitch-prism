pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::PayhereWebhookPayload;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Payhere<T> {
    _phantom: PhantomData<T>,
}

impl<T> Payhere<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Payhere<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Payhere<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let payload: PayhereWebhookPayload =
            serde_urlencoded::from_bytes::<PayhereWebhookPayload>(&request.body).map_err(|_| {
                error_stack::report!(errors::WebhookError::WebhookEventTypeNotFound)
            })?;

        match payload.status_code {
            2 => Ok(EventType::PaymentIntentSuccess),
            0 => Ok(EventType::PaymentIntentProcessing),
            -1 | -2 => Ok(EventType::PaymentIntentFailure),
            _ => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }
}
