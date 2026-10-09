//! Lean-generated vectors for PaymentService/Authorize · No3DS · card.
//!
//! The vectors and their expected outcomes come from the Lean spec in
//! `tests/lean/PrismSpec/Authorize/No3dsCard/` (regenerate: `cd tests/lean && lake build && lake exe gen`).
//! Each vector is pushed through the same conversion the gRPC server runs for Authorize
//! (`AuthorizationRequest::from` → card extraction → `PaymentsAuthorizeData::foreign_try_from`).
//!
//! Rules the spec enforces but UCS does not yet are listed in `KNOWN_GAPS`. The test fails
//! if a gap closes without being removed from the list, so the list cannot go stale.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use domain_types::{
    connector_types::PaymentsAuthorizeData,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData},
    types::{build_request_data_with_required_pmd, AuthorizationRequest},
};
use grpc_api_types::payments::{
    payment_method, AuthenticationData, AuthenticationType, CaptureMethod, CardDetails, Currency,
    Money, PaymentMethod, PaymentServiceAuthorizeRequest,
};
use hyperswitch_masking::{PeekInterface, Secret};
use prost::Message;
use serde::Deserialize;

const VECTORS: &str = include_str!("../../vectors/authorize_no3ds_card.json");

/// Spec rules UCS does not enforce today: a request breaking one of these is accepted.
/// Remove an entry once UCS starts rejecting it.
const KNOWN_GAPS: &[&str] = &[
    // `MinorUnit::new` takes any i64
    "amount_non_positive",
    // authentication_data is forwarded even when auth_type is NO_THREE_DS
    "authentication_data_with_no3ds",
    // the prost decoder for `CardNumber` stores the string without `sanitize_card_number`
    "card_number_non_digit",
    "card_number_length",
    "card_number_luhn",
    // expiry and CVC are opaque `Secret<String>` passthroughs
    "exp_month_invalid",
    "exp_year_invalid",
    "cvc_non_digit",
    "cvc_length_for_network",
];

