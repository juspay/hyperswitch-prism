pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{TamaraWebhookEvent, TamaraWebhookEventType};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Tamara<T> {
    _phantom: PhantomData<T>,
}

impl<T> Tamara<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Tamara<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Tamara<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(event.event_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let refund_id = event.data.as_ref().and_then(|d| d.refund_id.clone());

        match event.event_type {
            TamaraWebhookEvent::OrderRefunded => Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: refund_id.clone(),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(event.order_id),
                    merchant_transaction_id: refund_id,
                },
            ))),
            _ => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(event.order_id),
                    merchant_transaction_id: event.order_reference_id,
                    connector_preprocessing_id: None,
                },
            ))),
        }
    }
}
