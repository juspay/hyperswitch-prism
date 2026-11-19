use common_enums::RefundStatus;
use domain_types::{
    connector_types::{
        EventType, RefundWebhookDetailsResponse, ResponseId, WebhookDetailsResponse,
    },
    errors::IntegrationError,
};
use serde::{Deserialize, Serialize};

use crate::connectors::flywire::transformers::FlywirePaymentStatus;

// =============================================================================
// Webhook
//
// Spec: https://developers.flywire.com/education/Content/notifications-from-flywire.htm
//
// Top-level shape (both payments and refunds):
//   {
//     "event_type":     "<status name>",
//     "event_date":     "<RFC3339 timestamp>",
//     "event_resource": "payments" | "charges" | "refunds",
//     "data": { ... resource-specific fields ... }
//   }
// =============================================================================

/// Tag used by Flywire to route a webhook between payment-status and refund-status
/// payloads. `charges` is the legacy partial-capture variant of `payments` and
/// uses the same body schema; we treat them identically.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywireWebhookResource {
    Payments,
    Charges,
    Refunds,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FlywireWebhookBody {
    pub event_type: String,
    pub event_date: Option<String>,
    pub event_resource: FlywireWebhookResource,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywirePaymentWebhookData {
    pub payment_id: String,
    pub status: FlywirePaymentStatus,
    pub external_reference: Option<String>,
    pub amount_from: Option<String>,
    pub currency_from: Option<String>,
    pub amount_to: Option<String>,
    pub currency_to: Option<String>,
}

/// Refund-resource webhook payload. Flywire uses a separate set of statuses
/// for refund events (the documented values are `initiated`, `received`,
/// `finished`, `cancelled`, `returned`, `failed`). We model just success vs.
/// failure here; everything else maps to Pending.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlywireRefundWebhookStatus {
    Initiated,
    Received,
    Finished,
    Cancelled,
    Returned,
    Failed,
    #[serde(other)]
    Unknown,
}

impl FlywireRefundWebhookStatus {
    pub fn to_refund_status(&self) -> RefundStatus {
        match self {
            Self::Finished => RefundStatus::Success,
            Self::Cancelled | Self::Returned | Self::Failed => RefundStatus::Failure,
            Self::Initiated | Self::Received | Self::Unknown => RefundStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlywireRefundWebhookData {
    pub refund_id: String,
    pub payment_id: Option<String>,
    pub status: FlywireRefundWebhookStatus,
    pub external_reference: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
}

impl FlywireWebhookBody {
    pub fn parse_payment_data(
        &self,
    ) -> Result<FlywirePaymentWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|err| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify the webhook `data` object matches Flywire's payment-status \
                         notification schema (payment_id, status, ...)."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.flywire.com/education/Content/notifications-payment-status.htm"
                            .to_string(),
                    ),
                    additional_context: Some(format!(
                        "Failed to deserialize payments webhook `data`: {err}"
                    )),
                },
            })
            .map_err(error_stack::Report::from)
    }

    pub fn parse_refund_data(
        &self,
    ) -> Result<FlywireRefundWebhookData, error_stack::Report<IntegrationError>> {
        serde_json::from_value(self.data.clone())
            .map_err(|err| IntegrationError::InvalidDataFormat {
                field_name: "flywire webhook data",
                context: domain_types::errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Verify the webhook `data` object matches Flywire's refund-status \
                         notification schema (refund_id, status, ...)."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://developers.flywire.com/education/Content/resource_refunds.htm"
                            .to_string(),
                    ),
                    additional_context: Some(format!(
                        "Failed to deserialize refunds webhook `data`: {err}"
                    )),
                },
            })
            .map_err(error_stack::Report::from)
    }
}

impl TryFrom<&FlywireWebhookBody> for WebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(body: &FlywireWebhookBody) -> Result<Self, Self::Error> {
        let data = body.parse_payment_data()?;
        Ok(Self {
            raw_connector_status: None,
            amount: None,
            connector_returned_payment_method_details: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(data.payment_id)),
            status: data.status.to_attempt_status(),
            error_code: None,
            error_message: None,
            error_reason: None,
            status_code: 200,
            connector_response_reference_id: data.external_reference.clone(),
            connector_request_reference_id: data.external_reference,
            mandate_reference: None,
            raw_connector_response: None,
            response_headers: None,
            captured_money: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }
}

impl TryFrom<&FlywireWebhookBody> for RefundWebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(body: &FlywireWebhookBody) -> Result<Self, Self::Error> {
        let data = body.parse_refund_data()?;
        Ok(Self {
            raw_connector_status: None,
            connector_refund_id: Some(data.refund_id),
            merchant_transaction_id: None,
            status: data.status.to_refund_status(),
            connector_response_reference_id: data.external_reference,
            error_code: None,
            error_message: None,
            raw_connector_response: None,
            status_code: 200,
            response_headers: None,
        })
    }
}

pub fn webhook_event_type(body: &FlywireWebhookBody) -> EventType {
    match body.event_resource {
        FlywireWebhookResource::Refunds => {
            let status = body
                .parse_refund_data()
                .ok()
                .map(|d| d.status)
                .unwrap_or(FlywireRefundWebhookStatus::Unknown);
            match status {
                FlywireRefundWebhookStatus::Finished => EventType::RefundSuccess,
                FlywireRefundWebhookStatus::Cancelled
                | FlywireRefundWebhookStatus::Returned
                | FlywireRefundWebhookStatus::Failed => EventType::RefundFailure,
                FlywireRefundWebhookStatus::Initiated
                | FlywireRefundWebhookStatus::Received
                | FlywireRefundWebhookStatus::Unknown => EventType::PaymentIntentProcessing,
            }
        }
        // `charges` events are per-card-capture-attempt notifications. A single
        // payment can emit multiple charges events (e.g., first attempt declined,
        // second attempt succeeds) before the payment-level state is decided.
        // We MUST NOT promote a charges:failed event to PaymentIntentFailure -
        // that would terminate the txn in Euler before the payer's retry chance
        // is exhausted. The canonical terminal signal is the `payments:*` event
        // family; charges events are informational and always map to Processing.
        FlywireWebhookResource::Charges => EventType::PaymentIntentProcessing,
        FlywireWebhookResource::Payments | FlywireWebhookResource::Unknown => {
            let status = body
                .parse_payment_data()
                .ok()
                .map(|d| d.status)
                .unwrap_or(FlywirePaymentStatus::Unknown);
            match status {
                FlywirePaymentStatus::Guaranteed | FlywirePaymentStatus::Delivered => {
                    EventType::PaymentIntentSuccess
                }
                FlywirePaymentStatus::Cancelled
                | FlywirePaymentStatus::Failed
                | FlywirePaymentStatus::Expired => EventType::PaymentIntentFailure,
                FlywirePaymentStatus::Authorized => EventType::PaymentIntentAuthorizationSuccess,
                FlywirePaymentStatus::Reversed => EventType::RefundSuccess,
                FlywirePaymentStatus::Initiated
                | FlywirePaymentStatus::Adjusted
                | FlywirePaymentStatus::Processed
                | FlywirePaymentStatus::Pending
                | FlywirePaymentStatus::Unknown => EventType::PaymentIntentProcessing,
            }
        }
    }
}
