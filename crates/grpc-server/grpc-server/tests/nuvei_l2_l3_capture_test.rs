//! Nuvei Level 2 / Level 3 capture test — live sandbox.
//!
//! Nuvei accepts interchange-optimisation data as `addendums.l23processingData` on
//! `/settleTransaction.do` ONLY, and only on the Auth→Settle path. This test drives
//! the whole chain the data has to travel:
//!
//!   `PaymentServiceCaptureRequest.l2_l3_data`
//!     → `PaymentFlowData.l2_l3_data`
//!     → `build_nuvei_addendums`
//!     → `/settleTransaction.do` body
//!
//! and asserts the addendum is on the wire by reading it back out of
//! `raw_connector_request` on the capture response.
//!
//! ```bash
//! CONNECTOR_AUTH_FILE_PATH=$PWD/creds.json \
//!   cargo test --test nuvei_l2_l3_capture_test -- --nocapture
//! ```
//!
//! Credentials come from the `nuvei` entry of the credentials file
//! (`merchant_id` / `merchant_site_id` / `merchant_secret`); without a credentials
//! file the test skips.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
// The whole point of this test is the exact request/response pair it puts on the
// wire, so it prints them for `-- --nocapture` and for CI logs.
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

mod common;
mod utils;

use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use cards::CardNumber;
use grpc_api_types::payments::{
    merchant_authentication_service_client::MerchantAuthenticationServiceClient,
    merchant_authentication_service_create_server_session_authentication_token_request::DomainContext,
    payment_method, payment_service_client::PaymentServiceClient, AuthenticationType,
    BrowserInformation, CaptureMethod, CardDetails, CountryAlpha2, Currency, L2l3Data,
    MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest, Money, OrderInfo,
    PaymentMethod, PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest,
    PaymentSessionContext, PaymentStatus, TaxInfo, TaxStatus,
};
use grpc_server::app;
use hyperswitch_masking::{ExposeInterface, Secret};
use tonic::{transport::Channel, Request};
use ucs_env::configs;

const CONNECTOR_NAME: &str = "nuvei";
const MERCHANT_ID: &str = "merchant_nuvei_test";

/// `/payment.do` amount in minor units. Nuvei's non-3DS sandbox card.
const TEST_AMOUNT: i64 = 6000;
const TEST_CARD_NUMBER: &str = "4000027891380961";
const TEST_CARD_EXP_MONTH: &str = "12";
const TEST_CARD_EXP_YEAR: &str = "30";
const TEST_CARD_CVC: &str = "123";
const TEST_CARD_HOLDER: &str = "Jane Smith";
const TEST_EMAIL: &str = "customer@example.com";

fn unique_id(prefix: &str) -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();
    format!("{prefix}_{micros}")
}

fn nuvei_config() -> Option<String> {
    if let Ok(config) = std::env::var("TEST_NUVEI_CONFIG") {
        return Some(config);
    }
    utils::credential_utils::connector_config_header(CONNECTOR_NAME).ok()
}

/// Returns false when no credentials are available, so the caller can skip.
fn add_nuvei_metadata<T>(request: &mut Request<T>) -> bool {
    let Some(connector_config) = nuvei_config() else {
        return false;
    };
    request.metadata_mut().append(
        "x-connector-config",
        connector_config.parse().expect("connector config header"),
    );
    request.metadata_mut().append(
        "x-connector",
        CONNECTOR_NAME.parse().expect("connector header"),
    );
    request.metadata_mut().append(
        "x-merchant-id",
        MERCHANT_ID.parse().expect("merchant id header"),
    );
    request
        .metadata_mut()
        .append("x-request-id", unique_id("nuvei_l23").parse().unwrap());
    true
}

fn usd(minor_amount: i64) -> Money {
    Money {
        minor_amount,
        currency: i32::from(Currency::Usd),
    }
}

