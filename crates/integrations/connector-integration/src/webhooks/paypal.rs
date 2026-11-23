pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::get_paypal_event_type;
use self::transformers::{DisputeOutcome, PaypalWebooksEventType};
use crate::connectors::paypal::transformers::PaypalWebhookEventType;
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
pub struct Paypal<T> {
    _phantom: PhantomData<T>,
}

impl<T> Paypal<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Paypal<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Paypal<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let payload: PaypalWebooksEventType =
            request
                .body
                .parse_struct("PaypalWebooksEventType")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let outcome_code = match payload.event_type {
            PaypalWebhookEventType::CustomerDisputeResolved => Some(
                request
                    .body
                    .parse_struct::<DisputeOutcome>("PaypalDisputeOutcome")
                    .change_context(WebhookError::WebhookEventTypeNotFound)?
                    .outcome_code,
            ),
            _ => None,
        };

        Ok(get_paypal_event_type(payload.event_type, outcome_code))
    }
}
