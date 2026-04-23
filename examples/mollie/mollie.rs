// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py mollie
//
// Mollie — all scenarios and flows in one file.
// Run a scenario:  cargo run --example mollie -- process_checkout_card
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
    "authorize",
    "create_client_authentication_token",
    "get",
    "proxy_authorize",
    "refund",
    "refund_get",
    "token_authorize",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Mollie(MollieConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())), // Authentication credential
                profile_token: Some(hyperswitch_masking::Secret::new(
                    "YOUR_PROFILE_TOKEN".to_string(),
                )), // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                secondary_base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                ..Default::default()
            })),
        }),
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
        description: Some("Probe payment".to_string()),
        ..Default::default()
    }
}

pub fn build_create_client_authentication_token_request(
) -> MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        merchant_client_session_id: "probe_sdk_session_001".to_string(), // Infrastructure.
        // domain_context: {"payment": {"amount": {"minor_amount": 1000, "currency": "USD"}}}
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
        description: Some("Probe payment".to_string()), // Description of the transaction.
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
        description: Some("Probe payment".to_string()),
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

// Scenario: Refund
// Return funds to the customer for a completed payment.
#[allow(dead_code)]
pub async fn process_refund(
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

    // Step 2: Refund — return funds to the customer
    let refund_response = client
        .refund(
            build_refund_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    if refund_response.status() == RefundStatus::RefundFailure {
        return Err(format!("Refund failed: {:?}", refund_response.error).into());
    }

    Ok(format!("Refunded: {:?}", refund_response.status()))
}

// Scenario: Void Payment
// Cancel an authorized but not-yet-captured payment.
#[allow(dead_code)]
pub async fn process_void_payment(
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

    // Step 2: Void — release reserved funds (cancel authorization)
    let void_response = client
        .void(
            build_void_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Voided: {:?}", void_response.status()))
}

// Scenario: Get Payment Status
// Retrieve current payment status from the connector.
#[allow(dead_code)]
pub async fn process_get_payment(
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

    // Step 2: Get — retrieve current payment status from the connector
    let get_response = client
        .get(
            build_get_request(
                authorize_response
                    .connector_transaction_id
                    .as_deref()
                    .unwrap_or(""),
            ),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Status: {:?}", get_response.status()))
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

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
#[allow(dead_code)]
pub async fn process_create_client_authentication_token(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_client_authentication_token(
            build_create_client_authentication_token_request(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status_code))
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
        .unwrap_or_else(|| "process_checkout_autocapture".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_void_payment" => process_void_payment(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "process_authorize" => process_authorize(&client, "txn_001").await,
        "process_create_client_authentication_token" => {
            process_create_client_authentication_token(&client, "txn_001").await
        }
        "process_get" => process_get(&client, "txn_001").await,
        "process_proxy_authorize" => process_proxy_authorize(&client, "txn_001").await,
        "process_refund_get" => process_refund_get(&client, "txn_001").await,
        "process_token_authorize" => process_token_authorize(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_refund, process_void_payment, process_get_payment, process_authorize, process_create_client_authentication_token, process_get, process_proxy_authorize, process_refund_get, process_token_authorize, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
