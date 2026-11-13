use super::ConnectorOverride;

/// Glomopay-specific override.
///
/// No test-side normalisation is required — the sandbox supports every
/// suite listed in `connector_specs/glomopay/specs.json` end-to-end, so
/// assertions run against the real connector response.
#[derive(Debug, Clone, Default)]
pub struct GlomopayConnectorOverride;

impl GlomopayConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for GlomopayConnectorOverride {
    fn connector_name(&self) -> &str {
        "glomopay"
    }
}
