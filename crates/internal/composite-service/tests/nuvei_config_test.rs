use grpc_api_types::payments::{connector_specific_config, ConnectorSpecificConfig};
use hyperswitch_masking::PeekInterface;

#[test]
fn nuvei_accepts_legacy_txns_credentials_and_serializes_canonical_names() {
    for credentials in [
        serde_json::json!({"merchantId":"merchant", "merchantSiteId":"site", "merchantKey":"secret", "route_to_ucs":"true"}),
        serde_json::json!({"merchant_id":"merchant", "merchant_site_id":"site", "merchant_secret":"secret"}),
    ] {
        let config: ConnectorSpecificConfig =
            serde_json::from_value(serde_json::json!({"config":{"Nuvei":credentials}})).unwrap();
        let Some(connector_specific_config::Config::Nuvei(nuvei)) = config.config else {
            panic!("Expected Nuvei config");
        };
        assert_eq!(nuvei.merchant_id.as_ref().unwrap().peek(), "merchant");
        assert_eq!(nuvei.merchant_site_id.as_ref().unwrap().peek(), "site");
        assert_eq!(nuvei.merchant_secret.as_ref().unwrap().peek(), "secret");
        let canonical = serde_json::to_value(nuvei).unwrap();
        assert!(canonical.get("merchant_id").is_some());
        assert!(canonical.get("merchant_site_id").is_some());
        assert!(canonical.get("merchant_secret").is_some());
        assert!(canonical.get("merchantKey").is_none());
    }
}
