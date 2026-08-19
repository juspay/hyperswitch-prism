//! Hybrid RSA-OAEP-SHA256 + AES-256-GCM card encryption for BCPG's Direct
//! Card Payment flow (integration guideline section 4.2 "Card Data
//! Encryption").
//!
//! BCPG's scheme is a bespoke three-field payload (`encryptedKey`, `iv`,
//! `ciphertext`) rather than a standard envelope format (e.g. JWE):
//!   1. Generate a fresh 256-bit AES key and 96-bit IV per request.
//!   2. Encrypt the compact card JSON with AES-256-GCM, AAD =
//!      `merchantId|referenceId|createdEpochSeconds`, and append the
//!      16-byte authentication tag to the ciphertext bytes.
//!   3. Encrypt the AES key with the payment gateway's RSA public key using
//!      OAEP padding, with SHA-256 as both the OAEP digest and the MGF1
//!      digest (BCPG: `RSA/ECB/OAEPWithSHA-256AndMGF1Padding`).
//!   4. Base64-encode all three outputs.
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use base64::Engine;
use domain_types::errors;
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use openssl::{md::Md, pkey::PKey, pkey_ctx::PkeyCtx, rsa::Padding, symm::Cipher};
use serde::Serialize;

use super::transformers::BASE64_ENGINE;

/// 256-bit AES key.
const AES_KEY_LEN: usize = 32;
/// 96-bit GCM IV, per BCPG's spec (integration guideline section 4.2.2).
const GCM_IV_LEN: usize = 12;
/// 128-bit GCM authentication tag, appended to the ciphertext.
const GCM_TAG_LEN: usize = 16;

/// The `Card` object (integration guideline section 3.7.10), serialized
/// compactly (no whitespace) before encryption — BCPG's own debugging
/// checklist calls this out explicitly as a requirement, not just a nicety.
///
/// Fields are `Secret<String>` (not plain `String`) so an accidental
/// `{:?}`/`attach_printable` on this value — a panic message, a stray debug
/// log — can't leak the raw PAN/CVV; `Secret`'s `Debug` impl masks the
/// inner value while its `Serialize` impl still emits it, which is what
/// `serde_json::to_vec` below actually needs.
#[derive(Debug, Serialize)]
struct BoostCardPlaintext {
    number: Secret<String>,
    #[serde(rename = "expMonth")]
    exp_month: Secret<String>,
    #[serde(rename = "expYear")]
    exp_year: Secret<String>,
    cvv: Secret<String>,
    #[serde(rename = "cardHolderName")]
    card_holder_name: Secret<String>,
}

/// The `EncryptedCardDetails` object (integration guideline section 3.7.9).
#[derive(Debug, Clone)]
pub struct BoostEncryptedCard {
    pub encrypted_key: String,
    pub iv: String,
    pub ciphertext: String,
}

/// How long a cache entry may sit unconsumed before it's swept as
/// abandoned. Authorize requests complete in low single-digit seconds
/// end-to-end; anything still cached past this was left behind by a
/// request that errored between the two calls (see below) and never
/// reached the second, consuming call.
const CACHE_ENTRY_TTL: Duration = Duration::from_secs(60);

struct CachedEncryptedCard {
    card: BoostEncryptedCard,
    inserted_at: Instant,
}

/// Caches one encrypted card per `(merchant_id, referenceId)`, consumed
/// exactly twice and then evicted.
///
/// The framework calls this connector's request-building transform
/// independently twice per logical Authorize: once inside `build_headers`
/// (to compute the body it signs) and again for the actual body it sends
/// (see `stable_created_timestamp` in transformers.rs for the same
/// double-invocation issue with the `created` field). A timestamp can get
/// away with "close enough" — truncated to the second, the two calls
/// normally land within the same second. RSA-OAEP + AES-256-GCM cannot:
/// OAEP's padding is randomized by design (a core part of its security
/// proof, not a flaw to work around), so calling `encrypt_card` twice with
/// byte-identical inputs still yields a completely different ciphertext
/// each time. Without this cache, the signed body (call #1) and the sent
/// body (call #2) would never match — every Direct Card Payment request
/// would 401, deterministically, not occasionally.
///
/// Keyed by `(merchant_id, referenceId)` rather than `referenceId` alone —
/// this is a single process-wide static shared by every merchant this UCS
/// instance serves, so keying by referenceId alone would let one merchant's
/// cached ciphertext be handed back to a different merchant if their
/// referenceIds ever collided.
///
/// Entries are normally removed on the second read. `CACHE_ENTRY_TTL` bounds
/// the ones that aren't: if a request errors between the two calls (HMAC
/// signing fails, some other step aborts), its entry would otherwise never
/// be removed and the map would grow unboundedly over the life of the
/// process.
fn encrypted_card_cache() -> &'static Mutex<HashMap<String, CachedEncryptedCard>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedEncryptedCard>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn encryption_failed(context: String) -> errors::IntegrationError {
    errors::IntegrationError::RequestEncodingFailed {
        context: errors::IntegrationErrorContext {
            suggested_action: None,
            doc_url: None,
            additional_context: Some(context),
        },
    }
}

