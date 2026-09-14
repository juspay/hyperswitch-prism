//! HiPay signature verification.
//!
//! HiPay's signature scheme is **one-directional: HiPay → merchant**. It authenticates what
//! HiPay sends *to* the merchant; requests the merchant sends *to* the gateway are
//! authenticated by HTTP Basic credentials alone. `POST /v1/order` — and every maintenance
//! and sync call — therefore carries **no** signature, hash or MAC parameter, and none is
//! computed for them. (Error `1000002 Incorrect Signature` refers to inbound-verification
//! configuration, not to order requests.)
//!
//! There are two inbound channels, with **different** algorithms:
//!
//! | Channel | Carrier | Preimage |
//! |---|---|---|
//! | Server-to-server notification | `x-allopass-signature` header | raw POST body ‖ passphrase |
//! | Redirect / return URL | `hash` query parameter | for each param, sorted by name: name ‖ value ‖ passphrase |
//!
//! The passphrase is a separate credential from the Basic-auth API login/password,
//! configured in the HiPay back office under *Integration → Security settings*.
//!
//! Nothing on the Authorize path calls into this module; it exists so the inbound channels
//! verify against one implementation of each preimage rather than two.

use common_utils::{
    crypto::{GenerateDigest, Sha256, Sha512},
    errors::CustomResult,
};
use domain_types::errors::{IntegrationError, IntegrationErrorContext, WebhookError};
use hyperswitch_masking::{PeekInterface, Secret};

/// Digest algorithms HiPay can be configured to sign with. SHA-256 is the default; the
/// merchant selects the algorithm in the back office alongside the passphrase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HipaySignatureAlgorithm {
    #[default]
    Sha256,
    Sha512,
    /// HiPay still offers SHA-1. It is weak, but a merchant account configured for it
    /// produces SHA-1 digests and the notification cannot be verified any other way.
    Sha1,
}

impl HipaySignatureAlgorithm {
    /// Lowercase hex digest, the encoding HiPay uses in both carriers.
    fn hex_digest(self, message: &[u8]) -> CustomResult<String, IntegrationError> {
        let digest = match self {
            Self::Sha256 => Sha256
                .generate_digest(message)
                .change_context(unverifiable("SHA-256 digest failed"))?,
            Self::Sha512 => Sha512
                .generate_digest(message)
                .change_context(unverifiable("SHA-512 digest failed"))?,
            Self::Sha1 => ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, message)
                .as_ref()
                .to_vec(),
        };
        Ok(hex::encode(digest))
    }
}

fn unverifiable(detail: &'static str) -> IntegrationError {
    IntegrationError::NotSupported {
        message: "HiPay signature verification".to_string(),
        connector: "hipay",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Check the signature algorithm configured in the HiPay back office under \
                 Integration → Security settings."
                    .to_string(),
            ),
            doc_url: Some(
                "https://developer.hipay.com/payment-fundamentals/requirements/signature-verification"
                    .to_string(),
            ),
            additional_context: Some(detail.to_string()),
        },
    }
}

/// Preimage of the `x-allopass-signature` header: the **raw, unparsed** POST body
/// concatenated directly with the passphrase.
///
/// The body must be captured before any form or XML parsing — re-serialising a parsed body
/// does not reproduce the digest.
pub fn notification_preimage(raw_body: &[u8], passphrase: &Secret<String>) -> Vec<u8> {
    let mut preimage = raw_body.to_vec();
    preimage.extend_from_slice(passphrase.peek().as_bytes());
    preimage
}

/// Verify the `x-allopass-signature` header against the raw notification body.
pub fn verify_notification_signature(
    raw_body: &[u8],
    passphrase: &Secret<String>,
    algorithm: HipaySignatureAlgorithm,
    received_signature: &str,
) -> CustomResult<bool, WebhookError> {
    let expected = algorithm
        .hex_digest(&notification_preimage(raw_body, passphrase))
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    Ok(constant_time_eq(&expected, received_signature.trim()))
}

/// Parameter names that never take part in the redirect hash: the carrier itself, and the
/// legacy alias HiPay documents alongside it.
const REDIRECT_HASH_EXCLUDED_PARAMS: [&str; 2] = ["hash", "response"];

/// Preimage of the redirect `hash` query parameter.
///
/// The rules, in order:
/// 1. sort the parameters alphabetically by name;
/// 2. drop empty values and the `hash` / `response` parameters themselves;
/// 3. take each value **URL-decoded** (callers pass already-decoded values);
/// 4. stringify JSON booleans and numbers inside structured values — a JSON `true` becomes
///    the string `"1"`;
/// 5. concatenate `name ‖ value ‖ passphrase` for every parameter, in that order.
pub fn redirect_hash_preimage(params: &[(String, String)], passphrase: &Secret<String>) -> String {
    let mut retained: Vec<(&str, String)> = params
        .iter()
        .filter(|(name, value)| {
            !value.is_empty() && !REDIRECT_HASH_EXCLUDED_PARAMS.contains(&name.as_str())
        })
        .map(|(name, value)| (name.as_str(), normalize_structured_value(value)))
        .collect();
    retained.sort_by_key(|(name, _)| *name);

    retained
        .into_iter()
        .map(|(name, value)| format!("{name}{value}{}", passphrase.peek()))
        .collect()
}

