pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::{Report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::TrustpayWebhookResponse;

#[derive(Debug, Clone)]
pub struct Trustpay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Trustpay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Trustpay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Trustpay<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let webhook_response: TrustpayWebhookResponse = request
            .body
            .parse_struct("TrustpayWebhookResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        Ok(transformers::get_event_type_from_webhook(
            &webhook_response.payment_information.credit_debit_indicator,
            &webhook_response.payment_information.status,
        ))
    }
}
