use crate::connectors::stripe::transformers::{
    WebhookEvent, WebhookEventObjectType, WebhookEventStatus, WebhookEventType,
};
use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        WebhookResourceReference,
    },
    errors::WebhookError,
};
use serde::Deserialize;
use std::fmt::Debug;

#[derive(Debug, Deserialize)]
pub struct WebhookEventTypeBody {
    #[serde(rename = "type")]
    pub event_type: WebhookEventType,
    #[serde(rename = "data")]
    pub event_data: WebhookStatusData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookStatusData {
    #[serde(rename = "object")]
    pub event_object: WebhookStatusObjectData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookStatusObjectData {
    pub status: Option<WebhookEventStatus>,
    pub payment_method_details: Option<WebhookPaymentMethodDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookPaymentMethodType {
    AchCreditTransfer,
    MultibancoBankTransfers,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPaymentMethodDetails {
    #[serde(rename = "type")]
    pub payment_method: WebhookPaymentMethodType,
}

/// Map a decoded Stripe webhook event (type + object status) to the prism `EventType`.
///
/// Ports `get_event_type` verbatim. Operates on the lighter `WebhookEventTypeBody`, which
/// carries `payment_method_details` (required for the `charge.succeeded` ACH/Multibanco
/// sub-mapping); the full `WebhookEvent` object does not expose that field.
pub(crate) fn map_webhook_event_type(details: &WebhookEventTypeBody) -> EventType {
    let WebhookStatusObjectData {
        status,
        payment_method_details,
    } = &details.event_data.event_object;

    match details.event_type {
        WebhookEventType::PaymentIntentFailed => EventType::PaymentIntentFailure,
        WebhookEventType::PaymentIntentSucceed => EventType::PaymentIntentSuccess,
        WebhookEventType::PaymentIntentCanceled => EventType::PaymentIntentCancelled,
        WebhookEventType::PaymentIntentAmountCapturableUpdated => {
            EventType::PaymentIntentAuthorizationSuccess
        }
        WebhookEventType::ChargeSucceeded => match payment_method_details {
            Some(WebhookPaymentMethodDetails {
                payment_method:
                    WebhookPaymentMethodType::AchCreditTransfer
                    | WebhookPaymentMethodType::MultibancoBankTransfers,
            }) => EventType::PaymentIntentSuccess,
            _ => EventType::IncomingWebhookEventUnspecified,
        },
        // `refund.created`, `refund.updated` (current), and `charge.refund.updated` (deprecated)
        // carry a Refund object; its `status` decides the event
        // (https://docs.stripe.com/refunds, https://docs.stripe.com/api/refunds/object).
        WebhookEventType::RefundCreated
        | WebhookEventType::RefundUpdated
        | WebhookEventType::ChargeRefundUpdated => {
            match status.as_ref() {
                Some(WebhookEventStatus::Succeeded) => EventType::RefundSuccess,
                Some(WebhookEventStatus::Failed | WebhookEventStatus::Canceled) => {
                    EventType::RefundFailure
                }
                // `pending` / `requires_action` (decoded as `Unknown` / `RequiresAction`), any
                // status that is not a refund status, or no status: nothing terminal to report.
                Some(
                    WebhookEventStatus::WarningNeedsResponse
                    | WebhookEventStatus::WarningClosed
                    | WebhookEventStatus::WarningUnderReview
                    | WebhookEventStatus::Won
                    | WebhookEventStatus::Lost
                    | WebhookEventStatus::NeedsResponse
                    | WebhookEventStatus::UnderReview
                    | WebhookEventStatus::ChargeRefunded
                    | WebhookEventStatus::RequiresPaymentMethod
                    | WebhookEventStatus::RequiresConfirmation
                    | WebhookEventStatus::RequiresAction
                    | WebhookEventStatus::Processing
                    | WebhookEventStatus::RequiresCapture
                    | WebhookEventStatus::Chargeable
                    | WebhookEventStatus::Prevented
                    | WebhookEventStatus::Unknown,
                )
                | None => EventType::IncomingWebhookEventUnspecified,
            }
        }
        // "Sent when a refund has failed."
        WebhookEventType::RefundFailed => EventType::RefundFailure,
        WebhookEventType::SourceChargeable => EventType::SourceChargeable,
        // Dispute events: map the dispute object's `status` directly to the event; when the
        // status is absent, fall back to a per-event-code default.
        code @ (WebhookEventType::DisputeCreated
        | WebhookEventType::DisputeUpdated
        | WebhookEventType::DisputeClosed
        | WebhookEventType::ChargeDisputeFundsWithdrawn
        | WebhookEventType::ChargeDisputeFundsReinstated) => match status.as_ref() {
            Some(WebhookEventStatus::WarningNeedsResponse | WebhookEventStatus::NeedsResponse) => {
                EventType::DisputeOpened
            }
            Some(WebhookEventStatus::WarningClosed) => EventType::DisputeCancelled,
            Some(WebhookEventStatus::WarningUnderReview | WebhookEventStatus::UnderReview) => {
                EventType::DisputeChallenged
            }
            Some(WebhookEventStatus::Won) => EventType::DisputeWon,
            Some(WebhookEventStatus::Lost) => EventType::DisputeLost,
            // `prevented` is a documented dispute status with no lifecycle meaning and no
            // `DisputeStatus` equivalent: report it as unspecified, never as an opened dispute.
            Some(WebhookEventStatus::Prevented) => EventType::IncomingWebhookEventUnspecified,
            // A non-dispute or unrecognised status on a dispute event is not actionable.
            Some(
                WebhookEventStatus::ChargeRefunded
                | WebhookEventStatus::Succeeded
                | WebhookEventStatus::RequiresPaymentMethod
                | WebhookEventStatus::RequiresConfirmation
                | WebhookEventStatus::RequiresAction
                | WebhookEventStatus::Processing
                | WebhookEventStatus::RequiresCapture
                | WebhookEventStatus::Canceled
                | WebhookEventStatus::Chargeable
                | WebhookEventStatus::Failed
                | WebhookEventStatus::Unknown,
            ) => EventType::IncomingWebhookEventUnspecified,
            None => match code {
                WebhookEventType::DisputeClosed => EventType::DisputeCancelled,
                WebhookEventType::ChargeDisputeFundsWithdrawn => EventType::DisputeLost,
                WebhookEventType::ChargeDisputeFundsReinstated => EventType::DisputeWon,
                WebhookEventType::DisputeUpdated => EventType::IncomingWebhookEventUnspecified,
                _ => EventType::DisputeOpened,
            },
        },
        WebhookEventType::PaymentIntentPartiallyFunded => EventType::PaymentIntentPartiallyFunded,
        WebhookEventType::PaymentIntentRequiresAction => EventType::PaymentActionRequired,
        WebhookEventType::Unknown
        | WebhookEventType::ChargeCaptured
        | WebhookEventType::ChargeExpired
        | WebhookEventType::ChargeFailed
        | WebhookEventType::ChargePending
        | WebhookEventType::ChargeUpdated
        // `charge.refunded` carries a Charge, not a Refund.
        | WebhookEventType::ChargeRefunded
        | WebhookEventType::PaymentIntentCreated
        | WebhookEventType::PaymentIntentProcessing
        | WebhookEventType::SourceTransactionCreated => EventType::IncomingWebhookEventUnspecified,
    }
}

/// Build the typed resource reference for a decoded Stripe webhook event.
///
/// Preserves the exact either/or selection that was fixed for HS shadow parity.
pub(crate) fn get_webhook_reference(
    event: &WebhookEvent,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    let event_object = &event.event_data.event_object;
    let order_id = event_object
        .metadata
        .as_ref()
        .and_then(|meta_data| meta_data.order_id.clone());

    let reference = match event_object.object {
        WebhookEventObjectType::PaymentIntent => {
            // Expose every identifier present in the payload; HS owns lookup precedence.
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: Some(event_object.id.clone()),
                merchant_transaction_id: order_id,
                connector_preprocessing_id: None,
            })
        }
        WebhookEventObjectType::Charge => {
            // Expose the linked PaymentIntent and any echoed merchant reference.
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: event_object.payment_intent.clone(),
                merchant_transaction_id: order_id,
                connector_preprocessing_id: None,
            })
        }
        WebhookEventObjectType::Dispute => {
            // HS maps a dispute to its PARENT payment:
            // PaymentId(ConnectorTransactionId(payment_intent)). The shadow normaliser prefers
            // connector_dispute_id, so leave it None and surface the parent payment_intent as
            // connector_transaction_id to match HS byte-for-byte.
            WebhookResourceReference::Dispute(DisputeWebhookReference {
                connector_dispute_id: None,
                connector_transaction_id: event_object.payment_intent.clone(),
            })
        }
        WebhookEventObjectType::Source => {
            // Source IDs are preprocessing references, not payment transaction IDs.
            WebhookResourceReference::Payment(PaymentWebhookReference {
                connector_transaction_id: None,
                merchant_transaction_id: None,
                connector_preprocessing_id: Some(event_object.id.clone()),
            })
        }
        WebhookEventObjectType::Refund => {
            let is_refund_id_as_reference = event_object
                .metadata
                .as_ref()
                .and_then(|meta_data| meta_data.is_refund_id_as_reference.clone());
            // Ports HS issue-2076 logic: a refund-id reference becomes the merchant refund
            // id, otherwise the object id is the connector refund id.
            let (merchant_refund_id, connector_refund_id) =
                match (order_id, is_refund_id_as_reference) {
                    (Some(order_id), Some(_)) => (Some(order_id), None),
                    _ => (None, Some(event_object.id.clone())),
                };
            WebhookResourceReference::Refund(RefundWebhookReference {
                connector_refund_id,
                merchant_refund_id,
                connector_transaction_id: event_object.payment_intent.clone(),
                merchant_transaction_id: None,
            })
        }
    };

    Ok(Some(reference))
}
