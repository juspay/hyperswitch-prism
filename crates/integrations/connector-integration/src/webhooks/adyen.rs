pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use domain_types::{
    connector_types::{
        DisputeWebhookReference, EventType, PaymentWebhookReference, RefundWebhookReference,
        RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::report;
use interfaces::webhooks::WebHookParseEvent;
use serde::Serialize;

use self::transformers::AdyenNotificationRequestItemWH;

#[derive(Debug, Clone)]
pub struct Adyen<T> {
    _phantom: PhantomData<T>,
}

impl<T> Adyen<T> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Adyen<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> WebHookParseEvent
    for Adyen<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }
        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;
        transformers::get_adyen_webhook_event_type(
            notif.event_code,
            notif.success,
            notif.additional_data.dispute_status,
        )
        .map_err(|e| report!(e))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        use transformers::WebhookEventCode;

        let notif: AdyenNotificationRequestItemWH =
            transformers::get_webhook_object_from_body(request.body).map_err(|err| {
                report!(WebhookError::WebhookBodyDecodingFailed)
                    .attach_printable(format!("error while decoding webhook body {err}"))
            })?;

        let reference = match notif.event_code {
            // Capture/cancellation/adjustment events: psp_reference is the event's own PSP ref;
            // original_reference is the parent authorisation's PSP ref — that's the lookup key.
            WebhookEventCode::Capture
            | WebhookEventCode::CaptureFailed
            | WebhookEventCode::Cancellation
            | WebhookEventCode::AuthorisationAdjustment => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: notif.original_reference,
                    merchant_transaction_id: Some(notif.merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            // AUTHORISATION-like events carry both the Adyen PSP reference and
            // the merchant reference. Return both so Prism remains a transparent
            // parse layer; HS is responsible for choosing the lookup key.
            WebhookEventCode::Authorisation
            | WebhookEventCode::OfferClosed
            | WebhookEventCode::RecurringContract => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(notif.psp_reference),
                    merchant_transaction_id: Some(notif.merchant_reference),
                    connector_preprocessing_id: None,
                })
            }
            // Refund events carry the refund PSP reference, merchant refund
            // reference, and parent payment PSP reference. Return all of them.
            WebhookEventCode::Refund
            | WebhookEventCode::CancelOrRefund
            | WebhookEventCode::RefundFailed
            | WebhookEventCode::RefundReversed => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(notif.psp_reference),
                    merchant_refund_id: Some(notif.merchant_reference),
                    connector_transaction_id: notif.original_reference,
                    merchant_transaction_id: None,
                })
            }
            // Dispute events carry the dispute PSP reference and the parent
            // payment PSP reference (originalReference). Return both so Prism
            // stays a transparent parse layer; the HS bridge prefers the
            // parent payment reference (connector_transaction_id) for the
            // payment lookup, matching the Direct path.
            WebhookEventCode::NotificationOfChargeback
            | WebhookEventCode::Chargeback
            | WebhookEventCode::ChargebackReversed
            | WebhookEventCode::PrearbitrationWon
            | WebhookEventCode::SecondChargeback
            | WebhookEventCode::PrearbitrationLost
            | WebhookEventCode::RequestForInformation
            | WebhookEventCode::InformationSupplied
            | WebhookEventCode::PrearbitrationOpen
            | WebhookEventCode::PrearbitrationAccepted
            | WebhookEventCode::PrearbitrationDeclined
            | WebhookEventCode::PrearbitrationIssuerWithdrawn
            | WebhookEventCode::SchemeArbitration
            | WebhookEventCode::SchemeArbitrationWon
            | WebhookEventCode::SchemeArbitrationLost
            | WebhookEventCode::DisputeDefensePeriodEnded
            | WebhookEventCode::IssuerResponseTimeframeExpired => {
                WebhookResourceReference::Dispute(DisputeWebhookReference {
                    connector_dispute_id: Some(notif.psp_reference),
                    connector_transaction_id: notif.original_reference,
                })
            }
            WebhookEventCode::NotificationOfFraud | WebhookEventCode::IssuerComments => {
                return Ok(None);
            }
            // Unknown: no actionable reference.
            WebhookEventCode::Unknown => return Ok(None),
        };

        Ok(Some(reference))
    }
}
