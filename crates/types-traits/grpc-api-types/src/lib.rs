#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

pub mod payments {
    tonic::include_proto!("types");
}

pub mod health_check {
    tonic::include_proto!("grpc.health.v1");
}

pub mod payouts {
    tonic::include_proto!("types");
}

pub mod surcharge {
    tonic::include_proto!("types");
}

#[cfg(test)]
mod tests {
    use super::payments::BraintreeConfig;

    #[test]
    fn parity_16464_braintree_config_missing_repeated_fields() {
        let json = r#"{
            "public_key": "test_pub_key",
            "private_key": "test_priv_key"
        }"#;
        let config: BraintreeConfig = serde_json::from_str(json).unwrap();
        assert!(config.apple_pay_supported_networks.is_empty());
        assert!(config.apple_pay_merchant_capabilities.is_empty());
        assert!(config.gpay_allowed_auth_methods.is_empty());
        assert!(config.gpay_allowed_card_networks.is_empty());
    }

    #[test]
    fn parity_16464_braintree_config_with_repeated_fields() {
        let json = r#"{
            "public_key": "test_pub_key",
            "private_key": "test_priv_key",
            "apple_pay_supported_networks": ["visa", "mastercard"],
            "gpay_allowed_auth_methods": ["PAN_ONLY"]
        }"#;
        let config: BraintreeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.apple_pay_supported_networks, vec!["visa", "mastercard"]);
        assert_eq!(config.gpay_allowed_auth_methods, vec!["PAN_ONLY"]);
        assert!(config.apple_pay_merchant_capabilities.is_empty());
        assert!(config.gpay_allowed_card_networks.is_empty());
    }
}
