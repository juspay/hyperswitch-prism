//! JOSE (JSON Object Signing and Encryption) helpers for connectors that
//! wrap requests in a PS256 JWS inside an RSA-OAEP / A128CBC-HS256 JWE.
//!
//! Reusable across any JOSE-using connector with the same envelope shape
//! (request: PS256 JWS → RSA-OAEP/A128CBC-HS256 JWE; response: inverse).
//! Two non-obvious facts are isolated here so they don't leak into every
//! consumer:
//!
//! 1. **josekit 0.8.7's `RsassaPssJwsAlgorithm::from_pem` rejects PKCS#8
//!    PEMs with the generic-RSA OID** (`1.2.840.113549.1.1.1`). Real
//!    RSA-4096 keys we see in practice carry that OID rather than the
//!    `id-RSASSA-PSS` OID josekit looks for, so we reach for `openssl`
//!    directly with explicit PSS padding parameters.
//!
//! 2. **Some counterparties publish PEMs without the newline before
//!    `-----END...-----`.** OpenSSL refuses to parse those. We fix it
//!    once at [`JoseConfig::new`] so callers don't have to think about
//!    it on every request.
//!
//! ### Threat model
//!
//! - **Algorithm confusion**: enforced. The JWE `alg` is hard-coded to
//!   RSA-OAEP and `enc` to A128CBC-HS256; the JWS `alg` is hard-coded to
//!   PS256. We do not negotiate or accept alternate algorithms from a
//!   response header.
//! - **Weak keys**: enforced at [`JoseConfig::new`]. All four RSA keys
//!   must be ≥ 2048 bits, matching NIST SP 800-131A acceptance.
//! - **Replay**: optionally enforced via [`JoseClaimValidation`].
//!   Callers that want replay protection on response JWTs must pass an
//!   expected `aud` and the helper will then validate `exp` / `nbf`.
//!   Without that, the decrypted payload is returned verbatim and the
//!   caller takes responsibility for any temporal checks.

use base64::Engine;
use hyperswitch_masking::{PeekInterface, Secret};
use josekit::jwe::{alg::rsaes::RsaesJweAlgorithm, JweHeader};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    rsa::Padding,
    sign::{RsaPssSaltlen, Signer, Verifier},
};
use serde::Serialize;

use crate::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;

/// Minimum RSA key size (in bits) acceptable for any of the four PEMs.
/// Matches the NIST SP 800-131A floor; OpenSSL otherwise accepts 1024.
const MIN_RSA_BITS: u32 = 2048;

/// Hard-coded JWS algorithm header. We never accept anything else.
const JWS_ALG: &str = "PS256";
/// Hard-coded JWE key-wrap algorithm header. We never accept anything else.
const JWE_ALG: &str = "RSA-OAEP";
/// Hard-coded JWE content encryption header.
const JWE_ENC: &str = "A128CBC-HS256";

