//! Cryptographic utilities for Juspay UPI Merchant Stack
//!
//! This module provides JWS signing and verification, and optional JWE
//! encryption/decryption for banks that require it.

use base64::Engine;
use common_utils::consts::{BASE64_ENGINE_URL_SAFE_NO_PAD};
use domain_types::errors::IntegrationError;
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use ring::{rand, signature};

use crate::connectors::juspay_upi_stack::types::JwsObject;

/// Errors that can occur during crypto operations
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Failed to sign JWS")]
    SigningFailed,
    #[error("Failed to verify JWS signature")]
    VerificationFailed,
    #[error("Invalid key format")]
    InvalidKey,
    #[error("Base64 decoding failed")]
    Base64DecodeFailed,
    #[error("JSON serialization failed")]
    JsonSerializationFailed,
}

/// Sign a payload using JWS RS256
///
/// This creates a JWS object with:
/// - protected: base64url-encoded header containing alg and kid
/// - payload: base64url-encoded payload
/// - signature: RS256 signature of "<protected>.<payload>"
///
/// # Arguments
/// * `payload` - The JSON payload to sign
/// * `private_key_pem` - RSA private key in PEM format (PKCS#8)
/// * `kid` - Key ID to include in the JWS header
///
/// # Returns
/// A JwsObject containing the signed JWS components
pub fn sign_jws(
    payload: &str,
    private_key_pem: &Secret<String>,
    kid: &str,
) -> Result<JwsObject, error_stack::Report<IntegrationError>> {
    // Build the protected header
    let protected_header = serde_json::json!({
        "alg": "RS256",
        "kid": kid,
    });

    // Base64url encode the protected header
    let protected_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(protected_header.to_string().as_bytes());

    // Base64url encode the payload
    let payload_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(payload.as_bytes());

    // Create the signing input: "<protected_b64>.<payload_b64>"
    let signing_input = format!("{}.{}", protected_b64, payload_b64);

    // Sign using RSA-SHA256 (PKCS#1 v1.5 padding)
    let signature_bytes = sign_rsa_sha256(signing_input.as_bytes(), private_key_pem)?;

    // Base64url encode the signature
    let signature_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(&signature_bytes);

    Ok(JwsObject {
        protected: protected_b64,
        payload: payload_b64,
        signature: signature_b64,
    })
}

/// Sign data using RSA-SHA256 with PKCS#1 v1.5 padding
fn sign_rsa_sha256(
    data: &[u8],
    private_key_pem: &Secret<String>,
) -> Result<Vec<u8>, error_stack::Report<IntegrationError>> {
    // Parse the private key from PEM
    let pem_bytes = private_key_pem.peek().as_bytes();

    // Extract the key from PEM (handle both PKCS#8 and PKCS#1 formats)
    let key_bytes = extract_key_from_pem(pem_bytes)
        .change_context(IntegrationError::InvalidConnectorConfig {
            config: "merchant_private_key",
            context: Default::default(),
        })
        .attach_printable("Failed to extract key from PEM")?;

    // Try to parse as PKCS#8 first, then PKCS#1
    let key_pair_result = signature::RsaKeyPair::from_pkcs8(&key_bytes);
    let key_pair = match key_pair_result {
        Ok(kp) => kp,
        Err(_) => {
            let pkcs8_bytes = convert_pkcs1_to_pkcs8(&key_bytes)
                .change_context(IntegrationError::InvalidConnectorConfig {
                    config: "merchant_private_key",
                    context: Default::default(),
                })
                .attach_printable("Failed to convert PKCS#1 to PKCS#8")?;
            signature::RsaKeyPair::from_pkcs8(&pkcs8_bytes)
                .map_err(|_| {
                    IntegrationError::InvalidConnectorConfig {
                        config: "merchant_private_key",
                        context: Default::default(),
                    }
                })
                .attach_printable("Failed to parse RSA private key")?
        }
    };

    // Sign using RSA-PKCS1-v1.5-SHA256
    let rng = rand::SystemRandom::new();
    let mut signature = vec![0u8; key_pair.public().modulus_len()];

    key_pair
        .sign(&signature::RSA_PKCS1_SHA256, &rng, data, &mut signature)
        .map_err(|_| {
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            }
        })
        .attach_printable("Failed to sign data with RSA")?;

    Ok(signature)
}

/// Extract base64-encoded key material from PEM
fn extract_key_from_pem(pem: &[u8]) -> Result<Vec<u8>, error_stack::Report<CryptoError>> {
    let pem_str = std::str::from_utf8(pem)
        .change_context(CryptoError::InvalidKey)
        .attach_printable("PEM is not valid UTF-8")?;

    // Remove header and footer lines
    let base64_content: String = pem_str
        .lines()
        .filter(|line| !line.starts_with("-----") && !line.trim().is_empty())
        .collect();

    // Decode base64
    BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(&base64_content)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&base64_content))
        .change_context(CryptoError::Base64DecodeFailed)
        .attach_printable("Failed to decode PEM base64 content")
}

