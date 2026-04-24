use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use common_utils::{
    connector_request_kafka::{ConnectorRequestKafkaConfig, ConnectorRequestKafkaConfigPatch},
    consts,
    events::{EventConfig, EventConfigPatch},
    metadata::{HeaderMaskingConfig, HeaderMaskingConfigPatch},
    SuperpositionConfig,
};
use domain_types::{
    connector_types::ConnectorEnum,
    types::{Connectors, ConnectorsPatch, Proxy, ProxyPatch},
};

use crate::{
    error::ConfigurationError,
    logger::config::{Log, LogPatch},
};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
#[derive(Debug, Deserialize, Serialize, Clone, config_patch_derive::Patch)]
pub struct Config {
    pub common: Common,
    pub server: Server,
    pub metrics: MetricsServer,
    pub log: Log,
    pub proxy: Proxy,
    pub connectors: Connectors,
    #[serde(default)]
    pub events: EventConfig,
    #[serde(default)]
    pub lineage: LineageConfig,
    #[serde(default)]
    pub unmasked_headers: HeaderMaskingConfig,
    #[serde(default)]
    pub test: TestConfig,
    #[serde(default)]
    pub api_tags: ApiTagConfig,
    #[serde(default)]
    pub webhook_source_verification_call: WebhookSourceVerificationCall,
    #[serde(default)]
    pub connector_request_kafka: ConnectorRequestKafkaConfig,
    /// Superposition configuration for connector URL resolution
    /// This is loaded at startup from config/superposition.toml
    #[serde(skip)]
    #[patch(ignore)]
    pub superposition_config: Option<Arc<SuperpositionConfig>>,
}

#[derive(Clone, Deserialize, Debug, Default, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct LineageConfig {
    /// Enable processing of x-lineage-ids header
    pub enabled: bool,
    /// Custom header name (default: x-lineage-ids)
    #[serde(default = "default_lineage_header")]
    pub header_name: String,
    /// Prefix for lineage fields in events
    #[serde(default = "default_lineage_prefix")]
    pub field_prefix: String,
}

fn default_lineage_header() -> String {
    consts::X_LINEAGE_IDS.to_string()
}

fn default_lineage_prefix() -> String {
    consts::LINEAGE_FIELD_PREFIX.to_string()
}

/// Test mode configuration for mock server integration
#[derive(Clone, Deserialize, Debug, Default, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct TestConfig {
    #[serde(default)]
    pub enabled: bool,
    pub mock_server_url: Option<String>,
}

impl TestConfig {
    /// Create test context if enabled, validating configuration
    pub fn create_test_context(
        &self,
        request_id: &str,
    ) -> Result<Option<external_services::service::TestContext>, config::ConfigError> {
        self.enabled
            .then(|| {
                self.mock_server_url
                    .as_ref()
                    .ok_or_else(|| {
                        config::ConfigError::Message(
                            "Test mode enabled but mock_server_url is not set".to_string(),
                        )
                    })
                    .map(|url| external_services::service::TestContext {
                        session_id: request_id.to_string(),
                        mock_server_url: url.clone(),
                    })
            })
            .transpose()
    }
}

/// API tag configuration for flow-based tagging with payment method type support
///
/// Environment variable format (case-insensitive):
/// - Simple flow: CS__API_TAGS__TAGS__PSYNC=GW_TXN_SYNC
/// - With payment method: CS__API_TAGS__TAGS__AUTHORIZE_UPICOLLECT=GW_INIT_COLLECT
///
/// TOML format:
/// ```toml
/// [api_tags.tags]
/// psync = "GW_TXN_SYNC"
/// authorize_upicollect = "GW_INIT_COLLECT"
/// ```
///
/// Note: Config crate lowercases env var keys, lookup is case-insensitive
#[derive(Clone, Deserialize, Debug, Default, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct ApiTagConfig {
    #[serde(default)]
    pub tags: std::collections::HashMap<String, String>,
}

