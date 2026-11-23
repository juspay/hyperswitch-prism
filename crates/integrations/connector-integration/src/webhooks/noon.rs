pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{get_noon_webhook_event_type, NoonWebhookEvent};
use crate::connectors::noon::transformers::{NoonWebhookEventTypes, NoonWebhookObject};
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
pub struct Noon<T> {
    _phantom: PhantomData<T>,
}

impl<T> Noon<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Noon<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Noon<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Noon checks the endpoint with an empty body.
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }
        let details: NoonWebhookEvent = request
            .body
            .parse_struct("NoonWebhookEvent")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse webhook event type from Noon webhook body")?;

        Ok(get_noon_webhook_event_type(
            &details.event_type,
            &details.order_status,
        ))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let webhook_object: NoonWebhookObject = request
            .body
            .parse_struct("NoonWebhookObject")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable("Failed to parse NoonWebhookObject for reference extraction")?;

        // Noon's order_id is the connector transaction ID and the only identifier covered by
        // the v1 signature. A refund event also echoes the refund's merchant reference
        // (merchantTxnRef), but that field is NOT in the signed preimage, so it can be edited
        // on a validly signed body and is never used to pick a refund. Noon documents no
        // chargeback / dispute event, so no dispute reference is produced.
        let connector_transaction_id = Some(webhook_object.order_id.to_string());
        match webhook_object.event_type {
            NoonWebhookEventTypes::Refund => Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: None,
                    merchant_refund_id: None,
                    connector_transaction_id,
                    merchant_transaction_id: None,
                },
            ))),
            NoonWebhookEventTypes::ThreeDs
            | NoonWebhookEventTypes::Authenticate
            | NoonWebhookEventTypes::Authorize
            | NoonWebhookEventTypes::Cancel
            | NoonWebhookEventTypes::Capture
            | NoonWebhookEventTypes::Fail
            | NoonWebhookEventTypes::Lock
            | NoonWebhookEventTypes::Reject
            | NoonWebhookEventTypes::Reverse
            | NoonWebhookEventTypes::Review
            | NoonWebhookEventTypes::Sale
            | NoonWebhookEventTypes::Unknown => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id,
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                },
            ))),
        }
    }
}
