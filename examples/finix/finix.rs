// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py finix
//
// Finix — all scenarios and flows in one file.
// Run a scenario:  cargo run --example finix -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[
    "capture",
    "create_customer",
    "get",
    "recurring_charge",
    "refund",
    "refund_get",
    "token_authorize",
    "tokenize",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: None, // TODO: Add your connector config here,
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

// Flow: PaymentService.capture
#[allow(dead_code)]
pub async fn process_capture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .capture(
            TODO_FIX_MISSING_TYPE_capture {
                merchant_capture_id: "probe_capture_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount_to_capture: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.create_customer
#[allow(dead_code)]
pub async fn process_create_customer(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_customer(
            TODO_FIX_MISSING_TYPE_create_customer {
                merchant_customer_id: "cust_probe_123".to_string(),
                customer_name: "John Doe".to_string(),
                email: "test@example.com".to_string(),
                phone_number: "4155552671".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("customer_id: {}", response.connector_customer_id))
}

// Flow: PaymentService.get
#[allow(dead_code)]
pub async fn process_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get(
            TODO_FIX_MISSING_TYPE_get {
                merchant_transaction_id: "probe_merchant_txn_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.recurring_charge
#[allow(dead_code)]
pub async fn process_recurring_charge(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .recurring_charge(
            TODO_FIX_MISSING_TYPE_recurring_charge {
                // connector_recurring_payment_id: {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}}
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"token": {"token": "probe_pm_token"}}
                return_url: "https://example.com/recurring-return".to_string(),
                connector_customer_id: "cust_probe_123".to_string(),
                payment_method_type: "PAY_PAL".to_string(),
                off_session: true,
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.refund
#[allow(dead_code)]
pub async fn process_refund(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund(
            TODO_FIX_MISSING_TYPE_refund {
                merchant_refund_id: "probe_refund_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                payment_amount: 1000,
                // refund_amount: {"minor_amount": 1000, "currency": "USD"}
                reason: "customer_request".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.refund_get
#[allow(dead_code)]
pub async fn process_refund_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund_get(
            TODO_FIX_MISSING_TYPE_refund_get {
                merchant_refund_id: "probe_refund_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                refund_id: "probe_refund_id_001".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.token_authorize
#[allow(dead_code)]
pub async fn process_token_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .token_authorize(
            TODO_FIX_MISSING_TYPE_token_authorize {
                merchant_transaction_id: "probe_tokenized_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                connector_token: "pm_1AbcXyzStripeTestToken".to_string(),
                // address: {"billing_address": {}}
                capture_method: "AUTOMATIC".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.tokenize
#[allow(dead_code)]
pub async fn process_tokenize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .tokenize(
            TODO_FIX_MISSING_TYPE_tokenize {
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                // customer: {"id": "cust_probe_123"}
                // address: {"billing_address": {}}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("token: {}", response.payment_method_token))
}

// Flow: PaymentService.void
#[allow(dead_code)]
pub async fn process_void(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .void(
            TODO_FIX_MISSING_TYPE_void {
                merchant_void_id: "probe_void_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_capture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_create_customer" => process_create_customer(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_recurring_charge" => process_recurring_charge(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_token_authorize" => process_token_authorize(&client, "txn_001").await,
        "process_tokenize" => process_tokenize(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_capture, process_create_customer, process_get, process_recurring_charge, process_refund, process_refund_get, process_token_authorize, process_tokenize, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
