//! Webhook signature generation for testing
//!
//! This module provides signature generation functions for various connectors
//! to enable webhook testing via EventService.HandleEvent
//!
//! Note: This module intentionally uses simple error handling (String-based errors)
//! to avoid adding extra dependencies to the integration-tests crate.

use std::fmt::Write;

/// Connector-specific inputs a signature scheme needs beyond the payload and
/// secret. All fields are optional: a new scheme adds a field here and a match
/// arm below, rather than a branch in the caller.
#[derive(Debug, Default, Clone)]
pub struct SignatureContext<'a> {
    pub timestamp: Option<i64>,
    /// Request URI/path, for schemes that hash it in (e.g. phonepe).
    pub api_path: Option<&'a str>,
    /// Key/version index appended to the signature, for schemes that use one
    /// (e.g. phonepe's `###<key_index>` suffix). Defaults to "1" if the
    /// scheme requires one and none is supplied.
    pub key_index: Option<&'a str>,
}

/// Generate webhook signature for a given connector.
///
/// `payload` must be the exact bytes the connector's own verification code
/// hashes. For most connectors that is the raw request body; for phonepe it is
/// the inner base64 `response` string, which the caller must extract first.
///
/// Returns the signature string for the connector's signature header.
pub fn generate_signature(
    connector: &str,
    payload: &[u8],
    secret: &str,
    ctx: &SignatureContext<'_>,
) -> Result<String, String> {
    match connector {
        "stripe" => generate_stripe_signature(payload, secret, ctx.timestamp),
        "adyen" => generate_adyen_signature(),
        "authorizedotnet" => generate_authorizedotnet_signature(payload, secret),
        "checkout" => generate_checkout_signature(payload, secret),
        "paypal" => generate_paypal_signature(payload, secret),
        "phonepe" => generate_phonepe_signature(payload, secret, ctx),
        "ecommpay" => generate_ecommpay_signature(payload, secret),
        "cybersource" => generate_cybersource_signature(payload, secret, ctx.timestamp),
        _ => Err(format!("Unsupported connector: {}", connector)),
    }
}

/// Generate an Ecommpay callback signature (the value of the body's own `signature` field).
///
/// `payload` is the JSON body without `signature`. Ecommpay's documented algorithm
/// (developers.ecommpay.com, "Signature generation and verification"), re-implemented here
/// independently of the connector so the harness stays an oracle:
///   1. flatten every leaf to `parent:...:key:value` (array elements keyed by zero-based
///      index, booleans as `1`/`0`, empty strings kept, empty arrays skipped);
///   2. sort the strings in natural order and join them with `;`;
///   3. Base64(HMAC-SHA512(secret, joined)).
fn generate_ecommpay_signature(payload: &[u8], secret: &str) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use hmac::{Hmac, Mac};
    use sha2::Sha512;

    fn flatten(prefix: &str, value: &serde_json::Value, out: &mut Vec<String>) {
        let join = |key: &str| {
            if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{prefix}:{key}")
            }
        };
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    flatten(&join(key), child, out);
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    flatten(&join(&index.to_string()), child, out);
                }
            }
            serde_json::Value::Bool(flag) => {
                out.push(format!("{prefix}:{}", if *flag { 1 } else { 0 }))
            }
            serde_json::Value::Number(number) => out.push(format!("{prefix}:{number}")),
            serde_json::Value::String(text) => out.push(format!("{prefix}:{text}")),
            serde_json::Value::Null => out.push(format!("{prefix}:")),
        }
    }

    /// Natural order: where both strings have a run of digits at the same position, the runs
    /// compare by numeric value; every other position compares by byte.
    fn natural_order(left: &str, right: &str) -> std::cmp::Ordering {
        fn digit_run(bytes: &mut std::iter::Peekable<std::str::Bytes<'_>>) -> String {
            let mut run = String::new();
            while let Some(digit) = bytes.next_if(u8::is_ascii_digit) {
                run.push(char::from(digit));
            }
            run
        }
        let (mut l, mut r) = (left.bytes().peekable(), right.bytes().peekable());
        loop {
            let ordering = match (l.peek().copied(), r.peek().copied()) {
                (None, None) => return std::cmp::Ordering::Equal,
                (None, Some(_)) => return std::cmp::Ordering::Less,
                (Some(_), None) => return std::cmp::Ordering::Greater,
                (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                    let (l_run, r_run) = (digit_run(&mut l), digit_run(&mut r));
                    let (l_value, r_value) =
                        (l_run.trim_start_matches('0'), r_run.trim_start_matches('0'));
                    l_value
                        .len()
                        .cmp(&r_value.len())
                        .then(l_value.cmp(r_value))
                        .then(l_run.len().cmp(&r_run.len()))
                }
                (Some(a), Some(b)) => {
                    l.next();
                    r.next();
                    a.cmp(&b)
                }
            };
            if ordering != std::cmp::Ordering::Equal {
                return ordering;
            }
        }
    }

    let mut body: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| format!("ecommpay signature payload must be JSON: {e}"))?;
    if let Some(object) = body.as_object_mut() {
        object.remove("signature");
    }
    let mut leaves = Vec::new();
    flatten("", &body, &mut leaves);
    leaves.sort_by(|left, right| natural_order(left, right));
    let preimage = leaves.join(";");

    type HmacSha512 = Hmac<Sha512>;
    let mut mac = HmacSha512::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {e}"))?;
    mac.update(preimage.as_bytes());
    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}

