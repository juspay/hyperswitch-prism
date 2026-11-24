use serde_json::Value;

/// Applies RFC 7396 JSON Merge Patch semantics.
///
/// Behavior:
/// - object keys in `patch` are recursively merged into `target`
/// - `null` values in `patch` remove keys from `target`
/// - scalar/array/object value replacement happens for non-object pairs
/// - extra keys present only in `patch` are added to `target`
pub fn json_merge_patch(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                if patch_value.is_null() {
                    target_map.remove(key);
                    continue;
                }

                if let Some(target_value) = target_map.get_mut(key) {
                    json_merge_patch(target_value, patch_value);
                } else {
                    target_map.insert(key.clone(), patch_value.clone());
                }
            }
        }
        (target_value, patch_value) => {
            *target_value = patch_value.clone();
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use serde_json::json;

    use super::json_merge_patch;

    #[test]
    fn merge_patch_adds_replaces_and_removes_keys() {
        let mut target = json!({
            "amount": {
                "minor_amount": 1000,
                "currency": "USD"
            },
            "customer": {
                "id": "cust_123",
                "email": "john@example.com"
            }
        });
        let patch = json!({
            "amount": {
                "currency": "EUR"
            },
            "customer": {
                "email": null
            },
            "connector_feature_data": {
                "value": "{\"auth_id\":\"a_1\"}"
            }
        });

        json_merge_patch(&mut target, &patch);

        assert_eq!(target["amount"]["minor_amount"], json!(1000));
        assert_eq!(target["amount"]["currency"], json!("EUR"));
        assert_eq!(target["customer"]["id"], json!("cust_123"));
        assert!(target["customer"].get("email").is_none());
        assert_eq!(
            target["connector_feature_data"]["value"],
            json!("{\"auth_id\":\"a_1\"}")
        );
    }

    #[test]
    fn merge_patch_replaces_non_object_values() {
        let mut target = json!({"capture_method": "AUTOMATIC"});
        let patch = json!({"capture_method": {"value": "MANUAL"}});

        json_merge_patch(&mut target, &patch);

        assert_eq!(target["capture_method"]["value"], json!("MANUAL"));
    }
}
