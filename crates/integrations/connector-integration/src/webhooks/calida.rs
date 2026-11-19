use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Calida<T> {
    _phantom: PhantomData<T>,
}

impl<T> Calida<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Calida<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Calida<T>
{
    // Calida webhooks only carry payment status updates; the event type does not depend on the body.
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        _request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        Ok(EventType::Payment)
    }
}
