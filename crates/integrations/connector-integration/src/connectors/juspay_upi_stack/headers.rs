//! Header constants for Juspay UPI Merchant Stack

use common_utils::errors::CustomResult;
use hyperswitch_masking::Maskable;

use crate::connectors::juspay_upi_stack::crypto::get_current_timestamp_ms;

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

/// Build the standard request headers for Juspay UPI Merchant Stack APIs.
///
/// This function constructs the common headers shared across all banks in the UPI Stack
/// (Axis Bank, YES Bank, Kotak, RBL, AU Bank, etc.). The headers are:
/// - content-type: application/json
/// - x-merchant-id
/// - x-merchant-channel-id
/// - x-timestamp: current Unix timestamp in milliseconds
/// - jpupi-routing-id: the transaction/request ID (value differs per flow)
///
/// Banks can use this function and optionally add additional headers (e.g., signature headers).
pub fn build_request_headers(
    merchant_id: &str,
    merchant_channel_id: &str,
    routing_id: &str,
) -> CustomResult<Vec<(String, Maskable<String>)>, domain_types::errors::IntegrationError> {
    let headers = vec![
        (
            CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        ),
        (X_MERCHANT_ID.to_string(), merchant_id.to_string().into()),
        (
            X_MERCHANT_CHANNEL_ID.to_string(),
            merchant_channel_id.to_string().into(),
        ),
        (X_TIMESTAMP.to_string(), get_current_timestamp_ms().into()),
        (JPUP_ROUTING_ID.to_string(), routing_id.to_string().into()),
    ];
    Ok(headers)
}
