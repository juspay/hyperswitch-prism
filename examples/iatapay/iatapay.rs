// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py iatapay
//
// Iatapay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example iatapay -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your IatapayConfig
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
            "ideal": {
            },
        }
    },
    "capture_method": capture_method,  // Method for capturing the payment.
    "address": {  // Address Information.
        "billing_address": {
        },
    },
    "auth_type": "NO_THREE_DS",  // Authentication Details.
    "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks.
    "webhook_url": "https://example.com/webhook",
    "state": {  // State Information.
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
    "order_details": []  // Order Details.
    })).unwrap_or_default()
}

pub fn build_create_server_authentication_token_request() -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    serde_json::from_value::<MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest>(serde_json::json!({

    })).unwrap_or_default()
}

pub fn build_get_request(connector_transaction_id: &str) -> FraudServiceGetRequest {
    serde_json::from_value::<FraudServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": connector_transaction_id,
    "amount": {
        "minor_amount": 1000,
        "currency": "USD",
    },
    "state": {
        "access_token": {
            "token": "probe_access_token",
            "expires_in_seconds": 3600,
            "token_type": "Bearer",
        },
    },
    "connector_order_reference_id": "probe_order_ref_001",
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
    "webhook_url": "https://example.com/webhook",  // URL for webhook notifications.
    "state": {  // State data for access token storage and.
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
    })).unwrap_or_default()
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    serde_json::from_value::<RefundServiceGetRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification.
    "connector_transaction_id": "probe_connector_txn_001",
    "refund_id": "probe_refund_id_001",
    "state": {  // State Information.
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
    })).unwrap_or_default()
}


// Flow: PaymentService.Authorize (Ideal)
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

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
#[allow(dead_code)]
pub async fn create_server_authentication_token(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_server_authentication_token(build_create_server_authentication_token_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: FraudService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
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

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "authorize" => authorize(&client, "order_001").await,
        "create_server_authentication_token" => create_server_authentication_token(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "refund_get" => refund_get(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: authorize, create_server_authentication_token, get, refund, refund_get", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