/// Convert PKCS#1 RSA private key to PKCS#8 format
fn convert_pkcs1_to_pkcs8(pkcs1_bytes: &[u8]) -> Result<Vec<u8>, error_stack::Report<CryptoError>> {
    // PKCS#8 wrapper for RSA private key
    // See RFC 5208 and RFC 5958

    // OID for rsaEncryption: 1.2.840.113549.1.1.1
    const RSA_OID: &[u8] = &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00];

    // Wrap the PKCS#1 key in an OCTET STRING
    let octet_string = wrap_as_octet_string(pkcs1_bytes)?;

    // Build the PKCS#8 structure
    let mut pkcs8_content = Vec::new();

    // Version (INTEGER 0)
    pkcs8_content.push(0x02); // INTEGER tag
    pkcs8_content.push(0x01); // Length
    pkcs8_content.push(0x00); // Value (0)

    // AlgorithmIdentifier
    pkcs8_content.extend_from_slice(&wrap_sequence(RSA_OID)?);

    // PrivateKey (OCTET STRING containing PKCS#1 key)
    pkcs8_content.extend_from_slice(&octet_string);

    // Wrap everything in a SEQUENCE
    wrap_sequence(&pkcs8_content)
}

/// Wrap data as an ASN.1 OCTET STRING
fn wrap_as_octet_string(data: &[u8]) -> Result<Vec<u8>, error_stack::Report<CryptoError>> {
    let mut result = Vec::new();
    result.push(0x04); // OCTET STRING tag

    // Encode length
    if data.len() < 128 {
        result.push(data.len() as u8);
    } else if data.len() < 256 {
        result.push(0x81); // Long form, 1 byte length
        result.push(data.len() as u8);
    } else {
        result.push(0x82); // Long form, 2 byte length
        result.push((data.len() >> 8) as u8);
        result.push((data.len() & 0xFF) as u8);
    }

    result.extend_from_slice(data);
    Ok(result)
}

/// Wrap data as an ASN.1 SEQUENCE
fn wrap_sequence(data: &[u8]) -> Result<Vec<u8>, error_stack::Report<CryptoError>> {
    let mut result = Vec::new();
    result.push(0x30); // SEQUENCE tag

    // Encode length
    if data.len() < 128 {
        result.push(data.len() as u8);
    } else if data.len() < 256 {
        result.push(0x81); // Long form, 1 byte length
        result.push(data.len() as u8);
    } else {
        result.push(0x82); // Long form, 2 byte length
        result.push((data.len() >> 8) as u8);
        result.push((data.len() & 0xFF) as u8);
    }

    result.extend_from_slice(data);
    Ok(result)
}

/// Verify a response signature using RSA-SHA256 with PSS padding
///
/// This is used to verify the `x-response-signature` header
pub fn verify_response_signature(
    signature_b64: &str,
    response_body: &str,
    public_key_pem: &Secret<String>,
) -> Result<bool, error_stack::Report<IntegrationError>> {
    // Decode the signature
    let signature = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(signature_b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(signature_b64))
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "signature",
            context: Default::default(),
        })
        .attach_printable("Failed to decode signature from base64")?;

    // Parse the public key
    let pem_bytes = public_key_pem.peek().as_bytes();
    let key_bytes = extract_key_from_pem(pem_bytes)
        .change_context(IntegrationError::InvalidConnectorConfig {
            config: "juspay_public_key",
            context: Default::default(),
        })
        .attach_printable("Failed to extract public key from PEM")?;

    // Parse RSA public key using ring
    let public_key = signature::UnparsedPublicKey::new(
        &signature::RSA_PSS_2048_8192_SHA256,
        &key_bytes,
    );

    // Verify the signature
    match public_key.verify(response_body.as_bytes(), &signature) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Decode and return the JWS payload
pub fn decode_jws_payload(payload_b64: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    let payload_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(payload_b64))
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "jws_payload",
            context: Default::default(),
        })
        .attach_printable("Failed to decode JWS payload from base64")?;

    String::from_utf8(payload_bytes)
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "jws_payload",
            context: Default::default(),
        })
        .attach_printable("JWS payload is not valid UTF-8")
}

/// Get the current Unix timestamp in milliseconds as a string
pub fn get_current_timestamp_ms() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_generation() {
        let ts = get_current_timestamp_ms();
        assert!(!ts.is_empty());
        assert!(ts.parse::<u128>().is_ok());
    }
}
