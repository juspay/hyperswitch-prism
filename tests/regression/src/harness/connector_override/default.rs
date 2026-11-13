use super::ConnectorOverride;

/// Default override strategy that relies purely on JSON override files.
#[derive(Debug, Clone)]
pub struct DefaultConnectorOverride {
    connector: String,
}

impl DefaultConnectorOverride {
    /// Creates a default strategy bound to a connector name.
    pub fn new(connector: String) -> Self {
        Self { connector }
    }
}

impl ConnectorOverride for DefaultConnectorOverride {
    fn connector_name(&self) -> &str {
        &self.connector
    }
}
