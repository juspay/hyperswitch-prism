//! Type definitions for Juspay UPI Merchant Stack
//!
//! This module defines all shared types, structs, and enums used across
//! bank connectors in the Juspay UPI Merchant Stack.

use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

/// Authentication configuration for Juspay UPI Stack connectors
#[derive(Debug, Clone)]
pub struct JuspayUpiAuthConfig {
    /// Merchant ID for x-merchant-id header
    pub merchant_id: String,
    /// Merchant channel ID for x-merchant-channel-id header
    pub merchant_channel_id: String,
    /// Key ID for JWS signing (merchant's key identifier)
    pub merchant_kid: String,
    /// Juspay's key ID for response verification
    pub juspay_kid: String,
    /// Merchant's RSA private key for JWS signing
    pub merchant_private_key: Secret<String>,
    /// Juspay's RSA public key for response signature verification
    pub juspay_public_key: Secret<String>,
    /// Whether to use JWE encryption (false for Axis Bank UAT)
    pub use_jwe: bool,
    /// Optional: JWE key ID (only needed when use_jwe = true)
    pub jwe_kid: Option<String>,
    /// Optional: Juspay's JWE public key for encryption
    pub juspay_jwe_public_key: Option<Secret<String>>,
    /// Optional: Merchant's JWE private key for decryption
    pub merchant_jwe_private_key: Option<Secret<String>>,
}

/// JWS Object structure - the body of every API request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwsObject {
    /// Base64url-encoded protected header (contains alg and kid)
    pub protected: String,
    /// Base64url-encoded payload (the actual API request data)
    pub payload: String,
    /// Base64url-encoded RS256 signature
    pub signature: String,
}

// ============================================
// REQUEST TYPES
// ============================================

/// Request payload for Register Intent API (Authorize flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterIntentRequest {
    #[serde(rename = "merchantRequestId")]
    pub merchant_request_id: String,
    /// UPI Request ID - required for Multibank API requests
    /// Typically same as merchantRequestId
    #[serde(rename = "upiRequestId")]
    pub upi_request_id: String,
    pub amount: String,
    pub flow: String,
    #[serde(rename = "intentRequestExpiryMinutes")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_request_expiry_minutes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(rename = "refUrl", skip_serializing_if = "Option::is_none")]
    pub ref_url: Option<String>,
    #[serde(rename = "refCategory", skip_serializing_if = "Option::is_none")]
    pub ref_category: Option<String>,
    pub iat: String,
    #[serde(rename = "udfParameters", skip_serializing_if = "Option::is_none")]
    pub udf_parameters: Option<String>,
}

/// Request payload for Status 360 API (PSync flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status360Request {
    #[serde(rename = "merchantRequestId")]
    pub merchant_request_id: String,
    #[serde(rename = "transactionType")]
    pub transaction_type: String,
    #[serde(rename = "transactionTimestamp", skip_serializing_if = "Option::is_none")]
    pub transaction_timestamp: Option<String>,
    pub iat: String,
}

/// Request payload for Refund 360 API (Refund/RSync flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund360Request {
    #[serde(rename = "originalMerchantRequestId")]
    pub original_merchant_request_id: String,
    #[serde(rename = "refundRequestId")]
    pub refund_request_id: String,
    #[serde(rename = "refundType")]
    pub refund_type: String,
    #[serde(rename = "refundAmount")]
    pub refund_amount: String,
    pub remarks: String,
    #[serde(rename = "adjCode", skip_serializing_if = "Option::is_none")]
    pub adj_code: Option<String>,
    #[serde(rename = "adjFlag", skip_serializing_if = "Option::is_none")]
    pub adj_flag: Option<String>,
    #[serde(rename = "merchantRefundVpa", skip_serializing_if = "Option::is_none")]
    pub merchant_refund_vpa: Option<String>,
    #[serde(rename = "originalTransactionTimestamp", skip_serializing_if = "Option::is_none")]
    pub original_transaction_timestamp: Option<String>,
    pub iat: String,
    #[serde(rename = "udfParameters", skip_serializing_if = "Option::is_none")]
    pub udf_parameters: Option<String>,
}

// ============================================
// RESPONSE TYPES
// ============================================

