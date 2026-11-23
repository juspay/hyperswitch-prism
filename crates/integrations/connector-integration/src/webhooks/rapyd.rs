pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::{Report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::{RapydIncomingWebhook, RapydWebhookEventType};

#[derive(Debug, Clone)]
pub struct Rapyd<T> {
    _phantom: PhantomData<T>,
}

impl<T> Rapyd<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Rapyd<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Rapyd<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let event: RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        event.try_into()
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let event: RapydIncomingWebhook = request
            .body
            .parse_struct("RapydIncomingWebhook")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        Ok(match event.webhook_type {
            RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::PaymentFailed => {
                let data = event.payment_data()?;
                Some(WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(data.id),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                }))
            }
            RapydWebhookEventType::RefundCompleted
            | RapydWebhookEventType::PaymentRefundRejected
            | RapydWebhookEventType::PaymentRefundFailed => {
                let data = event.refund_data()?;
                Some(WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(data.id),
                    connector_transaction_id: Some(data.payment),
                    merchant_refund_id: None,
                    merchant_transaction_id: None,
                }))
            }
            RapydWebhookEventType::PaymentDisputeCreated
            | RapydWebhookEventType::PaymentDisputeUpdated => {
                let data = event.dispute_data()?;
                Some(WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(data.token),
                    connector_transaction_id: Some(data.original_transaction_id),
                }))
            }
            RapydWebhookEventType::Unknown => None,
        })
    }
}