impl ApiTagConfig {
    /// Get API tag for a flow, optionally refined by payment method type
    ///
    /// Lookup order (case-insensitive):
    /// 1. If payment_method_type provided: try "flow_paymentmethodtype" (composite key)
    /// 2. Fall back to "flow" (simple key)
    /// 3. Return None if not found
    ///
    /// Note: Keys are lowercased for lookup because config crate lowercases env var keys
    pub fn get_tag(
        &self,
        flow: common_utils::events::FlowName,
        payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> Option<String> {
        let flow_str = flow.as_str();

        payment_method_type.map_or_else(
            || {
                let result = self.tags.get(&flow_str.to_lowercase()).cloned();
                if result.is_none() {
                    tracing::debug!(
                        flow = %flow_str,
                        payment_method_type = ?payment_method_type,
                        "No API tag configured for flow"
                    );
                }
                result
            },
            |pmt| {
                let composite_key = format!("{flow_str}_{pmt:?}").to_lowercase();
                let result = self.tags.get(&composite_key).cloned();
                if result.is_none() {
                    tracing::debug!(
                        flow = %flow_str,
                        payment_method_type = ?payment_method_type,
                        "No API tag configured for flow with payment method type"
                    );
                }
                result
            },
        )
    }
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct Common {
    pub environment: consts::Env,
}

impl Default for Common {
    fn default() -> Self {
        Self {
            environment: consts::Env::Development,
        }
    }
}

impl Common {
    pub fn validate(&self) -> Result<(), config::ConfigError> {
        let Self { environment } = self;
        match environment {
            consts::Env::Development | consts::Env::Production | consts::Env::Sandbox => Ok(()),
        }
    }
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct Server {
    pub host: String,
    pub port: u16,
    #[serde(rename = "type", default)]
    pub type_: ServiceType,
}

#[derive(Clone, Deserialize, Debug, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct MetricsServer {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Deserialize, Serialize, Debug, Default, PartialEq, config_patch_derive::Patch)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    #[default]
    Grpc,
    Http,
}

/// Helper function to deserialize a comma-separated string into a HashSet
fn deserialize_hashset_inner<T>(value: impl AsRef<str>) -> Result<HashSet<T>, String>
where
    T: Eq + FromStr + std::hash::Hash,
    <T as FromStr>::Err: std::fmt::Display,
{
    let (values, errors) = value
        .as_ref()
        .trim()
        .split(',')
        .map(|s| {
            T::from_str(s.trim()).map_err(|error| {
                format!(
                    "Unable to deserialize `{}` as `{}`: {error}",
                    s.trim(),
                    std::any::type_name::<T>()
                )
            })
        })
        .fold(
            (HashSet::new(), Vec::new()),
            |(mut values, mut errors), result| match result {
                Ok(t) => {
                    values.insert(t);
                    (values, errors)
                }
                Err(error) => {
                    errors.push(error);
                    (values, errors)
                }
            },
        );
    if !errors.is_empty() {
        Err(format!("Some errors occurred:\n{}", errors.join("\n")))
    } else {
        Ok(values)
    }
}

/// Deserializer for Option<HashSet> from comma-separated string
/// Handles Option<String> input and returns Option<HashSet> for config_patch_derive::Patch
fn deserialize_hashset<'a, D, T>(deserializer: D) -> Result<Option<HashSet<T>>, D::Error>
where
    D: Deserializer<'a>,
    T: Eq + FromStr + std::hash::Hash,
    <T as FromStr>::Err: std::fmt::Display,
{
    match Option::<String>::deserialize(deserializer)? {
        Some(s) if !s.trim().is_empty() => deserialize_hashset_inner(s)
            .map(Some)
            .map_err(D::Error::custom),
        _ => Ok(None), // Empty string or None -> None
    }
}

/// Configuration for connectors that require external API calls for webhook source verification
/// (e.g., PayPal's verify-webhook-signature endpoint)
#[derive(Clone, Deserialize, Debug, Default, Serialize, PartialEq, config_patch_derive::Patch)]
pub struct WebhookSourceVerificationCall {
    /// Comma-separated list of connector names that require external verification calls
    /// Example: "paypal" or "paypal,adyen"
    #[serde(deserialize_with = "deserialize_hashset")]
    #[patch(ignore)]
    pub connectors_with_webhook_source_verification_call: Option<HashSet<ConnectorEnum>>,
}

impl WebhookSourceVerificationCall {
    /// Check if a connector requires external webhook source verification call
    pub fn requires_external_verification(&self, connector: &ConnectorEnum) -> bool {
        self.connectors_with_webhook_source_verification_call
            .as_ref()
            .map(|set| set.contains(connector))
            .unwrap_or(false)
    }
}

impl Config {
    /// Function to build the configuration by picking it from default locations
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::new_with_config_path(None)
    }

    /// Function to build the configuration by picking it from default locations
    pub fn new_with_config_path(
        explicit_config_path: Option<PathBuf>,
    ) -> Result<Self, config::ConfigError> {
        let env = consts::Env::current_env();
        let config_path = Self::config_path(&env, explicit_config_path);

        let config = Self::builder(&env)?
            .add_source(config::File::from(config_path).required(false))
            .add_source(
                config::Environment::with_prefix(consts::ENV_PREFIX)
                    .try_parsing(true)
                    .separator("__")
                    .list_separator(",")
                    .with_list_parse_key("proxy.bypass_proxy_urls")
                    .with_list_parse_key("redis.cluster_urls")
                    .with_list_parse_key("database.tenants")
                    .with_list_parse_key("log.kafka.brokers")
                    .with_list_parse_key("events.brokers")
                    .with_list_parse_key("connector_request_kafka.brokers")
                    .with_list_parse_key("unmasked_headers.keys"),
            )
            .build()?;

        #[allow(clippy::print_stderr)]
        let config: Self = serde_path_to_error::deserialize(config).map_err(|error| {
            eprintln!("Unable to deserialize application configuration: {error}");
            error.into_inner()
        })?;

        // Validate the environment field
        config.common.validate()?;

        Ok(config)
    }

    pub fn builder(
        environment: &consts::Env,
    ) -> Result<config::ConfigBuilder<config::builder::DefaultState>, config::ConfigError> {
        config::Config::builder()
            // Here, it should be `set_override()` not `set_default()`.
            // "env" can't be altered by config field.
            // Should be single source of truth.
            .set_override("env", environment.to_string())
    }

    /// Config path.
    pub fn config_path(
        environment: &consts::Env,
        explicit_config_path: Option<PathBuf>,
    ) -> PathBuf {
        let mut config_path = PathBuf::new();
        if let Some(explicit_config_path_val) = explicit_config_path {
            config_path.push(explicit_config_path_val);
        } else {
            let config_directory: String = "config".into();
            let config_file_name = environment.config_path();

            config_path.push(workspace_path());
            config_path.push(config_directory);
            config_path.push(config_file_name);
        }
        config_path
    }
}

impl Server {
    pub async fn tcp_listener(&self) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);

        tracing::info!(loc = %loc, "binding the server");

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}

impl MetricsServer {
    pub async fn tcp_listener(&self) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);

        tracing::info!(loc = %loc, "binding the server");

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}

pub fn workspace_path() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut path = PathBuf::from(manifest_dir.clone());

        // Traverse up until we find the workspace root (a Cargo.toml with [workspace])
        while path.parent().is_some() {
            let cargo_toml = path.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[workspace]") {
                        return path;
                    }
                }
            }
            path.pop();
        }

        // Fallback: return current dir if workspace not found
        PathBuf::from(manifest_dir)
    } else {
        PathBuf::from(".")
    }
}
