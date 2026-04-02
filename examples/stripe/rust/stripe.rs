// stripe SDK Examples

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

fn _build_setup_recurring_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_recurring_payment_id": "probe_mandate_001", "amount": {"minor_amount": 0, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "address": {"billing_address": {}}, "auth_type": "NO_THREE_DS", "enrolled_for_3ds": false, "return_url": "https://example.com/mandate-return", "setup_future_usage": "OFF_SESSION", "request_incremental_authorization": false, "customer_acceptance": {"acceptance_type": "OFFLINE", "accepted_at": 0}});
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

fn _build_tokenize_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"amount": {"minor_amount": 1000, "currency": "USD"}, "payment_method": {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}, "address": {"billing_address": {}}});
    if let Some(a) = arg {
        match a {
            "AUTOMATIC" | "MANUAL" => { payload["capture_method"] = json!(a); }
            _ => { payload["connector_transaction_id"] = json!(a); }
        }
    }
    payload
}

fn _build_void_request(arg: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({"merchant_void_id": "probe_void_001", "connector_transaction_id": "probe_connector_txn_001"});
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

async fn process_checkout_card() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Standard card authorization and capture flow
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Payment authorization failed: {}", authorize_response["error"]).into());
    }

    if authorize_response["status"].as_str() == Some("PENDING") {
        return Ok(serde_json::json!({ "status": authorize_response["status"], "transaction_id": authorize_response["connector_transaction_id"] }));
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;
    if capture_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Capture failed: {}", capture_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"], "amount": capture_response["amount"] }))
}

async fn process_checkout_bank() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Bank transfer or debit payment flow
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"ach": serde_json::json!({"account_number": "000123456789", "routing_number": "110000000", "bank_account_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if authorize_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Bank transfer failed: {}", authorize_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": authorize_response["status"], "transaction_id": authorize_response["connector_transaction_id"] }))
}

async fn process_checkout_wallet() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Apple Pay, Google Pay, or other wallet payment
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"apple_pay": serde_json::json!({"payment_data": serde_json::json!({"encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"}), "payment_method": serde_json::json!({"display_name": "Visa 1111", "network": "Visa", "type": "debit"}), "transaction_identifier": "probe_txn_id"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return", "payment_method_token": "probe_pm_token"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Wallet payment failed: {}", authorize_response["error"]).into());
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;

    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"] }))
}

async fn process_refund_payment() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Refund a completed payment
    let request = serde_json::json!({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "payment_amount": 1000, "refund_amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "reason": "customer_request"});
    let refund_response = client.refund(&request).await?;
    if refund_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Refund failed: {}", refund_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": refund_response["status"], "refund_id": refund_response["connector_refund_id"] }))
}

async fn process_setup_recurring() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Create a mandate for recurring charges
    let request = serde_json::json!({"merchant_recurring_payment_id": "probe_mandate_001", "amount": serde_json::json!({"minor_amount": 0, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "enrolled_for_3ds": false, "return_url": "https://example.com/mandate-return", "setup_future_usage": "OFF_SESSION", "request_incremental_authorization": false, "customer_acceptance": serde_json::json!({"acceptance_type": "OFFLINE", "accepted_at": 0})});
    let setup_recurring_response = client.setup_recurring(&request).await?;
    if setup_recurring_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Failed to setup recurring payment: {}", setup_recurring_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": setup_recurring_response["status"], "mandate_id": setup_recurring_response["mandate_reference.connector_mandate_id"] }))
}

async fn process_recurring_charge() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Charge against an existing mandate
    // Prerequisite: Create customer profile
    let request = serde_json::json!({"merchant_customer_id": "cust_probe_123", "customer_name": "John Doe", "email": "test@example.com", "phone_number": "4155552671"});
    let create_customer_response = client.create_customer(&request).await?;

    let request = serde_json::json!({"connector_recurring_payment_id": serde_json::json!({"mandate_id_type": serde_json::json!({"connector_mandate_id": serde_json::json!({"connector_mandate_id": "probe-mandate-123"})}), "connector_mandate_id": serde_json::json!({"connector_mandate_id": setup_recurring_response["mandate_reference"]})}), "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"token": serde_json::json!({"token": "probe_pm_token"})}), "return_url": "https://example.com/recurring-return", "connector_customer_id": create_customer_response["connector_customer_id"], "payment_method_type": "PAY_PAL", "off_session": true});
    let recurring_charge_response = client.recurring_charge(&request).await?;
    if recurring_charge_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Recurring charge failed: {}", recurring_charge_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": recurring_charge_response["status"], "transaction_id": recurring_charge_response["connector_transaction_id"] }))
}

