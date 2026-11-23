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
pub struct Finix<T> {
    _phantom: PhantomData<T>,
}

impl<T> Finix<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Finix<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Finix<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // HS: an empty body or a body that trims to `{}` is Finix's
        // endpoint-verification probe (get_webhook_event_type).
        if transformers::is_endpoint_verification_body(&request.body) {
            return Ok(EventType::EndpointVerification);
        }
        let webhook_body = transformers::parse_finix_webhook_body(&request.body)?;
        transformers::get_finix_webhook_event_type(&webhook_body)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // Endpoint-verification probes carry no resource; HS's Direct gateway
        // also resolves no reference for them (`get_webhook_object_reference_id`
        // fails to parse `{}` and the router `.ok()`s it away).
        if transformers::is_endpoint_verification_body(&request.body) {
            return Ok(None);
        }
        let webhook_body = transformers::parse_finix_webhook_body(&request.body)?;
        transformers::get_finix_webhook_reference(&webhook_body)
    }
}
