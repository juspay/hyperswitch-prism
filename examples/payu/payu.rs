// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payu
//
// Payu — all scenarios and flows in one file.
// Run a scenario:  cargo run --example payu -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your PayuConfig
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
            "upi_collect": {  // UPI Collect.
                "vpa_id": "test@upi",  // Virtual Payment Address.
            },
        }
    },
    "capture_method": capture_method,  // Method for capturing the payment.
    "address": {  // Address Information.
        "billing_address": {
            "first_name": "John",  // Personal Information.
            "email": "test@example.com",  // Contact Information.
            "phone_number": "4155552671",
            "phone_country_code": "+1",
        },
    },
    "auth_type": "NO_THREE_DS",  // Authentication Details.
    "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks.
    "browser_info": {
        "ip_address": "1.2.3.4",  // Device Information.
    },
    "order_details": []  // Order Details.
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


// Flow: PaymentService.Authorize (UpiCollect)
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
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "authorize" => authorize(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: authorize, get", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
