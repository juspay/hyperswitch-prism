use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::maya::transformers::MayaWebhookBody;
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
pub struct Maya<T> {
    _phantom: PhantomData<T>,
}

impl<T> Maya<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Maya<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Maya<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let body: MayaWebhookBody = request
            .body
            .parse_struct("MayaWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(body.status))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let body: MayaWebhookBody = request
            .body
            .parse_struct("MayaWebhookBody")
            .change_context(errors::WebhookError::WebhookResourceObjectNotFound)?;

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                // Maya's payment `id` is the connector transaction id.
                connector_transaction_id: Some(body.id),
                // `requestReferenceNumber` is the merchant-assigned reference we
                // sent when creating the payment, echoed back by Maya.
                merchant_transaction_id: body.request_reference_number,
                connector_preprocessing_id: None,
            },
        )))
    }
}
