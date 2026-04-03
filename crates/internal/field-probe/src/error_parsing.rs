//! Error message parsing and classification.
//!
//! This module provides functions to parse error messages from connector transformers
//! and classify them into different categories. The classification determines how
//! the probe engine responds to each error.
//!
//! # Error Categories
//!
//! 1. **NotImplemented** - The connector hasn't implemented this flow yet. The probe
//!    should stop and mark it as not_implemented.
//!
//! 2. **NotSupported** - This specific payment method/flow combination is not
//!    supported by design. The probe should stop and mark it as not_supported.
//!
//! 3. **MissingField** - A required field is missing. The probe should attempt to
//!    patch the request and retry.
//!
//! 4. **InvalidConfig** - Configuration error (missing account_id, invalid auth, etc).
//!    The probe should mark as error.
//!
//! # Examples
//!
//! ```rust,ignore
//! use error_parsing::{classify_error, parse_missing_field};
//!
//! let msg = "Missing required field: billing_address";
//! if let Some(field) = parse_missing_field(msg) {
//!     println!("Need to patch field: {}", field);
//! }
//!
//! let category = classify_error(msg);
//! assert!(category.is_patchable());
//! ```

use crate::status::ErrorCategory;

/// Pattern matchers for detecting "not implemented" errors.
/// These indicate the connector code exists but the flow isn't fully implemented yet.
const NOT_IMPLEMENTED_PATTERNS: &[&str] =
    &["not been implemented", "notimplemented", "not implemented"];

/// Pattern matchers for detecting "not supported" errors.
/// These indicate the connector explicitly doesn't support this PM/flow combination.
const NOT_SUPPORTED_PATTERNS: &[&str] = &[
    "not supported",
    "unsupported",
    "not configured with the given connector",
    "only card payment",
    "only interac",
    "only upi",
    "payment method not supported",
    "does not support this payment",
    "notsupported",
    "flownotsupported",
    "wallet payment method is not supported",
    "paylater payment method is not supported",
    "payment method not supported", // e.g. "Payment method not supported"
];

/// Pattern matchers for detecting missing field errors (plural form).
const MISSING_FIELDS_PATTERNS: &[&str] = &["Missing required fields: ["];

/// Pattern matchers for detecting missing field errors (singular form).
const MISSING_FIELD_PATTERNS: &[&str] = &[
    "Missing required param: ",
    "Missing required field: ",
    "MissingRequiredField { field_name: \"",
    "field_name: \"",
];

/// Alternative field patterns for non-standard error messages.
/// Each entry maps a field name to patterns that must all match.
const ALT_FIELD_PATTERNS: &[(&str, &[&str])] = &[
    ("payment_method_token", &["Failed to parse", "wallet token"]),
    ("metadata", &["Invalid Configuration", "metadata"]),
];

/// Classifies an error message into a category.
///
/// This is the main entry point for error classification. It checks the message
/// against all known patterns and returns the appropriate category.
///
/// # Arguments
/// * `msg` - The error message from the connector
///
/// # Returns
/// The classified error category
#[allow(dead_code)]
pub fn classify_error(msg: &str) -> ErrorCategory {
    if is_not_implemented(msg) {
        ErrorCategory::NotImplemented
    } else if is_not_supported(msg) {
        ErrorCategory::NotSupported
    } else if parse_missing_field(msg).is_some() {
        ErrorCategory::MissingField
    } else if msg.contains("InvalidConnectorConfig")
        || msg.contains("Invalid Configuration")
        || msg.contains("account_id")
    {
        ErrorCategory::InvalidConfig
    } else {
        ErrorCategory::Other
    }
}

/// Returns true when the connector explicitly says this flow/PM has not been
/// implemented yet (development work still pending).
///
/// These errors are recorded as `not_implemented` and rendered as ⚠ in the docs.
///
/// # Examples
/// * "Payment method not been implemented"
/// * "NotImplemented: Only card payments are supported"
/// * "This flow is not implemented"
pub fn is_not_implemented(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    NOT_IMPLEMENTED_PATTERNS
        .iter()
        .any(|&pattern| lower.contains(pattern))
}

/// Returns true when the connector definitively does not support this payment
/// method / flow combination (by design, not a missing implementation).
///
/// These errors are recorded as `not_supported` and rendered as `x` in the docs.
///
/// # Examples
/// * "Payment method not supported"
/// * "Only card payments are supported"
/// * "Selected payment method through fiserv"
pub fn is_not_supported(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    NOT_SUPPORTED_PATTERNS
        .iter()
        .any(|&pattern| lower.contains(pattern))
}

