pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{
    AuthorizedotnetIncomingWebhookEventType, AuthorizedotnetWebhookEventType,
};
use crate::connectors::authorizedotnet::transformers::{
    AuthorizedotnetWebhookEvent, AuthorizedotnetWebhookObjectId,
};
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
pub struct Authorizedotnet<T> {
    _phantom: PhantomData<T>,
}

impl<T> Authorizedotnet<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Authorizedotnet<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Authorizedotnet<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // An empty body is the endpoint check, not an event (techspec "IncomingWebhook",
        // "Expected result (empty body)").
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }

        let webhook_body: AuthorizedotnetWebhookEventType = request
            .body
            .parse_struct("AuthorizedotnetWebhookEventType")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse webhook event type from Authorize.Net webhook body"
            })?;

        let event_type = match webhook_body.event_type {
            AuthorizedotnetIncomingWebhookEventType::AuthorizationCreated => {
                EventType::PaymentIntentAuthorizationSuccess
            }
            AuthorizedotnetIncomingWebhookEventType::PriorAuthCapture
            | AuthorizedotnetIncomingWebhookEventType::CaptureCreated => {
                EventType::PaymentIntentCaptureSuccess
            }
            AuthorizedotnetIncomingWebhookEventType::AuthCapCreated => {
                EventType::PaymentIntentSuccess // Combined auth+capture
            }
            AuthorizedotnetIncomingWebhookEventType::VoidCreated => {
                EventType::PaymentIntentCancelled
            }
            AuthorizedotnetIncomingWebhookEventType::RefundCreated => EventType::RefundSuccess,
            AuthorizedotnetIncomingWebhookEventType::CustomerCreated
            | AuthorizedotnetIncomingWebhookEventType::CustomerPaymentProfileCreated => {
                EventType::MandateActive
            }
            // Authorize.Net delivers many event types this connector does not act on
            // (fraud, subscription, settlement, ...). They are acknowledged as
            // not-supported, as hyperswitch does, instead of failing the delivery.
            AuthorizedotnetIncomingWebhookEventType::Unknown => {
                EventType::IncomingWebhookEventUnspecified
            }
        };
        Ok(event_type)
    }

    /// Reference for `EventService.ParseEvent` (stateless: no secret, no verification).
    /// `payload.id` is the transId of the transaction the event is about - for
    /// `refund.created` that is the refund transaction itself - and
    /// `payload.merchantReferenceId` echoes the `refId` of the request
    /// (techspec "IncomingWebhook").
    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // Endpoint verification carries no resource.
        if request.body.is_empty() {
            return Ok(None);
        }

        let webhook_body: AuthorizedotnetWebhookObjectId = request
            .body
            .parse_struct("AuthorizedotnetWebhookObjectId")
            .change_context(WebhookError::WebhookBodyDecodingFailed)
            .attach_printable_lazy(|| {
                "Failed to parse Authorize.Net webhook body for the event reference"
            })?;

        match webhook_body.event_type {
            AuthorizedotnetWebhookEvent::RefundCreated => {
                let connector_refund_id = webhook_body
                    .payload
                    .id
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)
                    .attach_printable("payload.id missing in refund.created webhook")?;
                // The payload has no link from the refund to its original payment.
                Ok(Some(WebhookResourceReference::Refund(
                    RefundWebhookReference {
                        connector_refund_id: Some(connector_refund_id),
                        merchant_refund_id: None,
                        connector_transaction_id: None,
                        merchant_transaction_id: None,
                    },
                )))
            }
            AuthorizedotnetWebhookEvent::AuthorizationCreated
            | AuthorizedotnetWebhookEvent::PriorAuthCapture
            | AuthorizedotnetWebhookEvent::AuthCapCreated
            | AuthorizedotnetWebhookEvent::CaptureCreated
            | AuthorizedotnetWebhookEvent::VoidCreated => {
                // `priorAuthCapture` and `voidTransaction` act on the existing
                // transaction rather than minting a new one: a single transId carries
                // the whole lifecycle (`authorizedPendingCapture` ->
                // `capturedPendingSettlement` -> `settledSuccessfully`, or `voided` -
                // which is what `SyncStatus` models). So `payload.id` here
                // is the same transId the Authorize response stored, and a lookup by
                // `connector_transaction_id` resolves. `refund.created`, handled above,
                // is the one event that mints a new transId, and the payload carries no
                // link from it back to the payment.
                let connector_transaction_id = webhook_body
                    .payload
                    .id
                    .ok_or(WebhookError::WebhookReferenceIdNotFound)
                    .attach_printable("payload.id missing in payment webhook")?;
                Ok(Some(WebhookResourceReference::Payment(
                    PaymentWebhookReference {
                        connector_transaction_id: Some(connector_transaction_id),
                        merchant_transaction_id: webhook_body.payload.merchant_reference_id,
                        connector_preprocessing_id: None,
                    },
                )))
            }
            // Customer profile events identify a profile, not a payment or a refund.
            AuthorizedotnetWebhookEvent::CustomerCreated
            | AuthorizedotnetWebhookEvent::CustomerPaymentProfileCreated => Ok(None),
            // An event type this connector does not act on has no resource to resolve.
            AuthorizedotnetWebhookEvent::Unknown => Ok(None),
        }
    }
}
