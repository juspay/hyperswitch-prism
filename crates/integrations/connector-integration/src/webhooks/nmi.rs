pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{get_nmi_webhook_event, NmiWebhookEventBody, NmiWebhookObjectReference};
use crate::connectors::nmi::transformers::NmiActionType;
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
pub struct Nmi<T> {
    _phantom: PhantomData<T>,
}

impl<T> Nmi<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Nmi<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Nmi<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let event_type_body: NmiWebhookEventBody = serde_json::from_slice(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to decode the NMI webhook event body")?;

        Ok(get_nmi_webhook_event(event_type_body.event_type))
    }

    /// Ports HS `get_webhook_object_reference_id`: NMI echoes back the hyperswitch-side
    /// identifier in `event_body.order_id` — the payment attempt id for payment actions
    /// (`PaymentIdType::PaymentAttemptId`) and the refund id for refunds
    /// (`RefundIdType::RefundId`). Both are caller-assigned (merchant) references, so the
    /// connector-assigned id fields MUST stay `None` for the reference to normalise
    /// identically to the Direct gateway.
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let reference_body: NmiWebhookObjectReference = serde_json::from_slice(&request.body)
            .change_context(WebhookError::WebhookResourceObjectNotFound)
            .attach_printable("Failed to decode the NMI webhook object reference")?;

        match reference_body.event_body.action.action_type {
            NmiActionType::Sale
            | NmiActionType::Auth
            | NmiActionType::Capture
            | NmiActionType::Void => Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: None,
                    merchant_transaction_id: Some(reference_body.event_body.order_id),
                    connector_preprocessing_id: None,
                },
            ))),
            NmiActionType::Refund => Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: None,
                    merchant_refund_id: Some(reference_body.event_body.order_id),
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                },
            ))),
            // HS maps `credit` to `WebhooksNotImplemented`.
            NmiActionType::Credit => {
                Err(error_stack::report!(WebhookError::WebhooksNotImplemented {
                    operation: "nmi credit webhooks",
                }))
            }
        }
    }
}
