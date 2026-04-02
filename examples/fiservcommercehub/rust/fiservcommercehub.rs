// fiservcommercehub SDK Examples

use serde_json::json;

fn _build_refund_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": "probe_connector_txn_001", "payment_amount": 1000, "refund_amount": {"minor_amount": 1000, "currency": "USD"}, "reason": "customer_request", "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_void_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001", "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_get_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "state": {"access_token": {"token": "probe_key_id|||MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA", "expires_in_seconds": 3600, "token_type": "Bearer"}}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}
