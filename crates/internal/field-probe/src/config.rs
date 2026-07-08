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

/// A single field patch within a [[multi]] rule.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct MultiPatch {
    /// JSON path to set (e.g. "address.billing_address.phone_number").
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) patch_type: String,
    #[serde(default)]
    pub(crate) value: Option<String>,
}

/// A multi-field rule: one error alias triggers multiple simultaneous patches.
/// Defined as `[[multi]]` in patch-config.toml.
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct MultiRule {
    /// Error strings that trigger all patches at once.
    pub(crate) aliases: Vec<String>,
    /// Fields to set simultaneously when the alias matches.
    pub(crate) patches: Vec<MultiPatch>,
}

/// Full contents of patch-config.toml.
/// Uses table-based format:
///   - [path] for flow-agnostic rules (e.g., [browser_info])
///   - [flow.path] for flow-specific rules (e.g., [capture.amount_to_capture])
///   - [[multi]] for rules that patch multiple fields at once
#[derive(Debug, Clone, Default)]
pub(crate) struct PatchConfig {
    /// All rules indexed by their lookup key.
    /// Key is "path" for flow-agnostic, "flow.path" for flow-specific.
    pub(crate) rules: HashMap<String, Rule>,
    /// Multi-field rules: one alias → multiple fields patched simultaneously.
    pub(crate) multi: Vec<MultiRule>,
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
                return parse_patch_config(&contents, path);
            }
        }
        eprintln!("Warning: No patch-config.toml found, using empty patch config");
        PatchConfig::default()
    })
}

fn parse_patch_config(contents: &str, path: &str) -> PatchConfig {
    let value: toml::Value =
        toml::from_str(contents).unwrap_or_else(|e| panic!("Failed to parse {path} as TOML: {e}"));

    let table = value
        .as_table()
        .unwrap_or_else(|| panic!("{path}: root is not a table"));

    let multi = table
        .get("multi")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.clone().try_into().ok())
                .collect()
        })
        .unwrap_or_default();

    let rules = table
        .iter()
        .filter_map(|(key, val)| {
            if key == "multi" {
                return None;
            }
            let rule: Option<Rule> = val.clone().try_into().ok();
            rule.map(|r| (key.clone(), r))
        })
        .collect();

    PatchConfig { rules, multi }
}

// ── Operational config (probe-config.toml) ────────────────────────────────────

/// Per-connector request field overrides.
///
/// Connectors can override specific base request field values here.
///
/// Two formats under `[connector_overrides]`:
///
/// 1. Scalar connector overrides (keyed by connector name):
/// ```toml
/// [connector_overrides.braintree]
/// connector_transaction_id = "12345"
/// ```
///
/// 2. Flow-specific field pre-sets (keyed by flow first, then connector):
/// ```toml
/// [connector_overrides.refund_get.braintree]
/// "refund_metadata" = '{"merchant_account_id":"probe_merchant_account","merchant_config_currency":"USD","currency":"USD"}'
/// ```
///
/// Fields under `[connector_overrides.<flow>.<connector>]` are pre-applied to the base request
/// before probing begins, so the probe never gets stuck on connector-specific required fields.
#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct ConnectorRequestOverrides {
    /// Per-flow field pre-sets keyed as [connector_overrides."flow1,flow2".<connector>].
    /// These are pre-applied to the base request before probing begins.
    #[serde(flatten)]
    pub(crate) flow_overrides: HashMap<String, HashMap<String, String>>,
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

#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct AccessTokenConfig {
    pub(crate) token: String,
    pub(crate) token_type: String,
    pub(crate) expires_in_seconds: i64,
    /// Per-connector access token overrides. Key is lowercase connector name.
    /// Some OAuth connectors require special token formats.
    #[serde(default)]
    pub(crate) overrides: HashMap<String, String>,
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
                overrides: HashMap::new(),
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

/// Get connector-specific access token for OAuth connectors.
/// Some connectors (like fiservcommercehub) require special token formats.
/// Returns the default token if no override is configured.
pub(crate) fn connector_access_token_override(connector: &ConnectorEnum) -> Option<String> {
    let config = get_config();
    let name = format!("{connector:?}").to_lowercase();
    config.access_token.overrides.get(&name).cloned()
}

/// Returns per-flow field pre-sets for a connector, if any are configured.
/// These are applied to the base request before probing begins.
pub(crate) fn connector_flow_overrides(
    connector: &ConnectorEnum,
    flow: &str,
) -> Option<&'static HashMap<String, String>> {
    let config = get_config();
    let name = format!("{connector:?}").to_lowercase();
    // Pass 1: exact flow key  →  [connector_overrides.<flow>.<connector>]
    // Pass 2: multi-flow key  →  [connector_overrides."get,refund,void".<connector>]
    //         Find a comma-separated key that includes this flow AND has this connector.
    config
        .connector_overrides
        .get(flow)
        .and_then(|o| o.flow_overrides.get(&name))
        .or_else(|| {
            config
                .connector_overrides
                .iter()
                .find(|(k, o)| {
                    k.split(',').map(str::trim).any(|f| f == flow)
                        && o.flow_overrides.contains_key(&name)
                })
                .and_then(|(_, o)| o.flow_overrides.get(&name))
        })
}
