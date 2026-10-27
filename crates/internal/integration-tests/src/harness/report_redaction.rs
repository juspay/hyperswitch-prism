//! Type-aware redaction for certification reports.
//!
//! Request and response bodies are redacted from the protobuf descriptor set,
//! so every field declared as `types.SecretString` is covered automatically.
//! No field-name or credential-pattern matching is used.

use std::{collections::HashSet, sync::OnceLock};

use prost_reflect::{DescriptorPool, FieldDescriptor, Kind, MessageDescriptor};
use serde_json::Value;

const MASKED: &str = "***MASKED***";
const SECRET_STRING: &str = "types.SecretString";
const RESPONSE_HEADERS: &str = "response_headers";
pub const REDACTED_COMMAND_MARKER: &str = "# prism-certification:redacted-v1";

static DESCRIPTORS: OnceLock<Result<DescriptorPool, String>> = OnceLock::new();

fn descriptors() -> Result<&'static DescriptorPool, String> {
    DESCRIPTORS
        .get_or_init(|| {
            DescriptorPool::decode(grpc_api_types::FILE_DESCRIPTOR_SET)
                .map_err(|error| format!("failed to decode protobuf descriptors: {error}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn method_descriptors(rpc: &str) -> Result<(MessageDescriptor, MessageDescriptor), String> {
    let rpc = rpc.trim_start_matches('/');
    let (service_name, method_name) = rpc
        .split_once('/')
        .ok_or_else(|| format!("invalid RPC method '{rpc}'"))?;
    let service = descriptors()?
        .services()
        .find(|service| service.full_name() == service_name)
        .ok_or_else(|| format!("unknown gRPC service '{service_name}'"))?;
    let method = service
        .methods()
        .find(|method| method.name() == method_name)
        .ok_or_else(|| format!("unknown gRPC method '{rpc}'"))?;
    Ok((method.input(), method.output()))
}

pub fn redact_request(rpc: &str, value: &mut Value) -> Result<(), String> {
    let (request, _) = method_descriptors(rpc)?;
    redact_message(value, &request)
}

pub fn redact_response(rpc: &str, value: &mut Value) -> Result<(), String> {
    let (_, response) = method_descriptors(rpc)?;
    redact_message(value, &response)
}

fn redact_message(value: &mut Value, descriptor: &MessageDescriptor) -> Result<(), String> {
    let object = value.as_object_mut().ok_or_else(|| {
        format!(
            "expected JSON object for protobuf message '{}'",
            descriptor.full_name()
        )
    })?;

    let mut known_keys = HashSet::new();
    for field in descriptor.fields() {
        known_keys.insert(field.name().to_string());
        known_keys.insert(field.json_name().to_string());
        for key in [field.name(), field.json_name()] {
            let Some(field_value) = object.get_mut(key) else {
                continue;
            };

            if field.name() == RESPONSE_HEADERS {
                redact_header_map(field_value);
            } else {
                redact_field(field_value, &field)?;
            }
        }
    }

    // grpcurl normally rejects unknown protobuf fields. Report inputs can also
    // come from legacy fixtures, though, so mask anything the descriptor cannot
    // classify instead of allowing an untyped value through.
    for (key, value) in object {
        if !known_keys.contains(key) {
            redact_present_leaves(value);
        }
    }
    Ok(())
}

fn redact_field(value: &mut Value, field: &FieldDescriptor) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }

    if field.is_list() {
        let values = value
            .as_array_mut()
            .ok_or_else(|| format!("protobuf field '{}' must be an array", field.full_name()))?;
        for value in values {
            redact_kind(value, field.kind())?;
        }
        return Ok(());
    }

    if field.is_map() {
        let Kind::Message(entry) = field.kind() else {
            return Err(format!(
                "map field '{}' has no entry message",
                field.full_name()
            ));
        };
        let value_field = entry
            .get_field_by_name("value")
            .ok_or_else(|| format!("map field '{}' has no value descriptor", field.full_name()))?;
        let values = value
            .as_object_mut()
            .ok_or_else(|| format!("protobuf map '{}' must be an object", field.full_name()))?;
        for value in values.values_mut() {
            redact_kind(value, value_field.kind())?;
        }
        return Ok(());
    }

    redact_kind(value, field.kind())
}

fn redact_kind(value: &mut Value, kind: Kind) -> Result<(), String> {
    let Kind::Message(message) = kind else {
        return Ok(());
    };
    if message.full_name() == SECRET_STRING {
        redact_present_leaves(value);
        Ok(())
    } else {
        redact_message(value, &message)
    }
}

/// Preserve object/array/null shape while replacing every present scalar leaf.
fn redact_present_leaves(value: &mut Value) {
    match value {
        Value::Null => {}
        Value::Object(object) => {
            for value in object.values_mut() {
                redact_present_leaves(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_present_leaves(value);
            }
        }
        scalar => *scalar = Value::String(MASKED.to_string()),
    }
}

/// Connector response headers are `map<string, string>`, so they have no
/// `SecretString` information. Mask values by default and retain only the one
/// harmless value useful for payload debugging.
fn redact_header_map(value: &mut Value) {
    let Some(headers) = value.as_object_mut() else {
        if !value.is_null() {
            *value = Value::String(MASKED.to_string());
        }
        return;
    };
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("content-type") && !value.is_null() {
            *value = Value::String(MASKED.to_string());
        }
    }
}

pub fn redact_grpc_header(header: &str) -> Result<String, String> {
    let (name, raw_value) = header
        .split_once(':')
        .ok_or_else(|| format!("invalid gRPC header '{header}'"))?;
    let name = name.trim();
    let raw_value = raw_value.trim();

    if name.eq_ignore_ascii_case("content-type") {
        return Ok(format!("{name}: {raw_value}"));
    }

    if name.eq_ignore_ascii_case("x-connector-config") {
        let Ok(mut config) = serde_json::from_str::<Value>(raw_value) else {
            return Ok(format!("{name}: {MASKED}"));
        };
        redact_present_leaves(&mut config);
        return serde_json::to_string(&config)
            .map(|config| format!("{name}: {config}"))
            .map_err(|error| format!("failed to serialize masked connector config: {error}"));
    }

    Ok(format!("{name}: {MASKED}"))
}

pub fn masked_value() -> &'static str {
    MASKED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_secret_string_from_real_authorize_descriptor() {
        let mut body = serde_json::json!({
            "state": {
                "accessToken": {
                    "token": { "value": "access-token-value" },
                    "tokenType": { "value": "Bearer" },
                    "expiresInSeconds": 60
                }
            },
            "description": "visible",
            "futureCredential": {"value": "unknown-secret"}
        });

        redact_request("types.PaymentService/Authorize", &mut body).unwrap();

        assert_eq!(
            body.pointer("/state/accessToken/token/value"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert_eq!(
            body.pointer("/state/accessToken/tokenType/value"),
            Some(&Value::String("Bearer".to_string()))
        );
        assert_eq!(
            body.get("description"),
            Some(&Value::String("visible".to_string()))
        );
        assert_eq!(
            body.pointer("/futureCredential/value"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert!(!body.to_string().contains("access-token-value"));
        assert!(!body.to_string().contains("unknown-secret"));
    }

    #[test]
    fn masks_raw_connector_fields_and_response_headers_by_type() {
        let mut body = serde_json::json!({
            "rawConnectorRequest": { "value": "raw request" },
            "rawConnectorResponse": { "value": "raw response" },
            "responseHeaders": {
                "content-type": "application/json",
                "request-id": "connector-request-id"
            }
        });

        redact_response("types.PaymentService/Authorize", &mut body).unwrap();

        assert_eq!(
            body.pointer("/rawConnectorRequest/value"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert_eq!(
            body.pointer("/rawConnectorResponse/value"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert_eq!(
            body.pointer("/responseHeaders/content-type"),
            Some(&Value::String("application/json".to_string()))
        );
        assert_eq!(
            body.pointer("/responseHeaders/request-id"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert!(!body.to_string().contains("raw request"));
        assert!(!body.to_string().contains("raw response"));
        assert!(!body.to_string().contains("connector-request-id"));
    }

    #[test]
    fn connector_config_preserves_shape_null_and_absence() {
        let header =
            r#"x-connector-config: {"config":{"Stripe":{"api_key":"secret","optional":null}}}"#;
        let masked = redact_grpc_header(header).unwrap();
        let (_, value) = masked.split_once(':').unwrap();
        let parsed: Value = serde_json::from_str(value.trim()).unwrap();

        assert_eq!(
            parsed.pointer("/config/Stripe/api_key"),
            Some(&Value::String(MASKED.to_string()))
        );
        assert_eq!(
            parsed.pointer("/config/Stripe/optional"),
            Some(&Value::Null)
        );
        assert!(parsed.pointer("/config/Stripe/absent").is_none());
        assert!(!masked.contains("secret"));
    }
}
