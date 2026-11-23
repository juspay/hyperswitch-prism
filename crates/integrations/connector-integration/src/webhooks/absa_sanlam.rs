pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::{AbsaSanlamWebhookEvent, AbsaSanlamWebhookEventType};

#[derive(Debug, Clone)]
pub struct AbsaSanlam<T> {
    _phantom: PhantomData<T>,
}

impl<T> AbsaSanlam<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for AbsaSanlam<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for AbsaSanlam<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let details: AbsaSanlamWebhookEvent =
            request
                .body
                .parse_struct("AbsaSanlamWebhookEvent")
                .change_context(WebhookError::WebhookEventTypeNotFound)?;

        let event_type = match details {
            AbsaSanlamWebhookEvent::Payment(ref event) => match event.event_type {
                AbsaSanlamWebhookEventType::PaymentSucceeded => EventType::PaymentIntentSuccess,
                AbsaSanlamWebhookEventType::PaymentFailed => EventType::PaymentIntentFailure,
                AbsaSanlamWebhookEventType::DisputeOpened => EventType::DisputeOpened,
            },
        };

        Ok(event_type)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let details: AbsaSanlamWebhookEvent =
            request
                .body
                .parse_struct("AbsaSanlamWebhookEvent")
                .change_context(WebhookError::WebhookReferenceIdNotFound)?;

        let id = match details {
            AbsaSanlamWebhookEvent::Payment(ref event) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: None,
                    merchant_transaction_id: Some(event.payment.user_reference.clone()),
                    connector_preprocessing_id: None,
                })
            }
        };

        Ok(Some(id))
    }
}
