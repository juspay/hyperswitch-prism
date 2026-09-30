//! Deutsche Bank CSEAL request signing

use base64::Engine;
use common_utils::request::Method;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use ring::{rand, signature};
use sha2::{Digest, Sha256};
use time::{macros::format_description, OffsetDateTime};

pub struct CsealHeaders {
    pub date: String,
    pub digest: Option<String>,
    pub signature: String,
}

/// Build the Deutsche Bank CSEAL request headers (`Date`, `Digest`, `Signature`).
///
/// Implements the HTTP-Signatures scheme DB's CSEAL gateway expects:
/// - `Date`: current time as an HTTP-date.
/// - `Digest` (non-GET only): `SHA-256=<base64(sha256(body))>`.
/// - `Signature`: RSA-SHA256 over the canonical signing string
///   (`date`, `(request-target)`, and `digest` for non-GET), formatted as the
///   `keyId=…,algorithm="rsa-sha256",headers="…",signature="…"` header value.
pub fn build_cseal_headers(
    method: Method,
    path: &str,
    body: &[u8],
    key_id: &Secret<String>,
    signing_private_key: &Secret<String>,
) -> Result<CsealHeaders, error_stack::Report<IntegrationError>> {
    let date = format_http_date(common_utils::date_time::now().assume_utc())?;

    let method_lower = method.to_string().to_lowercase();

    let (digest, signing_string, headers_covered) = match method {
        Method::Get => {
            let signing_string = format!("date: {date}\n(request-target): {method_lower} {path}");
            (None, signing_string, "date (request-target)")
        }
        _ => {
            let digest_value = compute_digest(body);
            let signing_string = format!(
                "date: {date}\n(request-target): {method_lower} {path}\ndigest: {digest_value}"
            );
            (
                Some(digest_value),
                signing_string,
                "date (request-target) digest",
            )
        }
    };

    let signature_b64 = sign_rsa_sha256(signing_string.as_bytes(), signing_private_key)?;

    let signature_header = format!(
        r#"keyId="{key_id}",algorithm="rsa-sha256",headers="{headers}",signature="{sig}""#,
        key_id = key_id.peek(),
        headers = headers_covered,
        sig = signature_b64,
    );

    Ok(CsealHeaders {
        date,
        digest,
        signature: signature_header,
    })
}

/// Format a timestamp as an HTTP-date (RFC 7231, e.g. `Tue, 15 Nov 1994 08:12:31 GMT`)
/// for the CSEAL `Date` header.
fn format_http_date(dt: OffsetDateTime) -> Result<String, error_stack::Report<IntegrationError>> {
    let fmt = format_description!(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
    );
    dt.format(&fmt)
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some("formatting HTTP date for CSEAL Date header".to_string()),
                suggested_action: Some("Retry the request; report if persistent.".to_string()),
                doc_url: None,
            },
        })
}

/// Compute the CSEAL `Digest` header value for a request body:
/// `SHA-256=<base64(sha256(body))>`.
fn compute_digest(body: &[u8]) -> String {
    let hash = Sha256::digest(body);
    format!(
        "SHA-256={}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

/// RSA-SHA256 sign `data` with a PEM-encoded private key, returning the base64
/// signature.
///
/// `ring` only accepts PKCS#8 keys, so we first try PKCS#8 directly and, on
/// failure, assume the DER is a bare PKCS#1 `RSAPrivateKey` and wrap it in a
/// PKCS#8 envelope ([`wrap_pkcs1_as_pkcs8`]) before retrying. This lets merchants
/// upload either `-----BEGIN PRIVATE KEY-----` (PKCS#8) or
/// `-----BEGIN RSA PRIVATE KEY-----` (PKCS#1) forms.
fn sign_rsa_sha256(
    data: &[u8],
    private_key_pem: &Secret<String>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let pem = private_key_pem.peek();
    let der = extract_der_from_pem(pem).map_err(|_| IntegrationError::InvalidConnectorConfig {
        config: "signing_private_key",
        context: IntegrationErrorContext {
            additional_context: Some("PEM body is not valid base64".to_string()),
            suggested_action: Some(
                "Re-upload `signing_private_key` as a well-formed PEM block.".to_string(),
            ),
            doc_url: None,
        },
    })?;

    let key_pair = match signature::RsaKeyPair::from_pkcs8(&der) {
        Ok(key_pair) => key_pair,
        // Not PKCS#8 — assume a bare PKCS#1 `RSAPrivateKey` and wrap it in a PKCS#8
        // envelope before retrying.
        Err(_) => {
            let wrapped = wrap_pkcs1_as_pkcs8(&der)?;
            signature::RsaKeyPair::from_pkcs8(&wrapped).map_err(|rejection| {
                IntegrationError::InvalidConnectorConfig {
                    config: "signing_private_key",
                    context: IntegrationErrorContext {
                        additional_context: Some(format!(
                            "Could not parse RSA private key (expected PKCS#8 or PKCS#1 PEM): {rejection}"
                        )),
                        suggested_action: Some(
                            "Provide an RSA private key in PEM PKCS#8 or PKCS#1 form.".to_string(),
                        ),
                        doc_url: None,
                    },
                }
            })?
        }
    };

    let rng = rand::SystemRandom::new();
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(&signature::RSA_PKCS1_SHA256, &rng, data, &mut sig)
        .map_err(|_| IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "RSA-SHA256 sign failed for CSEAL Signature header".to_string(),
                ),
                suggested_action: Some(
                    "Verify `signing_private_key` is a valid RSA key and retry.".to_string(),
                ),
                doc_url: None,
            },
        })?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&sig))
}

