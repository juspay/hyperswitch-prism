pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{dispute_event_from_status, is_payout_dmn, map_notification_to_event};
use crate::connectors::nuvei::transformers::{
    get_dispute_status, parse_nuvei_webhook, NuveiTransactionType, NuveiWebhook,
};
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::Report;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Nuvei<T> {
    _phantom: PhantomData<T>,
}

impl<T> Nuvei<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Nuvei<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Nuvei<T>
{
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        match parse_nuvei_webhook(&request.body)? {
            NuveiWebhook::PaymentDmn(notification) => {
                if is_payout_dmn(&notification) || notification.has_conflicting_status() {
                    return Err(WebhookError::WebhookEventTypeNotFound.into());
                }
                match (
                    notification.status.as_ref(),
                    notification.transaction_type.as_ref(),
                ) {
                    (Some(status), Some(transaction_type)) => {
                        map_notification_to_event(status, transaction_type)
                    }
                    (None, _) | (_, None) => Err(WebhookError::WebhookEventTypeNotFound.into()),
                }
            }
            NuveiWebhook::Chargeback(notification) => {
                get_dispute_status(&notification.chargeback).map(dispute_event_from_status)
            }
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        match parse_nuvei_webhook(&request.body)? {
            NuveiWebhook::PaymentDmn(notification) => {
                if is_payout_dmn(&notification) {
                    return Err(WebhookError::WebhookEventTypeNotFound.into());
                }
                let payment_reference = notification.transaction_id.clone();
                match notification.transaction_type {
                    Some(NuveiTransactionType::Auth)
                    | Some(NuveiTransactionType::Sale)
                    | Some(NuveiTransactionType::Settle)
                    | Some(NuveiTransactionType::Void) => Ok(Some(
                        WebhookResourceReference::Payment(PaymentWebhookReference {
                            connector_transaction_id: Some(
                                payment_reference
                                    .ok_or(WebhookError::WebhookReferenceIdNotFound)?,
                            ),
                            merchant_transaction_id: notification.merchant_reference(),
                            connector_preprocessing_id: None,
                        }),
                    )),
                    Some(NuveiTransactionType::Credit) => Ok(Some(
                        WebhookResourceReference::Refund(RefundWebhookReference {
                            connector_refund_id: Some(
                                notification
                                    .transaction_id
                                    .ok_or(WebhookError::WebhookReferenceIdNotFound)?,
                            ),
                            merchant_refund_id: None,
                            connector_transaction_id: notification.related_transaction_id,
                            merchant_transaction_id: None,
                        }),
                    )),
                    Some(NuveiTransactionType::Auth3D)
                    | Some(NuveiTransactionType::InitAuth3D)
                    | Some(NuveiTransactionType::Unknown)
                    | None => Err(WebhookError::WebhookEventTypeNotFound.into()),
                }
            }
            NuveiWebhook::Chargeback(notification) => Ok(Some(WebhookResourceReference::Dispute(
                DisputeWebhookReference {
                    connector_dispute_id: notification.chargeback.dispute_id,
                    connector_transaction_id: Some(
                        notification.transaction_details.transaction_id.to_string(),
                    ),
                },
            ))),
        }
    }
}
