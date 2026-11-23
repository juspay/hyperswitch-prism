use common_utils::ext_traits::ByteSliceExt;
use domain_types::{connector_types::RequestDetails, errors::WebhookError};
use error_stack::{Report, ResultExt};

use crate::connectors::cybersource::transformers::CybersourceWebhookBody;

/// Deserializes the notification envelope. A body that is not the plaintext JSON envelope
/// (for example a message-level encrypted notification) is refused.
pub(crate) fn parse_cybersource_webhook_body(
    request: &RequestDetails,
) -> Result<CybersourceWebhookBody, Report<WebhookError>> {
    request
        .body
        .parse_struct("CybersourceWebhookBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)
}
