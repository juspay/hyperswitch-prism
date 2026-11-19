pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::BoostWebhookBody;

#[derive(Debug, Clone)]
pub struct Boost<T> {
    _phantom: PhantomData<T>,
}

impl<T> Boost<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Boost<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Boost<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse Boost webhook body to determine event type")?;
        Ok(payload.get_event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let payload: BoostWebhookBody = request
            .body
            .parse_struct("BoostWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse Boost webhook body to build the webhook reference",
            )?;
        Ok(Some(payload.into_webhook_event_reference()))
    }
}
