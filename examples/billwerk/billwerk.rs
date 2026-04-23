// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py billwerk
//
// Billwerk — all scenarios and flows in one file.
// Run a scenario:  cargo run --example billwerk -- process_checkout_card
use cards::CardNumber;
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::payment_method;
use grpc_api_types::payments::*;
use hyperswitch_masking::Secret;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;
use std::str::FromStr;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[
    "capture",
    "get",
    "recurring_charge",
    "refund",
    "refund_get",
    "token_authorize",
    "token_setup_recurring",
    "tokenize",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Billwerk(
                BillwerkConfig {
                    api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())), // Authentication credential
                    public_api_key: Some(hyperswitch_masking::Secret::new(
                        "YOUR_PUBLIC_API_KEY".to_string(),
                    )), // Authentication credential
                    base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    secondary_base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
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

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    PaymentServiceGetRequest {
        merchant_transaction_id: Some("probe_merchant_txn_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        amount: Some(Money {
            // Amount Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        connector_order_reference_id: Some("probe_order_ref_001".to_string()), // Connector Reference Id.
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
        connector_customer_id: Some("cust_probe_123".to_string()),
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

pub fn build_token_authorize_request() -> PaymentServiceTokenAuthorizeRequest {
    PaymentServiceTokenAuthorizeRequest {
        merchant_transaction_id: Some("probe_tokenized_txn_001".to_string()),
        amount: Some(Money {
            minor_amount: 1000,             // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        connector_token: Some(Secret::new("pm_1AbcXyzStripeTestToken".to_string())), // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        capture_method: Some(CaptureMethod::Automatic.into()),
        return_url: Some("https://example.com/return".to_string()),
        ..Default::default()
    }
}

pub fn build_token_setup_recurring_request() -> PaymentServiceTokenSetupRecurringRequest {
    PaymentServiceTokenSetupRecurringRequest {
        merchant_recurring_payment_id: "probe_tokenized_mandate_001".to_string(),
        amount: Some(Money {
            minor_amount: 0,                // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        connector_token: Some(Secret::new("pm_1AbcXyzStripeTestToken".to_string())),
        address: Some(PaymentAddress {
            billing_address: Some(Address {
                ..Default::default()
            }),
            ..Default::default()
        }),
        customer_acceptance: Some(CustomerAcceptance {
            acceptance_type: AcceptanceType::Online.into(), // Type of acceptance (e.g., online, offline).
            accepted_at: 0, // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            online_mandate_details: Some(OnlineMandate {
                // Details if the acceptance was an online mandate.
                ip_address: Some("127.0.0.1".to_string()), // IP address from which the mandate was accepted.
                user_agent: "Mozilla/5.0".to_string(), // User agent string of the browser used for mandate acceptance.
            }),
        }),
        setup_mandate_details: Some(SetupMandateDetails {
            mandate_type: Some(MandateType {
                // Type of mandate (single_use or multi_use) with amount details.
                mandate_type: Some(mandate_type::MandateType::MultiUse(MandateAmountData {
                    amount: 0,                      // Amount.
                    currency: Currency::Usd.into(), // Currency code (ISO 4217).
                    ..Default::default()
                })),
                ..Default::default()
            }),
            ..Default::default()
        }),
        setup_future_usage: Some(FutureUsage::OffSession.into()),
        ..Default::default()
    }
}

pub fn build_tokenize_request() -> PaymentMethodServiceTokenizeRequest {
    PaymentMethodServiceTokenizeRequest {
        amount: Some(Money {
            // Payment Information.
            minor_amount: 1000, // Amount in minor units (e.g., 1000 = $10.00).
            currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
        }),
        payment_method: Some(PaymentMethod {
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

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    PaymentServiceVoidRequest {
        merchant_void_id: Some("probe_void_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
        ..Default::default()
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

// Flow: PaymentService.TokenAuthorize
#[allow(dead_code)]
pub async fn process_token_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .token_authorize(build_token_authorize_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.TokenSetupRecurring
#[allow(dead_code)]
pub async fn process_token_setup_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .token_setup_recurring(build_token_setup_recurring_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentMethodService.Tokenize
#[allow(dead_code)]
pub async fn process_tokenize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .tokenize(build_tokenize_request(), &HashMap::new(), None)
        .await?;
    Ok(format!("token: {}", response.payment_method_token))
}

// Flow: PaymentService.Void
#[allow(dead_code)]
pub async fn process_void(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .void(
            build_void_request("probe_connector_txn_001"),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

#[allow(dead_code)]
#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "process_capture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_recurring_charge" => process_recurring_charge(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_token_authorize" => process_token_authorize(&client, "txn_001").await,
        "process_token_setup_recurring" => process_token_setup_recurring(&client, "txn_001").await,
        "process_tokenize" => process_tokenize(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_capture, process_get, process_recurring_charge, process_refund, process_refund_get, process_token_authorize, process_token_setup_recurring, process_tokenize, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
