pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::parse_cybersource_webhook_body;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::{Report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Cybersource<T> {
    _phantom: PhantomData<T>,
}

impl<T> Cybersource<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Cybersource<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Cybersource<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let body = parse_cybersource_webhook_body(&request)?;

        EventType::try_from(body.event_type)
    }

    /// Payment and refund events carry the PSP request id in `payload.data.id`. When the event
    /// data also echoes `clientReferenceInformation.code`, prefer that caller-assigned id during
    /// lookup; it is especially important for capture events, whose PSP id differs from the
    /// authorization id. The merchant reference remains optional because the payments payload
    /// schema is not published and live events are message-level encrypted.
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let body = parse_cybersource_webhook_body(&request)?;
        let merchant_reference_id = body.merchant_reference_id();

        if body.event_type.is_payment_event() {
            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(body.object_reference_id()?),
                    merchant_transaction_id: merchant_reference_id,
                    connector_preprocessing_id: None,
                },
            )))
        } else if body.event_type.refund_status().is_some() {
            Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: Some(body.object_reference_id()?),
                    merchant_refund_id: merchant_reference_id,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                },
            )))
        } else {
            Err(Report::new(WebhookError::WebhookEventTypeNotFound))
                .attach_printable("Cybersource webhook: unsupported eventType")
        }
    }
}
