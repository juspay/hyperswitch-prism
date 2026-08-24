// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payhound
//
// Payhound — all scenarios and flows in one file.
// Run a scenario:  cargo run --example payhound -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["get", "parse_event"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Payhound(
                PayhoundConfig {
                    api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())), // Authentication credential
                    api_secret: Some(hyperswitch_masking::Secret::new(
                        "YOUR_API_SECRET".to_string(),
                    )), // Authentication credential
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

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        merchant_transaction_id: Some("probe_merchant_txn_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn build_handle_event_request() -> EventServiceHandleRequest {
    EventServiceHandleRequest {
        merchant_event_id: Some("probe_event_001".to_string()),  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "{\"id\":\"378d8ec6e305f469b009cb4e2deedf93\",\"status\":\"completed\",\"address\":\"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi\",\"merchant_currency\":\"EUR\",\"merchant_amount\":\"266.45\",\"invoice_currency\":\"BTC\",\"invoice_amount\":\"0.88613839\",\"paid_currency\":\"BTC\",\"paid_amount\":\"0.88613839\",\"reference\":\"probe_ref\",\"invoice_url\":\"/invoices/378d8ec6e305f469b009cb4e2deedf93\",\"create_time\":1398871897.0,\"valid_until_time\":1398872497.0}".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn build_parse_event_request() -> EventServiceParseRequest {
    EventServiceParseRequest {
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "{\"id\":\"378d8ec6e305f469b009cb4e2deedf93\",\"status\":\"completed\",\"address\":\"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi\",\"merchant_currency\":\"EUR\",\"merchant_amount\":\"266.45\",\"invoice_currency\":\"BTC\",\"invoice_amount\":\"0.88613839\",\"paid_currency\":\"BTC\",\"paid_amount\":\"0.88613839\",\"reference\":\"probe_ref\",\"invoice_url\":\"/invoices/378d8ec6e305f469b009cb4e2deedf93\",\"create_time\":1398871897.0,\"valid_until_time\":1398872497.0}".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
    }
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn process_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get(
            build_get_request("probe_connector_txn_001"),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: EventService.ParseEvent
#[allow(dead_code)]
pub async fn process_parse_event(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.parse_event(build_parse_event_request())?;
    Ok(format!("{response:?}"))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_get".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_get" => process_get(&client, "txn_001").await,
        "process_parse_event" => process_parse_event(&client, "txn_001").await,
        _ => {
            eprintln!(
                "Unknown flow: {}. Available: process_get, process_parse_event",
                flow
            );
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
