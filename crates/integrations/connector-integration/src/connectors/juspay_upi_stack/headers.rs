//! Header constants for Juspay UPI Merchant Stack

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
