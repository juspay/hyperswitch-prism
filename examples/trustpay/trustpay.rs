// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py trustpay
//
// Trustpay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example trustpay -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your TrustpayConfig
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
            "card": {  // Generic card payment.
                "card_number": "4111111111111111",  // Card Identification.
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information.
            },
        }
    },
    "capture_method": capture_method,  // Method for capturing the payment.
    "customer": {  // Customer Information.
        "email": "test@example.com",  // Customer's email address.
    },
    "address": {  // Address Information.
        "billing_address": {
            "first_name": "John",  // Personal Information.
            "line1": "123 Main St",  // Address Details.
            "city": "Seattle",
            "zip_code": "98101",
            "country_alpha2_code": "US",
        },
    },
    "auth_type": "NO_THREE_DS",  // Authentication Details.
    "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks.
    "browser_info": {
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "ip_address": "1.2.3.4",  // Device Information.
    },
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

pub fn build_create_order_request() -> PaymentServiceCreateOrderRequest {
    serde_json::from_value::<PaymentServiceCreateOrderRequest>(serde_json::json!({
    "merchant_order_id": "probe_order_001",  // Identification.
    "amount": {  // Amount Information.
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "state": {  // State Information.
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
    })).unwrap_or_default()
}

pub fn build_create_server_authentication_token_request() -> MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    serde_json::from_value::<MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest>(serde_json::json!({

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
    "state": {  // State Information.
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
    })).unwrap_or_default()
}

pub fn build_handle_event_request() -> EventServiceHandleRequest {
    serde_json::from_value::<EventServiceHandleRequest>(serde_json::json!({

    })).unwrap_or_default()
}

pub fn build_proxy_authorize_request() -> PaymentServiceProxyAuthorizeRequest {
    serde_json::from_value::<PaymentServiceProxyAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_proxy_txn_001",
    "amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR").
    },
    "card_proxy": {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
        "card_number": "4111111111111111",  // Card Identification.
        "card_exp_month": "03",
        "card_exp_year": "2030",
        "card_cvc": "123",
        "card_holder_name": "John Doe",  // Cardholder Information.
    },
    "customer": {
        "email": "test@example.com",  // Customer's email address.
    },
    "address": {
        "billing_address": {
            "first_name": "John",  // Personal Information.
            "line1": "123 Main St",  // Address Details.
            "city": "Seattle",
            "zip_code": "98101",
            "country_alpha2_code": "US",
        },
    },
    "capture_method": "AUTOMATIC",
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return",
    "browser_info": {
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "ip_address": "1.2.3.4",  // Device Information.
    },
    "state": {
        "access_token": {  // Access token obtained from connector.
            "token": "probe_access_token",  // The token string.
            "expires_in_seconds": 3600,  // Expiration timestamp (seconds since epoch).
            "token_type": "Bearer",  // Token type (e.g., "Bearer", "Basic").
        },
    },
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


// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
#[allow(dead_code)]
pub async fn process_checkout_autocapture(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    Ok(format!("Payment: {:?} — {}", authorize_response.status(), authorize_response.connector_transaction_id.as_deref().unwrap_or("")))
}

// Scenario: Refund
// Return funds to the customer for a completed payment.
#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Refund — return funds to the customer
    let refund_response = client.refund(build_refund_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    if refund_response.status() == RefundStatus::RefundFailure {
        return Err(format!("Refund failed: {:?}", refund_response.error).into());
    }

    Ok(format!("Refunded: {:?}", refund_response.status()))
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
#[allow(dead_code)]
pub async fn process_get_payment(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client.authorize(build_authorize_request("MANUAL"), &HashMap::new(), None).await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => return Err(format!("Payment failed: {:?}", authorize_response.error).into()),
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _                      => {},
    }

    // Step 2: Get — retrieve current payment status from the connector
    let get_response = client.get(build_get_request(authorize_response.connector_transaction_id.as_deref().unwrap_or("")), &HashMap::new(), None).await?;

    Ok(format!("Status: {:?}", get_response.status()))
}

// Flow: PaymentService.Authorize (Card)
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

// Flow: PaymentService.CreateOrder
#[allow(dead_code)]
pub async fn create_order(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_order(build_create_order_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
#[allow(dead_code)]
pub async fn create_server_authentication_token(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.create_server_authentication_token(build_create_server_authentication_token_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: EventService.HandleEvent
#[allow(dead_code)]
pub async fn handle_event(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.handle_event(build_handle_event_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.ProxyAuthorize
#[allow(dead_code)]
pub async fn proxy_authorize(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.proxy_authorize(build_proxy_authorize_request(), &HashMap::new(), None).await?;
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
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_checkout_autocapture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "authorize" => authorize(&client, "order_001").await,
        "create_order" => create_order(&client, "order_001").await,
        "create_server_authentication_token" => create_server_authentication_token(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        "handle_event" => handle_event(&client, "order_001").await,
        "proxy_authorize" => proxy_authorize(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "refund_get" => refund_get(&client, "order_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_refund, process_get_payment, authorize, create_order, create_server_authentication_token, get, handle_event, proxy_authorize, refund, refund_get", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