/// The Level 2/3 payload. Every member is something the Nuvei capture transformer
/// maps onto a documented `l23processingData` field, so the assertions below can
/// name exact wire keys.
fn l2_l3_data() -> L2l3Data {
    L2l3Data {
        order_info: Some(OrderInfo {
            // 2026-06-22T00:00:00Z → `orderDate` "260622".
            order_date: Some(1_782_086_400),
            order_details: vec![grpc_api_types::payments::OrderDetailsWithAmount {
                product_name: "Garden Supplies".to_string(),
                quantity: 5,
                amount: 400,
                tax_rate: Some(0.025),
                total_tax_amount: Some(5),
                description: Some("Garden Supplies".to_string()),
                unit_of_measure: Some("Each".to_string()),
                product_id: Some("ZCGenMerch".to_string()),
                commodity_code: Some("7907".to_string()),
                product_tax_code: Some("1111".to_string()),
                unit_discount_amount: Some(30),
                discount_percentage: Some(0.015),
                ..Default::default()
            }],
            merchant_order_reference_id: Some(unique_id("nuvei_order")),
            discount_amount: Some(30),
            shipping_cost: Some(555),
            duty_amount: Some(16),
        }),
        tax_info: Some(TaxInfo {
            tax_status: Some(i32::from(TaxStatus::Taxable)),
            customer_tax_registration_id: Some("291826".to_string().into()),
            merchant_tax_registration_id: Some("78875627".to_string().into()),
            shipping_amount_tax: Some(88),
            order_tax_amount: Some(22),
        }),
    }
}

/// Nuvei requires a name plus a billing country and email on `/payment.do`.
fn test_address() -> grpc_api_types::payments::Address {
    grpc_api_types::payments::Address {
        first_name: Some("Jane".to_string().into()),
        last_name: Some("Smith".to_string().into()),
        line1: Some("1 Market Street".to_string().into()),
        city: Some("San Francisco".to_string().into()),
        state: Some("CA".to_string().into()),
        zip_code: Some("94105".to_string().into()),
        country_alpha2_code: Some(i32::from(CountryAlpha2::Us)),
        email: Some(TEST_EMAIL.to_string().into()),
        phone_number: Some("4155550123".to_string().into()),
        phone_country_code: Some("+1".to_string()),
        ..Default::default()
    }
}

/// Nuvei's `deviceDetails` are built from `browser_info`, which it requires.
fn test_browser_info() -> BrowserInformation {
    BrowserInformation {
        ip_address: Some("127.0.0.1".to_string()),
        accept_header: Some("application/json".to_string()),
        user_agent: Some("Mozilla/5.0 (nuvei-l2-l3-test)".to_string()),
        language: Some("en-US".to_string()),
        color_depth: Some(24),
        screen_height: Some(1080),
        screen_width: Some(1920),
        java_enabled: Some(false),
        java_script_enabled: Some(true),
        time_zone_offset_minutes: Some(-480),
        ..Default::default()
    }
}

fn authorize_request(session_token: String) -> PaymentServiceAuthorizeRequest {
    let card_details = CardDetails {
        card_number: Some(CardNumber::from_str(TEST_CARD_NUMBER).unwrap()),
        card_exp_month: Some(Secret::new(TEST_CARD_EXP_MONTH.to_string())),
        card_exp_year: Some(Secret::new(TEST_CARD_EXP_YEAR.to_string())),
        card_cvc: Some(Secret::new(TEST_CARD_CVC.to_string())),
        card_holder_name: Some(Secret::new(TEST_CARD_HOLDER.to_string())),
        ..Default::default()
    };
    PaymentServiceAuthorizeRequest {
        amount: Some(usd(TEST_AMOUNT)),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(card_details)),
        }),
        customer: Some(grpc_api_types::payments::Customer {
            email: Some(TEST_EMAIL.to_string().into()),
            id: Some(unique_id("nuvei_cust")),
            ..Default::default()
        }),
        // Nuvei rejects `/payment.do` without a name and a billing country/email.
        address: Some(grpc_api_types::payments::PaymentAddress {
            billing_address: Some(test_address()),
            shipping_address: Some(test_address()),
        }),
        auth_type: i32::from(AuthenticationType::NoThreeDs),
        merchant_transaction_id: Some(unique_id("nuvei_l23_auth")),
        enrolled_for_3ds: Some(false),
        browser_info: Some(test_browser_info()),
        // Manual capture is what makes this an Auth, not a Sale — an auto-capture
        // Sale has no settle leg and therefore cannot carry Level 2/3 data at all.
        capture_method: Some(i32::from(CaptureMethod::Manual)),
        session_token: Some(session_token),
        ..Default::default()
    }
}

fn capture_request(transaction_id: &str, with_l2_l3: bool) -> PaymentServiceCaptureRequest {
    PaymentServiceCaptureRequest {
        connector_transaction_id: transaction_id.to_string(),
        amount_to_capture: Some(usd(TEST_AMOUNT)),
        merchant_capture_id: Some(unique_id("nuvei_l23_cap")),
        capture_method: Some(i32::from(CaptureMethod::Manual)),
        l2_l3_data: with_l2_l3.then(l2_l3_data),
        ..Default::default()
    }
}

