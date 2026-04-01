use serde_json::Value;

use super::ConnectorOverride;

#[derive(Debug, Clone, Default)]
pub struct CybersourceConnectorOverride;

impl CybersourceConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for CybersourceConnectorOverride {
    fn connector_name(&self) -> &str {
        "cybersource"
    }

    fn transform_response(&self, suite: &str, _scenario: &str, response: &mut Value) {
        if suite != "authenticate" {
            return;
        }

        let has_connector_transaction_id = response
            .pointer("/authentication_data/connector_transaction_id")
            .or_else(|| response.pointer("/authenticationData/connectorTransactionId"))
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());

        if has_connector_transaction_id {
            return;
        }

        let raw_response = response
            .pointer("/raw_connector_response/value")
            .or_else(|| response.pointer("/rawConnectorResponse/value"))
            .and_then(Value::as_str);

        let Some(raw_response) = raw_response else {
            return;
        };

        let Ok(parsed_raw): Result<Value, _> = serde_json::from_str(raw_response) else {
            return;
        };

        let Some(authentication_transaction_id) = parsed_raw
            .pointer("/consumerAuthenticationInformation/authenticationTransactionId")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return;
        };

        let Some(root) = response.as_object_mut() else {
            return;
        };

        if let Some(Value::Object(authentication_data)) = root.get_mut("authenticationData") {
            authentication_data.insert(
                "connectorTransactionId".to_string(),
                Value::String(authentication_transaction_id),
            );
            return;
        }

        if let Some(Value::Object(authentication_data)) = root.get_mut("authentication_data") {
            authentication_data.insert(
                "connector_transaction_id".to_string(),
                Value::String(authentication_transaction_id),
            );
            return;
        }

        let mut authentication_data = serde_json::Map::new();
        authentication_data.insert(
            "connector_transaction_id".to_string(),
            Value::String(authentication_transaction_id),
        );
        root.insert(
            "authentication_data".to_string(),
            Value::Object(authentication_data),
        );
    }
}
