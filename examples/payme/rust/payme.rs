// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payme
//
// Payme — all scenarios and flows in one file.
// Run a scenario:  cargo run --example payme -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your PaymeConfig
    let config = ConnectorConfig {
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    serde_json::from_value::<PaymentServiceCaptureRequest>(serde_json::json!({
    "merchant_capture_id": "probe_capture_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "amount_to_capture": {  // Capture Details
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    })).unwrap_or_default()
}

fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    serde_json::from_value::<PaymentServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    })).unwrap_or_default()
}

fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    serde_json::from_value::<PaymentServiceRefundRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "payment_amount": 1000,  // Amount Information
    "refund_amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    "reason": "customer_request",  // Reason for the refund
    })).unwrap_or_default()
}

fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    serde_json::from_value::<PaymentServiceVoidRequest>(serde_json::json!({
    "merchant_void_id": "probe_void_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    })).unwrap_or_default()
}


// Flow: PaymentService.Capture
#[allow(dead_code)]
pub async fn capture(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.capture(build_capture_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.CreateOrder
#[allow(dead_code)]
pub async fn create_order(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_order(serde_json::from_value::<PaymentServiceCreateOrderRequest>(serde_json::json!({
    "merchant_order_id": "probe_order_001",  // Identification
    "amount": {  // Amount Information
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    })).unwrap_or_default(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund(build_refund_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
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
    let flow = std::env::args().nth(1).unwrap_or_else(|| "capture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "capture" => capture(&client, "order_001").await,
        "create_order" => create_order(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "void" => void(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: capture, create_order, get, refund, void", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
