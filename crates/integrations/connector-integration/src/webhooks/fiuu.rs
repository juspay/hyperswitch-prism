pub mod transformers;

use std::{fmt::Debug, marker::PhantomData};

use crate::connectors::fiuu::{
    parse_and_log_keys_in_url_encoded_response, transformers::FiuuWebhooksResponse,
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
use error_stack::{report, ResultExt};
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Fiuu<T> {
    _phantom: PhantomData<T>,
}

impl<T> Fiuu<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for Fiuu<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for Fiuu<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        if request.body.is_empty() {
            return Ok(EventType::EndpointVerification);
        }
        let header = request
            .headers
            .get("content-type")
            .ok_or_else(|| report!(WebhookError::WebhookBodyDecodingFailed))?;

        let resource: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(WebhookError::WebhookBodyDecodingFailed)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?
        };

        match resource {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(webhooks_payment_response) => {
                Ok(EventType::from(webhooks_payment_response.status))
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(webhooks_refunds_response) => {
                Ok(EventType::from(webhooks_refunds_response.status))
            }
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let header = request
            .headers
            .get("content-type")
            .ok_or_else(|| report!(WebhookError::WebhookBodyDecodingFailed))?;

        let payload: FiuuWebhooksResponse = if header == "application/x-www-form-urlencoded" {
            parse_and_log_keys_in_url_encoded_response::<FiuuWebhooksResponse>(&request.body);
            serde_urlencoded::from_bytes::<FiuuWebhooksResponse>(&request.body)
                .change_context(WebhookError::WebhookResourceObjectNotFound)?
        } else {
            request
                .body
                .parse_struct("fiuu::FiuuWebhooksResponse")
                .change_context(WebhookError::WebhookResourceObjectNotFound)?
        };

        let reference = match payload {
            FiuuWebhooksResponse::FiuuWebhookPaymentResponse(p) => {
                WebhookResourceReference::Payment(PaymentWebhookReference {
                    connector_transaction_id: Some(p.tran_id),
                    // order_id is the merchant-assigned order reference echoed back by Fiuu.
                    merchant_transaction_id: Some(p.order_id),
                    connector_preprocessing_id: None,
                })
            }
            FiuuWebhooksResponse::FiuuWebhookRefundResponse(r) => {
                WebhookResourceReference::Refund(RefundWebhookReference {
                    connector_refund_id: Some(r.refund_id),
                    merchant_refund_id: None,
                    // txn_id is the connector ID of the original payment.
                    connector_transaction_id: Some(r.txn_id),
                    merchant_transaction_id: None,
                })
            }
        };

        Ok(Some(reference))
    }
}
