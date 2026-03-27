//! Superposition configuration wrapper for connector-service
//!
//! This module provides a thin wrapper around `superposition_core::Config`
//! for loading and resolving configuration based on dimensions (connector, environment).

use serde_json::{Map, Value};
use std::collections::HashMap;
use superposition_core::{eval_config, ConfigFormat, MergeStrategy, TomlFormat};
use superposition_types::DetailedConfig;

use crate::consts::{
    CONFIG_KEY_CONNECTOR_BASE_URL, CONFIG_KEY_CONNECTOR_BASE_URL_BANK_REDIRECTS,
    CONFIG_KEY_CONNECTOR_DISPUTE_BASE_URL, CONFIG_KEY_CONNECTOR_SECONDARY_BASE_URL,
    CONFIG_KEY_CONNECTOR_THIRD_BASE_URL, DIMENSION_CONNECTOR, DIMENSION_ENVIRONMENT,
};

/// Error type for superposition configuration operations
#[derive(Debug, thiserror::Error)]
pub enum SuperpositionConfigError {
    /// Failed to read the configuration file
    #[error("Failed to read superposition config file '{path}': {source}")]
    FileReadError {
        path: String,
        source: std::io::Error,
    },
    /// Failed to parse the TOML configuration
    #[error("Failed to parse superposition.toml: {0}")]
    ParseError(String),
    /// Failed to resolve configuration for given context
    #[error("Failed to resolve configuration: {0}")]
    ResolutionError(String),
}

/// Parsed and cached representation of superposition.toml
#[derive(Debug, Clone)]
pub struct SuperpositionConfig {
    config: DetailedConfig,
}

impl SuperpositionConfig {
    /// Load and parse superposition.toml from the given path.
    ///
    /// # Arguments
    /// * `path` - Path to the superposition.toml file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    /// ```ignore
    /// let config = SuperpositionConfig::from_file("config/superposition.toml")?;
    /// ```
    pub fn from_file(path: &str) -> Result<Self, SuperpositionConfigError> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| SuperpositionConfigError::FileReadError {
                path: path.to_string(),
                source: e,
            })?;

        let config = TomlFormat::parse_into_detailed(&contents)
            .map_err(|e| SuperpositionConfigError::ParseError(e.to_string()))?;

        Ok(Self { config })
    }

    /// Resolve the flat key-value map for given dimensions.
    ///
    /// # Arguments
    /// * `connector` - The connector name (e.g., "stripe", "adyen")
    /// * `environment` - The environment name (e.g., "production", "sandbox", "development")
    ///
    /// # Returns
    /// A HashMap of configuration keys to their resolved values.
    ///
    /// # Example
    /// ```ignore
    /// let resolved = config.resolve("stripe", "production")?;
    /// let base_url = resolved.get("connector_base_url").and_then(|v| v.as_str());
    /// ```
    pub fn resolve(
        &self,
        connector: &str,
        environment: &str,
    ) -> Result<HashMap<String, Value>, SuperpositionConfigError> {
        let mut dims: Map<String, Value> = Map::new();
        dims.insert(
            DIMENSION_CONNECTOR.to_string(),
            Value::String(connector.to_string()),
        );
        dims.insert(
            DIMENSION_ENVIRONMENT.to_string(),
            Value::String(environment.to_string()),
        );

        // Convert DefaultConfigsWithSchema to Map<String, Value> by extracting the value field
        let default_configs: Map<String, Value> = self
            .config
            .default_configs
            .clone()
            .into_inner()
            .into_iter()
            .map(|(k, v)| (k, v.value))
            .collect();

        eval_config(
            default_configs,
            &self.config.contexts,
            &self.config.overrides,
            &self.config.dimensions,
            &dims,
            MergeStrategy::MERGE,
            None,
        )
        .map(|m| m.into_iter().collect())
        .map_err(SuperpositionConfigError::ResolutionError)
    }
}

/// Helper function to extract a string value from the resolved configuration.
///
/// Returns `Some(String)` if the key exists and the value is a string, `None` otherwise.
pub fn get_string(resolved: &HashMap<String, Value>, key: &str) -> Option<String> {
    resolved
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Helper function to extract an optional non-empty string from the resolved configuration.
///
/// Returns `Some(String)` if the key exists, is a string, and is non-empty; `None` otherwise.
pub fn get_optional_nonempty_string(
    resolved: &HashMap<String, Value>,
    key: &str,
) -> Option<String> {
    get_string(resolved, key).filter(|s| !s.is_empty())
}

/// Container for resolved connector URLs from superposition configuration
#[derive(Debug, Clone, Default)]
pub struct ConnectorUrls {
    /// Primary base URL for the connector
    pub base_url: Option<String>,
    /// Base URL for dispute operations
    pub dispute_base_url: Option<String>,
    /// Secondary base URL (used by some connectors)
    pub secondary_base_url: Option<String>,
    /// Third base URL (used by some connectors like HiPay)
    pub third_base_url: Option<String>,
    /// Base URL for bank redirect operations (used by TrustPay)
    pub base_url_bank_redirects: Option<String>,
}

/// Extract connector URLs from resolved superposition configuration
///
/// # Arguments
/// * `resolved` - The resolved configuration HashMap from `SuperpositionConfig::resolve()`
///
/// # Returns
/// A `ConnectorUrls` struct containing all resolved URL fields
///
/// # Example
/// ```ignore
/// let resolved = config.resolve("stripe", Some("production"))?;
/// let urls = get_connector_urls(&resolved);
/// ```
pub fn get_connector_urls(resolved: &HashMap<String, Value>) -> ConnectorUrls {
    ConnectorUrls {
        base_url: get_optional_nonempty_string(resolved, CONFIG_KEY_CONNECTOR_BASE_URL),
        dispute_base_url: get_optional_nonempty_string(
            resolved,
            CONFIG_KEY_CONNECTOR_DISPUTE_BASE_URL,
        ),
        secondary_base_url: get_optional_nonempty_string(
            resolved,
            CONFIG_KEY_CONNECTOR_SECONDARY_BASE_URL,
        ),
        third_base_url: get_optional_nonempty_string(resolved, CONFIG_KEY_CONNECTOR_THIRD_BASE_URL),
        base_url_bank_redirects: get_optional_nonempty_string(
            resolved,
            CONFIG_KEY_CONNECTOR_BASE_URL_BANK_REDIRECTS,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_string_returns_none_for_missing_key() {
        let resolved = HashMap::new();
        assert_eq!(get_string(&resolved, "missing_key"), None);
    }

    #[test]
    fn test_get_string_returns_some_for_value() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "connector_base_url".to_string(),
            Value::String("https://api.example.com/".to_string()),
        );
        assert_eq!(
            get_string(&resolved, "connector_base_url"),
            Some("https://api.example.com/".to_string())
        );
    }

    #[test]
    fn test_get_optional_nonempty_string_returns_none_for_empty() {
        let mut resolved = HashMap::new();
        resolved.insert("key".to_string(), Value::String("".to_string()));
        assert_eq!(get_optional_nonempty_string(&resolved, "key"), None);
    }

    #[test]
    fn test_get_optional_nonempty_string_returns_some_for_value() {
        let mut resolved = HashMap::new();
        resolved.insert("key".to_string(), Value::String("value".to_string()));
        assert_eq!(
            get_optional_nonempty_string(&resolved, "key"),
            Some("value".to_string())
        );
    }
}
