pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{RevolutWebhookBody, RevolutWebhookEvent};
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
pub struct Revolut<T> {
    _phantom: PhantomData<T>,
}

impl<T> Revolut<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Revolut<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Revolut<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: RevolutWebhookBody = request
            .body
            .parse_struct("RevolutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        match notif.event {
            RevolutWebhookEvent::OrderCompleted => Ok(EventType::PaymentIntentSuccess),
            RevolutWebhookEvent::OrderAuthorised => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            RevolutWebhookEvent::OrderCancelled => Ok(EventType::PaymentIntentCancelled),
            RevolutWebhookEvent::OrderFailed => Ok(EventType::PaymentIntentFailure),
            RevolutWebhookEvent::OrderPaymentAuthenticated => {
                Ok(EventType::PaymentIntentAuthorizationSuccess)
            }
            RevolutWebhookEvent::OrderPaymentDeclined => {
                Ok(EventType::PaymentIntentAuthorizationFailure)
            }
            RevolutWebhookEvent::OrderPaymentFailed => Ok(EventType::PaymentIntentFailure),
            RevolutWebhookEvent::PayoutInitiated => Ok(EventType::PayoutCreated),
            RevolutWebhookEvent::PayoutCompleted => Ok(EventType::PayoutSuccess),
            RevolutWebhookEvent::PayoutFailed => Ok(EventType::PayoutFailure),
            RevolutWebhookEvent::DisputeActionRequired => Ok(EventType::DisputeOpened),
            RevolutWebhookEvent::DisputeUnderReview => Ok(EventType::DisputeOpened),
            RevolutWebhookEvent::DisputeWon => Ok(EventType::DisputeWon),
            RevolutWebhookEvent::DisputeLost => Ok(EventType::DisputeLost),
        }
    }
}
