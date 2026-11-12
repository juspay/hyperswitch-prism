//! Flow result status constants and types.
//!
//! This module centralizes all status values used across field-probe to avoid
//! magic strings and make the codebase more maintainable.

use std::fmt;

/// Represents the result status of probing a connector flow.
///
/// Each variant maps to a specific string value that's stored in the JSON output
/// and used by the documentation generation scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlowStatus {
    /// Flow is fully supported - the connector produced a valid HTTP request.
    Supported,

    /// Flow is not implemented - the connector returns Ok(None) or has an empty
    /// default implementation (e.g., default trait impl with no URL override).
    NotImplemented,

    /// Flow is implemented but this specific payment method is not supported.
    /// The connector explicitly rejects this combination.
    NotSupported,

    /// An error occurred during probing - usually a missing required field
    /// that couldn't be patched automatically.
    #[default]
    Failed,
}

impl FlowStatus {
    /// Returns the string representation used in JSON output.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::NotImplemented => "not_implemented",
            Self::NotSupported => "not_supported",
            Self::Failed => "error",
        }
    }

    /// Returns true if this status represents a successful probe.
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Returns true if this status should be included in the compact output.
    /// NotSupported entries are typically omitted to reduce output size.
    pub const fn should_include_in_compact(&self) -> bool {
        !matches!(self, Self::NotSupported)
    }
}

impl fmt::Display for FlowStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<FlowStatus> for String {
    fn from(status: FlowStatus) -> Self {
        status.as_str().to_string()
    }
}

impl TryFrom<&str> for FlowStatus {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "supported" => Ok(Self::Supported),
            "not_implemented" => Ok(Self::NotImplemented),
            "not_supported" => Ok(Self::NotSupported),
            "error" => Ok(Self::Failed),
            other => Err(format!("Unknown flow status: {}", other)),
        }
    }
}

/// Represents error categories that can be detected from connector error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorCategory {
    /// The connector has not implemented this flow (returns NotImplemented error).
    NotImplemented,

    /// This specific payment method is not supported (returns NotSupported error).
    NotSupported,

    /// A required field is missing - can potentially be patched.
    MissingField,

    /// Configuration error (invalid auth, missing account_id, etc).
    InvalidConfig,

    /// Other uncategorized error.
    Other,
}

impl ErrorCategory {
    /// Returns true if this error category indicates the flow is not available.
    #[allow(dead_code)]
    pub const fn is_flow_unavailable(&self) -> bool {
        matches!(self, Self::NotImplemented | Self::NotSupported)
    }

    /// Returns true if this error can potentially be fixed by field patching.
    #[allow(dead_code)]
    pub const fn is_patchable(&self) -> bool {
        matches!(self, Self::MissingField)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_as_str() {
        assert_eq!(FlowStatus::Supported.as_str(), "supported");
        assert_eq!(FlowStatus::NotImplemented.as_str(), "not_implemented");
        assert_eq!(FlowStatus::NotSupported.as_str(), "not_supported");
        assert_eq!(FlowStatus::Failed.as_str(), "error");
    }

    #[test]
    fn test_status_try_from() {
        assert_eq!(
            FlowStatus::try_from("supported").unwrap(),
            FlowStatus::Supported
        );
        assert_eq!(
            FlowStatus::try_from("not_implemented").unwrap(),
            FlowStatus::NotImplemented
        );
        assert_eq!(
            FlowStatus::try_from("not_supported").unwrap(),
            FlowStatus::NotSupported
        );
        assert_eq!(FlowStatus::try_from("error").unwrap(), FlowStatus::Failed);
        assert!(FlowStatus::try_from("unknown").is_err());
    }

    #[test]
    fn test_status_is_success() {
        assert!(FlowStatus::Supported.is_success());
        assert!(!FlowStatus::NotImplemented.is_success());
        assert!(!FlowStatus::NotSupported.is_success());
        assert!(!FlowStatus::Failed.is_success());
    }
}
