use common_utils::ext_traits::ByteSliceExt;
use domain_types::{
    connector_types::{
        PaymentWebhookReference, PayoutWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookParseResponse, WebhookResourceReference,
    },
    errors::WebhookError,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerWebhookEventType {
    PaymentAuthorized,
    PaymentFailed,
    PaymentSettled,
    PaymentExecuted,
    PaymentCreditable,
    PaymentSettlementStalled,
    RefundExecuted,
    RefundFailed,
    PaymentDisputed,
    PaymentReversed,
    PaymentFundsReceived,
    PayoutExecuted,
    PayoutFailed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerWebhookEventTypeBody {
    #[serde(rename = "type")]
    pub _type: TruelayerWebhookEventType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerWebhookBody {
    #[serde(rename = "type")]
    pub _type: TruelayerWebhookEventType,
    pub event_version: i32,
    pub event_id: String,
    pub payment_id: String,
    pub refund_id: Option<String>,
    pub failure_reason: Option<String>,
    pub failure_stage: Option<String>,
    pub user_id: Option<String>,
    pub payment_source: Option<TruelayerPaymentSource>,
    pub payment_method: Option<TruelayerPaymentMethod>,
}

/// Body of a Truelayer payout webhook.
///
/// Payout notifications share the envelope of payment and refund webhooks but
/// carry `payout_id` in place of `payment_id`, so they need their own struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPayoutWebhookBody {
    #[serde(rename = "type")]
    pub _type: TruelayerWebhookEventType,
    pub event_version: i32,
    pub event_id: String,
    pub payout_id: String,
    pub executed_at: Option<String>,
    pub failed_at: Option<String>,
    pub failure_reason: Option<String>,
    pub scheme_id: Option<String>,
}

/// Discriminator for the type of account identifier provided in a payment source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TruelayerAccountIdentifierType {
    SortCodeAccountNumber,
    Iban,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPaymentSource {
    pub id: Option<String>,
    pub account_holder_name: Option<Secret<String>>,
    pub account_identifiers: Option<Vec<TruelayerAccountIdentifier>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerAccountIdentifier {
    #[serde(rename = "type")]
    pub(crate) identifier_type: TruelayerAccountIdentifierType,
    pub(crate) sort_code: Option<Secret<String>>,
    pub(crate) account_number: Option<Secret<String>>,
    pub(crate) iban: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerPaymentMethod {
    pub(crate) provider_selection: Option<TruelayerProviderSelection>,
    /// Webhook payloads flatten `provider_id` onto `payment_method`, while the
    /// payments API nests it under `provider_selection`. Accept both shapes.
    pub(crate) provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TruelayerProviderSelection {
    pub(crate) provider_id: Option<String>,
}

impl TruelayerPaymentMethod {
    pub(crate) fn get_provider_id(&self) -> Option<String> {
        self.provider_selection
            .as_ref()
            .and_then(|selection| selection.provider_id.clone())
            .or_else(|| self.provider_id.clone())
    }
}

pub fn get_webhook_event(
    event: TruelayerWebhookEventType,
) -> domain_types::connector_types::EventType {
    match event {
        TruelayerWebhookEventType::PaymentExecuted
        | TruelayerWebhookEventType::PaymentAuthorized
        | TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentFundsReceived
        | TruelayerWebhookEventType::PaymentSettlementStalled => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        TruelayerWebhookEventType::PaymentSettled => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        TruelayerWebhookEventType::PaymentFailed => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        TruelayerWebhookEventType::RefundExecuted => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        TruelayerWebhookEventType::RefundFailed => {
            domain_types::connector_types::EventType::RefundFailure
        }
        TruelayerWebhookEventType::PaymentReversed => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        TruelayerWebhookEventType::PayoutExecuted => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        TruelayerWebhookEventType::PayoutFailed => {
            domain_types::connector_types::EventType::PayoutFailure
        }
        TruelayerWebhookEventType::PaymentDisputed | TruelayerWebhookEventType::Unknown => {
            domain_types::connector_types::EventType::IncomingWebhookEventUnspecified
        }
    }
}

fn get_reference(
    request: &RequestDetails,
    event: TruelayerWebhookEventType,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
    match event {
        TruelayerWebhookEventType::PayoutExecuted | TruelayerWebhookEventType::PayoutFailed => {
            let payout_body: TruelayerPayoutWebhookBody = request
                .body
                .parse_struct("TruelayerPayoutWebhookBody")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

            Ok(Some(WebhookResourceReference::Payout(
                PayoutWebhookReference {
                    connector_payout_id: Some(payout_body.payout_id),
                    merchant_payout_id: None,
                },
            )))
        }
        TruelayerWebhookEventType::PaymentAuthorized
        | TruelayerWebhookEventType::PaymentExecuted
        | TruelayerWebhookEventType::PaymentReversed
        | TruelayerWebhookEventType::PaymentFailed
        | TruelayerWebhookEventType::PaymentSettlementStalled
        | TruelayerWebhookEventType::PaymentSettled
        | TruelayerWebhookEventType::PaymentCreditable
        | TruelayerWebhookEventType::PaymentFundsReceived => {
            let webhook_body: TruelayerWebhookBody = request
                .body
                .parse_struct("TruelayerWebhookBody")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

            Ok(Some(WebhookResourceReference::Payment(
                PaymentWebhookReference {
                    connector_transaction_id: Some(webhook_body.payment_id),
                    merchant_transaction_id: None,
                    connector_preprocessing_id: None,
                },
            )))
        }
        TruelayerWebhookEventType::RefundExecuted | TruelayerWebhookEventType::RefundFailed => {
            let webhook_body: TruelayerWebhookBody = request
                .body
                .parse_struct("TruelayerWebhookBody")
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;

            Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: webhook_body.refund_id,
                    merchant_refund_id: None,
                    connector_transaction_id: Some(webhook_body.payment_id),
                    merchant_transaction_id: None,
                },
            )))
        }
        TruelayerWebhookEventType::PaymentDisputed | TruelayerWebhookEventType::Unknown => Ok(None),
    }
}

pub fn parse_event(
    request: &RequestDetails,
) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
    let envelope: TruelayerWebhookEventTypeBody = request
        .body
        .parse_struct("TruelayerWebhookEventTypeBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let event_type = get_webhook_event(envelope._type.clone());
    let reference = get_reference(request, envelope._type)?;
    Ok(WebhookParseResponse {
        event_type,
        reference,
    })
}
