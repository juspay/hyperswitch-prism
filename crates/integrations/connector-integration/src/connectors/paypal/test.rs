#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use std::collections::HashMap;

    use domain_types::{
        connector_types::{EventType, HttpMethod, RequestDetails, WebhookResourceReference},
        payment_method_data::DefaultPCIHolder,
    };
    use interfaces::webhooks::IncomingWebhook;

    use crate::connectors::Paypal;

    fn request(body: serde_json::Value) -> RequestDetails {
        RequestDetails {
            method: HttpMethod::Post,
            uri: None,
            headers: HashMap::new(),
            body: body.to_string().into_bytes(),
            query_params: None,
        }
    }

    fn capture_event(event_type: &str) -> RequestDetails {
        request(serde_json::json!({
            "event_type": event_type,
            "resource": {
                "id": "CAPTURE1",
                "status": "DECLINED",
                "amount": { "value": "10.00", "currency_code": "USD" },
                "supplementary_data": { "related_ids": { "order_id": "ORDER1" } }
            }
        }))
    }

    fn refund_event(event_type: &str) -> RequestDetails {
        request(serde_json::json!({
            "event_type": event_type,
            "resource": { "id": "REFUND1", "status": "FAILED" }
        }))
    }

    fn connector() -> &'static Paypal<DefaultPCIHolder> {
        Paypal::<DefaultPCIHolder>::new()
    }

    #[test]
    fn capture_event_reference_is_the_order() {
        match connector()
            .get_webhook_event_reference(capture_event("PAYMENT.CAPTURE.DENIED"))
            .unwrap()
        {
            Some(WebhookResourceReference::Payment(reference)) => {
                assert_eq!(
                    reference.connector_transaction_id.as_deref(),
                    Some("ORDER1")
                )
            }
            other => panic!("expected a payment reference, got {other:?}"),
        }
    }

    #[test]
    fn refund_event_reference_is_the_refund() {
        match connector()
            .get_webhook_event_reference(refund_event("PAYMENT.REFUND.FAILED"))
            .unwrap()
        {
            Some(WebhookResourceReference::Refund(reference)) => {
                assert_eq!(reference.connector_refund_id.as_deref(), Some("REFUND1"))
            }
            other => panic!("expected a refund reference, got {other:?}"),
        }
    }

    #[test]
    fn dispute_event_reference_is_the_dispute() {
        let body = request(serde_json::json!({
            "event_type": "CUSTOMER.DISPUTE.CREATED",
            "resource": {
                "dispute_id": "DISPUTE1",
                "disputed_transactions": [{ "seller_transaction_id": "TXN1" }],
                "dispute_amount": { "value": "10.00", "currency_code": "USD" },
                "dispute_life_cycle_stage": "CHARGEBACK",
                "status": "OPEN"
            }
        }));
        match connector().get_webhook_event_reference(body).unwrap() {
            Some(WebhookResourceReference::Dispute(reference)) => {
                assert_eq!(reference.connector_dispute_id.as_deref(), Some("DISPUTE1"));
                assert_eq!(reference.connector_transaction_id.as_deref(), Some("TXN1"));
            }
            other => panic!("expected a dispute reference, got {other:?}"),
        }
    }

    #[test]
    fn new_event_types_are_mapped() {
        let cases = [
            ("PAYMENT.CAPTURE.DENIED", EventType::PaymentIntentFailure),
            (
                "PAYMENT.AUTHORIZATION.VOIDED",
                EventType::PaymentIntentCancelled,
            ),
            ("PAYMENT.REFUND.PENDING", EventType::RefundProcessing),
            ("PAYMENT.REFUND.FAILED", EventType::RefundFailure),
        ];
        for (event_type, expected) in cases {
            let event = connector()
                .get_event_type(request(serde_json::json!({ "event_type": event_type })))
                .unwrap();
            assert_eq!(event, expected, "{event_type}");
        }
    }

    #[test]
    fn denied_capture_webhook_is_a_failed_payment() {
        let details = connector()
            .process_payment_webhook(capture_event("PAYMENT.CAPTURE.DENIED"), None, None, None)
            .unwrap();
        assert_eq!(details.status, common_enums::AttemptStatus::Failure);
    }

    #[test]
    fn failed_refund_webhook_is_a_failed_refund() {
        let details = connector()
            .process_refund_webhook(refund_event("PAYMENT.REFUND.FAILED"), None, None)
            .unwrap();
        assert_eq!(details.status, common_enums::RefundStatus::Failure);
        assert_eq!(details.connector_refund_id.as_deref(), Some("REFUND1"));
    }
}
