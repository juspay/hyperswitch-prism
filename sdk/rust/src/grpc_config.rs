use serde::Serialize;
use std::collections::HashMap;

/// Connector-specific configuration value for x-connector-config header.
///
/// This represents the inner config object for a specific connector.
/// The connector name should match the connector enum variant (e.g., "Stripe", "Adyen").
#[derive(Debug, Clone, Serialize)]
pub struct ConnectorSpecificConfig {
    /// Primary API key / access token for the connector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// API secret — required by some connectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_secret: Option<String>,
    /// Additional credential — connector-specific.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key1: Option<String>,
    /// Merchant identifier — required by some connectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_id: Option<String>,
    /// Tenant identifier — required by multi-tenant deployments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

impl ConnectorSpecificConfig {
    /// Create a new connector-specific config with just an API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            api_secret: None,
            key1: None,
            merchant_id: None,
            tenant_id: None,
        }
    }
}

/// Configuration for connecting to the hosted connector-service gRPC server.
///
/// The typed fields map directly to the gRPC metadata headers the server
/// expects on every call. Set once at client init — headers are injected
/// automatically on every request via [`GrpcClient`].
///
/// | Field             | Header              | Required |
/// |-------------------|---------------------|----------|
/// | `endpoint`        | —                   | always   |
/// | `connector`       | `x-connector`       | always   |
/// | `connector_config`| `x-connector-config`| always   |
///
/// The `connector_config` field contains the connector-specific authentication
/// and configuration in the format expected by the server:
/// `{"config": {"ConnectorName": {"api_key": "...", ...}}}`
///
/// [`GrpcClient`]: crate::GrpcClient
pub struct GrpcConfig {
    /// Server endpoint, e.g. `"http://localhost:8000"` (plain) or
    /// `"https://grpc.example.com"` (TLS).
    pub endpoint: String,
    /// Which payment connector to route to, e.g. `"stripe"`, `"worldpay"`.
    pub connector: String,
    /// Connector-specific configuration for authentication.
    /// This will be serialized as JSON and sent in the `x-connector-config` header.
    /// Format: `{"config": {"ConnectorName": {"api_key": "...", ...}}}`
    pub connector_config: serde_json::Value,
}

impl GrpcConfig {
    pub(crate) fn into_headers(self) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("x-connector".into(), self.connector);
        // Serialize connector_config to JSON string for x-connector-config header
        let config_json =
            serde_json::to_string(&self.connector_config).unwrap_or_else(|_| "{}".to_string());
        h.insert("x-connector-config".into(), config_json);
        h
    }
}

/// Helper to build connector config in the expected format.
///
/// # Example
/// ```
/// use hyperswitch_payments_client::{build_connector_config, ConnectorSpecificConfig};
///
/// let config = build_connector_config("Stripe", ConnectorSpecificConfig::new("sk_test_..."));
/// // Results in: {"config": {"Stripe": {"api_key": "sk_test_..."}}}
/// ```
pub fn build_connector_config(
    connector_name: impl Into<String>,
    config: ConnectorSpecificConfig,
) -> serde_json::Value {
    let connector_name = connector_name.into();
    let config_obj = serde_json::to_value(config).unwrap_or_default();

    let mut connector_map = serde_json::Map::new();
    connector_map.insert(connector_name, config_obj);

    let mut root = serde_json::Map::new();
    root.insert(
        "config".to_string(),
        serde_json::Value::Object(connector_map),
    );

    serde_json::Value::Object(root)
}
