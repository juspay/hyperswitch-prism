// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py globalpaymentsrealex
//
// Globalpaymentsrealex — all scenarios and flows in one file.
// Run a scenario:  cargo run --example globalpaymentsrealex -- process_checkout_card
use cards::CardNumber;
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::payment_method;
use grpc_api_types::payments::*;
use hyperswitch_masking::Secret;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use std::str::FromStr;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["authorize", "capture", "proxy_authorize"];

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

pub fn build_authorize_request(capture_method: &str) -> PaymentServiceAuthorizeRequest {
    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some("probe_txn_001".to_string()), // Identification.
        amount: Some(Money {
            // The amount for the payment.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            // Payment method to be used.
            payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
                card_number: Some(CardNumber::from_str("4111111111111111").unwrap()), // Card Identification.
                card_exp_month: Some(Secret::new("03".to_string())),
                card_exp_year: Some(Secret::new("2030".to_string())),
                card_cvc: Some(Secret::new("737".to_string())),
                card_holder_name: Some(Secret::new("John Doe".to_string())), // Cardholder Information.
                ..Default::default()
            })),
            ..Default::default()
        }),
        capture_method: Some(
            CaptureMethod::from_str_name(capture_method)
                .unwrap_or_default()
                .into(),
        ), // Method for capturing the payment.
        address: Some(PaymentAddress {
            // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        auth_type: AuthenticationType::NoThreeDs.into(), // Authentication Details.
        return_url: Some("https://example.com/return".to_string()), // URLs for Redirection and Webhooks.
        ..Default::default()
    }
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        merchant_capture_id: Some("probe_capture_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount_to_capture: Some(Money {
            // Capture Details.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        ..Default::default()
    }
}

pub fn build_proxy_authorize_request() -> PaymentServiceProxyAuthorizeRequest {
    PaymentServiceProxyAuthorizeRequest {
        merchant_transaction_id: Some("probe_proxy_txn_001".to_string()),
        amount: Some(Money {
            minor_amount: 1000,             // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        card_proxy: Some(ProxyCardDetails {
            // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            card_number: Some(Secret::new("4111111111111111".to_string())), // Card Identification.
            card_exp_month: Some(Secret::new("03".to_string())),
            card_exp_year: Some(Secret::new("2030".to_string())),
            card_cvc: Some(Secret::new("123".to_string())),
            card_holder_name: Some(Secret::new("John Doe".to_string())), // Cardholder Information.
            card_network: Some(CardNetwork::Visa.into()),
            ..Default::default()
        }),
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        capture_method: Some(CaptureMethod::Automatic.into()),
        auth_type: AuthenticationType::NoThreeDs.into(),
        return_url: Some("https://example.com/return".to_string()),
        ..Default::default()
    }
}

// Scenario: One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
#[allow(dead_code)]
pub async fn process_checkout_autocapture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    Ok(format!(
        "Payment: {:?} — {}",
        authorize_response.status(),
        authorize_response
            .connector_transaction_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
#[allow(dead_code)]
pub async fn process_checkout_card(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(build_authorize_request("MANUAL"), &HashMap::new(), None)
        .await?;

    match authorize_response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            return Err(format!("Payment failed: {:?}", authorize_response.error).into())
        }
        PaymentStatus::Pending => return Ok("pending — awaiting webhook".to_string()),
        _ => {}
    }

    // Step 2: Capture — settle the reserved funds
    let capture_response = client
        .capture(
            build_capture_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    if capture_response.status() == PaymentStatus::Failure {
        return Err(format!("Capture failed: {:?}", capture_response.error).into());
    }

    Ok(format!(
        "Payment completed: {}",
        authorize_response
            .connector_transaction_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Flow: PaymentService.Authorize (Card)
#[allow(dead_code)]
pub async fn process_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .authorize(build_authorize_request("AUTOMATIC"), &HashMap::new(), None)
        .await?;
    match response.status() {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            Err(format!("Authorize failed: {:?}", response.error).into())
        }
        PaymentStatus::Pending => Ok("pending — await webhook".to_string()),
        _ => Ok(format!(
            "Authorized: {}",
            response.connector_transaction_id.as_deref().unwrap_or("")
        )),
    }
}

// Flow: PaymentService.Capture
#[allow(dead_code)]
pub async fn process_capture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .capture(
            build_capture_request("probe_connector_txn_001"),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.ProxyAuthorize
#[allow(dead_code)]
pub async fn process_proxy_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .proxy_authorize(build_proxy_authorize_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_checkout_autocapture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_checkout_card" => process_checkout_card(&client, "order_001").await,
        "process_authorize" => process_authorize(&client, "txn_001").await,
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_proxy_authorize" => process_proxy_authorize(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_checkout_card, process_authorize, process_capture, process_proxy_authorize", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
