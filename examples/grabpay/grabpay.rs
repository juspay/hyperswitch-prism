// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py grabpay
//
// Grabpay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example grabpay -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["create_order"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Grabpay(GrabpayConfig {
                partner_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_PARTNER_ID".to_string(),
                )), // Authentication credential
                partner_secret: Some(hyperswitch_masking::Secret::new(
                    "YOUR_PARTNER_SECRET".to_string(),
                )), // Authentication credential
                client_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_CLIENT_ID".to_string(),
                )), // Authentication credential
                client_secret: Some(hyperswitch_masking::Secret::new(
                    "YOUR_CLIENT_SECRET".to_string(),
                )), // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_MERCHANT_ID".to_string(),
                )), // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                ..Default::default()
            })),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_create_order_request() -> PaymentServiceCreateOrderRequest {
    PaymentServiceCreateOrderRequest {
        merchant_order_id: Some("probe_order_001".to_string()), // Identification.
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn build_verify_redirect_request() -> PaymentServiceVerifyRedirectResponseRequest {
    PaymentServiceVerifyRedirectResponseRequest {
        ..Default::default()
    }
}

// Flow: PaymentService.CreateOrder
#[allow(dead_code)]
pub async fn process_create_order(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_order(build_create_order_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_create_order".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_create_order" => process_create_order(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_create_order", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
