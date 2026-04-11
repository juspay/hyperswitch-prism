// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py itaubank
//
// Itaubank — all scenarios and flows in one file.
// Run a scenario:  cargo run --example itaubank -- process_checkout_card

use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;


#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Itaubank(ItaubankConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),
                ..Default::default()
            }),),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_create_server_authentication_token_request() -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {

        ..Default::default()
    }
}


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
#[allow(dead_code)]
pub async fn process_create_server_authentication_token(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_server_authentication_token(build_create_server_authentication_token_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_create_server_authentication_token".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_create_server_authentication_token" => process_create_server_authentication_token(&client, "txn_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_create_server_authentication_token", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
