//! Superposition configuration wrapper for connector-service
//!
//! This module provides a thin wrapper around Superposition's local provider
//! for loading and resolving configuration based on dimensions (connector, environment).

use std::{fmt, path::PathBuf};

use hyperswitch_masking::{PeekInterface, Secret};
use serde_json::{Map, Value};
use superposition_provider::{
    data_source::{file::FileDataSource, http::HttpDataSource},
    traits::AllFeatureProvider,
    EvaluationContext, LocalResolutionProvider, PollingStrategy, RefreshStrategy,
    SuperpositionDataSource, SuperpositionOptions, WatchStrategy,
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
    #[error("Invalid superposition configuration: {0}")]
    InvalidConfiguration(String),
}

/// The `[superposition]` table: where policy comes from.
///
/// `enabled = false` (the default) keeps today's behaviour — the baked
/// `config/superposition.toml`, watched for changes. `enabled = true` points the same
/// provider at a remote workspace (polled), with the baked file as the fallback the
/// provider consults if the remote source cannot initialise. Mirrors hyperswitch's
/// `SuperpositionClientConfig` field for field; `enabled` is the one addition, because
/// prism must boot file-first wherever no workspace exists yet.
///
/// Env overrides: `CS__SUPERPOSITION__{ENABLED,ENDPOINT,TOKEN,ORG_ID,WORKSPACE_ID,
/// POLLING_INTERVAL,REQUEST_TIMEOUT,BACKUP_FILE_PATH}`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SuperpositionSettings {
    pub enabled: bool,
    /// Superposition server URL.
    pub endpoint: String,
    /// Workspace bearer token (a secret: never logged).
    pub token: Secret<String>,
    pub org_id: String,
    pub workspace_id: String,
    /// Seconds between polls of the remote workspace.
    pub polling_interval: u64,
    /// Request timeout in seconds for the poll. Kept for parity with hyperswitch; the
    /// provider only logs it today.
    pub request_timeout: Option<u64>,
    /// Fallback the provider loads if the remote source fails at init. Consulted at init
    /// ONLY — after a successful init a failed poll keeps the last good snapshot. Defaults
    /// to the baked `config/superposition.toml` when unset.
    pub backup_file_path: Option<PathBuf>,
}

impl Default for SuperpositionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            token: Secret::new(String::new()),
            org_id: String::new(),
            workspace_id: String::new(),
            polling_interval: 15,
            request_timeout: None,
            backup_file_path: None,
        }
    }
}

impl SuperpositionSettings {
    /// A deliberately enabled remote source with a broken config is a deployment error:
    /// fail loud at boot rather than silently serving the file. Only meaningful when
    /// `enabled`; callers skip it otherwise.
    pub fn validate(&self) -> Result<(), SuperpositionConfigError> {
        let invalid = |message: &str| {
            Err(SuperpositionConfigError::InvalidConfiguration(
                message.to_string(),
            ))
        };
        if self.endpoint.trim().is_empty() {
            return invalid("superposition.endpoint cannot be empty");
        }
        if url::Url::parse(&self.endpoint).is_err() {
            return invalid("superposition.endpoint must be a valid URL");
        }
        if self.token.peek().trim().is_empty() {
            return invalid("superposition.token cannot be empty");
        }
        if self.org_id.trim().is_empty() {
            return invalid("superposition.org_id cannot be empty");
        }
        if self.workspace_id.trim().is_empty() {
            return invalid("superposition.workspace_id cannot be empty");
        }
        Ok(())
    }
}

/// Which source the provider was built on. Logged once at boot so a pod that fell
/// back to the file — and therefore will not follow the workspace — is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Remote workspace polled on an interval, the baked file as init-time fallback.
    /// The only source that can carry experiments.
    Remote,
    /// The baked `config/superposition.toml`, watched for changes. No experiments.
    File,
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Remote => "remote",
            Self::File => "file",
        })
    }
}

/// Superposition's local provider over whichever source [`SuperpositionConfig::new`]
/// selected. Evaluation is always in-process against the provider's cached snapshot;
/// only the source's refresh differs (poll vs. file watch).
#[derive(Clone)]
pub struct SuperpositionConfig {
    provider: LocalResolutionProvider,
    source: SourceKind,
}

impl fmt::Debug for SuperpositionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SuperpositionConfig({})", self.source)
    }
}

