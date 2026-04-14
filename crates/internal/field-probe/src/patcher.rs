//! Unified field patcher — single entry point for all field patching.
//!
//! Resolution order for every `smart_patch` call:
//!   1. [[multi]]  — one alias → multiple fields simultaneously
//!   2. [[rule]]   — explicit typed mapping; flow-specific rules win over generic

use domain_types::connector_types::ConnectorEnum;
use serde::Serialize;
use serde_json::Value;

use crate::config::{connector_flow_overrides, get_patch_config, PatchConfig};
use crate::sample_data::{full_browser_info, usd_money};

// ── Struct-level patch values ─────────────────────────────────────────────────
// These build full proto structs used as patch values (e.g. when a connector
// reports "billing_address is missing" and we need the whole Address object).
// They live here — not in sample_data — so requests.rs cannot import them and
// accidentally pre-populate base requests, which would hide required-field
// discovery.

fn full_address() -> grpc_api_types::payments::Address {
    use grpc_api_types::payments as proto;
    use hyperswitch_masking::Secret;
    proto::Address {
        first_name: Some(Secret::new("John".to_string())),
        last_name: Some(Secret::new("Doe".to_string())),
        line1: Some(Secret::new("123 Main St".to_string())),
        line2: None,
        line3: None,
        city: Some(Secret::new("Seattle".to_string())),
        state: Some(Secret::new("WA".to_string())),
        zip_code: Some(Secret::new("98101".to_string())),
        country_alpha2_code: Some(proto::CountryAlpha2::Us as i32),
        email: Some(Secret::new("test@example.com".to_string())),
        phone_number: Some(Secret::new("4155552671".to_string())),
        phone_country_code: Some("+1".to_string()),
    }
}