fn invalid_config(context: String) -> errors::IntegrationError {
    errors::IntegrationError::InvalidConnectorConfig {
        config: "boost.public_key",
        context: errors::IntegrationErrorContext {
            suggested_action: Some(
                "Configure Boost's public_key with a base64-encoded, raw DER X.509 \
                 SubjectPublicKeyInfo RSA public key — the exact bytes returned by \
                 GET /v1/payments/card-encryption-key (integration guideline section \
                 4.2.3), base64-encoded."
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some(context),
        },
    }
}

/// Zero-pad a 1-digit expiry month to 2 digits (BCPG's sample always shows
/// 2 digits, e.g. `"08"`, `"12"`).
fn normalize_exp_month(exp_month: &str) -> String {
    let trimmed = exp_month.trim();
    if trimmed.len() == 1 {
        format!("0{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Encrypt a card for BCPG's Direct Card Payment flow.
///
/// `merchant_id`, `reference_id`, and `created_epoch_seconds` must be the
/// exact same values sent in the payment request body — BCPG's AAD binds
/// the ciphertext to them and rejects any mismatch (integration guideline
/// section 4.2.5).
#[allow(clippy::too_many_arguments)]
pub fn encrypt_card(
    card_number: &str,
    card_exp_month: &str,
    card_exp_year: &str,
    card_cvc: &str,
    card_holder_name: &str,
    public_key_der_b64: &Secret<String>,
    merchant_id: &str,
    reference_id: &str,
    created_epoch_seconds: i64,
) -> Result<BoostEncryptedCard, error_stack::Report<errors::IntegrationError>> {
    let cache_key = format!("{merchant_id}:{reference_id}");

    // See `encrypted_card_cache`: the second of the two calls the framework
    // makes per logical Authorize hits this and returns immediately, without
    // re-running any crypto.
    {
        let mut cache = encrypted_card_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Sweep anything abandoned by a prior request that errored between
        // its two calls, so the cache can't grow unboundedly.
        cache.retain(|_, entry| entry.inserted_at.elapsed() < CACHE_ENTRY_TTL);
        if let Some(cached) = cache.remove(&cache_key) {
            return Ok(cached.card);
        }
    }

    let card = BoostCardPlaintext {
        number: Secret::new(card_number.to_string()),
        exp_month: Secret::new(normalize_exp_month(card_exp_month)),
        // Sent as-is (UCS always supplies a 4-digit year). BCPG's own doc
        // sample and a written debugging checklist both say to use a
        // 2-digit year, but live testing against BCPG's staging Direct Card
        // Payment flow showed the opposite: a 2-digit year makes BCPG's own
        // redirect page build a truncated expiry (`MPI_PAN_EXP` ends up as
        // just the month) when forwarding to their downstream Paydee
        // processor, which then rejects the card (`MPI_ERROR_CODE 203 /
        // cardExpiryDate`) with no MAC — the "Unable to do MAC
        // Verification" failure. Confirmed empirical behavior over the
        // documented sample/checklist: keep the 4-digit year.
        exp_year: Secret::new(card_exp_year.to_string()),
        cvv: Secret::new(card_cvc.to_string()),
        card_holder_name: Secret::new(card_holder_name.to_string()),
    };

    // serde_json::to_vec produces compact JSON (no whitespace) by construction.
    let card_json = serde_json::to_vec(&card)
        .change_context(encryption_failed(
            "Failed to serialize the card object for Boost's Direct Card Payment \
             encryption."
                .to_string(),
        ))
        .attach_printable("card JSON serialization failed")?;

    let der_bytes = BASE64_ENGINE
        .decode(public_key_der_b64.peek().as_bytes())
        .change_context(invalid_config(
            "Boost's configured public_key is not valid base64.".to_string(),
        ))?;
    let public_key = PKey::public_key_from_der(&der_bytes)
        .change_context(invalid_config(
            "Boost's configured public_key does not decode as a DER X.509 \
             SubjectPublicKeyInfo RSA public key."
                .to_string(),
        ))
        .attach_printable("failed to parse RSA public key")?;

    let mut aes_key = [0u8; AES_KEY_LEN];
    openssl::rand::rand_bytes(&mut aes_key)
        .change_context(encryption_failed(
            "Failed to generate a random AES-256 key for card encryption.".to_string(),
        ))
        .attach_printable("RNG failure generating AES key")?;

    let mut iv = [0u8; GCM_IV_LEN];
    openssl::rand::rand_bytes(&mut iv)
        .change_context(encryption_failed(
            "Failed to generate a random GCM IV for card encryption.".to_string(),
        ))
        .attach_printable("RNG failure generating GCM IV")?;

    let aad = format!("{merchant_id}|{reference_id}|{created_epoch_seconds}");

    let mut tag = [0u8; GCM_TAG_LEN];
    let aes_ciphertext = openssl::symm::encrypt_aead(
        Cipher::aes_256_gcm(),
        &aes_key,
        Some(&iv),
        aad.as_bytes(),
        &card_json,
        &mut tag,
    )
    .change_context(encryption_failed(
        "AES-256-GCM encryption of the card payload failed.".to_string(),
    ))
    .attach_printable("AES-GCM encryption failed")?;

    // BCPG expects the ciphertext and the 16-byte auth tag concatenated in a
    // single base64 field, tag last (integration guideline section 4.2.6).
    let mut ciphertext_with_tag = aes_ciphertext;
    ciphertext_with_tag.extend_from_slice(&tag);

    // RSA-OAEP with SHA-256 for both the OAEP digest and MGF1 — the
    // high-level `Rsa::public_encrypt` API only supports SHA-1, so this
    // requires the lower-level PkeyCtx to select SHA-256 for both.
    let mut ctx = PkeyCtx::new(&public_key)
        .change_context(encryption_failed(
            "Failed to initialize the RSA encryption context.".to_string(),
        ))
        .attach_printable("PkeyCtx::new failed")?;
    ctx.encrypt_init()
        .change_context(encryption_failed(
            "Failed to initialize RSA-OAEP encryption.".to_string(),
        ))
        .attach_printable("encrypt_init failed")?;
    ctx.set_rsa_padding(Padding::PKCS1_OAEP)
        .change_context(encryption_failed(
            "Failed to set RSA-OAEP padding.".to_string(),
        ))
        .attach_printable("set_rsa_padding failed")?;
    ctx.set_rsa_oaep_md(Md::sha256())
        .change_context(encryption_failed(
            "Failed to set the RSA-OAEP digest to SHA-256.".to_string(),
        ))
        .attach_printable("set_rsa_oaep_md failed")?;
    ctx.set_rsa_mgf1_md(Md::sha256())
        .change_context(encryption_failed(
            "Failed to set the RSA-OAEP MGF1 digest to SHA-256.".to_string(),
        ))
        .attach_printable("set_rsa_mgf1_md failed")?;

    let mut encrypted_key = Vec::new();
    ctx.encrypt_to_vec(&aes_key, &mut encrypted_key)
        .change_context(encryption_failed(
            "RSA-OAEP encryption of the AES key failed — this can indicate the \
             configured Boost public_key is stale or malformed."
                .to_string(),
        ))
        .attach_printable("RSA-OAEP encryption failed")?;

    let result = BoostEncryptedCard {
        encrypted_key: BASE64_ENGINE.encode(encrypted_key),
        iv: BASE64_ENGINE.encode(iv),
        ciphertext: BASE64_ENGINE.encode(ciphertext_with_tag),
    };

    // Stash for the second call (see `encrypted_card_cache`) so it gets the
    // byte-identical ciphertext this call is about to return, instead of
    // re-running OAEP/GCM and producing a different one.
    encrypted_card_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            cache_key,
            CachedEncryptedCard {
                card: result.clone(),
                inserted_at: Instant::now(),
            },
        );

    Ok(result)
}
