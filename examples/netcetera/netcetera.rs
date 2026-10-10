// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py netcetera
//
// Netcetera — all scenarios and flows in one file.
// Run a scenario:  cargo run --example netcetera -- process_checkout_card
use cards::CardNumber;
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::payment_method;
use grpc_api_types::payments::*;
use hyperswitch_masking::Secret;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use std::str::FromStr;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["authenticate", "post_authenticate", "pre_authenticate"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Netcetera(
                NetceteraConfig {
                    certificate: Some(hyperswitch_masking::Secret::new(
                        "YOUR_CERTIFICATE".to_string(),
                    )), // Authentication credential
                    private_key: Some(hyperswitch_masking::Secret::new(
                        "YOUR_PRIVATE_KEY".to_string(),
                    )), // Authentication credential
                    three_ds_requestor_id: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    three_ds_requestor_name: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    merchant_configuration_id: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    ..Default::default()
                },
            )),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_authenticate_request() -> PaymentMethodAuthenticationServiceAuthenticateRequest {
    PaymentMethodAuthenticationServiceAuthenticateRequest {
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            // Payment Method.
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
        address: Some(PaymentAddress {
            // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        return_url: Some("https://example.com/3ds-return".to_string()), // URLs for Redirection. For 3DS this is the browser challenge return URL (EMVCo notificationURL / threeDSRequestorURL).
        ..Default::default()
    }
}

pub fn build_post_authenticate_request() -> PaymentMethodAuthenticationServicePostAuthenticateRequest
{
    PaymentMethodAuthenticationServicePostAuthenticateRequest {
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            // Payment Method.
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
        address: Some(PaymentAddress {
            // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_pre_authenticate_request() -> PaymentMethodAuthenticationServicePreAuthenticateRequest
{
    PaymentMethodAuthenticationServicePreAuthenticateRequest {
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            // Payment Method.
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
        address: Some(PaymentAddress {
            // Address Information.
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        enrolled_for_3ds: false, // Authentication Details.
        return_url: Some("https://example.com/3ds-return".to_string()), // URLs for Redirection.
        ..Default::default()
    }
}

// Flow: PaymentMethodAuthenticationService.Authenticate
#[allow(dead_code)]
pub async fn process_authenticate(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .authenticate(build_authenticate_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
#[allow(dead_code)]
pub async fn process_post_authenticate(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .post_authenticate(build_post_authenticate_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
#[allow(dead_code)]
pub async fn process_pre_authenticate(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .pre_authenticate(build_pre_authenticate_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_authenticate".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_authenticate" => process_authenticate(&client, "txn_001").await,
        "process_post_authenticate" => process_post_authenticate(&client, "txn_001").await,
        "process_pre_authenticate" => process_pre_authenticate(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_authenticate, process_post_authenticate, process_pre_authenticate", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
