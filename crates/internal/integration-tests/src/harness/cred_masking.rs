//! Shared credential masking and detection utilities.
//!
//! Used by:
//! - `report.rs` to sanitize entries at generation time
//! - `mask_report_creds` binary to retroactively mask existing MD files
//! - `check_report_creds` binary to verify no unmasked creds remain

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub const MASKED_VALUE: &str = "***MASKED***";

// ---------------------------------------------------------------------------
// JSON-level masking (for structured req_body / res_body)
// ---------------------------------------------------------------------------

/// Returns `true` if `key` (after normalisation) looks like a sensitive field.
pub fn is_sensitive_key(key: &str) -> bool {
    let key = normalize_key(key);
    key.contains("secret")
        || key.contains("token")
        || key.contains("password")
        || key.contains("authorization")
        || key == "apikey"
        || key == "xapikey"
        || key == "xconnectorconfig"
        || key == "idempotencykey"
        || key == "setcookie"
        || key == "xauth"
        || key == "key1"
        || key == "xkey1"
        || key == "key2"
        || key == "xkey2"
        || key == "key3"
        || key == "xkey3"
        || key == "key0"
        || key == "xkey0"
        || key.contains("signature")
        || key.contains("cardnumber")
        || key.contains("cvv")
        || key.contains("cvc")
        || key == "expmonth"
        || key == "expyear"
        || key == "rawconnectorrequest"
        || key == "rawconnectorresponse"
        || key == "accesscode"
        || key == "entityid"
        || key == "pin"
}

/// Recursively mask sensitive keys inside a JSON `Value` in-place.
pub fn mask_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *child = Value::String(MASKED_VALUE.to_string());
                } else {
                    mask_json_value(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                mask_json_value(item);
            }
        }
        Value::String(text) => {
            *text = mask_sensitive_text(text);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Free-text masking (for grpc_request, grpc_response, error strings)
// ---------------------------------------------------------------------------

/// Apply all text-level masking passes to a multi-line string.
pub fn mask_sensitive_text(text: &str) -> String {
    // First pass: mask rawConnectorRequest / rawConnectorResponse objects.
    // This MUST run before line-by-line passes because Bearer-token masking
    // can corrupt escaped quotes inside the value strings, making the regex
    // unable to match the structural `{ "value": "..." }` pattern.
    let text = mask_raw_connector_fields(text);

    let mut masked_lines = Vec::new();
    for line in text.lines() {
        let line = mask_connector_config_header(line);
        let line = mask_sensitive_header_line(&line);
        masked_lines.push(mask_bearer_and_jwt_tokens(&line));
    }
    masked_lines.join("\n")
}

// ---------------------------------------------------------------------------
// rawConnectorRequest / rawConnectorResponse masking (multi-line)
// ---------------------------------------------------------------------------

/// Regex that matches a JSON key `"rawConnectorRequest"` or
/// `"rawConnectorResponse"` followed by a JSON object of the form
/// `{ "value": "..." }` (potentially spanning multiple lines).
///
/// The value string may contain escaped quotes (`\"`), so we use
/// `(?:[^"\\]|\\.)*` to correctly skip them and only stop at the
/// unescaped closing quote of the value string.
static RAW_CONNECTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)("(?:rawConnector(?:Request|Response)|raw_connector_(?:request|response))")\s*:\s*\{\s*"value"\s*:\s*"(?:[^"\\]|\\.)*"\s*\}"#,
    )
    .expect("rawConnector regex must compile")
});

/// Regex for direct string form:
///   "raw_connector_response": "..."
/// or
///   "rawConnectorResponse": "..."
static RAW_CONNECTOR_STRING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)("(?:rawConnector(?:Request|Response)|raw_connector_(?:request|response))")\s*:\s*"(?:[^"\\]|\\.)*""#,
    )
    .expect("rawConnector string regex must compile")
});

