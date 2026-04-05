use common_utils::types::MinorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsErrorResponse {
    pub error_ref: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PeachpaymentsPaymentStatus {
    Successful,
    Pending,
    Authorized,
    Approved,
    ApprovedConfirmed,
    Declined,
    Failed,
    Reversed,
    ThreedsRequired,
    Voided,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PeachpaymentsRefundStatus {
    ApprovedConfirmed,
    Declined,
    Failed,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PeachpaymentsPaymentsResponse {
    Response(Box<PeachpaymentsPaymentsData>),
    WebhookResponse(Box<PeachpaymentsIncomingWebhook>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsPaymentsData {
    pub transaction_id: String,
    pub reference_id: String,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub ecommerce_card_payment_only_transaction_data: Option<PeachpaymentsCardResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsMerchantInformationResponse {
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsAuthorizeResponse {
    pub transaction_id: String,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub merchant_information: Option<PeachpaymentsMerchantInformationResponse>,
    pub ecommerce_card_payment_only_transaction_data: Option<PeachpaymentsCardResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsCaptureResponse {
    pub transaction_id: String,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub merchant_information: Option<PeachpaymentsMerchantInformationResponse>,
    #[serde(rename = "authorizationCode")]
    pub authorization_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsVoidResponse {
    pub transaction_id: String,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub merchant_information: Option<PeachpaymentsMerchantInformationResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRefundResponse {
    pub transaction_id: String,
    pub original_transaction_id: Option<String>,
    pub reference_id: String,
    pub transaction_result: PeachpaymentsRefundStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub refund_balance_data: Option<PeachpaymentsRefundBalance>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRefundHistory {
    pub transaction_id: String,
    pub reference_id: String,
    pub amount: PeachpaymentsAmountDetails,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRefundBalance {
    pub amount: PeachpaymentsAmountDetails,
    pub balance: PeachpaymentsAmountDetails,
    pub refund_history: Vec<PeachpaymentsRefundHistory>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsRefundSyncResponse {
    pub transaction_id: String,
    pub reference_id: String,
    pub transaction_result: PeachpaymentsRefundStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub refund_balance_data: Option<PeachpaymentsRefundBalance>,
}

pub type PeachpaymentsRsyncResponse = PeachpaymentsRefundSyncResponse;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged, rename_all = "camelCase")]
pub enum PeachpaymentsResponseCode {
    Text(String),
    Structured {
        value: String,
        description: String,
        terminal_outcome_string: Option<String>,
        receipt_string: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsSyncResponse {
    pub transaction_id: String,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub merchant_information: Option<PeachpaymentsMerchantInformationResponse>,
    pub ecommerce_card_payment_only_transaction_data: Option<PeachpaymentsCardResponseData>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsCardResponseData {
    pub amount: Option<PeachpaymentsAmountDetails>,
    pub stan: Option<Secret<String>>,
    pub rrn: Option<String>,
    pub approval_code: Option<String>,
    pub merchant_advice_code: Option<String>,
    pub description: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsAmountDetails {
    pub amount: MinorUnit,
    pub currency_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_amount: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsIncomingWebhook {
    pub webhook_id: String,
    pub webhook_type: String,
    pub reversal_failure_reason: Option<String>,
    pub transaction: Option<PeachpaymentsWebhookTransaction>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeachpaymentsWebhookTransaction {
    pub transaction_id: String,
    pub original_transaction_id: Option<String>,
    pub reference_id: String,
    pub transaction_result: PeachpaymentsPaymentStatus,
    pub error_message: Option<String>,
    pub transaction_type: PeachpaymentsTransactionType,
    pub response_code: Option<PeachpaymentsResponseCode>,
    pub ecommerce_card_payment_only_transaction_data: Option<PeachpaymentsCardResponseData>,
    pub refund_balance_data: Option<PeachpaymentsRefundBalance>,
    pub payment_method: Secret<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PeachpaymentsTransactionType {
    pub value: i32,
    pub description: String,
}

// SetupMandate and RepeatPayment reuse the same response structure as Authorize
pub type PeachpaymentsSetupMandateResponse = PeachpaymentsPaymentsResponse;
pub type PeachpaymentsRepeatPaymentResponse = PeachpaymentsPaymentsResponse;
