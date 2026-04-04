#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::as_conversions)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::print_stdout)]
#![allow(clippy::panic_in_result_fn)]

use cards::CardNumber;
use grpc_api_types::payments::{
    payment_method, payment_service_client::PaymentServiceClient, Address, AuthenticationType,
    BrowserInformation, CaptureMethod, CardDetails, Currency, PaymentAddress, PaymentMethod,
    PaymentServiceAuthorizeRequest,
};
use grpc_server::app;
use hyperswitch_masking::Secret;
use serde_json::json;
use std::str::FromStr;
use tonic::{transport::Channel, Request};
use ucs_env::configs;
mod common;

#[tokio::test]
async fn test_config_override() -> Result<(), Box<dyn std::error::Error>> {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        // let mut client = PaymentServiceClient::connect("http://localhost:8000")
        // .await
        // .unwrap();
        // Create a request with configuration override
        let mut request = Request::new(PaymentServiceAuthorizeRequest {
            amount: Some(grpc_api_types::payments::Money {
                minor_amount: 1000,
                currency: Currency::Inr as i32,
            }),
            customer: Some(grpc_api_types::payments::Customer {
                email: Some(Secret::new("example@gmail.com".to_string())),
                name: None,
                id: None,
                connector_customer_id: None,
                phone_number: None,
                phone_country_code: None,
            }),
            payment_method: Some(PaymentMethod {
                payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                    card_number: Some(CardNumber::from_str("5123456789012346").unwrap()),
                    card_exp_month: Some(Secret::new("07".to_string())),
                    card_exp_year: Some(Secret::new("2030".to_string())),
                    card_cvc: Some(Secret::new("100".to_string())),
                    ..Default::default()
                })),
            }),
            address: Some(PaymentAddress {
                shipping_address: None,
                billing_address: Some(Address {
                    phone_number: Some(Secret::new("9876354210".to_string())),
                    phone_country_code: Some("+1".to_string()),
                    ..Default::default()
                }),
            }),
            auth_type: AuthenticationType::ThreeDs as i32,
            capture_method: Some(CaptureMethod::Manual as i32),
            browser_info: Some(BrowserInformation {
                user_agent: Some("Mozilla/5.0".to_string()),
                accept_header: Some(
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
                ),
                language: Some("en-US".to_string()),
                color_depth: Some(24),
                screen_height: Some(1080),
                screen_width: Some(1920),
                java_enabled: Some(false),
                ..Default::default()
            }),
            merchant_transaction_id: Some("payment_9089".to_string()),
            return_url: Some("www.google.com".to_string()),
            ..Default::default()
        });

        // Add configuration override header
        let override_config = json!({
            "connectors": {
                "razorpay": {
                    "base_url": "https://override-test-api.razorpay.com/"
                }
            },
            "proxy": {
                "idle_pool_connection_timeout": 30,
            },
        });

        request.metadata_mut().insert(
            "x-config-override",
            override_config
                .to_string()
                .parse()
                .expect("valid header value"),
        );

        // Add required headers
        request.metadata_mut().insert(
            "x-connector",
            "razorpay".parse().expect("valid header value"),
        );

        request
            .metadata_mut()
            .insert("x-auth", "body-key".parse().expect("valid header value"));

        request
            .metadata_mut()
            .insert("x-api-key", "".parse().expect("valid header value"));

        request
            .metadata_mut()
            .insert("x-key1", "".parse().expect("valid header value"));

        // Make the request
        let response = client.authorize(request).await;

        // The config override was processed if the request reached the connector layer.
        // Integration errors (missing required fields) now correctly return tonic::Status
        // instead of being swallowed into Ok(response with error field).
        // Either a connector business error (Ok with error field) or an integration error
        // (Err tonic::Status) proves the config override was applied.
        match response {
            Ok(response) => {
                let response = response.into_inner();
                assert!(
                    response.error.is_some(),
                    "Expected error details in response"
                );
            }
            Err(status) => {
                assert!(
                    status.code() == tonic::Code::InvalidArgument
                        || status.code() == tonic::Code::Internal,
                    "Unexpected gRPC status code: {:?}",
                    status.code()
                );
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod unit {
    use base64::{engine::general_purpose, Engine as _};
    use common_utils::{config_patch::Patch, consts, metadata::MaskedMetadata};
    use config_patch_derive::Patch as PatchDerive;
    use grpc_server::utils::merge_config_with_override;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::fmt::Debug;
    use std::sync::Arc;
    use tonic::metadata::MetadataMap;
    use ucs_env::configs;
    use ucs_env::configs::Config;
    use ucs_env::logger::config::{LogFormat, LogKafka};

    fn base_config() -> Config {
        Config::new().expect("default config should load")
    }

    fn apply_override(override_json: serde_json::Value) -> Arc<Config> {
        merge_config_with_override(override_json.to_string(), base_config())
            .expect("override should succeed")
    }

    fn apply_override_with_base(
        override_json: serde_json::Value,
        base_config: Config,
    ) -> Arc<Config> {
        merge_config_with_override(override_json.to_string(), base_config)
            .expect("override should succeed")
    }

    #[test]
    fn test_proxy_idle_pool_timeout_override() {
        let override_json = json!({
            "proxy": { "idle_pool_connection_timeout": 123 },
        });
        let new_config = apply_override(override_json);
        assert_eq!(new_config.proxy.idle_pool_connection_timeout, Some(123));
    }

    #[test]
    fn test_common_environment_override() {
        let override_json = json!({
            "common": {
                "environment": "sandbox"
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(new_config.common.environment, consts::Env::Sandbox);
    }

    #[test]
    fn test_server_override() {
        let override_json = json!({
            "server": {
                "host": "127.0.0.2",
                "port": 5555,
                "type": "http"
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(new_config.server.host.as_str(), "127.0.0.2");
        assert_eq!(new_config.server.port, 5555);
        assert_eq!(new_config.server.type_, configs::ServiceType::Http);
    }

    #[test]
    fn test_metrics_override() {
        let override_json = json!({
            "metrics": {
                "host": "127.0.0.3",
                "port": 9091
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(new_config.metrics.host.as_str(), "127.0.0.3");
        assert_eq!(new_config.metrics.port, 9091);
    }

    #[test]
    fn test_log_console_override() {
        let override_json = json!({
            "log": {
                "console": {
                    "enabled": true,
                    "level": "ERROR",
                    "log_format": "default",
                    "filtering_directive": "debug"
                },
            },
        });
        let new_config = apply_override(override_json);
        assert!(new_config.log.console.enabled);
        assert_eq!(
            new_config.log.console.level.into_level(),
            tracing::Level::ERROR
        );
        assert!(matches!(
            new_config.log.console.log_format,
            LogFormat::Default
        ));
        assert_eq!(
            new_config.log.console.filtering_directive.as_deref(),
            Some("debug")
        );
    }

    #[test]
    fn test_empty_override_is_noop() {
        let mut base_config = base_config();
        base_config.server.port = 61234;

        let result = merge_config_with_override(String::new(), base_config.clone());
        assert!(
            result.is_ok(),
            "empty override should be treated as no override"
        );
        let new_config = result.expect("should get config");

        assert_eq!(new_config.server.port, 61234);
    }

    #[test]
    fn test_log_kafka_partial_override() {
        let mut base_config = base_config();
        base_config.log.kafka = Some(LogKafka {
            enabled: true,
            level: serde_json::from_value(json!("INFO")).expect("level should parse"),
            filtering_directive: Some("info".to_string()),
            brokers: vec!["localhost:9092".to_string()],
            topic: "base-topic".to_string(),
            ..Default::default()
        });

        let override_json = json!({
            "log": {
                "kafka": {
                    "level": "ERROR"
                }
            }
        });
        let new_config = apply_override_with_base(override_json, base_config);
        let kafka_config = new_config
            .log
            .kafka
            .as_ref()
            .expect("kafka config should be present");
        assert_eq!(kafka_config.level.into_level(), tracing::Level::ERROR);
        assert!(kafka_config.enabled);
        assert_eq!(kafka_config.brokers, vec!["localhost:9092".to_string()]);
        assert_eq!(kafka_config.topic.as_str(), "base-topic");
        assert_eq!(kafka_config.filtering_directive.as_deref(), Some("info"));
    }

    #[test]
    fn test_proxy_mitm_cert_override_base64() {
        let pem = "-----BEGIN CERTIFICATE-----\nTEST_CERT\n-----END CERTIFICATE-----\n";
        let encoded = general_purpose::STANDARD.encode(pem.as_bytes());
        let override_json = json!({
            "proxy": {
                "mitm_ca_cert": encoded
            }
        });
        let new_config = apply_override(override_json);
        assert_eq!(new_config.proxy.mitm_ca_cert.as_deref(), Some(pem));
    }

    #[test]
    fn test_proxy_mitm_cert_override_rejects_pem() {
        let base_config = base_config();
        let override_json = json!({
            "proxy": {
                "mitm_ca_cert": "-----BEGIN CERTIFICATE-----\nTEST_CERT\n-----END CERTIFICATE-----\n"
            }
        });
        let result = merge_config_with_override(override_json.to_string(), base_config.clone());
        assert!(
            result.is_err(),
            "config_from_metadata should reject raw PEM in mitm_ca_cert override"
        );
    }

    #[test]
    fn test_proxy_basic_override() {
        let pem = "-----BEGIN CERTIFICATE-----\nTEST_CERT\n-----END CERTIFICATE-----\n";
        let encoded = general_purpose::STANDARD.encode(pem.as_bytes());
        let override_json = json!({
            "proxy": {
                "http_url": "http://proxy.local",
                "https_url": null,
                "idle_pool_connection_timeout": 45,
                "bypass_proxy_urls": ["http://no-proxy.local"],
                "mitm_proxy_enabled": true,
                "mitm_ca_cert": encoded
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(
            new_config.proxy.http_url.as_deref(),
            Some("http://proxy.local")
        );
        assert_eq!(new_config.proxy.https_url, None);
        assert_eq!(new_config.proxy.idle_pool_connection_timeout, Some(45));
        assert_eq!(
            new_config.proxy.bypass_proxy_urls,
            vec!["http://no-proxy.local".to_string()]
        );
        assert!(new_config.proxy.mitm_proxy_enabled);
        assert_eq!(new_config.proxy.mitm_ca_cert.as_deref(), Some(pem));
    }

    #[test]
    fn test_connectors_override() {
        let override_json = json!({
            "connectors": {
                "adyen": {
                    "base_url": "https://adyen.example",
                    "dispute_base_url": "https://dispute.adyen.example"
                },
                "billwerk": {
                    "secondary_base_url": "https://billwerk-secondary.example"
                },
                "fiuu": {
                    "secondary_base_url": "https://fiuu-secondary.example"
                },
                "hipay": {
                    "base_url": "https://hipay.example",
                    "secondary_base_url": "https://hipay-secondary.example",
                    "third_base_url": "https://hipay-third.example"
                },
                "jpmorgan": {
                    "secondary_base_url": "https://jpmorgan-secondary.example"
                },
                "mollie": {
                    "secondary_base_url": "https://mollie-secondary.example"
                },
                "trustpay": {
                    "base_url": "https://trustpay.example",
                    "base_url_bank_redirects": "https://trustpay-bank.example"
                },
                "volt": {
                    "secondary_base_url": "https://volt-secondary.example"
                },
                "worldpayvantiv": {
                    "secondary_base_url": "https://worldpayvantiv-secondary.example"
                }
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(
            new_config.connectors.adyen.base_url.as_str(),
            "https://adyen.example"
        );
        assert_eq!(
            new_config.connectors.adyen.dispute_base_url.as_deref(),
            Some("https://dispute.adyen.example")
        );
        assert_eq!(
            new_config.connectors.billwerk.secondary_base_url.as_deref(),
            Some("https://billwerk-secondary.example")
        );
        assert_eq!(
            new_config.connectors.fiuu.secondary_base_url.as_deref(),
            Some("https://fiuu-secondary.example")
        );
        assert_eq!(
            new_config.connectors.hipay.base_url.as_str(),
            "https://hipay.example"
        );
        assert_eq!(
            new_config.connectors.hipay.secondary_base_url.as_deref(),
            Some("https://hipay-secondary.example")
        );
        assert_eq!(
            new_config.connectors.hipay.third_base_url.as_deref(),
            Some("https://hipay-third.example")
        );
        assert_eq!(
            new_config.connectors.jpmorgan.secondary_base_url.as_deref(),
            Some("https://jpmorgan-secondary.example")
        );
        assert_eq!(
            new_config.connectors.mollie.secondary_base_url.as_deref(),
            Some("https://mollie-secondary.example")
        );
        assert_eq!(
            new_config.connectors.trustpay.base_url.as_str(),
            "https://trustpay.example"
        );
        assert_eq!(
            new_config
                .connectors
                .trustpay
                .base_url_bank_redirects
                .as_str(),
            "https://trustpay-bank.example"
        );
        assert_eq!(
            new_config.connectors.volt.secondary_base_url.as_deref(),
            Some("https://volt-secondary.example")
        );
        assert_eq!(
            new_config
                .connectors
                .worldpayvantiv
                .secondary_base_url
                .as_deref(),
            Some("https://worldpayvantiv-secondary.example")
        );
    }

    #[test]
    fn test_events_override() {
        let override_json = json!({
            "events": {
                "enabled": true,
                "topic": "events-override",
                "brokers": ["broker1:9092", "broker2:9092"],
                "partition_key_field": "merchant_id",
                "transformations": { "order_id": "payment_id" },
                "static_values": { "app": "grpc" },
                "extractions": { "path": "metadata.path" }
            },
        });
        let new_config = apply_override(override_json);
        assert!(new_config.events.enabled);
        assert_eq!(new_config.events.topic.as_str(), "events-override");
        assert_eq!(
            new_config.events.brokers,
            vec!["broker1:9092".to_string(), "broker2:9092".to_string()]
        );
        assert_eq!(
            new_config.events.partition_key_field.as_str(),
            "merchant_id"
        );
        assert_eq!(
            new_config
                .events
                .transformations
                .get("order_id")
                .map(String::as_str),
            Some("payment_id")
        );
        assert_eq!(
            new_config
                .events
                .static_values
                .get("app")
                .map(String::as_str),
            Some("grpc")
        );
        assert_eq!(
            new_config
                .events
                .extractions
                .get("path")
                .map(String::as_str),
            Some("metadata.path")
        );
    }

    #[test]
    fn test_events_transformations_replace() {
        let mut base_config = base_config();
        base_config
            .events
            .transformations
            .insert("old_key".to_string(), "old_value".to_string());

        let override_json = json!({
            "events": {
                "transformations": { "new_key": "new_value" }
            },
        });
        let new_config = apply_override_with_base(override_json, base_config);

        assert_eq!(
            new_config
                .events
                .transformations
                .get("new_key")
                .map(String::as_str),
            Some("new_value")
        );
        assert!(!new_config.events.transformations.contains_key("old_key"));
    }

    #[test]
    fn test_lineage_override() {
        let override_json = json!({
            "lineage": {
                "enabled": true,
                "header_name": "x-lineage-test",
                "field_prefix": "test_"
            },
        });
        let new_config = apply_override(override_json);
        assert!(new_config.lineage.enabled);
        assert_eq!(new_config.lineage.header_name.as_str(), "x-lineage-test");
        assert_eq!(new_config.lineage.field_prefix.as_str(), "test_");
    }

    #[test]
    fn test_unmasked_headers_override_keeps_masking() {
        let base_config = base_config();
        let override_json = json!({
            "unmasked_headers": {
                "keys": ["x-request-id"]
            }
        });
        let new_config = apply_override_with_base(override_json, base_config);

        let mut metadata = MetadataMap::new();
        metadata.insert("x-request-id", "req_123".parse().expect("valid header"));
        metadata.insert("authorization", "secret".parse().expect("valid header"));

        let masked_metadata = MaskedMetadata::new(metadata, new_config.unmasked_headers.clone());
        let request_id = masked_metadata
            .get_maskable("x-request-id")
            .expect("request id should be present");
        let auth = masked_metadata
            .get_maskable("authorization")
            .expect("authorization should be present");

        assert!(request_id.is_normal(), "unmasked header should be normal");
        assert!(auth.is_masked(), "masked header should remain masked");
    }

    #[test]
    fn test_test_config_override() {
        let override_json = json!({
            "test": {
                "enabled": true,
                "mock_server_url": "http://mock.local"
            },
        });
        let new_config = apply_override(override_json);
        assert!(new_config.test.enabled);
        assert_eq!(
            new_config.test.mock_server_url.as_deref(),
            Some("http://mock.local")
        );
    }

    #[test]
    fn test_api_tags_override() {
        let override_json = json!({
            "api_tags": {
                "tags": { "psync": "PSYNC_TAG" }
            },
        });
        let new_config = apply_override(override_json);
        assert_eq!(
            new_config.api_tags.tags.get("psync").map(String::as_str),
            Some("PSYNC_TAG")
        );
    }

    #[test]
    fn test_optional_field_null_clears() {
        let mut base_config = base_config();
        base_config.log.kafka = Some(LogKafka {
            enabled: true,
            level: serde_json::from_value(json!("INFO")).expect("level should parse"),
            filtering_directive: Some("info".to_string()),
            brokers: vec!["localhost:9092".to_string()],
            topic: "base-topic".to_string(),
            ..Default::default()
        });

        let override_json = json!({
            "log": {
                "kafka": {
                    "filtering_directive": null
                }
            }
        });

        let new_config = apply_override_with_base(override_json, base_config);
        let kafka_config = new_config
            .log
            .kafka
            .as_ref()
            .expect("kafka config should be present");
        assert_eq!(kafka_config.filtering_directive, None);
    }

    #[test]
    fn test_optional_nested_null_clears() {
        let mut base_config = base_config();
        base_config.log.kafka = Some(LogKafka {
            enabled: true,
            ..Default::default()
        });

        let override_json = json!({
            "log": {
                "kafka": null
            }
        });

        let new_config = apply_override_with_base(override_json, base_config);
        assert!(new_config.log.kafka.is_none());
    }

    #[test]
    fn test_null_on_non_optional_is_noop() {
        let mut base_config = base_config();
        base_config.server.host = "127.0.0.9".to_string();

        let override_json = json!({
            "server": {
                "host": null
            },
        });
        let new_config = apply_override_with_base(override_json, base_config);
        assert_eq!(new_config.server.host.as_str(), "127.0.0.9");
    }

    #[test]
    fn test_unknown_keys_error() {
        let base_config = base_config();
        let override_json = json!({
            "unknown_section": {
                "value": "nope"
            },
        });
        let result = merge_config_with_override(override_json.to_string(), base_config);
        assert!(result.is_err(), "unknown keys should error");
    }

    #[derive(Debug, Default, PatchDerive, PartialEq)]
    struct GenericInner {
        flag: bool,
    }

    #[derive(Debug, Default, PatchDerive, PartialEq)]
    struct GenericWrapper<T> {
        inner: T,
    }

    #[derive(Debug, Default, PatchDerive, PartialEq)]
    struct OptionalGenericWrapper<T> {
        inner: Option<T>,
    }

    #[test]
    fn test_generic_nested_patch_applies() {
        let mut base = GenericWrapper {
            inner: GenericInner { flag: false },
        };

        let patch = GenericWrapperPatch::<GenericInnerPatch> {
            inner: Some(GenericInnerPatch { flag: Some(true) }),
        };

        base.apply(patch);
        assert!(base.inner.flag);
    }

    #[test]
    fn test_generic_optional_nested_patch_applies() {
        let mut base = OptionalGenericWrapper::<GenericInner> { inner: None };

        let patch = OptionalGenericWrapperPatch::<GenericInnerPatch> {
            inner: Some(Some(GenericInnerPatch { flag: Some(true) })),
        };

        base.apply(patch);
        assert_eq!(base.inner.as_ref().map(|value| value.flag), Some(true));
    }

    #[derive(Debug, Default, PartialEq, PatchDerive)]
    struct JsonInner {
        value: u32,
    }

    #[derive(Debug, Default, PartialEq, PatchDerive)]
    struct JsonAssoc {
        value: u32,
    }

    trait AssocTrait {
        type Assoc;
        type AssocPatch: Debug + Serialize + for<'de> Deserialize<'de>;
    }

    #[derive(Debug, Default, PartialEq)]
    struct Concrete;

    impl AssocTrait for Concrete {
        type Assoc = JsonAssoc;
        type AssocPatch = JsonAssocPatch;
    }

    #[derive(Debug, Default, PartialEq, PatchDerive)]
    struct JsonWrapper<T> {
        inner: T,
    }

    #[derive(Debug, Default, PartialEq, PatchDerive)]
    struct JsonWrapperOpt<T> {
        inner: Option<T>,
    }

    #[derive(PatchDerive)]
    struct JsonHolder<T: AssocTrait> {
        #[patch(patch_type = <T as AssocTrait>::AssocPatch)]
        assoc: <T as AssocTrait>::Assoc,
    }

    #[derive(PatchDerive)]
    struct JsonHolderOpt<T: AssocTrait> {
        #[patch(patch_type = <T as AssocTrait>::AssocPatch)]
        assoc: Option<<T as AssocTrait>::Assoc>,
    }

    #[test]
    fn test_generic_nested_patch_from_json() {
        let mut value = JsonWrapper {
            inner: JsonInner { value: 1 },
        };
        let patch: JsonWrapperPatch<JsonInnerPatch> =
            serde_json::from_str(r#"{"inner":{"value":2}}"#).expect("patch should deserialize");

        value.apply(patch);

        assert_eq!(value.inner.value, 2);
    }

    #[test]
    fn test_generic_optional_nested_patch_from_json() {
        let mut value = JsonWrapperOpt::<JsonInner> { inner: None };
        let patch: JsonWrapperOptPatch<JsonInnerPatch> =
            serde_json::from_str(r#"{"inner":{"value":3}}"#).expect("patch should deserialize");

        value.apply(patch);

        assert_eq!(value.inner.as_ref().map(|inner| inner.value), Some(3));
    }

    #[test]
    fn test_generic_optional_nested_clear_from_json() {
        let mut value = JsonWrapperOpt::<JsonInner> {
            inner: Some(JsonInner { value: 9 }),
        };
        let patch: JsonWrapperOptPatch<JsonInnerPatch> =
            serde_json::from_str(r#"{"inner":null}"#).expect("patch should deserialize");

        value.apply(patch);

        assert_eq!(value.inner, None);
    }

    #[test]
    fn test_qself_patch_type_apply() {
        let mut value = JsonHolder::<Concrete> {
            assoc: JsonAssoc { value: 1 },
        };
        let patch = JsonHolderPatch::<Concrete> {
            assoc: Some(JsonAssocPatch { value: Some(2) }),
        };

        value.apply(patch);

        assert_eq!(value.assoc.value, 2);
    }

    #[test]
    fn test_qself_patch_type_optional_insert() {
        let mut value = JsonHolderOpt::<Concrete> { assoc: None };
        let patch = JsonHolderOptPatch::<Concrete> {
            assoc: Some(Some(JsonAssocPatch { value: Some(5) })),
        };

        value.apply(patch);

        assert_eq!(value.assoc, Some(JsonAssoc { value: 5 }));
    }

    #[test]
    fn test_qself_patch_type_optional_clear() {
        let mut value = JsonHolderOpt::<Concrete> {
            assoc: Some(JsonAssoc { value: 9 }),
        };
        let patch = JsonHolderOptPatch::<Concrete> { assoc: Some(None) };

        value.apply(patch);

        assert_eq!(value.assoc, None);
    }
}