async fn process_tokenize_payment_method() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Tokenize a card or bank account for later use
    let request = serde_json::json!({"amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "address": serde_json::json!({"billing_address": serde_json::json!({})})});
    let tokenize_response = client.tokenize(&request).await?;
    if tokenize_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Tokenization failed: {}", tokenize_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": tokenize_response["status"], "token": tokenize_response["payment_method_token"] }))
}

async fn process_void_authorization() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Cancel an uncaptured authorization
    let request = serde_json::json!({"merchant_void_id": "probe_void_001", "connector_transaction_id": authorize_response["connector_transaction_id"]});
    let void_response = client.void(&request).await?;
    if void_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Void failed: {}", void_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": void_response["status"] }))
}

async fn process_get_payment_status() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Retrieve current status of a payment
    let request = serde_json::json!({"merchant_transaction_id": "probe_merchant_txn_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let get_response = client.get(&request).await?;

    Ok(serde_json::json!({ "status": get_response["status"], "amount": get_response["amount"] }))
}

async fn process_partial_refund() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Refund a portion of a captured payment
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Payment authorization failed: {}", authorize_response["error"]).into());
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;
    if capture_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Capture failed: {}", capture_response["error"]).into());
    }


    let request = serde_json::json!({"merchant_refund_id": "probe_refund_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "payment_amount": 1000, "refund_amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "reason": "customer_request"});
    let refund_response = client.refund(&request).await?;
    if refund_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Refund failed: {}", refund_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": refund_response["status"], "refund_id": refund_response["connector_refund_id"], "refunded_amount": refund_response["amount"] }))
}

async fn process_multi_capture() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Split a single authorization into multiple captures (e.g., for split shipments)
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Payment authorization failed: {}", authorize_response["error"]).into());
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;
    if capture_response["status"].as_str() == Some("FAILED") {
        return Err(format!("Capture failed: {}", capture_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"], "captured_amount": capture_response["amount"] }))
}

async fn process_incremental_authorization() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Increase the authorized amount after initial authorization
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Payment authorization failed: {}", authorize_response["error"]).into());
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;

    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"], "authorized_amount": capture_response["amount"] }))
}

async fn process_checkout_3ds() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Card payment with 3D Secure authentication
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"card": serde_json::json!({"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("Payment authorization failed: {}", authorize_response["error"]).into());
    }

    if authorize_response["status"].as_str() == Some("PENDING_AUTHENTICATION") {
        return Ok(serde_json::json!({ "status": authorize_response["status"], "transaction_id": authorize_response["connector_transaction_id"], "redirect_url": authorize_response["next_action.redirect_url"] }));
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;

    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"] }))
}

async fn process_checkout_bnpl() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Buy Now Pay Later payment flow (Klarna, Afterpay, Affirm)
    let request = serde_json::json!({"merchant_transaction_id": "probe_txn_001", "amount": serde_json::json!({"minor_amount": 1000, "currency": "USD"}), "payment_method": serde_json::json!({"klarna": serde_json::json!({})}), "capture_method": "AUTOMATIC", "address": serde_json::json!({"billing_address": serde_json::json!({})}), "auth_type": "NO_THREE_DS", "return_url": "https://example.com/return"});
    let authorize_response = client.authorize(&request).await?;
    if ["FAILED", "AUTHORIZATION_FAILED"].contains(&authorize_response["status"].as_str().unwrap_or("")) {
        return Err(format!("BNPL authorization failed: {}", authorize_response["error"]).into());
    }

    if authorize_response["status"].as_str() == Some("PENDING_AUTHENTICATION") {
        return Ok(serde_json::json!({ "status": authorize_response["status"], "transaction_id": authorize_response["connector_transaction_id"], "redirect_url": authorize_response["next_action.redirect_url"] }));
    }


    let request = serde_json::json!({"merchant_capture_id": "probe_capture_001", "connector_transaction_id": authorize_response["connector_transaction_id"], "amount_to_capture": serde_json::json!({"minor_amount": 1000, "currency": "USD"})});
    let capture_response = client.capture(&request).await?;
    if capture_response["status"].as_str() == Some("FAILED") {
        return Err(format!("BNPL capture failed: {}", capture_response["error"]).into());
    }


    Ok(serde_json::json!({ "status": capture_response["status"], "transaction_id": capture_response["connector_transaction_id"] }))
}