/// Errors raised by the JOSE pipeline.
#[derive(Debug, thiserror::Error)]
pub enum JoseError {
    #[error("Invalid PEM key material for {context}")]
    InvalidKey { context: &'static str },
    #[error("RSA key for {context} is shorter than {min_bits} bits ({actual_bits} bits)")]
    KeyTooSmall {
        context: &'static str,
        min_bits: u32,
        actual_bits: u32,
    },
    #[error("Failed to sign JWS")]
    SigningFailed,
    #[error("Failed to verify JWS signature")]
    VerificationFailed,
    #[error("Failed to encrypt JWE")]
    EncryptionFailed,
    #[error("Failed to decrypt JWE")]
    DecryptionFailed,
    #[error("Failed to serialise claims to JSON")]
    SerdeSerializeFailed,
    /// Catch-all for any failure after the JWE/JWS layers have completed —
    /// includes invalid UTF-8 in the JWS payload, malformed JSON, missing
    /// or wrong-type claim fields, etc. Distinguishable variants here would
    /// leak which post-decryption stage failed.
    #[error("Decrypted payload could not be parsed")]
    PayloadParseFailed,
    #[error("Compact JWS does not have three dot-separated segments")]
    MalformedJws,
    #[error("JWE protected header algorithm {got} does not match expected {expected}")]
    UnexpectedJweAlgorithm { got: String, expected: &'static str },
    #[error("JWE protected header content encryption {got} does not match expected {expected}")]
    UnexpectedJweContentEncryption { got: String, expected: &'static str },
    #[error("JWT claim validation failed: {reason}")]
    ClaimValidationFailed { reason: &'static str },
}

/// Bundle of credentials needed by [`sign_then_encrypt`] /
/// [`decrypt_then_verify`].
///
/// Field names are deliberately generic — the four PEMs carry the
/// signer/encrypter roles, not the institution names. PEM normalisation
/// runs once at [`JoseConfig::new`] time and the minimum RSA key size is
/// enforced at the same point.
///
/// Naming convention:
/// - `*_signing_private_key` — signs outbound JWS (sender role).
/// - `*_encryption_private_key` — decrypts inbound JWE (receiver role).
/// - `*_signing_public_key` — verifies inbound JWS.
/// - `*_encryption_public_key` — seals outbound JWE against.
///
/// "self" is this side; "peer" is the counterparty.
#[derive(Debug, Clone)]
pub struct JoseConfig {
    /// Environment-specific JWE key id placed in the JWE protected header.
    /// The peer uses this to pick which decryption key to apply on its side.
    pub kid: String,
    /// This side's RSA private key that signs the outbound JWS.
    pub self_signing_private_key: Secret<String>,
    /// This side's RSA private key that decrypts the inbound JWE.
    pub self_encryption_private_key: Secret<String>,
    /// Counterparty public key used to verify the inbound JWS.
    pub peer_signing_public_key: Secret<String>,
    /// Counterparty public key the outbound JWE is sealed against.
    pub peer_encryption_public_key: Secret<String>,
}

/// Optional JWT claim assertions applied after JWS verification.
///
/// Callers that care about replay protection on signed responses pass an
/// expected `aud` (or leave it `None` to skip the audience check) and the
/// helper enforces `exp` / `nbf` against `clock_skew_seconds`. Pass the
/// whole struct as `None` to skip these checks entirely (appropriate for
/// envelopes whose freshness is gated by an outer transport, but document
/// the threat model when doing so).
#[derive(Debug, Clone)]
pub struct JoseClaimValidation {
    /// Required `aud` claim value. `None` skips the audience check while
    /// still enforcing `exp` / `nbf`. Some counterparties (PACO) emit a
    /// response `aud` equal to the merchant's access-token — a value
    /// known only at runtime — so a `String` is more flexible than
    /// `&'static str`.
    pub expected_audience: Option<String>,
    /// Clock skew tolerance for `exp` / `nbf` checks.
    pub clock_skew_seconds: i64,
}

impl JoseClaimValidation {
    pub fn new(expected_audience: impl Into<String>) -> Self {
        Self {
            expected_audience: Some(expected_audience.into()),
            clock_skew_seconds: 30,
        }
    }

    /// Skip the `aud` check entirely — still enforces `exp` / `nbf`.
    pub fn temporal_only() -> Self {
        Self {
            expected_audience: None,
            clock_skew_seconds: 30,
        }
    }
}

impl JoseConfig {
    /// Construct a [`JoseConfig`], normalising the four PEMs, verifying
    /// every key parses with OpenSSL, and rejecting any RSA key under
    /// [`MIN_RSA_BITS`].
    pub fn new(
        kid: String,
        self_signing_private_key: Secret<String>,
        self_encryption_private_key: Secret<String>,
        peer_signing_public_key: Secret<String>,
        peer_encryption_public_key: Secret<String>,
    ) -> Result<Self, JoseError> {
        let cfg = Self {
            kid,
            self_signing_private_key: normalise_pem_secret(self_signing_private_key),
            self_encryption_private_key: normalise_pem_secret(self_encryption_private_key),
            peer_signing_public_key: normalise_pem_secret(peer_signing_public_key),
            peer_encryption_public_key: normalise_pem_secret(peer_encryption_public_key),
        };
        cfg.validate_keys()?;
        Ok(cfg)
    }

    fn validate_keys(&self) -> Result<(), JoseError> {
        validate_private_pem(
            self.self_signing_private_key.peek(),
            "self_signing_private_key",
        )?;
        validate_private_pem(
            self.self_encryption_private_key.peek(),
            "self_encryption_private_key",
        )?;
        validate_public_pem(
            self.peer_signing_public_key.peek(),
            "peer_signing_public_key",
        )?;
        validate_public_pem(
            self.peer_encryption_public_key.peek(),
            "peer_encryption_public_key",
        )?;
        Ok(())
    }
}

fn validate_private_pem(pem: &str, context: &'static str) -> Result<(), JoseError> {
    let pkey = PKey::private_key_from_pem(pem.as_bytes())
        .map_err(|_| JoseError::InvalidKey { context })?;
    let bits = pkey.bits();
    if bits < MIN_RSA_BITS {
        return Err(JoseError::KeyTooSmall {
            context,
            min_bits: MIN_RSA_BITS,
            actual_bits: bits,
        });
    }
    Ok(())
}

fn validate_public_pem(pem: &str, context: &'static str) -> Result<(), JoseError> {
    let pkey =
        PKey::public_key_from_pem(pem.as_bytes()).map_err(|_| JoseError::InvalidKey { context })?;
    let bits = pkey.bits();
    if bits < MIN_RSA_BITS {
        return Err(JoseError::KeyTooSmall {
            context,
            min_bits: MIN_RSA_BITS,
            actual_bits: bits,
        });
    }
    Ok(())
}

/// Sign a serialisable claim set with PS256, then seal the resulting JWS
/// inside an RSA-OAEP / A128CBC-HS256 JWE bound to `cfg.kid`. Returns the
/// compact JWE — five dot-separated base64url segments.
pub fn sign_then_encrypt<T: Serialize>(claims: &T, cfg: &JoseConfig) -> Result<String, JoseError> {
    let payload = serde_json::to_vec(claims).map_err(|_| JoseError::SerdeSerializeFailed)?;
    let jws = sign_jws_ps256(&payload, cfg.self_signing_private_key.peek())?;
    encrypt_jwe_rsa_oaep(
        jws.as_bytes(),
        &cfg.kid,
        cfg.peer_encryption_public_key.peek(),
    )
}

/// Inverse of [`sign_then_encrypt`] without temporal claim checks: decrypt
/// the compact JWE, verify the inner JWS signature, return the inner JSON
/// payload verbatim.
///
/// Use [`decrypt_then_verify_with_claims`] when the envelope's `aud` /
/// `exp` / `nbf` claims must be validated for replay protection.
pub fn decrypt_then_verify(
    jwe_compact: &str,
    cfg: &JoseConfig,
) -> Result<serde_json::Value, JoseError> {
    decrypt_then_verify_with_claims(jwe_compact, cfg, None)
}

/// Same as [`decrypt_then_verify`] but additionally validates the JWT
/// claims when `validation` is provided.
pub fn decrypt_then_verify_with_claims(
    jwe_compact: &str,
    cfg: &JoseConfig,
    validation: Option<&JoseClaimValidation>,
) -> Result<serde_json::Value, JoseError> {
    let jws_bytes = decrypt_jwe_rsa_oaep(jwe_compact, cfg.self_encryption_private_key.peek())?;
    let jws = std::str::from_utf8(&jws_bytes).map_err(|_| JoseError::PayloadParseFailed)?;
    let payload = verify_jws_ps256(jws, cfg.peer_signing_public_key.peek())?;
    let value: serde_json::Value =
        serde_json::from_slice(&payload).map_err(|_| JoseError::PayloadParseFailed)?;
    if let Some(v) = validation {
        enforce_claim_validation(&value, v)?;
    }
    Ok(value)
}

fn enforce_claim_validation(
    payload: &serde_json::Value,
    v: &JoseClaimValidation,
) -> Result<(), JoseError> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let obj = payload
        .as_object()
        .ok_or(JoseError::ClaimValidationFailed {
            reason: "claims-not-object",
        })?;

    if let Some(expected) = v.expected_audience.as_deref() {
        let aud =
            obj.get("aud")
                .and_then(|v| v.as_str())
                .ok_or(JoseError::ClaimValidationFailed {
                    reason: "missing-aud",
                })?;
        if aud != expected {
            return Err(JoseError::ClaimValidationFailed {
                reason: "aud-mismatch",
            });
        }
    }

    if let Some(exp) = obj.get("exp").and_then(|v| v.as_i64()) {
        if now > exp + v.clock_skew_seconds {
            return Err(JoseError::ClaimValidationFailed {
                reason: "exp-elapsed",
            });
        }
    }
    if let Some(nbf) = obj.get("nbf").and_then(|v| v.as_i64()) {
        if now + v.clock_skew_seconds < nbf {
            return Err(JoseError::ClaimValidationFailed {
                reason: "nbf-not-yet",
            });
        }
    }
    Ok(())
}

// ---------- PEM normalisation ----------

fn normalise_pem_secret(secret: Secret<String>) -> Secret<String> {
    let raw = secret.peek().clone();
    Secret::new(normalise_pem(&raw))
}

/// Some counterparties publish PEMs without the newline before
/// `-----END-----`. OpenSSL rejects that. Fix it idempotently.
fn normalise_pem(pem: &str) -> String {
    let trimmed = pem.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len() + 2);
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        out.push(ch);
        // Insert a newline before any `-----END` that didn't already start
        // a new line.
        if ch != '\n' && chars.peek() == Some(&'-') {
            let lookahead: String = chars.clone().take(8).collect();
            if lookahead.starts_with("-----END") {
                out.push('\n');
            }
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

// ---------- PS256 JWS via OpenSSL ----------

fn sign_jws_ps256(payload: &[u8], private_key_pem: &str) -> Result<String, JoseError> {
    let header = serde_json::json!({ "alg": JWS_ALG, "typ": "JWT" });
    let header_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(header.to_string());
    let payload_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(payload);
    let signing_input = format!("{header_b64}.{payload_b64}");

    let pkey = PKey::private_key_from_pem(private_key_pem.as_bytes()).map_err(|_| {
        JoseError::InvalidKey {
            context: "self_signing_private_key",
        }
    })?;

    let mut signer =
        Signer::new(MessageDigest::sha256(), &pkey).map_err(|_| JoseError::SigningFailed)?;
    signer
        .set_rsa_padding(Padding::PKCS1_PSS)
        .map_err(|_| JoseError::SigningFailed)?;
    signer
        .set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
        .map_err(|_| JoseError::SigningFailed)?;
    signer
        .set_rsa_mgf1_md(MessageDigest::sha256())
        .map_err(|_| JoseError::SigningFailed)?;
    signer
        .update(signing_input.as_bytes())
        .map_err(|_| JoseError::SigningFailed)?;
    let sig = signer.sign_to_vec().map_err(|_| JoseError::SigningFailed)?;

    let sig_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(sig);
    Ok(format!("{signing_input}.{sig_b64}"))
}

fn verify_jws_ps256(jws_compact: &str, public_key_pem: &str) -> Result<Vec<u8>, JoseError> {
    let parts: Vec<&str> = jws_compact.split('.').collect();
    let (header_b64, payload_b64, sig_b64) = match parts.as_slice() {
        [h, p, s] => (*h, *p, *s),
        _ => return Err(JoseError::MalformedJws),
    };

    // Assert the protected header announces PS256 — defence in depth against
    // an attacker that swaps `alg` to `none` or downgrades to HMAC.
    let header_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|_| JoseError::VerificationFailed)?;
    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|_| JoseError::VerificationFailed)?;
    let alg = header
        .get("alg")
        .and_then(|v| v.as_str())
        .ok_or(JoseError::VerificationFailed)?;
    if alg != JWS_ALG {
        return Err(JoseError::VerificationFailed);
    }

    let signing_input = format!("{header_b64}.{payload_b64}");
    let sig = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| JoseError::VerificationFailed)?;
    let payload = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| JoseError::VerificationFailed)?;

