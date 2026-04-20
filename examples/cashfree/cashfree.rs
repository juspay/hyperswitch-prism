// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashfree
//
// Cashfree — all scenarios and flows in one file.
// Run a scenario:  cargo run --example cashfree -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[
    "capture",
    "create_order",
    "get",
    "refund",
    "refund_get",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: None, // TODO: Add your connector config here,
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

// Flow: PaymentService.capture
#[allow(dead_code)]
pub async fn process_capture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .capture(
            TODO_FIX_MISSING_TYPE_capture {
                merchant_capture_id: "probe_capture_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount_to_capture: {"minor_amount": 1000, "currency": "USD"}
                merchant_order_id: "probe_order_001".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.create_order
#[allow(dead_code)]
pub async fn process_create_order(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_order(
            TODO_FIX_MISSING_TYPE_create_order {
                merchant_order_id: "probe_order_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.get
#[allow(dead_code)]
pub async fn process_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get(
            TODO_FIX_MISSING_TYPE_get {
                merchant_transaction_id: "probe_merchant_txn_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.refund
#[allow(dead_code)]
pub async fn process_refund(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund(
            TODO_FIX_MISSING_TYPE_refund {
                merchant_refund_id: "probe_refund_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                payment_amount: 1000,
                // refund_amount: {"minor_amount": 1000, "currency": "USD"}
                reason: "customer_request".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.refund_get
#[allow(dead_code)]
pub async fn process_refund_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund_get(
            TODO_FIX_MISSING_TYPE_refund_get {
                merchant_refund_id: "probe_refund_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                refund_id: "probe_refund_id_001".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.void
#[allow(dead_code)]
pub async fn process_void(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .void(
            TODO_FIX_MISSING_TYPE_void {
                merchant_void_id: "probe_void_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                merchant_order_id: "probe_order_001".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_capture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_create_order" => process_create_order(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_capture, process_create_order, process_get, process_refund, process_refund_get, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
