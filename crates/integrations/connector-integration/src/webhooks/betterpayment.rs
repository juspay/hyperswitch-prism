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
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::BetterpaymentWebhookBody;

#[derive(Debug, Clone)]
pub struct Betterpayment<T> {
    _phantom: PhantomData<T>,
}

impl<T> Betterpayment<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Betterpayment<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Betterpayment<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: BetterpaymentWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(transformers::get_webhook_event_type(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: BetterpaymentWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        if let Some(refund_id) = body.refund_id {
            return Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: Some(refund_id),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(body.transaction_id),
                    merchant_transaction_id: None,
                },
            )));
        }

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_preprocessing_id: None,
                connector_transaction_id: Some(body.transaction_id),
                merchant_transaction_id: None,
            },
        )))
    }
}
