use domain_types::payment_method_data::DefaultPCIHolder;
use interfaces::webhooks::IncomingWebhook;

use super::*;

fn callback(status: &str) -> RequestDetails {
    RequestDetails {
        method: HttpMethod::Post,
        uri: None,
        headers: Default::default(),
        query_params: None,
        body: serde_json::to_vec(&serde_json::json!({
            "uuid": "forged-payment-uuid", "referenceId": "merchant-reference",
            "status": status, "amount": 12.34, "currency": "MYR",
        }))
        .expect("webhook fixture"),
    }
}

#[test]
fn unsigned_callbacks_cannot_produce_payment_outcomes() {
    let connector = Boost::<DefaultPCIHolder>::new();
    for status in ["succeeded", "failed", "canceled", "processing"] {
        let request = callback(status);
        assert!(!connector
            .verify_webhook_source(request.clone(), None, None)
            .expect("verification"));
        let error = connector
            .process_payment_webhook(request, None, None, None)
            .expect_err("unsigned callbacks must not return payment outcomes");
        assert!(matches!(
            error.current_context(),
            errors::WebhookError::WebhookSourceVerificationFailed
        ));
    }
}

#[test]
fn unsigned_callback_references_remain_available_for_psync() {
    let connector = Boost::<DefaultPCIHolder>::new();
    let reference = connector
        .get_webhook_event_reference(callback("succeeded"))
        .expect("parse reference")
        .expect("payment reference");
    match reference {
        WebhookResourceReference::Payment(reference) => {
            assert_eq!(
                reference.connector_transaction_id.as_deref(),
                Some("forged-payment-uuid")
            );
            assert_eq!(
                reference.merchant_transaction_id.as_deref(),
                Some("merchant-reference")
            );
        }
        _ => panic!("expected payment reference"),
    }
}

#[test]
fn shared_event_processing_rejects_unverified_success_callbacks() {
    let connector = crate::types::ConnectorData::<DefaultPCIHolder>::get_connector_by_name(
        &ConnectorEnum::Boost,
    );
    let error = crate::webhook_utils::process_webhook_event(
        connector,
        callback("succeeded"),
        None,
        None,
        false,
        None,
        None,
    )
    .expect_err("the gRPC/FFI processing path must reject forged success callbacks");
    assert!(matches!(
        error.current_context(),
        errors::WebhookError::WebhookSourceVerificationFailed
    ));
}
