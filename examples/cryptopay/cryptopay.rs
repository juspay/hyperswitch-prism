// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cryptopay
//
// Cryptopay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example cryptopay -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your CryptopayConfig
    let config = ConnectorConfig {
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
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

pub fn build_handle_event_request() -> EventServiceHandleRequest {
    serde_json::from_value::<EventServiceHandleRequest>(serde_json::json!({

    })).unwrap_or_default()
}


// Flow: FraudService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: EventService.HandleEvent
#[allow(dead_code)]
pub async fn handle_event(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.handle_event(build_handle_event_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "get".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "get" => get(&client, "order_001").await,
        "handle_event" => handle_event(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: get, handle_event", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