/// Generic outer response structure for all APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JuspayUpiApiResponse<T> {
    pub status: String,
    #[serde(rename = "responseCode")]
    pub response_code: String,
    #[serde(rename = "responseMessage")]
    pub response_message: String,
    pub payload: Option<T>,
}

/// Payload for Register Intent response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterIntentResponsePayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantChannelId")]
    pub merchant_channel_id: String,
    #[serde(rename = "merchantRequestId")]
    pub merchant_request_id: String,
    #[serde(rename = "gatewayTransactionId")]
    pub gateway_transaction_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "payeeVpa")]
    pub payee_vpa: String,
    #[serde(rename = "payeeName")]
    pub payee_name: String,
    #[serde(rename = "payeeMcc")]
    pub payee_mcc: String,
    pub amount: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(rename = "refUrl", skip_serializing_if = "Option::is_none")]
    pub ref_url: Option<String>,
    #[serde(rename = "refCategory", skip_serializing_if = "Option::is_none")]
    pub ref_category: Option<String>,
    #[serde(rename = "TxnInitiationMode", skip_serializing_if = "Option::is_none")]
    pub txn_initiation_mode: Option<String>,
}

/// Payload for Status 360 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status360ResponsePayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantChannelId")]
    pub merchant_channel_id: String,
    #[serde(rename = "merchantRequestId")]
    pub merchant_request_id: String,
    #[serde(rename = "gatewayTransactionId")]
    pub gateway_transaction_id: String,
    #[serde(rename = "gatewayReferenceId")]
    pub gateway_reference_id: Option<String>,
    #[serde(rename = "gatewayResponseCode")]
    pub gateway_response_code: String,
    #[serde(rename = "gatewayResponseMessage")]
    pub gateway_response_message: String,
    #[serde(rename = "gatewayResponseStatus")]
    pub gateway_response_status: String,
    pub amount: String,
    #[serde(rename = "bankAccountUniqueId", skip_serializing_if = "Option::is_none")]
    pub bank_account_unique_id: Option<String>,
    #[serde(rename = "bankCode", skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    #[serde(rename = "transactionTimestamp", skip_serializing_if = "Option::is_none")]
    pub transaction_timestamp: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<String>,
    #[serde(rename = "payeeVpa", skip_serializing_if = "Option::is_none")]
    pub payee_vpa: Option<String>,
    #[serde(rename = "customResponse", skip_serializing_if = "Option::is_none")]
    pub custom_response: Option<String>,
}

/// Payload for Refund 360 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund360ResponsePayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantChannelId")]
    pub merchant_channel_id: String,
    #[serde(rename = "refundRequestId")]
    pub refund_request_id: String,
    #[serde(rename = "originalMerchantRequestId")]
    pub original_merchant_request_id: String,
    #[serde(rename = "gatewayResponseCode")]
    pub gateway_response_code: String,
    #[serde(rename = "gatewayResponseStatus")]
    pub gateway_response_status: String,
    #[serde(rename = "gatewayResponseMessage")]
    pub gateway_response_message: String,
    #[serde(rename = "gatewayTransactionId")]
    pub gateway_transaction_id: String,
    #[serde(rename = "gatewayRefundTransactionId")]
    pub gateway_refund_transaction_id: String,
    #[serde(rename = "gatewayRefundReferenceId")]
    pub gateway_refund_reference_id: Option<String>,
    #[serde(rename = "refundAmount")]
    pub refund_amount: String,
    #[serde(rename = "refundType")]
    pub refund_type: String,
    #[serde(rename = "refundTimestamp")]
    pub refund_timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(rename = "adjFlag", skip_serializing_if = "Option::is_none")]
    pub adj_flag: Option<String>,
    #[serde(rename = "adjCode", skip_serializing_if = "Option::is_none")]
    pub adj_code: Option<String>,
    #[serde(rename = "reqAdjFlag", skip_serializing_if = "Option::is_none")]
    pub req_adj_flag: Option<String>,
    #[serde(rename = "reqAdjCode", skip_serializing_if = "Option::is_none")]
    pub req_adj_code: Option<String>,
}

