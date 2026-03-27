use proptest::{
    prelude::*,
    sample,
    strategy::{Strategy, ValueTree},
    test_runner::TestRunner,
};
use serde_json::Value;
use uuid::Uuid;

use crate::harness::scenario_types::ScenarioError;

/// Replaces `auto_generate` sentinel placeholders in a request payload.
///
/// Context-deferred fields are intentionally skipped here because those are
/// expected to be copied from dependency responses later in the pipeline.
pub fn resolve_auto_generate(current_grpc_req: &mut Value) -> Result<(), ScenarioError> {
    let mut paths = Vec::new();
    collect_leaf_paths(current_grpc_req, String::new(), &mut paths);

    let mut runner = TestRunner::default();
    for path in paths {
        let lower_path = path.to_ascii_lowercase();
        let should_generate = lookup_json_path(current_grpc_req, &path)
            .map(is_auto_generate_sentinel)
            .unwrap_or(false);
        if !should_generate {
            continue;
        }

        // Context-carried fields should not be synthesized.
        // They are expected to be populated from dependency responses and
        // pruned if still unresolved before execution.
        if is_context_deferred_path(&lower_path) {
            continue;
        }

        let generated = generate_value_for_path(&path, &mut runner)?;
        let _ = set_json_path_value(current_grpc_req, &path, Value::String(generated));
    }

    Ok(())
}

/// Generates a deterministic-by-shape value for a specific JSON path.
fn generate_value_for_path(path: &str, runner: &mut TestRunner) -> Result<String, ScenarioError> {
    let lower = path.to_ascii_lowercase();

    if lower == "refund_id" {
        return Ok(prefixed_uuid("rfi"));
    }

    if lower.ends_with("connector_mandate_id.connector_mandate_id") {
        return Ok(prefixed_uuid("cmi"));
    }

    if let Some(prefix) = id_prefix_for_leaf_path(&lower) {
        return Ok(prefixed_uuid(prefix));
    }

    if lower.ends_with(".id") {
        return Ok(prefixed_uuid(id_prefix_for_path(&lower)));
    }

    if lower.ends_with("phone_number.value") {
        return sample_string(path, local_phone_strategy(), runner);
    }

    if lower.ends_with("phone_number") {
        return sample_string(path, international_phone_strategy(), runner);
    }

    if lower.ends_with("email.value") {
        return sample_string(path, email_strategy(), runner);
    }

    if lower.ends_with("first_name.value") {
        return sample_string(path, first_name_strategy(), runner);
    }

    if lower.ends_with("last_name.value") {
        return sample_string(path, last_name_strategy(), runner);
    }

    if lower.ends_with("customer_name") || lower.ends_with(".name") {
        return sample_string(path, full_name_strategy(), runner);
    }

    if lower.ends_with("card_holder_name.value") {
        return sample_string(path, full_name_strategy(), runner);
    }

    if lower.ends_with("line1.value")
        || lower.ends_with("line2.value")
        || lower.ends_with("line3.value")
    {
        return sample_string(path, address_line_strategy(), runner);
    }

    if lower.ends_with("city.value") {
        return sample_string(path, city_strategy(), runner);
    }

    if lower.ends_with("zip_code.value") {
        return sample_string(path, zip_code_strategy(), runner);
    }

    sample_string(path, generic_string_strategy(), runner)
}

/// Returns semantic ID prefixes so generated identifiers are easier to debug.
fn id_prefix_for_path(path: &str) -> &'static str {
    let Some((left, _)) = path.rsplit_once('.') else {
        return "id";
    };
    let parent = left.rsplit('.').next().unwrap_or(left);
    match parent {
        "merchant_transaction_id" => "mti",
        "merchant_refund_id" => "mri",
        "merchant_capture_id" => "mci",
        "merchant_void_id" => "mvi",
        "merchant_charge_id" => "mchi",
        "merchant_recurring_payment_id" => "mrpi",
        "merchant_access_token_id" => "mati",
        "merchant_customer_id" => "mcui",
        "connector_transaction_id" => "cti",
        "customer" => "cust",
        _ => "id",
    }
}

fn id_prefix_for_leaf_path(path: &str) -> Option<&'static str> {
    let field = path.rsplit('.').next().unwrap_or(path);
    let prefix = match field {
        "merchant_transaction_id" => "mti",
        "merchant_refund_id" => "mri",
        "merchant_capture_id" => "mci",
        "merchant_void_id" => "mvi",
        "merchant_charge_id" => "mchi",
        "merchant_recurring_payment_id" => "mrpi",
        "merchant_access_token_id" => "mati",
        "merchant_customer_id" => "mcui",
        "connector_transaction_id" => "cti",
        _ => return None,
    };
    Some(prefix)
}

/// Samples one string value from a proptest strategy and wraps any generation
/// failure into harness-level errors.
fn sample_string<S>(
    path: &str,
    strategy: S,
    runner: &mut TestRunner,
) -> Result<String, ScenarioError>
where
    S: Strategy<Value = String>,
{
    let tree = strategy
        .new_tree(runner)
        .map_err(|error| ScenarioError::GrpcurlExecution {
            message: format!("auto-generate failed for '{path}': {error}"),
        })?;
    Ok(tree.current())
}