    let pkey = PKey::public_key_from_pem(public_key_pem.as_bytes()).map_err(|_| {
        JoseError::InvalidKey {
            context: "peer_signing_public_key",
        }
    })?;

    let mut verifier =
        Verifier::new(MessageDigest::sha256(), &pkey).map_err(|_| JoseError::VerificationFailed)?;
    verifier
        .set_rsa_padding(Padding::PKCS1_PSS)
        .map_err(|_| JoseError::VerificationFailed)?;
    verifier
        .set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
        .map_err(|_| JoseError::VerificationFailed)?;
    verifier
        .set_rsa_mgf1_md(MessageDigest::sha256())
        .map_err(|_| JoseError::VerificationFailed)?;
    verifier
        .update(signing_input.as_bytes())
        .map_err(|_| JoseError::VerificationFailed)?;

    if !verifier
        .verify(&sig)
        .map_err(|_| JoseError::VerificationFailed)?
    {
        return Err(JoseError::VerificationFailed);
    }
    Ok(payload)
}

// ---------- RSA-OAEP / A128CBC-HS256 JWE via josekit ----------

fn encrypt_jwe_rsa_oaep(
    plaintext: &[u8],
    kid: &str,
    public_key_pem: &str,
) -> Result<String, JoseError> {
    let mut header = JweHeader::new();
    header.set_content_encryption(JWE_ENC);
    header.set_key_id(kid);

    let encrypter = RsaesJweAlgorithm::RsaOaep
        .encrypter_from_pem(public_key_pem.as_bytes())
        .map_err(|_| JoseError::InvalidKey {
            context: "peer_encryption_public_key",
        })?;

    josekit::jwe::serialize_compact(plaintext, &header, &encrypter)
        .map_err(|_| JoseError::EncryptionFailed)
}

