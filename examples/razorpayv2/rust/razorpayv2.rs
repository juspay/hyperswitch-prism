// Auto-generated for razorpayv2
// Run: cargo run --example razorpayv2 -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

fn build_client() -> ConnectorClient {
    let config = ConnectorConfig {
        connector_config: None,  // TODO: set credentials
        options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
    };
    ConnectorClient::new(config, None).unwrap()
}

#[allow(dead_code)]
pub async fn process_checkout_autocapture(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Card Payment (Automatic Capture)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_checkout_wallet(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Wallet Payment (Google Pay / Apple Pay)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_checkout_bank(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Bank Transfer (SEPA / ACH / BACS)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Refund a Payment
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    // Step 2: Refund — return funds to the customer
    let response = client.refund(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_get_payment(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Get Payment Status
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    // Step 2: Get — retrieve current payment status from the connector
    let response = client.get(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn authorize(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.authorize (Card)
    let response = client.authorize(
        serde_json::json!({
    "merchant_transaction_id": "probe_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "capture_method": "AUTOMATIC",
    "address": {
        "billing_address": {
        }
    },
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return",
    "merchant_order_id": "probe_order_001"
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn create_order(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.create_order
    let response = client.create_order(
        serde_json::json!({
    "merchant_order_id": "probe_order_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    }
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.get
    let response = client.get(
        serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    }
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.refund
    let response = client.refund(
        serde_json::json!({
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request"
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn tokenized_authorize(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: TokenizedPaymentService.Authorize
    let response = client.tokenized_authorize(
        serde_json::from_value::<TokenizedPaymentServiceAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_tokenized_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "connector_token": "pm_1AbcXyzStripeTestToken",
    "address": {
        "billing_address": {
        }
    },
    "capture_method": "AUTOMATIC",
    "return_url": "https://example.com/return",
    "merchant_order_id": "probe_order_001"
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    println!("Running flow: {}", flow);
}