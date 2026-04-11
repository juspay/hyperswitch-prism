// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py bambora
//
// Bambora — all scenarios and flows in one file.
// Run a scenario:  cargo run --example bambora -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your BamboraConfig
    let config = ConnectorConfig {
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_authorize_request(capture_method: &str) -> PaymentServiceAuthorizeRequest {
    serde_json::from_value::<PaymentServiceAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_txn_001",  // Identification.
    "amount": {  // The amount for the payment.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "payment_method": {  // Payment method to be used.
        "payment_method": {
            "card": {  // Generic card payment.
                "card_number": "4111111111111111",  // Card Identification.
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information.
            },
        }
    },
    "capture_method": capture_method,  // Method for capturing the payment.
    "address": {  // Address Information.
        "billing_address": {
            "last_name": "Doe",
        },
    },
    "auth_type": "NO_THREE_DS",  // Authentication Details.
    "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks.
    "order_details": []  // Order Details.
    })).unwrap_or_default()
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    serde_json::from_value::<PaymentServiceCaptureRequest>(serde_json::json!({
    "merchant_capture_id": "probe_capture_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "amount_to_capture": {  // Capture Details.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    })).unwrap_or_default()
}

pub fn build_get_request(connector_transaction_id: &str) -> FraudServiceGetRequest {
    serde_json::from_value::<FraudServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": connector_transaction_id,
    "amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
    })).unwrap_or_default()
}

pub fn build_proxy_authorize_request() -> PaymentServiceProxyAuthorizeRequest {
    serde_json::from_value::<PaymentServiceProxyAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_proxy_txn_001",
    "amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "card_proxy": {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
        "card_number": "4111111111111111",  // Card Identification.
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "123",
        "card_holder_name": "John Doe",  // Cardholder Information.
    },
    "address": {
        "billing_address": {
            "last_name": "Doe",
        },
    },
    "capture_method": "AUTOMATIC",
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return",
    })).unwrap_or_default()
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    serde_json::from_value::<PaymentServiceRefundRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "payment_amount": 1000,  // Amount Information.
    "refund_amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "reason": "customer_request",  // Reason for the refund.
    })).unwrap_or_default()
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    serde_json::from_value::<RefundServiceGetRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification.
    "connector_transaction_id": "probe_connector_txn_001",
    "refund_id": "probe_refund_id_001",
    })).unwrap_or_default()
}

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    serde_json::from_value::<PaymentServiceVoidRequest>(serde_json::json!({
    "merchant_void_id": "probe_void_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    })).unwrap_or_default()
}


// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
#[allow(dead_code)]
pub async fn process_checkout_autocapture(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    Ok(format!("Payment: {:?} — {}", authorize_response.status(), authorize_response.connector_transaction_id.as_deref().unwrap_or("")))
}

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
#[allow(dead_code)]
pub async fn process_checkout_card(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("MANUAL"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Capture — settle the reserved funds
    let capture_response = client.capture(build_capture_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    if capture_response.status() == PaymentStatus::Failure {
        return Err(format!("Capture failed: {:?}", capture_response.error).into());
    }

    Ok(format!("Payment completed: {}", authorize_response.connector_transaction_id.as_deref().unwrap_or("")))
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Refund — return funds to the customer
    let refund_response = client.refund(build_refund_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    if refund_response.status() == RefundStatus::RefundFailure {
        return Err(format!("Refund failed: {:?}", refund_response.error).into());
    }

    Ok(format!("Refunded: {:?}", refund_response.status()))
}

// Scenario: Void Payment
// Cancel an authorized but not-yet-captured payment.
#[allow(dead_code)]
pub async fn process_void_payment(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("MANUAL"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    let void_response = client.void(build_void_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    Ok(format!("Voided: {:?}", void_response.status()))
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
#[allow(dead_code)]
pub async fn process_get_payment(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("MANUAL"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Get — retrieve current payment status from the connector
    let get_response = client.get(build_get_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    Ok(format!("Status: {:?}", get_response.status()))
}

// Flow: PaymentService.Authorize (Card)
#[allow(dead_code)]
pub async fn authorize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;
    match response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed
            => Err(format!("Authorize failed: {:?}", response.error).into()),
        PaymentStatus::Pending => Ok("pending — await webhook".to_string()),
        _  => Ok(format!("Authorized: {}", response.connector_transaction_id.as_deref().unwrap_or(""))),
    }
}

// Flow: PaymentService.Capture
#[allow(dead_code)]
pub async fn capture(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.capture(build_capture_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: FraudService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.ProxyAuthorize
#[allow(dead_code)]
pub async fn proxy_authorize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.proxy_authorize(build_proxy_authorize_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund(build_refund_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RefundService.Get
#[allow(dead_code)]
pub async fn refund_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund_get(build_refund_get_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Void
#[allow(dead_code)]
pub async fn void(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.void(build_void_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_checkout_autocapture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_checkout_card" => process_checkout_card(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_void_payment" => process_void_payment(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "authorize" => authorize(&client, "order_001").await,
        "capture" => capture(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        "proxy_authorize" => proxy_authorize(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "refund_get" => refund_get(&client, "order_001").await,
        "void" => void(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_checkout_card, process_refund, process_void_payment, process_get_payment, authorize, capture, get, proxy_authorize, refund, refund_get, void", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
