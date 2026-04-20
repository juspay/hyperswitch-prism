// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py aci
//
// Aci — all scenarios and flows in one file.
// Run a scenario:  cargo run --example aci -- process_checkout_card
use grpc_api_types::payments::connector_specific_config;
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

#[allow(dead_code)]
pub const SUPPORTED_FLOWS: &[&str] = &[
    "authorize",
    "capture",
    "get",
    "proxy_authorize",
    "proxy_setup_recurring",
    "recurring_charge",
    "refund",
    "setup_recurring",
    "void",
];

#[allow(dead_code)]
fn build_client() -> ConnectorClient {
    // Configure the connector with authentication
    let config = ConnectorConfig {
        connector_config: None, // TODO: Add your connector config here,
        options: Some(SdkOptions {
            environment: Environment::Sandbox.into(),
        }),
    };
    ConnectorClient::new(config, None).unwrap()
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
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "AUTOMATIC".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
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

// Scenario: Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
#[allow(dead_code)]
pub async fn process_checkout_card(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "MANUAL".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
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

    // Step 2: Capture — settle the reserved funds
    let capture_response = client
        .capture(
            TODO_FIX_MISSING_TYPE_capture {
                merchant_capture_id: "probe_capture_001".to_string(),
                // amount_to_capture: {"minor_amount": 1000, "currency": "USD"}
                connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()), // from Authorize
                ..Default::default()
            },
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

// Scenario: Refund
// Return funds to the customer for a completed payment.
#[allow(dead_code)]
pub async fn process_refund(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Step 1: Authorize — reserve funds on the payment method
    let authorize_response = client
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "AUTOMATIC".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
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

    // Step 2: Refund — return funds to the customer
    let refund_response = client
        .refund(
            TODO_FIX_MISSING_TYPE_refund {
                merchant_refund_id: "probe_refund_001".to_string(),
                payment_amount: 1000,
                // refund_amount: {"minor_amount": 1000, "currency": "USD"}
                reason: "customer_request".to_string(),
                connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()), // from Authorize
                ..Default::default()
            },
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
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "MANUAL".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
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

    // Step 2: Void — release reserved funds (cancel authorization)
    let void_response = client
        .void(
            TODO_FIX_MISSING_TYPE_void {
                merchant_void_id: "probe_void_001".to_string(),
                connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()), // from Authorize
                ..Default::default()
            },
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
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "MANUAL".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
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

    // Step 2: Get — retrieve current payment status from the connector
    let get_response = client
        .get(
            TODO_FIX_MISSING_TYPE_get {
                merchant_transaction_id: "probe_merchant_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                connector_transaction_id: Some(authorize_response.connector_transaction_id.clone()), // from Authorize
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;

    Ok(format!("Status: {:?}", get_response.status()))
}

// Flow: PaymentService.authorize (Card)
#[allow(dead_code)]
pub async fn process_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .authorize(
            TODO_FIX_MISSING_TYPE_authorize {
                merchant_transaction_id: "probe_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                capture_method: "AUTOMATIC".to_string(),
                // address: {"billing_address": {"first_name": "John"}}
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
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

// Flow: PaymentService.capture
#[allow(dead_code)]
pub async fn process_capture(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .capture(
            TODO_FIX_MISSING_TYPE_capture {
                merchant_capture_id: "probe_capture_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount_to_capture: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.get
#[allow(dead_code)]
pub async fn process_get(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .get(
            TODO_FIX_MISSING_TYPE_get {
                merchant_transaction_id: "probe_merchant_txn_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.proxy_authorize
#[allow(dead_code)]
pub async fn process_proxy_authorize(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .proxy_authorize(
            TODO_FIX_MISSING_TYPE_proxy_authorize {
                merchant_transaction_id: "probe_proxy_txn_001".to_string(),
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // card_proxy: {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "123", "card_holder_name": "John Doe"}
                // address: {"billing_address": {"first_name": "John"}}
                capture_method: "AUTOMATIC".to_string(),
                auth_type: "NO_THREE_DS".to_string(),
                return_url: "https://example.com/return".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.proxy_setup_recurring
#[allow(dead_code)]
pub async fn process_proxy_setup_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .proxy_setup_recurring(
            TODO_FIX_MISSING_TYPE_proxy_setup_recurring {
                merchant_recurring_payment_id: "probe_proxy_mandate_001".to_string(),
                // amount: {"minor_amount": 0, "currency": "USD"}
                // card_proxy: {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "123", "card_holder_name": "John Doe"}
                // address: {"billing_address": {}}
                // customer_acceptance: {"acceptance_type": "OFFLINE", "accepted_at": 0}
                auth_type: "NO_THREE_DS".to_string(),
                setup_future_usage: "OFF_SESSION".to_string(),
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.recurring_charge
#[allow(dead_code)]
pub async fn process_recurring_charge(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .recurring_charge(
            TODO_FIX_MISSING_TYPE_recurring_charge {
                // connector_recurring_payment_id: {"mandate_id_type": {"connector_mandate_id": {"connector_mandate_id": "probe-mandate-123"}}}
                // amount: {"minor_amount": 1000, "currency": "USD"}
                // payment_method: {"token": {"token": "probe_pm_token"}}
                return_url: "https://example.com/recurring-return".to_string(),
                connector_customer_id: "cust_probe_123".to_string(),
                payment_method_type: "PAY_PAL".to_string(),
                off_session: true,
                ..Default::default()
            },
            &HashMap::new(),
            None,
        )
        .await?;
    Ok(format!("status: {:?}", response.status()))
}

// Flow: PaymentService.setup_recurring
#[allow(dead_code)]
pub async fn process_setup_recurring(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .setup_recurring(
            TODO_FIX_MISSING_TYPE_setup_recurring {
                merchant_recurring_payment_id: "probe_mandate_001".to_string(),
                // amount: {"minor_amount": 0, "currency": "USD"}
                // payment_method: {"card": {"card_number": "4111111111111111", "card_exp_month": "03", "card_exp_year": "2030", "card_cvc": "737", "card_holder_name": "John Doe"}}
                // address: {"billing_address": {}}
                auth_type: "NO_THREE_DS".to_string(),
                enrolled_for_3ds: false,
                return_url: "https://example.com/mandate-return".to_string(),
                setup_future_usage: "OFF_SESSION".to_string(),
                request_incremental_authorization: false,
                // customer_acceptance: {"acceptance_type": "OFFLINE", "accepted_at": 0}
                ..Default::default()
            },
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

// Flow: PaymentService.void
#[allow(dead_code)]
pub async fn process_void(
    client: &ConnectorClient,
    _merchant_transaction_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = client
        .void(
            TODO_FIX_MISSING_TYPE_void {
                merchant_void_id: "probe_void_001".to_string(),
                connector_transaction_id: "probe_connector_txn_001".to_string(),
                ..Default::default()
            },
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
        "process_checkout_card" => process_checkout_card(&client, "order_001").await,
        "process_refund" => process_refund(&client, "order_001").await,
        "process_void_payment" => process_void_payment(&client, "order_001").await,
        "process_get_payment" => process_get_payment(&client, "order_001").await,
        "process_authorize" => process_authorize(&client, "txn_001").await,
        "process_capture" => process_capture(&client, "txn_001").await,
        "process_get" => process_get(&client, "txn_001").await,
        "process_proxy_authorize" => process_proxy_authorize(&client, "txn_001").await,
        "process_proxy_setup_recurring" => process_proxy_setup_recurring(&client, "txn_001").await,
        "process_recurring_charge" => process_recurring_charge(&client, "txn_001").await,
        "process_setup_recurring" => process_setup_recurring(&client, "txn_001").await,
        "process_void" => process_void(&client, "txn_001").await,
        _ => {
            eprintln!("Unknown flow: {}. Available: process_checkout_autocapture, process_checkout_card, process_refund, process_void_payment, process_get_payment, process_authorize, process_capture, process_get, process_proxy_authorize, process_proxy_setup_recurring, process_recurring_charge, process_setup_recurring, process_void", flow);
            return;
        }
    };
    match result {
        Ok(msg) => println!("✓ {msg}"),
        Err(e) => eprintln!("✗ {e}"),
    }
}
