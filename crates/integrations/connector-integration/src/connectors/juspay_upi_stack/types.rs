use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

/// Authentication configuration for Juspay UPI Stack connectors
#[derive(Debug, Clone)]
pub struct JuspayUpiAuthConfig {
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

/// Flow type for Register Intent API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RegisterIntentFlowType {
    Transaction,
    Mandate,
}

/// Request payload for Register Intent API (Authorize flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterIntentRequest {
    pub merchant_request_id: String,
    pub upi_request_id: String,
    pub amount: String,
    pub flow: RegisterIntentFlowType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_request_expiry_minutes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_url: Option<String>,
    pub iat: String,
}

/// Transaction type for Status 360 API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status360TransactionType {
    MerchantCreditedViaPay,
    MerchantCreditedViaCollect,
}

/// Request payload for Status 360 API (PSync flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status360Request {
    pub merchant_request_id: String,
    pub transaction_type: Status360TransactionType,
    pub iat: String,
}

/// Refund type for Refund 360 API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Refund360Type {
    Udir,
    Online,
    Offline,
}

/// Adjustment code for UDIR refunds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdjustmentCode {
    #[serde(rename = "1064")]
    GoodsNotProvided,
    #[serde(rename = "1084")]
    DuplicateTxn,
    #[serde(rename = "1063")]
    AlternatePayment,
    #[serde(rename = "1061")]
    ReturnedGoods,
}

/// Adjustment flag for UDIR refunds
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AdjustmentFlag {
    Ref,
}

/// Request payload for Refund 360 API (Refund/RSync flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refund360Request {
    pub original_merchant_request_id: String,
    pub refund_request_id: String,
    pub refund_type: Refund360Type,
    pub refund_amount: String,
    pub remarks: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adj_code: Option<AdjustmentCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adj_flag: Option<AdjustmentFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_refund_vpa: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_transaction_timestamp: Option<String>,
    pub iat: String,
}

/// Generic outer response structure for all APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JuspayUpiApiResponse<T> {
    pub status: String,
    pub response_code: OuterResponseCode,
    pub response_message: String,
    pub payload: Option<T>,
}

/// Payload for Register Intent response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterIntentResponsePayload {
    pub merchant_request_id: String,
    pub gateway_transaction_id: String,
    pub order_id: String,
    pub payee_vpa: String,
    pub payee_name: String,
    pub payee_mcc: String,
    pub amount: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_url: Option<String>,
    /// TxnInitiationMode is PascalCase, not camelCase
    #[serde(rename = "TxnInitiationMode", skip_serializing_if = "Option::is_none")]
    pub txn_initiation_mode: Option<String>,
}

/// Payload for Status 360 response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status360ResponsePayload {
    pub gateway_transaction_id: String,
    pub gateway_response_code: String,
}

/// Payload for Refund 360 response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refund360ResponsePayload {
    pub refund_request_id: String,
    pub gateway_response_code: String,
    pub gateway_response_status: String,
    pub refund_type: String,
}

/// Callback payload for payment completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayCallbackPayload {
    pub merchant_id: String,
    pub merchant_channel_id: String,
    pub merchant_request_id: String,
    pub gateway_transaction_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway_reference_id: Option<String>,
    pub gateway_response_code: String,
    pub gateway_response_message: String,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_code: Option<String>,
    pub transaction_timestamp: String,
    /// 'type' is a reserved keyword
    #[serde(rename = "type")]
    pub transaction_type: String,
}

/// Refund callback payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefundCallbackPayload {
    pub merchant_id: String,
    pub merchant_channel_id: String,
    pub refund_request_id: String,
    pub original_merchant_request_id: String,
    pub gateway_response_code: String,
    pub gateway_response_status: String,
    pub refund_amount: String,
    pub refund_type: String,
    pub refund_timestamp: String,
}

// Type aliases for specific responses
pub type RegisterIntentResponse = JuspayUpiApiResponse<RegisterIntentResponsePayload>;
pub type Status360Response = JuspayUpiApiResponse<Status360ResponsePayload>;
pub type Refund360Response = JuspayUpiApiResponse<Refund360ResponsePayload>;

// ============================================
// ENUMS
// ============================================

/// Outer API response codes from Juspay UPI Merchant Stack
/// Covers all documented responseCode values from the Codes Guide
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OuterResponseCode {
    Success,                   // Transaction/payment was successful
    Failure,                   // Transaction/payment failed
    RequestNotFound,           // Transaction not found in system
    RequestExpired,            // Transaction expired (user dropout)
    Dropout,                   // User dropped out during UPI flow
    RequestPending,            // Transaction still pending (not terminal state)
    BadRequest,                // Bad request - missing mandatory parameter or regex mismatch
    InvalidData,               // Invalid data - mandatory keys present but incorrect values
    Unauthorized,              // Unauthorized - signature validation failed
    InvalidMerchant,           // Invalid merchant ID or channel ID
    ServiceUnavailable,        // Third-party services unreachable (NPCI, bank systems)
    GatewayTimeout,            // Timeout from NPCI
    DuplicateRequest,          // Duplicate merchantRequestId or upiRequestId
    DeviceFingerprintMismatch, // Device fingerprint validation failed
    InternalServerError,       // Internal server error
    InvalidTransactionId,      // Invalid transaction ID for refund
    UninitiatedRequest,        // Original transaction was not successful
    InvalidRefundAmount,       // Refund amount exceeds original transaction
}

