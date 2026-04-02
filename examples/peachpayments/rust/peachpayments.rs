// peachpayments SDK Examples

use serde_json::json;

fn _build_authorize_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "capture_method": "AUTOMATIC", "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_capture_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": "probe_connector_txn_001", "amount_to_capture": {"minor_amount": 1000, "currency": "USD"}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

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

fn _build_void_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001", "amount": {"minor_amount": 1000, "currency": "USD"}});
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