fn full_customer() -> grpc_api_types::payments::Customer {
    use hyperswitch_masking::Secret;
    grpc_api_types::payments::Customer {
        name: Some("John Doe".to_string()),
        email: Some(Secret::new("test@example.com".to_string())),
        id: Some("cust_probe_123".to_string()),
        connector_customer_id: Some("cust_probe_123".to_string()),
        phone_number: Some("4155552671".to_string()),
        phone_country_code: Some("+1".to_string()),
    }
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Resolves `error_field` (as reported by the connector) to a typed patch and
/// applies it to `req` via a JSON round-trip.
///
/// Resolution order:
///   1. `[[multi]]`  — one alias → multiple fields simultaneously
///   2. `[[rule]]`   — explicit alias → path + typed value (flow-specific wins)
pub(crate) fn smart_patch<T>(req: &mut T, flow: &str, error_field: &str)
where
    T: Serialize + for<'de> serde::Deserialize<'de>,
{
    let field = clean_error_field(error_field);
    let config = get_patch_config();

    let mut json = match serde_json::to_value(&req) {
        Ok(v) => v,
        Err(_) => return,
    };

    if apply_patch(config, flow, field, &mut json) {
        if let Ok(patched) = serde_json::from_value(json) {
            *req = patched;
        }
    }
}

/// Strips parenthetical notes and type annotations from connector error strings.
/// "foo (bar from SomeFlow)" → "foo"
/// "field_name: SomeType"    → "field_name"
fn clean_error_field(field: &str) -> &str {
    field
        .split(" (")
        .next()
        .unwrap_or(field)
        .trim()
        .split(": ")
        .next()
        .unwrap_or(field)
        .trim()
}

/// Parses a config key into (target_path, flow).
/// Key format: "path" (flow-agnostic) or "flow.path" (flow-specific).
fn parse_rule_key(key: &str) -> (&str, Option<&str>) {
    const FLOWS: &[&str] = &[
        "capture",
        "refund",
        "void",
        "get",
        "setup_recurring",
        "tokenize",
        "recurring_charge",
        "proxy_authorize",
        "proxy_setup_recurring",
        "pre_authenticate",
        "authenticate",
        "post_authenticate",
        "token_authorize",
        "token_setup_recurring",
        "create_order",
        "incremental_authorization",
        "refund_get",
    ];
    for flow in FLOWS {
        if let Some(rest) = key.strip_prefix(&format!("{}.", flow)) {
            return (rest, Some(flow));
        }
    }
    (key, None)
}

/// Tries all resolution strategies in order. Returns true if the JSON was modified.
fn apply_patch(config: &PatchConfig, flow: &str, field: &str, json: &mut Value) -> bool {
    // [[multi]] rules: one alias → multiple fields patched simultaneously.
    for rule in &config.multi {
        if rule.aliases.iter().any(|a| a == field) {
            let mut applied = false;
            for patch in &rule.patches {
                if let Some(v) = patch_type_to_value(&patch.patch_type, patch.value.as_deref()) {
                    set_at_path(json, &patch.path, v);
                    applied = true;
                }
            }
            if applied {
                return true;
            }
        }
    }

    // Single-field rules: flow-specific first, then flow-agnostic.
    for pass_flow_specific in [true, false] {
        for (key, rule) in &config.rules {
            let (target_path, rule_flow) = parse_rule_key(key);
            let is_flow_specific = rule_flow.is_some();

            if pass_flow_specific != is_flow_specific {
                continue;
            }

            let flow_matches = rule_flow.is_none_or(|f| f == flow);
            if flow_matches && rule.aliases.iter().any(|a| a == field) {
                if let Some(v) = patch_type_to_value(&rule.patch_type, rule.value.as_deref()) {
                    set_at_path(json, target_path, v);
                    return true;
                }
            }
        }
    }

    false
}

/// Converts a `type = "..."` + optional `value = "..."` from the config into a
/// `serde_json::Value` ready to be written at the target JSON path.
pub(crate) fn patch_type_to_value(patch_type: &str, value: Option<&str>) -> Option<Value> {
    use grpc_api_types::payments as proto;

    match patch_type {
        "secret_string" | "string" => value.map(|v| Value::String(v.to_string())),
        "secret_json" => value.and_then(|v| serde_json::from_str(v).ok()),
        "bool" => value?.parse::<bool>().ok().map(Value::Bool),
        "i32" => value?.parse::<i64>().ok().map(|n| Value::Number(n.into())),
        "country_us" => Some(Value::Number((proto::CountryAlpha2::Us as i32).into())),
        "future_usage_off_session" => Some(Value::Number(
            (proto::FutureUsage::OffSession as i32).into(),
        )),
        "usd_money" => serde_json::to_value(usd_money(1000)).ok(),
        "full_browser_info" => serde_json::to_value(full_browser_info()).ok(),
        "full_address" => serde_json::to_value(full_address()).ok(),
        "full_customer" => serde_json::to_value(full_customer()).ok(),
        "bank_names_ing" => Some(Value::Number((proto::BankNames::Ing as i32).into())),
        _ => None,
    }
}

/// Navigates `json` along `path` (dot-separated), creating missing objects,
/// then sets the leaf to `value`.
fn set_at_path(json: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }

    let mut current = json;

    for part in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(serde_json::Map::new());
        }
        let obj = current.as_object_mut().unwrap();
        if !obj.contains_key(*part) {
            obj.insert(part.to_string(), Value::Object(serde_json::Map::new()));
        }
        current = obj.get_mut(*part).unwrap();
    }

    if let Some(obj) = current.as_object_mut() {
        obj.insert(parts.last().unwrap().to_string(), value);
    }
}

