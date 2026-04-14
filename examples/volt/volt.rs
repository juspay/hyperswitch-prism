// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py volt
//
// Volt — all scenarios and flows in one file.
// Run a scenario:  cargo run --example volt -- process_checkout_card
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use hyperswitch_masking::Secret;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["create_server_authentication_token", "get", "refund"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Volt(VoltConfig {
                username: Some(hyperswitch_masking::Secret::new("YOUR_USERNAME".to_string())),  // Authentication credential
                password: Some(hyperswitch_masking::Secret::new("YOUR_PASSWORD".to_string())),  // Authentication credential
                client_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_ID".to_string())),  // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_SECRET".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                ..Default::default()
            })),
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

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        merchant_transaction_id: Some("probe_merchant_txn_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        state: Some(ConnectorState {  // State Information.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        payment_amount: 1000,  // Amount Information.
        refund_amount: Some(Money {
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        reason: Some("customer_request".to_string()),  // Reason for the refund.
        state: Some(ConnectorState {  // State data for access token storage and.
            access_token: Some(AccessToken {  // Access token obtained from connector.
                token: Some(Secret::new("probe_access_token".to_string())),  // The token string.
                expires_in_seconds: Some(3600),  // Expiration timestamp (seconds since epoch).
                token_type: Some("Bearer".to_string()),  // Token type (e.g., "Bearer", "Basic").
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}


// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
#[allow(dead_code)]
pub async fn process_create_server_authentication_token(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_server_authentication_token(build_create_server_authentication_token_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn process_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund(build_refund_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_create_server_authentication_token".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_create_server_authentication_token" => process_create_server_authentication_token(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_create_server_authentication_token, process_get, process_refund", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