/// Generates `<prefix>_<short-uuid>` helper identifiers.
///
/// Uses the first 24 hex characters of a UUIDv4 (96 bits of entropy) to keep
/// the total length ≤ 28 characters (e.g. `mti_<24hex>`). Some connectors such
/// as TrustPay (max 35) and Nexinets (max 30) reject longer merchant reference
/// values.
fn prefixed_uuid(prefix: &str) -> String {
    let full = Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &full[..24])
}

fn email_strategy() -> BoxedStrategy<String> {
    (
        sample::select(vec!["alex", "riley", "sam", "jordan", "morgan", "casey"]),
        1000u16..=9999u16,
        sample::select(vec!["example.com", "sandbox.example.com", "testmail.io"]),
    )
        .prop_map(|(local, suffix, domain)| format!("{local}.{suffix}@{domain}"))
        .boxed()
}

fn international_phone_strategy() -> BoxedStrategy<String> {
    (
        sample::select(vec!["+1", "+44", "+91"]),
        1_000_000_000u64..=9_999_999_999u64,
    )
        .prop_map(|(country_code, number)| format!("{country_code}{number}"))
        .boxed()
}

fn local_phone_strategy() -> BoxedStrategy<String> {
    (1_000_000_000u64..=9_999_999_999u64)
        .prop_map(|number| number.to_string())
        .boxed()
}

fn first_name_strategy() -> BoxedStrategy<String> {
    sample::select(vec!["Ava", "Liam", "Emma", "Noah", "Mia", "Ethan"])
        .prop_map(str::to_string)
        .boxed()
}

fn last_name_strategy() -> BoxedStrategy<String> {
    sample::select(vec![
        "Smith", "Johnson", "Brown", "Taylor", "Wilson", "Miller",
    ])
    .prop_map(str::to_string)
    .boxed()
}

fn full_name_strategy() -> BoxedStrategy<String> {
    (first_name_strategy(), last_name_strategy())
        .prop_map(|(first, last)| format!("{first} {last}"))
        .boxed()
}

fn address_line_strategy() -> BoxedStrategy<String> {
    (
        1u16..=9999u16,
        sample::select(vec!["Main", "Oak", "Pine", "Market", "Lake", "Sunset"]),
        sample::select(vec!["St", "Ave", "Blvd", "Rd", "Ln", "Dr"]),
    )
        .prop_map(|(num, street, suffix)| format!("{num} {street} {suffix}"))
        .boxed()
}

fn city_strategy() -> BoxedStrategy<String> {
    sample::select(vec![
        "San Francisco",
        "New York",
        "Los Angeles",
        "Chicago",
        "Seattle",
        "Austin",
    ])
    .prop_map(str::to_string)
    .boxed()
}

fn zip_code_strategy() -> BoxedStrategy<String> {
    (10_000u32..=99_999u32)
        .prop_map(|zip| zip.to_string())
        .boxed()
}

fn generic_string_strategy() -> BoxedStrategy<String> {
    (100_000u32..=999_999u32)
        .prop_map(|n| format!("gen_{n}"))
        .boxed()
}

fn is_auto_generate_sentinel(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    text.to_ascii_lowercase().contains("auto_generate")
}

fn is_context_deferred_path(path: &str) -> bool {
    matches!(
        path,
        "customer.connector_customer_id"
            | "state.connector_customer_id"
            | "state.access_token.token.value"
            | "state.access_token.token_type"
            | "state.access_token.expires_in_seconds"
            | "connector_feature_data"
            | "connector_feature_data.value"
            | "connector_transaction_id"
            | "connector_transaction_id.id"
            | "refund_id"
    )
}

fn collect_leaf_paths(value: &Value, current: String, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if current.is_empty() {
                    key.to_string()
                } else {
                    format!("{current}.{key}")
                };
                collect_leaf_paths(child, next, paths);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let next = if current.is_empty() {
                    index.to_string()
                } else {
                    format!("{current}.{index}")
                };
                collect_leaf_paths(child, next, paths);
            }
        }
        _ => paths.push(current),
    }
}

fn lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }

    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }

        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)?
        } else {
            current.get(segment)?
        };
    }

    Some(current)
}