/// Verify the `hash` query parameter of a redirect / return URL.
pub fn verify_redirect_hash(
    params: &[(String, String)],
    passphrase: &Secret<String>,
    algorithm: HipaySignatureAlgorithm,
    received_hash: &str,
) -> CustomResult<bool, WebhookError> {
    let expected = algorithm
        .hex_digest(redirect_hash_preimage(params, passphrase).as_bytes())
        .change_context(WebhookError::WebhookSourceVerificationFailed)?;
    Ok(constant_time_eq(&expected, received_hash.trim()))
}

/// A structured (JSON object or array) parameter value is re-serialised with its booleans
/// and numbers turned into strings, because that is the form HiPay hashes. A scalar value
/// is hashed exactly as it arrived — notably `amount=125.7` stays `125.7`, unquoted.
fn normalize_structured_value(value: &str) -> String {
    let trimmed = value.trim_start();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return value.to_string();
    }
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .map(stringify_json_scalars)
        .and_then(|normalized| serde_json::to_string(&normalized).ok())
        .unwrap_or_else(|| value.to_string())
}

fn stringify_json_scalars(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Bool(flag) => {
            serde_json::Value::String(if flag { "1" } else { "0" }.to_string())
        }
        serde_json::Value::Number(number) => serde_json::Value::String(number.to_string()),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(stringify_json_scalars).collect())
        }
        serde_json::Value::Object(entries) => serde_json::Value::Object(
            entries
                .into_iter()
                .map(|(key, entry)| (key, stringify_json_scalars(entry)))
                .collect(),
        ),
        other => other,
    }
}

/// Compare two hex digests without leaking where they first differ.
fn constant_time_eq(expected: &str, received: &str) -> bool {
    let expected = expected.to_lowercase();
    let received = received.to_lowercase();
    let (expected, received) = (expected.as_bytes(), received.as_bytes());
    // Fold every byte into the accumulator so the comparison takes the same time whatever
    // the digests are; a length mismatch alone already makes them unequal.
    let mut difference = u8::from(expected.len() != received.len());
    for (index, expected_byte) in expected.iter().enumerate() {
        difference |= expected_byte ^ received.get(index).copied().unwrap_or(0);
    }
    difference == 0
}

use error_stack::ResultExt;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn passphrase() -> Secret<String> {
        Secret::new("SecretPassphrase".to_string())
    }

    /// The worked example published on HiPay's signature-verification page: with
    /// `amount=125.7`, `currency=EUR`, `custom_data={"testing":true}` and
    /// `orderid=15424657`, the string to hash is exactly this. Note the boolean `true`
    /// stringified to `"1"` inside `custom_data`, and `125.7` left unquoted.
    #[test]
    fn redirect_preimage_matches_the_documented_example() {
        let params = vec![
            ("orderid".to_string(), "15424657".to_string()),
            ("amount".to_string(), "125.7".to_string()),
            ("custom_data".to_string(), r#"{"testing":true}"#.to_string()),
            ("currency".to_string(), "EUR".to_string()),
        ];

        assert_eq!(
            redirect_hash_preimage(&params, &passphrase()),
            concat!(
                "amount125.7SecretPassphrase",
                "currencyEURSecretPassphrase",
                r#"custom_data{"testing":"1"}SecretPassphrase"#,
                "orderid15424657SecretPassphrase",
            )
        );
    }

    #[test]
    fn redirect_preimage_drops_empty_values_and_the_hash_parameter() {
        let params = vec![
            ("hash".to_string(), "deadbeef".to_string()),
            ("response".to_string(), "ignored".to_string()),
            ("state".to_string(), String::new()),
            ("orderid".to_string(), "15424657".to_string()),
        ];

        assert_eq!(
            redirect_hash_preimage(&params, &passphrase()),
            "orderid15424657SecretPassphrase"
        );
    }

    #[test]
    fn redirect_hash_verifies_the_documented_example() {
        let params = vec![
            ("amount".to_string(), "125.7".to_string()),
            ("currency".to_string(), "EUR".to_string()),
            ("custom_data".to_string(), r#"{"testing":true}"#.to_string()),
            ("orderid".to_string(), "15424657".to_string()),
        ];

        assert!(verify_redirect_hash(
            &params,
            &passphrase(),
            HipaySignatureAlgorithm::Sha256,
            "4ba55196d83f32dd9c47489834ede83881d3f23dacd835c2fc32965a57296c94",
        )
        .expect("digest"));
    }

    /// The notification preimage is the raw body followed immediately by the passphrase,
    /// with no separator and no re-serialisation of the body.
    #[test]
    fn notification_preimage_is_raw_body_then_passphrase() {
        let body = br#"{"status":"118","transactionReference":"800451428387"}"#;

        assert_eq!(
            notification_preimage(body, &passphrase()),
            br#"{"status":"118","transactionReference":"800451428387"}SecretPassphrase"#.to_vec()
        );

        assert!(verify_notification_signature(
            body,
            &passphrase(),
            HipaySignatureAlgorithm::Sha256,
            "6663ee693fa0c63c8c8864a094203599cd675c7c4342e10e2e7e0da29e5f0d4e",
        )
        .expect("digest"));
    }

    #[test]
    fn notification_signature_rejects_a_wrong_digest() {
        assert!(!verify_notification_signature(
            b"{}",
            &passphrase(),
            HipaySignatureAlgorithm::Sha256,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("digest"));
    }
}
