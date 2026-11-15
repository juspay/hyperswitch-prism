use common_utils::{ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::errors::IntegrationError;
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, strum::Display, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhookEventCode {
    Authorisation,
    AuthorisationAdjustment,
    Cancellation,
    Capture,
    CaptureFailed,
    Refund,
    RefundFailed,
    RefundReversed,
    CancelOrRefund,
    NotificationOfChargeback,
    Chargeback,
    ChargebackReversed,
    SecondChargeback,
    PrearbitrationWon,
    PrearbitrationLost,
    RequestForInformation,
    NotificationOfFraud,
    InformationSupplied,
    PrearbitrationOpen,
    PrearbitrationAccepted,
    PrearbitrationDeclined,
    PrearbitrationIssuerWithdrawn,
    SchemeArbitration,
    SchemeArbitrationWon,
    SchemeArbitrationLost,
    DisputeDefensePeriodEnded,
    IssuerResponseTimeframeExpired,
    IssuerComments,
    OfferClosed,
    RecurringContract,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub enum DisputeStatus {
    Undefended,
    Pending,
    Lost,
    Accepted,
    Won,
    Responded,
    Expired,
    Unresponded,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenAdditionalDataWH {
    #[serde(rename = "hmacSignature")]
    pub hmac_signature: Option<String>,
    pub dispute_status: Option<DisputeStatus>,
    pub chargeback_reason_code: Option<String>,
    /// Enable recurring details in Adyen dashboard to receive this ID.
    #[serde(rename = "recurring.recurringDetailReference")]
    pub recurring_detail_reference: Option<Secret<String>>,
    #[serde(rename = "recurring.shopperReference")]
    pub recurring_shopper_reference: Option<String>,
    pub network_tx_reference: Option<Secret<String>>,
    pub refusal_reason_raw: Option<String>,
    pub refusal_code_raw: Option<String>,
    pub shopper_email: Option<Secret<String>>,
    pub shopper_reference: Option<String>,
    pub expiry_date: Option<Secret<String>>,
    pub card_summary: Option<Secret<String>>,
    pub card_issuing_country: Option<String>,
    pub card_issuing_bank: Option<String>,
    pub payment_method_variant: Option<Secret<String>>,
    pub card_holder_name: Option<Secret<String>>,
    pub defense_period_ends_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenNotificationRequestItemWH {
    pub original_reference: Option<String>,
    pub psp_reference: String,
    pub amount: AdyenAmountWH,
    pub event_code: WebhookEventCode,
    pub merchant_account_code: String,
    pub merchant_reference: String,
    pub success: String,
    pub reason: Option<String>,
    pub additional_data: AdyenAdditionalDataWH,
}

#[derive(Debug, Deserialize)]
pub struct AdyenAmountWH {
    pub value: MinorUnit,
    pub currency: common_enums::Currency,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AdyenItemObjectWH {
    pub notification_request_item: AdyenNotificationRequestItemWH,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdyenIncomingWebhook {
    pub notification_items: Vec<AdyenItemObjectWH>,
}

pub fn get_webhook_object_from_body(
    body: Vec<u8>,
) -> Result<AdyenNotificationRequestItemWH, error_stack::Report<IntegrationError>> {
    let mut webhook: AdyenIncomingWebhook = body
        .parse_struct("AdyenIncomingWebhook")
        .change_context(IntegrationError::NotImplemented(
            "webhook body decoding failed".to_string(),
            Default::default(),
        ))?;

    let item_object =
        webhook
            .notification_items
            .drain(..)
            .next()
            .ok_or(IntegrationError::NotImplemented(
                "webhook body decoding failed".to_string(),
                Default::default(),
            ))?;

    Ok(item_object.notification_request_item)
}

pub(crate) fn is_success_scenario(is_success: &str) -> bool {
    is_success == "true"
}