fn decrypt_jwe_rsa_oaep(jwe_compact: &str, private_key_pem: &str) -> Result<Vec<u8>, JoseError> {
    // Parse the protected header explicitly so we can assert alg / enc
    // before handing it to josekit. This rejects an attacker that flips
    // the JWE to e.g. `alg: dir` (direct encryption) or
    // `enc: A256GCM` and counts on josekit's default behaviour.
    let first_dot = jwe_compact.find('.').ok_or(JoseError::DecryptionFailed)?;
    let header_b64 = &jwe_compact[..first_dot];
    let header_bytes = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(header_b64)
        .map_err(|_| JoseError::DecryptionFailed)?;
    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|_| JoseError::DecryptionFailed)?;
    let alg = header
        .get("alg")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if alg != JWE_ALG {
        return Err(JoseError::UnexpectedJweAlgorithm {
            got: alg.to_string(),
            expected: JWE_ALG,
        });
    }
    let enc = header
        .get("enc")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if enc != JWE_ENC {
        return Err(JoseError::UnexpectedJweContentEncryption {
            got: enc.to_string(),
            expected: JWE_ENC,
        });
    }

    let decrypter = RsaesJweAlgorithm::RsaOaep
        .decrypter_from_pem(private_key_pem.as_bytes())
        .map_err(|_| JoseError::InvalidKey {
            context: "self_encryption_private_key",
        })?;

    let (plaintext, _header) = josekit::jwe::deserialize_compact(jwe_compact, &decrypter)
        .map_err(|_| JoseError::DecryptionFailed)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use hyperswitch_masking::{PeekInterface, Secret};
    use openssl::{pkey::PKey, rsa::Rsa};
    use serde::{Deserialize, Serialize};

    use super::{
        decrypt_then_verify, decrypt_then_verify_with_claims, normalise_pem, sign_then_encrypt,
        JoseClaimValidation, JoseConfig, JoseError,
    };

    fn keypair_pems(bits: u32) -> (String, String) {
        let rsa = Rsa::generate(bits).expect("generate rsa");
        let pkey = PKey::from_rsa(rsa).expect("pkey");
        let priv_pem = pkey.private_key_to_pem_pkcs8().expect("priv pem");
        let pub_pem = pkey.public_key_to_pem().expect("pub pem");
        (
            String::from_utf8(priv_pem).expect("priv utf8"),
            String::from_utf8(pub_pem).expect("pub utf8"),
        )
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Claims {
        iss: String,
        aud: String,
        exp: i64,
        nbf: i64,
        n: u32,
    }

    fn sender_config() -> (JoseConfig, String) {
        let (self_sign_priv, self_sign_pub) = keypair_pems(2048);
        let (peer_enc_priv, peer_enc_pub) = keypair_pems(2048);
        let cfg = JoseConfig::new(
            "test-kid".into(),
            Secret::new(self_sign_priv),
            Secret::new(peer_enc_priv),
            Secret::new(self_sign_pub.clone()),
            Secret::new(peer_enc_pub),
        )
        .expect("cfg");
        (cfg, self_sign_pub)
    }

    #[test]
    fn round_trip_sign_encrypt_then_decrypt_verify() {
        let (cfg, _) = sender_config();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let claims = Claims {
            iss: "test".into(),
            aud: "TestAudience".into(),
            exp: now + 300,
            nbf: now,
            n: 7,
        };
        let jwe = sign_then_encrypt(&claims, &cfg).expect("encrypt");
        assert_eq!(jwe.split('.').count(), 5);
        let decoded = decrypt_then_verify(&jwe, &cfg).expect("decrypt");
        let recovered: Claims = serde_json::from_value(decoded).expect("claims");
        assert_eq!(recovered, claims);
    }

    #[test]
    fn pem_without_trailing_end_newline_is_repaired() {
        let (_, pub_pem) = keypair_pems(2048);
        let mut broken = pub_pem.replace("\n-----END", "-----END");
        if broken.ends_with('\n') {
            broken.pop();
        }
        assert!(!broken.contains("\n-----END"));
        let fixed = normalise_pem(&broken);
        assert!(fixed.contains("\n-----END"));
        PKey::public_key_from_pem(fixed.as_bytes()).expect("parse fixed pem");
    }

    #[test]
    fn invalid_pem_is_rejected_at_construction() {
        let result = JoseConfig::new(
            "kid".into(),
            Secret::new("not-a-pem".into()),
            Secret::new("not-a-pem".into()),
            Secret::new("not-a-pem".into()),
            Secret::new("not-a-pem".into()),
        );
        assert!(matches!(result, Err(JoseError::InvalidKey { .. })));
    }

    #[test]
    fn rsa_key_below_minimum_bits_is_rejected() {
        // 1024-bit RSA is too weak; must be rejected even though OpenSSL
        // parses it.
        let (weak_priv, weak_pub) = keypair_pems(1024);
        let (ok_priv, ok_pub) = keypair_pems(2048);
        let res = JoseConfig::new(
            "kid".into(),
            Secret::new(weak_priv),
            Secret::new(ok_priv),
            Secret::new(weak_pub),
            Secret::new(ok_pub),
        );
        assert!(matches!(
            res,
            Err(JoseError::KeyTooSmall {
                context: "self_signing_private_key",
                ..
            })
        ));
    }

    #[test]
    fn wrong_signing_key_fails_verify() {
        let (sign_priv_a, _) = keypair_pems(2048);
        let (_, sign_pub_b) = keypair_pems(2048);
        let (enc_priv, enc_pub) = keypair_pems(2048);
        let cfg = JoseConfig::new(
            "kid".into(),
            Secret::new(sign_priv_a),
            Secret::new(enc_priv),
            Secret::new(sign_pub_b),
            Secret::new(enc_pub),
        )
        .expect("cfg");

        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let jwe = sign_then_encrypt(
            &Claims {
                iss: "x".into(),
                aud: "y".into(),
                exp: now + 60,
                nbf: now,
                n: 1,
            },
            &cfg,
        )
        .expect("encrypt");
        let err = decrypt_then_verify(&jwe, &cfg).expect_err("must fail verify");
        assert!(matches!(err, JoseError::VerificationFailed));
    }

    #[test]
    fn tampered_jws_signature_fails_verify() {
        let (cfg, _) = sender_config();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let claims = Claims {
            iss: "x".into(),
            aud: "y".into(),
            exp: now + 60,
            nbf: now,
            n: 9,
        };
        let _ = sign_then_encrypt(&claims, &cfg).expect("encrypt");

        // The compact-JWE-then-JWS pipeline runs the JWS verify inside the
        // decrypted payload. Construct a deliberately broken JWS by hand
        // and re-encrypt it with the peer's encryption key.
        let broken_jws = "eyJhbGciOiJQUzI1NiJ9.eyJpc3MiOiJ4In0.AAA";
        let enc_pub = cfg.peer_encryption_public_key.peek().clone();
        let mut header = josekit::jwe::JweHeader::new();
        header.set_content_encryption("A128CBC-HS256");
        header.set_key_id(&cfg.kid);
        let encrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep
            .encrypter_from_pem(enc_pub.as_bytes())
            .expect("enc");
        let tampered = josekit::jwe::serialize_compact(broken_jws.as_bytes(), &header, &encrypter)
            .expect("ser");
        let err = decrypt_then_verify(&tampered, &cfg).expect_err("must fail");
        assert!(matches!(err, JoseError::VerificationFailed));
    }

    #[test]
    fn jws_alg_none_is_rejected_before_signature_check() {
        // Defence-in-depth: a forged inner JWS with `{"alg":"none"}` and
        // *no signature* MUST be refused at the alg-header guard before the
        // signature verifier ever runs. Without this guard, a downgrade
        // attack that strips the signature could pass verification on a
        // permissive `Verifier`. We confirm the same `VerificationFailed`
        // error fires here as it would for a tampered-signature payload —
        // the failure mode is symmetric on purpose; what matters is that
        // the alg check happens *first*.
        use crate::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;
        use base64::Engine;
        let (cfg, _) = sender_config();
        let none_header = serde_json::json!({"alg": "none"});
        let header_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(none_header.to_string());
        let payload_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(r#"{"iss":"x"}"#);
        // Empty signature segment — what an `alg:none` attack would actually
        // produce. Wrap in a legitimately-encrypted JWE so we get past JWE
        // decryption and the guard becomes the first thing the verifier sees.
        let alg_none_jws = format!("{header_b64}.{payload_b64}.");
        let enc_pub = cfg.peer_encryption_public_key.peek().clone();
        let mut header = josekit::jwe::JweHeader::new();
        header.set_content_encryption("A128CBC-HS256");
        header.set_key_id(&cfg.kid);
        let encrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep
            .encrypter_from_pem(enc_pub.as_bytes())
            .expect("enc");
        let jwe = josekit::jwe::serialize_compact(alg_none_jws.as_bytes(), &header, &encrypter)
            .expect("ser");
        let err = decrypt_then_verify(&jwe, &cfg).expect_err("must fail");
        assert!(matches!(err, JoseError::VerificationFailed));
    }

    #[test]
    fn jwe_alg_mismatch_is_rejected() {
        // Hand-craft a JWE header that announces `dir` (direct encryption)
        // and confirm decrypt_then_verify refuses it before handing off to
        // josekit.
        use crate::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;
        use base64::Engine;
        let header = serde_json::json!({"alg": "dir", "enc": "A128CBC-HS256"});
        let header_b64 = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(header.to_string());
        let fake = format!("{header_b64}.AAA.AAA.AAA.AAA");
        let (cfg, _) = sender_config();
        let err = decrypt_then_verify(&fake, &cfg).expect_err("must fail");
        assert!(matches!(err, JoseError::UnexpectedJweAlgorithm { .. }));
    }

    #[test]
    fn claim_validation_rejects_wrong_audience() {
        let (cfg, _) = sender_config();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let jwe = sign_then_encrypt(
            &Claims {
                iss: "x".into(),
                aud: "Wrong".into(),
                exp: now + 60,
                nbf: now,
                n: 1,
            },
            &cfg,
        )
        .expect("encrypt");
        let validation = JoseClaimValidation::new("Expected");
        let err =
            decrypt_then_verify_with_claims(&jwe, &cfg, Some(&validation)).expect_err("must fail");
        assert!(matches!(
            err,
            JoseError::ClaimValidationFailed {
                reason: "aud-mismatch"
            }
        ));
    }

    #[test]
    fn claim_validation_rejects_expired() {
        let (cfg, _) = sender_config();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let jwe = sign_then_encrypt(
            &Claims {
                iss: "x".into(),
                aud: "Aud".into(),
                exp: now - 3600,
                nbf: now - 7200,
                n: 1,
            },
            &cfg,
        )
        .expect("encrypt");
        let validation = JoseClaimValidation::new("Aud");
        let err =
            decrypt_then_verify_with_claims(&jwe, &cfg, Some(&validation)).expect_err("must fail");
        assert!(matches!(
            err,
            JoseError::ClaimValidationFailed {
                reason: "exp-elapsed"
            }
        ));
    }
}
