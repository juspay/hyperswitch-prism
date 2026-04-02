// ppro SDK Examples

use serde_json::json;

fn _build_refund_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": "probe_connector_txn_001", "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request"});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_recurring_charge_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"connector_recurring_payment_id": {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}}, "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"token": {"token": "probe_pm_token"}}, "return_url": "https://example.com/recurring-return", "connector_customer_id": "cust_probe_123", "payment_method_type": "PAY_PAL", "off_session": true});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_get_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}
