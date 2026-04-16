use common_utils::{crypto, CustomResult};
use domain_types::{
    connector_types::{ConnectorRedirectResponseSecrets, ConnectorWebhookSecrets},
    router_data::ConnectorSpecificConfig,
};
use error_stack::ResultExt;

#[derive(Clone)]
pub enum ConnectorSourceVerificationSecrets {
    AuthHeaders(ConnectorSpecificConfig),
    WebhookSecret(ConnectorWebhookSecrets),
    RedirectResponseSecret(ConnectorRedirectResponseSecrets),
    AuthWithWebHookSecret {
        auth_headers: ConnectorSpecificConfig,
        webhook_secret: ConnectorWebhookSecrets,
    },
}

/// Core trait for source verification
pub trait SourceVerification {
    fn get_secrets(
        &self,
        _secrets: ConnectorSourceVerificationSecrets,
    ) -> CustomResult<Vec<u8>, domain_types::errors::IntegrationError> {
        Ok(Vec::new())
    }

    /// Get the verification algorithm being used
    fn get_algorithm(
        &self,
    ) -> CustomResult<Box<dyn crypto::VerifySignature + Send>, domain_types::errors::IntegrationError>
    {
        Ok(Box::new(crypto::NoAlgorithm))
    }

    /// Get the signature/hash value from the payload for verification
    fn get_signature(
        &self,
        _payload: &[u8],
        _secrets: &[u8],
    ) -> CustomResult<Vec<u8>, domain_types::errors::IntegrationError> {
        Ok(Vec::new())
    }

    /// Get the message/payload that should be verified
    fn get_message(
        &self,
        payload: &[u8],
        _secrets: &[u8],
    ) -> CustomResult<Vec<u8>, domain_types::errors::IntegrationError> {
        Ok(payload.to_owned())
    }

    /// Perform the verification
    fn verify(
        &self,
        secrets: ConnectorSourceVerificationSecrets,
        payload: &[u8],
    ) -> CustomResult<bool, domain_types::errors::IntegrationError> {
        let algorithm = self.get_algorithm()?;
        let extracted_secrets = self.get_secrets(secrets)?;
        let signature = self.get_signature(payload, &extracted_secrets)?;
        let message = self.get_message(payload, &extracted_secrets)?;

        // Verify the signature against the message
        algorithm
            .verify_signature(&extracted_secrets, &signature, &message)
            .change_context(
                domain_types::errors::IntegrationError::SourceVerificationFailed {
                    context: Default::default(),
                },
            )
    }
}
