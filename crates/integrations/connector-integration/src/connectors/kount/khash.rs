//! Kount KHASH payment-token hashing.
//!
//! KHASH is Kount's salted, prefix-preserving one-way hash for payment
//! instrument numbers. Seeded with a merchant's Kount-issued configuration
//! key, the same input number always yields the same token, letting Kount
//! link an instrument across orders without ever receiving the raw number.
//!
//! Algorithm (per Kount's specification):
//!
//! 1. The configuration key is a Kount-flavoured Ascii85 string; decoding it
//!    yields the salt (see [`decode_config_key`]).
//! 2. The message is `"{number}.{salt}"`, CRLF-normalised and encoded one
//!    UTF-16 code unit at a time — ASCII stays a single byte, other units
//!    become two/three-byte sequences, and supplementary characters become
//!    two three-byte sequences (CESU-8 style, not UTF-8).
//! 3. The token is the first six characters of the input number (the BIN for
//!    a card) followed by `length` base-36 characters (capped at 17), each
//!    derived from a seven-hex-digit window of the SHA-1 digest of that
//!    message, stepping two hex digits per output character and reducing
//!    modulo 36.
//!
//! SHA-1 is retained deliberately for KHASH protocol compatibility; this is
//! not a general-purpose password hasher.

use sha1::{Digest, Sha1};

/// Suffix length used for card payment tokens: six preserved prefix
/// characters (the BIN) plus this many base-36 characters — 20 characters
/// total for a 16-digit PAN, matching Kount's reference behaviour.
pub(crate) const CARD_TOKEN_SUFFIX_LENGTH: usize = 14;

/// Maximum suffix characters KHASH emits, regardless of the requested length.
const MAX_SUFFIX_LENGTH: usize = 17;

/// Number of leading characters of the input preserved verbatim in the token.
const PRESERVED_PREFIX_LENGTH: usize = 6;

/// Base-36 alphabet used for the hash suffix.
const BASE36_ALPHABET: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// A KHASH generator seeded with a merchant's Kount configuration key.
pub(crate) struct Khash {
    /// Salt decoded from the Kount-issued (Ascii85) configuration key.
    salt: String,
}

// The salt derives from a merchant secret; `Debug` must never expose it.
impl std::fmt::Debug for Khash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Khash").finish_non_exhaustive()
    }
}

impl Khash {
    /// Create a generator from a Kount-issued configuration key. The key is
    /// Ascii85-encoded with Kount-specific expansions and is not validated —
    /// any input decodes to some salt, matching Kount's reference behaviour.
    pub(crate) fn new(config_key: &str) -> Self {
        Self {
            salt: decode_config_key(config_key),
        }
    }

    /// KHASH an instrument number.
    ///
    /// Card numbers should be passed as strings to preserve leading zeroes.
    /// A `suffix_length` of zero returns only the preserved prefix.
    pub(crate) fn hash(&self, number: &str, suffix_length: usize) -> String {
        let message = format!("{number}.{}", self.salt);
        let digest = Sha1::digest(encode_message(&message));
        let hex = hex::encode(digest);

        let prefix: Vec<u16> = number
            .encode_utf16()
            .take(PRESERVED_PREFIX_LENGTH)
            .collect();
        let mut token = String::from_utf16_lossy(&prefix);

        let suffix_chars = suffix_length.min(MAX_SUFFIX_LENGTH);
        for offset in (0..2 * suffix_chars).step_by(2) {
            if offset >= hex.len() {
                break;
            }
            // The final window may be shorter than seven hexadecimal digits.
            let window_end = (offset + 7).min(hex.len());
            let window = hex.get(offset..window_end).unwrap_or_default();
            let value = u32::from_str_radix(window, 16).unwrap_or_default();
            let index = usize::try_from(value % 36).unwrap_or_default();
            let digit = char::from(BASE36_ALPHABET.get(index).copied().unwrap_or(b'0'));
            token.push(digit);
        }
        token
    }
}

/// Decode a Kount configuration key into the KHASH salt.
///
/// The key is Ascii85 with Kount-specific rules: whitespace is stripped, `z`
/// expands to the zero group (`!!!!!`), `y` expands to the six-character
/// sequence `+<VdL/`, partial groups are padded with `u`, and each
/// five-character group folds into four big-endian bytes via
/// `value = value * 85 + (unit - 33)` (wrapping). The result is a string of
/// byte-valued characters, not UTF-8 text.
fn decode_config_key(key: &str) -> String {
    let expanded = key
        .chars()
        .filter(|c| !is_key_whitespace(*c))
        .collect::<String>()
        .replace('z', "!!!!!")
        .replace('y', "+<VdL/");

    let mut units: Vec<u16> = expanded.encode_utf16().collect();
    let padding = (5 - units.len() % 5) % 5;
    units.resize(units.len() + padding, u16::from(b'u'));

    let mut bytes = Vec::with_capacity(units.len() / 5 * 4);
    for chunk in units.as_chunks::<5>().0 {
        let value = chunk.iter().fold(0u32, |value, &unit| {
            value
                .wrapping_mul(85)
                .wrapping_add(u32::from(unit).wrapping_sub(33))
        });
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes.truncate(bytes.len().saturating_sub(padding));
    bytes.into_iter().map(char::from).collect()
}

/// Whitespace stripped from configuration keys before decoding.
fn is_key_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// Encode the hash message: CRLF is normalised to LF and each UTF-16 code unit
/// is encoded individually (ASCII as one byte, other units as two/three-byte
/// sequences — supplementary characters therefore encode as two three-byte
/// sequences rather than one four-byte UTF-8 sequence).
fn encode_message(message: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(message.len());
    for unit in message.replace("\r\n", "\n").encode_utf16() {
        match unit {
            0x00..=0x7f => bytes.push(u8::try_from(unit).unwrap_or_default()),
            0x80..=0x7ff => {
                bytes.push(u8::try_from((unit >> 6) | 0xc0).unwrap_or_default());
                bytes.push(u8::try_from((unit & 0x3f) | 0x80).unwrap_or_default());
            }
            _ => {
                bytes.push(u8::try_from((unit >> 12) | 0xe0).unwrap_or_default());
                bytes.push(u8::try_from(((unit >> 6) & 0x3f) | 0x80).unwrap_or_default());
                bytes.push(u8::try_from((unit & 0x3f) | 0x80).unwrap_or_default());
            }
        }
    }
    bytes
}
