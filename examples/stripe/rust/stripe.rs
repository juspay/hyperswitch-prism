// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py stripe
//
// Stripe — all scenarios and flows in one file.
// Run a scenario:  cargo run --example stripe -- process_checkout_card

use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Set connector_config to authenticate: use ConnectorSpecificConfig with your StripeConfig
    let config = ConnectorConfig {
        connector_config: None, // TODO: Some(ConnectorSpecificConfig { config: Some(...) })
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
}

pub fn build_authorize_request(capture_method: &str) -> PaymentServiceAuthorizeRequest {
    serde_json::from_value::<PaymentServiceAuthorizeRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_txn_001",  // Identification
    "amount": {  // The amount for the payment
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    "payment_method": {  // Payment method to be used
        "payment_method": {
            "card": {  // Generic card payment
                "card_number": "4111111111111111",  // Card Identification
                "card_exp_month": "03",
                "card_exp_year": "2030",
                "card_cvc": "737",
                "card_holder_name": "John Doe",  // Cardholder Information
            },
        }
    },
    "capture_method": capture_method,  // Method for capturing the payment
    "address": {  // Address Information
        "billing_address": {
        },
    },
    "auth_type": "NO_THREE_DS",  // Authentication Details
    "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks
    "order_details": [],  // List of order details (required, can be empty)
    }))
    .unwrap_or_default()
}

pub fn build_capture_request(connector_transaction_id: &str) -> PaymentServiceCaptureRequest {
    serde_json::from_value::<PaymentServiceCaptureRequest>(serde_json::json!({
    "merchant_capture_id": "probe_capture_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "amount_to_capture": {  // Capture Details
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    }))
    .unwrap_or_default()
}

pub fn build_get_request(connector_transaction_id: &str) -> PaymentServiceGetRequest {
    serde_json::from_value::<PaymentServiceGetRequest>(serde_json::json!({
    "merchant_transaction_id": "probe_merchant_txn_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "amount": {  // Amount Information
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    }))
    .unwrap_or_default()
}

pub fn build_refund_request(connector_transaction_id: &str) -> PaymentServiceRefundRequest {
    serde_json::from_value::<PaymentServiceRefundRequest>(serde_json::json!({
    "merchant_refund_id": "probe_refund_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    "payment_amount": 1000,  // Amount Information
    "refund_amount": {
        "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
        "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
    },
    "reason": "customer_request",  // Reason for the refund
    }))
    .unwrap_or_default()
}

pub fn build_void_request(connector_transaction_id: &str) -> PaymentServiceVoidRequest {
    serde_json::from_value::<PaymentServiceVoidRequest>(serde_json::json!({
    "merchant_void_id": "probe_void_001",  // Identification
    "connector_transaction_id": connector_transaction_id,
    }))
    .unwrap_or_default()
}

// Scenario: Card Payment (Authorize + Capture)
// Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.
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

// Scenario: Card Payment (Automatic Capture)
// Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
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

