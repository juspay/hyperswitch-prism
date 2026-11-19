pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::ReddotWebhookBody;
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RequestDetails, WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Reddot<T> {
    _phantom: PhantomData<T>,
}

impl<T> Reddot<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Reddot<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Reddot<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body: ReddotWebhookBody = request
            .body
            .parse_struct("ReddotWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(match body.response_code.as_str() {
            "0" => EventType::PaymentIntentSuccess,
            "-01" => EventType::PaymentIntentProcessing,
            _ => EventType::PaymentIntentFailure,
        })
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body: ReddotWebhookBody = request
            .body
            .parse_struct("ReddotWebhookBody")
            .change_context(errors::WebhookError::WebhookResourceObjectNotFound)?;

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: body.transaction_id,
                merchant_transaction_id: body.merchant_reference,
                connector_preprocessing_id: None,
            },
        )))
    }
}