impl OuterResponseCode {
    /// Check if this is a terminal status (no further state changes expected)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Success | Self::Failure | Self::RequestExpired | Self::Dropout
        )
    }

    /// Check if this represents a pending/in-progress transaction
    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            Self::RequestPending
                | Self::RequestNotFound
                | Self::ServiceUnavailable
                | Self::GatewayTimeout
        )
    }

    /// Check if this represents a failure response code
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::Failure
                | Self::BadRequest
                | Self::InvalidData
                | Self::Unauthorized
                | Self::InvalidMerchant
                | Self::DeviceFingerprintMismatch
                | Self::InternalServerError
                | Self::InvalidTransactionId
                | Self::UninitiatedRequest
                | Self::InvalidRefundAmount
                | Self::DuplicateRequest
        )
    }
}

/// Gateway response codes from NPCI/PSP
/// These are the gatewayResponseCode values in the payload
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GatewayResponseCode {
    Success,           // Transaction successful
    Pending,           // Transaction pending
    Deemed,            // Deemed success (RB code)
    Declined,          // Transaction declined
    Expired,           // Collect request expired
    BeneAddrIncorrect, // Beneficiary payment address incorrect
    IntentExpired,     // Merchant intent expired
    ValidationError,   // Validation error (amount mismatch, tid/tr change)
    MandateRevoked,    // Mandate revoked
    MandatePaused,     // Mandate paused
    MandateCompleted,  // Mandate completed
    MandateDeclined,   // Mandate declined by payer
    MandateExpired,    // Mandate expired
    Unknown(String),   // Unknown gateway code
}

impl GatewayResponseCode {
    /// Parse gateway response code from a string
    #[must_use]
    pub fn parse(code: &str) -> Self {
        match code {
            "00" => Self::Success,
            "01" => Self::Pending,
            "RB" => Self::Deemed,
            "ZA" => Self::Declined,
            "U69" => Self::Expired,
            "ZH" => Self::BeneAddrIncorrect,
            "X1" => Self::IntentExpired,
            "YG" => Self::ValidationError,
            "JPMR" => Self::MandateRevoked,
            "JPMP" => Self::MandatePaused,
            "JPMC" => Self::MandateCompleted,
            "JPMD" => Self::MandateDeclined,
            "JPMX" => Self::MandateExpired,
            unknown => Self::Unknown(unknown.to_string()),
        }
    }
}

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

/// Refund status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefundStatus {
    Success,
    Pending,
    Failed,
    Deemed,
}

impl RefundStatus {
    /// Parse refund status from UDIR gateway response code
    /// Uses GatewayResponseCode enum for exhaustive matching
    pub fn from_udir_gateway_code(code: &str, _status: &str) -> Self {
        let gateway = GatewayResponseCode::parse(code);
        match gateway {
            GatewayResponseCode::Success => Self::Success,
            GatewayResponseCode::Pending => Self::Pending,
            GatewayResponseCode::Deemed => Self::Deemed,
            GatewayResponseCode::Declined
            | GatewayResponseCode::Expired
            | GatewayResponseCode::BeneAddrIncorrect
            | GatewayResponseCode::IntentExpired
            | GatewayResponseCode::ValidationError
            | GatewayResponseCode::MandateRevoked
            | GatewayResponseCode::MandatePaused
            | GatewayResponseCode::MandateCompleted
            | GatewayResponseCode::MandateDeclined
            | GatewayResponseCode::MandateExpired
            | GatewayResponseCode::Unknown(_) => Self::Failed,
        }
    }

    /// Parse refund status from offline/online gateway response code
    /// Uses GatewayResponseCode enum for exhaustive matching
    pub fn from_offline_gateway_code(code: &str, _status: &str) -> Self {
        let gateway = GatewayResponseCode::parse(code);
        match gateway {
            GatewayResponseCode::Success
            | GatewayResponseCode::Pending
            | GatewayResponseCode::Deemed => Self::Pending,
            GatewayResponseCode::Declined
            | GatewayResponseCode::Expired
            | GatewayResponseCode::BeneAddrIncorrect
            | GatewayResponseCode::IntentExpired
            | GatewayResponseCode::ValidationError
            | GatewayResponseCode::MandateRevoked
            | GatewayResponseCode::MandatePaused
            | GatewayResponseCode::MandateCompleted
            | GatewayResponseCode::MandateDeclined
            | GatewayResponseCode::MandateExpired
            | GatewayResponseCode::Unknown(_) => Self::Failed,
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
