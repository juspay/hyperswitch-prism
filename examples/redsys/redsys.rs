// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py redsys
//
// Redsys — all scenarios and flows in one file.
// Run a scenario:  cargo run --example redsys -- process_checkout_card
#![allow(clippy::needless_update)]
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use hyperswitch_masking::Secret;
use grpc_api_types::payments::payment_method;
use cards::CardNumber;
use std::str::FromStr;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["authenticate", "capture", "get", "pre_authenticate", "refund", "refund_get", "void"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Redsys(RedsysConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
                terminal_id: Some(hyperswitch_masking::Secret::new("YOUR_TERMINAL_ID".to_string())),  // Authentication credential
                sha256_pwd: Some(hyperswitch_masking::Secret::new("YOUR_SHA256_PWD".to_string())),  // Authentication credential
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

pub fn build_authenticate_request() -> PaymentMethodAuthenticationServiceAuthenticateRequest {
    PaymentMethodAuthenticationServiceAuthenticateRequest {
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Payment Method.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
            ..Default::default()
        }),
        address: Some(PaymentAddress {  // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        authentication_data: Some(AuthenticationData {  // Authentication Details.
            eci: Some("05".to_string()),  // Electronic Commerce Indicator (ECI) from 3DS.
            cavv: Some("AAAAAAAAAA==".to_string()),  // Cardholder Authentication Verification Value (CAVV).
            threeds_server_transaction_id: Some("probe-3ds-txn-001".to_string()),  // 3DS Server Transaction ID.
            message_version: Some("2.1.0".to_string()),  // 3DS Message Version (e.g., "2.1.0", "2.2.0").
            ds_transaction_id: Some("probe-ds-txn-001".to_string()),  // Directory Server Transaction ID (DS Trans ID).
            ..Default::default()
        }),
        return_url: Some("https://example.com/3ds-return".to_string()),  // URLs for Redirection.
        continue_redirection_url: Some("https://example.com/3ds-continue".to_string()),
        browser_info: Some(BrowserInformation {  // Contextual Information.
            color_depth: Some(24),  // Display Information.
            screen_height: Some(900),
            screen_width: Some(1440),
            java_enabled: Some(false),  // Browser Settings.
            java_script_enabled: Some(true),
            language: Some("en-US".to_string()),
            time_zone_offset_minutes: Some(-480),
            accept_header: Some("application/json".to_string()),  // Browser Headers.
            user_agent: Some("Mozilla/5.0 (probe-bot)".to_string()),
            accept_language: Some("en-US,en;q=0.9".to_string()),
            ip_address: Some("1.2.3.4".to_string()),  // Device Information.
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        merchant_capture_id: Some("probe_capture_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount_to_capture: Some(Money {  // Capture Details.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
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

pub fn build_pre_authenticate_request() -> PaymentMethodAuthenticationServicePreAuthenticateRequest {
    PaymentMethodAuthenticationServicePreAuthenticateRequest {
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {  // Payment Method.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()),  // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())),  // Cardholder Information.
                ..Default::default()
            })),
            ..Default::default()
        }),
        address: Some(PaymentAddress {  // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        enrolled_for_3ds: false,  // Authentication Details.
        return_url: Some("https://example.com/3ds-return".to_string()),  // URLs for Redirection.
        ..Default::default()
    }
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        payment_amount: 1000,  // Amount Information.
        refund_amount: Some(Money {
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        reason: Some("customer_request".to_string()),  // Reason for the refund.
        ..Default::default()
    }
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()),  // Identification.
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        refund_id: "probe_refund_id_001".to_string(),
        ..Default::default()
    }
}

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        merchant_void_id: Some("probe_void_001".to_string()),  // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount: Some(Money {  // Amount Information.
            minor_amount: 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(),  // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        ..Default::default()
    }
}


// Flow: PaymentMethodAuthenticationService.Authenticate
#[allow(dead_code)]
pub async fn process_authenticate(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.authenticate(build_authenticate_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Capture
#[allow(dead_code)]
pub async fn process_capture(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.capture(build_capture_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn process_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.get(build_get_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
#[allow(dead_code)]
pub async fn process_pre_authenticate(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.pre_authenticate(build_pre_authenticate_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund(build_refund_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RefundService.Get
#[allow(dead_code)]
pub async fn process_refund_get(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.refund_get(build_refund_get_request(), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Void
#[allow(dead_code)]
pub async fn process_void(client: &ConnectorClient, _merchant_transaction_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.void(build_void_request("probe_connector_txn_001"), &HashMap::new(), None).await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "process_authenticate".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_authenticate" => process_authenticate(&client, "txn_001").await,
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_pre_authenticate" => process_pre_authenticate(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => { eprintln!("Unknown flow: {}. Available: process_authenticate, process_capture, process_get, process_pre_authenticate, process_refund, process_refund_get, process_void", flow); return; }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
