use crate::verification::ConnectorSourceVerificationSecrets;
use common_utils::{crypto, CustomResult};
use error_stack::ResultExt;

/// Core trait for decoding message
pub trait BodyDecoding {
    fn get_secrets(
        &self,
        _secrets: ConnectorSourceVerificationSecrets,
    ) -> CustomResult<Vec<u8>, domain_types::errors::ConnectorError> {
        Ok(Vec::new())
    }

    /// Get the decoding algorithm being used
    fn get_algorithm(
        &self,
    ) -> CustomResult<Box<dyn crypto::DecodeMessage + Send>, domain_types::errors::ConnectorError>
    {
        Ok(Box::new(crypto::NoAlgorithm))
    }

    /// Get the message/payload that should be decoded
    fn get_message(
        &self,
        body: &[u8],
    ) -> CustomResult<Vec<u8>, domain_types::errors::ConnectorError> {
        Ok(body.to_owned())
    }

    /// Perform the decoding
    fn decode(
        &self,
        // The `secrets` argument is an `Option` to support decoding algorithms that do not require
        // a secret (e.g., Base64 decoding).
        //
        // If a secret is not required, the implementing connector can override this method
        // to handle `None` gracefully. The default implementation assumes a secret is mandatory.
        secrets: Option<ConnectorSourceVerificationSecrets>,
        body: &[u8],
    ) -> CustomResult<Vec<u8>, domain_types::errors::ConnectorError> {
        let secrets = secrets.ok_or(domain_types::errors::ConnectorError::DecodingFailed(None))?;

        let algorithm = self.get_algorithm()?;
        let extracted_secrets = self.get_secrets(secrets)?;
        let message = self.get_message(body)?;

        algorithm
            .decode_message(&extracted_secrets, message.into())
            .change_context(domain_types::errors::ConnectorError::DecodingFailed(None))
    }
}
