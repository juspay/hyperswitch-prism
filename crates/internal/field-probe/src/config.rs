use domain_types::connector_types::ConnectorEnum;
use grpc_api_types::payments::PaymentMethod;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

// ── Patch config (patch-config.toml) ──────────────────────────────────────────

/// A single patch rule.
/// The TOML table key is either "path" (flow-agnostic) or "flow.path" (flow-specific).
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Rule {
    /// Error strings the connector may emit (exact match after cleaning).
    pub(crate) aliases: Vec<String>,
    /// Value type: secret_string, string, bool, i32, country_us,
    /// future_usage_off_session, usd_money, full_browser_info, full_address,
    /// full_customer, redirection_response.
    #[serde(rename = "type")]
    pub(crate) patch_type: String,
    #[serde(default)]
    pub(crate) value: Option<String>,
}

/// Full contents of patch-config.toml.
/// Uses table-based format:
///   - [path] for flow-agnostic rules (e.g., [browser_info])
///   - [flow.path] for flow-specific rules (e.g., [capture.amount_to_capture])
#[derive(Debug, Clone, Default)]
pub(crate) struct PatchConfig {
    /// All rules indexed by their lookup key.
    /// Key is "path" for flow-agnostic, "flow.path" for flow-specific.
    pub(crate) rules: HashMap<String, Rule>,
}

impl<'de> Deserialize<'de> for PatchConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = HashMap::<String, Rule>::deserialize(deserializer)?;
        Ok(Self { rules: map })
    }
}

static PATCH_CONFIG: OnceLock<PatchConfig> = OnceLock::new();

pub(crate) fn get_patch_config() -> &'static PatchConfig {
    PATCH_CONFIG.get_or_init(|| {
        let paths = [
            "crates/internal/field-probe/patch-config.toml",
            "patch-config.toml",
            concat!(env!("CARGO_MANIFEST_DIR"), "/patch-config.toml"),
        ];
        for path in &paths {
            if let Ok(contents) = std::fs::read_to_string(path) {
                eprintln!("Loaded patch config from: {path}");
                return toml::from_str(&contents)
                    .unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"));
            }
        }
        eprintln!("Warning: No patch-config.toml found, using empty patch config");
        PatchConfig::default()
    })
}

// ── Operational config (probe-config.toml) ────────────────────────────────────

/// Per-connector request field overrides.
///
/// Connectors can override specific base request field values here.
/// For example, some connectors require `connector_transaction_id` to be
/// a numeric string (parseable as i64/u64) rather than the default
/// `"probe_connector_txn_001"`.
#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct ConnectorRequestOverrides {
    /// Override for connector_transaction_id (used by capture/void/get/refund/reverse flows).
    /// Set to a numeric string (e.g. "12345") for connectors that parse it as an integer.
    pub(crate) connector_transaction_id: Option<String>,
}

/// Configuration for the field-probe, loaded from probe-config.toml
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct ProbeConfig {
    pub(crate) probe: ProbeSettings,
    pub(crate) access_token: AccessTokenConfig,
    pub(crate) oauth_connectors: Vec<OAuthConnector>,
    /// Connectors to skip (exclude from probing). All others are probed.
    pub(crate) skip_connectors: Vec<String>,
    pub(crate) payment_methods: HashMap<String, bool>,
    pub(crate) connector_metadata: HashMap<String, String>,
    /// Per-connector request field overrides. Key is lowercase connector name.
    #[serde(default)]
    pub(crate) connector_overrides: HashMap<String, ConnectorRequestOverrides>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct ProbeSettings {
    pub(crate) max_iterations: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct AccessTokenConfig {
    pub(crate) token: String,
    pub(crate) token_type: String,
    pub(crate) expires_in_seconds: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct OAuthConnector {
    pub(crate) name: String,
}

impl ProbeConfig {
    pub(crate) fn load() -> Self {
        let config_paths = [
            "crates/internal/field-probe/probe-config.toml",
            "probe-config.toml",
            concat!(env!("CARGO_MANIFEST_DIR"), "/probe-config.toml"),
        ];
        for path in &config_paths {
            if let Ok(contents) = std::fs::read_to_string(path) {
                eprintln!("Loaded config from: {path}");
                return toml::from_str(&contents)
                    .unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"));
            }
        }
        eprintln!("Warning: No probe-config.toml found, using defaults");
        Self::default()
    }

    pub(crate) fn get_enabled_payment_methods(&self) -> Vec<(&'static str, fn() -> PaymentMethod)> {
        let all_methods = crate::registry::authorize_pm_variants_static();
        all_methods
            .into_iter()
            .filter(|(name, _)| self.payment_methods.get(*name).copied().unwrap_or(true))
            .collect()
    }
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            probe: ProbeSettings { max_iterations: 30 },
            connector_overrides: HashMap::new(),
            access_token: AccessTokenConfig {
                token: "probe_access_token".to_string(),
                token_type: "Bearer".to_string(),
                expires_in_seconds: 3600,
            },
            oauth_connectors: vec![
                OAuthConnector {
                    name: "airwallex".to_string(),
                },
                OAuthConnector {
                    name: "globalpay".to_string(),
                },
                OAuthConnector {
                    name: "jpmorgan".to_string(),
                },
                OAuthConnector {
                    name: "iatapay".to_string(),
                },
                OAuthConnector {
                    name: "getnet".to_string(),
                },
                OAuthConnector {
                    name: "payload".to_string(),
                },
                OAuthConnector {
                    name: "paypal".to_string(),
                },
                OAuthConnector {
                    name: "truelayer".to_string(),
                },
                OAuthConnector {
                    name: "volt".to_string(),
                },
            ],
            skip_connectors: vec![],
            payment_methods: HashMap::new(),
            connector_metadata: HashMap::new(),
        }
    }
}

static PROBE_CONFIG: OnceLock<ProbeConfig> = OnceLock::new();

pub(crate) fn get_config() -> &'static ProbeConfig {
    PROBE_CONFIG.get_or_init(ProbeConfig::load)
}

pub(crate) fn max_iterations() -> usize {
    get_config().probe.max_iterations
}

/// Returns an overridden `connector_transaction_id` for connectors that require a
/// specific format (e.g. a numeric string for connectors that parse it as i64/u64).
/// Returns `None` if no override is configured — the base request default is used.
pub(crate) fn connector_transaction_id_override(connector: &ConnectorEnum) -> Option<String> {
    let config = get_config();
    let name = format!("{connector:?}").to_lowercase();
    config
        .connector_overrides
        .get(&name)
        .and_then(|o| o.connector_transaction_id.clone())
}

/// Get connector-specific metadata JSON for connectors that require it
pub(crate) fn connector_feature_data_json(connector: &ConnectorEnum) -> Option<String> {
    let config = get_config();
    let name = format!("{connector:?}").to_lowercase();

    // First check if config has metadata for this connector
    if let Some(meta) = config.connector_metadata.get(&name) {
        return Some(meta.clone());
    }

    // Fall back to default if available
    config.connector_metadata.get("default").cloned()
}
