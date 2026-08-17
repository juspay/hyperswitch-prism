//! Superposition configuration wrapper for connector-service
//!
//! This module provides a thin wrapper around Superposition's local provider
//! for loading and resolving configuration based on dimensions (connector, environment).

use std::{fmt, path::PathBuf};

use serde_json::{Map, Value};
use superposition_provider::{
    data_source::file::FileDataSource, traits::AllFeatureProvider, EvaluationContext,
    LocalResolutionProvider, RefreshStrategy, WatchStrategy,
};

use crate::consts::{
    CONFIG_KEY_CONNECTOR_BASE_URL, CONFIG_KEY_CONNECTOR_BASE_URL_BANK_REDIRECTS,
    CONFIG_KEY_CONNECTOR_DISPUTE_BASE_URL, CONFIG_KEY_CONNECTOR_SECONDARY_BASE_URL,
    CONFIG_KEY_CONNECTOR_THIRD_BASE_URL, DIMENSION_CONNECTOR, DIMENSION_ENVIRONMENT,
};

/// Error type for superposition configuration operations
#[derive(Debug, thiserror::Error)]
pub enum SuperpositionConfigError {
    #[error("Failed to initialize superposition local provider: {0}")]
    InitializationError(String),
    #[error("Failed to resolve superposition configuration: {0}")]
    ResolutionError(String),
}

/// Local provider backed by superposition.toml.
#[derive(Clone)]
pub struct SuperpositionConfig {
    provider: LocalResolutionProvider,
}

impl fmt::Debug for SuperpositionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SuperpositionConfig")
    }
}

impl SuperpositionConfig {
    /// Load superposition.toml and watch it for changes.
    ///
    /// # Arguments
    /// * `path` - Path to the superposition.toml file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    ///
    /// # Example
    /// ```ignore
    /// let config = SuperpositionConfig::from_file("config/superposition.toml").await?;
    /// ```
    pub async fn from_file(path: &str) -> Result<Self, SuperpositionConfigError> {
        let source = FileDataSource::new(PathBuf::from(path))
            .map_err(SuperpositionConfigError::InitializationError)?;
        let provider = LocalResolutionProvider::new(
            Box::new(source),
            None,
            RefreshStrategy::Watch(WatchStrategy::default()),
        );
        provider
            .init(EvaluationContext::default())
            .await
            .map_err(|error| SuperpositionConfigError::InitializationError(error.to_string()))?;

        Ok(Self { provider })
    }

    /// Resolve the flat key-value map for given dimensions.
    ///
    /// # Arguments
    /// * `connector` - The connector name (e.g., "stripe", "adyen")
    /// * `environment` - The environment name (e.g., "production", "sandbox", "development")
    ///
    /// # Returns
    /// A map of configuration keys to their resolved values.
    ///
    /// # Example
    /// ```ignore
    /// let resolved = config.resolve("stripe", "production").await?;
    /// let base_url = resolved.get("connector_base_url").and_then(|v| v.as_str());
    /// ```
    pub async fn resolve(
        &self,
        connector: &str,
        environment: &str,
    ) -> Result<Map<String, Value>, SuperpositionConfigError> {
        let context = EvaluationContext::default()
            .with_custom_field(DIMENSION_CONNECTOR, connector)
            .with_custom_field(DIMENSION_ENVIRONMENT, environment);

        self.provider
            .resolve_all_features(context)
            .await
            .map_err(|error| SuperpositionConfigError::ResolutionError(error.to_string()))
    }
}

/// Helper function to extract a string value from the resolved configuration.
///
/// Returns `Some(String)` if the key exists and the value is a string, `None` otherwise.
pub fn get_string(resolved: &Map<String, Value>, key: &str) -> Option<String> {
    resolved
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Helper function to extract an optional non-empty string from the resolved configuration.
///
/// Returns `Some(String)` if the key exists, is a string, and is non-empty; `None` otherwise.
pub fn get_optional_nonempty_string(resolved: &Map<String, Value>, key: &str) -> Option<String> {
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
/// let resolved = config.resolve("stripe", "production").await?;
/// let urls = get_connector_urls(&resolved);
/// ```
pub fn get_connector_urls(resolved: &Map<String, Value>) -> ConnectorUrls {
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
        let resolved = Map::new();
        assert_eq!(get_string(&resolved, "missing_key"), None);
    }

    #[test]
    fn test_get_string_returns_some_for_value() {
        let mut resolved = Map::new();
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
        let mut resolved = Map::new();
        resolved.insert("key".to_string(), Value::String("".to_string()));
        assert_eq!(get_optional_nonempty_string(&resolved, "key"), None);
    }

    #[test]
    fn test_get_optional_nonempty_string_returns_some_for_value() {
        let mut resolved = Map::new();
        resolved.insert("key".to_string(), Value::String("value".to_string()));
        assert_eq!(
            get_optional_nonempty_string(&resolved, "key"),
            Some("value".to_string())
        );
    }
}
