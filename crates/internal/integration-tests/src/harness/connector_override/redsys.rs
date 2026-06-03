use serde_json::Value;
use uuid::Uuid;

use super::ConnectorOverride;

/// Redsys-specific test-side request normalisation.
///
/// Redsys's `/v1/payments` form field `DS_MERCHANT_ORDER` (and the
/// equivalent at `/v1/preauthorize`) must be exactly:
///
/// - 12 characters total,
/// - alphanumeric,
/// - first 4 characters digits.
///
/// The production connector transformer (`get_ds_merchant_order` in
/// `redsys/transformers.rs`) correctly rejects anything that doesn't
/// match — that's real validation we don't want to relax just to make
/// tests pass.
///
/// Instead, this hook generates a `0001<8hex>` value at test time and
/// writes it into the request body's `merchant_transaction_id` (for the
/// Authorize family) or `merchant_order_id` (for PreAuthenticate). Prism
/// then derives `connector_request_reference_id` from that field and
/// hands it to the connector in the expected shape.
///
/// Other suites (Capture/Refund/Void/Authenticate) inherit the reference
/// through suite context propagation from their upstream Authorize or
/// PreAuthenticate response, so they don't need a fresh value generated
/// here.
#[derive(Debug, Clone, Default)]
pub struct RedsysConnectorOverride;

impl RedsysConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for RedsysConnectorOverride {
    fn connector_name(&self) -> &str {
        "redsys"
    }

    fn normalize_tonic_request(&self, suite: &str, _scenario: &str, req: &mut Value) {
        let field = match suite {
            "PaymentService/Authorize" => "merchant_transaction_id",
            "PaymentMethodAuthenticationService/PreAuthenticate" => "merchant_order_id",
            _ => return,
        };

        let Some(obj) = req.as_object_mut() else {
            return;
        };

        obj.insert(
            field.to_string(),
            Value::String(redsys_short_reference_id()),
        );
    }
}

/// `0001` + 8 hex chars = 12 chars total, all alphanumeric, first 4 digits.
fn redsys_short_reference_id() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    format!("0001{}", &uuid[..8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn short_reference_id_matches_redsys_format() {
        let id = redsys_short_reference_id();
        assert_eq!(id.len(), 12);
        assert!(id.starts_with("0001"));
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn authorize_sets_merchant_transaction_id() {
        let strategy = RedsysConnectorOverride::new();
        let mut req = json!({"amount": {"minor_amount": 100}});
        strategy.normalize_tonic_request("PaymentService/Authorize", "no3ds", &mut req);

        let id = req["merchant_transaction_id"].as_str().expect("string");
        assert_eq!(id.len(), 12);
        assert!(id.starts_with("0001"));
    }

    #[test]
    fn pre_authenticate_sets_merchant_order_id() {
        let strategy = RedsysConnectorOverride::new();
        let mut req = json!({"amount": {"minor_amount": 100}});
        strategy.normalize_tonic_request(
            "PaymentMethodAuthenticationService/PreAuthenticate",
            "threeds_card",
            &mut req,
        );

        let id = req["merchant_order_id"].as_str().expect("string");
        assert_eq!(id.len(), 12);
        assert!(id.starts_with("0001"));
        assert!(req.get("merchant_transaction_id").is_none());
    }

    #[test]
    fn unrelated_suites_are_unchanged() {
        let strategy = RedsysConnectorOverride::new();
        let mut req = json!({"connector_transaction_id": "0001abc"});
        let original = req.clone();
        strategy.normalize_tonic_request("PaymentService/Capture", "capture_full", &mut req);
        assert_eq!(req, original);
    }
}
