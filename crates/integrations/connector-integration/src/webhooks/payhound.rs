use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::payhound::transformers::{PayhoundInvoiceResponse, PayhoundInvoiceStatus};
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

#[derive(Debug, Clone)]
pub struct Payhound<T> {
    _phantom: PhantomData<T>,
}

impl<T> Payhound<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Payhound<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Payhound<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Payhound sends no event-type field: the callback body *is* the invoice object, so the
        // event is derived from its status.
        let invoice: PayhoundInvoiceResponse = request
            .body
            .parse_struct("PayhoundInvoiceResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        match invoice.status {
            PayhoundInvoiceStatus::Completed | PayhoundInvoiceStatus::Overpaid => {
                Ok(EventType::PaymentIntentSuccess)
            }
            PayhoundInvoiceStatus::Pending | PayhoundInvoiceStatus::Underpaid => {
                Ok(EventType::PaymentActionRequired)
            }
            PayhoundInvoiceStatus::Aborted | PayhoundInvoiceStatus::Timeout => {
                Ok(EventType::PaymentIntentFailure)
            }
            PayhoundInvoiceStatus::Unknown => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let invoice: PayhoundInvoiceResponse = request
            .body
            .parse_struct("PayhoundInvoiceResponse")
            .change_context(WebhookError::WebhookResourceObjectNotFound)?;

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: Some(invoice.id),
                merchant_transaction_id: invoice.reference,
                connector_preprocessing_id: None,
            },
        )))
    }
}
