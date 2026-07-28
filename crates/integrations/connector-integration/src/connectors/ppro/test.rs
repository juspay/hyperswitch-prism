#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use domain_types::{
        connector_types::{EventType, HttpMethod, RequestDetails},
        payment_method_data::DefaultPCIHolder,
    };
    use interfaces::{api::ConnectorCommon, connector_types::IncomingWebhook};

    use crate::connectors;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_request(body: &[u8]) -> RequestDetails {
        RequestDetails {
            method: HttpMethod::Post,
            uri: None,
            headers: HashMap::new(),
            body: body.to_vec(),
            query_params: None,
        }
    }

    fn charge_webhook(event_type: &str, status: &str) -> Vec<u8> {
        format!(
            r#"{{
                "specversion": "1.0",
                "type": "{event_type}",
                "source": "https://api.sandbox.eu.ppro.com",
                "id": "evt_test_001",
                "time": "2024-01-01T00:00:00Z",
                "data": {{
                    "paymentChargeId": "pc_test_123",
                    "paymentChargeStatus": "{status}"
                }}
            }}"#
        )
        .into_bytes()
    }

    fn agreement_webhook(event_type: &str, status: &str) -> Vec<u8> {
        format!(
            r#"{{
                "specversion": "1.0",
                "type": "{event_type}",
                "source": "https://api.sandbox.eu.ppro.com",
                "id": "evt_test_001",
                "time": "2024-01-01T00:00:00Z",
                "data": {{
                    "paymentAgreementId": "agr_test_123",
                    "paymentAgreementStatus": "{status}"
                }}
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
    fn test_ppro_connector_creation() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        assert_eq!(connector.id(), "ppro");
    }

    #[test]
    fn test_ppro_currency_unit() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        assert!(matches!(
            connector.get_currency_unit(),
            common_enums::CurrencyUnit::Minor
        ));
    }

    #[test]
    fn test_ppro_content_type() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        assert_eq!(connector.common_get_content_type(), "application/json");
    }

    // ── Webhook: get_event_type ───────────────────────────────────────────────

    #[test]
    fn test_webhook_event_type_capture_succeeded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCaptureSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_charge_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        for event in &[
            "PAYMENT_CHARGE_FAILED",
            "PAYMENT_CHARGE_AUTHORIZATION_FAILED",
            "PAYMENT_CHARGE_DISCARDED",
        ] {
            let body = charge_webhook(event, "FAILED");
            let event_type = connector.get_event_type(make_request(&body))?;
            ensure_eq!(
                event_type,
                EventType::PaymentIntentFailure,
                "expected PaymentIntentFailure for {event}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_authorization_succeeded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        for event in &[
            "PAYMENT_CHARGE_AUTHORIZATION_SUCCEEDED",
            "PAYMENT_CHARGE_SUCCESS",
        ] {
            let body = charge_webhook(event, "SUCCESS");
            let event_type = connector.get_event_type(make_request(&body))?;
            ensure_eq!(
                event_type,
                EventType::PaymentIntentAuthorizationSuccess,
                "expected PaymentIntentAuthorizationSuccess for {event}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_refund_succeeded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_REFUND_SUCCEEDED", "REFUNDED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::RefundSuccess);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_refund_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_REFUND_FAILED", "FAILED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::RefundFailure);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_void_succeeded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_VOID_SUCCEEDED", "VOIDED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCancelled);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_void_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_VOID_FAILED", "FAILED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCancelFailure);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_capture_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_FAILED", "FAILED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::PaymentIntentCaptureFailure);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_mandate_active() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = agreement_webhook("PAYMENT_AGREEMENT_ACTIVE", "ACTIVE");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::MandateActive);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_mandate_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = agreement_webhook("PAYMENT_AGREEMENT_FAILED", "FAILED");
        let event_type = connector.get_event_type(make_request(&body))?;
        ensure_eq!(event_type, EventType::MandateFailed);
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_mandate_revoked() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        for event in &[
            "PAYMENT_AGREEMENT_REVOKED_BY_CONSUMER",
            "PAYMENT_AGREEMENT_REVOKED_BY_MERCHANT",
            "PAYMENT_AGREEMENT_REVOKED_BY_PROVIDER",
        ] {
            let body = agreement_webhook(event, "REVOKED");
            let event_type = connector.get_event_type(make_request(&body))?;
            ensure_eq!(
                event_type,
                EventType::MandateRevoked,
                "expected MandateRevoked for {event}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_webhook_event_type_invalid_body() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let result = connector.get_event_type(make_request(b"not-valid-json"));
        assert!(result.is_err(), "invalid JSON should return an error");
    }

    // ── Webhook: process_payment_webhook ─────────────────────────────────────

    #[test]
    fn test_process_payment_webhook_captured() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            common_enums::AttemptStatus::Charged,
            "CAPTURED charge should map to Charged"
        );
        ensure!(
            details.resource_id.is_some(),
            "resource_id should be set from charge.id"
        );
        ensure!(
            details.raw_connector_response.is_some(),
            "raw_connector_response should be populated"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_failed_with_failure_details(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_CHARGE_FAILED",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_002",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentChargeId": "pc_test_456",
                "paymentChargeStatus": "FAILED",
                "failure": {
                    "failureType": "AUTHORIZATION",
                    "failureCode": "CARD_DECLINED",
                    "failureMessage": "Card was declined"
                }
            }
        }"#
        .as_bytes();
        let details = connector.process_payment_webhook(make_request(body), None, None, None)?;
        ensure_eq!(details.status, common_enums::AttemptStatus::Failure);
        ensure_eq!(
            details.error_code.as_deref(),
            Some("CARD_DECLINED"),
            "error_code should be populated from failure"
        );
        ensure!(
            details.error_message.is_some(),
            "error_message should be populated"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_agreement_returns_error() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_AGREEMENT_ACTIVE",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_003",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentAgreementId": "pa_test_789",
                "paymentAgreementStatus": "ACTIVE"
            }
        }"#
        .as_bytes();
        let result = connector.process_payment_webhook(make_request(body), None, None, None);
        assert!(
            result.is_err(),
            "Agreement webhook data should return an error for process_payment_webhook"
        );
    }

    // ── Webhook: process_refund_webhook ──────────────────────────────────────

    #[test]
    fn test_process_refund_webhook_success() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_CHARGE_REFUND_SUCCEEDED",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_refund_success",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentChargeId": "pc_test_123",
                "paymentChargeStatus": "REFUNDED",
                "refundId": "rf_test_123",
                "merchantRefundReference": "merchant_rf_123"
            }
        }"#
        .as_bytes();
        let details = connector.process_refund_webhook(make_request(body), None, None)?;
        ensure_eq!(
            details.status,
            common_enums::RefundStatus::Success,
            "REFUNDED status should map to RefundStatus::Success"
        );
        ensure!(
            details.connector_refund_id.is_some(),
            "connector_refund_id should be set from charge.refundId"
        );
        Ok(())
    }

    #[test]
    fn test_process_refund_webhook_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_REFUND_FAILED", "FAILED");
        let details = connector.process_refund_webhook(make_request(&body), None, None)?;
        ensure_eq!(
            details.status,
            common_enums::RefundStatus::Failure,
            "FAILED status should map to RefundStatus::Failure"
        );
        Ok(())
    }

    // ── Webhook: verify_webhook_source ───────────────────────────────────────

    /// Helper: compute SHA256(body + "." + secret) and return hex-encoded signature.
    fn sign_body(secret: &[u8], body: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        use common_utils::crypto::GenerateDigest;
        let mut message = body.to_vec();
        message.push(b'.');
        message.extend_from_slice(secret);
        let digest = common_utils::crypto::Sha256
            .generate_digest(&message)
            .map_err(|e| format!("SHA256 digest failed: {e:?}"))?;
        Ok(hex::encode(digest))
    }

    fn make_signed_request(body: &[u8], signature: &str) -> RequestDetails {
        let mut headers = HashMap::new();
        headers.insert("Webhook-Signature".to_string(), signature.to_string());
        RequestDetails {
            method: HttpMethod::Post,
            uri: None,
            headers,
            body: body.to_vec(),
            query_params: None,
        }
    }

    #[test]
    fn test_verify_webhook_source_valid_signature() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let secret = b"my_webhook_secret";
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let signature = sign_body(secret, &body)?;
        let request = make_signed_request(&body, &signature);

        let secrets = domain_types::connector_types::ConnectorWebhookSecrets {
            secret: secret.to_vec(),
            additional_secret: None,
        };

        let result = connector.verify_webhook_source(request, Some(secrets), None)?;
        ensure!(result, "valid SHA256 signature should verify as true");
        Ok(())
    }

    #[test]
    fn test_verify_webhook_source_invalid_signature() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let secret = b"my_webhook_secret";
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        // Sign with a different secret
        let wrong_signature = sign_body(b"wrong_secret", &body)?;
        let request = make_signed_request(&body, &wrong_signature);

        let secrets = domain_types::connector_types::ConnectorWebhookSecrets {
            secret: secret.to_vec(),
            additional_secret: None,
        };

        let result = connector.verify_webhook_source(request, Some(secrets), None)?;
        ensure!(!result, "invalid SHA256 signature should verify as false");
        Ok(())
    }

    #[test]
    fn test_verify_webhook_source_missing_header() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        // No Webhook-Signature header
        let request = make_request(&body);
        let secrets = domain_types::connector_types::ConnectorWebhookSecrets {
            secret: b"my_webhook_secret".to_vec(),
            additional_secret: None,
        };

        let result = connector.verify_webhook_source(request, Some(secrets), None);
        assert!(
            result.is_err(),
            "missing Webhook-Signature header should return an error"
        );
    }

    #[test]
    fn test_verify_webhook_source_no_secret_returns_error() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let request = make_request(&body);

        let result = connector.verify_webhook_source(request, None, None);
        assert!(
            result.is_err(),
            "missing connector_webhook_secret should return NotImplemented error"
        );
    }

    #[test]
    fn test_verify_webhook_source_tampered_body() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let secret = b"my_webhook_secret";
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let signature = sign_body(secret, &body)?;

        // Tamper with the body after signing
        let tampered_body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "FAILED");
        let request = make_signed_request(&tampered_body, &signature);

        let secrets = domain_types::connector_types::ConnectorWebhookSecrets {
            secret: secret.to_vec(),
            additional_secret: None,
        };

        let result = connector.verify_webhook_source(request, Some(secrets), None)?;
        ensure!(!result, "tampered body should fail signature verification");
        Ok(())
    }

    // ── Webhook: get_webhook_resource_object ─────────────────────────────────

    #[test]
    fn test_get_webhook_resource_object_charge() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_CAPTURE_SUCCEEDED", "CAPTURED");
        let result = connector.get_webhook_resource_object(make_request(&body));
        ensure!(
            result.is_ok(),
            "charge webhook should return a valid resource object"
        );
        Ok(())
    }

    #[test]
    fn test_get_webhook_resource_object_agreement() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = agreement_webhook("PAYMENT_AGREEMENT_ACTIVE", "ACTIVE");
        let result = connector.get_webhook_resource_object(make_request(&body));
        ensure!(
            result.is_ok(),
            "agreement webhook should return a valid resource object"
        );
        Ok(())
    }

    #[test]
    fn test_get_webhook_resource_object_invalid_body() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let result = connector.get_webhook_resource_object(make_request(b"not-json"));
        assert!(
            result.is_err(),
            "invalid JSON should fail resource object extraction"
        );
    }

    // ── Webhook: process_payment_webhook — additional statuses ───────────────

    #[test]
    fn test_process_payment_webhook_authorization_success() -> Result<(), Box<dyn std::error::Error>>
    {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_AUTHORIZATION_SUCCEEDED", "SUCCESS");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            common_enums::AttemptStatus::Charged,
            "SUCCESS charge should map to Charged"
        );
        ensure!(
            details.resource_id.is_some(),
            "resource_id should be set from charge.id"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_voided() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_VOID_SUCCEEDED", "VOIDED");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            common_enums::AttemptStatus::Voided,
            "VOIDED charge should map to Voided"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_capture_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_CHARGE_CAPTURE_FAILED",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_cap_fail",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentChargeId": "pc_cap_fail",
                "paymentChargeStatus": "FAILED",
                "failure": {
                    "failureType": "CAPTURE",
                    "failureCode": "CAPTURE_TIMEOUT",
                    "failureMessage": "Capture timed out"
                }
            }
        }"#
        .as_bytes();
        let details = connector.process_payment_webhook(make_request(body), None, None, None)?;
        ensure_eq!(details.status, common_enums::AttemptStatus::Failure);
        ensure_eq!(
            details.error_code.as_deref(),
            Some("CAPTURE_TIMEOUT"),
            "error_code should reflect capture failure"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_discarded() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_DISCARDED", "DISCARDED");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure_eq!(
            details.status,
            common_enums::AttemptStatus::Failure,
            "DISCARDED charge should map to Failure"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_void_failed() -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_CHARGE_VOID_FAILED",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_void_fail",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentChargeId": "pc_void_fail",
                "paymentChargeStatus": "FAILED",
                "failure": {
                    "failureType": "VOID",
                    "failureCode": "VOID_NOT_ALLOWED",
                    "failureMessage": "Void not allowed"
                }
            }
        }"#
        .as_bytes();
        let details = connector.process_payment_webhook(make_request(body), None, None, None)?;
        ensure_eq!(details.status, common_enums::AttemptStatus::Failure);
        ensure_eq!(details.error_code.as_deref(), Some("VOID_NOT_ALLOWED"),);
        ensure!(
            details.error_message.is_some(),
            "error_message should be populated"
        );
        Ok(())
    }

    #[test]
    fn test_process_payment_webhook_raw_connector_response_present(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_SUCCESS", "SUCCESS");
        let details = connector.process_payment_webhook(make_request(&body), None, None, None)?;
        ensure!(
            details.raw_connector_response.is_some(),
            "raw_connector_response should always be populated"
        );
        ensure_eq!(details.status_code, 200, "status_code should be 200");
        Ok(())
    }

    // ── Webhook: process_payment_webhook — empty body / malformed ────────────

    #[test]
    fn test_process_payment_webhook_empty_body() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let result = connector.process_payment_webhook(make_request(b""), None, None, None);
        assert!(result.is_err(), "empty body should return an error");
    }

    #[test]
    fn test_process_payment_webhook_malformed_json() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let result =
            connector.process_payment_webhook(make_request(b"{invalid json}"), None, None, None);
        assert!(result.is_err(), "malformed JSON should return an error");
    }

    // ── Webhook: process_refund_webhook — edge cases ─────────────────────────

    #[test]
    fn test_process_refund_webhook_with_failure_details() -> Result<(), Box<dyn std::error::Error>>
    {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = r#"{
            "specversion": "1.0",
            "type": "PAYMENT_CHARGE_REFUND_FAILED",
            "source": "https://api.sandbox.eu.ppro.com",
            "id": "evt_test_refund_fail_detail",
            "time": "2024-01-01T00:00:00Z",
            "data": {
                "paymentChargeId": "pc_refund_fail_detail",
                "paymentChargeStatus": "FAILED",
                "refundId": "rf_refund_fail_detail",
                "failure": {
                    "failureType": "REFUND",
                    "failureCode": "REFUND_LIMIT_EXCEEDED",
                    "failureMessage": "Refund amount exceeds limit"
                }
            }
        }"#
        .as_bytes();
        let details = connector.process_refund_webhook(make_request(body), None, None)?;
        ensure_eq!(details.status, common_enums::RefundStatus::Failure);
        ensure_eq!(
            details.error_code.as_deref(),
            Some("REFUND_LIMIT_EXCEEDED"),
            "error_code should be populated from refund failure"
        );
        ensure!(
            details.error_message.is_some(),
            "error_message should be populated"
        );
        ensure!(
            details.connector_refund_id.is_some(),
            "connector_refund_id should be set even on failure"
        );
        Ok(())
    }

    #[test]
    fn test_process_refund_webhook_agreement_returns_error() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = agreement_webhook("PAYMENT_AGREEMENT_ACTIVE", "ACTIVE");
        let result = connector.process_refund_webhook(make_request(&body), None, None);
        assert!(
            result.is_err(),
            "Agreement webhook should return an error for process_refund_webhook"
        );
    }

    // ── Webhook: process_dispute_webhook — not implemented ───────────────────

    #[test]
    fn test_process_dispute_webhook_not_implemented() {
        let connector = connectors::ppro::Ppro::<DefaultPCIHolder>::new();
        let body = charge_webhook("PAYMENT_CHARGE_FAILED", "FAILED");
        let result = connector.process_dispute_webhook(make_request(&body), None, None);
        assert!(
            result.is_err(),
            "process_dispute_webhook should return NotImplemented error"
        );
    }
}