fn set_json_path_value(root: &mut Value, path: &str, value: Value) -> bool {
    let mut segments = path.split('.').peekable();
    let mut current = root;

    while let Some(segment) = segments.next() {
        let is_last = segments.peek().is_none();

        if let Ok(index) = segment.parse::<usize>() {
            let Some(items) = current.as_array_mut() else {
                return false;
            };
            let Some(next) = items.get_mut(index) else {
                return false;
            };
            if is_last {
                *next = value;
                return true;
            }
            current = next;
            continue;
        }

        let Some(map) = current.as_object_mut() else {
            return false;
        };

        if is_last {
            if map.contains_key(segment) {
                map.insert(segment.to_string(), value);
                return true;
            }
            return false;
        }

        let Some(next) = map.get_mut(segment) else {
            return false;
        };
        current = next;
    }

    false
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use serde_json::json;

    use super::{
        id_prefix_for_leaf_path, id_prefix_for_path, is_auto_generate_sentinel,
        is_context_deferred_path, resolve_auto_generate,
    };

    #[test]
    fn sentinel_detection_supports_auto_generate_variants() {
        assert!(is_auto_generate_sentinel(&json!("auto_generate")));
        assert!(is_auto_generate_sentinel(&json!("cust_auto_generate")));
        assert!(!is_auto_generate_sentinel(&json!("fixed_value")));
    }

    #[test]
    fn id_prefix_mapping_uses_expected_prefixes() {
        assert_eq!(id_prefix_for_path("merchant_transaction_id.id"), "mti");
        assert_eq!(id_prefix_for_path("merchant_refund_id.id"), "mri");
        assert_eq!(id_prefix_for_path("merchant_customer_id.id"), "mcui");
        assert_eq!(id_prefix_for_path("unknown.id"), "id");
        assert_eq!(
            id_prefix_for_leaf_path("merchant_transaction_id"),
            Some("mti")
        );
        assert_eq!(id_prefix_for_leaf_path("merchant_capture_id"), Some("mci"));
        assert_eq!(id_prefix_for_leaf_path("unknown"), None);
    }

    #[test]
    fn resolves_auto_generate_placeholders_in_request_payload() {
        let mut req = json!({
            "merchant_transaction_id": "auto_generate",
            "customer": {
                "name": "auto_generate",
                "email": {"value": "auto_generate"},
                "phone_number": "auto_generate"
            },
            "address": {
                "shipping_address": {
                    "first_name": {"value": "auto_generate"},
                    "last_name": {"value": "auto_generate"},
                    "line1": {"value": "auto_generate"},
                    "city": {"value": "auto_generate"},
                    "zip_code": {"value": "auto_generate"},
                    "phone_number": {"value": "auto_generate"}
                }
            },
            "payment_method": {
                "card": {
                    "card_holder_name": {"value": "auto_generate"},
                    "card_number": {"value": "4111111111111111"}
                }
            }
        });

        resolve_auto_generate(&mut req).expect("auto generation should succeed");

        let generated_id = req["merchant_transaction_id"]
            .as_str()
            .expect("id should be string");
        assert!(generated_id.starts_with("mti_"));

        assert_ne!(req["customer"]["name"], json!("auto_generate"));
        assert_ne!(req["customer"]["email"]["value"], json!("auto_generate"));
        assert_ne!(req["customer"]["phone_number"], json!("auto_generate"));
        assert_ne!(
            req["address"]["shipping_address"]["first_name"]["value"],
            json!("auto_generate")
        );
        assert_ne!(
            req["address"]["shipping_address"]["phone_number"]["value"],
            json!("auto_generate")
        );
        assert_ne!(
            req["payment_method"]["card"]["card_holder_name"]["value"],
            json!("auto_generate")
        );
        assert_eq!(
            req["payment_method"]["card"]["card_number"]["value"],
            json!("4111111111111111")
        );
    }

    #[test]
    fn keeps_context_deferred_fields_unresolved() {
        let mut req = json!({
            "customer": { "connector_customer_id": "auto_generate" },
            "state": {
                "connector_customer_id": "auto_generate",
                "access_token": {
                    "token": { "value": "auto_generate" },
                    "token_type": "auto_generate",
                    "expires_in_seconds": "auto_generate"
                }
            },
            "connector_feature_data": { "value": "auto_generate" },
            "connector_transaction_id": "auto_generate",
            "refund_id": "auto_generate",
            "merchant_transaction_id": "auto_generate"
        });

        resolve_auto_generate(&mut req).expect("auto generation should succeed");

        assert_eq!(
            req["customer"]["connector_customer_id"],
            json!("auto_generate")
        );
        assert_eq!(
            req["state"]["connector_customer_id"],
            json!("auto_generate")
        );
        assert_eq!(
            req["state"]["access_token"]["token"]["value"],
            json!("auto_generate")
        );
        assert_eq!(
            req["connector_feature_data"]["value"],
            json!("auto_generate")
        );
        assert_eq!(req["connector_transaction_id"], json!("auto_generate"));
        assert_eq!(req["refund_id"], json!("auto_generate"));

        // Non-deferred path should still be generated.
        let merchant_txn_id = req["merchant_transaction_id"]
            .as_str()
            .expect("merchant transaction id should be generated");
        assert!(merchant_txn_id.starts_with("mti_"));
    }

    #[test]
    fn context_deferred_path_matching() {
        assert!(is_context_deferred_path("customer.connector_customer_id"));
        assert!(is_context_deferred_path("state.access_token.token.value"));
        assert!(is_context_deferred_path("connector_feature_data.value"));
        assert!(is_context_deferred_path("connector_transaction_id"));
        assert!(is_context_deferred_path("connector_transaction_id.id"));
        assert!(!is_context_deferred_path("merchant_transaction_id.id"));
    }
}
