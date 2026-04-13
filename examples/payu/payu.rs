// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payu
//
// Payu — all scenarios and flows in one file.
// Run a scenario:  cargo run --example payu -- process_checkout_card
#![allow(clippy::needless_update)]
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use hyperswitch_masking::Secret;
use grpc_api_types::payments::payment_method;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["authorize", "get"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payu(PayuConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                api_secret: Some(hyperswitch_masking::Secret::new("YOUR_API_SECRET".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                ..Default::default()
            })),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_authorize_request(capture_method: &str) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some("probe_txn_001".to_string()),  // Identification.
        amount: Some(Money {  // The amount for the payment.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Payment method to be used.
            payment_method: Some(payment_method::PaymentMethod::UpiCollect(UpiCollect {
                vpa_id: Some(Secret::new("test@upi".to_string())),  // Virtual Payment Address.
                ..Default::default()
            })),
            ..Default::default()
        }),
        capture_method: Some(CaptureMethod::from_str_name(capture_method).unwrap_or_default().into()),  // Method for capturing the payment.
        address: Some(PaymentAddress {  // Address Information.
            billing_address: Some(Address {
                first_name: Some(Secret::new("John".to_string())),  // Personal Information.
                email: Some(Secret::new("test@example.com".to_string())),  // Contact Information.
                phone_number: Some(Secret::new("4155552671".to_string())),
                phone_country_code: Some("+1".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        auth_type: AuthenticationType::NoThreeDs.into(),  // Authentication Details.
        return_url: Some("https://example.com/return".to_string()),  // URLs for Redirection and Webhooks.
        browser_info: Some(BrowserInformation {
            ip_address: Some("1.2.3.4".to_string()),  // Device Information.
            ..Default::default()
        }),
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
        ..Default::default()
    }
}


// Flow: PaymentService.Authorize (UpiCollect)
#[allow(dead_code)]
pub async fn process_authorize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;
    match response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed
            => Err(format!("Authorize failed: {:?}", response.error).into()),
        PaymentStatus::Pending => Ok("pending — await webhook".to_string()),
        _  => Ok(format!("Authorized: {}", response.connector_transaction_id.as_deref().unwrap_or(""))),
    }
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn process_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_authorize".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_authorize" => process_authorize(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_authorize, process_get", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
