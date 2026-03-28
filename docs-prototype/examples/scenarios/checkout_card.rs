// Card Payment (Authorize + Capture) - Universal Example
// 
// Works with any connector that supports card payments.
// Usage: cargo run --example checkout_card -- --connector=stripe

use clap::Parser;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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

fn load_connector_data(connector_name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let probe_path = format!("data/field_probe/{}.json", connector_name);
    let content = fs::read_to_string(&probe_path)?;
    let data: Value = serde_json::from_str(&content)?;
    Ok(data)
}

fn build_authorize_request(connector_data: &Value) -> Result<PaymentServiceAuthorizeRequest, Box<dyn std::error::Error>> {
    let flows = connector_data.get("flows")
        .and_then(|f| f.get("authorize"))
        .ok_or("Connector doesn't support authorize flow")?;
    
    // Find Card or first supported payment method
    let mut card_data: Option<&Value> = None;
    
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
    
    // Convert JSON to protobuf request
    let mut request: PaymentServiceAuthorizeRequest = serde_json::from_value(proto_request.clone())?;
    
    // Ensure capture_method is MANUAL
    request.capture_method = "MANUAL".to_string();
    
    Ok(request)
}

fn build_client_config(connector_name: &str, credentials: &HashMap<String, String>) -> ConnectorConfig {
    // Map connector to auth type
    let auth = match connector_name {
        "stripe" => ConnectorAuth::HeaderKey {
            api_key: credentials.get("api_key").cloned().unwrap_or_default().into(),
        },
        "adyen" => ConnectorAuth::BodyKey {
            api_key: credentials.get("api_key").cloned().unwrap_or_default().into(),
            key1: credentials.get("merchant_account").cloned().unwrap_or_default().into(),
        },
        _ => ConnectorAuth::HeaderKey {
            api_key: credentials.get("api_key").cloned().unwrap_or_default().into(),
        },
    };
    
    ConnectorConfig {
        connector: connector_name.to_string(),
        environment: Environment::Sandbox.into(),
        auth,
        ..Default::default()
    }
}

pub async fn process_checkout_card(
    connector_name: &str,
    credentials: &HashMap<String, String>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load connector probe data
    let connector_data = load_connector_data(connector_name)?;
    
    // Build config
    let config = build_client_config(connector_name, credentials);
    let client = ConnectorClient::new(config, None)?;
    
    // Build request from probe data
    let auth_request = build_authorize_request(&connector_data)?;
    
    println!("\n{}", "=".repeat(60));
    println!("Processing Card Payment via {}", connector_name);
    println!("{}", "=".repeat(60));
    
    // Step 1: Authorize
    println!("\n[1/2] Authorizing...");
    let auth_response = client.authorize(auth_request).await?;
    
    let status = auth_response.status();
    println!("Status: {:?}", status);
    println!("Connector Transaction ID: {}", auth_response.connector_transaction_id);
    
    // Handle status
    match status {
        PaymentStatus::Failure => {
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
    let capture_request = PaymentServiceCaptureRequest {
        merchant_capture_id: format!("capture_{}", auth_response.merchant_transaction_id),
        connector_transaction_id: auth_response.connector_transaction_id,
        amount_to_capture: auth_response.amount.clone(),
        ..Default::default()
    };
    
    let capture_response = client.capture(capture_request).await?;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Load credentials
    let credentials: HashMap<String, String> = if let Some(creds_path) = args.credentials {
        let content = fs::read_to_string(creds_path)?;
        serde_json::from_str(&content)?
    } else {
        // Use dummy credentials for demo
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