/// Parses a "missing field" error message and extracts the field name.
///
/// Handles both singular and plural forms:
/// - Singular: "Missing required field: billing_address"
/// - Plural: "Missing required fields: ["billing_address.city", "country"]"
///
/// For plural forms, only the first field is returned - the probe will iterate
/// and patch remaining fields in subsequent attempts.
///
/// # Arguments
/// * `msg` - The error message
///
/// # Returns
/// Some(field_name) if a missing field was detected, None otherwise
pub fn parse_missing_field(msg: &str) -> Option<String> {
    // Try plural form first
    if let Some(field) = parse_plural_missing_fields(msg) {
        return Some(field);
    }

    // Try singular forms
    for needle in MISSING_FIELD_PATTERNS {
        if let Some(field) = extract_field_after_pattern(msg, needle) {
            return Some(field);
        }
    }

    None
}

/// Parses the plural form of missing fields error.
///
/// Example input: `Missing required fields: ["billing_address.city", "country"]`
/// Returns: "billing_address.city" (first field only)
fn parse_plural_missing_fields(msg: &str) -> Option<String> {
    for pattern in MISSING_FIELDS_PATTERNS {
        if let Some(pos) = msg.find(pattern) {
            let rest = &msg[pos + pattern.len()..];
            // Names are double-quoted inside the list
            if let Some(first) = rest.split('"').nth(1) {
                if !first.is_empty() {
                    return Some(first.to_string());
                }
            }
        }
    }
    None
}

/// Extracts a field name that appears after a specific pattern.
///
/// The field name is extracted up to the first " (", newline, quote, or closing brace.
fn extract_field_after_pattern(msg: &str, pattern: &str) -> Option<String> {
    if let Some(pos) = msg.find(pattern) {
        let rest = &msg[pos + pattern.len()..];
        // Field name ends at " (" (parenthetical note), newline, quote, or closing brace
        let field = rest
            .split(" (")
            .next()
            .unwrap_or(rest)
            .split('"')
            .next()
            .unwrap_or(rest)
            .split("}")
            .next()
            .unwrap_or(rest)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('"')
            .trim_end_matches('}')
            .to_string();
        if !field.is_empty() {
            return Some(field);
        }
    }
    None
}

/// Alternative parser for non-standard error messages.
///
/// Some connectors use error formats that don't match the standard
/// "Missing required field" pattern. This function handles those cases.
///
/// # Examples
/// * "Failed to parse Apple Pay wallet token" → "payment_method_token"
pub fn parse_missing_field_alt(msg: &str) -> Option<String> {
    for (field_name, patterns) in ALT_FIELD_PATTERNS {
        if patterns.iter().all(|&p| msg.contains(p)) {
            return Some((*field_name).to_string());
        }
    }
    None
}

/// Returns true when this connector requires an OAuth access token.
///
/// OAuth connectors need a prior ServerAuthenticationToken step before they can
/// make payment requests. The probe handles this by providing mock state.
pub fn is_oauth_connector(connector: &domain_types::connector_types::ConnectorEnum) -> bool {
    let config = crate::config::get_config();
    let name = format!("{connector:?}").to_lowercase();
    config.oauth_connectors.iter().any(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_missing_field_singular() {
        assert_eq!(
            parse_missing_field("Missing required field: billing_address"),
            Some("billing_address".to_string())
        );
        assert_eq!(
            parse_missing_field("Missing required param: amount"),
            Some("amount".to_string())
        );
    }

    #[test]
    fn test_parse_missing_field_plural() {
        assert_eq!(
            parse_missing_field(r#"Missing required fields: ["billing_address.city", "country"]"#),
            Some("billing_address.city".to_string())
        );
    }

    #[test]
    fn test_is_not_implemented() {
        assert!(is_not_implemented("This flow is not implemented"));
        assert!(is_not_implemented("NotImplemented error"));
        assert!(!is_not_implemented("This field is required"));
    }

    #[test]
    fn test_is_not_supported() {
        assert!(is_not_supported("Payment method not supported"));
        assert!(is_not_supported("Only card payments are supported"));
        assert!(!is_not_supported("Missing required field"));
    }

    #[test]
    fn test_classify_error() {
        assert_eq!(
            classify_error("This flow is not implemented"),
            ErrorCategory::NotImplemented
        );
        assert_eq!(
            classify_error("Payment method not supported"),
            ErrorCategory::NotSupported
        );
        assert_eq!(
            classify_error("Missing required field: amount"),
            ErrorCategory::MissingField
        );
    }
}
