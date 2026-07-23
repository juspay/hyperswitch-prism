//! JWE encryption and decryption for Juspay Card Synchronization.
//! The shared JOSE helper is a sign-then-encrypt / decrypt-then-verify pipeline
//! (a PS256 JWS wrapped around an RSA-OAEP + A128CBC-HS256 JWE, four keys). Juspay
//! card sync is JWE-only: no signing, one key per direction, and different
//! algorithms (RSA-OAEP-256 + A128GCM). It shares neither the algorithms nor the
//! two-layer shape, so this is a distinct helper, not a candidate for dedup.
//!
use base64::Engine;
use common_utils::consts::BASE64_ENGINE_URL_SAFE_NO_PAD;
use domain_types::errors::IntegrationError;
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};

/// The only accepted JWE key-management algorithm.
pub const JWE_ALGORITHM: &str = "RSA-OAEP-256";

/// The only accepted JWE content-encryption algorithm.
pub const JWE_ENCRYPTION: &str = "A128GCM";

/// Number of segments in a compact JWE serialization:
/// `protected.encrypted_key.iv.ciphertext.tag`
const COMPACT_JWE_SEGMENTS: usize = 5;

/// NIST SP 800-131A floor; josekit would otherwise load weaker keys silently.
const MIN_RSA_BITS: u32 = 2048;

fn invalid_format(field_name: &'static str) -> IntegrationError {
    IntegrationError::InvalidDataFormat {
        field_name,
        context: Default::default(),
    }
}

fn invalid_config(config: &'static str) -> IntegrationError {
    IntegrationError::InvalidConnectorConfig {
        config,
        context: Default::default(),
    }
}

/// Reject an RSA key PEM below [`MIN_RSA_BITS`]. Keys are provisioned externally;
/// this fails a misprovisioned weak key loudly rather than sealing PAN under it.
fn ensure_min_rsa_bits(
    pem: &Secret<String>,
    is_private: bool,
    config: &'static str,
) -> Result<(), error_stack::Report<IntegrationError>> {
    // Read the size in each branch: the two loaders return distinct `PKey` types.
    let bits = if is_private {
        openssl::pkey::PKey::private_key_from_pem(pem.peek().as_bytes())
            .change_context(invalid_config(config))
            .attach_printable("failed to parse the RSA private key PEM")?
            .bits()
    } else {
        openssl::pkey::PKey::public_key_from_pem(pem.peek().as_bytes())
            .change_context(invalid_config(config))
            .attach_printable("failed to parse the RSA public key PEM")?
            .bits()
    };

    if bits < MIN_RSA_BITS {
        return Err(error_stack::report!(invalid_config(config))).attach_printable(format!(
            "RSA key for {config} is {bits} bits; minimum is {MIN_RSA_BITS}"
        ));
    }
    Ok(())
}

/// The subset of the JWE protected header this integration cares about.
#[derive(Debug, serde::Deserialize)]
struct ProtectedHeader {
    alg: Option<String>,
    enc: Option<String>,
}

/// Encrypt the card payload into the compact JWE used as `cardData`. Returns a
/// `Secret` though it is ciphertext, to keep the PAN-derived value out of logs.
pub fn encrypt_card_data(
    plaintext: &Secret<String>,
    juspay_public_key_pem: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    ensure_min_rsa_bits(juspay_public_key_pem, false, "juspay_encryption_public_key")?;

    let mut header = josekit::jwe::JweHeader::new();
    header.set_algorithm(JWE_ALGORITHM);
    header.set_content_encryption(JWE_ENCRYPTION);

    let encrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep256
        .encrypter_from_pem(juspay_public_key_pem.peek().as_bytes())
        .change_context(invalid_config("juspay_encryption_public_key"))
        .attach_printable("failed to load Juspay's public key for JWE encryption")?;

    josekit::jwe::serialize_compact(plaintext.peek().as_bytes(), &header, &encrypter)
        .change_context(invalid_format("card_data"))
        .attach_printable("failed to JWE-encrypt card data")
        .map(Secret::new)
}