// ── Transformer unit tests ────────────────────────────────────────────────────
//
// These tests validate the serde round-trips and status mappings for each flow's
// request / response structs without requiring a full RouterDataV2 setup.
#[cfg(test)]
mod transformer_tests {
    use super::super::transformers::*;
    use common_utils::MinorUnit;
    use domain_types::router_response_types::RedirectForm;

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

    // ── Authorize / PSync response deserialization ────────────────────────────

    /// All PproPaymentStatus values round-trip through serde correctly.
    #[test]
    fn test_payment_status_deserialization() -> Result<(), Box<dyn std::error::Error>> {
        let cases = [
            (
                "\"AUTHORIZATION_PROCESSING\"",
                PproPaymentStatus::AuthorizationProcessing,
            ),
            (
                "\"CAPTURE_PROCESSING\"",
                PproPaymentStatus::CaptureProcessing,
            ),
            (
                "\"AUTHENTICATION_PENDING\"",
                PproPaymentStatus::AuthenticationPending,
            ),
            (
                "\"AUTHORIZATION_ASYNC\"",
                PproPaymentStatus::AuthorizationAsync,
            ),
            ("\"CAPTURE_PENDING\"", PproPaymentStatus::CapturePending),
            ("\"CAPTURED\"", PproPaymentStatus::Captured),
            ("\"FAILED\"", PproPaymentStatus::Failed),
            ("\"DISCARDED\"", PproPaymentStatus::Discarded),
            ("\"VOIDED\"", PproPaymentStatus::Voided),
            ("\"REFUND_SETTLED\"", PproPaymentStatus::RefundSettled),
            ("\"SUCCESS\"", PproPaymentStatus::Success),
            ("\"REFUNDED\"", PproPaymentStatus::Refunded),
            ("\"REJECTED\"", PproPaymentStatus::Rejected),
            ("\"DECLINED\"", PproPaymentStatus::Declined),
        ];
        for (json, expected) in cases {
            let parsed: PproPaymentStatus = serde_json::from_str(json)?;
            ensure_eq!(parsed, expected, "mismatch for {json}");
        }
        Ok(())
    }

