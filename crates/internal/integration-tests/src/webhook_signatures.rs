//! Webhook signature generation for testing
//!
//! This module provides signature generation functions for various connectors
//! to enable webhook testing via EventService.HandleEvent
//!
//! Note: This module intentionally uses simple error handling (String-based errors)
//! to avoid adding extra dependencies to the integration-tests crate.

use std::fmt::Write;

/// Generate webhook signature for a given connector
///
/// Returns the signature string that should be placed in the appropriate header
pub fn generate_signature(
    connector: &str,
    payload: &[u8],
    secret: &str,
    timestamp: Option<i64>,
) -> Result<String, String> {
    match connector {
        "stripe" => generate_stripe_signature(payload, secret, timestamp),
        "adyen" => generate_adyen_signature(),
        "authorizedotnet" => generate_authorizedotnet_signature(payload, secret),
        "paypal" => generate_paypal_signature(payload, secret),
        _ => Err(format!("Unsupported connector: {}", connector)),
    }
}

/// Generate Stripe webhook signature
///
/// Stripe uses: t=<timestamp>,v1=<hmac_sha256_hex>
/// Message format: {timestamp}.{payload}
fn generate_stripe_signature(
    payload: &[u8],
    secret: &str,
    timestamp: Option<i64>,
) -> Result<String, String> {
    // Use current timestamp if not provided
    let timestamp = timestamp.unwrap_or_else(|| {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .unwrap_or(0)
    });

    // Stripe's signed_payload = timestamp.body
    let payload_str = std::str::from_utf8(payload)
        .map_err(|e| format!("Failed to convert payload to UTF-8: {}", e))?;

    let signed_payload = format!("{}.{}", timestamp, payload_str);

    // Compute HMAC-SHA256 using hmac/sha2 crates (available via dependencies)
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(signed_payload.as_bytes());
    let signature_bytes = mac.finalize().into_bytes();

    // Convert to hex
    let mut hex_signature = String::with_capacity(signature_bytes.len() * 2);
    for byte in signature_bytes {
        write!(&mut hex_signature, "{:02x}", byte)
            .map_err(|e| format!("Failed to write hex: {}", e))?;
    }

    // Stripe format: t=timestamp,v1=signature
    Ok(format!("t={},v1={}", timestamp, hex_signature))
}

/// Generate Adyen webhook signature
///
/// Adyen's signature is embedded in the webhook body itself (additionalData.hmacSignature)
/// For testing purposes, we return a marker indicating the signature needs to be
/// computed and injected into the body
fn generate_adyen_signature() -> Result<String, String> {
    // Adyen's signature is embedded in the webhook body itself
    // For testing, the signature would need to be pre-computed and
    // included in the scenario.json payload
    Ok("ADYEN_SIGNATURE_IN_BODY".to_string())
}

/// Generate Authorize.Net webhook signature
///
/// Authorize.Net uses HMAC-SHA512 with lowercase hex encoding
/// Header: X-ANET-Signature: sha512=<signature>
fn generate_authorizedotnet_signature(payload: &[u8], secret: &str) -> Result<String, String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha512;

    type HmacSha512 = Hmac<Sha512>;

    let mut mac = HmacSha512::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(payload);
    let signature_bytes = mac.finalize().into_bytes();

    // Convert to lowercase hex
    let mut hex_signature = String::with_capacity(signature_bytes.len() * 2);
    for byte in signature_bytes {
        write!(&mut hex_signature, "{:02x}", byte)
            .map_err(|e| format!("Failed to write hex: {}", e))?;
    }

    Ok(format!("sha512={}", hex_signature))
}

/// Generate PayPal webhook signature
///
/// PayPal uses HMAC-SHA256 with base64 encoding
fn generate_paypal_signature(payload: &[u8], secret: &str) -> Result<String, String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(payload);
    let signature_bytes = mac.finalize().into_bytes();

    // Base64 encode
    use base64::{engine::general_purpose::STANDARD, Engine};
    let signature_b64 = STANDARD.encode(signature_bytes);

    Ok(signature_b64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stripe_signature_generation() {
        let payload = b"{\"id\":\"evt_test\",\"type\":\"payment_intent.succeeded\"}";
        let secret = "whsec_test_secret";
        let timestamp = Some(1234567890i64);

        let signature = generate_stripe_signature(payload, secret, timestamp)
            .expect("Failed to generate Stripe signature");

        assert!(signature.starts_with("t=1234567890,v1="));
        assert!(signature.len() > 20);
    }

    #[test]
    fn test_authorizedotnet_signature_generation() {
        let payload = b"{\"eventType\":\"net.authorize.payment.authorization.created\"}";
        let secret = "test_secret";

        let signature = generate_authorizedotnet_signature(payload, secret)
            .expect("Failed to generate Authorize.Net signature");

        assert!(signature.starts_with("sha512="));
        assert!(signature.len() > 100); // SHA512 is 128 hex chars
    }
}
