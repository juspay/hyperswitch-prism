use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::globalpay::transformers::{
    GlobalpayWebhookBody, GlobalpayWebhookStatus, GlobalpayWebhookTransactionType,
};
use common_utils::ext_traits::ByteSliceExt;
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

#[derive(Debug, Clone)]
pub struct Globalpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Globalpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Globalpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Globalpay<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse GlobalPay webhook body for event type detection")?;

        let event_type = if body.transaction_type == Some(GlobalpayWebhookTransactionType::Refund) {
            match body.status {
                GlobalpayWebhookStatus::Captured | GlobalpayWebhookStatus::Funded => {
                    EventType::RefundSuccess
                }
                GlobalpayWebhookStatus::Declined
                | GlobalpayWebhookStatus::Failed
                | GlobalpayWebhookStatus::Rejected
                | GlobalpayWebhookStatus::Reversed => EventType::RefundFailure,
                _ => EventType::RefundProcessing,
            }
        } else {
            match body.status {
                GlobalpayWebhookStatus::Captured | GlobalpayWebhookStatus::Funded => {
                    EventType::PaymentIntentSuccess
                }
                GlobalpayWebhookStatus::Preauthorized => {
                    EventType::PaymentIntentAuthorizationSuccess
                }
                GlobalpayWebhookStatus::Declined
                | GlobalpayWebhookStatus::Failed
                | GlobalpayWebhookStatus::Rejected => EventType::PaymentIntentFailure,
                GlobalpayWebhookStatus::Reversed => EventType::PaymentIntentCancelled,
                _ => EventType::PaymentIntentProcessing,
            }
        };

        Ok(event_type)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: GlobalpayWebhookBody = request
            .body
            .parse_struct("GlobalpayWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable(
                "Failed to parse GlobalPay webhook body for event reference extraction",
            )?;

        let reference = if body.transaction_type == Some(GlobalpayWebhookTransactionType::Refund) {
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id: Some(body.id),
                merchant_refund_id: None,
                connector_transaction_id: None,
                merchant_transaction_id: body.reference,
            })
        } else {
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(body.id),
                merchant_transaction_id: body.reference,
                connector_preprocessing_id: None,
            })
        };

        Ok(Some(reference))
    }
}
