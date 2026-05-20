use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    state::AppState,
    types::{Charge, IntentStatus, StripeErrorObject},
};

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
pub struct RedirectQuery {
    #[serde(default)]
    pub reject: Option<u8>,
    #[serde(default)]
    pub manual: Option<u8>,
}

pub async fn page(
    Path(attempt_id): Path<String>,
    Query(q): Query<RedirectQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Snapshot the attempt outcome and drop the shard lock before touching payment_intents.
    let Some(att) = state.attempts.get(&attempt_id).map(|e| e.value().clone()) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html("<h1>Unknown attempt</h1>".to_string()),
        )
            .into_response();
    };

    let rejecting = q.reject == Some(1);
    let final_status = if rejecting {
        IntentStatus::Failed.as_str()
    } else {
        // on_complete_status is a free-form string from AttemptOutcome (default "succeeded").
        att.on_complete_status.as_str()
    };
    let err_msg = if rejecting {
        Some("User rejected at redirect page.".to_string())
    } else {
        att.error_message.clone()
    };

    // Mutate the underlying PaymentIntent.
    if let Some(mut pi_entry) = state.payment_intents.get_mut(&att.payment_intent_id) {
        let pi = &mut pi_entry.0;
        pi.status = final_status.to_string();
        pi.next_action = None;
        if rejecting {
            pi.last_payment_error = Some(StripeErrorObject {
                kind: "card_error".into(),
                code: "redirect_rejected".into(),
                message: err_msg.clone().unwrap_or_default(),
                decline_code: None,
            });
        } else if final_status == IntentStatus::Succeeded.as_str() {
            if let Some(ch) = pi.latest_charge.as_mut() {
                ch.status = IntentStatus::Succeeded.as_str().to_string();
                ch.paid = true;
                ch.captured = true;
                ch.amount_captured = ch.amount;
            } else {
                pi.latest_charge = Some(Charge {
                    id: format!("ch_{}", Uuid::new_v4().simple()),
                    object: "charge",
                    amount: pi.amount,
                    amount_captured: pi.amount,
                    status: IntentStatus::Succeeded.as_str().to_string(),
                    paid: true,
                    captured: true,
                    payment_intent: pi.id.clone(),
                });
            }
        }
    }

    let manual = q.manual == Some(1);
    let body = if manual {
        let attempt_id_esc = html_escape(&attempt_id);
        let final_status_esc = html_escape(final_status);
        format!(
            r#"<!doctype html><html><body><h1>mock redirect ({attempt_id_esc})</h1>
<p>Outcome recorded: <b>{final_status_esc}</b>.</p>
<a href="?manual=1">Refresh</a>
</body></html>"#
        )
    } else {
        let attempt_id_esc = html_escape(&attempt_id);
        let final_status_esc = html_escape(final_status);
        format!(
            r#"<!doctype html><html><head>
<meta http-equiv="refresh" content="0;url=about:blank">
</head><body>
<p>mock: completed attempt <code>{attempt_id_esc}</code> as <b>{final_status_esc}</b>.</p>
</body></html>"#
        )
    };
    Html(body).into_response()
}
