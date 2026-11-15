// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py revpay
//
// Revpay — all scenarios and flows in one file.
// Run a scenario:  cargo run --example revpay -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["parse_event"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Revpay(RevpayConfig {
                merchant_id: Some(hyperswitch_masking::Secret::new(
                    "YOUR_MERCHANT_ID".to_string(),
                )), // Authentication credential
                merchant_key: Some(hyperswitch_masking::Secret::new(
                    "YOUR_MERCHANT_KEY".to_string(),
                )), // Authentication credential
                key_index: Some(hyperswitch_masking::Secret::new(
                    "YOUR_KEY_INDEX".to_string(),
                )), // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                ..Default::default()
            })),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

#[allow(dead_code)]
pub fn build_handle_event_request() -> EventServiceHandleRequest {
    EventServiceHandleRequest {
        merchant_event_id: Some("probe_event_001".to_string()),
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05".as_bytes().to_vec(),  // Body of the HTTP request.
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
            body: "Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&Transaction_ID=RT000000007222&Reference_Number=RF201774111&Bank_Reference_Number=BR000001&Amount=52.40&Currency=MYR&Transaction_Description=Order+RF201774111&Fraud_Score=0&Response_Code=00&Error_Description=&Key_Index=1&Signature=fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad&Request_Datetime=2020-01-01+12%3A00%3A00&Response_Datetime=2020-01-01+12%3A00%3A05".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
    }
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
        .unwrap_or_else(|| "process_parse_event".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_parse_event" => process_parse_event(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_parse_event", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
