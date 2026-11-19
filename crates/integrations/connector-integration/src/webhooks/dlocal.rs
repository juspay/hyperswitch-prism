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

use self::transformers::DlocalWebhookBody;

#[derive(Debug, Clone)]
pub struct Dlocal<T> {
    _phantom: PhantomData<T>,
}

impl<T> Dlocal<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Dlocal<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Dlocal<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(EventType::from(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: DlocalWebhookBody = request
            .body
            .parse_struct("DlocalWebhookBody")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                // dLocal payment `id` is the connector transaction id.
                connector_transaction_id: Some(body.id),
                // `order_id` is used for payment objects; `external_id` is used
                // for enrollment objects.
                merchant_transaction_id: body.order_id.or(body.external_id),
                connector_preprocessing_id: None,
            },
        )))
    }
}
