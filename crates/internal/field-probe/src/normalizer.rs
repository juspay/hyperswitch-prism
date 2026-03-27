use std::collections::BTreeMap;

use crate::types::SamplePayload;

/// Normalize content for stable output - replaces dynamic values with placeholders.
/// Applied to HTTP request headers and body strings so that docs are stable across runs.
pub(crate) fn normalize_content(s: &str) -> String {
    let mut result = s.to_string();

    // 1. Datetime strings (date headers, ISO 8601 timestamps in bodies)
    result = replace_datetimes(&result);
    // 2. UUID patterns (8-4-4-4-12 hex format)
    result = replace_uuids(&result);
    // 3. Long numeric timestamps (13+ consecutive digits, e.g. YYYYMMDDHHMMSS or ms epoch)
    result = replace_timestamps(&result);
    // 4. Lowercase hex digests ≥32 chars (SHA256/HMAC checksums)
    result = replace_hex_digests(&result);
    // 5. Base64 values ≥20 chars with = padding (short HMACs, nonces with padding)
    result = replace_base64_signatures(&result);
    // 6. Known-random JSON field values (nonce, invoiceNumber, etc.)
    result = replace_json_dynamic_fields(&result);

    result
}

/// Copy one Unicode scalar value from `s` at byte offset `*i` into `result`,
/// advancing `*i` by the correct number of bytes (1 for ASCII, 2-4 for multi-byte).
/// This prevents UTF-8 corruption when scanner functions fall through to their
/// "copy one byte" path for non-ASCII characters like `•` (U+2022, 3 bytes).
#[inline]
fn push_char_at(result: &mut String, s: &str, i: &mut usize) {
    let b = s.as_bytes()[*i];
    if b < 0x80 {
        result.push(b as char);
        *i += 1;
    } else {
        let c = s[*i..].chars().next().expect("valid UTF-8");
        result.push(c);
        *i += c.len_utf8();
    }
}