/// Decode a PEM block to raw DER bytes: drop the `-----BEGIN/END-----` armor and
/// blank lines, then base64-decode the concatenated body.
fn extract_der_from_pem(pem: &str) -> Result<Vec<u8>, base64::DecodeError> {
    let stripped: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----") && !l.trim().is_empty())
        .collect();
    base64::engine::general_purpose::STANDARD.decode(stripped.trim())
}

/// Wrap a bare PKCS#1 `RSAPrivateKey` (DER) in a PKCS#8 `PrivateKeyInfo` envelope,
/// which is the only form `ring` accepts.
///
/// Produces the ASN.1 DER:
/// ```text
/// SEQUENCE {                       -- PrivateKeyInfo
///   INTEGER 0,                     -- version
///   SEQUENCE {                     -- AlgorithmIdentifier
///     OID 1.2.840.113549.1.1.1,    -- rsaEncryption
///     NULL
///   },
///   OCTET STRING { <pkcs1 DER> }   -- privateKey
/// }
/// ```
/// `ALG_ID` is the pre-encoded `AlgorithmIdentifier` (SEQUENCE of the rsaEncryption
/// OID + NULL parameters).
fn wrap_pkcs1_as_pkcs8(pkcs1: &[u8]) -> Result<Vec<u8>, error_stack::Report<IntegrationError>> {
    const ALG_ID: &[u8] = &[
        0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];

    let octet_string = encode_der_tlv(0x04, pkcs1)?; // OCTET STRING wrapping the PKCS#1 key
    let version = vec![0x02, 0x01, 0x00]; // INTEGER 0 (PKCS#8 version)
    let mut content = Vec::with_capacity(version.len() + ALG_ID.len() + octet_string.len());
    content.extend_from_slice(&version);
    content.extend_from_slice(ALG_ID);
    content.extend_from_slice(&octet_string);
    encode_der_tlv(0x30, &content) // wrap the three fields in the outer SEQUENCE
}

/// Encode a single ASN.1 DER TLV (tag-length-value).
///
/// Handles DER definite-length encoding: short form for lengths ≤ 0x7f (one byte),
/// and long form for larger lengths (`0x81 <len>` for ≤ 0xff, `0x82 <hi> <lo>` for
/// ≤ 0xffff). Lengths beyond 0xffff are rejected explicitly — they are unreachable
/// for RSA keys, but we error rather than silently emit a truncated (malformed) length.
fn encode_der_tlv(tag: u8, data: &[u8]) -> Result<Vec<u8>, error_stack::Report<IntegrationError>> {
    let len = data.len();
    if len > 0xffff {
        return Err(error_stack::report!(
            IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "ASN.1 DER value length {len} exceeds the supported maximum (0xffff)"
                    )),
                    suggested_action: Some(
                        "Unexpected for RSA keys; verify `signing_private_key` is a valid RSA key."
                            .to_string(),
                    ),
                    doc_url: None,
                },
            }
        ));
    }
    let mut out = Vec::with_capacity(len + 4);
    out.push(tag);
    let len_bytes = len.to_le_bytes();
    match len {
        0..=0x7f => out.push(len_bytes[0]),
        0x80..=0xff => out.extend_from_slice(&[0x81, len_bytes[0]]),
        _ => out.extend_from_slice(&[0x82, len_bytes[1], len_bytes[0]]),
    }
    out.extend_from_slice(data);
    Ok(out)
}
