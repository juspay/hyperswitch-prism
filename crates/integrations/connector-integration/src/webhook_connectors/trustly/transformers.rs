use common_enums::Currency;
use common_utils::{ext_traits::ByteSliceExt, StringMajorUnit};
use domain_types::{
    connector_types::{
        PaymentWebhookReference, PayoutWebhookReference, RefundWebhookReference, RequestDetails,
        WebhookParseResponse, WebhookResourceReference,
    },
    errors::WebhookError,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookBody {
    pub method: TrustlyWebhookMethod,
    pub params: TrustlyWebhookParams,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TrustlyWebhookMethod {
    Credit,
    Debit,
    Cancel,
    Account,
    Pending,
    PayoutConfirmation,
    PayoutFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookParams {
    pub signature: String,
    pub uuid: String,
    pub data: TrustlyWebhookData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookData {
    pub amount: Option<StringMajorUnit>,
    pub currency: Option<Currency>,
    pub messageid: Secret<String>,
    pub orderid: String,
    pub enduserid: Option<String>,
    pub accountid: Option<String>,
    pub verified: Option<String>,
    pub notificationid: String,
    pub timestamp: Option<String>,
    pub attributes: Option<TrustlyWebhookAttributes>,
    pub errorcode: Option<String>,
    pub errormessage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustlyWebhookAttributes {
    pub bank: Option<Secret<String>>,
    pub city: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
    pub address: Option<Secret<String>>,
    pub zipcode: Option<Secret<String>>,
    pub personid: Option<Secret<String>>,
    pub descriptor: Option<String>,
    pub lastdigits: Option<String>,
    pub clearinghouse: Option<String>,
}

impl TrustlyWebhookMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Credit => "credit",
            Self::Debit => "debit",
            Self::Cancel => "cancel",
            Self::Account => "account",
            Self::Pending => "pending",
            Self::PayoutConfirmation => "payoutconfirmation",
            Self::PayoutFailed => "payoutfailed",
        }
    }
}

pub const TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX: &str = "payout_";

fn is_payout_message_id(message_id: &str) -> bool {
    message_id.starts_with(TRUSTLY_PAYOUT_MESSAGE_ID_PREFIX)
}

pub fn is_payment_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Debit
            | TrustlyWebhookMethod::Cancel
            | TrustlyWebhookMethod::Account
            | TrustlyWebhookMethod::Pending
    ) && !is_payout_message_id(message_id)
}

pub fn is_refund_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation | TrustlyWebhookMethod::PayoutFailed
    ) && !is_payout_message_id(message_id)
}

pub fn is_payout_webhook_event(event: &TrustlyWebhookMethod, message_id: &str) -> bool {
    matches!(
        event,
        TrustlyWebhookMethod::PayoutConfirmation
            | TrustlyWebhookMethod::PayoutFailed
            | TrustlyWebhookMethod::Credit
            | TrustlyWebhookMethod::Cancel
    ) && is_payout_message_id(message_id)
}

pub fn get_webhook_event(
    event: TrustlyWebhookMethod,
    message_id: String,
) -> domain_types::connector_types::EventType {
    match (event, !message_id.as_str().starts_with("payout_")) {
        (TrustlyWebhookMethod::Credit, true) => {
            domain_types::connector_types::EventType::PaymentIntentSuccess
        }
        (TrustlyWebhookMethod::Credit, false) => {
            domain_types::connector_types::EventType::PayoutReversed
        }
        (TrustlyWebhookMethod::Debit, _) => {
            domain_types::connector_types::EventType::PaymentIntentFailure
        }
        (TrustlyWebhookMethod::Cancel, true) => {
            domain_types::connector_types::EventType::PaymentIntentCancelled
        }
        (TrustlyWebhookMethod::Cancel, false) => {
            domain_types::connector_types::EventType::PayoutCancelled
        }
        (TrustlyWebhookMethod::Account, _) => {
            domain_types::connector_types::EventType::PaymentAssociatedDataUpdate
        }
        (TrustlyWebhookMethod::Pending, _) => {
            domain_types::connector_types::EventType::PaymentIntentProcessing
        }
        (TrustlyWebhookMethod::PayoutConfirmation, true) => {
            domain_types::connector_types::EventType::RefundSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, true) => {
            domain_types::connector_types::EventType::RefundFailure
        }
        (TrustlyWebhookMethod::PayoutConfirmation, false) => {
            domain_types::connector_types::EventType::PayoutSuccess
        }
        (TrustlyWebhookMethod::PayoutFailed, false) => {
            domain_types::connector_types::EventType::PayoutFailure
        }
    }
}

pub fn parse_event(
    request: &RequestDetails,
) -> Result<WebhookParseResponse, error_stack::Report<WebhookError>> {
    let body: TrustlyWebhookBody = request
        .body
        .parse_struct("TrustlyWebhookBody")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let message_id = body.params.data.messageid.expose();
    let event_type = get_webhook_event(body.method.clone(), message_id.clone());
    let order_id = body.params.data.orderid;
    let reference = if is_payment_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: Some(order_id),
            merchant_transaction_id: None,
            connector_preprocessing_id: None,
        }))
    } else if is_refund_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Refund(RefundWebhookReference {
            connector_refund_id: Some(order_id.clone()),
            merchant_refund_id: None,
            connector_transaction_id: Some(order_id),
            merchant_transaction_id: None,
        }))
    } else if is_payout_webhook_event(&body.method, &message_id) {
        Some(WebhookResourceReference::Payout(PayoutWebhookReference {
            connector_payout_id: Some(order_id),
            merchant_payout_id: None,
        }))
    } else {
        None
    };
    Ok(WebhookParseResponse {
        event_type,
        reference,
    })
}
