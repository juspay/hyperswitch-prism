// Card Payment (Authorize + Capture) - Universal Example
//
// Works with any connector that supports card payments.
// Usage: cargo run --example checkout_card -- --connector stripe

use clap::Parser;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// [START imports]
use std::error::Error;
// [END imports]

#[derive(Parser, Debug)]
#[command(name = "checkout_card")]
#[command(about = "Card Payment (Authorize + Capture) - Universal Example")]
struct Args {
    /// Connector name (stripe, adyen, checkout, etc.)
    #[arg(long)]
    connector: String,
    
    /// JSON file with connector credentials
    #[arg(long)]
    credentials: Option<String>,
}

// [START load_probe_data]
fn load_probe_data(connector_name: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let probe_path = format!("data/field_probe/{}.json", connector_name);
    let content = fs::read_to_string(&probe_path)?;
    let data: serde_json::Value = serde::from_str(&content)?;
    Ok(data)
}
// [END load_probe_data]

// [START stripe_config]
fn get_stripe_config(api_key: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "stripe".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::HeaderKey { 
            api_key: api_key.to_string().into() 
        },
        ..Default::default()
    }
}
// [END stripe_config]

// [START adyen_config]
fn get_adyen_config(api_key: &str, merchant_account: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "adyen".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::BodyKey { 
            api_key: api_key.to_string().into(),
            key1: merchant_account.to_string().into(),
        },
        ..Default::default()
    }
}
// [END adyen_config]

// [START checkout_config]
fn get_checkout_config(api_key: &str) -> ConnectorConfig {
    ConnectorConfig {
        connector: "checkout".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::HeaderKey { 
            api_key: api_key.to_string().into() 
        },
        ..Default::default()
    }
}
// [END checkout_config]

// [START get_connector_config]
fn get_connector_config(connector_name: &str, credentials: &HashMap<String, String>) -> Result<ConnectorConfig, Box<dyn Error>> {
    match connector_name {
        "stripe" => Ok(get_stripe_config(
            credentials.get("api_key").ok_or("api_key required")?
        )),
        "adyen" => Ok(get_adyen_config(
            credentials.get("api_key").ok_or("api_key required")?,
            credentials.get("merchant_account").ok_or("merchant_account required")?
        )),
        "checkout" => Ok(get_checkout_config(
            credentials.get("api_key").ok_or("api_key required")?
        )),
        _ => Err(format!("Unknown connector: {}", connector_name).into()),
    }
}
// [END get_connector_config]

// [START build_authorize_request]
fn build_authorize_request(probe_data: &serde_json::Value, capture_method: &str) -> Result<PaymentServiceAuthorizeRequest, Box<dyn Error>> {
    let flows = probe_data.get("flows")
        .and_then(|f| f.get("authorize"))
        .ok_or("Connector doesn't support authorize flow")?;
    
    // Find Card or first supported payment method
    let mut card_data: Option<&serde_json::Value> = None;
    
    if let Some(obj) = flows.as_object() {
        for (pm_key, pm_data) in obj {
            if pm_data.get("status").and_then(|s| s.as_str()) == Some("supported") {
                if pm_key == "Card" {
                    card_data = Some(pm_data);
                    break;
                } else if card_data.is_none() {
                    card_data = Some(pm_data);
                }
            }
        }
    }
    
    let card_data = card_data.ok_or("No supported payment method found")?;
    let proto_request = card_data.get("proto_request")
        .ok_or("No proto_request in probe data")?;
    
    let mut request: PaymentServiceAuthorizeRequest = serde_json::from_value(proto_request.clone())?;
    request.capture_method = capture_method.to_string();
    
    Ok(request)
}
// [END build_authorize_request]

// [START build_capture_request]
fn build_capture_request(connector_transaction_id: &str, amount: &Amount) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        merchant_capture_id: "capture_001".to_string(),
        connector_transaction_id: connector_transaction_id.to_string(),
        amount_to_capture: Some(amount.clone()),
        ..Default::default()
    }
}
// [END build_capture_request]

// [START process_checkout_card]
pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn Error>> {
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Load connector probe data
    let probe_data = load_probe_data(connector_name)?;
    
    // Build config
    let config = get_connector_config(connector_name, credentials)?;
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&probe_data, "MANUAL")?;
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request, &HashMap::new(), None).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {:?}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure | PaymentStatus::AuthorizationFailed => {
            println!("❌ Payment declined");
            return Ok("failed".to_string());
        }
        PaymentStatus::Pending => {
            println!("⏳ Payment pending - awaiting async confirmation");
            println!("   (In production: wait for webhook, then poll get())");
            return Ok("pending".to_string());
        }
        PaymentStatus::Authorized => {
            println!("✅ Funds reserved successfully");
        }
        _ => {
            println!("⚠️  Unexpected status: {:?}", status);
            return Ok("error".to_string());
        }
    }
    
    // Step 2: Capture
    println!("\n[2/2] Capturing...");
    let capture_request = build_capture_request(
        auth_response.connector_transaction_id.as_deref().unwrap_or(""),
        auth_response.amount.as_ref().unwrap()
    );
    
    let capture_response = client.capture(capture_request, &HashMap::new(), None).await?;
    let capture_status = capture_response.status();
    println!("Capture Status: {:?}", capture_status);
    
    match capture_status {
        PaymentStatus::Captured | PaymentStatus::Authorized => {
            println!("✅ Payment captured successfully");
            Ok("success".to_string())
        }
        _ => {
            println!("⚠️  Capture returned: {:?}", capture_status);
            Ok("partial".to_string())
        }
    }
}
// [END process_checkout_card]

// [START main]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    // Load credentials
    let credentials: HashMap<String, String> = if let Some(creds_path) = args.credentials {
        let content = fs::read_to_string(creds_path)?;
        serde_json::from_str(&content)?
    } else {
        println!("⚠️  Using dummy credentials. Set --credentials for real API calls.");
        let mut map = HashMap::new();
        map.insert("api_key".to_string(), "sk_test_dummy".to_string());
        map
    };
    
    // Run the flow
    let result = process_checkout_card(&args.connector, &credentials).await?;
    
    println!("\n{}", "=".repeat(60));
    println!("Result: {}", result);
    println!("{}", "=".repeat(60));
    
    if result == "success" || result == "pending" {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
// [END main]
