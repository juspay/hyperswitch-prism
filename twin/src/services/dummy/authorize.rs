//! Dummy backend — Authorize.
//!
//! Takes a Stripe-shape AuthorizeReq, dispatches via the scenario tables in
//! `crate::scenarios`, mutates `AppState`, returns a Stripe-shape `PaymentIntent`.

use std::time::Instant;

use uuid::Uuid;

use crate::handlers::authorize::AuthorizeReq;
use crate::scenarios::{classify_card, classify_upi, is_redirect_pm, Outcome};
use crate::state::{AppState, AttemptOutcome};
use crate::types::{
    Charge, IntentStatus, NextAction, PaymentIntent, RedirectToUrl, StripeErrorObject,
};

pub fn create(state: &AppState, req: &AuthorizeReq) -> PaymentIntent {
    let pi_id = format!("pi_{}", Uuid::new_v4().simple());
    let now = chrono::Utc::now().timestamp();
    let capture_method = req
        .capture_method
        .clone()
        .unwrap_or_else(|| "automatic".to_string());

    let pm = req.payment_method_data.as_ref();
    let pm_kind = pm.and_then(|p| p.kind.as_deref()).unwrap_or("");

    let return_url = req.return_url.as_deref();
    let (status, next_action, last_err, attempt_record) =
        if let Some(card) = pm.and_then(|p| p.card.as_ref()) {
            match classify_card(&card.number) {
                Outcome::Succeed => succeed_or_capture(&capture_method),
                Outcome::Decline { message, code } => failed(&message, &code),
                Outcome::RequiresAction => requires_action(&pi_id, "succeeded", None, return_url),
            }
        } else if let Some(upi) = pm.and_then(|p| p.upi.as_ref()) {
            match classify_upi(&upi.vpa) {
                Outcome::Succeed => succeed_or_capture(&capture_method),
                Outcome::Decline { message, code } => failed(&message, &code),
                Outcome::RequiresAction => unreachable!("UPI never returns RequiresAction"),
            }
        } else if is_redirect_pm(pm_kind) {
            requires_action(&pi_id, "succeeded", None, return_url)
        } else {
            failed(
                "Unsupported payment method.",
                "payment_method_not_supported",
            )
        };

    if let Some(att) = attempt_record {
        state.attempts.insert(att.0.clone(), att.1);
    }

    let is_succeeded = status == IntentStatus::Succeeded.as_str();
    let needs_charge = is_succeeded || status == IntentStatus::RequiresCapture.as_str();
    let charge = needs_charge.then(|| Charge {
        id: format!("ch_{}", Uuid::new_v4().simple()),
        object: "charge",
        amount: req.amount,
        amount_captured: if is_succeeded { req.amount } else { 0 },
        status: status.clone(),
        paid: is_succeeded,
        captured: is_succeeded,
        payment_intent: pi_id.clone(),
    });

    let pi = PaymentIntent {
        id: pi_id.clone(),
        object: "payment_intent",
        status,
        amount: req.amount,
        currency: req.currency.clone(),
        capture_method,
        client_secret: Some(format!("{}_secret_{}", pi_id, Uuid::new_v4().simple())),
        next_action,
        latest_charge: charge,
        last_payment_error: last_err,
        created: now,
        metadata: req.metadata.clone(),
    };

    state
        .payment_intents
        .insert(pi_id.clone(), (pi.clone(), Instant::now()));
    pi
}

type AuthResult = (
    String,
    Option<NextAction>,
    Option<StripeErrorObject>,
    Option<(String, AttemptOutcome)>,
);

fn succeed_or_capture(capture_method: &str) -> AuthResult {
    let status = if capture_method == "manual" {
        IntentStatus::RequiresCapture
    } else {
        IntentStatus::Succeeded
    };
    (status.as_str().to_string(), None, None, None)
}

fn failed(message: &str, code: &str) -> AuthResult {
    (
        IntentStatus::Failed.as_str().to_string(),
        None,
        Some(StripeErrorObject {
            kind: "card_error".into(),
            code: code.into(),
            message: message.into(),
            decline_code: Some(code.into()),
        }),
        None,
    )
}

fn requires_action(
    pi_id: &str,
    on_complete: &str,
    err: Option<String>,
    return_url: Option<&str>,
) -> AuthResult {
    let attempt_id = format!("att_{}", Uuid::new_v4().simple());
    let host = std::env::var("TWIN_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:8777".into());
    let url = format!("{host}/dummy/redirect/{attempt_id}");
    let record = AttemptOutcome {
        payment_intent_id: pi_id.to_string(),
        on_complete_status: on_complete.to_string(),
        error_message: err,
    };
    (
        IntentStatus::RequiresAction.as_str().to_string(),
        Some(NextAction {
            kind: "redirect_to_url".into(),
            redirect_to_url: RedirectToUrl {
                url,
                return_url: return_url.unwrap_or("about:blank").to_string(),
            },
        }),
        None,
        Some((attempt_id, record)),
    )
}
