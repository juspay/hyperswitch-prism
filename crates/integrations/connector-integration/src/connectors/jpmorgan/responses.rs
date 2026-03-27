use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::requests::CapMethod;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JpmorganAuthUpdateResponse {
    pub access_token: Secret<String>,
    pub scope: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganErrorResponse {
    pub response_status: JpmorganTransactionStatus,
    pub response_code: String,
    pub response_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganTransactionStatus {
    Success,
    Denied,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganResponseStatus {
    Success,
    Denied,
    Error,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum JpmorganTransactionState {
    Closed,
    Authorized,
    Voided,
    #[default]
    Pending,
    Declined,
    Error,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganPaymentsResponse {
    pub transaction_id: String,
    pub request_id: String,
    pub transaction_state: JpmorganTransactionState,
    pub response_status: JpmorganTransactionStatus,
    pub response_code: String,
    pub response_message: String,
    pub payment_method_type: Option<PaymentMethodType>,
    pub capture_method: Option<CapMethod>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodType {
    pub card: Option<Card>,
    pub ach: Option<AchResponse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AchResponse {
    pub account_number: Option<Secret<String>>,
    pub account_type: Option<String>,
    pub financial_institution_routing_number: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub expiry: Option<ExpiryResponse>,
    pub card_type: Option<Secret<String>>,
    pub card_type_name: Option<Secret<String>>,
    pub masked_account_number: Option<Secret<String>>,
    pub card_type_indicators: Option<CardTypeIndicators>,
    pub network_response: Option<NetworkResponse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkResponse {
    pub address_verification_result: Option<Secret<String>>,
    pub address_verification_result_code: Option<Secret<String>>,
    pub card_verification_result_code: Option<Secret<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExpiryResponse {
    pub month: Option<Secret<i32>>,
    pub year: Option<Secret<i32>>,
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CardTypeIndicators {
    pub issuance_country_code: Option<Secret<String>>,
    pub is_durbin_regulated: Option<bool>,
    pub card_product_types: Secret<Vec<String>>,
}

#[derive(Debug, Serialize, Default, Deserialize, Clone)]
pub enum RefundStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed => Self::Failure,
            RefundStatus::Processing => Self::Pending,
        }
    }
}

impl From<(JpmorganResponseStatus, JpmorganTransactionState)> for RefundStatus {
    fn from(
        (response_status, transaction_state): (JpmorganResponseStatus, JpmorganTransactionState),
    ) -> Self {
        match response_status {
            JpmorganResponseStatus::Success => match transaction_state {
                JpmorganTransactionState::Voided | JpmorganTransactionState::Closed => {
                    Self::Succeeded
                }
                JpmorganTransactionState::Declined | JpmorganTransactionState::Error => {
                    Self::Failed
                }
                JpmorganTransactionState::Pending | JpmorganTransactionState::Authorized => {
                    Self::Processing
                }
            },
            JpmorganResponseStatus::Denied | JpmorganResponseStatus::Error => Self::Failed,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JpmorganRefundResponse {
    pub transaction_id: String,
    pub request_id: String,
    pub transaction_state: JpmorganTransactionState,
    pub response_status: JpmorganResponseStatus,
    pub response_code: String,
    pub response_message: String,
}

pub type JpmorganPSyncResponse = JpmorganPaymentsResponse;
pub type JpmorganCaptureResponse = JpmorganPaymentsResponse;
pub type JpmorganVoidResponse = JpmorganPaymentsResponse;
pub type JpmorganRSyncResponse = JpmorganRefundResponse;
