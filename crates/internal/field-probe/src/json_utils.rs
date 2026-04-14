/// Convert PascalCase to snake_case
pub(crate) fn pascal_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

/// Convert Rust serde JSON format to proper proto JSON format.
///
/// Transformations:
/// - oneof variant names: "ApplePay" → "apple_pay" (snake_case)
/// - Nested oneof: {"payment_method": {"ApplePay": {...}}} → {"payment_method": {"apple_pay": {...}}}
pub(crate) fn convert_rust_to_proto_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                // Check if this is a oneof wrapper: key is PascalCase and value is a single-entry object
                if let serde_json::Value::Object(inner_map) = val {
                    if inner_map.len() == 1 {
                        let inner_key = inner_map.keys().next().unwrap();
                        // If inner key starts with uppercase and isn't all uppercase, it's a oneof variant
                        if inner_key
                            .chars()
                            .next()
                            .map(|c| c.is_uppercase())
                            .unwrap_or(false)
                            && !inner_key.chars().all(|c| c.is_uppercase() || c == '_')
                        {
                            let snake_key = pascal_to_snake(inner_key);
                            let converted_inner =
                                convert_rust_to_proto_json(inner_map.values().next().unwrap());
                            result.insert(
                                key.clone(),
                                serde_json::Value::Object({
                                    let mut m = serde_json::Map::new();
                                    m.insert(snake_key, converted_inner);
                                    m
                                }),
                            );
                            continue;
                        }
                    }
                }
                result.insert(key.clone(), convert_rust_to_proto_json(val));
            }
            serde_json::Value::Object(result)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(convert_rust_to_proto_json).collect())
        }
        other => other.clone(),
    }
}

/// Keys that are probe-internal and should be removed from the output
pub(crate) const PROBE_INTERNAL_KEYS: &[&str] = &["connector_feature_data"];

/// Check if a string value is a proto3 default enum value
pub(crate) fn is_default_enum(value: &str) -> bool {
    value.ends_with("_UNSPECIFIED") || value.ends_with("_UNKNOWN")
}

/// Flatten proto3 oneof wrappers that serde adds as an extra nesting level.
///
/// Prost generates oneof fields as `Option<Enum>` stored under a field with the
/// same name as the oneof itself. When serde serializes, we get:
///   {"payment_method": {"payment_method": {"card": {...}}}}
/// In proto3 JSON the oneof variant is inlined:
///   {"payment_method": {"card": {...}}}
pub(crate) fn flatten_oneof_wrappers(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                let v = flatten_oneof_wrappers(v);
                // Collapse the oneof wrapper: {"k": {"k": inner}} → {"k": inner}
                // Only when `inner` is itself an object — scalar inner values (e.g.
                // TokenPaymentMethodType where field name == parent field name) must
                // NOT be collapsed, or we lose the message nesting.
                if let serde_json::Value::Object(inner_map) = &v {
                    if inner_map.len() == 1
                        && inner_map.keys().next().map(|ik| ik == k).unwrap_or(false)
                    {
                        let inner_value = inner_map.values().next().unwrap();
                        if inner_value.is_object() || inner_value.is_array() {
                            result.insert(k.clone(), flatten_oneof_wrappers(inner_value));
                            continue;
                        }
                    }
                }
                result.insert(k.clone(), v);
            }
            serde_json::Value::Object(result)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(flatten_oneof_wrappers).collect())
        }
        other => other.clone(),
    }
}

/// Clean a proto_request for documentation output:
///   1. Remove probe-internal keys (connector_feature_data, etc.)
///   2. Remove null values and empty arrays
///   3. Remove proto3 default enum values (*_UNSPECIFIED / *_UNKNOWN)
///   4. Collapse proto3 oneof wrappers
///   5. Remove empty objects (e.g. `"billing_address": {}` — artifact of domain-layer gate)
pub(crate) fn clean_proto_request(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                // Skip probe-internal keys
                if PROBE_INTERNAL_KEYS.contains(&k.as_str()) {
                    continue;
                }
                // Skip null values
                if v.is_null() {
                    continue;
                }
                // Skip empty arrays
                if let serde_json::Value::Array(arr) = v {
                    if arr.is_empty() {
                        continue;
                    }
                }
                // Skip default enum values
                if let serde_json::Value::String(s) = v {
                    if is_default_enum(s) {
                        continue;
                    }
                }
                result.insert(k.clone(), clean_proto_request(v));
            }
            flatten_oneof_wrappers(&serde_json::Value::Object(result))
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(clean_proto_request).collect())
        }
        other => other.clone(),
    }
}
