pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::revpay::transformers::RevpayWebhookBody;
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

#[derive(Debug, Clone)]
pub struct Revpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Revpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Revpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Revpay<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        Ok(body.event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: RevpayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: Some(body.reference_number.clone()),
                // Reference_Number is the merchant-assigned reference the
                // original Payment call was keyed by.
                merchant_transaction_id: Some(body.reference_number),
                connector_preprocessing_id: None,
            },
        )))
    }
}