/// Cleanup regex for previously-corrupted masking output.
///
/// When the first (buggy) regex matched too little, it left behind orphaned
/// value content like:
///   `"rawConnectorRequest": "***MASKED***",\"body\":{...}}`
///
/// This regex matches the `"***MASKED***"` followed by leftover content up to
/// the next newline that starts a new JSON key or the end of the enclosing
/// object.
static RAW_CONNECTOR_CLEANUP_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Match: "rawConnectorX": "***MASKED***" followed by non-newline junk
    // The junk is everything after MASKED_VALUE's closing quote until end-of-line.
    Regex::new(
        r#"("(?:rawConnector(?:Request|Response)|raw_connector_(?:request|response))":\s*"\*\*\*MASKED\*\*\*"\s*,?)[^\n]*"#,
    )
        .expect("rawConnector cleanup regex must compile")
});

static RAW_CONNECTOR_OBJECT_START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(\s*)"(rawConnector(?:Request|Response)|raw_connector_(?:request|response))"\s*:\s*\{\s*$"#,
    )
    .expect("rawConnector object-start regex must compile")
});

static RAW_CONNECTOR_STRING_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(\s*)"(rawConnector(?:Request|Response)|raw_connector_(?:request|response))"\s*:\s*".*$"#,
    )
    .expect("rawConnector string-line regex must compile")
});

/// Replace `"rawConnectorRequest": { "value": "..." }` and
/// `"rawConnectorResponse": { "value": "..." }` with
/// `"rawConnectorRequest": "***MASKED***"` in free-text (grpc_response, MD
/// files, etc.).
fn mask_raw_connector_fields(text: &str) -> String {
    // Pass 1: Match the proper { "value": "..." } structure
    let result = RAW_CONNECTOR_RE.replace_all(text, |caps: &regex::Captures<'_>| {
        format!("{}: \"{}\"", &caps[1], MASKED_VALUE)
    });

    // Pass 2: Match direct string values under raw connector keys.
    let result = RAW_CONNECTOR_STRING_RE.replace_all(&result, |caps: &regex::Captures<'_>| {
        format!("{}: \"{}\"", &caps[1], MASKED_VALUE)
    });

    // Pass 3: Line-based fallback for malformed rawConnector objects from
    // older masking runs (for example, escaped-quote corruption around Bearer).
    let result = mask_raw_connector_blocks_fallback(&result);

    // Pass 4: Clean up any leftover junk from a previous buggy masking run.
    let result = RAW_CONNECTOR_CLEANUP_RE
        .replace_all(&result, |caps: &regex::Captures<'_>| caps[1].to_string());

    result.into_owned()
}

