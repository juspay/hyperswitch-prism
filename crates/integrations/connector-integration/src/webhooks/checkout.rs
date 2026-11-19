pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{is_chargeback_event, is_refund_event, CheckoutWebhookEventTypeBody};
use crate::connectors::checkout::transformers::{CheckoutDisputeWebhookBody, CheckoutWebhookBody};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Checkout<T> {
    _phantom: PhantomData<T>,
}

impl<T> Checkout<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Checkout<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Checkout<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }

        let body: CheckoutWebhookEventTypeBody = request
            .body
            .parse_struct("CheckoutWebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(body.transaction_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        if request.body.is_empty() {
            return Ok(None);
        }

        // Peek at only the event type first to branch on the body shape.
        let type_body: CheckoutWebhookEventTypeBody = request
            .body
            .parse_struct("CheckoutWebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        if is_chargeback_event(&type_body.transaction_type) {
            let body: CheckoutDisputeWebhookBody = request
                .body
                .parse_struct("CheckoutDisputeWebhookBody")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

            return Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: Some(body.data.id),
                    connector_transaction_id: body.data.payment_id,
                },
            )));
        }

        let body: CheckoutWebhookBody = request
            .body
            .parse_struct("CheckoutWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        if is_refund_event(&type_body.transaction_type) {
            return Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    // Checkout uses action_id as the refund identifier
                    connector_refund_id: body.data.action_id,
                    // The `reference` field echoes the merchant-assigned refund ID
                    merchant_refund_id: body.data.reference,
                    connector_transaction_id: body.data.payment_id,
                    merchant_transaction_id: None,
                },
            )));
        }

        // Payment event — the canonical payment ID is `payment_id`; `id` may be an action ID
        let connector_transaction_id = body.data.payment_id.or(Some(body.data.id));
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id,
                merchant_transaction_id: body.data.reference,
                connector_preprocessing_id: None,
            },
        )))
    }
}