#[derive(Deserialize)]
struct Document {
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
struct Vector {
    id: String,
    rule: Option<String>,
    request: RawRequest,
    expected: Option<Expected>,
}

#[derive(Deserialize)]
struct RawRequest {
    merchant_transaction_id: Option<String>,
    amount: Option<RawMoney>,
    capture_method: String,
    auth_type: String,
    authentication_data: bool,
    card: Option<RawCard>,
}

#[derive(Deserialize)]
struct RawMoney {
    minor_amount: i64,
    currency: String,
}

#[derive(Deserialize)]
struct RawCard {
    card_number: Option<String>,
    card_exp_month: Option<String>,
    card_exp_year: Option<String>,
    card_cvc: Option<String>,
    card_holder_name: Option<String>,
}

#[derive(Deserialize)]
struct Expected {
    minor_amount: i64,
    currency: String,
    capture_method: String,
}

/// Build a `CardNumber` the way the gRPC wire decoder does (`impl prost::Message for
/// CardNumber`): the raw string is stored as-is, with no Luhn/length/charset validation.
/// `CardNumber::from_str` would reject invalid numbers before UCS ever saw them.
fn card_number_from_wire(value: &str) -> cards::CardNumber {
    let mut buf = Vec::new();
    prost::encoding::string::encode(1, &value.to_string(), &mut buf);
    cards::CardNumber::decode(buf.as_slice()).unwrap()
}

fn currency(code: &str) -> Currency {
    match code {
        "UNSPECIFIED" => Currency::Unspecified,
        other => {
            Currency::from_str_name(other).unwrap_or_else(|| panic!("unknown currency {other}"))
        }
    }
}

fn build_request(raw: &RawRequest) -> PaymentServiceAuthorizeRequest {
    let capture_method = match raw.capture_method.as_str() {
        "UNSPECIFIED" => None,
        other => Some(i32::from(
            CaptureMethod::from_str_name(other)
                .unwrap_or_else(|| panic!("unknown capture_method {other}")),
        )),
    };
    let auth_type = match raw.auth_type.as_str() {
        "UNSPECIFIED" => AuthenticationType::Unspecified,
        other => AuthenticationType::from_str_name(other)
            .unwrap_or_else(|| panic!("unknown auth_type {other}")),
    };
    let payment_method = raw.card.as_ref().map(|card| PaymentMethod {
        payment_method: Some(payment_method::PaymentMethod::Card(CardDetails {
            card_number: card.card_number.as_deref().map(card_number_from_wire),
            card_exp_month: card.card_exp_month.clone().map(Secret::new),
            card_exp_year: card.card_exp_year.clone().map(Secret::new),
            card_cvc: card.card_cvc.clone().map(Secret::new),
            card_holder_name: card.card_holder_name.clone().map(Secret::new),
            ..Default::default()
        })),
    });

    PaymentServiceAuthorizeRequest {
        merchant_transaction_id: raw.merchant_transaction_id.clone(),
        amount: raw.amount.as_ref().map(|m| Money {
            minor_amount: m.minor_amount,
            currency: i32::from(currency(&m.currency)),
        }),
        capture_method,
        auth_type: i32::from(auth_type),
        authentication_data: raw.authentication_data.then(AuthenticationData::default),
        payment_method,
        ..Default::default()
    }
}

/// The Authorize request conversion as run by the gRPC server (server/payments.rs).
fn convert(
    req: PaymentServiceAuthorizeRequest,
) -> Result<PaymentsAuthorizeData<DefaultPCIHolder>, String> {
    build_request_data_with_required_pmd::<AuthorizationRequest, PaymentsAuthorizeData<_>>(
        req.payment_method.clone(),
        AuthorizationRequest::from(req),
    )
    .map_err(|e| format!("{:?}", e.current_context()))
}

/// Accepted request: spec-normalized values must come out, card fields must pass through.
fn check_accepted(v: &Vector, data: &PaymentsAuthorizeData<DefaultPCIHolder>) -> Vec<String> {
    let mut problems = Vec::new();
    let expected = v
        .expected
        .as_ref()
        .expect("valid vector has expected values");

    if data.amount.get_amount_as_i64() != expected.minor_amount {
        problems.push(format!(
            "amount {} != {}",
            data.amount.get_amount_as_i64(),
            expected.minor_amount
        ));
    }
    if data.currency.to_string() != expected.currency {
        problems.push(format!(
            "currency {} != {}",
            data.currency, expected.currency
        ));
    }
    let capture = data.capture_method.map(|c| format!("{c:?}"));
    if capture.as_deref() != Some(expected.capture_method.as_str()) {
        problems.push(format!(
            "capture_method {capture:?} != {}",
            expected.capture_method
        ));
    }

    let raw = v.request.card.as_ref().expect("valid vector has a card");
    match &data.payment_method_data {
        PaymentMethodData::Card(card) => {
            let pairs = [
                (
                    "card_number",
                    Some(card.card_number.peek()),
                    raw.card_number.as_deref(),
                ),
                (
                    "card_exp_month",
                    Some(card.card_exp_month.peek().as_str()),
                    raw.card_exp_month.as_deref(),
                ),
                (
                    "card_exp_year",
                    Some(card.card_exp_year.peek().as_str()),
                    raw.card_exp_year.as_deref(),
                ),
                (
                    "card_cvc",
                    Some(card.card_cvc.peek().as_str()),
                    raw.card_cvc.as_deref(),
                ),
                (
                    "card_holder_name",
                    card.card_holder_name.as_ref().map(|h| h.peek().as_str()),
                    raw.card_holder_name.as_deref(),
                ),
            ];
            for (field, got, want) in pairs {
                if got != want {
                    problems.push(format!("{field} not passed through"));
                }
            }
        }
        _ => problems.push("payment_method_data is not Card".to_string()),
    }
    problems
}

#[test]
fn authorize_no3ds_card_matches_lean_spec() {
    let doc: Document = serde_json::from_str(VECTORS).expect("vector file parses");
    assert!(!doc.vectors.is_empty());

    let mut failures = Vec::new();
    // rule -> vector ids UCS accepted although the spec rejects them
    let mut gaps_seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rules_rejected: BTreeMap<String, bool> = BTreeMap::new();

    for v in &doc.vectors {
        let result = convert(build_request(&v.request));
        match (&v.rule, result) {
            (None, Ok(data)) => {
                for p in check_accepted(v, &data) {
                    failures.push(format!("{}: {p}", v.id));
                }
            }
            (None, Err(e)) => failures.push(format!("{}: spec accepts, UCS rejected: {e}", v.id)),
            (Some(rule), Ok(_)) => {
                gaps_seen
                    .entry(rule.clone())
                    .or_default()
                    .push(v.id.clone());
                rules_rejected.entry(rule.clone()).or_insert(false);
            }
            (Some(rule), Err(_)) => {
                rules_rejected.insert(rule.clone(), true);
            }
        }
    }

    for (rule, ids) in &gaps_seen {
        if !KNOWN_GAPS.contains(&rule.as_str()) {
            failures.push(format!(
                "rule `{rule}`: spec rejects, UCS accepted {ids:?} (fix UCS or add to KNOWN_GAPS)"
            ));
        }
    }
    for gap in KNOWN_GAPS {
        match rules_rejected.get(*gap) {
            Some(true) if !gaps_seen.contains_key(*gap) => failures.push(format!(
                "rule `{gap}` is now enforced by UCS: remove it from KNOWN_GAPS"
            )),
            None => failures.push(format!("KNOWN_GAPS entry `{gap}` has no vector")),
            _ => {}
        }
    }

    assert!(
        failures.is_empty(),
        "{} mismatch(es) against the Lean spec:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
