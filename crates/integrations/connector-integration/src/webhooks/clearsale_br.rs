use std::{fmt::Debug, marker::PhantomData};

use crate::frm_connectors::clearsale_br::transformers::ClearSaleWebhookPayload;
use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        EventType, PaymentWebhookReference, RequestDetails, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
};
use error_stack::ResultExt;
use interfaces::webhooks::IncomingWebhookEventParser;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ClearsaleBr<T> {
    _phantom: PhantomData<T>,
}

impl<T> ClearsaleBr<T> {
    pub const fn new() -> &'static Self {
        &Self {
            _phantom: PhantomData,
        }
    }
}

impl<T> Default for ClearsaleBr<T> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    IncomingWebhookEventParser<T> for ClearsaleBr<T>
{
    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Map the payload `type` explicitly. ClearSale only documents
        // `"status"` (an order status change — decision still unknown until a
        // follow-up `GET /v1/orders/{code}/status`). Anything else is
        // unrecognised and must not be hard-coded to Processing.
        let payload: ClearSaleWebhookPayload = request
            .body
            .parse_struct("ClearSaleWebhookPayload")
            .change_context(WebhookError::WebhookEventTypeNotFound)?;
        match payload.event_type.as_str() {
            "status" => Ok(EventType::Payment),
            _ => Err(error_stack::report!(WebhookError::WebhookEventTypeNotFound)),
        }
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // `code` is the order reference the merchant created the order with —
        // it is the correlation key stored at order-create time. The same
        // `code` is what `process_payment_webhook` returns as the resource id
        // (merchant_transaction_id carrier — not a PSP connector id).
        let payload: ClearSaleWebhookPayload = request
            .body
            .parse_struct("ClearSaleWebhookPayload")
            .change_context(WebhookError::WebhookReferenceIdNotFound)?;
        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: None,
                merchant_transaction_id: Some(payload.code),
                connector_preprocessing_id: None,
            },
        )))
    }
}
