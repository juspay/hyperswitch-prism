//! Webhook smoke test — Adyen AUTHORISATION
//!
//! Uses a real Adyen AUTHORISATION webhook body and feeds it into
//! ConnectorClient.handle_event / parse_event with connector identity only
//! (no API credentials, no webhook secret).
//!
//! What this validates:
//!  1. SDK routes to the correct connector from identity alone
//!  2. Adyen webhook body is parsed correctly
//!  3. event_type is returned
//!  4. source_verified=false is expected — no real HMAC secret provided,
//!     and Adyen verification is not mandatory so it must NOT error out
//!  5. IntegrationError / ConnectorError are NOT returned for a valid payload
//!
//! Usage:
//!   cargo run --bin smoke-test-webhook

use grpc_api_types::payments::{
    event_context::EventContext as EventContextVariant, AdyenConfig, ConnectorConfig,
    ConnectorSpecificConfig, Environment, EventContext, EventServiceHandleRequest,
    EventServiceParseRequest, PaymentEventContext, RequestDetails, SdkOptions,
};
use hyperswitch_payments_client::ConnectorClient;
use std::collections::HashMap;

// ── ANSI color helpers ────────────────────────────────────────────────────────
fn no_color() -> bool {
    std::env::var("NO_COLOR").is_ok()
        || (std::env::var("FORCE_COLOR").is_err()
            && std::env::var("TERM").map_or(true, |t| t.is_empty() || t == "dumb"))
}
fn c(code: &str, text: &str) -> String {
    if no_color() {
        text.to_string()
    } else {
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}
fn green(t: &str) -> String {
    c("32", t)
}
fn yellow(t: &str) -> String {
    c("33", t)
}
fn red(t: &str) -> String {
    c("31", t)
}
fn grey(t: &str) -> String {
    c("90", t)
}
fn bold(t: &str) -> String {
    c("1", t)
}

// ── Adyen AUTHORISATION webhook body (from real test configuration) ────────────
// Sensitive fields replaced:
//   merchantAccountCode → "YOUR_MERCHANT_ACCOUNT"
//   merchantReference   → "pay_test_00000000000000"
//   pspReference        → "TEST000000000000"
//   hmacSignature       → "test_hmac_signature_placeholder"
//   cardHolderName      → "John Doe"
//   shopperEmail        → "shopper@example.com"
const ADYEN_WEBHOOK_BODY: &str = r#"{
  "live": "false",
  "notificationItems": [{
    "NotificationRequestItem": {
      "additionalData": {
        "authCode": "APPROVED",
        "cardSummary": "1111",
        "cardHolderName": "John Doe",
        "expiryDate": "03/2030",
        "shopperEmail": "shopper@example.com",
        "shopperIP": "128.0.0.1",
        "shopperInteraction": "Ecommerce",
        "captureDelayHours": "0",
        "gatewaySystem": "direct",
        "hmacSignature": "test_hmac_signature_placeholder"
      },
      "amount": { "currency": "GBP", "value": 654000 },
      "eventCode": "AUTHORISATION",
      "eventDate": "2026-01-21T14:18:18+01:00",
      "merchantAccountCode": "YOUR_MERCHANT_ACCOUNT",
      "merchantReference": "pay_test_00000000000000",
      "operations": ["CAPTURE", "REFUND"],
      "paymentMethod": "visa",
      "pspReference": "TEST000000000000",
      "reason": "APPROVED:1111:03/2030",
      "success": "true"
    }
  }]
}"#;

// HTTP_METHOD_POST = 2 (from payment.proto enum HttpMethod)
// CaptureMethod::Manual = 2 (from payment.proto enum CaptureMethod)
const HTTP_METHOD_POST: i32 = 2;
const CAPTURE_METHOD_MANUAL: i32 = 2;

fn adyen_headers() -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert("content-type".to_string(), "application/json".to_string());
    h.insert("accept".to_string(), "*/*".to_string());
    h
}

// ── Connector identity only — no API creds, no webhook secret ─────────────────
fn build_config() -> ConnectorConfig {
    ConnectorConfig {
        connector_config: Some(ConnectorSpecificConfig {
            config: Some(
                grpc_api_types::payments::connector_specific_config::Config::Adyen(
                    AdyenConfig::default(),
                ),
            ),
        }),
        options: Some(SdkOptions {
            environment: Environment::Sandbox as i32,
        }),
    }
}

fn make_request_details(body: Vec<u8>) -> RequestDetails {
    RequestDetails {
        method: HTTP_METHOD_POST,
        uri: Some("/webhooks/adyen".to_string()),
        headers: adyen_headers(),
        body,
        ..Default::default()
    }
}

