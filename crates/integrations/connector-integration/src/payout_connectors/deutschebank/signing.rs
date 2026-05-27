//! Deutsche Bank CSEAL request signing.
//!
//! CSEAL uses the legacy IETF "HTTP Signatures" draft (`Signature` header with
//! `keyId` / `algorithm` / `headers` / `signature` parameters) plus a `Digest`
//! header for the request body. The signing input is the covered headers
//! joined by `\n`, each as `<lowercase-name>: <value>`. `(request-target)`
//! is included as `<method-lowercase> <path>`.
//!
//! Mirrors the postman pre-request script bundled with the DB BaaS sandbox kit.

use base64::Engine;
use common_utils::request::Method;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};
use hyperswitch_masking::{PeekInterface, Secret};
use ring::{rand, signature};
use sha2::{Digest, Sha256};
use time::{Month, OffsetDateTime, Weekday};

/// Headers computed for a single CSEAL request.
pub struct CsealHeaders {
    /// RFC 7231 / RFC 1123 GMT date (e.g. `Tue, 27 May 2026 14:33:00 GMT`).
    pub date: String,
    /// `SHA-256=<base64(sha256(body))>` — `None` for GET requests.
    pub digest: Option<String>,
    /// The full `Signature` header value (keyId/algorithm/headers/signature).
    pub signature: String,
}

/// Compute the three CSEAL headers for a request.
///
/// - `method`: HTTP method.
/// - `path`: URL path component including leading slash (e.g.
///   `/v1/cseal/payments/sepa/vop-check/vop`). Query string included verbatim
///   if present.
/// - `body`: request body bytes, or `&[]` for GET.
/// - `key_id`: keyId placed in the Signature header.
/// - `signing_private_key`: PEM-encoded RSA private key (PKCS#8 or PKCS#1).
pub fn build_cseal_headers(
    method: Method,
    path: &str,
    body: &[u8],
    key_id: &Secret<String>,
    signing_private_key: &Secret<String>,
) -> Result<CsealHeaders, error_stack::Report<IntegrationError>> {
    let date = format_http_date(OffsetDateTime::now_utc());

    let method_lower = match method {
        Method::Get => "get",
        Method::Post => "post",
        Method::Put => "put",
        Method::Delete => "delete",
        Method::Patch => "patch",
    };

    let (digest, signing_string, headers_covered) = if matches!(method, Method::Get) {
        let signing_string = format!(
            "date: {date}\n(request-target): {method_lower} {path}"
        );
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

/// Format an RFC 7231 IMF-fixdate (e.g. `Tue, 27 May 2026 14:33:00 GMT`).
/// Hand-rolled to avoid pulling in the `time` formatting feature.
fn format_http_date(dt: OffsetDateTime) -> String {
    let weekday = match dt.weekday() {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    };
    let month = match dt.month() {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    };
    format!(
        "{weekday}, {day:02} {month} {year:04} {hour:02}:{minute:02}:{second:02} GMT",
        day = dt.day(),
        year = dt.year(),
        hour = dt.hour(),
        minute = dt.minute(),
        second = dt.second(),
    )
}

fn compute_digest(body: &[u8]) -> String {
    let hash = Sha256::digest(body);
    format!(
        "SHA-256={}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

/// Sign data with RSA-SHA256 (PKCS#1 v1.5). Accepts both PKCS#8 (`BEGIN PRIVATE KEY`)
/// and PKCS#1 (`BEGIN RSA PRIVATE KEY`) PEM inputs. Returns base64-encoded signature.
fn sign_rsa_sha256(
    data: &[u8],
    private_key_pem: &Secret<String>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    let pem = private_key_pem.peek();
    let der = extract_der_from_pem(pem).ok_or_else(|| {
        IntegrationError::InvalidConnectorConfig {
            config: "signing_private_key",
            context: IntegrationErrorContext {
                additional_context: Some("PEM body could not be base64-decoded".to_string()),
                ..Default::default()
            },
        }
    })?;

    let key_pair = signature::RsaKeyPair::from_pkcs8(&der)
        .or_else(|_| {
            // Fall back to wrapping PKCS#1 as PKCS#8.
            let wrapped = wrap_pkcs1_as_pkcs8(&der);
            signature::RsaKeyPair::from_pkcs8(&wrapped)
        })
        .map_err(|_| IntegrationError::InvalidConnectorConfig {
            config: "signing_private_key",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Could not parse RSA private key; expected PKCS#8 or PKCS#1 PEM".to_string(),
                ),
                ..Default::default()
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
                ..Default::default()
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

/// Wrap a PKCS#1 RSA private key (bare RSAPrivateKey DER) in a PKCS#8
/// PrivateKeyInfo so `ring::signature::RsaKeyPair::from_pkcs8` accepts it.
fn wrap_pkcs1_as_pkcs8(pkcs1: &[u8]) -> Vec<u8> {
    // OID rsaEncryption (1.2.840.113549.1.1.1) + NULL params.
    const ALG_ID: &[u8] = &[
        0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];

    let octet_string = der_wrap(0x04, pkcs1);
    let version = vec![0x02, 0x01, 0x00]; // INTEGER 0
    let mut content = Vec::with_capacity(version.len() + ALG_ID.len() + octet_string.len());
    content.extend_from_slice(&version);
    content.extend_from_slice(ALG_ID);
    content.extend_from_slice(&octet_string);
    der_wrap(0x30, &content)
}

fn der_wrap(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    out.push(tag);
    let len = data.len();
    if len < 0x80 {
        out.push(len as u8);
    } else if len < 0x100 {
        out.push(0x81);
        out.push(len as u8);
    } else {
        out.push(0x82);
        out.push((len >> 8) as u8);
        out.push((len & 0xff) as u8);
    }
    out.extend_from_slice(data);
    out
}
