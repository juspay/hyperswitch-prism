pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{get_webhook_reference, map_webhook_event_type, WebhookEventTypeBody};
use crate::connectors::stripe::transformers::get_webhook_object_from_body;
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::{Report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Stripe<T> {
    _phantom: PhantomData<T>,
}

impl<T> Stripe<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Stripe<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Stripe<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        // `get_event_type` decodes the lighter `WebhookEventTypeBody` (it exposes
        // `payment_method_details`, needed for the `charge.succeeded` sub-mapping), so it keeps
        // its own decode/error variant rather than sharing `get_webhook_object_from_body`.
        let details: WebhookEventTypeBody = request
            .body
            .parse_struct("WebhookEventTypeBody")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        Ok(map_webhook_event_type(&details))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let details = get_webhook_object_from_body(&request.body)
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        get_webhook_reference(&details)
    }
}
