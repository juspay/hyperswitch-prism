//! Security policy for request-controlled configuration versus operator configuration.
#![allow(clippy::expect_used)]

use common_utils::config_patch::Patch;
use common_utils::consts::Env;
use serde_json::json;
use ucs_env::configs::{Config, ConfigPatch};

#[test]
fn request_overrides_reject_sensitive_logging_and_response_settings() {
    for patch in [
        json!({"unmasked_headers": {"keys": ["x-connector-config", "x-api-key"]}}),
        json!({"connector_response_masking": {"enabled": false}}),
        json!({"connector_response_masking": {"connector_keys": {"adyen": "email,address"}}}),
        json!({"log": {"fields": {"enabled": true}}}),
        json!({"log": {"fields": {"outgoing": {"email": {"source": "response.body"}}}}}),
        json!({"log": {"console": {"level": "DEBUG"}}}),
        json!({"log": {"kafka": {"brokers": ["example.invalid:9092"]}}}),
        json!({"events": {"enabled": false}}),
        json!({"events": {"static_values": {"merchant_id": "another-merchant"}}}),
        json!({"common": {"return_raw_connector_data": true}}),
        json!({"common": {"return_raw_connector_data": false}}),
        json!({"common": {"return_typed_connector_data": true}}),
        json!({"common": {"return_typed_connector_data": false}}),
        // A valid sibling must not cause a forbidden override to be silently accepted.
        json!({"common": {"environment": "sandbox", "return_raw_connector_data": true}}),
    ] {
        assert!(
            serde_json::from_value::<ConfigPatch>(patch.clone()).is_err(),
            "sensitive override was accepted: {patch}"
        );
    }
}

#[test]
fn operator_config_still_controls_sensitive_settings() {
    let operator_toml =
        std::fs::read_to_string(ucs_env::configs::workspace_path().join("config/development.toml"))
            .expect("read operator config");
    let mut value = serde_json::to_value(
        toml::from_str::<toml::Value>(&operator_toml).expect("parse operator config"),
    )
    .expect("convert operator config");
    value["unmasked_headers"] = json!({"keys": ["x-request-id"]});
    value["connector_response_masking"] =
        json!({"enabled": true, "connector_keys": {"adyen": "pspReference"}});
    value["log"]["fields"]["enabled"] = json!(true);
    value["events"]["enabled"] = json!(true);
    value["common"]["return_raw_connector_data"] = json!(false);
    value["common"]["return_typed_connector_data"] = json!(false);

    let mut config: Config = serde_json::from_value(value).expect("load operator config");
    config.post_patch_processing();
    assert!(config.unmasked_headers.should_unmask("x-request-id"));
    assert!(!config.unmasked_headers.should_unmask("x-api-key"));
    assert!(config.connector_response_masking.enabled);
    assert!(config.log.fields.enabled);
    assert!(config.events.enabled);
    assert!(!config.common.return_raw_connector_data);
    assert!(!config.common.return_typed_connector_data);
    #[cfg(feature = "log-transformations")]
    {
        assert!(config.masking_keys.enabled);
        assert!(config.log_fields.enabled);
    }

    let patch: ConfigPatch = serde_json::from_value(json!({
        "common": {"environment": "sandbox"},
        "connectors": {"adyen": {"base_url": "https://override.adyen.example"}},
        "proxy": {"connector_request_timeout": 15}
    }))
    .expect("supported request overrides should deserialize");
    config.apply(patch);
    config.post_patch_processing();
    assert_eq!(config.common.environment, Env::Sandbox);
    assert_eq!(
        config.connectors.adyen.base_url,
        "https://override.adyen.example"
    );
    assert_eq!(config.proxy.connector_request_timeout, Some(15));
    assert!(config.unmasked_headers.should_unmask("x-request-id"));
    assert!(config.connector_response_masking.enabled);
    assert!(config.log.fields.enabled);
    assert!(config.events.enabled);
    assert!(!config.common.return_raw_connector_data);
    assert!(!config.common.return_typed_connector_data);
}
