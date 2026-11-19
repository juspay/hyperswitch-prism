pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails, WebhookResourceReference},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::GrabpayWebhookBody;

#[derive(Debug, Clone)]
pub struct Grabpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Grabpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Grabpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Grabpay<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        transformers::grabpay_webhook_event_type(&webhook_body)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let webhook_body: GrabpayWebhookBody = request
            .body
            .parse_struct("GrabpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(Some(webhook_body.webhook_reference()))
    }
}
