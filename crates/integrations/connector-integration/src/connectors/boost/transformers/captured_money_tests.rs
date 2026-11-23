use super::*;

#[test]
fn captured_money_is_present_only_for_succeeded_webhooks() {
    for (status, captured) in [
        ("succeeded", true),
        ("failed", false),
        ("denied", false),
        ("error", false),
        ("canceled", false),
        ("expired", false),
        ("processing", false),
        ("pending_payment_method", false),
        ("pending_confirmation", false),
        ("awaiting_confirmation", false),
        ("unrecognized_status", false),
    ] {
        let payload: BoostWebhookBody = serde_json::from_value(serde_json::json!({
            "uuid": "payment-uuid", "referenceId": "merchant-reference",
            "status": status, "amount": 12.34, "currency": "MYR",
        }))
        .expect("valid webhook fixture");
        let expected_status = AttemptStatus::from(payload.status.clone());
        let response = payload.into_webhook_details_response(200, b"{}");
        assert_eq!(response.status, expected_status);
        assert_eq!(
            response.captured_money.is_some(),
            captured,
            "status: {status}"
        );
        if let Some(money) = response.captured_money {
            assert_eq!(money.amount().get_amount_as_i64(), 1234);
            assert_eq!(money.currency(), Currency::MYR);
        }
    }
}

#[test]
fn succeeded_webhooks_without_amount_or_currency_have_no_captured_money() {
    for fields in [
        serde_json::json!({"amount": 12.34}),
        serde_json::json!({"currency": "MYR"}),
    ] {
        let mut json = serde_json::json!({
            "uuid": "payment-uuid", "referenceId": "merchant-reference", "status": "succeeded",
        });
        json.as_object_mut()
            .expect("object")
            .extend(fields.as_object().expect("object").clone());
        let payload: BoostWebhookBody = serde_json::from_value(json).expect("webhook");
        assert!(payload
            .into_webhook_details_response(200, b"{}")
            .captured_money
            .is_none());
    }
}