/// Key id placed in the `keyId` parameter of a generated Cybersource
/// `v-c-signature` header. It is a fixture value: the connector verifies with
/// the single configured webhook secret and does not look a key up by id.
const CYBERSOURCE_FIXTURE_KEY_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Generate Cybersource webhook `v-c-signature` header value.
///
/// Cybersource uses: t=<timestamp>;keyId=<key id>;sig=<hmac_sha256_base64>
/// Message format: {timestamp}.{payload}
/// The HMAC key is the Base64-decoded digital-signature key (the webhook secret
/// is stored in its Base64 form).
fn generate_cybersource_signature(
    payload: &[u8],
    secret: &str,
    timestamp: Option<i64>,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // Cybersource timestamps are epoch milliseconds; use the current time if not provided.
    let timestamp = match timestamp {
        Some(timestamp) => timestamp,
        None => {
            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| format!("Failed to read system time: {e}"))?;
            i64::try_from(elapsed.as_millis())
                .map_err(|e| format!("Timestamp out of range: {e}"))?
        }
    };

    let key = STANDARD
        .decode(secret)
        .map_err(|e| format!("Cybersource webhook secret must be Base64: {e}"))?;

    let mut mac =
        HmacSha256::new_from_slice(&key).map_err(|e| format!("Failed to create HMAC: {e}"))?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let signature_b64 = STANDARD.encode(mac.finalize().into_bytes());

    Ok(format!(
        "t={timestamp};keyId={CYBERSOURCE_FIXTURE_KEY_ID};sig={signature_b64}"
    ))
}

/// Generate PhonePe webhook X-VERIFY signature.
///
/// PhonePe's checksum, matching connectors/phonepe/transformers.rs's
/// compute_phonepe_webhook_checksum exactly:
///   sha256(base64_body + api_path + salt_key) + "###" + key_index
fn generate_phonepe_signature(
    inner_base64_body: &[u8],
    salt_key: &str,
    ctx: &SignatureContext<'_>,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let base64_body = std::str::from_utf8(inner_base64_body)
        .map_err(|e| format!("phonepe signature payload must be UTF-8: {e}"))?;
    let api_path = ctx.api_path.unwrap_or("");
    let key_index = ctx.key_index.unwrap_or("1");

    let checksum_input = format!("{base64_body}{api_path}{salt_key}");
    let hash_bytes = Sha256::digest(checksum_input.as_bytes());

    let mut hex_hash = String::with_capacity(hash_bytes.len() * 2);
    for byte in hash_bytes {
        write!(&mut hex_hash, "{byte:02x}").map_err(|e| format!("Failed to write hex: {e}"))?;
    }

    Ok(format!("{hex_hash}###{key_index}"))
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

/// Generate Checkout.com webhook signature
///
/// Checkout.com uses HMAC-SHA256 with lowercase hex encoding on the raw body.
/// Header: cko-signature: <hex>
fn generate_checkout_signature(payload: &[u8], secret: &str) -> Result<String, String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(payload);
    let signature_bytes = mac.finalize().into_bytes();

    let mut hex_signature = String::with_capacity(signature_bytes.len() * 2);
    for byte in signature_bytes {
        write!(&mut hex_signature, "{:02x}", byte)
            .map_err(|e| format!("Failed to write hex: {}", e))?;
    }

    Ok(hex_signature)
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