/// Replace UUID patterns using byte-based matching to ensure UTF-8 safety
fn replace_uuids(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    let placeholder = "00000000-0000-0000-0000-000000000000";

    while i < bytes.len() {
        // Look for start of potential UUID (hex digit)
        if bytes[i].is_ascii_hexdigit() {
            let mut j = i;
            let mut count = 0;
            let mut dashes = 0;
            let mut positions = [0usize; 4];

            // Scan forward up to 36 characters in the original bytes
            while j < bytes.len() && count < 36 {
                let b = bytes[j];
                if b == b'-' {
                    if dashes < 4 {
                        positions[dashes] = count;
                    }
                    dashes += 1;
                } else if !b.is_ascii_hexdigit() {
                    break;
                }
                j += 1;
                count += 1;
            }

            // Check if this matches UUID pattern: 8-4-4-4-12
            if count == 36
                && dashes == 4
                && positions[0] == 8
                && positions[1] == 13
                && positions[2] == 18
                && positions[3] == 23
            {
                result.push_str(placeholder);
                i = j; // advance past the UUID in the original
                continue;
            }
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace timestamp patterns (13+ consecutive digits)
fn replace_timestamps(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        // Look for start of digit sequence
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let count = i - start;

            // 13 digits = millisecond-epoch timestamp; 14 digits = compact YYYYMMDDHHMMSS
            // (16-digit card numbers must NOT be replaced)
            if count == 13 {
                result.push_str("0000000000000");
                continue;
            }
            if count == 14 {
                result.push_str("00000000000000");
                continue;
            }
            // Not a timestamp — copy the digits verbatim
            result.push_str(&s[start..i]);
            continue;
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace base64 signature patterns (20+ base64 body chars with trailing `=` padding).
///
/// Real base64 padding `=` only appears at the very END of a value, never followed by
/// more alphanumeric chars.  URL-encoded params use `key=value` where alphanumeric
/// content follows the `=`, so they are correctly excluded.
fn replace_base64_signatures(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    // Static base64 value (decodes to "probe_key:probe_secret")
    let placeholder = "cHJvYmVfa2V5OnByb2JlX3NlY3JldA==";

    while i < bytes.len() {
        let b = bytes[i];
        // Phase 1: consume base64 body chars [A-Za-z0-9+/]
        if b.is_ascii_alphanumeric() || b == b'+' || b == b'/' {
            let start = i;
            while i < bytes.len() {
                let cb = bytes[i];
                if cb.is_ascii_alphanumeric() || cb == b'+' || cb == b'/' {
                    i += 1;
                } else {
                    break;
                }
            }
            let body_count = i - start;

            // Phase 2: consume trailing `=` padding (at most 2)
            let pad_start = i;
            while i < bytes.len() && bytes[i] == b'=' && (i - pad_start) < 2 {
                i += 1;
            }
            let pad_count = i - pad_start;

            // Valid base64: padding must NOT be followed by more body chars.
            // If it is, this `=` is a URL `key=value` delimiter, not base64 padding.
            let followed_by_body = i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'+' || bytes[i] == b'/');

            if pad_count > 0 && !followed_by_body && body_count >= 20 {
                result.push_str(placeholder);
                continue;
            }
            result.push_str(&s[start..i]);
            continue;
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace datetime strings with a stable placeholder.
/// Handles:
///   - `2026-03-13 9:29:07.123311 +00:00:00`  (space-separated, Barclaycard/CyberSource)
///   - `2026-03-13T09:29:07+00:00`             (ISO 8601 with timezone)
///   - `2026-03-13T09:29:22.388Z`              (ISO 8601 with ms, UTC)
fn replace_datetimes(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let placeholder = "2020-01-01T00:00:00+00:00";
    let mut i = 0;

    while i < bytes.len() {
        // Look for YYYY-MM-DD (needs 10 bytes: 4 digits + '-' + 2 digits + '-' + 2 digits)
        if i + 10 <= bytes.len()
            && bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
            && bytes[i + 4] == b'-'
            && bytes[i + 5].is_ascii_digit()
            && bytes[i + 6].is_ascii_digit()
            && bytes[i + 7] == b'-'
            && bytes[i + 8].is_ascii_digit()
            && bytes[i + 9].is_ascii_digit()
        {
            let sep_pos = i + 10;
            // Must be followed by 'T' or ' ' and then a digit (time component)
            if sep_pos + 1 < bytes.len()
                && (bytes[sep_pos] == b'T' || bytes[sep_pos] == b' ')
                && bytes[sep_pos + 1].is_ascii_digit()
            {
                let mut j = sep_pos + 1;
                // Consume time digits, colons, and decimal point
                while j < bytes.len() {
                    let b = bytes[j];
                    if b.is_ascii_digit() || b == b':' || b == b'.' {
                        j += 1;
                    } else if b == b'Z' {
                        j += 1;
                        break;
                    } else if b == b'+' || b == b'-' {
                        // Timezone offset: ±HH:MM or ±HH:MM:SS
                        j += 1;
                        while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b':') {
                            j += 1;
                        }
                        break;
                    } else if b == b' ' {
                        // Allow " +" or " -" for timezone like " +00:00:00"
                        if j + 1 < bytes.len() && (bytes[j + 1] == b'+' || bytes[j + 1] == b'-') {
                            j += 1; // consume space; next iteration picks up +/-
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.push_str(placeholder);
                i = j;
                continue;
            }
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace lowercase hex digests ≥32 chars (SHA-256 checksums, long HMACs).
/// Requires at least one a-f letter to avoid replacing pure-digit strings.
fn replace_hex_digests(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    // Static SHA-256 hex digest (hash of "probe")
    let placeholder = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3";
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_digit() || (b'a'..=b'f').contains(&b) {
            let start = i;
            let mut has_letter = false;
            while i < bytes.len() {
                let cb = bytes[i];
                if cb.is_ascii_digit() {
                    i += 1;
                } else if (b'a'..=b'f').contains(&cb) {
                    has_letter = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let count = i - start;
            if count >= 32 && has_letter {
                result.push_str(placeholder);
                continue;
            }
            result.push_str(&s[start..i]);
            continue;
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace known-random JSON field values that aren't caught by the other normalizers.
fn replace_json_dynamic_fields(s: &str) -> String {
    let mut r = s.to_string();

    // Purely alphanumeric random values (no base64 padding, not hex)
    for (key, placeholder) in &[
        ("\"nonce\"", "\"probeNonceVal001\""),
        ("\"invoiceNumber\"", "\"ProbeInvoiceRef1\""),
    ] {
        r = replace_json_alphanum_value(&r, key, placeholder);
    }

    // 10-digit numeric timestamps (too short for replace_timestamps which needs 13+)
    for key in &["\"timestamp\"", "\"requestTimestamp\""] {
        r = replace_json_digit_value(&r, key, "\"0000000000\"");
    }

    // Paybox NUMQUESTION URL parameter (9-digit number derived from wall clock)
    r = replace_url_param_digits(&r, "NUMQUESTION", "000000000");

    r
}

/// Replace `"KEY":"ALPHANUM"` or `"KEY": "ALPHANUM"` where the value is purely alphanumeric
/// and not already a probe placeholder.
fn replace_json_alphanum_value(s: &str, json_key: &str, replacement: &str) -> String {
    let key_bytes = json_key.as_bytes();
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        if i + key_bytes.len() <= bytes.len() && bytes[i..i + key_bytes.len()] == *key_bytes {
            let mut j = i + key_bytes.len();
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                j += 1;
                while j < bytes.len() && bytes[j] == b' ' {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut k = val_start;
                    while k < bytes.len() && bytes[k] != b'"' && bytes[k] != b'\\' {
                        k += 1;
                    }
                    if k < bytes.len() && bytes[k] == b'"' {
                        let val = &s[val_start..k];
                        if !val.is_empty()
                            && !val.starts_with("probe_")
                            && val.bytes().all(|b| b.is_ascii_alphanumeric())
                        {
                            result.push_str(json_key);
                            result.push(':');
                            result.push_str(replacement);
                            i = k + 1;
                            continue;
                        }
                    }
                }
            }
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace `"KEY":"DIGITS"` or `"KEY": "DIGITS"` where the value is purely numeric digits.
fn replace_json_digit_value(s: &str, json_key: &str, replacement: &str) -> String {
    let key_bytes = json_key.as_bytes();
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        if i + key_bytes.len() <= bytes.len() && bytes[i..i + key_bytes.len()] == *key_bytes {
            let mut j = i + key_bytes.len();
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                j += 1;
                while j < bytes.len() && bytes[j] == b' ' {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut k = val_start;
                    while k < bytes.len() && bytes[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k > val_start && k < bytes.len() && bytes[k] == b'"' {
                        result.push_str(json_key);
                        result.push(':');
                        result.push_str(replacement);
                        i = k + 1;
                        continue;
                    }
                }
            }
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

/// Replace `PARAM=<digits>` URL parameter values (for paybox-style bodies).
fn replace_url_param_digits(s: &str, param_name: &str, placeholder: &str) -> String {
    let search = format!("{}=", param_name);
    let search_bytes = search.as_bytes();
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < bytes.len() {
        if i + search_bytes.len() <= bytes.len()
            && bytes[i..i + search_bytes.len()] == *search_bytes
        {
            let val_start = i + search_bytes.len();
            if val_start < bytes.len()
                && bytes[val_start].is_ascii_digit()
                && !s[val_start..].starts_with("probe_")
            {
                let mut k = val_start;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > val_start {
                    result.push_str(&search);
                    result.push_str(placeholder);
                    i = k;
                    continue;
                }
            }
        }
        push_char_at(&mut result, s, &mut i);
    }

    result
}

#[test]
fn test_normalize_content() {
    let input =
        r#"{"TransactionIdentifier":"6700473c-d3d1-4b5e-aafd-5b6447fa2ef3","TotalAmount":10.0}"#;
    let output = normalize_content(input);
    assert!(
        output.contains("00000000-0000-0000-0000-000000000000"),
        "Expected UUID to be replaced, got: {}",
        output
    );
    assert!(!output.contains("6700473c"), "Original UUID should be gone");
}

/// Normalize a single HTTP header value, taking the header name into account.
/// Some headers (salt, idempotency-key) always carry random values that can't
/// be detected from the value string alone.
pub(crate) fn normalize_header_value(name: &str, value: String) -> String {
    match name {
        // Always-random/dynamic header values — replace by name rather than by pattern
        "salt" => "probeSaltVal0001".to_string(),
        "idempotency-key" => "HS_probe00000000000000000".to_string(),
        "timestamp" => "0000000000".to_string(),
        _ => normalize_content(&value),
    }
}

pub(crate) fn extract_sample(req: &common_utils::request::Request) -> SamplePayload {
    use hyperswitch_masking::ExposeInterface;
    let method = format!("{:?}", req.method);
    let headers: BTreeMap<String, String> = req
        .get_headers_map()
        .into_iter()
        .map(|(k, v)| {
            let normalized = normalize_header_value(&k, v);
            (k, normalized)
        })
        .collect();
    let body = req.body.as_ref().map(|b| {
        let content = b.get_inner_value().expose();
        normalize_content(&content)
    });

    SamplePayload {
        url: req.url.clone(),
        method,
        headers,
        body,
    }
}
