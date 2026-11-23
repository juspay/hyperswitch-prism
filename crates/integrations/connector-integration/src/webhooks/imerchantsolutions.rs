pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::imerchantsolutions::transformers::{
    ForeignTryFrom, ImerchantsolutionsWebhookData, ImerchantsolutionsWebhookEventType,
};
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
pub struct Imerchantsolutions<T> {
    _phantom: PhantomData<T>,
}

impl<T> Imerchantsolutions<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Imerchantsolutions<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Imerchantsolutions<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        EventType::foreign_try_from((webhook_body.event_type, webhook_body.status))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let webhook_body: ImerchantsolutionsWebhookData = request
            .body
            .parse_struct("ImerchantsolutionsWebhookData")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_resource_reference = match webhook_body.event_type {
            ImerchantsolutionsWebhookEventType::PaymentCompleted
            | ImerchantsolutionsWebhookEventType::PaymentCancelled
            | ImerchantsolutionsWebhookEventType::PaymentFailed => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(webhook_body.psp_reference),
                    merchant_transaction_id: webhook_body.merchant_reference,
                    connector_preprocessing_id: None,
                })
            }
            ImerchantsolutionsWebhookEventType::PaymentRefunded => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(webhook_body.psp_reference.clone()),
                    merchant_refund_id: Some(webhook_body.psp_reference),
                    connector_transaction_id: webhook_body.original_reference,
                    merchant_transaction_id: None,
                })
            }
        };

        Ok(Some(webhook_resource_reference))
    }
}
