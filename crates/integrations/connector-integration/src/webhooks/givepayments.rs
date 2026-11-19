pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{GivepaymentsIncomingWebhookData, GivepaymentsWebhookData};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookResourceReference,
    },
    errors::{self},
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Givepayments<T> {
    _phantom: PhantomData<T>,
}

impl<T> Givepayments<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Givepayments<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Givepayments<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(webhook_body.event_type.into())
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let webhook_body: GivepaymentsIncomingWebhookData = request
            .body
            .parse_struct("GivepaymentsIncomingWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_resource_reference = match webhook_body.data {
            GivepaymentsWebhookData::Payment(response_data) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(response_data.id),
                    merchant_transaction_id: response_data.external_reference,
                    connector_preprocessing_id: None,
                })
            }
            GivepaymentsWebhookData::Refund(response_data) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(response_data.id),
                    merchant_refund_id: response_data.external_reference,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                })
            }
        };

        Ok(Some(webhook_resource_reference))
    }
}