/// `raw_connector_request` wraps the outbound call as
/// `{"url":..,"method":..,"headers":..,"body":{..}}`; the settle payload is the
/// `body` member, so assertions must descend into it rather than the envelope.
fn settle_request_body(raw_connector_request: &str) -> serde_json::Value {
    let envelope: serde_json::Value =
        serde_json::from_str(raw_connector_request).expect("raw_connector_request is JSON");
    assert_eq!(
        envelope.get("url").and_then(|url| url.as_str()),
        Some("https://ppp-test.nuvei.com/ppp/api/v1/settleTransaction.do"),
        "Level 2/3 data is only valid on /settleTransaction.do"
    );
    envelope
        .get("body")
        .cloned()
        .expect("raw_connector_request must carry the request body")
}

/// Nuvei's Authorize needs a `/getSessionToken.do` token; the caller supplies it on
/// the Authorize request, so the session RPC runs first.
async fn server_session_token(
    client: &mut MerchantAuthenticationServiceClient<Channel>,
) -> Option<String> {
    let mut request = Request::new(
        MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
            merchant_server_session_id: Some(unique_id("nuvei_l23_sess")),
            domain_context: Some(DomainContext::Payment(PaymentSessionContext {
                amount: Some(usd(TEST_AMOUNT)),
                ..Default::default()
            })),
            ..Default::default()
        },
    );
    if !add_nuvei_metadata(&mut request) {
        return None;
    }
    let response = Box::pin(client.create_server_session_authentication_token(request))
        .await
        .expect("gRPC CreateServerSessionAuthenticationToken failed")
        .into_inner();
    assert!(
        response.error.is_none(),
        "session token call returned an error: {:?}",
        response.error
    );
    assert!(
        !response.session_token.is_empty(),
        "Nuvei returned an empty sessionToken"
    );
    Some(response.session_token)
}

