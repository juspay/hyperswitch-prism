// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashtocode
//
// Cashtocode — all scenarios and flows in one file.
// Run a scenario:  cargo run --example cashtocode -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["parse_event"];

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

// Flow: PaymentService.parse_event
#[allow(dead_code)]
pub async fn process_parse_event(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .parse_event(
            TODO_FIX_MISSING_TYPE_parse_event {
                // request_details: {"method": "HTTP_METHOD_POST", "uri": "https://example.com/webhook", "headers": {}, "body": "{\"amount\":10.0,\"currency\":\"EUR\",\"foreignTransactionId\":\"probe_foreign_001\",\"type\":\"payment\",\"transactionId\":\"probe_txn_001\"}"}
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
        .unwrap_or_else(|| "process_parse_event".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_parse_event" => process_parse_event(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_parse_event", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