/// Assert the JWE's `alg`/`enc` — the guard against algorithm confusion. Other
/// header params (`kid`, `cty`, …) are inert: we decrypt with one fixed key and
/// never act on header metadata. Must run before decryption, since josekit takes
/// `enc` from the header.
fn validate_protected_header(
    compact_jwe: &str,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let segments: Vec<&str> = compact_jwe.split('.').collect();
    if segments.len() != COMPACT_JWE_SEGMENTS {
        return Err(error_stack::report!(invalid_format("payload")))
            .attach_printable("response payload is not a compact JWE");
    }

    let protected = segments
        .first()
        .ok_or_else(|| error_stack::report!(invalid_format("payload")))?;

    let decoded = BASE64_ENGINE_URL_SAFE_NO_PAD
        .decode(protected)
        .change_context(invalid_format("payload"))
        .attach_printable("JWE protected header is not valid base64url")?;

    let header: ProtectedHeader = serde_json::from_slice(&decoded)
        .change_context(invalid_format("payload"))
        .attach_printable("JWE protected header is not valid JSON")?;

    match header.alg.as_deref() {
        Some(JWE_ALGORITHM) => {}
        other => {
            return Err(error_stack::report!(invalid_format("payload"))).attach_printable(format!(
                "unsupported JWE alg: expected {JWE_ALGORITHM}, received {}",
                other.unwrap_or("none")
            ));
        }
    }

    match header.enc.as_deref() {
        Some(JWE_ENCRYPTION) => {}
        other => {
            return Err(error_stack::report!(invalid_format("payload"))).attach_printable(format!(
                "unsupported JWE enc: expected {JWE_ENCRYPTION}, received {}",
                other.unwrap_or("none")
            ));
        }
    }

    Ok(())
}

