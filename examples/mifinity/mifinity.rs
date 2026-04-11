// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py mifinity
//
// Mifinity — all scenarios and flows in one file.
// Run a scenario:  cargo run --example mifinity -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your MifinityConfig
    let config = ConnectorConfig {
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_create_client_authentication_token_request() -> MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    serde_json::from_value::<MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest>(serde_json::json!({
    "merchant_client_session_id": "probe_sdk_session_001",  // Infrastructure.
    "domain_context": {
        "payment": {
            "amount": {
                "minor_amount": 1000,
                "currency": "USD",
            },
        },
    },
    })).unwrap_or_default()
}

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    serde_json::from_value::<PaymentServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    })).unwrap_or_default()
}


// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
#[allow(dead_code)]
pub async fn create_client_authentication_token(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_client_authentication_token(build_create_client_authentication_token_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status_code))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "create_client_authentication_token".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "create_client_authentication_token" => create_client_authentication_token(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: create_client_authentication_token, get", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
