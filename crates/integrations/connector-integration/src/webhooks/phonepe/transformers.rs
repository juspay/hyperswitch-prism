use base64::Engine;
use common_utils::{
    crypto::{self, GenerateDigest},
    types::MinorUnit,
};
use domain_types::{connector_types::EventType, errors::WebhookError};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

// ===== WEBHOOK STRUCTURES =====

/// Top-level PhonePe webhook request body: contains the base64-encoded payload
#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeWebhookRequest {
    pub response: String,
}

/// Decoded webhook payload from PhonePe (PhonePeV2UpiWebhookRequestData)
#[derive(Debug, Deserialize, Serialize)]
pub struct PhonepeWebhookPayload {
    #[serde(rename = "merchantId")]
    pub merchant_id: String,
    #[serde(rename = "merchantTransactionId")]
    pub merchant_transaction_id: String,
    #[serde(rename = "transactionId")]
    pub transaction_id: String,
    pub amount: Option<MinorUnit>,
    pub state: String,
    #[serde(rename = "responseCode")]
    pub response_code: Option<String>,
    #[serde(rename = "paymentInstrument")]
    pub payment_instrument: Option<serde_json::Value>,
}

/// Map PhonePe webhook state string to UCS EventType
pub fn map_phonepe_webhook_state_to_event_type(state: &str) -> EventType {
    match state {
        "PAYMENT_SUCCESS" | "SUCCESS" => EventType::PaymentIntentSuccess,
        "PAYMENT_PENDING" | "PENDING" => EventType::PaymentIntentProcessing,
        "PAYMENT_ERROR" | "FAILED" | "ERROR" | "PAYMENT_CANCELLED" | "PAYMENT_DECLINED"
        | "TIMED_OUT" => EventType::PaymentIntentFailure,
        _ => EventType::PaymentIntentProcessing,
    }
}

/// Map PhonePe webhook state string to UCS AttemptStatus
pub fn map_phonepe_webhook_state_to_attempt_status(state: &str) -> common_enums::AttemptStatus {
    match state {
        "PAYMENT_SUCCESS" | "SUCCESS" => common_enums::AttemptStatus::Charged,
        "PAYMENT_PENDING" | "PENDING" => common_enums::AttemptStatus::Pending,
        "PAYMENT_ERROR" | "FAILED" | "ERROR" | "PAYMENT_CANCELLED" | "PAYMENT_DECLINED"
        | "TIMED_OUT" => common_enums::AttemptStatus::Failure,
        _ => common_enums::AttemptStatus::Pending,
    }
}

/// Decode and parse the PhonePe webhook body (base64-encoded JSON payload)
pub fn decode_phonepe_webhook_payload(
    raw_body: &[u8],
) -> Result<PhonepeWebhookPayload, error_stack::Report<WebhookError>> {
    use common_utils::ext_traits::ByteSliceExt;

    let webhook_request: PhonepeWebhookRequest = raw_body
        .parse_struct("PhonepeWebhookRequest")
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;

    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&webhook_request.response)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Failed to base64-decode PhonePe webhook response field")?;

    let payload: PhonepeWebhookPayload = serde_json::from_slice(&decoded_bytes)
        .change_context(WebhookError::WebhookBodyDecodingFailed)
        .attach_printable("Failed to parse decoded PhonePe webhook payload as JSON")?;

    Ok(payload)
}

/// Compute expected PhonePe webhook HMAC checksum:
/// sha256(base64Body + apiPath + saltKey) + "###" + keyIndex
pub fn compute_phonepe_webhook_checksum(
    base64_body: &str,
    api_path: &str,
    salt_key: &Secret<String>,
    key_index: &str,
) -> Result<String, error_stack::Report<WebhookError>> {
    use hyperswitch_masking::PeekInterface;

    let checksum_input = format!("{}{}{}", base64_body, api_path, salt_key.peek());
    let sha256 = crypto::Sha256;
    let hash_bytes = sha256
        .generate_digest(checksum_input.as_bytes())
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    let hash = hash_bytes.iter().fold(String::new(), |mut acc, byte| {
        use std::fmt::Write;
        let _ = write!(&mut acc, "{byte:02x}");
        acc
    });
    Ok(format!(
        "{}{}{}",
        hash,
        crate::connectors::phonepe::constants::CHECKSUM_SEPARATOR,
        key_index
    ))
}
