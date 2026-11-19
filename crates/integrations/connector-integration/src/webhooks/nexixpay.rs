pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::Report;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Nexixpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Nexixpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Nexixpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Nexixpay<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let notification = transformers::parse_nexixpay_webhook(&request.body)?;
        transformers::get_nexixpay_webhook_event_type(&notification.operation)
            .ok_or_else(|| WebhookError::WebhookEventTypeNotFound.into())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notification = transformers::parse_nexixpay_webhook(&request.body)?;
        let operation = notification.operation;
        Ok(
            match transformers::get_nexixpay_webhook_event_type(&operation) {
                // orderId is the connector_transaction_id of every Nexi payment flow.
                Some(event) if event.is_payment_event() => {
                    Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                        connector_transaction_id: Some(operation.order_id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    }))
                }
                // The refund's operationId is the connector_refund_id returned by Refund.
                Some(event) if event.is_refund_event() => {
                    Some(WebhookResourceReference::Refund(RefundWebhookReference {
                        connector_refund_id: Some(operation.operation_id),
                        merchant_refund_id: None,
                        connector_transaction_id: Some(operation.order_id),
                        merchant_transaction_id: None,
                    }))
                }
                Some(_) | None => None,
            },
        )
    }
}
