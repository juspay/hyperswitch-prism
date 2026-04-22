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

/// Verify a response signature using RSA-SHA256 with PKCS#1 v1.5 padding
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
    // RS256 = RSA PKCS#1 v1.5 with SHA-256 (per RFC 7515)
    let public_key = signature::UnparsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        &key_bytes,
    );

    // Verify the signature
    match public_key.verify(response_body.as_bytes(), &signature) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Verify a JWS signature using RSA-PSS with SHA-256
///
/// This is used for verifying JWS signatures from Axis Bank responses.
/// Per Axis Bank documentation, responses use RSA-PSS (not PKCS#1 v1.5).
pub fn verify_jws_signature_pss(
    signature_b64: &str,
    signing_input: &str,
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
        .attach_printable("Failed to decode JWS signature from base64")?;

    // Parse the public key
    let pem_bytes = public_key_pem.peek().as_bytes();
    let key_bytes = extract_key_from_pem(pem_bytes)
        .change_context(IntegrationError::InvalidConnectorConfig {
            config: "juspay_public_key",
            context: Default::default(),
        })
        .attach_printable("Failed to extract public key from PEM")?;

    // Parse RSA public key using ring with PSS padding
    // RSA-PSS with SHA-256 (used by Axis Bank for response signatures)
    let public_key = signature::UnparsedPublicKey::new(
        &signature::RSA_PSS_2048_8192_SHA256,
        &key_bytes,
    );

    // Verify the signature
    match public_key.verify(signing_input.as_bytes(), &signature) {
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

// ============================================
// JWE DECRYPTION
// ============================================

/// Decrypt a JWE-encrypted response using RSA-OAEP-256 + A256GCM
///
/// This handles the JWE envelope that Axis Bank sends in response to successful
/// API calls. The JWE contains a nested JWS that must then be verified.
///
/// # Arguments
/// * `cipher_text` - Base64url-encoded ciphertext
/// * `encrypted_key` - Base64url-encoded encrypted content encryption key
/// * `iv` - Base64url-encoded initialization vector
/// * `protected` - Base64url-encoded JWE protected header
/// * `tag` - Base64url-encoded authentication tag
/// * `merchant_private_key_pem` - Merchant's RSA private key for decryption
///
/// # Returns
/// The decrypted plaintext (which is a JWS string)
pub fn decrypt_jwe_response(
    cipher_text: &str,
    encrypted_key: &str,
    iv: &str,
    protected: &str,
    tag: &str,
    merchant_private_key_pem: &Secret<String>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    // Build the compact JWE serialization format
    // Format: BASE64URL(UTF8(JWE Protected Header)) || '.' ||
    //         BASE64URL(JWE Encrypted Key) || '.' ||
    //         BASE64URL(JWE Initialization Vector) || '.' ||
    //         BASE64URL(JWE Ciphertext) || '.' ||
    //         BASE64URL(JWE Authentication Tag)
    let compact_jwe = format!(
        "{}.{}.{}.{}.{}",
        protected, encrypted_key, iv, cipher_text, tag
    );

    // Load the merchant private key for RSA-OAEP-256
    let private_key_pem = merchant_private_key_pem.peek();

    // Create RSA key pair from PEM
    let rsa_key = josekit::jwk::alg::rsa::RsaKeyPair::from_pem(private_key_pem.as_bytes())
        .change_context(IntegrationError::InvalidConnectorConfig {
            config: "merchant_private_key",
            context: Default::default(),
        })
        .attach_printable("Failed to load merchant private key for JWE decryption")?;

    // Convert to JWK for decrypter
    let jwk = rsa_key.to_jwk_key_pair();

    // Create a JWE decrypter with RSA-OAEP-256 algorithm
    let decrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep256
        .decrypter_from_jwk(&jwk)
        .change_context(IntegrationError::InvalidConnectorConfig {
            config: "merchant_private_key",
            context: Default::default(),
        })
        .attach_printable("Failed to create JWE decrypter from JWK")?;

    // Decrypt the JWE using the high-level deserialize_compact function
    // This handles the A256GCM content encryption automatically from the header
    let (decrypted_bytes, _header) = josekit::jwe::deserialize_compact(&compact_jwe, &decrypter)
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "jwe_response",
            context: Default::default(),
        })
        .attach_printable("Failed to decrypt JWE response")?;

    // Convert to UTF-8 string
    String::from_utf8(decrypted_bytes)
        .change_context(IntegrationError::InvalidDataFormat {
            field_name: "jwe_response",
            context: Default::default(),
        })
        .attach_printable("JWE decryption result is not valid UTF-8")
}

