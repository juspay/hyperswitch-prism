// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py nexixpay
//
// Nexixpay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example nexixpay -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your NexixpayConfig
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

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
#[allow(dead_code)]
pub async fn pre_authenticate(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.pre_authenticate(serde_json::from_value::<PaymentMethodAuthenticationServicePreAuthenticateRequest>(serde_json::json!({
    "amount": {  // Amount Information
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    "payment_method": {  // Payment Method
        "payment_method": {
            "card": {  // Generic card payment
                "card_number": "4111111111111111",  // Card Identification
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information
            },
        }
    },
    "address": {  // Address Information
        "billing_address": {
        },
    },
    "enrolled_for_3ds": false,  // Authentication Details
    "return_url": "https://example.com/3ds-return",  // URLs for Redirection
    })).unwrap_or_default(), &HashMap::new(), None).await?;
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
        "get" => get(&client, "order_001").await,
        "pre_authenticate" => pre_authenticate(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "void" => void(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: capture, get, pre_authenticate, refund, void", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
