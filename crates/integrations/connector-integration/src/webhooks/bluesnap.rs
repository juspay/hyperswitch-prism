pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::errors::CustomResult;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use crate::connectors::bluesnap::transformers::{
    BluesnapDisputeWebhookBody, BluesnapWebhookBody, BluesnapWebhookEvent,
};

#[derive(Debug, Clone)]
pub struct Bluesnap<T> {
    _phantom: PhantomData<T>,
}

impl<T> Bluesnap<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Bluesnap<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Bluesnap<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(&self, request: RequestDetails) -> CustomResult<EventType, WebhookError> {
        match serde_urlencoded::from_bytes::<BluesnapWebhookBody>(&request.body) {
            Ok(webhook_body) => match webhook_body.transaction_type {
                BluesnapWebhookEvent::Chargeback
                | BluesnapWebhookEvent::ChargebackStatusChanged => {
                    let dispute_body: BluesnapDisputeWebhookBody =
                        serde_urlencoded::from_bytes(&request.body)
                            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                    transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
                }
                _ => Ok(transformers::map_webhook_event_to_incoming_webhook_event(
                    &webhook_body.transaction_type,
                )),
            },
            Err(_) => {
                let dispute_body: BluesnapDisputeWebhookBody =
                    serde_urlencoded::from_bytes(&request.body)
                        .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                transformers::map_chargeback_status_to_event_type(&dispute_body.cb_status)
            }
        }
    }
}
