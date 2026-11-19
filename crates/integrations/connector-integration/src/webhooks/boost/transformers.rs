use common_enums::{AttemptStatus, Currency};
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector};
use domain_types::connector_types::{
    EventType, PaymentWebhookReference, ResponseId, WebhookDetailsResponse,
    WebhookResourceReference,
};
use serde::{Deserialize, Serialize};

use crate::connectors::boost::transformers::BoostPaymentStatus;

// =============================================================================
// WEBHOOKS (tech spec "Webhook Events" section)
// =============================================================================
// BCPG POSTs a single payment-result payload to `callbackUrl` per outcome
// (success or failure) — there is no categorized event-type feed, just a
// `status` field that reuses the exact same vocabulary as the Retrieve
// Payment Details (PSync) endpoint, so `BoostPaymentStatus` and its existing
// `From<BoostPaymentStatus> for AttemptStatus` mapping are reused as-is
// rather than duplicating the status logic.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostWebhookBody {
    pub timestamp: Option<String>,
    /// BCPG's own transaction id — same value returned as `uuid` on the
    /// Authorize (Init) and PSync responses.
    pub uuid: String,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub created: Option<String>,
    pub description: Option<String>,
    pub status: BoostPaymentStatus,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    /// The same merchant-generated reference sent as `referenceId` at
    /// Authorize and used to look the payment up again at PSync.
    #[serde(rename = "referenceId")]
    pub reference_id: String,
}

impl BoostWebhookBody {
    pub fn get_event_type(&self) -> EventType {
        match AttemptStatus::from(self.status.clone()) {
            AttemptStatus::Charged => EventType::PaymentIntentSuccess,
            AttemptStatus::Failure => EventType::PaymentIntentFailure,
            AttemptStatus::Voided => EventType::PaymentIntentCancelled,
            AttemptStatus::Expired => EventType::PaymentIntentExpired,
            AttemptStatus::PaymentMethodAwaited | AttemptStatus::ConfirmationAwaited => {
                EventType::PaymentActionRequired
            }
            _ => EventType::PaymentIntentProcessing,
        }
    }

    pub fn into_webhook_details_response(
        self,
        http_code: u16,
        raw_body: &[u8],
    ) -> WebhookDetailsResponse {
        let status = AttemptStatus::from(self.status);

        let amount_captured = match (self.amount, self.currency) {
            (Some(amount), Some(currency)) => {
                match FloatMajorUnitForConnector.convert_back(amount, currency) {
                    Ok(money) => Some(money),
                    Err(err) => {
                        tracing::warn!(
                            error = ?err,
                            amount = ?amount,
                            currency = ?currency,
                            "Failed to convert Boost webhook amount to minor units; \
                             leaving amount_captured unset"
                        );
                        None
                    }
                }
            }
            _ => None,
        };

        WebhookDetailsResponse {
            raw_connector_status: None,
            amount: None,
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(self.uuid.clone())),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id: Some(self.reference_id),
            mandate_reference: None,
            // BCPG's documented webhook payload (tech spec "Webhook Events") carries
            // no error_code/error_message fields on failure — only `status`. Nothing
            // here to surface without fabricating it.
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: http_code,
            response_headers: None,
            captured_money: amount_captured,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        }
    }

    pub fn into_webhook_event_reference(&self) -> WebhookResourceReference {
        WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: Some(self.uuid.clone()),
            merchant_transaction_id: Some(self.reference_id.clone()),
            connector_preprocessing_id: None,
        })
    }
}