/// Decrypt Juspay's response `payload`. The plaintext contains a PAN, so it must
/// never reach a log, an event, or a raw-response field.
pub fn decrypt_payload(
    compact_jwe: &str,
    response_decryption_private_key_pem: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    validate_protected_header(compact_jwe)?;
    ensure_min_rsa_bits(
        response_decryption_private_key_pem,
        true,
        "response_decryption_private_key",
    )?;

    let decrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep256
        .decrypter_from_pem(response_decryption_private_key_pem.peek().as_bytes())
        .change_context(invalid_config("response_decryption_private_key"))
        .attach_printable("failed to load the response-decryption private key")?;

    let (decrypted, _header) = josekit::jwe::deserialize_compact(compact_jwe, &decrypter)
        .change_context(invalid_format("payload"))
        .attach_printable("failed to decrypt the Juspay response payload")?;

    String::from_utf8(decrypted)
        .change_context(invalid_format("payload"))
        .attach_printable("decrypted Juspay response payload is not valid UTF-8")
        .map(Secret::new)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A throwaway RSA key pair per test run — key material is never a fixture.
    struct TestKeyPair {
        public_pem: Secret<String>,
        private_pem: Secret<String>,
    }

    fn generate_key_pair() -> TestKeyPair {
        let rsa = openssl::rsa::Rsa::generate(2048).expect("failed to generate RSA key");
        let pkey = openssl::pkey::PKey::from_rsa(rsa).expect("failed to wrap RSA key");

        TestKeyPair {
            public_pem: Secret::new(
                String::from_utf8(pkey.public_key_to_pem().expect("public pem"))
                    .expect("public pem is utf-8"),
            ),
            private_pem: Secret::new(
                String::from_utf8(pkey.private_key_to_pem_pkcs8().expect("private pem"))
                    .expect("private pem is utf-8"),
            ),
        }
    }

    /// Encrypt under an arbitrary alg/enc pair, to exercise the header check.
    fn encrypt_with(plaintext: &str, public_pem: &Secret<String>, alg: &str, enc: &str) -> String {
        let mut header = josekit::jwe::JweHeader::new();
        header.set_algorithm(alg);
        header.set_content_encryption(enc);

        let encrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep256
            .encrypter_from_pem(public_pem.peek().as_bytes())
            .expect("encrypter");

        josekit::jwe::serialize_compact(plaintext.as_bytes(), &header, &encrypter)
            .expect("serialize")
    }

    const PAYLOAD: &str = r#"{"updatedAccountNumber":"4622943127913727","isAccountUpdated":true,"updatedExpiryDate":"0829"}"#;

    #[test]
    fn round_trips_a_card_payload() {
        let keys = generate_key_pair();
        let plaintext = Secret::new(PAYLOAD.to_string());

        let jwe = encrypt_card_data(&plaintext, &keys.public_pem).expect("encrypt");
        let decrypted = decrypt_payload(jwe.peek(), &keys.private_pem).expect("decrypt");

        assert_eq!(decrypted.peek(), PAYLOAD);
    }

    #[test]
    fn emits_the_agreed_protected_header() {
        let keys = generate_key_pair();
        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &keys.public_pem)
            .expect("encrypt");

        let protected = jwe.peek().split('.').next().expect("protected segment");
        let decoded = BASE64_ENGINE_URL_SAFE_NO_PAD
            .decode(protected)
            .expect("base64url");
        let header: serde_json::Value = serde_json::from_slice(&decoded).expect("json");

        assert_eq!(header["alg"], JWE_ALGORITHM);
        assert_eq!(header["enc"], JWE_ENCRYPTION);
    }

    #[test]
    fn accepts_the_observed_juspay_header() {
        // Every sampled payload carries exactly this header, with no `kid`.
        let protected =
            BASE64_ENGINE_URL_SAFE_NO_PAD.encode(r#"{"alg":"RSA-OAEP-256","enc":"A128GCM"}"#);
        let compact = format!("{protected}.key.iv.ciphertext.tag");

        assert!(validate_protected_header(&compact).is_ok());
    }

    #[test]
    fn rejects_an_unexpected_content_encryption() {
        let keys = generate_key_pair();
        // A128CBC-HS256 is what the existing `jose.rs` helper uses — decryptable
        // by josekit, and exactly the substitution the header check exists for.
        let jwe = encrypt_with(PAYLOAD, &keys.public_pem, JWE_ALGORITHM, "A128CBC-HS256");

        let err = decrypt_payload(&jwe, &keys.private_pem).expect_err("must reject");
        assert_eq!(
            err.current_context(),
            &invalid_format("payload"),
            "unexpected enc must fail as an invalid payload format"
        );
    }

    #[test]
    fn rejects_an_unexpected_key_management_algorithm() {
        let protected =
            BASE64_ENGINE_URL_SAFE_NO_PAD.encode(r#"{"alg":"RSA-OAEP","enc":"A128GCM"}"#);
        let compact = format!("{protected}.key.iv.ciphertext.tag");

        assert!(validate_protected_header(&compact).is_err());
    }

    #[test]
    fn rejects_a_header_missing_alg_or_enc() {
        for header in [r#"{"enc":"A128GCM"}"#, r#"{"alg":"RSA-OAEP-256"}"#, "{}"] {
            let protected = BASE64_ENGINE_URL_SAFE_NO_PAD.encode(header);
            let compact = format!("{protected}.key.iv.ciphertext.tag");
            assert!(
                validate_protected_header(&compact).is_err(),
                "header {header} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_a_wrong_private_key() {
        let keys = generate_key_pair();
        let other = generate_key_pair();

        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &keys.public_pem)
            .expect("encrypt");

        assert!(decrypt_payload(jwe.peek(), &other.private_pem).is_err());
    }

    #[test]
    fn rejects_a_tampered_ciphertext() {
        let keys = generate_key_pair();
        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &keys.public_pem)
            .expect("encrypt");

        let mut segments: Vec<String> = jwe.peek().split('.').map(str::to_string).collect();
        // Flip a character in the ciphertext. A128GCM is AEAD, so the tag check
        // must catch this rather than yielding garbled plaintext.
        let ciphertext = &mut segments[3];
        let first = ciphertext.remove(0);
        ciphertext.insert(0, if first == 'A' { 'B' } else { 'A' });

        assert!(decrypt_payload(&segments.join("."), &keys.private_pem).is_err());
    }

    #[test]
    fn rejects_a_tampered_authentication_tag() {
        let keys = generate_key_pair();
        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &keys.public_pem)
            .expect("encrypt");

        let mut segments: Vec<String> = jwe.peek().split('.').map(str::to_string).collect();
        let tag = &mut segments[4];
        let first = tag.remove(0);
        tag.insert(0, if first == 'A' { 'B' } else { 'A' });

        assert!(decrypt_payload(&segments.join("."), &keys.private_pem).is_err());
    }

    #[test]
    fn rejects_malformed_input() {
        let keys = generate_key_pair();

        for malformed in [
            // The literal value the sandbox was probed with, which Juspay
            // answers with a generic "Internal Server Error".
            "some-random-decryption",
            "",
            "a.b.c",
            "a.b.c.d.e.f",
            "!!!.b.c.d.e",
        ] {
            assert!(
                decrypt_payload(malformed, &keys.private_pem).is_err(),
                "{malformed:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_a_non_utf8_plaintext() {
        let keys = generate_key_pair();

        let mut header = josekit::jwe::JweHeader::new();
        header.set_algorithm(JWE_ALGORITHM);
        header.set_content_encryption(JWE_ENCRYPTION);
        let encrypter = josekit::jwe::alg::rsaes::RsaesJweAlgorithm::RsaOaep256
            .encrypter_from_pem(keys.public_pem.peek().as_bytes())
            .expect("encrypter");
        let jwe =
            josekit::jwe::serialize_compact(&[0xff, 0xfe], &header, &encrypter).expect("serialize");

        assert!(decrypt_payload(&jwe, &keys.private_pem).is_err());
    }

    #[test]
    fn rejects_a_public_key_that_is_not_a_key() {
        let not_a_key =
            Secret::new("-----BEGIN PUBLIC KEY-----\nnope\n-----END PUBLIC KEY-----".to_string());

        let err = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &not_a_key)
            .expect_err("must reject");
        assert_eq!(
            err.current_context(),
            &invalid_config("juspay_encryption_public_key")
        );
    }

    /// Symmetric to the public-key case: a malformed private key must map to the
    /// `response_decryption_private_key` config error, not a generic failure.
    #[test]
    fn rejects_a_private_key_that_is_not_a_key() {
        let keys = generate_key_pair();
        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &keys.public_pem)
            .expect("encrypt");
        let not_a_key =
            Secret::new("-----BEGIN PRIVATE KEY-----\nnope\n-----END PRIVATE KEY-----".to_string());

        let err = decrypt_payload(jwe.peek(), &not_a_key).expect_err("must reject");
        assert_eq!(
            err.current_context(),
            &invalid_config("response_decryption_private_key")
        );
    }

    fn pem_of(bits: u32) -> (Secret<String>, Secret<String>) {
        let rsa = openssl::rsa::Rsa::generate(bits).expect("rsa");
        let pkey = openssl::pkey::PKey::from_rsa(rsa).expect("pkey");
        (
            Secret::new(
                String::from_utf8(pkey.public_key_to_pem().expect("pub pem")).expect("utf8"),
            ),
            Secret::new(
                String::from_utf8(pkey.private_key_to_pem_pkcs8().expect("priv pem"))
                    .expect("utf8"),
            ),
        )
    }

    #[test]
    fn rejects_an_undersized_public_key_on_encrypt() {
        let (weak_public, _) = pem_of(1024);
        let err = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &weak_public)
            .expect_err("a sub-2048-bit key must be rejected");
        assert_eq!(
            err.current_context(),
            &invalid_config("juspay_encryption_public_key")
        );
    }

    #[test]
    fn rejects_an_undersized_private_key_on_decrypt() {
        let strong = generate_key_pair();
        let jwe = encrypt_card_data(&Secret::new(PAYLOAD.to_string()), &strong.public_pem)
            .expect("encrypt");
        let (_, weak_private) = pem_of(1024);

        let err = decrypt_payload(jwe.peek(), &weak_private)
            .expect_err("a sub-2048-bit key must be rejected");
        assert_eq!(
            err.current_context(),
            &invalid_config("response_decryption_private_key")
        );
    }
}