/// Pre-applies connector-specific flow overrides to the request before probing begins.
///
/// Reads `[connector_overrides.<name>.<flow>]` from probe-config.toml and sets each
/// listed field in the request via a JSON round-trip. This prevents the probe from
/// getting stuck on connector-specific required fields (e.g. Braintree's
/// `refund_connector_metadata` which embeds `currency` and `merchant_account_id`).
pub(crate) fn apply_connector_flow_overrides<T>(req: &mut T, connector: &ConnectorEnum, flow: &str)
where
    T: Serialize + for<'de> serde::Deserialize<'de>,
{
    let Some(overrides) = connector_flow_overrides(connector, flow) else {
        return;
    };

    let mut json = match serde_json::to_value(&req) {
        Ok(v) => v,
        Err(_) => return,
    };

    for (field, value) in overrides {
        set_at_path(&mut json, field, Value::String(value.clone()));
    }

    if let Ok(patched) = serde_json::from_value(json) {
        *req = patched;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperswitch_masking::PeekInterface;
    use serde_json::json;

    #[test]
    fn test_smart_patch_billing_city() {
        use grpc_api_types::payments::PaymentServiceAuthorizeRequest;
        let mut req = PaymentServiceAuthorizeRequest::default();
        smart_patch(&mut req, "authorize", "billing_address.city");
        let addr = req.address.as_ref().unwrap();
        let billing = addr.billing_address.as_ref().unwrap();
        assert_eq!(
            billing.city.as_ref().map(|s| s.peek().as_str()),
            Some("Seattle")
        );
    }

    #[test]
    fn test_smart_patch_email_via_rule() {
        use grpc_api_types::payments::PaymentServiceAuthorizeRequest;
        let mut req = PaymentServiceAuthorizeRequest::default();
        smart_patch(&mut req, "authorize", "email");
        assert!(req.customer.as_ref().unwrap().email.is_some());
    }

    #[test]
    fn test_smart_patch_address_country_via_rule() {
        use grpc_api_types::payments::PaymentServiceAuthorizeRequest;
        let mut req = PaymentServiceAuthorizeRequest::default();
        smart_patch(&mut req, "authorize", "address.country");
        let billing = req
            .address
            .as_ref()
            .unwrap()
            .billing_address
            .as_ref()
            .unwrap();
        assert!(billing.country_alpha2_code.is_some());
    }

    #[test]
    fn test_smart_patch_capture_amount() {
        use grpc_api_types::payments::PaymentServiceCaptureRequest;
        let mut req = PaymentServiceCaptureRequest {
            connector_transaction_id: "test_txn_001".to_string(),
            ..Default::default()
        };
        smart_patch(&mut req, "capture", "amount");
        assert!(req.amount_to_capture.is_some());
    }

    #[test]
    fn test_smart_patch_void_amount() {
        use grpc_api_types::payments::PaymentServiceVoidRequest;
        let mut req = PaymentServiceVoidRequest {
            connector_transaction_id: "test_txn_001".to_string(),
            ..Default::default()
        };
        smart_patch(&mut req, "void", "amount");
        assert!(req.amount.is_some());
    }

    #[test]
    fn test_smart_patch_simple_json() {
        let mut req = json!({ "address": { "billing_address": {} } });
        smart_patch(&mut req, "authorize", "billing_address.city");
        assert_eq!(req["address"]["billing_address"]["city"], "Seattle");
    }

    #[test]
    fn test_smart_patch_browser_info_time_zone() {
        let mut req = json!({ "browser_info": {} });
        smart_patch(&mut req, "authorize", "browser_info.time_zone");
        assert_eq!(req["browser_info"]["time_zone_offset_minutes"], -480);
    }

    #[test]
    fn test_smart_patch_generic_nested() {
        let mut req = json!({ "browser_info": {} });
        smart_patch(&mut req, "authorize", "browser_info.ip_address");
        assert_eq!(req["browser_info"]["ip_address"], "1.2.3.4");
    }

    #[test]
    fn test_patch_type_to_value_full_browser_info() {
        let val = patch_type_to_value("full_browser_info", None);
        assert!(val.is_some());
        let val = val.unwrap();
        assert!(val.get("ip_address").is_some());
        assert!(val.get("user_agent").is_some());
    }

    #[test]
    fn test_patch_type_to_value_full_address() {
        let val = patch_type_to_value("full_address", None);
        assert!(val.is_some());
        let val = val.unwrap();
        assert!(val.get("city").is_some());
        assert!(val.get("country_alpha2_code").is_some());
    }

    #[test]
    fn test_patch_type_to_value_full_customer() {
        let val = patch_type_to_value("full_customer", None);
        assert!(val.is_some());
        let val = val.unwrap();
        assert!(val.get("email").is_some());
        assert!(val.get("name").is_some());
    }

    #[test]
    fn test_set_at_path() {
        let mut json = json!({});
        set_at_path(&mut json, "a.b.c", json!("value"));
        assert_eq!(json["a"]["b"]["c"], "value");
    }

    #[test]
    fn test_set_at_path_existing() {
        let mut json = json!({ "a": { "b": {} } });
        set_at_path(&mut json, "a.b.c", json!("value"));
        assert_eq!(json["a"]["b"]["c"], "value");
    }

    #[test]
    fn test_set_at_path_ideal_bank_name() {
        use grpc_api_types::payments as proto;
        // Simulating the actual authorize request structure
        let mut json = json!({
            "payment_method": {
                "ideal": {}
            }
        });

        // The patch should set bank_name within the ideal object
        set_at_path(
            &mut json,
            "payment_method.ideal.bank_name",
            json!((proto::BankNames::Ing as i32)),
        );

        // Verify the structure is correct
        assert!(json["payment_method"]["ideal"]["bank_name"].is_number());
        assert_eq!(
            json["payment_method"]["ideal"]["bank_name"],
            (proto::BankNames::Ing as i32)
        );
    }

    #[test]
    fn test_smart_patch_ideal_bank_name_proto() {
        // Test with actual proto-generated types
        use grpc_api_types::payments::{
            payment_method::PaymentMethod as PmVariant, Ideal, PaymentMethod,
            PaymentServiceAuthorizeRequest,
        };

        // Create a request with Ideal payment method (no bank_name)
        let mut req = PaymentServiceAuthorizeRequest {
            payment_method: Some(PaymentMethod {
                payment_method: Some(PmVariant::Ideal(Ideal { bank_name: None })),
            }),
            ..Default::default()
        };

        // Debug: print initial JSON structure
        let json_before = serde_json::to_value(&req).unwrap();
        println!(
            "Before patch: {}",
            serde_json::to_string_pretty(&json_before).unwrap()
        );

        // Apply the patch
        smart_patch(&mut req, "authorize", "ideal.bank_name");

        // Debug: print final JSON structure
        let json_after = serde_json::to_value(&req).unwrap();
        println!(
            "After patch: {}",
            serde_json::to_string_pretty(&json_after).unwrap()
        );

        // Verify the patch was applied
        if let Some(pm) = &req.payment_method {
            if let Some(PmVariant::Ideal(ideal)) = &pm.payment_method {
                assert!(
                    ideal.bank_name.is_some(),
                    "bank_name should be set. Got: {:?}",
                    ideal
                );
            } else {
                panic!("Expected Ideal payment method");
            }
        } else {
            panic!("Expected payment_method to be set");
        }
    }

    #[test]
    fn test_clean_error_field() {
        assert_eq!(
            clean_error_field("field_name (from SomeFlow)"),
            "field_name"
        );
        assert_eq!(clean_error_field("field_name: String"), "field_name");
        assert_eq!(clean_error_field("  field_name  "), "field_name");
    }

    #[test]
    fn test_redirect_response_patching() {
        use grpc_api_types::payments::PaymentMethodAuthenticationServiceAuthenticateRequest;

        let mut req = PaymentMethodAuthenticationServiceAuthenticateRequest::default();

        // Debug: print initial JSON
        let json_before = serde_json::to_value(&req).unwrap();
        println!(
            "Before patch: {}",
            serde_json::to_string_pretty(&json_before).unwrap()
        );

        // Apply patch
        smart_patch(&mut req, "authenticate", "redirect_response");

        // Debug: print final JSON
        let json_after = serde_json::to_value(&req).unwrap();
        println!(
            "After patch: {}",
            serde_json::to_string_pretty(&json_after).unwrap()
        );

        // Verify the field is set
        assert!(
            req.redirection_response.is_some(),
            "redirection_response should be set after patching. Got: {:?}",
            req.redirection_response
        );
    }
}
