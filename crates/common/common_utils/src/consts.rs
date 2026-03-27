//! Consolidated constants for the connector service

// =============================================================================
// ID Generation and Length Constants
// =============================================================================

use serde::{de::IntoDeserializer, Deserialize, Serialize};

pub const ID_LENGTH: usize = 20;

/// Characters to use for generating NanoID
pub(crate) const ALPHABETS: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z',
];

/// Max Length for MerchantReferenceId
pub const MAX_ALLOWED_MERCHANT_REFERENCE_ID_LENGTH: u8 = 64;
/// Minimum allowed length for MerchantReferenceId
pub const MIN_REQUIRED_MERCHANT_REFERENCE_ID_LENGTH: u8 = 1;
/// Length of a cell identifier in a distributed system
pub const CELL_IDENTIFIER_LENGTH: u8 = 5;
/// Minimum length required for a global id
pub const MAX_GLOBAL_ID_LENGTH: u8 = 64;
/// Maximum length allowed for a global id
pub const MIN_GLOBAL_ID_LENGTH: u8 = 32;

// =============================================================================
// HTTP Headers
// =============================================================================

/// Header key for tenant identification
pub const X_TENANT_ID: &str = "x-tenant-id";
/// Header key for request ID
pub const X_REQUEST_ID: &str = "x-request-id";
/// Header key for connector identification
pub const X_CONNECTOR_NAME: &str = "x-connector";
/// Header key for merchant identification
pub const X_MERCHANT_ID: &str = "x-merchant-id";
/// Header key for reference identification
pub const X_REFERENCE_ID: &str = "x-reference-id";
/// Header key for resource identification
pub const X_RESOURCE_ID: &str = "x-resource-id";

pub const X_SOURCE_NAME: &str = "x-source";

pub const X_CONNECTOR_SERVICE: &str = "connector-service";

pub const X_FLOW_NAME: &str = "x-flow";
/// Header key for shadow mode
pub const X_SHADOW_MODE: &str = "x-shadow-mode";
/// Header key for environment (superposition dimension)
pub const X_ENVIRONMENT: &str = "x-environment";

// =============================================================================
// Base64 engine
// =============================================================================

/// General purpose base64 engine
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// General purpose base64 engine standard nopad
pub const BASE64_ENGINE_STD_NO_PAD: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::STANDARD_NO_PAD;

/// URL Safe base64 engine
pub const BASE64_ENGINE_URL_SAFE: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE;

/// URL Safe base64 engine without padding
pub const BASE64_ENGINE_URL_SAFE_NO_PAD: base64::engine::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

// =============================================================================
// Test Environment Headers
// =============================================================================

/// Header key for session ID (test mode)
pub const X_SESSION_ID: &str = "x-session-id";
/// Header key for API URL (test mode)
pub const X_API_URL: &str = "x-api-url";
/// Header key for API tag (test mode)
pub const X_API_TAG: &str = "x-api-tag";

// =============================================================================
// Authentication Headers (Internal)
// =============================================================================

/// Authentication header
pub const X_AUTH: &str = "x-auth";
/// API key header for authentication
pub const X_API_KEY: &str = "x-api-key";
/// API key header variant
pub const X_KEY1: &str = "x-key1";
/// API key header variant
pub const X_KEY2: &str = "x-key2";
/// API secret header
pub const X_API_SECRET: &str = "x-api-secret";
/// Auth Key Map header
pub const X_AUTH_KEY_MAP: &str = "x-auth-key-map";
/// Typed connector config header (JSON-serialized ConnectorSpecificConfig proto)
pub const X_CONNECTOR_CONFIG: &str = "x-connector-config";
/// Header key for external vault metadata
pub const X_EXTERNAL_VAULT_METADATA: &str = "x-external-vault-metadata";

/// Header key for lineage metadata fields
pub const X_LINEAGE_IDS: &str = "x-lineage-ids";

/// Prefix for lineage fields in additional_fields
pub const LINEAGE_FIELD_PREFIX: &str = "lineage_";

// =============================================================================
// Error Messages and Codes
// =============================================================================

/// No error message string const
pub const NO_ERROR_MESSAGE: &str = "No error message";
/// No error code string const
pub const NO_ERROR_CODE: &str = "No error code";
/// A string constant representing a redacted or masked value
pub const REDACTED: &str = "Redacted";
/// Unsupported response type error message
pub const UNSUPPORTED_ERROR_MESSAGE: &str = "Unsupported response type";
/// Error message when Refund request has been voided
pub const REFUND_VOIDED: &str = "Refund request has been voided.";

