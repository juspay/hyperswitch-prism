#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use base64::Engine;

    use crate::connectors::trustly::transformers::{
        generate_trustly_signature, verify_webhook_signature, TrustlyWebhookBody,
    };

    #[test]
    fn signature_covers_data_fields_the_struct_does_not_model() {
        let rsa = openssl::rsa::Rsa::generate(2048).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD;
        let private_key = b64.encode(rsa.private_key_to_pem().unwrap());
        let public_key = b64.encode(rsa.public_key_to_pem().unwrap());

        // `attributes.unmodelled` is not a TrustlyWebhookData field.
        let data = serde_json::json!({
            "messageid": "msg-1",
            "orderid": "order-1",
            "notificationid": "notif-1",
            "amount": "10.00",
            "currency": "EUR",
            "attributes": { "unmodelled": "value" }
        });
        let signature =
            generate_trustly_signature("credit", "uuid-1", &data, &private_key).unwrap();
        let raw_body = serde_json::json!({
            "method": "credit",
            "params": { "signature": signature, "uuid": "uuid-1", "data": data },
            "version": "1.1"
        })
        .to_string()
        .into_bytes();
        let webhook_body: TrustlyWebhookBody = serde_json::from_slice(&raw_body).unwrap();

        assert!(
            verify_webhook_signature(webhook_body, &raw_body, public_key.into_bytes()).unwrap()
        );
    }
}
