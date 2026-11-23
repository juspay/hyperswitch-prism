pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use self::transformers::{REFUND, TRANSACTION};
use crate::connectors::peachpayments::responses;
use common_utils::ext_traits::StringExt;
use domain_types::{
    connector_types::{EventType, RequestDetails},
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::{report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Peachpayments<T> {
    _phantom: PhantomData<T>,
}

impl<T> Peachpayments<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Peachpayments<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Peachpayments<T>
{
    // `get_webhook_event_reference` keeps the trait default (`Ok(None)`), as before the migration.
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = String::from_utf8(request.body.clone())
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let webhook_body: responses::PeachpaymentsIncomingWebhook = body
            .parse_struct("PeachpaymentsIncomingWebhook")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let description = webhook_body
            .transaction
            .as_ref()
            .map(|transaction| transaction.transaction_type.description.clone());

        match webhook_body.webhook_type.as_str() {
            TRANSACTION => {
                if let Some(transaction) = webhook_body.transaction {
                    match transaction.transaction_result {
                        responses::PeachpaymentsPaymentStatus::Successful => {
                            Ok(EventType::PaymentIntentSuccess)
                        }
                        responses::PeachpaymentsPaymentStatus::ApprovedConfirmed => {
                            if description == Some(REFUND.to_string()) {
                                Ok(EventType::RefundSuccess)
                            } else {
                                Ok(EventType::PaymentIntentSuccess)
                            }
                        }
                        responses::PeachpaymentsPaymentStatus::Authorized
                        | responses::PeachpaymentsPaymentStatus::Approved => {
                            Ok(EventType::PaymentIntentAuthorizationSuccess)
                        }
                        responses::PeachpaymentsPaymentStatus::Pending => {
                            Ok(EventType::PaymentIntentProcessing)
                        }
                        responses::PeachpaymentsPaymentStatus::Declined
                        | responses::PeachpaymentsPaymentStatus::Failed => {
                            if description == Some(REFUND.to_string()) {
                                Ok(EventType::RefundFailure)
                            } else {
                                Ok(EventType::PaymentIntentFailure)
                            }
                        }
                        responses::PeachpaymentsPaymentStatus::Voided
                        | responses::PeachpaymentsPaymentStatus::Reversed => {
                            Ok(EventType::PaymentIntentCancelled)
                        }
                        responses::PeachpaymentsPaymentStatus::ThreedsRequired => {
                            Ok(EventType::PaymentActionRequired)
                        }
                    }
                } else {
                    Err(report!(WebhookError::WebhookEventTypeNotFound))
                }
            }
            _ => Err(report!(WebhookError::WebhookEventTypeNotFound)),
        }
    }
}
