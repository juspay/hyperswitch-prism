pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::elavon_pg::parse_elavon_pg_notification;
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
pub struct ElavonPg<T> {
    _phantom: PhantomData<T>,
}

impl<T> ElavonPg<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for ElavonPg<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for ElavonPg<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        Ok(parse_elavon_pg_notification(&request)?.event_type())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notification = parse_elavon_pg_notification(&request)?;
        Ok(match notification.event_type() {
            EventType::IncomingWebhookEventUnspecified => None,
            _ if notification.is_refund_event() => {
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(notification.transaction_id()?),
                    merchant_refund_id: None,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                }))
            }
            _ => Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(notification.transaction_id()?),
                merchant_transaction_id: None,
                connector_preprocessing_id: None,
            })),
        })
    }
}