// =============================================================================
// Card Validation Constants
// =============================================================================

/// Minimum limit of a card number will not be less than 8 by ISO standards
pub const MIN_CARD_NUMBER_LENGTH: usize = 8;
/// Maximum limit of a card number will not exceed 19 by ISO standards
pub const MAX_CARD_NUMBER_LENGTH: usize = 19;

// =============================================================================
// Log Field Names
// =============================================================================

/// Log field for message content
pub const LOG_MESSAGE: &str = "message";
/// Log field for hostname
pub const LOG_HOSTNAME: &str = "hostname";
/// Log field for process ID
pub const LOG_PID: &str = "pid";
/// Log field for log level
pub const LOG_LEVEL: &str = "level";
/// Log field for target
pub const LOG_TARGET: &str = "target";
/// Log field for service name
pub const LOG_SERVICE: &str = "service";
/// Log field for line number
pub const LOG_LINE: &str = "line";
/// Log field for file name
pub const LOG_FILE: &str = "file";
/// Log field for function name
pub const LOG_FN: &str = "fn";
/// Log field for full name
pub const LOG_FULL_NAME: &str = "full_name";
/// Log field for timestamp
pub const LOG_TIME: &str = "time";

/// Constant variable for name
pub const NAME: &str = "UCS";
/// Constant variable for payment service name
pub const PAYMENT_SERVICE_NAME: &str = "payment_service";

pub const CONST_DEVELOPMENT: &str = "development";
pub const CONST_PRODUCTION: &str = "production";

// =============================================================================
// Superposition Dimensions
// =============================================================================

/// Dimension key for connector in superposition config
pub const DIMENSION_CONNECTOR: &str = "connector";
/// Dimension key for environment in superposition config
pub const DIMENSION_ENVIRONMENT: &str = "environment";

// =============================================================================
// Superposition Config Keys
// =============================================================================

/// Config key for connector base URL
pub const CONFIG_KEY_CONNECTOR_BASE_URL: &str = "connector_base_url";
/// Config key for connector dispute base URL
pub const CONFIG_KEY_CONNECTOR_DISPUTE_BASE_URL: &str = "connector_dispute_base_url";
/// Config key for connector secondary base URL
pub const CONFIG_KEY_CONNECTOR_SECONDARY_BASE_URL: &str = "connector_secondary_base_url";
/// Config key for connector third base URL
pub const CONFIG_KEY_CONNECTOR_THIRD_BASE_URL: &str = "connector_third_base_url";
/// Config key for connector bank redirects base URL
pub const CONFIG_KEY_CONNECTOR_BASE_URL_BANK_REDIRECTS: &str = "connector_base_url_bank_redirects";

// =============================================================================
// Environment and Configuration
// =============================================================================

pub const ENV_PREFIX: &str = "CS";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, config_patch_derive::Patch)]
#[serde(rename_all = "snake_case")]
pub enum Env {
    Development,
    Production,
    Sandbox,
}

impl Env {
    /// Returns the current environment based on the `CS__COMMON__ENVIRONMENT` environment variable.
    ///
    /// If the environment variable is not set, it defaults to `Development` in debug builds
    /// and `Production` in release builds.
    ///
    /// # Panics
    ///
    /// Panics if the `CS__COMMON__ENVIRONMENT` environment variable contains an invalid value
    /// that cannot be deserialized into one of the valid environment variants.
    #[allow(clippy::panic)]
    pub fn current_env() -> Self {
        let env_key = format!("{ENV_PREFIX}__COMMON__ENVIRONMENT");
        std::env::var(&env_key).map_or_else(
            |_| Self::Development,
            |v| {
                Self::deserialize(v.into_deserializer()).unwrap_or_else(|err: serde_json::Error| {
                    panic!("Invalid value found in environment variable {env_key}: {err}")
                })
            },
        )
    }

    pub const fn config_path(self) -> &'static str {
        match self {
            Self::Development => "development.toml",
            Self::Production => "production.toml",
            Self::Sandbox => "sandbox.toml",
        }
    }
}

impl std::fmt::Display for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Production => write!(f, "production"),
            Self::Sandbox => write!(f, "sandbox"),
        }
    }
}
