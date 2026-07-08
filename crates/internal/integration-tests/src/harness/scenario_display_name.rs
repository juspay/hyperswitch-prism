use std::collections::BTreeMap;

fn known_subject_display_name(subject_key: &str) -> Option<&'static str> {
    match subject_key {
        "google_pay_encrypted" => Some("Google Pay (Encrypted Token)"),
        "credit_card" => Some("Credit Card"),
        "debit_card" => Some("Debit Card"),
        "ideal" => Some("iDEAL"),
        "giropay" => Some("Giropay"),
        "bancontact" => Some("Bancontact"),
        "klarna" => Some("Klarna"),
        "affirm" => Some("Affirm"),
        "afterpay_clearpay" => Some("Afterpay/Clearpay"),
        "przelewy24" => Some("Przelewy24"),
        "alipay" => Some("Alipay"),
        "eps" => Some("EPS"),
        "sepa_bank_transfer" => Some("SEPA Bank Transfer"),
        "ach_bank_transfer" => Some("ACH Bank Transfer"),
        "bacs_bank_transfer" => Some("BACS Bank Transfer"),
        "merchant_order_id" => Some("Merchant Order ID Reference"),
        "full_amount" => Some("Full Amount"),
        "partial_amount" => Some("Partial Amount"),
        "fail_payment" => Some("Payment Failure"),
        _ => None,
    }
}

fn title_case_token(token: &str) -> String {
    match token {
        "id" => "ID".to_string(),
        "api" => "API".to_string(),
        "sdk" => "SDK".to_string(),
        "ach" => "ACH".to_string(),
        "bacs" => "BACS".to_string(),
        "sepa" => "SEPA".to_string(),
        "eps" => "EPS".to_string(),
        "ideal" => "iDEAL".to_string(),
        "threeds" | "3ds" => "3DS".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
            }
        }
    }
}

fn title_case_phrase(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| title_case_token(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_prefix_tokens(tokens: &mut Vec<&str>, prefix_tokens: &[&str]) {
    if prefix_tokens.is_empty() || tokens.len() < prefix_tokens.len() {
        return;
    }
    if tokens
        .iter()
        .zip(prefix_tokens.iter())
        .all(|(left, right)| left == right)
    {
        tokens.drain(..prefix_tokens.len());
    }
}

fn subject_from_tokens(tokens: &[&str]) -> String {
    let key = tokens.join("_");
    if let Some(known) = known_subject_display_name(&key) {
        return known.to_string();
    }
    title_case_phrase(tokens)
}

fn suite_label(suite: &str) -> String {
    let flow = suite.split('/').nth(1).unwrap_or(suite);
    let mut label = String::new();
    for (i, ch) in flow.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            label.push(' ');
        }
        label.push(ch);
    }
    label
}

/// Generates a style-A display name from scenario key data.
///
/// Format:
/// - Primary: `<Subject> | <Auth Type> | <Capture Mode>`
/// - For suite-scoped keys without auth/capture: `<Suite> | <Subject>`
pub fn generate_style_a_display_name(suite: &str, scenario: &str) -> String {
    let mut tokens: Vec<&str> = scenario
        .split('_')
        .filter(|part| !part.is_empty())
        .collect();
    if tokens.is_empty() {
        return suite_label(suite);
    }

    let auth_type = match tokens.first().copied() {
        Some("no3ds") => {
            tokens.remove(0);
            Some("No 3DS")
        }
        Some("threeds") | Some("3ds") => {
            tokens.remove(0);
            Some("3DS")
        }
        _ => None,
    };

    let capture_mode = if tokens.starts_with(&["auto", "capture"]) {
        tokens.drain(..2);
        Some("Automatic Capture")
    } else if tokens.starts_with(&["manual", "capture"]) {
        tokens.drain(..2);
        Some("Manual Capture")
    } else {
        None
    };

    let flow = suite.split('/').nth(1).unwrap_or(suite);
    let suite_tokens: Vec<String> = flow.chars().fold(Vec::new(), |mut words: Vec<String>, ch| {
        if ch.is_uppercase() || words.is_empty() {
            words.push(ch.to_lowercase().to_string());
        } else {
            words.last_mut().unwrap().push(ch);
        }
        words
    });
    let suite_token_refs: Vec<&str> = suite_tokens.iter().map(|s| s.as_str()).collect();
    strip_prefix_tokens(&mut tokens, &suite_token_refs);
    strip_prefix_tokens(&mut tokens, &["with"]);

    let subject = if tokens.is_empty() {
        suite_label(suite)
    } else {
        subject_from_tokens(&tokens)
    };

    let mut parts = Vec::new();
    if auth_type.is_none() && capture_mode.is_none() {
        parts.push(suite_label(suite));
        if subject != suite_label(suite) {
            parts.push(subject);
        }
    } else {
        parts.push(subject);
        if let Some(auth) = auth_type {
            parts.push(auth.to_string());
        }
        if let Some(capture) = capture_mode {
            parts.push(capture.to_string());
        }
    }

    parts.join(" | ")
}

pub fn generate_suite_display_names(
    suite: &str,
    scenario_names: &[String],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for scenario_name in scenario_names {
        map.insert(
            scenario_name.clone(),
            generate_style_a_display_name(suite, scenario_name),
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::generate_style_a_display_name;

    #[test]
    fn authorize_style_a_name_uses_subject_auth_capture_order() {
        let name = generate_style_a_display_name(
            "PaymentService/Authorize",
            "no3ds_auto_capture_google_pay_encrypted",
        );
        assert_eq!(
            name,
            "Google Pay (Encrypted Token) | No 3DS | Automatic Capture"
        );
    }

    #[test]
    fn authorize_manual_capture_threeds_is_rendered() {
        let name = generate_style_a_display_name(
            "PaymentService/Authorize",
            "threeds_manual_capture_credit_card",
        );
        assert_eq!(name, "Credit Card | 3DS | Manual Capture");
    }

    #[test]
    fn capture_merchant_order_id_name_is_human_readable() {
        let name = generate_style_a_display_name(
            "PaymentService/Capture",
            "capture_with_merchant_order_id",
        );
        assert_eq!(name, "Capture | Merchant Order ID Reference");
    }
}
