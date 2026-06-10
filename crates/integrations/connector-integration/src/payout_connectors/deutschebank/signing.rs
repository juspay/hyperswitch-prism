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

pub fn build_cseal_headers(
    method: Method,
    path: &str,
    body: &[u8],
    key_id: &Secret<String>,
    signing_private_key: &Secret<String>,
) -> Result<CsealHeaders, error_stack::Report<IntegrationError>> {
    let date = format_http_date(OffsetDateTime::now_utc())?;

    let method_lower = match method {
        Method::Get => "get",
        Method::Post => "post",
        Method::Put => "put",
        Method::Delete => "delete",
        Method::Patch => "patch",
    };

    let (digest, signing_string, headers_covered) = if matches!(method, Method::Get) {
        let signing_string = format!("date: {date}\n(request-target): {method_lower} {path}");
        (None, signing_string, "date (request-target)")
    } else {
        let digest_value = compute_digest(body);
        let signing_string = format!(
            "date: {date}\n(request-target): {method_lower} {path}\ndigest: {digest_value}"
        );
        (
            Some(digest_value),
            signing_string,
            "date (request-target) digest",
        )
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

fn compute_digest(body: &[u8]) -> String {
    let hash = Sha256::digest(body);
    format!(
        "SHA-256={}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

fn sign_rsa_sha256(
    data: &[u8],
    private_key_pem: &Secret<String>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let pem = private_key_pem.peek();
    let der =
        extract_der_from_pem(pem).ok_or_else(|| IntegrationError::InvalidConnectorConfig {
            config: "signing_private_key",
            context: IntegrationErrorContext {
                additional_context: Some("PEM body could not be base64-decoded".to_string()),
                suggested_action: Some(
                    "Re-upload `signing_private_key` as a well-formed PEM block.".to_string(),
                ),
                doc_url: None,
            },
        })?;

    let key_pair = signature::RsaKeyPair::from_pkcs8(&der)
        .or_else(|_| {
            let wrapped = wrap_pkcs1_as_pkcs8(&der);
            signature::RsaKeyPair::from_pkcs8(&wrapped)
        })
        .map_err(|_| IntegrationError::InvalidConnectorConfig {
            config: "signing_private_key",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Could not parse RSA private key; expected PKCS#8 or PKCS#1 PEM".to_string(),
                ),
                suggested_action: Some(
                    "Provide an RSA private key in PEM PKCS#8 or PKCS#1 form.".to_string(),
                ),
                doc_url: None,
            },
        })?;

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

fn extract_der_from_pem(pem: &str) -> Option<Vec<u8>> {
    let stripped: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----") && !l.trim().is_empty())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(stripped.trim())
        .ok()
}

fn wrap_pkcs1_as_pkcs8(pkcs1: &[u8]) -> Vec<u8> {
    const ALG_ID: &[u8] = &[
        0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];

    let octet_string = encode_der_tlv(0x04, pkcs1);
    let version = vec![0x02, 0x01, 0x00]; // INTEGER 0
    let mut content = Vec::with_capacity(version.len() + ALG_ID.len() + octet_string.len());
    content.extend_from_slice(&version);
    content.extend_from_slice(ALG_ID);
    content.extend_from_slice(&octet_string);
    encode_der_tlv(0x30, &content)
}

fn encode_der_tlv(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    out.push(tag);
    let len = data.len();
    let len_bytes = len.to_le_bytes();
    if len < 0x80 {
        out.push(len_bytes[0]);
    } else if len < 0x100 {
        out.push(0x81);
        out.push(len_bytes[0]);
    } else {
        out.push(0x82);
        out.push(len_bytes[1]);
        out.push(len_bytes[0]);
    }
    out.extend_from_slice(data);
    out
}
