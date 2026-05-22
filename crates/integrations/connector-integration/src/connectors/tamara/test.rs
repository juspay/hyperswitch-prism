#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use common_enums::{AttemptStatus, RefundStatus};
    use domain_types::{
        connector_types::{EventType, HttpMethod, RequestDetails},
        payment_method_data::DefaultPCIHolder,
    };
    use interfaces::{api::ConnectorCommon, connector_types::IncomingWebhook};

    use crate::connectors;

    fn make_request(body: &[u8]) -> RequestDetails {
        RequestDetails {
            method: HttpMethod::Post,
            uri: None,
            headers: HashMap::new(),
            body: body.to_vec(),
            query_params: None,
        }
    }

    fn tamara_webhook(event_type: &str, order_id: &str) -> Vec<u8> {
        format!(
            r#"{{
                "order_id": "{order_id}",
                "order_reference_id": "4464602579098",
                "order_number": "90001860",
                "event_type": "{event_type}",
                "data": []
            }}"#
        )
        .into_bytes()
    }

    macro_rules! ensure_eq {
        ($left:expr, $right:expr $(,)?) => {{
            let left = &$left;
            let right = &$right;
            if left != right {
                return Err(format!("assertion failed: {left:?} != {right:?}").into());
            }
        }};
        ($left:expr, $right:expr, $($msg:tt)+) => {{
            let left = &$left;
            let right = &$right;
            if left != right {
                return Err(
                    format!("{}: {left:?} != {right:?}", format_args!($($msg)+)).into(),
                );
            }
        }};
    }

    macro_rules! ensure {
        ($cond:expr $(,)?) => {{
            if !($cond) {
                return Err(concat!("assertion failed: ", stringify!($cond)).into());
            }
        }};
        ($cond:expr, $($msg:tt)+) => {{
            if !($cond) {
                return Err(format!($($msg)+).into());
            }
        }};
    }

    // ── Connector Setup ───────────────────────────────────────────────────────

    #[test]
    fn test_tamara_connector_creation() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        assert_eq!(connector.id(), "tamara");
    }

    #[test]
    fn test_tamara_currency_unit() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        assert!(matches!(
            connector.get_currency_unit(),
            common_enums::CurrencyUnit::Minor
        ));
    }

    #[test]
    fn test_tamara_content_type() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        assert_eq!(connector.common_get_content_type(), "application/json");
    }

    // ── Webhook: sample_webhook_body ──────────────────────────────────────────

    #[test]
    fn test_sample_webhook_body_is_valid_json() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = connector.sample_webhook_body();
        let parsed: serde_json::Value =
            serde_json::from_slice(body).expect("sample_webhook_body should be valid JSON");
        assert!(parsed.is_object(), "sample body should be a JSON object");
        assert!(
            parsed.get("order_id").is_some(),
            "sample body should contain order_id"
        );
        assert!(
            parsed.get("event_type").is_some(),
            "sample body should contain event_type"
        );
    }

    // ── Webhook: get_event_type ───────────────────────────────────────────────

    #[test]
    fn test_webhook_event_type_order_approved() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_approved", "order-001");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentAuthorizationSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_order_authorised() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_authorised", "order-002");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentAuthorizationSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_order_canceled() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_canceled", "order-003");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCancelled);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_order_captured() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_captured", "order-004");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCaptureSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_order_refunded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_refunded", "order-005");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::RefundSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_order_updated() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_updated", "order-006");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentProcessing);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_unknown() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_something_else", "order-007");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::IncomingWebhookEventUnspecified);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_invalid_body() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let result = connector.get_event_type(make_request(b"not-valid-json"));
        assert!(result.is_err(), "invalid JSON should return an error");
    }

    // ── Webhook: get_webhook_event_reference ──────────────────────────────────

    #[test]
    fn test_webhook_event_reference() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_approved", "order-ref-123");
        let reference = connector.get_webhook_event_reference(make_request(&body))?;
        ensure!(reference.is_some(), "reference should be present");
        Ok(())
    }

    #[test]
    fn test_webhook_event_reference_invalid_body() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let result = connector.get_webhook_event_reference(make_request(b"{}"));
        assert!(result.is_err(), "missing order_id should return an error");
    }

    // ── Webhook: process_payment_webhook ─────────────────────────────────────

    #[test]
    fn test_process_payment_webhook_approved() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_approved", "order-approved-1");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            AttemptStatus::Authorized,
            "order_approved should map to Authorized"
        );
        ensure!(
            details.resource_id.is_some(),
            "resource_id should be set from order_id"
        );
        ensure!(
            details.raw_connector_response.is_some(),
            "raw_connector_response should be populated"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_authorised() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_authorised", "order-auth-1");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            AttemptStatus::Authorized,
            "order_authorised should map to Authorized"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_captured() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_captured", "order-captured-1");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            AttemptStatus::Charged,
            "order_captured should map to Charged"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_canceled() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_canceled", "order-canceled-1");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            AttemptStatus::Voided,
            "order_canceled should map to Voided"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_updated() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_updated", "order-updated-1");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            AttemptStatus::Pending,
            "order_updated should map to Pending"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_invalid_body() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let result = connector.process_payment_webhook(make_request(b"bad"), None, None, None);
        assert!(result.is_err(), "invalid body should return an error");
    }

    // ── Webhook: process_refund_webhook ──────────────────────────────────────

    #[test]
    fn test_process_refund_webhook_refunded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_refunded", "order-refund-1");
        let details = connector.process_refund_webhook(make_request(&body), None, None)?;
        ensure_eq!(
            details.status,
            RefundStatus::Success,
            "order_refunded should map to RefundStatus::Success"
        );
        ensure!(
            details.raw_connector_response.is_some(),
            "raw_connector_response should be populated"
        );
        Ok(())
    }

    #[test]
    fn test_process_refund_webhook_non_refund_event() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_updated", "order-refund-2");
        let details = connector.process_refund_webhook(make_request(&body), None, None)?;
        ensure_eq!(
            details.status,
            RefundStatus::Pending,
            "non-refund event should map to RefundStatus::Pending"
        );
        Ok(())
    }

    // ── Webhook: verify_webhook_source (should error — requires external PSync) ─

    #[test]
    fn test_verify_webhook_source_returns_error() {
        let connector = connectors::tamara::Tamara::<DefaultPCIHolder>::new();
        let body = tamara_webhook("order_approved", "order-verify-1");
        let result = connector.verify_webhook_source(make_request(&body), None, None);
        assert!(
            result.is_err(),
            "verify_webhook_source should error since Tamara requires external PSync verification"
        );
    }
}