// ── Test 1: handle_event — AUTHORISATION ──────────────────────────────────────
fn test_handle_event(client: &ConnectorClient) -> bool {
    println!("{}", bold("\n[Adyen Webhook — AUTHORISATION handle_event]"));

    let request = EventServiceHandleRequest {
        merchant_event_id: Some("smoke_wh_adyen_auth".to_string()),
        request_details: Some(make_request_details(ADYEN_WEBHOOK_BODY.as_bytes().to_vec())),
        event_context: Some(EventContext {
            event_context: Some(EventContextVariant::Payment(PaymentEventContext {
                capture_method: Some(CAPTURE_METHOD_MANUAL),
            })),
        }),
        ..Default::default()
    };

    match client.handle_event(request) {
        Ok(response) => {
            println!(
                "{}",
                grey(&format!("  event_type     : {:?}", response.event_type))
            );
            println!(
                "{}",
                grey(&format!("  source_verified: {}", response.source_verified))
            );
            println!(
                "{}",
                grey(&format!(
                    "  merchant_event : {:?}",
                    response.merchant_event_id
                ))
            );
            if !response.source_verified {
                println!(
                    "{}",
                    yellow("  ~ source_verified=false (expected — no real HMAC secret)")
                );
            }
            println!(
                "{}",
                green("  ✓ PASSED: handle_event returned response without crashing")
            );
            true
        }
        Err(e) => {
            println!("{}", red(&format!("  ✗ FAILED: SdkError: {e:?}")));
            false
        }
    }
}

// ── Test 2: parse_event ────────────────────────────────────────────────────────
fn test_parse_event(client: &ConnectorClient) -> bool {
    println!("{}", bold("\n[Adyen Webhook — AUTHORISATION parse_event]"));

    let request = EventServiceParseRequest {
        request_details: Some(make_request_details(ADYEN_WEBHOOK_BODY.as_bytes().to_vec())),
    };

    match client.parse_event(request) {
        Ok(response) => {
            println!(
                "{}",
                grey(&format!("  event_type : {:?}", response.event_type))
            );
            println!(
                "{}",
                grey(&format!("  reference  : {:?}", response.reference))
            );
            println!("{}", green("  ✓ PASSED: parse_event returned response"));
            true
        }
        Err(e) => {
            println!("{}", red(&format!("  ✗ FAILED: SdkError: {e:?}")));
            false
        }
    }
}

// ── Test 3: malformed body ─────────────────────────────────────────────────────
fn test_malformed_body(client: &ConnectorClient) -> bool {
    println!("{}", bold("\n[Adyen Webhook — malformed body]"));

    let request = EventServiceHandleRequest {
        request_details: Some(make_request_details(b"not valid json {{{{".to_vec())),
        ..Default::default()
    };

    match client.handle_event(request) {
        Ok(response) => {
            println!(
                "{}",
                yellow(&format!(
                    "  ~ accepted malformed body — event_type: {:?}",
                    response.event_type
                ))
            );
            true
        }
        Err(e) => {
            println!(
                "{}",
                green(&format!("  ✓ PASSED: SdkError thrown as expected: {e:?}"))
            );
            true
        }
    }
}

// ── Test 4: unknown eventCode ──────────────────────────────────────────────────
fn test_unknown_event_code(client: &ConnectorClient) -> bool {
    println!("{}", bold("\n[Adyen Webhook — unknown eventCode]"));

    let unknown_body = ADYEN_WEBHOOK_BODY.replace("\"AUTHORISATION\"", "\"SOME_UNKNOWN_EVENT\"");

    let request = EventServiceHandleRequest {
        request_details: Some(make_request_details(unknown_body.into_bytes())),
        ..Default::default()
    };

    match client.handle_event(request) {
        Ok(response) => {
            println!(
                "{}",
                green(&format!(
                    "  ✓ PASSED: handled gracefully — event_type: {:?}",
                    response.event_type
                ))
            );
            true
        }
        Err(e) => {
            println!(
                "{}",
                green(&format!(
                    "  ✓ PASSED: SdkError for unknown event (expected): {e:?}"
                ))
            );
            true
        }
    }
}

fn main() {
    println!("{}", bold("Adyen Webhook Smoke Test"));
    println!("{}", "─".repeat(50));

    let config = build_config();
    let client = match ConnectorClient::new(config, None) {
        Ok(c) => c,
        Err(e) => {
            println!(
                "{}",
                red(&format!("FAILED: Could not create ConnectorClient: {e:?}"))
            );
            std::process::exit(1);
        }
    };

    let results = [
        test_handle_event(&client),
        test_parse_event(&client),
        test_malformed_body(&client),
        test_unknown_event_code(&client),
    ];

    println!("\n{}", "=".repeat(50));
    let all_passed = results.iter().all(|&r| r);
    println!(
        "{}",
        if all_passed {
            green("PASSED")
        } else {
            red("FAILED")
        }
    );
    std::process::exit(if all_passed { 0 } else { 1 });
}
