// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all scenarios and flows in one file.
// Run a scenario:  cargo run --example braintree -- process_checkout_card
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
    "create_client_authentication_token",
    "get",
    "parse_event",
    "post_authenticate",
    "pre_authenticate",
    "refund",
    "reverse",
    "token_authorize",
    "token_setup_recurring",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::Braintree(
                BraintreeConfig {
                    public_key: Some(hyperswitch_masking::Secret::new(
                        "YOUR_PUBLIC_KEY".to_string(),
                    )), // Authentication credential
                    private_key: Some(hyperswitch_masking::Secret::new(
                        "YOUR_PRIVATE_KEY".to_string(),
                    )), // Authentication credential
                    base_url: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    merchant_account_id: Some(hyperswitch_masking::Secret::new(
                        "YOUR_MERCHANT_ACCOUNT_ID".to_string(),
                    )), // Authentication credential
                    merchant_config_currency: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    apple_pay_supported_networks: vec!["value".to_string()], // Array field
                    apple_pay_merchant_capabilities: vec!["value".to_string()], // Array field
                    apple_pay_label: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    gpay_merchant_name: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    gpay_merchant_id: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    gpay_allowed_auth_methods: vec!["value".to_string()], // Array field
                    gpay_allowed_card_networks: vec!["value".to_string()], // Array field
                    paypal_client_id: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
                    gpay_gateway_merchant_id: Some("https://sandbox.example.com".to_string()), // Base URL for API calls
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

#[allow(dead_code)]
pub fn build_handle_event_request() -> EventServiceHandleRequest {
    EventServiceHandleRequest {
        merchant_event_id: Some("probe_event_001".to_string()),
        request_details: Some(RequestDetails {
            method: HttpMethod::Post.into(),  // HTTP method of the request (e.g., GET, POST).
            uri: Some("https://example.com/webhook".to_string()),  // URI of the request.
            headers: [].into_iter().collect::<HashMap<_, _>>(),  // Headers of the HTTP request.
            body: "bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D".as_bytes().to_vec(),  // Body of the HTTP request.
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
            body: "bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48a2luZD5kaXNwdXRlX29wZW5lZDwva2luZD48dGltZXN0YW1wPjIwMjQtMDEtMDFUMDA6MDA6MDBaPC90aW1lc3RhbXA%2BPGRpc3B1dGU%2BPGFtb3VudF9kaXNwdXRlZD4xMDAwPC9hbW91bnRfZGlzcHV0ZWQ%2BPGN1cnJlbmN5X2lzb19jb2RlPlVTRDwvY3VycmVuY3lfaXNvX2NvZGU%2BPGlkPmR1bW15X2Rpc3B1dGVfaWRfMDAxPC9pZD48a2luZD5DSEFSR0VCQUNLPC9raW5kPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjxyZWFzb24%2BZnJhdWQ8L3JlYXNvbj48cmVhc29uX2NvZGU%2BODM8L3JlYXNvbl9jb2RlPjx0cmFuc2FjdGlvbj48YW1vdW50PjEwLjAwPC9hbW91bnQ%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjwvdHJhbnNhY3Rpb24%2BPC9kaXNwdXRlPjwvbm90aWZpY2F0aW9uPg%3D%3D".as_bytes().to_vec(),  // Body of the HTTP request.
            ..Default::default()
        }),
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
        connector_order_reference_id: Some("probe_order_ref_001".to_string()),
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

pub fn build_reverse_request(connector_transaction_id: &str) -> PaymentServiceReverseRequest {
    PaymentServiceReverseRequest {
        merchant_reverse_id: Some("probe_reverse_001".to_string()), // Identification.
        connector_transaction_id: connector_transaction_id.to_string(),
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
                    amount_money: Some(Money {
                        // Amount in Money type.
                        minor_amount: 0, // Amount in minor units (e.g., 1000 = $10.00).
                        currency: Currency::Usd.into(), // ISO 4217 currency code (e.g., "USD", "EUR").
                    }),
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

// Flow: EventService.ParseEvent
#[allow(dead_code)]
pub async fn process_parse_event(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client.parse_event(build_parse_event_request())?;
    Ok(format!("{response:?}"))
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

// Flow: PaymentService.Reverse
#[allow(dead_code)]
pub async fn process_reverse(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .reverse(
            build_reverse_request("probe_connector_txn_001"),
            &HashMap::new(),
            None,
        )
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
        "process_create_client_authentication_token" => {
            process_create_client_authentication_token(&client, "txn_001").await
        }
        "process_get" => process_get(&client, "txn_001").await,
        "process_parse_event" => process_parse_event(&client, "txn_001").await,
        "process_post_authenticate" => process_post_authenticate(&client, "txn_001").await,
        "process_pre_authenticate" => process_pre_authenticate(&client, "txn_001").await,
        "process_refund" => process_refund(&client, "txn_001").await,
        "process_reverse" => process_reverse(&client, "txn_001").await,
        "process_token_authorize" => process_token_authorize(&client, "txn_001").await,
        "process_token_setup_recurring" => process_token_setup_recurring(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_capture, process_create_client_authentication_token, process_get, process_parse_event, process_post_authenticate, process_pre_authenticate, process_refund, process_reverse, process_token_authorize, process_token_setup_recurring, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
