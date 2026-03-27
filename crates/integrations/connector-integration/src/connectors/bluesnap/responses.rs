use common_utils::FloatMajorUnit;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use super::requests::BluesnapTxnType;

// Error message constants
const DEFAULT_ERROR_CODE: &str = "UNKNOWN_ERROR";
const DEFAULT_ERROR_MESSAGE: &str = "Unknown error occurred";

// Error response structure - BlueSnap API uses nested format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluesnapErrorResponse {
    pub message: Vec<BluesnapErrorMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapErrorMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub description: String,
}

impl BluesnapErrorResponse {
    pub fn code(&self) -> String {
        self.message
            .first()
            .and_then(|msg| msg.code.clone())
            .unwrap_or_else(|| DEFAULT_ERROR_CODE.to_string())
    }

    pub fn message(&self) -> String {
        self.message
            .first()
            .map(|msg| msg.description.clone())
            .unwrap_or_else(|| DEFAULT_ERROR_MESSAGE.to_string())
    }
}

impl Default for BluesnapErrorResponse {
    fn default() -> Self {
        Self {
            message: vec![BluesnapErrorMessage {
                error_name: None,
                code: None,
                description: DEFAULT_ERROR_MESSAGE.to_string(),
            }],
        }
    }
}

// BlueSnap processing status enum
// Note: BlueSnap returns both lowercase ("success") and uppercase ("SUCCESS")
// depending on the operation, so we handle both cases
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BluesnapProcessingStatus {
    #[serde(alias = "success")]
    Success,
    #[default]
    #[serde(alias = "pending")]
    Pending,
    #[serde(alias = "fail")]
    Fail,
    #[serde(alias = "pending_merchant_review")]
    PendingMerchantReview,
}

// Processing info from BlueSnap response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapProcessingInfo {
    pub processing_status: BluesnapProcessingStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv_response_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_response_code_zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_response_code_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avs_response_code_name: Option<String>,
    pub network_transaction_id: Option<Secret<String>>,
}

// Credit card info in response
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapCreditCardResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_last_four_digits: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_sub_type: Option<String>,
}

// Main authorize response structure based on tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapPaymentsResponse {
    pub transaction_id: String,
    // Note: card_transaction_type is not present in ACH/ECP responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_transaction_type: Option<BluesnapTxnType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<FloatMajorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    pub processing_info: BluesnapProcessingInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_card: Option<BluesnapCreditCardResponse>,
}

// Type aliases for response types to avoid template conflicts
pub type BluesnapAuthorizeResponse = BluesnapPaymentsResponse;
pub type BluesnapCaptureResponse = BluesnapPaymentsResponse;
pub type BluesnapPSyncResponse = BluesnapPaymentsResponse;
pub type BluesnapVoidResponse = BluesnapPaymentsResponse;

// Refund response structure based on BlueSnap tech spec
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapRefundResponse {
    pub refund_transaction_id: i32,
    pub refund_status: BluesnapRefundStatus,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum BluesnapRefundStatus {
    Success,
    #[default]
    Pending,
}

pub type BluesnapRefundSyncResponse = BluesnapPSyncResponse;

// ===== 3DS AUTHENTICATION RESPONSES =====

// 3DS redirect response after authentication
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapRedirectionResponse {
    pub authentication_response: String, // JSON string containing 3DS result
}

// 3DS result structure (parsed from authentication_response)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapThreeDsResult {
    pub three_d_secure: Option<BluesnapThreeDsReference>,
    pub status: String, // "Success" or error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<RedirectErrorMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapThreeDsReference {
    pub three_d_secure_reference_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RedirectErrorMessage {
    pub errors: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BluesnapWebhookEvent {
    Decline,
    CcChargeFailed,
    Charge,
    Refund,
    Chargeback,
    ChargebackStatusChanged,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapWebhookBody {
    pub merchant_transaction_id: String,
    pub reference_number: String,
    pub transaction_type: BluesnapWebhookEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reversal_ref_num: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapWebhookObjectResource {
    pub reference_number: String,
    pub transaction_type: BluesnapWebhookEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reversal_ref_num: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BluesnapChargebackStatus {
    #[serde(alias = "New")]
    New,
    #[serde(alias = "Working")]
    Working,
    #[serde(alias = "Closed")]
    Closed,
    #[serde(alias = "Completed_Lost")]
    CompletedLost,
    #[serde(alias = "Completed_Pending")]
    CompletedPending,
    #[serde(alias = "Completed_Won")]
    CompletedWon,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluesnapDisputeWebhookBody {
    pub invoice_charge_amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reversal_reason: Option<String>,
    pub reversal_ref_num: String,
    pub cb_status: String,
}