// Scenario: Wallet Payment (Google Pay / Apple Pay)
// Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.
#[allow(dead_code)]
pub async fn process_checkout_wallet(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(
            serde_json::from_value::<PaymentServiceAuthorizeRequest>(serde_json::json!({
                "merchant_transaction_id": "probe_txn_001",  // Identification
                "amount": {  // The amount for the payment
                    "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                    "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
                },
                "payment_method": {  // Payment method to be used
                    "payment_method": {
                        "google_pay": {  // Google Pay
                            "type": "CARD",  // Type of payment method
                            "description": "Visa 1111",  // User-facing description of the payment method
                            "info": {
                                "card_network": "VISA",  // Card network name
                                "card_details": "1111",  // Card details (usually last 4 digits)
                            },
                            "tokenization_data": {
                                "encrypted_data": {  // Encrypted Google Pay payment data
                                    "token_type": "PAYMENT_GATEWAY",  // The type of the token
                                    "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}",  // Token generated for the wallet
                                },
                            },
                        },
                    }
                },
                "capture_method": "AUTOMATIC",  // Method for capturing the payment
                "address": {  // Address Information
                    "billing_address": {
                    },
                },
                "auth_type": "NO_THREE_DS",  // Authentication Details
                "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
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

// Scenario: Bank Transfer (SEPA / ACH / BACS)
// Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.
#[allow(dead_code)]
pub async fn process_checkout_bank(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(
            serde_json::from_value::<PaymentServiceAuthorizeRequest>(serde_json::json!({
                "merchant_transaction_id": "probe_txn_001",  // Identification
                "amount": {  // The amount for the payment
                    "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                    "currency": "EUR",  // ISO 4217 currency code (e.g., "USD", "EUR")
                },
                "payment_method": {  // Payment method to be used
                    "payment_method": {
                        "sepa": {  // Sepa - Single Euro Payments Area direct debit
                            "iban": "DE89370400440532013000",  // International bank account number (iban) for SEPA
                            "bank_account_holder_name": "John Doe",  // Owner name for bank debit
                        },
                    }
                },
                "capture_method": "AUTOMATIC",  // Method for capturing the payment
                "address": {  // Address Information
                    "billing_address": {
                    },
                },
                "auth_type": "NO_THREE_DS",  // Authentication Details
                "return_url": "https://example.com/return",  // URLs for Redirection and Webhooks
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
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

// Scenario: Refund a Payment
// Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.
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

// Scenario: Recurring / Mandate Payments
// Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.
#[allow(dead_code)]
pub async fn process_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Setup Recurring — store the payment mandate
    let setup_response = client
        .setup_recurring(
            serde_json::from_value::<PaymentServiceSetupRecurringRequest>(serde_json::json!({
                "merchant_recurring_payment_id": "probe_mandate_001",  // Identification
                "amount": {  // Mandate Details
                    "minor_amount": 0,  // Amount in minor units (e.g., 1000 = $10.00)
                    "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
                },
                "payment_method": {
                    "payment_method": {
                        "card": {  // Generic card payment
                            "card_number": "4111111111111111",  // Card Identification
                            "card_exp_month": "03",
                            "card_exp_year": "2030",
                            "card_cvc": "737",
                            "card_holder_name": "John Doe",  // Cardholder Information
                        },
                    }
                },
                "address": {  // Address Information
                    "billing_address": {
                    },
                },
                "auth_type": "NO_THREE_DS",  // Type of authentication to be used
                "enrolled_for_3ds": false,  // Indicates if the customer is enrolled for 3D Secure
                "return_url": "https://example.com/mandate-return",  // URL to redirect after setup
                "setup_future_usage": "OFF_SESSION",  // Indicates future usage intention
                "request_incremental_authorization": false,  // Indicates if incremental authorization is requested
                "customer_acceptance": {  // Details of customer acceptance
                    "acceptance_type": "OFFLINE",  // Type of acceptance (e.g., online, offline).
                    "accepted_at": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
                },
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;

    if setup_response.status() == PaymentStatus::Failure {
        return Err(format!("Setup Recurring failed: {:?}", setup_response.error).into());
    }

    // Step 2: Recurring Charge — charge against the stored mandate
    let recurring_response = client
        .recurring_charge(
            serde_json::from_value::<RecurringPaymentServiceChargeRequest>(serde_json::json!({
                "amount": {  // Amount Information
                    "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                    "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
                },
                "return_url": "https://example.com/recurring-return",
                "connector_customer_id": "cust_probe_123",
                "off_session": true,  // Behavioral Flags and Preferences
                // "connector_recurring_payment_id": ???,  // TODO: extract from setup_response.mandate_reference
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;

    if recurring_response.status() == PaymentStatus::Failure {
        return Err(format!("Recurring Charge failed: {:?}", recurring_response.error).into());
    }

    Ok(format!("Charged: {:?}", recurring_response.status()))
}

// Scenario: Void a Payment
// Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.
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
// Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.
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

// Scenario: Create Customer
// Register a customer record in the connector system. Returns a connector_customer_id that can be reused for recurring payments and tokenized card storage.
#[allow(dead_code)]
pub async fn process_create_customer(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Create Customer — register customer record in the connector
    let create_response = client
        .create_customer(
            serde_json::from_value::<CustomerServiceCreateRequest>(serde_json::json!({
                "merchant_customer_id": "cust_probe_123",  // Identification
                "customer_name": "John Doe",  // Name of the customer
                "email": "test@example.com",  // Email address of the customer
                "phone_number": "4155552671",  // Phone number of the customer
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!(
        "Customer: {}",
        create_response.connector_customer_id
    ))
}

// Scenario: Tokenize Payment Method
// Store card details in the connector's vault and receive a reusable payment token. Use the returned token for one-click payments and recurring billing without re-collecting card data.
#[allow(dead_code)]
pub async fn process_tokenize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Tokenize — store card details and return a reusable token
    let tokenize_response = client
        .tokenize(
            serde_json::from_value::<PaymentMethodServiceTokenizeRequest>(serde_json::json!({
                "amount": {  // Payment Information
                    "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                    "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
                },
                "payment_method": {
                    "payment_method": {
                        "card": {  // Generic card payment
                            "card_number": "4111111111111111",  // Card Identification
                            "card_exp_month": "03",
                            "card_exp_year": "2030",
                            "card_cvc": "737",
                            "card_holder_name": "John Doe",  // Cardholder Information
                        },
                    }
                },
                "address": {  // Address Information
                    "billing_address": {
                    },
                },
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Token: {}", tokenize_response.payment_method_token))
}

// Flow: PaymentService.Authorize (Card)
#[allow(dead_code)]
pub async fn authorize(
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
pub async fn capture(
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

// Flow: CustomerService.Create
#[allow(dead_code)]
pub async fn create_customer(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .create_customer(
            serde_json::from_value::<CustomerServiceCreateRequest>(serde_json::json!({
            "merchant_customer_id": "cust_probe_123",  // Identification
            "customer_name": "John Doe",  // Name of the customer
            "email": "test@example.com",  // Email address of the customer
            "phone_number": "4155552671",  // Phone number of the customer
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("customer_id: {}", response.connector_customer_id))
}

// Flow: PaymentService.Get
#[allow(dead_code)]
pub async fn get(
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
pub async fn recurring_charge(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .recurring_charge(
            serde_json::from_value::<RecurringPaymentServiceChargeRequest>(serde_json::json!({
            "connector_recurring_payment_id": {  // Reference to existing mandate
                "mandate_id_type": {
                    "connector_mandate_id": "probe-mandate-123",
                },
            },
            "amount": {  // Amount Information
                "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
            },
            "payment_method": {  // Optional payment Method Information (for network transaction flows)
                "payment_method": {
                    "token": "probe_pm_token",  // Payment tokens
                }
            },
            "return_url": "https://example.com/recurring-return",
            "connector_customer_id": "cust_probe_123",
            "payment_method_type": "PAY_PAL",
            "off_session": true,  // Behavioral Flags and Preferences
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.Refund
#[allow(dead_code)]
pub async fn refund(
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

// Flow: PaymentService.SetupRecurring
#[allow(dead_code)]
pub async fn setup_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .setup_recurring(
            serde_json::from_value::<PaymentServiceSetupRecurringRequest>(serde_json::json!({
            "merchant_recurring_payment_id": "probe_mandate_001",  // Identification
            "amount": {  // Mandate Details
                "minor_amount": 0,  // Amount in minor units (e.g., 1000 = $10.00)
                "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
            },
            "payment_method": {
                "payment_method": {
                    "card": {  // Generic card payment
                        "card_number": "4111111111111111",  // Card Identification
                        "card_exp_month": "03",
                        "card_exp_year": "2030",
                        "card_cvc": "737",
                        "card_holder_name": "John Doe",  // Cardholder Information
                    },
                }
            },
            "address": {  // Address Information
                "billing_address": {
                },
            },
            "auth_type": "NO_THREE_DS",  // Type of authentication to be used
            "enrolled_for_3ds": false,  // Indicates if the customer is enrolled for 3D Secure
            "return_url": "https://example.com/mandate-return",  // URL to redirect after setup
            "setup_future_usage": "OFF_SESSION",  // Indicates future usage intention
            "request_incremental_authorization": false,  // Indicates if incremental authorization is requested
            "customer_acceptance": {  // Details of customer acceptance
                "acceptance_type": "OFFLINE",  // Type of acceptance (e.g., online, offline).
                "accepted_at": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            },
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;
    if response.status() == PaymentStatus::Failure {
        return Err(format!("Setup failed: {:?}", response.error).into());
    }
    Ok(format!(
        "Mandate: {}",
        response
            .connector_recurring_payment_id
            .as_deref()
            .unwrap_or("")
    ))
}

// Flow: PaymentMethodService.Tokenize
#[allow(dead_code)]
pub async fn tokenize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .tokenize(
            serde_json::from_value::<PaymentMethodServiceTokenizeRequest>(serde_json::json!({
            "amount": {  // Payment Information
                "minor_amount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
                "currency": "USD",  // ISO 4217 currency code (e.g., "USD", "EUR")
            },
            "payment_method": {
                "payment_method": {
                    "card": {  // Generic card payment
                        "card_number": "4111111111111111",  // Card Identification
                        "card_exp_month": "03",
                        "card_exp_year": "2030",
                        "card_cvc": "737",
                        "card_holder_name": "John Doe",  // Cardholder Information
                    },
                }
            },
            "address": {  // Address Information
                "billing_address": {
                },
            },
            }))
            .unwrap_or_default(),
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("token: {}", response.payment_method_token))
}

// Flow: PaymentService.Void
#[allow(dead_code)]
pub async fn void(
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
        .unwrap_or_else(|| "process_checkout_card".to_string());
    let result: Result<String, Box<dyn std::error::Error>> = match flow.as_str() {
        "process_checkout_card" => process_checkout_card(&client, "order_001").await,
        "process_checkout_autocapture" => process_checkout_autocapture(&client, "order_001").await,
        "process_checkout_wallet" => process_checkout_wallet(&client, "order_001").await,
        "process_checkout_bank" => process_checkout_bank(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_recurring" => process_recurring(&client, "order_001").await,
        "process_void_payment" => process_void_payment(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "process_create_customer" => process_create_customer(&client, "order_001").await,
        "process_tokenize" => process_tokenize(&client, "order_001").await,
        "authorize" => authorize(&client, "order_001").await,
        "capture" => capture(&client, "order_001").await,
        "create_customer" => create_customer(&client, "order_001").await,
        "get" => get(&client, "order_001").await,
        "recurring_charge" => recurring_charge(&client, "order_001").await,
        "refund" => refund(&client, "order_001").await,
        "setup_recurring" => setup_recurring(&client, "order_001").await,
        "tokenize" => tokenize(&client, "order_001").await,
        "void" => void(&client, "order_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_checkout_card, process_checkout_autocapture, process_checkout_wallet, process_checkout_bank, process_refund, process_recurring, process_void_payment, process_get_payment, process_create_customer, process_tokenize, authorize, capture, create_customer, get, recurring_charge, refund, setup_recurring, tokenize, void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
