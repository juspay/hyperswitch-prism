pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::get_incoming_webhook_event;
use crate::connectors::novalnet::{get_webhook_object_from_body, transformers::WebhookEventType};
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Novalnet<T> {
    _phantom: PhantomData<T>,
}

impl<T> Novalnet<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Novalnet<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Novalnet<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif = get_webhook_object_from_body(&request.body)?;

        // Classified from the signed checksum preimage (event.type, result.status).
        // transaction.status is outside it, so it is passed under the lower-only rule: it can
        // narrow PAYMENT/SUCCESS to authorization-success or processing, never widen an event.
        let transaction_status = notif.transaction.transaction_status();
        Ok(get_incoming_webhook_event(
            notif.event.event_type,
            notif.result.status,
            transaction_status,
        ))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let notif = get_webhook_object_from_body(&request.body)?;
        // `event.tid` / `event.type` are inside the Novalnet checksum preimage
        // (spec: ### Webhook Authentication & Signature Verification) and decide the event class.
        // `event.parent_tid` (spec: ### IncomingWebhook, source_24: parentTid = event.parent_tid
        // ?: event.tid) is outside that preimage; it is used only as the lookup key that locates
        // the original payment of a dispute-class event, never to classify or to derive a status.
        let tid = notif.event.tid.to_string();

        let reference = match notif.event.event_type {
            // spec: ### Webhook Payload Structure (TRANSACTION_REFUND -> refund id = event.tid)
            WebhookEventType::TransactionRefund => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(tid),
                    merchant_refund_id: None,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                })
            }
            // spec: ### IncomingWebhook (dispute id = event.tid; original payment =
            // event.parent_tid ?: event.tid)
            WebhookEventType::Chargeback
            | WebhookEventType::ReturnDebit
            | WebhookEventType::Reversal
            | WebhookEventType::Credit => {
                let original_payment_tid = notif
                    .event
                    .parent_tid
                    .map(|parent_tid| parent_tid.to_string())
                    .unwrap_or_else(|| tid.clone());
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(tid),
                    connector_transaction_id: Some(original_payment_tid),
                })
            }
            // spec: ### Webhook Payload Structure (otherwise payment by event.tid)
            WebhookEventType::Payment
            | WebhookEventType::TransactionCapture
            | WebhookEventType::TransactionCancel
            | WebhookEventType::Unknown => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(tid),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                })
            }
        };

        Ok(Some(reference))
    }
}
