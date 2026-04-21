//! Constants for Juspay UPI Merchant Stack

/// API endpoint paths (shared across all banks)
pub const REGISTER_INTENT_PATH: &str = "merchants/transactions/registerIntent";
pub const STATUS_360_PATH: &str = "merchants/transactions/status360";
pub const REFUND_360_PATH: &str = "merchants/transactions/refund360";

/// Transaction types
pub const TRANSACTION_TYPE_PAY: &str = "MERCHANT_CREDITED_VIA_PAY";
pub const TRANSACTION_TYPE_COLLECT: &str = "MERCHANT_CREDITED_VIA_COLLECT";

/// Flow types
pub const FLOW_TRANSACTION: &str = "TRANSACTION";
pub const FLOW_MANDATE: &str = "MANDATE";

/// Refund types
pub const REFUND_TYPE_UDIR: &str = "UDIR";
pub const REFUND_TYPE_OFFLINE: &str = "OFFLINE";
pub const REFUND_TYPE_ONLINE: &str = "ONLINE";

/// UDIR adjustment codes and flags
pub const ADJ_FLAG_REF: &str = "REF";
pub const ADJ_CODE_GOODS_NOT_PROVIDED: &str = "1064";
pub const ADJ_CODE_DUPLICATE_TXN: &str = "1084";
pub const ADJ_CODE_ALTERNATE_PAYMENT: &str = "1063";
pub const ADJ_CODE_RETURNED_GOODS: &str = "1061";

/// Reference category for payments
pub const REF_CATEGORY_INVOICE: &str = "02";

/// Default intent expiry minutes
pub const DEFAULT_INTENT_EXPIRY_MINUTES: &str = "10";

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

/// JWS algorithm
pub const JWS_ALG_RS256: &str = "RS256";

/// JWE algorithms (for future JWE support)
pub const JWE_ALG_RSA_OAEP_256: &str = "RSA-OAEP-256";
pub const JWE_ENC_A256GCM: &str = "A256GCM";