impl SuperpositionConfig {
    /// Build the provider the settings ask for.
    ///
    /// `enabled` → the remote workspace as primary with the backup file (default: the
    /// baked file at `baked_path`) as the provider's init-time fallback, polled every
    /// `polling_interval` seconds — hyperswitch's `SuperpositionClient::new` shape. If
    /// even that cannot initialise (remote unreachable AND fallback unreadable), boot
    /// continues on the baked file alone, loudly: prism's contract is fail-open.
    ///
    /// Not `enabled` → the baked file, watched (today's behaviour).
    ///
    /// `Err` only when NO source could initialise; the caller then runs without
    /// Superposition (static connector config, sampler in its no-source state).
    pub async fn new(
        settings: &SuperpositionSettings,
        baked_path: &str,
    ) -> Result<Self, SuperpositionConfigError> {
        if settings.enabled {
            let fallback_path = settings
                .backup_file_path
                .clone()
                .unwrap_or_else(|| PathBuf::from(baked_path));
            let primary = HttpDataSource::new(SuperpositionOptions::new(
                settings.endpoint.clone(),
                settings.token.peek().clone(),
                settings.org_id.clone(),
                settings.workspace_id.clone(),
            ));
            // A missing fallback file only warns: the remote source may still
            // initialise on its own, exactly as hyperswitch treats it.
            let fallback: Option<Box<dyn SuperpositionDataSource>> = match FileDataSource::new(
                fallback_path.clone(),
            ) {
                Ok(source) => Some(Box::new(source)),
                Err(error) => {
                    tracing::warn!(
                        path = %fallback_path.display(),
                        %error,
                        "superposition fallback file unavailable; the remote source has no init-time fallback"
                    );
                    None
                }
            };
            let strategy = RefreshStrategy::Polling(PollingStrategy {
                interval: settings.polling_interval,
                timeout: settings.request_timeout,
            });
            match Self::with_sources(Box::new(primary), fallback, strategy, SourceKind::Remote)
                .await
            {
                Ok(config) => return Ok(config),
                Err(error) => tracing::error!(
                    %error,
                    endpoint = %settings.endpoint,
                    workspace = %settings.workspace_id,
                    "superposition remote source failed to initialise; falling back to the \
                     baked file — policy will NOT follow the workspace until restart"
                ),
            }
        }
        Self::from_file(baked_path).await
    }

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
        Self::with_sources(
            Box::new(source),
            None,
            RefreshStrategy::Watch(WatchStrategy::default()),
            SourceKind::File,
        )
        .await
    }

    /// The provider over explicit sources. `from_file` and `new` are the two shapes
    /// prism ships; this is the seam for either, and for tests that need a custom
    /// data source.
    pub async fn with_sources(
        primary: Box<dyn SuperpositionDataSource>,
        fallback: Option<Box<dyn SuperpositionDataSource>>,
        strategy: RefreshStrategy,
        source: SourceKind,
    ) -> Result<Self, SuperpositionConfigError> {
        let provider = LocalResolutionProvider::new(primary, fallback, strategy);
        provider
            .init(EvaluationContext::default())
            .await
            .map_err(|error| SuperpositionConfigError::InitializationError(error.to_string()))?;
        Ok(Self { provider, source })
    }

    /// Which source this provider was built on.
    pub fn source(&self) -> SourceKind {
        self.source
    }

    /// Whether this source can carry experiments at all. A file cannot — a policy
    /// that expects experiments to sample requests in will silently see none.
    pub fn experiments_supported(&self) -> bool {
        matches!(self.source, SourceKind::Remote)
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
        self.resolve_with(&[
            (DIMENSION_CONNECTOR, connector),
            (DIMENSION_ENVIRONMENT, environment),
        ])
        .await
    }

    /// Resolve with caller-supplied dimensions. `resolve` delegates here; callers
    /// with other dimension sets (the déjà sampler's `environment` × `rpc_method` ×
    /// `rpc_service`) use this directly. Evaluation runs in-process against the
    /// provider's cached snapshot, which the file watcher refreshes on change — so a
    /// caller must not cache the result across requests.
    pub async fn resolve_with(
        &self,
        dimensions: &[(&str, &str)],
    ) -> Result<Map<String, Value>, SuperpositionConfigError> {
        let context = dimensions
            .iter()
            .fold(EvaluationContext::default(), |context, (key, value)| {
                context.with_custom_field(*key, *value)
            });

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

    fn remote_settings() -> SuperpositionSettings {
        SuperpositionSettings {
            enabled: true,
            endpoint: "http://superposition:8080".to_string(),
            token: Secret::new("sp_token".to_string()),
            org_id: "hyperswitch".to_string(),
            workspace_id: "prism".to_string(),
            ..SuperpositionSettings::default()
        }
    }

    /// The default table is the file-only posture: off, nothing to validate.
    #[test]
    fn settings_default_is_disabled_with_hyperswitch_polling_interval() {
        let settings = SuperpositionSettings::default();
        assert!(!settings.enabled);
        assert_eq!(settings.polling_interval, 15);
        assert!(settings.backup_file_path.is_none());
    }

    /// Mirrors hyperswitch's `SuperpositionClientConfig::validate`: every field a
    /// remote source needs is checked, and each failure names its field.
    #[test]
    fn settings_validate_rejects_each_missing_remote_field() {
        assert!(remote_settings().validate().is_ok());
        let cases: [(&str, Box<dyn Fn(&mut SuperpositionSettings)>); 5] = [
            ("endpoint", Box::new(|s| s.endpoint = "  ".to_string())),
            (
                "valid URL",
                Box::new(|s| s.endpoint = "not a url".to_string()),
            ),
            ("token", Box::new(|s| s.token = Secret::new(String::new()))),
            ("org_id", Box::new(|s| s.org_id = String::new())),
            ("workspace_id", Box::new(|s| s.workspace_id = String::new())),
        ];
        for (needle, break_it) in cases {
            let mut settings = remote_settings();
            break_it(&mut settings);
            let error = settings.validate().expect_err(needle).to_string();
            assert!(error.contains(needle), "{error} should mention {needle}");
        }
    }
}
