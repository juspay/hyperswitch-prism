//! Provider-level webhook operations shared across resource families.

use domain_types::{
    connector_types::{
        ConnectorWebhookSecrets, EventType, RequestDetails, WebhookIntegrityCheck,
        WebhookParseResponse,
    },
    errors::WebhookError,
    router_data::ConnectorSpecificConfig,
};

use crate::{api::EventAckResponse, connector_types::IncomingWebhookFlowError};

/// Provider webhook protocol operations; resource normalization stays in resource connectors.
pub type WebhookVerificationIntegration<'a> =
    crate::connector_integration_v2::BoxedConnectorIntegrationV2<
        'a,
        domain_types::connector_flow::VerifyWebhookSource,
        domain_types::connector_types::VerifyWebhookSourceFlowData,
        domain_types::router_request_types::VerifyWebhookSourceRequestData,
        domain_types::router_response_types::VerifyWebhookSourceResponseData,
    >;

pub trait Webhook: Send + Sync {
    /// Classifies the notification and extracts its unverified resource reference.
    fn parse_event(
        &self,
        request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>>;

    /// Override to classify HandleEvent without requiring ParseEvent's resource references.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        self.parse_event(&request).map(|parsed| parsed.event_type)
    }

    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(false)
    }

    /// Providers can require an external verification call without a payment connector.
    /// The server's existing configured verification list is also honored.
    fn requires_external_webhook_verification(&self) -> bool {
        false
    }

    /// Supply the provider's verification flow when an external call is required.
    /// Missing integration is an explicit error, never a local-verification fallback.
    fn get_external_verification_integration(&self) -> Option<WebhookVerificationIntegration<'_>> {
        None
    }

    fn get_webhook_source_verification_signature(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

    fn get_webhook_source_verification_message(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

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

    fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck> {
        vec![]
    }
}

pub type BoxedWebhookConnector = Box<dyn Webhook>;
