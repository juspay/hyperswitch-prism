use domain_types::{
    connector_types::{
        ConnectorWebhookSecrets, DisputeWebhookDetailsResponse, EventContext, EventType,
        PayoutWebhookDetailsResponse, RefundWebhookDetailsResponse, RequestDetails,
        WebhookDetailsResponse, WebhookIntegrityCheck, WebhookResourceReference,
    },
    errors::WebhookError,
    router_data::ConnectorSpecificConfig,
};
use serde::{Deserialize, Serialize};

use crate::api::EventAckResponse;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Copy)]
#[serde(rename_all = "snake_case")]
pub enum IncomingWebhookEvent {
    /// Authorization + Capture success
    PaymentIntentFailure,
    /// Authorization + Capture failure
    PaymentIntentSuccess,
    PaymentIntentProcessing,
    PaymentIntentPartiallyFunded,
    PaymentIntentCancelled,
    PaymentIntentCancelFailure,
    PaymentIntentAuthorizationSuccess,
    PaymentIntentAuthorizationFailure,
    PaymentIntentExtendAuthorizationSuccess,
    PaymentIntentExtendAuthorizationFailure,
    PaymentIntentCaptureSuccess,
    PaymentIntentCaptureFailure,
    PaymentActionRequired,
    EventNotSupported,
    SourceChargeable,
    SourceTransactionCreated,
    RefundFailure,
    RefundSuccess,
    RefundProcessing,
    RefundReview,
    DisputeOpened,
    DisputeExpired,
    DisputeAccepted,
    DisputeCancelled,
    DisputeChallenged,
    // dispute has been successfully challenged by the merchant
    DisputeWon,
    // dispute has been unsuccessfully challenged
    DisputeLost,
    MandateActive,
    MandateRevoked,
    EndpointVerification,
    ExternalAuthenticationARes,
    FrmApproved,
    FrmRejected,
}

#[derive(Debug, Clone, Copy)]
pub enum IncomingWebhookFlowError {
    ResourceNotFound,
    InternalError,
}

pub trait IncomingWebhook {
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(false)
    }

    fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck> {
        vec![]
    }

    /// fn get_webhook_source_verification_signature
    fn get_webhook_source_verification_signature(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

    /// fn get_webhook_source_verification_message
    fn get_webhook_source_verification_message(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

    fn get_event_type(
        &self,
        _request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "get_event_type",
        }
        .into())
    }

    fn get_webhook_event_reference(
        &self,
        _request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        Ok(None)
    }

    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_payment_webhook",
        }
        .into())
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_refund_webhook",
        }
        .into())
    }
    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }
        .into())
    }

    /// fn process_payout_webhook
    ///
    /// Maps a payout webhook to the same shape a `PayoutService.Get` would
    /// return: the terminal status derived from the event type, the payout
    /// identifiers, and the failure details when the payout failed.
    fn process_payout_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<PayoutWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_payout_webhook",
        }
        .into())
    }

    /// fn get_webhook_resource_object
    fn get_webhook_resource_object(
        &self,
        _request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "get_webhook_resource_object",
        }
        .into())
    }

    /// A minimal, structurally valid webhook body for this connector.
    ///
    /// Used by the field-probe to verify that webhook handling is implemented
    fn sample_webhook_body(&self) -> &'static [u8] {
        b"{}"
    }

    /// fn get_webhook_api_response
    ///
    /// This is used by callers to decide what HTTP response
    /// should be sent back to the connector for webhook acknowledgement.
    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventAckResponse, error_stack::Report<WebhookError>> {
        Ok(EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: None,
        })
    }
}
