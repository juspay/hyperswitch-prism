//! Constants for Juspay UPI Merchant Stack

/// API endpoint paths (shared across all banks)
pub const REGISTER_INTENT_PATH: &str = "merchants/transactions/registerIntent";
pub const STATUS_360_PATH: &str = "merchants/transactions/status360";
pub const REFUND_360_PATH: &str = "merchants/transactions/refund360";

/// Transaction types
pub const TRANSACTION_TYPE_PAY: &str = "MERCHANT_CREDITED_VIA_PAY";

/// Flow types
pub const FLOW_TRANSACTION: &str = "TRANSACTION";

/// Refund types
pub const REFUND_TYPE_UDIR: &str = "UDIR";
pub const REFUND_TYPE_OFFLINE: &str = "OFFLINE";

/// UDIR adjustment codes and flags
pub const ADJ_FLAG_REF: &str = "REF";
pub const ADJ_CODE_GOODS_NOT_PROVIDED: &str = "1064";
pub const ADJ_CODE_DUPLICATE_TXN: &str = "1084";
pub const ADJ_CODE_ALTERNATE_PAYMENT: &str = "1063";
pub const ADJ_CODE_RETURNED_GOODS: &str = "1061";

/// Default intent expiry minutes
pub const DEFAULT_INTENT_EXPIRY_MINUTES: &str = "5";

/// Default TxnInitiationMode
pub const DEFAULT_TXN_INITIATION_MODE: &str = "00";

/// Gateway response codes
pub const GATEWAY_RESPONSE_CODE_SUCCESS: &str = "00";
pub const GATEWAY_RESPONSE_CODE_PENDING: &str = "01";
pub const GATEWAY_RESPONSE_CODE_DEEMED: &str = "JPREFD";

/// Outer API response codes
pub const RESPONSE_CODE_SUCCESS: &str = "SUCCESS";
pub const RESPONSE_CODE_FAILURE: &str = "FAILURE";
pub const RESPONSE_CODE_REQUEST_NOT_FOUND: &str = "REQUEST_NOT_FOUND";
pub const RESPONSE_CODE_REQUEST_EXPIRED: &str = "REQUEST_EXPIRED";
pub const RESPONSE_CODE_DROPOUT: &str = "DROPOUT";
pub const RESPONSE_CODE_BAD_REQUEST: &str = "BAD_REQUEST";
pub const RESPONSE_CODE_UNAUTHORIZED: &str = "UNAUTHORIZED";
pub const RESPONSE_CODE_INVALID_DATA: &str = "INVALID_DATA";
pub const RESPONSE_CODE_INVALID_MERCHANT: &str = "INVALID_MERCHANT";
pub const RESPONSE_CODE_DEVICE_FINGERPRINT_MISMATCH: &str = "DEVICE_FINGERPRINT_MISMATCH";
pub const RESPONSE_CODE_INTERNAL_SERVER_ERROR: &str = "INTERNAL_SERVER_ERROR";
pub const RESPONSE_CODE_INVALID_TRANSACTION_ID: &str = "INVALID_TRANSACTION_ID";
pub const RESPONSE_CODE_UNINITIATED_REQUEST: &str = "UNINITIATED_REQUEST";
pub const RESPONSE_CODE_INVALID_REFUND_AMOUNT: &str = "INVALID_REFUND_AMOUNT";
pub const RESPONSE_CODE_REQUEST_PENDING: &str = "REQUEST_PENDING";
pub const RESPONSE_CODE_SERVICE_UNAVAILABLE: &str = "SERVICE_UNAVAILABLE";
pub const RESPONSE_CODE_GATEWAY_TIMEOUT: &str = "GATEWAY_TIMEOUT";
pub const RESPONSE_CODE_DUPLICATE_REQUEST: &str = "DUPLICATE_REQUEST";

/// JWS algorithm
pub const JWS_ALG_RS256: &str = "RS256";

/// JWE algorithms (for future JWE support)
pub const JWE_ALG_RSA_OAEP_256: &str = "RSA-OAEP-256";
pub const JWE_ENC_A256GCM: &str = "A256GCM";

// ============================================
// HEADER CONSTANTS (from former headers.rs)
// ============================================

/// Content-Type header
pub const CONTENT_TYPE: &str = "content-type";

/// Accept header
pub const ACCEPT: &str = "accept";

/// Merchant ID header (x-merchant-id)
pub const X_MERCHANT_ID: &str = "x-merchant-id";

/// Merchant Channel ID header (x-merchant-channel-id)
pub const X_MERCHANT_CHANNEL_ID: &str = "x-merchant-channel-id";

/// Timestamp header (x-timestamp)
pub const X_TIMESTAMP: &str = "x-timestamp";

/// Routing ID header (jpupi-routing-id)
pub const JPUP_ROUTING_ID: &str = "jpupi-routing-id";

/// API Version header (x-api-version)
pub const X_API_VERSION: &str = "x-api-version";

/// Response signature header (x-response-signature)
pub const X_RESPONSE_SIGNATURE: &str = "x-response-signature";

/// Payload signature header for callbacks (x-merchant-payload-signature)
pub const X_MERCHANT_PAYLOAD_SIGNATURE: &str = "x-merchant-payload-signature";

/// Juspay KID header
pub const X_JUSPAY_KID: &str = "x-juspay-kid";
