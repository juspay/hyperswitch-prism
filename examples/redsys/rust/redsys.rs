// Auto-generated for redsys
// Run: cargo run --example redsys -- process_checkout_card

use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;
use std::collections::HashMap;

fn build_client() -> ConnectorClient {
    let config = ConnectorConfig {
        connector_config: None,  // TODO: set credentials
        options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
    };
    ConnectorClient::new(config, None).unwrap()
}

#[allow(dead_code)]
pub async fn capture(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.capture
    let response = client.capture(
        serde_json::from_value::<PaymentServiceCaptureRequest>(serde_json::json!({
    "merchant_capture_id": "probe_capture_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount_to_capture": {
        "minor_amount": 1000,
        "currency": "USD",
    },
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.get
    let response = client.get(
        serde_json::from_value::<PaymentServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn pre_authenticate(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentMethodAuthenticationService.PreAuthenticate
    let response = client.pre_authenticate(
        serde_json::from_value::<PaymentMethodAuthenticationServicePreAuthenticateRequest>(serde_json::json!({
    "amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
    "payment_method": {
        "payment_method": {
            "card": {
                "card_number": "4111111111111111",
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",
            },
        }
    },
    "address": {
        "billing_address": {
        },
    },
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/3ds-return",
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.refund
    let response = client.refund(
        serde_json::from_value::<PaymentServiceRefundRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
    "reason": "customer_request",
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn void(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.void
    let response = client.void(
        serde_json::from_value::<PaymentServiceVoidRequest>(serde_json::json!({
    "merchant_void_id": "probe_void_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok("success".to_string())
}

#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    println!("Running flow: {}", flow);
}