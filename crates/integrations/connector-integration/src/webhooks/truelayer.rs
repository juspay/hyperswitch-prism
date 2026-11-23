pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::get_webhook_event;
use crate::connectors::truelayer::transformers::{
    TruelayerPayoutWebhookBody, TruelayerWebhookBody, TruelayerWebhookEventType,
    TruelayerWebhookEventTypeBody,
};
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, PayoutWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Truelayer<T> {
    _phantom: PhantomData<T>,
}

impl<T> Truelayer<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Truelayer<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Truelayer<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let webhook_body: TruelayerWebhookEventTypeBody = request
            .body
            .parse_struct("TruelayerPayoutsWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(get_webhook_event(webhook_body._type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let event_type_body: TruelayerWebhookEventTypeBody = request
            .body
            .parse_struct("TruelayerWebhookEventTypeBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        // Payout webhooks carry `payout_id` and no `payment_id`, so each arm
        // deserializes the body shape that matches its event family.
        match event_type_body._type {
            TruelayerWebhookEventType::PayoutExecuted | TruelayerWebhookEventType::PayoutFailed => {
                let payout_body: TruelayerPayoutWebhookBody = request
                    .body
                    .parse_struct("TruelayerPayoutWebhookBody")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                Ok(Some(WebhookResourceReference::Payout(
                    PayoutWebhookReference {
                        connector_payout_id: Some(payout_body.payout_id),
                        merchant_payout_id: None,
                    },
                )))
            }
            TruelayerWebhookEventType::PaymentAuthorized
            | TruelayerWebhookEventType::PaymentExecuted
            | TruelayerWebhookEventType::PaymentReversed
            | TruelayerWebhookEventType::PaymentFailed
            | TruelayerWebhookEventType::PaymentSettlementStalled
            | TruelayerWebhookEventType::PaymentSettled
            | TruelayerWebhookEventType::PaymentCreditable
            | TruelayerWebhookEventType::PaymentFundsReceived => {
                let webhook_body: TruelayerWebhookBody = request
                    .body
                    .parse_struct("TruelayerWebhookBody")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(webhook_body.payment_id),
                        merchant_transaction_id: None,
                        connector_preprocessing_id: None,
                    },
                )))
            }
            TruelayerWebhookEventType::RefundExecuted | TruelayerWebhookEventType::RefundFailed => {
                let webhook_body: TruelayerWebhookBody = request
                    .body
                    .parse_struct("TruelayerWebhookBody")
                    .change_context(WebhookError::WebhookBodyDecodingFailed)?;

                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: webhook_body.refund_id,
                        merchant_refund_id: None,
                        connector_transaction_id: Some(webhook_body.payment_id),
                        merchant_transaction_id: None,
                    },
                )))
            }
            TruelayerWebhookEventType::PaymentDisputed | TruelayerWebhookEventType::Unknown => {
                Ok(None)
            }
        }
    }
}
