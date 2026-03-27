// Auto-generated for adyen
// Run: cargo run --example adyen -- process_checkout_card

use grpc_api_types::payments::{connector_specific_config, *};
use hyperswitch_payments_client::ConnectorClient;
use hyperswitch_masking::Secret;
use std::collections::HashMap;

fn build_client() -> ConnectorClient {
    let config = ConnectorConfig {
        connector_config: None,  // TODO: set credentials
        options: Some(SdkOptions { environment: Environment::Sandbox.into() }),
    };
    ConnectorClient::new(config, None).unwrap()
}

#[allow(dead_code)]
pub async fn process_checkout_card(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Card Payment (Authorize + Capture)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    // Step 2: Capture — settle the reserved funds
    let response = client.capture(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_checkout_autocapture(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Card Payment (Automatic Capture)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_checkout_wallet(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Wallet Payment (Google Pay / Apple Pay)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_checkout_bank(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Bank Transfer (SEPA / ACH / BACS)
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_refund(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Refund a Payment
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    // Step 2: Refund — return funds to the customer
    let response = client.refund(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_recurring(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Recurring / Mandate Payments
    // Step 1: Setup Recurring — store the payment mandate
    let response = client.setup_recurring(todo!(), &HashMap::new(), None).await?;

    // Step 2: Recurring Charge — charge against the stored mandate
    let response = client.recurring_charge(
        serde_json::from_value::<RecurringPaymentServiceChargeRequest>(serde_json::json!({
    "connector_recurring_payment_id": {
        "mandate_id_type": {
            "connector_mandate_id": {
                "connector_mandate_id": "probe-mandate-123"
            }
        }
    },
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "payment_method": {
            "token": {
                "token": "probe_pm_token"
            }
        }
    },
    "return_url": "https://example.com/recurring-return",
    "connector_customer_id": "cust_probe_123",
    "payment_method_type": "PAY_PAL",
    "off_session": true
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn process_void_payment(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Void a Payment
    // Step 1: Authorize — reserve funds on the payment method
    let response = client.authorize(todo!(), &HashMap::new(), None).await?;

    // Step 2: Void — release reserved funds (cancel authorization)
    let response = client.void(todo!(), &HashMap::new(), None).await?;

    Ok("success".to_string())
}

#[allow(dead_code)]
pub async fn authorize(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.authorize (Card)
    let response = client.authorize(
        serde_json::json!({
    "merchant_transaction_id": "probe_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "capture_method": "AUTOMATIC",
    "address": {
        "billing_address": {
        }
    },
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return",
    "browser_info": {
        "color_depth": 24,
        "screen_height": 900,
        "screen_width": 1440,
        "java_enabled": false,
        "java_script_enabled": true,
        "language": "en-US",
        "time_zone_offset_minutes": -480,
        "accept_header": "application/json",
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "accept_language": "en-US,en;q=0.9",
        "ip_address": "1.2.3.4"
    }
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn capture(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.capture
    let response = client.capture(
        serde_json::json!({
    "merchant_capture_id": "probe_capture_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount_to_capture": {
        "minor_amount": 1000,
        "currency": "USD"
    }
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn dispute_accept(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: DisputeService.Accept
    let response = client.dispute_accept(
        serde_json::from_value::<DisputeServiceAcceptRequest>(serde_json::json!({
    "merchant_dispute_id": "probe_dispute_001",
    "connector_transaction_id": "probe_txn_001",
    "dispute_id": "probe_dispute_id_001"
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn dispute_defend(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: DisputeService.Defend
    let response = client.dispute_defend(
        serde_json::from_value::<DisputeServiceDefendRequest>(serde_json::json!({
    "merchant_dispute_id": "probe_dispute_001",
    "connector_transaction_id": "probe_txn_001",
    "dispute_id": "probe_dispute_id_001",
    "reason_code": "probe_reason"
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn dispute_submit_evidence(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: DisputeService.SubmitEvidence
    let response = client.dispute_submit_evidence(
        serde_json::from_value::<DisputeServiceSubmitEvidenceRequest>(serde_json::json!({
    "merchant_dispute_id": "probe_dispute_001",
    "connector_transaction_id": "probe_txn_001",
    "dispute_id": "probe_dispute_id_001",
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn recurring_charge(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: RecurringPaymentService.Charge
    let response = client.recurring_charge(
        serde_json::from_value::<RecurringPaymentServiceChargeRequest>(serde_json::json!({
    "connector_recurring_payment_id": {
        "mandate_id_type": {
            "connector_mandate_id": {
                "connector_mandate_id": "probe-mandate-123"
            }
        }
    },
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "payment_method": {
            "token": {
                "token": "probe_pm_token"
            }
        }
    },
    "return_url": "https://example.com/recurring-return",
    "connector_customer_id": "cust_probe_123",
    "payment_method_type": "PAY_PAL",
    "off_session": true
        })).unwrap_or_default(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn refund(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.refund
    let response = client.refund(
        serde_json::json!({
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request"
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn setup_recurring(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.setup_recurring
    let response = client.setup_recurring(
        serde_json::json!({
    "merchant_recurring_payment_id": "probe_mandate_001",
    "amount": {
        "minor_amount": 0,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "customer": {
        "id": "cust_probe_123"
    },
    "address": {
        "billing_address": {
        }
    },
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/mandate-return",
    "setup_future_usage": "OFF_SESSION",
    "request_incremental_authorization": false,
    "customer_acceptance": {
        "acceptance_type": "OFFLINE",
        "accepted_at": 0
    },
    "browser_info": {
        "color_depth": 24,
        "screen_height": 900,
        "screen_width": 1440,
        "java_enabled": false,
        "java_script_enabled": true,
        "language": "en-US",
        "time_zone_offset_minutes": -480,
        "accept_header": "application/json",
        "user_agent": "Mozilla/5.0 (probe-bot)",
        "accept_language": "en-US,en;q=0.9",
        "ip_address": "1.2.3.4"
    }
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[allow(dead_code)]
pub async fn void(client: &ConnectorClient, _merchant_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Flow: PaymentService.void
    let response = client.void(
        serde_json::json!({
    "merchant_void_id": "probe_void_001",
    "connector_transaction_id": "probe_connector_txn_001"
        }).into(),
        &HashMap::new(), None
    ).await?;
    Ok(format!("Flow completed: {:?}", response.status()))
}

#[tokio::main]
async fn main() {
    let client = build_client();
    let flow = std::env::args().nth(1).unwrap_or_else(|| "authorize".to_string());
    println!("Running flow: {}", flow);
}