/// Callback payload for payment completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayCallbackPayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantChannelId")]
    pub merchant_channel_id: String,
    #[serde(rename = "merchantRequestId")]
    pub merchant_request_id: String,
    #[serde(rename = "gatewayTransactionId")]
    pub gateway_transaction_id: String,
    #[serde(rename = "gatewayReferenceId")]
    pub gateway_reference_id: Option<String>,
    #[serde(rename = "gatewayResponseCode")]
    pub gateway_response_code: String,
    #[serde(rename = "gatewayResponseMessage")]
    pub gateway_response_message: String,
    pub amount: String,
    #[serde(rename = "bankCode", skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    #[serde(rename = "transactionTimestamp")]
    pub transaction_timestamp: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
}

/// Refund callback payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundCallbackPayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantChannelId")]
    pub merchant_channel_id: String,
    #[serde(rename = "refundRequestId")]
    pub refund_request_id: String,
    #[serde(rename = "originalMerchantRequestId")]
    pub original_merchant_request_id: String,
    #[serde(rename = "gatewayResponseCode")]
    pub gateway_response_code: String,
    #[serde(rename = "gatewayResponseStatus")]
    pub gateway_response_status: String,
    #[serde(rename = "refundAmount")]
    pub refund_amount: String,
    #[serde(rename = "refundType")]
    pub refund_type: String,
    #[serde(rename = "refundTimestamp")]
    pub refund_timestamp: String,
}

// Type aliases for specific responses
pub type RegisterIntentResponse = JuspayUpiApiResponse<RegisterIntentResponsePayload>;
pub type Status360Response = JuspayUpiApiResponse<Status360ResponsePayload>;
pub type Refund360Response = JuspayUpiApiResponse<Refund360ResponsePayload>;

// ============================================
// ENUMS
// ============================================

/// Transaction status from gateway response
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionStatus {
    Charged,
    Pending,
    Failed,
    Expired,
    Dropout,
    RequestNotFound,
}

impl TransactionStatus {
    pub fn from_gateway_code(code: &str) -> Self {
        match code {
            "00" => TransactionStatus::Charged,
            "01" => TransactionStatus::Pending,
            _ => TransactionStatus::Failed,
        }
    }

    pub fn from_outer_code(code: &str) -> Self {
        match code {
            "SUCCESS" => TransactionStatus::Pending,
            "FAILURE" => TransactionStatus::Failed,
            "REQUEST_NOT_FOUND" => TransactionStatus::RequestNotFound,
            "REQUEST_EXPIRED" => TransactionStatus::Expired,
            "DROPOUT" => TransactionStatus::Dropout,
            _ => TransactionStatus::Failed,
        }
    }
}

/// Refund status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefundStatus {
    Success,
    Pending,
    Failed,
    Deemed,
}

impl RefundStatus {
    pub fn from_udir_gateway_code(code: &str, _status: &str) -> Self {
        match code {
            "00" => RefundStatus::Success,
            "01" => RefundStatus::Pending,
            "JPREFD" => RefundStatus::Success,
            _ => RefundStatus::Failed,
        }
    }

    pub fn from_offline_gateway_code(code: &str, _status: &str) -> Self {
        match code {
            "00" | "01" => RefundStatus::Pending,
            "RB" | "BT" => RefundStatus::Pending,
            _ => RefundStatus::Failed,
        }
    }
}

// ============================================
// JWE RESPONSE TYPES
// ============================================

/// JWE encrypted response from Axis Bank
/// Contains the encrypted payload that needs to be decrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JweResponse {
    /// Base64url-encoded ciphertext (the encrypted payload)
    pub cipher_text: String,
    /// Base64url-encoded encrypted content encryption key
    pub encrypted_key: String,
    /// Base64url-encoded initialization vector
    pub iv: String,
    /// Base64url-encoded JWE protected header (contains alg, enc, kid)
    pub protected: String,
    /// Base64url-encoded authentication tag
    pub tag: String,
}

impl JweResponse {
    /// Check if the bytes appear to be a JWE response
    pub fn is_jwe_response(bytes: &[u8]) -> bool {
        if let Ok(json_str) = std::str::from_utf8(bytes) {
            json_str.contains("cipherText") && json_str.contains("encryptedKey")
        } else {
            false
        }
    }
}
