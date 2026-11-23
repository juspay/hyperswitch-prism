pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::get_webhook_event;
use crate::connectors::trustly::transformers::{
    is_payment_webhook_event, is_payout_webhook_event, is_refund_webhook_event, TrustlyWebhookBody,
};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, PayoutWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use hyperswitch_masking::ExposeInterface;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Trustly<T> {
    _phantom: PhantomData<T>,
}

impl<T> Trustly<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Trustly<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Trustly<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;
        let message_id = webhook_body.params.data.messageid.clone();

        Ok(get_webhook_event(webhook_body.method, message_id.expose()))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let message_id = webhook_body.params.data.messageid.clone().expose();
        let order_id = webhook_body.params.data.orderid;

        if is_payment_webhook_event(&webhook_body.method, &message_id) {
            return Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(order_id),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                },
            )));
        }

        if is_refund_webhook_event(&webhook_body.method, &message_id) {
            return Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: Some(order_id.clone()),
                    merchant_refund_id: None,
                    connector_transaction_id: Some(order_id),
                    merchant_transaction_id: None,
                },
            )));
        }

        if is_payout_webhook_event(&webhook_body.method, &message_id) {
            return Ok(Some(WebhookResourceReference::Payout(
                PayoutWebhookReference {
                    connector_payout_id: Some(order_id),
                    merchant_payout_id: None,
                },
            )));
        }

        Ok(None)
    }
}