fn mask_raw_connector_blocks_fallback(text: &str) -> String {
    let mut output_lines = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        if let Some(caps) = RAW_CONNECTOR_OBJECT_START_RE.captures(line) {
            let indent = caps.get(1).map_or("", |m| m.as_str());
            let key = caps.get(2).map_or("rawConnectorResponse", |m| m.as_str());

            let mut trailing_comma = false;
            for next_line in lines.by_ref() {
                let trimmed = next_line.trim();
                if trimmed == "}" || trimmed == "}," {
                    trailing_comma = trimmed.ends_with(',');
                    break;
                }
            }

            if !trailing_comma {
                if let Some(next_line) = lines.peek() {
                    let trimmed = next_line.trim_start();
                    if trimmed.starts_with('"') {
                        trailing_comma = true;
                    }
                }
            }

            let comma = if trailing_comma { "," } else { "" };
            output_lines.push(format!(r#"{indent}"{key}": "{MASKED_VALUE}"{comma}"#));
            continue;
        }

        if let Some(caps) = RAW_CONNECTOR_STRING_LINE_RE.captures(line) {
            let indent = caps.get(1).map_or("", |m| m.as_str());
            let key = caps.get(2).map_or("rawConnectorResponse", |m| m.as_str());
            let trailing_comma = line.trim_end().ends_with(',');
            let comma = if trailing_comma { "," } else { "" };
            output_lines.push(format!(r#"{indent}"{key}": "{MASKED_VALUE}"{comma}"#));
            continue;
        }

        output_lines.push(line.to_string());
    }

    output_lines.join("\n")
}

// ---------------------------------------------------------------------------
// `x-connector-config` header masking
// ---------------------------------------------------------------------------

/// Masks the entire JSON value after `x-connector-config:`.
///
/// The header looks like:
///   -H "x-connector-config: {"config":{"Stripe":{"api_key":"<connector-secret>"}}}" \
///
/// We replace everything after the colon with `***MASKED***`, preserving any
/// trailing `"` or ` \`.
fn mask_connector_config_header(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let Some(idx) = lower.find("x-connector-config") else {
        return line.to_string();
    };

    // Find the colon after "x-connector-config"
    let after_key = idx + "x-connector-config".len();
    let Some(colon_rel) = line[after_key..].find(':') else {
        return line.to_string();
    };
    let colon_idx = after_key + colon_rel;

    let prefix = &line[..=colon_idx];
    let mut masked = format!("{} {}", prefix, MASKED_VALUE);

    // Preserve trailing `"` or ` \` from the original line
    let trailer = line[colon_idx + 1..].trim_end();
    if trailer.ends_with("\" \\") || trailer.ends_with("\"\\") {
        masked.push_str("\" \\");
    } else if trailer.ends_with('"') {
        masked.push('"');
    } else if trailer.ends_with('\\') {
        masked.push_str(" \\");
    }

    masked
}

// ---------------------------------------------------------------------------
// Generic sensitive header-line masking
// ---------------------------------------------------------------------------

/// Masks the value portion of lines that look like `key: value` where `key`
/// is a known sensitive header name (e.g. `x-api-key`, `authorization`).
fn mask_sensitive_header_line(line: &str) -> String {
    let Some(colon_index) = line.find(':') else {
        return line.to_string();
    };

    let key_candidate = line[..colon_index]
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_matches('"')
        .trim_matches('>')
        .trim_matches('<');

    if !is_sensitive_key(key_candidate) {
        return line.to_string();
    }

    // rawConnectorRequest / rawConnectorResponse are multi-line objects handled
    // by `mask_raw_connector_fields()` — skip them in the line-by-line pass.
    let key_norm = normalize_key(key_candidate);
    if key_norm == "rawconnectorrequest" || key_norm == "rawconnectorresponse" {
        return line.to_string();
    }

    let mut masked = format!("{} {}", &line[..=colon_index], MASKED_VALUE);
    if line[colon_index + 1..].contains('"') {
        masked.push('"');
    }
    if line.trim_end().ends_with('\\') {
        masked.push_str(" \\");
    }
    masked
}

// ---------------------------------------------------------------------------
// Bearer / JWT token masking
// ---------------------------------------------------------------------------

/// Masks `Bearer <token>` patterns and standalone JWT tokens (three base64
/// segments separated by dots).
static JWT_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"#)
        .expect("JWT regex must compile")
});

fn mask_bearer_and_jwt_tokens(line: &str) -> String {
    let mut masked = line.to_string();
    let mut search_start = 0usize;

    // Pass 1: Mask Bearer tokens (e.g., "Bearer eyJ...")
    loop {
        if search_start >= masked.len() {
            break;
        }
        let lowercase = masked.to_ascii_lowercase();
        let Some(relative_start) = lowercase[search_start..].find("bearer ") else {
            break;
        };
        let start = search_start + relative_start;
        let token_start = start + "bearer ".len();
        let token_end = masked[token_start..]
            .find(|ch: char| {
                ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ',' || ch == '\\'
            })
            .map(|offset| token_start + offset)
            .unwrap_or(masked.len());

        if &masked[token_start..token_end] != MASKED_VALUE {
            masked.replace_range(token_start..token_end, MASKED_VALUE);
            search_start = token_start + MASKED_VALUE.len();
        } else {
            search_start = token_end;
        }
    }

    // Pass 2: Mask standalone JWT tokens (e.g., "eyJ..." without "Bearer" prefix)
    JWT_TOKEN_RE.replace_all(&masked, MASKED_VALUE).into_owned()
}

// ---------------------------------------------------------------------------
// Plain-text credential detection (for check_report_creds)
// ---------------------------------------------------------------------------

/// Checks whether a single line of text contains an unmasked credential.
///
/// Returns `Some(reason)` if a credential pattern is detected, `None` if clean.
pub fn detect_unmasked_cred(line: &str) -> Option<String> {
    // Already masked — skip
    if line.contains(MASKED_VALUE)
        && !has_real_cred_alongside_mask(line)
        && !JWT_TOKEN_RE.is_match(line)
    {
        return None;
    }

    // 1. x-connector-config with actual JSON (not masked)
    let lower = line.to_ascii_lowercase();
    if lower.contains("x-connector-config") && line.contains('{') && !line.contains(MASKED_VALUE) {
        return Some("x-connector-config header contains unmasked JSON".to_string());
    }

    // 2. Known secret header patterns: `x-api-key: <value>`, etc.
    if let Some(colon_idx) = line.find(':') {
        let key_candidate = line[..colon_idx]
            .split_whitespace()
            .last()
            .unwrap_or_default()
            .trim_matches('"')
            .trim_matches('>')
            .trim_matches('<');

        if is_sensitive_key(key_candidate) {
            let value_part = line[colon_idx + 1..].trim();
            if !value_part.is_empty()
                && value_part != MASKED_VALUE
                && !value_part.starts_with(MASKED_VALUE)
            {
                return Some(format!(
                    "sensitive header '{}' has unmasked value",
                    key_candidate
                ));
            }
        }
    }

    // 3. Bearer token not masked
    if lower.contains("bearer ") {
        let after_bearer = lower
            .find("bearer ")
            .map(|i| &line[i + "bearer ".len()..])
            .unwrap_or("");
        let token_part: String = after_bearer
            .chars()
            .take_while(|ch| !ch.is_whitespace() && *ch != '"' && *ch != '\'')
            .collect();
        if !token_part.is_empty() && token_part != MASKED_VALUE {
            return Some("Bearer token not masked".to_string());
        }
    }

    // 4. JWT token not masked
    if JWT_TOKEN_RE.is_match(line) {
        return Some("JWT token not masked".to_string());
    }

    None
}

/// Edge case: a line might contain `***MASKED***` for one field but still have
/// a real credential elsewhere. This is a quick heuristic.
fn has_real_cred_alongside_mask(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    // If the line has x-connector-config with JSON despite having MASKED_VALUE elsewhere
    if lower.contains("x-connector-config") && line.contains('{') {
        // Check if the JSON part itself is not masked
        if let Some(idx) = lower.find("x-connector-config") {
            let after = &line[idx..];
            if let Some(colon) = after.find(':') {
                let value = after[colon + 1..].trim();
                if value.starts_with('{') {
                    return true;
                }
            }
        }
    }
    false
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_connector_config_header() {
        let line = r#"  -H "x-connector-config: {"config":{"Stripe":{"api_key":"stripe_key_example_abc123"}}}" \"#;
        let masked = mask_connector_config_header(line);
        assert!(
            !masked.contains("stripe_key_example_abc123"),
            "API key should be masked"
        );
        assert!(masked.contains(MASKED_VALUE));
        assert!(masked.contains("x-connector-config:"));
    }

    #[test]
    fn masks_connector_config_with_jwt() {
        let line = r#"  -H "x-connector-config: {"config":{"Stax":{"api_key":"eyJ0eXAi..."}}}" \"#;
        let masked = mask_connector_config_header(line);
        assert!(!masked.contains("eyJ0eXAi"), "JWT should be masked");
        assert!(masked.contains(MASKED_VALUE));
    }

    #[test]
    fn preserves_non_config_headers() {
        let line = r#"  -H "x-merchant-id: test_merchant" \"#;
        let masked = mask_connector_config_header(line);
        assert_eq!(masked, line);
    }

    #[test]
    fn mask_sensitive_text_handles_all_patterns() {
        let text = concat!(
            "grpcurl -plaintext \\\n",
            "  -H \"x-merchant-id: test_merchant\" \\\n",
            "  -H \"x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"stripe_key_example_123\"}}}\" \\\n",
            "  -H \"authorization: Bearer token123\" \\\n",
            "  -d @ localhost:50051 types.PaymentService/Authorize"
        );
        let masked = mask_sensitive_text(text);
        assert!(
            !masked.contains("stripe_key_example_123"),
            "API key should be masked"
        );
        assert!(
            !masked.contains("token123"),
            "Bearer token should be masked"
        );
        assert!(
            masked.contains("test_merchant"),
            "Merchant ID should be preserved"
        );
    }

    #[test]
    fn detect_unmasked_cred_catches_config_header() {
        let line = r#"  -H "x-connector-config: {"config":{"Stripe":{"api_key":"stripe_key_example_abc"}}}" \"#;
        assert!(detect_unmasked_cred(line).is_some());
    }

    #[test]
    fn detect_unmasked_cred_passes_masked_config() {
        let line = r#"  -H "x-connector-config: ***MASKED***" \"#;
        assert!(detect_unmasked_cred(line).is_none());
    }

    #[test]
    fn detect_unmasked_cred_catches_bearer() {
        let line = "Authorization: Bearer real_token_123";
        assert!(detect_unmasked_cred(line).is_some());
    }

    #[test]
    fn detect_unmasked_cred_passes_masked_bearer() {
        let line = "Authorization: ***MASKED***";
        assert!(detect_unmasked_cred(line).is_none());
    }

    #[test]
    fn mask_sensitive_text_masks_standalone_jwt() {
        let line = r#"\"value\": \"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.sgnf0r8V0Y6k1B8sN8Q8Vf9T0n7b9M5ZQ2LxWwA9QeM\""#;
        let masked = mask_sensitive_text(line);
        assert!(!masked.contains("eyJhbGciOiJIUzI1NiJ9"));
        assert!(masked.contains(MASKED_VALUE));
    }

    #[test]
    fn detect_unmasked_cred_catches_jwt_even_with_masked_values_present() {
        let line = r#"\"token\": \"***MASKED***\", \"value\": \"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.sgnf0r8V0Y6k1B8sN8Q8Vf9T0n7b9M5ZQ2LxWwA9QeM\""#;
        assert!(detect_unmasked_cred(line).is_some());
    }

    #[test]
    fn mask_sensitive_header_line_masks_idempotency_key() {
        let line = r#"\"idempotency-key\": \"1c9b3b39-3536-44e9-b914-c1f2784e7d47\""#;
        let masked = mask_sensitive_text(line);
        assert!(!masked.contains("1c9b3b39-3536-44e9-b914-c1f2784e7d47"));
        assert!(masked.contains(MASKED_VALUE));
    }

    #[test]
    fn mask_sensitive_header_line_masks_set_cookie() {
        let line = r#"\"set-cookie\": \"__cf_bm=abc123.456.789; HttpOnly\""#;
        let masked = mask_sensitive_text(line);
        assert!(!masked.contains("__cf_bm=abc123.456.789"));
        assert!(masked.contains(MASKED_VALUE));
    }

    #[test]
    fn masking_is_idempotent() {
        let line = r#"  -H "x-connector-config: {"config":{"Stripe":{"api_key":"stripe_key_example_abc"}}}" \"#;
        let masked_once = mask_sensitive_text(line);
        let masked_twice = mask_sensitive_text(&masked_once);
        assert_eq!(masked_once, masked_twice);
    }

    #[test]
    fn mask_json_value_masks_nested_keys() {
        let mut val = serde_json::json!({
            "config": {
                "Stripe": {
                    "api_key": "stripe_key_example_123",
                    "merchant_account": "acct_123"
                }
            }
        });
        mask_json_value(&mut val);
        // api_key matches "apikey" after normalization
        assert_eq!(
            val.pointer("/config/Stripe/api_key")
                .and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
        // merchant_account should NOT be masked (it's not a secret key pattern)
        assert_ne!(
            val.pointer("/config/Stripe/merchant_account")
                .and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
    }

    #[test]
    fn mask_json_value_masks_x_connector_config_field() {
        let mut val = serde_json::json!({
            "headers": {
                "x-connector-config": "{\"config\":{\"Stripe\":{\"api_key\":\"stripe_key_example_123\"}}}"
            }
        });
        mask_json_value(&mut val);
        assert_eq!(
            val.pointer("/headers/x-connector-config")
                .and_then(Value::as_str),
            Some(MASKED_VALUE)
        );
    }

    // --- rawConnectorRequest / rawConnectorResponse tests ---

    #[test]
    fn mask_json_value_masks_raw_connector_response() {
        let mut val = serde_json::json!({
            "status": "CHARGED",
            "rawConnectorResponse": {
                "value": "{\"id\": \"pi_123\", \"client_secret\": \"secret_456\"}"
            },
            "rawConnectorRequest": {
                "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"body\":\"amount=6000\"}"
            }
        });
        mask_json_value(&mut val);
        assert_eq!(
            val.pointer("/rawConnectorResponse").and_then(Value::as_str),
            Some(MASKED_VALUE),
            "rawConnectorResponse should be replaced with MASKED_VALUE"
        );
        assert_eq!(
            val.pointer("/rawConnectorRequest").and_then(Value::as_str),
            Some(MASKED_VALUE),
            "rawConnectorRequest should be replaced with MASKED_VALUE"
        );
        // status should NOT be masked
        assert_eq!(
            val.pointer("/status").and_then(Value::as_str),
            Some("CHARGED")
        );
    }

    #[test]
    fn mask_raw_connector_fields_multiline() {
        let text = r#"{
  "status": "CHARGED",
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TEA9gD5R7gDAGff1etYFP4Y\",\n  \"amount\": 6000\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\"}"
  }
}"#;
        let masked = mask_raw_connector_fields(text);
        assert!(
            !masked.contains("pi_3TEA9gD5R7gDAGff1etYFP4Y"),
            "connector response data should be masked"
        );
        assert!(
            !masked.contains("api.stripe.com"),
            "connector request URL should be masked"
        );
        assert!(
            masked.contains("\"rawConnectorResponse\": \"***MASKED***\""),
            "rawConnectorResponse should be replaced with masked value, got: {}",
            masked
        );
        assert!(
            masked.contains("\"rawConnectorRequest\": \"***MASKED***\""),
            "rawConnectorRequest should be replaced with masked value, got: {}",
            masked
        );
        assert!(masked.contains("CHARGED"), "non-sensitive fields preserved");
    }

    #[test]
    fn mask_raw_connector_fields_single_line() {
        // Compact JSON (as might appear in res_body strings)
        let text = r#"{"rawConnectorResponse": {"value": "{\"id\":\"pi_123\"}"}, "rawConnectorRequest": {"value": "{\"url\":\"https://stripe.com\"}"}}"#;
        let masked = mask_raw_connector_fields(text);
        assert!(
            !masked.contains("pi_123"),
            "connector response should be masked"
        );
        assert!(
            !masked.contains("stripe.com"),
            "connector request should be masked"
        );
    }

    #[test]
    fn mask_raw_connector_fields_masks_snake_case_direct_string() {
        let text = r#"{"raw_connector_response":"{\"error\":\"bad key\"}"}"#;
        let masked = mask_sensitive_text(text);
        assert!(
            !masked.contains("bad key"),
            "snake_case raw connector response should be masked"
        );
        assert!(
            masked.contains("\"raw_connector_response\": \"***MASKED***\""),
            "snake_case key should be replaced with masked value"
        );
    }

    #[test]
    fn mask_raw_connector_fields_fallback_handles_malformed_block() {
        let text = concat!(
            "{\n",
            "  \"rawConnectorRequest\": {\n",
            "    \"value\": \"{\\\"url\\\":\\\"https://api.example.com\\\",\\\"headers\\\":{\\\"Authorization\\\":\\\"Bearer ***MASKED***\",\\\"via\\\":\\\"HyperSwitch\\\"}}\"\n",
            "  },\n",
            "  \"status\": \"ok\"\n",
            "}\n",
        );
        let masked = mask_sensitive_text(text);
        assert!(
            masked.contains("\"rawConnectorRequest\": \"***MASKED***\","),
            "malformed raw connector block should be replaced, got: {masked}"
        );
        assert!(masked.contains("\"status\": \"ok\""));
    }

    #[test]
    fn mask_sensitive_text_masks_raw_connector() {
        // Full integration test via mask_sensitive_text
        let text = "Some prefix\n  \"rawConnectorResponse\": {\n    \"value\": \"{\\\"id\\\": \\\"pi_secret_123\\\"}\"\n  }";
        let masked = mask_sensitive_text(text);
        assert!(
            !masked.contains("pi_secret_123"),
            "rawConnectorResponse data should be masked via mask_sensitive_text"
        );
    }

    #[test]
    fn mask_raw_connector_fields_idempotent() {
        let text = r#"  "rawConnectorResponse": {
    "value": "some secret data"
  }"#;
        let masked_once = mask_raw_connector_fields(text);
        let masked_twice = mask_raw_connector_fields(&masked_once);
        assert_eq!(masked_once, masked_twice, "masking should be idempotent");
    }
}
