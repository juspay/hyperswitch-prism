pub mod transformers;

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::ConnectorWebhookSecrets, errors, router_data::ConnectorSpecificConfig,
};
use domain_types::{
    connector_types::{RequestDetails, WebhookParseResponse},
    errors::WebhookError,
};
use error_stack::ResultExt;
use hyperswitch_masking::ExposeInterface;
use interfaces::{connector_types, webhook_common::WebhookCommon};
use transformers::{self as trustly, TrustlyWebhookBody};

#[derive(Debug, Clone, Default)]
pub struct TrustlyWebhook;

impl TrustlyWebhook {
    pub const fn new() -> Self {
        Self
    }
}

impl WebhookCommon for TrustlyWebhook {
    fn parse_event(
        &self,
        request: &RequestDetails,
    ) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
        transformers::parse_event(request)
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_secret = match connector_webhook_secret {
            Some(secrets) => secrets.secret,
            None => return Ok(false),
        };

        trustly::verify_webhook_signature(webhook_body, webhook_secret)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let webhook_body: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let message_id = webhook_body.params.data.messageid.clone();

        Ok(trustly::get_webhook_event(
            webhook_body.method,
            message_id.expose(),
        ))
    }

    fn get_webhook_api_response(
        &self,
        request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, error_stack::Report<WebhookError>> {
        let details: TrustlyWebhookBody = request
            .body
            .parse_struct("TrustlyWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let data = trustly::TrustlyWebhookResponseResultData {
            status: "OK".to_string(),
        };

        let connector_auth_type =
            connector_account_details.ok_or(WebhookError::WebhookResponseEncodingFailed)?;

        let private_key = match connector_auth_type {
            ConnectorSpecificConfig::Trustly { private_key, .. } => private_key,
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    }
                )
                .change_context(WebhookError::WebhookResponseEncodingFailed))
            }
        };

        let signature = trustly::generate_trustly_signature(
            details.method.as_str(),
            &details.params.uuid,
            &data,
            &private_key.expose(),
        )
        .change_context(WebhookError::WebhookResponseEncodingFailed)?;

        let response = trustly::TrustlyWebhookResponse {
            result: trustly::TrustlyWebhookResponseResult {
                signature: signature.into(),
                uuid: details.params.uuid,
                method: details.method.as_str().to_string(),
                data,
            },
            version: details.version,
        };

        let response_body = serde_json::to_vec(&response)
            .change_context(WebhookError::WebhookResponseEncodingFailed)?;

        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(response_body),
        })
    }
}
