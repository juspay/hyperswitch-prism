// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paysafe
//
// Paysafe — all scenarios and flows in one file.
// Run a scenario:  cargo run --example paysafe -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your PaysafeConfig
    let config = ConnectorConfig {
        connector_config: None,  // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    serde_json::from_value::<PaymentServiceCaptureRequest>(serde_json::json!({
    "merchant_capture_id": "probe_capture_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "amount_to_capture": {  // Capture Details.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
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

pub fn build_proxy_setup_recurring_request() -> PaymentServiceProxySetupRecurringRequest {
    serde_json::from_value::<PaymentServiceProxySetupRecurringRequest>(serde_json::json!({
    "merchant_recurring_payment_id": "probe_proxy_mandate_001",
    "amount": {
        "minor_amount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "card_proxy": {  // Card proxy for vault-aliased payments.
        "card_number": "4111111111111111",  // Card Identification.
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "123",
        "card_holder_name": "John Doe",  // Cardholder Information.
    },
    "address": {
        "billing_address": {
        },
    },
    "return_url": "https://example.com/return",
    "customer_acceptance": {
        "acceptance_type": "OFFLINE",  // Type of acceptance (e.g., online, offline).
        "accepted_at": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
    },
    "auth_type": "NO_THREE_DS",
    "setup_future_usage": "OFF_SESSION",
    })).unwrap_or_default()
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    serde_json::from_value::<PaymentServiceRefundRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "payment_amount": 1000,  // Amount Information.
    "refund_amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "reason": "customer_request",  // Reason for the refund.
    })).unwrap_or_default()
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    serde_json::from_value::<RefundServiceGetRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification.
    "connector_transaction_id": "probe_connector_txn_001",
    "refund_id": "probe_refund_id_001",
    })).unwrap_or_default()
}

pub fn build_setup_recurring_request() -> PaymentServiceSetupRecurringRequest {
    serde_json::from_value::<PaymentServiceSetupRecurringRequest>(serde_json::json!({
    "merchant_recurring_payment_id": "probe_mandate_001",  // Identification.
    "amount": {  // Mandate Details.
        "minor_amount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "payment_method": {
        "payment_method": {
            "card": {  // Generic card payment.
                "card_number": "4111111111111111",  // Card Identification.
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information.
            },
        }
    },
    "address": {  // Address Information.
        "billing_address": {
        },
    },
    "auth_type": "NO_THREE_DS",  // Type of authentication to be used.
    "enrolled_for_3ds": false,  // Indicates if the customer is enrolled for 3D Secure.
    "return_url": "https://example.com/mandate-return",  // URL to redirect after setup.
    "setup_future_usage": "OFF_SESSION",  // Indicates future usage intention.
    "request_incremental_authorization": false,  // Indicates if incremental authorization is requested.
    "customer_acceptance": {  // Details of customer acceptance.
        "acceptance_type": "OFFLINE",  // Type of acceptance (e.g., online, offline).
        "accepted_at": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
    },
    })).unwrap_or_default()
}

pub fn build_token_authorize_request() -> PaymentServiceTokenAuthorizeRequest {
    serde_json::from_value::<PaymentServiceTokenAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_tokenized_txn_001",
    "amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "connector_token": "pm_1AbcXyzStripeTestToken",  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
    "address": {
        "billing_address": {
        },
    },
    "capture_method": "AUTOMATIC",
    "return_url": "https://example.com/return",
    })).unwrap_or_default()
}

pub fn build_tokenize_request() -> PaymentMethodServiceTokenizeRequest {
    serde_json::from_value::<PaymentMethodServiceTokenizeRequest>(serde_json::json!({
    "amount": {  // Payment Information.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "payment_method": {
        "payment_method": {
            "card": {  // Generic card payment.
                "card_number": "4111111111111111",  // Card Identification.
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information.
            },
        }
    },
    "address": {  // Address Information.
        "billing_address": {
        },
    },
    "return_url": "https://example.com/return",  // URLs for Redirection.
    })).unwrap_or_default()
}

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    serde_json::from_value::<PaymentServiceVoidRequest>(serde_json::json!({
    "merchant_void_id": "probe_void_001",  // Identification.
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
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

// Flow: PaymentService.ProxySetupRecurring
#[allow(dead_code)]
pub async fn proxy_setup_recurring(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.proxy_setup_recurring(build_proxy_setup_recurring_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund(build_refund_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RefundService.Get
#[allow(dead_code)]
pub async fn refund_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund_get(build_refund_get_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.SetupRecurring
#[allow(dead_code)]
pub async fn setup_recurring(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.setup_recurring(build_setup_recurring_request(), &HashMap::new(), None).await?;
    if response.status() == PaymentStatus::Failure {
        return Err(format!("Setup failed: {:?}", response.error).into());
    }
    Ok(format!("Mandate: {}", response.connector_recurring_payment_id.as_deref().unwrap_or("")))
}

// Flow: PaymentService.TokenAuthorize
#[allow(dead_code)]
pub async fn token_authorize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.token_authorize(build_token_authorize_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodService.Tokenize
#[allow(dead_code)]
pub async fn tokenize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.tokenize(build_tokenize_request(), &HashMap::new(), None).await?;
    Ok(format!("token: {}", response.payment_method_token))
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
        "proxy_setup_recurring" => proxy_setup_recurring(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "refund_get" => refund_get(&client, "order_001").await,
        "setup_recurring" => setup_recurring(&client, "order_001").await,
        "token_authorize" => token_authorize(&client, "order_001").await,
        "tokenize" => tokenize(&client, "order_001").await,
        "void" => void(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: capture, get, proxy_setup_recurring, refund, refund_get, setup_recurring, token_authorize, tokenize, void", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
