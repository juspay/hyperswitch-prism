//! Constants for PhonePe connector

// ===== API ENDPOINTS =====
pub const API_PAY_ENDPOINT: &str = "pg/v1/pay";
pub const API_STATUS_ENDPOINT: &str = "pg/v1/status";

// ===== IRCTC MERCHANT-BASED ENDPOINTS =====
pub const API_IRCTC_PAY_ENDPOINT: &str = "pg/v1/irctc-pay";
pub const API_IRCTC_STATUS_ENDPOINT: &str = "pg/v1/irctc-pay/status";
pub const IRCTC_IDENTIFIER: &str = "Disable_IRCTC";

// ===== UPI INSTRUMENT TYPES =====
pub const UPI_INTENT: &str = "UPI_INTENT";
pub const UPI_COLLECT: &str = "UPI_COLLECT";
pub const UPI_QR: &str = "UPI_QR";
pub const UPI: &str = "UPI";

// ===== ACCOUNT TYPES =====
pub const ACCOUNT_TYPE_CREDIT: &str = "CREDIT";
pub const ACCOUNT_TYPE_SAVINGS: &str = "SAVINGS";

// ===== CARD NETWORKS =====
pub const CARD_NETWORK_RUPAY: &str = "RUPAY";

// ===== RESPONSE CODES =====
pub const RESPONSE_CODE_CREDIT_ACCOUNT_NOT_ALLOWED: &str = "CREDIT_ACCOUNT_NOT_ALLOWED_FOR_SENDER";
pub const RESPONSE_CODE_PAY0071: &str = "PAY0071";

// ===== DEFAULT VALUES =====
pub const DEFAULT_KEY_INDEX: &str = "1";
pub const DEFAULT_DEVICE_OS: &str = "Android";
pub const DEFAULT_IP: &str = "127.0.0.1";
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0";

// ===== CHECKSUM =====
pub const CHECKSUM_SEPARATOR: &str = "###";

// ===== CONTENT TYPES =====
pub const APPLICATION_JSON: &str = "application/json";
