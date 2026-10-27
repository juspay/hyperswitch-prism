use serde_json::Value;

use super::ConnectorOverride;

/// Nuvei-specific override.
///
/// **Mandate-reference oneof** (`normalize_tonic_request`) – the
/// `RecurringPaymentService/Charge` suite `context_map` injects the
/// SetupRecurring UPO into `connector_recurring_payment_id.connector_mandate_id`
/// even when the scenario chose `network_mandate_id` / `network_token_with_nti`.
/// A real caller can send only one variant of that oneof, so the injected
/// `connector_mandate_id` is removed and the scenario's choice reaches Nuvei.
/// `connector_feature_data` is left alone: the connector prefers the request
/// `session_token` for Charge.
#[derive(Debug, Clone, Default)]
pub struct NuveiConnectorOverride;

impl NuveiConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for NuveiConnectorOverride {
    fn connector_name(&self) -> &str {
        "nuvei"
    }

    fn normalize_tonic_request(&self, suite: &str, _scenario: &str, req: &mut Value) {
        if suite != "RecurringPaymentService/Charge" {
            return;
        }

        let Some(map) = req.as_object_mut() else {
            return;
        };

        for mandate_key in [
            "connector_recurring_payment_id",
            "connectorRecurringPaymentId",
        ] {
            let Some(Value::Object(mandate_ref)) = map.get_mut(mandate_key) else {
                continue;
            };

            let scenario_chose_other_variant = [
                "network_mandate_id",
                "networkMandateId",
                "network_token_with_nti",
                "networkTokenWithNti",
            ]
            .iter()
            .any(|key| mandate_ref.get(*key).is_some_and(|value| !value.is_null()));

            if scenario_chose_other_variant {
                mandate_ref.remove("connector_mandate_id");
                mandate_ref.remove("connectorMandateId");
            }
        }
    }
}