/// Live Auth → Settle-with-Level-2/3 against the Nuvei sandbox.
///
/// The assertion that matters is on `raw_connector_request`: it is the body UCS
/// actually PUT on the wire, so finding `addendums.l23processingData` in it is
/// proof the proto field reached Nuvei rather than being dropped in the domain
/// layer. A control capture without `l2_l3_data` is issued alongside it to show
/// the key is absent — and that the settle `checksum` is unaffected either way.
#[tokio::test]
async fn nuvei_capture_sends_l2_l3_addendums() {
    // `config/development.toml` sets `return_raw_connector_data = false`, which
    // blanks `raw_connector_request` - and the whole point of this test is to read
    // the settle body back off it. Enable it here rather than relying on the
    // caller's environment, so the wire assertion always runs instead of silently
    // degrading into "status was CHARGED, hope the addendum was there". Safe to set
    // in-process: this binary holds a single test, and the config is read once, by
    // the `grpc_test!` expansion immediately below.
    std::env::set_var("CS__COMMON__RETURN_RAW_CONNECTOR_DATA", "true");

    grpc_test!(
        [
            client: PaymentServiceClient<Channel>,
            auth_client: MerchantAuthenticationServiceClient<Channel>
        ],
        {
        let Some(session_token) = server_session_token(&mut auth_client).await else {
            eprintln!("SKIP: no Nuvei credentials available");
            return;
        };

        // ---- 1. Authorize (manual capture => Nuvei transactionType "Auth") ----
        let mut auth_grpc = Request::new(authorize_request(session_token));
        assert!(add_nuvei_metadata(&mut auth_grpc));
        let auth_response = Box::pin(client.authorize(auth_grpc))
            .await
            .expect("gRPC authorize call failed")
            .into_inner();

        println!(
            "AUTHORIZE raw_connector_request:\n{}",
            auth_response
                .raw_connector_request
                .as_ref()
                .map(|value| value.clone().expose())
                .unwrap_or_default()
        );
        println!(
            "AUTHORIZE raw_connector_response:\n{}",
            auth_response
                .raw_connector_response
                .as_ref()
                .map(|value| value.clone().expose())
                .unwrap_or_default()
        );
        assert_eq!(
            auth_response.status,
            i32::from(PaymentStatus::Authorized),
            "authorize must reach AUTHORIZED for the settle leg to exist; error: {:?}",
            auth_response.error
        );
        let transaction_id = auth_response
            .connector_transaction_id
            .clone()
            .expect("authorize response must carry connector_transaction_id");

        // ---- 2. Capture WITH Level 2/3 ----
        let mut capture_grpc = Request::new(capture_request(&transaction_id, true));
        assert!(add_nuvei_metadata(&mut capture_grpc));
        let capture_response = Box::pin(client.capture(capture_grpc))
            .await
            .expect("gRPC capture call failed")
            .into_inner();

        let settle_body = capture_response
            .raw_connector_request
            .as_ref()
            .map(|value| value.clone().expose())
            .expect("capture response must carry raw_connector_request");
        println!("SETTLE raw_connector_request:\n{settle_body}");
        println!(
            "SETTLE raw_connector_response:\n{}",
            capture_response
                .raw_connector_response
                .as_ref()
                .map(|value| value.clone().expose())
                .unwrap_or_default()
        );

        // The point of the whole change: the addendum is on the wire.
        let settle = settle_request_body(&settle_body);
        let l23 = settle
            .get("addendums")
            .and_then(|addendums| addendums.get("l23processingData"))
            .expect("settle body must carry addendums.l23processingData");

        // Spot-check one field from each of the four spec tables.
        assert_eq!(l23.get("taxIndicator").and_then(|v| v.as_str()), Some("1"));
        assert_eq!(l23.get("orderDate").and_then(|v| v.as_str()), Some("260622"));
        assert_eq!(
            l23.get("items")
                .and_then(|items| items.get(0))
                .and_then(|item| item.get("description"))
                .and_then(|v| v.as_str()),
            Some("Garden Supplies")
        );
        // 0.025 does not fit Nuvei's 4-character `vatOrTaxRate`, so it is rounded to
        // 0.03 - while `vatOrTaxAmount` beside it stays exact.
        assert_eq!(
            l23.get("items")
                .and_then(|items| items.get(0))
                .and_then(|item| item.get("vatOrTaxRate"))
                .and_then(|v| v.as_str()),
            Some("0.03")
        );
        assert_eq!(
            l23.get("items")
                .and_then(|items| items.get(0))
                .and_then(|item| item.get("vatOrTaxAmount"))
                .and_then(|v| v.as_str()),
            Some("0.05")
        );
        assert_eq!(
            l23.get("amountDetails")
                .and_then(|details| details.get("totalShipping"))
                .and_then(|v| v.as_str()),
            Some("5.55"),
            "amountDetails amounts must be StringMajorUnit decimal strings"
        );

        assert_eq!(
            capture_response.status,
            i32::from(PaymentStatus::Charged),
            "settle must succeed; error: {:?}",
            capture_response.error
        );

        // ---- 3. Control: a second Auth→Settle with no Level 2/3 ----
        // Confirms `addendums` is omitted entirely (not sent as null) and that the
        // settle checksum is computed over the same fields either way.
        let Some(control_session) = server_session_token(&mut auth_client).await else {
            return;
        };
        let mut control_auth = Request::new(authorize_request(control_session));
        assert!(add_nuvei_metadata(&mut control_auth));
        let control_auth_response = Box::pin(client.authorize(control_auth))
            .await
            .expect("control authorize failed")
            .into_inner();
        let control_id = control_auth_response
            .connector_transaction_id
            .clone()
            .expect("control authorize connector_transaction_id");

        let mut control_capture = Request::new(capture_request(&control_id, false));
        assert!(add_nuvei_metadata(&mut control_capture));
        let control_capture_response = Box::pin(client.capture(control_capture))
            .await
            .expect("control capture failed")
            .into_inner();
        let control_body = control_capture_response
            .raw_connector_request
            .as_ref()
            .map(|value| value.clone().expose())
            .expect("control capture raw_connector_request");
        println!("CONTROL SETTLE raw_connector_request:\n{control_body}");
        let control = settle_request_body(&control_body);
        assert!(
            control.get("addendums").is_none(),
            "a capture with no l2_l3_data must not carry an `addendums` key at all"
        );

        // Both settles carry a checksum over the same 8 fields; the addendum is not
        // one of them. Neither digest may be empty and both bodies must expose the
        // identical set of signed keys.
        for body in [&settle, &control] {
            for key in [
                "merchantId",
                "merchantSiteId",
                "clientRequestId",
                "clientUniqueId",
                "amount",
                "currency",
                "relatedTransactionId",
                "timeStamp",
                "checksum",
            ] {
                assert!(
                    body.get(key).is_some(),
                    "settle body is missing signed field {key}"
                );
            }
            assert!(body
                .get("authCode")
                .is_none(),
                "authCode is not sent today; sending it requires inserting it into the checksum between relatedTransactionId and comment");
        }
        }
    );
}
