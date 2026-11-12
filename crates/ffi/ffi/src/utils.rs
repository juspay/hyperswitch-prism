use common_utils::metadata::{HeaderMaskingConfig, MaskedMetadata};
use domain_types::errors::{IntegrationError as DomainIntegrationError, IntegrationErrorContext};
use grpc_api_types::payments::IntegrationError;
use std::collections::HashMap;
use std::sync::Arc;
use ucs_env::configs::Config;

/// Converts FFI headers (HashMap) to gRPC metadata with masking support.
/// Delegates to the shared `headers_to_masked_metadata` implementation.
pub fn ffi_headers_to_masked_metadata(
    headers: &HashMap<String, String>,
) -> Result<MaskedMetadata, IntegrationError> {
    ucs_interface_common::headers::headers_to_masked_metadata(
        headers,
        HeaderMaskingConfig::default(),
    )
    .map_err(|e| match e {
        ucs_interface_common::error::InterfaceError::MissingRequiredHeader { key } => {
            IntegrationError {
                error_message: format!("Missing required header: {}", key),
                error_code: "MISSING_REQUIRED_HEADER".to_string(),
                suggested_action: None,
                doc_url: None,
            }
        }
        ucs_interface_common::error::InterfaceError::InvalidHeaderValue { key, reason } => {
            IntegrationError {
                error_message: format!("{}: {}", key, reason),
                error_code: "INVALID_HEADER_VALUE".to_string(),
                suggested_action: None,
                doc_url: None,
            }
        }
    })
}

/// Load development config from the embedded config string.
/// This avoids runtime path lookup by embedding the config at build time.
#[allow(clippy::result_large_err)]
pub fn load_config(embedded_config: &str) -> Result<Arc<Config>, DomainIntegrationError> {
    let config: Config =
        toml::from_str(embedded_config).map_err(|e| DomainIntegrationError::InvalidDataFormat {
            field_name: "config",
            context: IntegrationErrorContext {
                additional_context: Some(e.to_string()),
                ..Default::default()
            },
        })?;
    connector_integration::webhook_dispatch::validate_webhook_migration_config(
        config.webhook_parse.migrated_connectors.as_ref(),
    )
    .map_err(|error| DomainIntegrationError::InvalidDataFormat {
        field_name: "webhook_parse.migrated_connectors",
        context: IntegrationErrorContext {
            additional_context: Some(error.to_string()),
            ..Default::default()
        },
    })?;
    Ok(Arc::new(config))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    #[test]
    fn embedded_development_and_production_config_keep_their_settings() {
        for source in [
            crate::handlers::payments::EMBEDDED_DEVELOPMENT_CONFIG,
            crate::handlers::payments::EMBEDDED_PROD_CONFIG,
        ] {
            let expected: ucs_env::configs::Config = toml::from_str(source).unwrap();
            let actual = super::load_config(source).unwrap();
            assert_eq!(expected.common.environment, actual.common.environment);
            assert_eq!(expected.server.host, actual.server.host);
            assert_eq!(expected.server.port, actual.server.port);
            assert_eq!(
                serde_json::to_value(&expected.connectors).unwrap(),
                serde_json::to_value(&actual.connectors).unwrap()
            );
            assert_eq!(expected.webhook_parse, actual.webhook_parse);
            assert_eq!(
                expected
                    .webhook_source_verification_call
                    .connectors_with_webhook_source_verification_call,
                actual
                    .webhook_source_verification_call
                    .connectors_with_webhook_source_verification_call
            );
            assert_eq!(expected.unmasked_headers, actual.unmasked_headers);
        }
    }
    #[cfg(feature = "uniffi")]
    #[test]
    fn native_byte_api_preserves_legacy_payout_handle_event() {
        use grpc_api_types::payments::{self as proto, connector_specific_config, ffi_result};
        use prost::Message;
        for environment in [proto::Environment::Sandbox, proto::Environment::Production] {
            let options = proto::FfiOptions {
                environment: environment.into(),
                connector_config: Some(proto::ConnectorSpecificConfig {
                    config: Some(connector_specific_config::Config::Truelayer(
                        proto::TruelayerConfig::default(),
                    )),
                }),
            };
            for (event, expected) in [
                ("payout_executed", proto::WebhookEventType::PayoutSuccess),
                ("payout_failed", proto::WebhookEventType::PayoutFailure),
            ] {
                let request=proto::EventServiceHandleRequest {
                    merchant_event_id: Some("native_roundtrip".into()),
                    request_details: Some(proto::RequestDetails {
                        method: 2,
                        body: serde_json::to_vec(&serde_json::json!({"type":event,"event_version":1,"event_id":"evt_native","payout_id":"po_native"})).unwrap(),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let bytes = crate::bindings::uniffi::handle_event_transformer(
                    request.encode_to_vec(),
                    options.encode_to_vec(),
                );
                let envelope = proto::FfiResult::decode(bytes.as_slice()).unwrap();
                let Some(ffi_result::Payload::ProtoResponse(bytes)) = envelope.payload else {
                    panic!("native webhook returned an error");
                };
                let response = proto::EventServiceHandleResponse::decode(bytes.as_slice()).unwrap();
                assert_eq!(response.event_type, i32::from(expected));
                assert_eq!(
                    response.merchant_event_id.as_deref(),
                    Some("native_roundtrip")
                );
                assert!(!response.source_verified);
                assert_eq!(response.event_ack_response.unwrap().status_code, 200);
                assert!(matches!(
                    response.event_content.unwrap().content,
                    Some(proto::event_content::Content::PayoutsResponse(_))
                ));
            }
        }
    }
}
