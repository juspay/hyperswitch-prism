use serde_json::Value;

use super::ConnectorOverride;

/// Helcim-specific override.
///
/// 1. **Amount jitter** (`normalize_tonic_request`) – Helcim's sandbox rejects
///    identical card+amount combinations within a 5-minute window ("Suspected
///    duplicate transaction").  A small epoch-derived offset is added to every
///    authorize amount so that the standalone authorize run and the capture
///    dependency's authorize run never collide.  The capture suite picks up the
///    actual authorized amount via `context_map` in the suite spec.
///
/// 2. **Transaction ID promotion** (`transform_response`) – Helcim returns the
///    pre-auth transaction ID inside `connectorFeatureData.value` (JSON string
///    `{"preauth_transaction_id":"<id>"}`) instead of the standard
///    `connectorTransactionId` field.  This hook promotes the value so implicit
///    context resolution propagates it into downstream capture/void/refund
///    requests.
#[derive(Debug, Clone, Default)]
pub struct HelcimConnectorOverride;

impl HelcimConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for HelcimConnectorOverride {
    fn connector_name(&self) -> &str {
        "helcim"
    }

    #[allow(clippy::as_conversions)] // value is ≤ 999 after modulo, safe to narrow
    fn normalize_tonic_request(&self, suite: &str, _scenario: &str, req: &mut Value) {
        if suite != "PaymentService/Authorize" {
            return;
        }

        // Use low-order bits of epoch millis as jitter (1..999 cents).
        // This ensures every authorize call within a 1-second window gets a
        // unique amount while staying within a small range.
        let jitter = ((std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 999)
            + 1) as i64;

        if let Some(amount) = req.pointer_mut("/amount/minor_amount") {
            if let Some(base) = amount.as_i64() {
                *amount = Value::Number((base + jitter).into());
            }
        }
    }

    fn transform_response(&self, suite: &str, _scenario: &str, response: &mut Value) {
        if suite != "PaymentService/Authorize" {
            return;
        }

        // Already present — nothing to do.
        let has_txn_id = response
            .pointer("/connector_transaction_id")
            .or_else(|| response.pointer("/connectorTransactionId"))
            .and_then(Value::as_str)
            .is_some_and(|v| !v.trim().is_empty());

        if has_txn_id {
            return;
        }

        // Extract from connectorFeatureData.value JSON string.
        let feature_str = response
            .pointer("/connectorFeatureData/value")
            .or_else(|| response.pointer("/connector_feature_data/value"))
            .and_then(Value::as_str);

        let Some(feature_str) = feature_str else {
            return;
        };

        let Ok(parsed): Result<Value, _> = serde_json::from_str(feature_str) else {
            return;
        };

        let Some(preauth_id) = parsed
            .get("preauth_transaction_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return;
        };

        if let Some(root) = response.as_object_mut() {
            root.insert(
                "connectorTransactionId".to_string(),
                Value::String(preauth_id),
            );
        }
    }
}
