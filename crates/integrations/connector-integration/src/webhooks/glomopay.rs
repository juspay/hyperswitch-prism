pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::glomopay::transformers::{
    GlomopayRefundWebhookPayload, GlomopayWebhookEntityProbe, GlomopayWebhookPayload,
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
pub struct Glomopay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Glomopay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Glomopay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Glomopay<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let probe: GlomopayWebhookEntityProbe = request
            .body
            .parse_struct("GlomopayWebhookEntityProbe")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to read entity_type from Glomopay webhook")?;

        match probe.entity_type.as_str() {
            "payment" => {
                let payload: GlomopayWebhookPayload = request
                    .body
                    .parse_struct("GlomopayWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay payment webhook event type")?;
                Ok(payload.get_event_type())
            }
            "refund" => {
                let payload: GlomopayRefundWebhookPayload = request
                    .body
                    .parse_struct("GlomopayRefundWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay refund webhook event type")?;
                Ok(payload.get_event_type())
            }
            other => Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)
                .attach_printable(format!("Unknown Glomopay webhook entity_type: {other}"))),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let probe: GlomopayWebhookEntityProbe = request
            .body
            .parse_struct("GlomopayWebhookEntityProbe")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to read entity_type from Glomopay webhook")?;

        match probe.entity_type.as_str() {
            "payment" => {
                let payload: GlomopayWebhookPayload = request
                    .body
                    .parse_struct("GlomopayWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay payment webhook for reference")?;
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(payload.data.id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                )))
            }
            "refund" => {
                let payload: GlomopayRefundWebhookPayload = request
                    .body
                    .parse_struct("GlomopayRefundWebhookPayload")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable("Failed to parse Glomopay refund webhook for reference")?;
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(payload.data.id),
                        merchant_refund_id: None,
                        connector_transaction_id: payload.data.payment_id,
                        merchant_transaction_id: None,
                    },
                )))
            }
            _ => Ok(None),
        }
    }
}
