// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py givepayments
//
// Givepayments — all scenarios and flows in one file.
// Run a scenario:  cargo run --example givepayments -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::payment_method;
use grpc_api_types::payments::*;
use hyperswitch_masking::Secret;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &["get", "recurring_charge", "refund", "refund_get"];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Givepayments(
                GivepaymentsConfig {
                    api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())), // Authentication credential
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

pub fn build_recurring_charge_request() -> RecurringPaymentServiceChargeRequest {
    RecurringPaymentServiceChargeRequest {
        connector_recurring_payment_id: Some(MandateReference {
            // Reference to existing mandate.
            // mandate_id_type: {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}
            ..Default::default()
        }),
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
            // Optional payment Method Information (for network transaction flows).
            payment_method: Some(payment_method::PaymentMethod::Token(
                TokenPaymentMethodType {
                    token: Some(Secret::new("probe_pm_token".to_string())), // The token string representing a payment method.
                },
            )),
            ..Default::default()
        }),
        return_url: Some("https://example.com/recurring-return".to_string()),
        address: Some(PaymentAddress {
            // Address Information.
            billing_address: Some(Address {
                email: Some(Secret::new("test@example.com".to_string())), // Contact Information.
                ..Default::default()
            }),
            ..Default::default()
        }),
        connector_customer_id: Some("cust_probe_123".to_string()),
        browser_info: Some(BrowserInformation {
            // Browser Information.
            color_depth: Some(24), // Display Information.
            screen_height: Some(900),
            screen_width: Some(1440),
            java_enabled: Some(false), // Browser Settings.
            java_script_enabled: Some(true),
            language: Some("en-US".to_string()),
            time_zone_offset_minutes: Some(-480),
            accept_header: Some("application/json".to_string()), // Browser Headers.
            user_agent: Some("Mozilla/5.0 (probe-bot)".to_string()),
            accept_language: Some("en-US,en;q=0.9".to_string()),
            ip_address: Some("1.2.3.4".to_string()), // Device Information.
            ..Default::default()
        }),
        payment_method_type: Some(PaymentMethodType::PayPal.into()),
        off_session: Some(true), // Behavioral Flags and Preferences.
        ..Default::default()
    }
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    PaymentServiceRefundRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        payment_amount: 1000, // Amount Information.
        refund_amount: Some(Money {
            minor_amount: 1000,             // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        reason: Some("customer_request".to_string()), // Reason for the refund.
        ..Default::default()
    }
}

pub fn build_refund_get_request() -> RefundServiceGetRequest {
    RefundServiceGetRequest {
        merchant_refund_id: Some("probe_refund_001".to_string()), // Identification.
        connector_transaction_id: "probe_connector_txn_001".to_string(),
        refund_id: "probe_refund_id_001".to_string(), // Deprecated.
        ..Default::default()
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

// Flow: RecurringPaymentService.Charge
#[allow(dead_code)]
pub async fn process_recurring_charge(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .recurring_charge(build_recurring_charge_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn process_refund(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund(
            build_refund_request("probe_connector_txn_001"),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: RefundService.Get
#[allow(dead_code)]
pub async fn process_refund_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .refund_get(build_refund_get_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
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
        "process_recurring_charge" => process_recurring_charge(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_get, process_recurring_charge, process_refund, process_refund_get", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
