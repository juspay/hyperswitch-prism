use common_utils::{errors::CustomResult, ext_traits::XmlExt};
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, PayoutWebhookReference,
        RefundWebhookReference, WebhookResourceReference,
    },
    errors::WebhookError,
};
use error_stack::ResultExt;

use crate::connectors::worldpayxml::responses::{WorldpayxmlLastEvent, WorldpayxmlWebhookBody};

pub(crate) fn parse_webhook_body(
    body: &[u8],
) -> CustomResult<WorldpayxmlWebhookBody, WebhookError> {
    std::str::from_utf8(body)
        .change_context(WebhookError::WebhookBodyDecodingFailed)?
        .parse_xml()
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}

impl WorldpayxmlWebhookBody {
    pub fn event_type(&self) -> EventType {
        match self.notify.order_status_event.payment.last_event {
            WorldpayxmlLastEvent::Authorised | WorldpayxmlLastEvent::SentForAuthorisation => {
                EventType::PaymentIntentProcessing
            }
            WorldpayxmlLastEvent::Captured | WorldpayxmlLastEvent::Settled => {
                EventType::PaymentIntentSuccess
            }
            WorldpayxmlLastEvent::Cancelled => EventType::PaymentIntentCancelled,
            WorldpayxmlLastEvent::Refused => EventType::PaymentIntentFailure,
            WorldpayxmlLastEvent::Refunded | WorldpayxmlLastEvent::RefundedByMerchant => {
                EventType::RefundSuccess
            }
            WorldpayxmlLastEvent::RefundFailed => EventType::RefundFailure,
            WorldpayxmlLastEvent::RefundRequested
            | WorldpayxmlLastEvent::SentForRefund
            | WorldpayxmlLastEvent::SentForFastRefund => EventType::RefundProcessing,
            WorldpayxmlLastEvent::ChargedBack => EventType::DisputeOpened,
            WorldpayxmlLastEvent::ChargebackReversed => EventType::DisputeWon,
            WorldpayxmlLastEvent::PushRequested | WorldpayxmlLastEvent::PushPending => {
                EventType::PayoutProcessing
            }
            WorldpayxmlLastEvent::PushApproved | WorldpayxmlLastEvent::SettledByMerchant => {
                EventType::PayoutSuccess
            }
            WorldpayxmlLastEvent::PushRefused => EventType::PayoutFailure,
            WorldpayxmlLastEvent::Expired
            | WorldpayxmlLastEvent::Error
            | WorldpayxmlLastEvent::QueryRequired
            | WorldpayxmlLastEvent::CancelReceived
            | WorldpayxmlLastEvent::RefundReceived
            | WorldpayxmlLastEvent::Unknown => EventType::IncomingWebhookEventUnspecified,
        }
    }

    pub fn event_reference(&self) -> CustomResult<Option<WebhookResourceReference>, WebhookError> {
        let event = &self.notify.order_status_event;
        let order_code = Some(event.order_code.clone());
        let reference = match event.payment.last_event {
            WorldpayxmlLastEvent::Authorised
            | WorldpayxmlLastEvent::Settled
            | WorldpayxmlLastEvent::Captured
            | WorldpayxmlLastEvent::SentForAuthorisation
            | WorldpayxmlLastEvent::Cancelled
            | WorldpayxmlLastEvent::Refused => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: order_code,
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                })
            }
            WorldpayxmlLastEvent::SentForRefund
            | WorldpayxmlLastEvent::SentForFastRefund
            | WorldpayxmlLastEvent::RefundedByMerchant
            | WorldpayxmlLastEvent::RefundRequested
            | WorldpayxmlLastEvent::Refunded
            | WorldpayxmlLastEvent::RefundFailed => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: order_code,
                    merchant_refund_id: None,
                    connector_transaction_id: None,
                    merchant_transaction_id: None,
                })
            }
            WorldpayxmlLastEvent::ChargedBack | WorldpayxmlLastEvent::ChargebackReversed => {
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: None,
                    connector_transaction_id: order_code,
                })
            }
            WorldpayxmlLastEvent::PushApproved
            | WorldpayxmlLastEvent::PushPending
            | WorldpayxmlLastEvent::PushRequested
            | WorldpayxmlLastEvent::SettledByMerchant
            | WorldpayxmlLastEvent::PushRefused => {
                WebhookResourceReference::Payout(PayoutWebhookReference {
                    connector_payout_id: order_code,
                    merchant_payout_id: None,
                })
            }
            WorldpayxmlLastEvent::Expired
            | WorldpayxmlLastEvent::Error
            | WorldpayxmlLastEvent::QueryRequired
            | WorldpayxmlLastEvent::CancelReceived
            | WorldpayxmlLastEvent::RefundReceived
            | WorldpayxmlLastEvent::Unknown => {
                return Err(WebhookError::WebhookReferenceIdNotFound.into());
            }
        };
        Ok(Some(reference))
    }
}