    /// A minimal authorize response with `AUTHENTICATION_PENDING` and a redirect URL.
    #[test]
    fn test_authorize_response_with_redirect() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "id": "charge_abc123",
            "status": "AUTHENTICATION_PENDING",
            "authenticationMethods": [
                {
                    "type": "REDIRECT",
                    "details": {
                        "requestUrl": "https://redirect.ppro.com/auth",
                        "requestMethod": "GET"
                    }
                }
            ]
        }"#;
        let resp: PproPaymentsResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.id, "charge_abc123");
        ensure_eq!(resp.status, PproPaymentStatus::AuthenticationPending);
        let methods = resp
            .authentication_methods
            .ok_or("should have auth methods")?;
        ensure_eq!(methods.len(), 1);
        let method = methods.first().ok_or("methods should be non-empty")?;
        ensure_eq!(method.r#type, PproAuthenticationType::Redirect);
        let details = method.details.as_ref().ok_or("should have details")?;
        ensure_eq!(
            details.request_url.as_deref(),
            Some("https://redirect.ppro.com/auth")
        );
        ensure_eq!(details.request_method, Some(PproHttpMethod::Get));
        Ok(())
    }

    /// A captured charge response carries the instrument_id. It is deserialized but
    /// deliberately not surfaced as a mandate reference -- see PproPaymentsResponse.
    #[test]
    fn test_authorize_response_captured_with_instrument_id(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "id": "charge_xyz789",
            "status": "CAPTURED",
            "instrumentId": "instr_abc123"
        }"#;
        let resp: PproPaymentsResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.status, PproPaymentStatus::Captured);
        ensure_eq!(
            resp.instrument_id.as_deref(),
            Some("instr_abc123"),
            "instrumentId should be captured"
        );
        Ok(())
    }

    /// A failed response carries failure details.
    #[test]
    fn test_authorize_response_failed_with_failure() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "id": "charge_fail",
            "status": "FAILED",
            "failure": {
                "failureType": "AUTHORIZATION",
                "failureCode": "INSUFFICIENT_FUNDS",
                "failureMessage": "Insufficient funds"
            }
        }"#;
        let resp: PproPaymentsResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.status, PproPaymentStatus::Failed);
        let failure = resp.failure.ok_or("should have failure")?;
        ensure_eq!(failure.failure_type, "AUTHORIZATION");
        ensure_eq!(failure.failure_code.as_deref(), Some("INSUFFICIENT_FUNDS"));
        ensure_eq!(failure.failure_message, "Insufficient funds");
        Ok(())
    }

    // ── Capture request serialization ────────────────────────────────────────

    #[test]
    fn test_capture_request_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let req = PproCaptureRequest {
            amount: MinorUnit::new(2500),
        };
        let json: serde_json::Value = serde_json::to_value(&req)?;
        ensure_eq!(
            json.get("amount"),
            Some(&serde_json::json!(2500)),
            "amount should be serialized as integer"
        );
        Ok(())
    }

    // ── Void request serialization ────────────────────────────────────────────

    #[test]
    fn test_void_request_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let req = PproVoidRequest {
            amount: MinorUnit::new(1000),
        };
        let json: serde_json::Value = serde_json::to_value(&req)?;
        ensure_eq!(json.get("amount"), Some(&serde_json::json!(1000)));
        Ok(())
    }

    // ── Refund request serialization ─────────────────────────────────────────

    #[test]
    fn test_refund_request_serialization_with_reason() -> Result<(), Box<dyn std::error::Error>> {
        let req = PproRefundRequest {
            amount: MinorUnit::new(500),
            merchant_refund_reference: "ref_500".to_string(),
            refund_reason: Some(PproRefundReason::Fraud),
        };
        let json: serde_json::Value = serde_json::to_value(&req)?;
        ensure_eq!(json.get("amount"), Some(&serde_json::json!(500)));
        ensure!(
            json.get("refundReason").is_some_and(|v| !v.is_null()),
            "refundReason should be present"
        );
        Ok(())
    }

    #[test]
    fn test_refund_request_serialization_no_reason() -> Result<(), Box<dyn std::error::Error>> {
        let req = PproRefundRequest {
            amount: MinorUnit::new(300),
            merchant_refund_reference: "ref_300".to_string(),
            refund_reason: None,
        };
        let json: serde_json::Value = serde_json::to_value(&req)?;
        ensure_eq!(json.get("amount"), Some(&serde_json::json!(300)));
        ensure!(
            json.get("refundReason").is_none(),
            "refundReason should be omitted when None"
        );
        Ok(())
    }

    // ── RSync response (refund sync) ─────────────────────────────────────────

    /// REFUND_SETTLED and REFUNDED indicate a successful refund.
    #[test]
    fn test_rsync_response_refunded_statuses() -> Result<(), Box<dyn std::error::Error>> {
        for status in &["REFUND_SETTLED", "REFUNDED"] {
            let json = format!(r#"{{"id":"ref_001","status":"{status}"}}"#);
            let resp: PproPaymentsResponse = serde_json::from_str(&json)?;
            ensure!(
                matches!(
                    resp.status,
                    PproPaymentStatus::RefundSettled | PproPaymentStatus::Refunded
                ),
                "status {status} should deserialize to a refund-success variant"
            );
        }
        Ok(())
    }

    // ── SetupMandate (agreement) response deserialization ────────────────────

    #[test]
    fn test_agreement_response_authentication_pending() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "id": "agr_abc123",
            "status": "AUTHENTICATION_PENDING",
            "authenticationMethods": [
                {
                    "type": "REDIRECT",
                    "details": {
                        "requestUrl": "https://auth.ppro.com/agr",
                        "requestMethod": "GET"
                    }
                }
            ]
        }"#;
        let resp: PproAgreementResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.id, "agr_abc123");
        ensure_eq!(resp.status, PproAgreementStatus::AuthenticationPending);
        let methods = resp
            .authentication_methods
            .ok_or("should have auth methods")?;
        let method = methods.first().ok_or("methods should be non-empty")?;
        ensure_eq!(method.r#type, PproAuthenticationType::Redirect);
        ensure_eq!(
            method
                .details
                .as_ref()
                .and_then(|d| d.request_url.as_deref()),
            Some("https://auth.ppro.com/agr")
        );
        Ok(())
    }

    #[test]
    fn test_agreement_response_active_with_instrument_id() -> Result<(), Box<dyn std::error::Error>>
    {
        let json = r#"{
            "id": "agr_xyz456",
            "status": "ACTIVE",
            "instrumentId": "instr_mandate_001"
        }"#;
        let resp: PproAgreementResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.status, PproAgreementStatus::Active);
        ensure_eq!(
            resp.instrument_id.as_deref(),
            Some("instr_mandate_001"),
            "instrumentId should be deserialized"
        );
        ensure_eq!(
            resp.id,
            "agr_xyz456",
            "the agreement id -- not instrumentId -- is what becomes connector_mandate_id"
        );
        Ok(())
    }

    #[test]
    fn test_agreement_response_failed() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{
            "id": "agr_fail",
            "status": "FAILED",
            "failure": {
                "failureType": "AUTHENTICATION",
                "failureMessage": "Consumer rejected the mandate"
            }
        }"#;
        let resp: PproAgreementResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.status, PproAgreementStatus::Failed);
        let failure = resp.failure.ok_or("should have failure")?;
        ensure_eq!(failure.failure_type, "AUTHENTICATION");
        Ok(())
    }

    // ── Error response deserialization ───────────────────────────────────────

    #[test]
    fn test_error_response_deserialization() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"status": 422, "failureMessage": "Validation failed"}"#;
        let resp: PproErrorResponse = serde_json::from_str(json)?;
        ensure_eq!(resp.status, 422);
        ensure_eq!(resp.failure_message, "Validation failed");
        Ok(())
    }

    // ── build_auth_redirect: Satispay's 3-way auth flow resolution ───────────

    fn auth_response(
        r#type: PproAuthenticationType,
        details: Option<PproAuthDetailsResponse>,
    ) -> PproAuthenticationResponse {
        PproAuthenticationResponse { r#type, details }
    }

    fn empty_details() -> PproAuthDetailsResponse {
        PproAuthDetailsResponse {
            request_url: None,
            request_method: None,
            code_type: None,
            code_image: None,
            code_payload: None,
            code_document: None,
            scan_by: None,
            mobile_intent_uri: None,
        }
    }

    #[test]
    fn test_build_auth_redirect_satispay_intent_builds_uri(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let methods = vec![auth_response(
            PproAuthenticationType::AppIntent,
            Some(PproAuthDetailsResponse {
                mobile_intent_uri: Some("satispay://intent/xyz".to_string()),
                ..empty_details()
            }),
        )];
        let redirect = build_auth_redirect(
            &methods,
            Some(common_enums::PaymentMethodType::SatispayIntent),
        );
        match redirect {
            Some(RedirectForm::Uri { uri }) => {
                ensure_eq!(uri, "satispay://intent/xyz".to_string())
            }
            other => return Err(format!("expected RedirectForm::Uri, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_build_auth_redirect_satispay_qr_builds_qr_form(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let methods = vec![auth_response(
            PproAuthenticationType::ScanCode,
            Some(PproAuthDetailsResponse {
                code_payload: Some("qr_payload_abc".to_string()),
                code_image: Some("base64_image_data".to_string()),
                scan_by: Some("2026-07-23T12:00:00Z".to_string()),
                ..empty_details()
            }),
        )];
        let redirect =
            build_auth_redirect(&methods, Some(common_enums::PaymentMethodType::SatispayQr));
        match redirect {
            Some(RedirectForm::Qr {
                payload,
                image_base64,
                expires_at,
                ..
            }) => {
                ensure_eq!(payload, Some("qr_payload_abc".to_string()));
                ensure_eq!(image_base64, Some("base64_image_data".to_string()));
                ensure!(expires_at.is_some(), "expires_at should have parsed");
            }
            other => return Err(format!("expected RedirectForm::Qr, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_build_auth_redirect_satispay_plain_still_uses_uri_redirect(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Plain Satispay (PPRO's normal REDIRECT flow) is unaffected by the new variants.
        let methods = vec![auth_response(
            PproAuthenticationType::Redirect,
            Some(PproAuthDetailsResponse {
                request_url: Some("https://redirect.ppro.com/satispay".to_string()),
                ..empty_details()
            }),
        )];
        let redirect =
            build_auth_redirect(&methods, Some(common_enums::PaymentMethodType::Satispay));
        match redirect {
            Some(RedirectForm::Uri { uri }) => {
                ensure_eq!(uri, "https://redirect.ppro.com/satispay".to_string())
            }
            other => return Err(format!("expected RedirectForm::Uri, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_build_auth_redirect_satispay_intent_no_match_returns_none(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Only an unrelated auth type is present; SatispayIntent should not match it.
        let methods = vec![auth_response(PproAuthenticationType::ScanCode, None)];
        let redirect = build_auth_redirect(
            &methods,
            Some(common_enums::PaymentMethodType::SatispayIntent),
        );
        ensure!(redirect.is_none(), "expected no redirect to be built");
        Ok(())
    }

    /// Regression: UPI Intent's pre-existing `Uri`-based behavior must be unaffected by
    /// the addition of Satispay's `SatispayIntent`/`SatispayQr` variants.
    #[test]
    fn test_build_auth_redirect_upi_intent_still_builds_uri(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let methods = vec![auth_response(
            PproAuthenticationType::AppIntent,
            Some(PproAuthDetailsResponse {
                mobile_intent_uri: Some("upi://pay?xyz".to_string()),
                ..empty_details()
            }),
        )];
        let redirect =
            build_auth_redirect(&methods, Some(common_enums::PaymentMethodType::UpiIntent));
        match redirect {
            Some(RedirectForm::Uri { uri }) => ensure_eq!(uri, "upi://pay?xyz".to_string()),
            other => return Err(format!("expected RedirectForm::Uri, got {other:?}").into()),
        }
        Ok(())
    }

    /// UPI QR now shares Satispay QR's `Qr` form: `code_payload` lands in `payload`
    /// instead of the old `Uri`-from-`code_payload` shape.
    #[test]
    fn test_build_auth_redirect_upi_qr_builds_qr_form() -> Result<(), Box<dyn std::error::Error>> {
        let methods = vec![auth_response(
            PproAuthenticationType::ScanCode,
            Some(PproAuthDetailsResponse {
                code_payload: Some("upi_qr_payload".to_string()),
                ..empty_details()
            }),
        )];
        let redirect = build_auth_redirect(&methods, Some(common_enums::PaymentMethodType::UpiQr));
        match redirect {
            Some(RedirectForm::Qr {
                payload,
                image_base64,
                expires_at,
                ..
            }) => {
                ensure_eq!(payload, Some("upi_qr_payload".to_string()));
                ensure_eq!(image_base64, None);
                ensure_eq!(expires_at, None);
            }
            other => return Err(format!("expected RedirectForm::Qr, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_build_auth_redirect_upi_intent_unaffected_by_satispay_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // UPI Intent must still only match AppIntent, not ScanCode.
        let methods = vec![auth_response(
            PproAuthenticationType::ScanCode,
            Some(PproAuthDetailsResponse {
                code_payload: Some("should_not_match".to_string()),
                ..empty_details()
            }),
        )];
        let redirect =
            build_auth_redirect(&methods, Some(common_enums::PaymentMethodType::UpiIntent));
        ensure!(
            redirect.is_none(),
            "UPI Intent should not match a ScanCode entry"
        );
        Ok(())
    }
}

// ── Satispay authentication_settings selection (request builder) ─────────────
//
// These tests exercise `PproPaymentsRequest::try_from` directly, covering the
// 3-way flow-selection logic driven by `PaymentMethodType::{Satispay, SatispayIntent,
// SatispayQr}` -- mirroring the pre-existing `UpiIntent`/`UpiQr` precedent exactly.
#[cfg(test)]
mod satispay_request_tests {
    use std::marker::PhantomData;

    use common_utils::types::MinorUnit;
    use domain_types::{
        connector_flow::Authorize,
        connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
        payment_address::PaymentAddress,
        payment_method_data::{DefaultPCIHolder, PaymentMethodData, SatispayData, WalletData},
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        types::{ConnectorParams, Connectors},
    };
    use hyperswitch_masking::Secret;

    use super::super::transformers::{PproAuthenticationType, PproPaymentsRequest};
    use super::super::PproRouterData;
    use crate::connectors::Ppro;

    macro_rules! ensure_eq {
        ($left:expr, $right:expr $(,)?) => {{
            let left = &$left;
            let right = &$right;
            if left != right {
                return Err(format!("assertion failed: {left:?} != {right:?}").into());
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

    fn base_router_data(
        payment_method_type: common_enums::PaymentMethodType,
        router_return_url: Option<String>,
    ) -> RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: PhantomData::<Authorize>,
            resource_common_data: PaymentFlowData {
                raw_connector_status: None,
                vault_headers: None,
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_satispay_test".to_string(),
                attempt_id: "attempt_satispay_test".to_string(),
                status: common_enums::AttemptStatus::Pending,
                payment_method: common_enums::PaymentMethod::Wallet,
                description: None,
                return_url: router_return_url.clone(),
                order_details: None,
                address: PaymentAddress::new(None, None, None, None),
                auth_type: common_enums::AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "conn_ref_satispay_test".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                connectors: Connectors {
                    ppro: ConnectorParams {
                        base_url: "https://api.sandbox.eu.ppro.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                external_latency: None,
                connector_response_headers: None,
                raw_connector_response: None,
                raw_connector_request: None,
                minor_amount_capturable: None,
                amount: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                settlement_status: None,
            },
            connector_config: ConnectorSpecificConfig::Ppro {
                api_key: Secret::new("test_api_key".to_string()),
                merchant_id: Secret::new("test_merchant_id".to_string()),
                base_url: None,
            },
            request: PaymentsAuthorizeData {
                customer_document_details: None,
                authentication_data: None,
                connector_testing_data: None,
                access_token: None,
                payment_method_data: PaymentMethodData::Wallet(WalletData::Satispay(
                    SatispayData {},
                )),
                amount: MinorUnit::new(1000),
                order_tax_amount: None,
                surcharge_amount: None,
                email: None,
                customer_name: None,
                currency: common_enums::Currency::EUR,
                confirm: true,
                capture_method: None,
                integrity_object: None,
                router_return_url,
                webhook_url: Some("https://example.com/webhook".to_string()),
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage: None,
                off_session: None,
                browser_info: None,
                order_category: None,
                session_token: None,
                enrolled_for_3ds: Some(false),
                related_transaction_id: None,
                payment_experience: None,
                payment_method_type: Some(payment_method_type),
                customer_id: None,
                request_incremental_authorization: Some(false),
                metadata: None,
                minor_amount: MinorUnit::new(1000),
                merchant_order_id: None,
                shipping_cost: None,
                merchant_account_id: None,
                merchant_config_currency: None,
                all_keys_required: None,
                customer_acceptance: None,
                split_payments: None,
                request_extended_authorization: None,
                setup_mandate_details: None,
                enable_overcapture: None,
                connector_feature_data: None,
                billing_descriptor: None,
                enable_partial_authorization: None,
                locale: None,
                continue_redirection_url: None,
                redirect_response: None,
                threeds_method_comp_ind: None,
                tokenization: None,
                mit_category: None,
                payment_channel: None,
                domain_data: None,
                partner_merchant_identifier_details: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    fn build_request(
        payment_method_type: common_enums::PaymentMethodType,
        router_return_url: Option<String>,
    ) -> Result<PproPaymentsRequest, Box<dyn std::error::Error>> {
        let router_data = base_router_data(payment_method_type, router_return_url);
        let ppro_router_data = PproRouterData {
            connector: (*Ppro::<DefaultPCIHolder>::new()).clone(),
            router_data,
        };
        PproPaymentsRequest::try_from(ppro_router_data)
            .map_err(|err| format!("Satispay request should build successfully: {err:?}").into())
    }

    /// `PaymentMethodType::SatispayIntent` selects PPRO's `APP_INTENT` auth type, sourcing
    /// `mobileIntentUri` from the generic `router_return_url` -- exactly like `UpiIntent`.
    #[test]
    fn test_satispay_intent_selects_app_intent() -> Result<(), Box<dyn std::error::Error>> {
        let req = build_request(
            common_enums::PaymentMethodType::SatispayIntent,
            Some("https://example.com/return".to_string()),
        )?;
        let settings = req
            .authentication_settings
            .as_ref()
            .and_then(|v| v.first())
            .ok_or("expected authentication_settings to be present")?;
        ensure_eq!(settings.r#type, PproAuthenticationType::AppIntent);
        let details = settings
            .settings
            .as_ref()
            .ok_or("expected settings details")?;
        ensure_eq!(
            details.mobile_intent_uri,
            Some("https://example.com/return".to_string())
        );
        Ok(())
    }

    /// `PaymentMethodType::SatispayQr` selects PPRO's `SCAN_CODE` auth type with no extra
    /// settings payload -- exactly like `UpiQr`.
    #[test]
    fn test_satispay_qr_selects_scan_code() -> Result<(), Box<dyn std::error::Error>> {
        let req = build_request(common_enums::PaymentMethodType::SatispayQr, None)?;
        let settings = req
            .authentication_settings
            .as_ref()
            .and_then(|v| v.first())
            .ok_or("expected authentication_settings to be present")?;
        ensure_eq!(settings.r#type, PproAuthenticationType::ScanCode);
        ensure!(
            settings.settings.is_none(),
            "SatispayQr should not carry a settings payload, matching UpiQr"
        );
        Ok(())
    }

    /// Plain `PaymentMethodType::Satispay` is untouched: it still falls into the generic
    /// `_` catch-all and resolves to `REDIRECT` via `router_return_url`.
    #[test]
    fn test_satispay_plain_falls_back_to_redirect() -> Result<(), Box<dyn std::error::Error>> {
        let req = build_request(
            common_enums::PaymentMethodType::Satispay,
            Some("https://example.com/return".to_string()),
        )?;
        let settings = req
            .authentication_settings
            .as_ref()
            .and_then(|v| v.first())
            .ok_or("expected authentication_settings to be present")?;
        ensure_eq!(settings.r#type, PproAuthenticationType::Redirect);
        let details = settings
            .settings
            .as_ref()
            .ok_or("expected settings details")?;
        ensure_eq!(
            details.return_url,
            Some("https://example.com/return".to_string())
        );
        Ok(())
    }

    /// Regression: `UpiIntent`'s existing `authentication_settings` behavior is unaffected.
    #[test]
    fn test_upi_intent_still_selects_app_intent() -> Result<(), Box<dyn std::error::Error>> {
        let req = build_request(
            common_enums::PaymentMethodType::UpiIntent,
            Some("https://example.com/upi-return".to_string()),
        )?;
        let settings = req
            .authentication_settings
            .as_ref()
            .and_then(|v| v.first())
            .ok_or("expected authentication_settings to be present")?;
        ensure_eq!(settings.r#type, PproAuthenticationType::AppIntent);
        Ok(())
    }

    /// Regression: `UpiQr`'s existing `authentication_settings` behavior is unaffected.
    #[test]
    fn test_upi_qr_still_selects_scan_code() -> Result<(), Box<dyn std::error::Error>> {
        let req = build_request(common_enums::PaymentMethodType::UpiQr, None)?;
        let settings = req
            .authentication_settings
            .as_ref()
            .and_then(|v| v.first())
            .ok_or("expected authentication_settings to be present")?;
        ensure_eq!(settings.r#type, PproAuthenticationType::ScanCode);
        Ok(())
    }
}