// ============================================
// JWE RESPONSE PREPROCESSING (SHARED ACROSS ALL UPI STACK BANKS)
// ============================================

use crate::connectors::juspay_upi_stack::types::JweResponse;
use domain_types::errors::ConnectorError;
use tracing::debug;

/// Preprocess a potentially JWE-encrypted response.
///
/// This function is shared across all banks in the Juspay UPI Merchant Stack family
/// (Axis Bank, YES Bank, Kotak Bank, RBL, AU Bank, etc.).
///
/// The pipeline:
/// 1. Check if response is JWE-encrypted (by looking for JWE fields)
/// 2. If not JWE, return as-is
/// 3. Parse JWE envelope
/// 4. Decrypt JWE using merchant's RSA private key (RSA-OAEP-256 + A256GCM)
/// 5. Parse the decrypted JWS object
/// 6. Base64url-decode the JWS payload
/// 7. Extract nested response structure and reconstruct flat response
///
/// Following Newton Gateway approach: JWS signature verification is skipped
/// because JWE with AEAD (A256GCM) already provides integrity protection.
///
/// # Arguments
/// * `response_bytes` - Raw response bytes from the bank API
/// * `merchant_private_key` - Merchant's RSA private key for JWE decryption
///
/// # Returns
/// Decrypted plaintext response bytes (ready for JSON deserialization)
pub fn preprocess_jwe_response(
    response_bytes: bytes::Bytes,
    merchant_private_key: &Secret<String>,
) -> Result<bytes::Bytes, ConnectorError> {
    use domain_types::errors::ResponseTransformationErrorContext;

    // Check if this is a JWE-encrypted response
    if !JweResponse::is_jwe_response(&response_bytes) {
        // Not a JWE response (possibly error response), return as-is
        return Ok(response_bytes);
    }

    // Parse the JWE response
    let jwe_response: JweResponse = serde_json::from_slice(&response_bytes)
        .map_err(|e| {
            ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(format!("Could not parse JWE JSON envelope: {}", e)),
                },
            }
        })?;

    // Decrypt the JWE to get the inner JWS
    // Following Newton Gateway approach: JWE AEAD (A256GCM) provides integrity,
    // so JWS signature verification is skipped after decryption.
    let jws_json = decrypt_jwe_response(
        &jwe_response.cipher_text,
        &jwe_response.encrypted_key,
        &jwe_response.iv,
        &jwe_response.protected,
        &jwe_response.tag,
        merchant_private_key,
    )
    .map_err(|e| {
        ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!("JWE decryption failed: {}", e)),
            },
        }
    })?;

    // Parse the decrypted JWS object
    let jws_obj: JwsObject =
        serde_json::from_str(&jws_json).map_err(|e| {
            ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(format!("Could not parse JWS JSON structure: {}", e)),
                },
            }
        })?;

    // Decode the JWS payload (base64url-encoded)
    let payload_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(&jws_obj.payload)
        .map_err(|e| {
            ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: None,
                    additional_context: Some(format!("Could not base64url-decode JWS payload: {}", e)),
                },
            }
        })?;

    // The JWS payload contains a nested structure with the actual response:
    // {"payload": {...}, "responseCode": "...", "responseMessage": "...", "status": "..."}
    let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes).map_err(|e| {
        ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!("Could not parse JWS payload as JSON: {}", e)),
            },
        }
    })?;

    debug!(
        decrypted_response = %String::from_utf8_lossy(&payload_bytes),
        "Juspay UPI Stack decrypted response"
    );

    let final_response = serde_json::json!({
        "status": payload_json.get("status").cloned().unwrap_or(serde_json::json!("UNKNOWN")),
        "responseCode": payload_json.get("responseCode").cloned().unwrap_or(serde_json::json!("UNKNOWN")),
        "responseMessage": payload_json.get("responseMessage").cloned().unwrap_or(serde_json::json!("Unknown")),
        "payload": payload_json.get("payload").cloned()
    });

    let final_bytes = serde_json::to_vec(&final_response).map_err(|e| {
        ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: None,
                additional_context: Some(format!("Could not serialize final response: {}", e)),
            },
        }
    })?;

    Ok(bytes::Bytes::from(final_bytes))
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
