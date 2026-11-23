pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

use self::transformers::CryptopayWebhookDetails;
use crate::connectors::cryptopay::transformers::CryptopayPaymentStatus;

#[derive(Debug, Clone)]
pub struct Cryptopay<T> {
    _phantom: PhantomData<T>,
}

impl<T> Cryptopay<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Cryptopay<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Cryptopay<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: CryptopayWebhookDetails =
            request
                .body
                .parse_struct("CryptopayWebhookDetails")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        match notif.data.status {
            CryptopayPaymentStatus::Completed => Ok(EventType::PaymentIntentSuccess),
            CryptopayPaymentStatus::Unresolved => Ok(EventType::PaymentActionRequired),
            CryptopayPaymentStatus::Cancelled => Ok(EventType::PaymentIntentFailure),
            _ => Ok(EventType::IncomingWebhookEventUnspecified),
        }
    }
}
