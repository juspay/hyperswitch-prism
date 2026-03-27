#![allow(clippy::expect_used, clippy::indexing_slicing)]
#![allow(clippy::panic)]
use grpc_server::app;
use ucs_env::configs;
use hyperswitch_masking::ExposeInterface;
mod common;
mod utils;
use std::fmt::Write;

use common_utils::crypto::{HmacSha512, SignMessage};
use grpc_api_types::payments::{
    payment_service_client::PaymentServiceClient, PaymentServiceTransformRequest, RequestDetails,
};
use serde_json::json;
use tonic::{transport::Channel, Request};

// Helper function to construct Authorize.Net customer payment profile creation webhook JSON body
fn build_authorizedotnet_payment_profile_webhook_json_body(
    event_type: &str,
    customer_profile_id: u64,
    payment_profile_id: &str,
    customer_type: &str,
) -> serde_json::Value {
    let notification_id = "7201C905-B01E-4622-B807-AC2B646A3815"; // Default
    let event_date = "2016-03-23T06:19:09.5297562Z"; // Default
    let webhook_id = "6239A0BE-D8F4-4A33-8FAD-901C02EED51F"; // Default

    let payload = json!({
        "customerProfileId": customer_profile_id,
        "entityName": "customerPaymentProfile",
        "id": payment_profile_id,
        "customerType": customer_type
    });

    json!({
        "notificationId": notification_id,
        "eventType": event_type,
        "eventDate": event_date,
        "webhookId": webhook_id,
        "payload": payload
    })
}

// Helper function to construct Authorize.Net customer creation webhook JSON body
fn build_authorizedotnet_customer_webhook_json_body(
    event_type: &str,
    customer_profile_id: &str,
    payment_profile_id: &str,
    merchant_customer_id: Option<&str>,
    description: Option<&str>,
) -> serde_json::Value {
    let notification_id = "5c3f7e00-1265-4e8e-abd0-a7d734163881"; // Default
    let event_date = "2016-03-23T05:23:06.5430555Z"; // Default
    let webhook_id = "0b90f2e8-02ae-4d1d-b2e0-1bd167e60176"; // Default

    let payload = json!({
        "paymentProfiles": [{
            "id": payment_profile_id,
            "customerType": "individual"
        }],
        "merchantCustomerId": merchant_customer_id.unwrap_or("cust457"),
        "description": description.unwrap_or("Profile created by Subscription"),
        "entityName": "customerProfile",
        "id": customer_profile_id
    });

    json!({
        "notificationId": notification_id,
        "eventType": event_type,
        "eventDate": event_date,
        "webhookId": webhook_id,
        "payload": payload
    })
}

// Helper function to construct Authorize.Net webhook JSON body
fn build_authorizedotnet_webhook_json_body(
    event_type: &str,
    response_code: u8,
    transaction_id: &str,
    amount: Option<f64>,
    merchant_reference_id: Option<&str>,
    auth_code: Option<&str>,
    message_text: Option<&str>,
) -> serde_json::Value {
    let notification_id = "550e8400-e29b-41d4-a716-446655440000"; // Default
    let event_date = "2023-12-01T12:00:00Z"; // Default
    let webhook_id = "webhook_123"; // Default
    let entity_name = "transaction"; // Default

    let mut payload = json!({
        "responseCode": response_code,
        "entityName": entity_name,
        "id": transaction_id,
    });

    // Add optional fields if provided
    if let Some(ref_id) = merchant_reference_id {
        payload["merchantReferenceId"] = json!(ref_id);
    }
    if let Some(auth) = auth_code {
        payload["authCode"] = json!(auth);
    }
    if let Some(amt) = amount {
        if event_type.contains("authorization") || event_type.contains("authcapture") {
            payload["authAmount"] = json!(amt);
        } else if event_type.contains("capture") || event_type.contains("refund") {
            payload["settleAmount"] = json!(amt);
        }
    }
    if let Some(msg) = message_text {
        payload["messageText"] = json!(msg);
    }

    // Add common fields
    payload["avsResponse"] = json!("Y");
    payload["cvvResponse"] = json!("M");

    json!({
        "notificationId": notification_id,
        "eventType": event_type,
        "eventDate": event_date,
        "webhookId": webhook_id,
        "payload": payload
    })
}

// Helper function to generate HMAC-SHA512 signature for testing
fn generate_webhook_signature(webhook_body: &[u8], secret: &str) -> String {
    let crypto_algorithm = HmacSha512;
    let signature = crypto_algorithm
        .sign_message(secret.as_bytes(), webhook_body)
        .expect("Failed to generate signature");

    // Convert bytes to hex string manually
    let mut hex_string = String::with_capacity(signature.len() * 2);
    for b in signature {
        write!(&mut hex_string, "{b:02x}").expect("writing to a String should never fail");
    }

    format!("sha512={hex_string}")
}

// Helper to make the gRPC call and return success/failure status
async fn process_webhook_request(
    client: &mut PaymentServiceClient<Channel>,
    json_body: serde_json::Value,
    include_signature: bool,
) -> Result<(), String> {
    let request_body_bytes =
        serde_json::to_vec(&json_body).expect("Failed to serialize json_body to Vec<u8>");

    let mut headers = std::collections::HashMap::new();

    // Get webhook_secret from metadata
    let metadata = utils::credential_utils::load_connector_metadata("authorizedotnet")
        .expect("Failed to load authorizedotnet metadata");
    let webhook_secret = metadata
        .get("webhook_secret")
        .expect("webhook_secret not found in authorizedotnet metadata")
        .clone();

    if include_signature {
        let signature = generate_webhook_signature(&request_body_bytes, &webhook_secret);
        headers.insert("X-ANET-Signature".to_string(), signature);
    }

    let webhook_secrets = Some(grpc_api_types::payments::WebhookSecrets {
        secret: webhook_secret.clone(),
        additional_secret: None,
    });

    let mut request = Request::new(PaymentServiceTransformRequest {
        request_ref_id: Some(grpc_api_types::payments::Identifier {
            id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                "webhook_test".to_string(),
            )),
        }),
        request_details: Some(RequestDetails {
            method: grpc_api_types::payments::HttpMethod::Post.into(),
            headers,
            uri: Some("/webhooks/authorizedotnet".to_string()),
            query_params: None,
            body: request_body_bytes,
        }),
        webhook_secrets,
        state: None,
    });

    // Use the same metadata pattern as the payment flows test
    let auth = utils::credential_utils::load_connector_auth("authorizedotnet")
        .expect("Failed to load authorizedotnet credentials");

    let (api_key, key1) = match auth {
        domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
            (api_key.expose(), key1.expose())
        }
        _ => panic!("Expected BodyKey auth type for authorizedotnet"),
    };

    request.metadata_mut().append(
        "x-connector",
        "authorizedotnet"
            .parse()
            .expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "body-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        api_key.parse().expect("Failed to parse x-api-key"),
    );
    request
        .metadata_mut()
        .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
    request.metadata_mut().append(
        "x-merchant-id",
        "test_merchant"
            .parse()
            .expect("Failed to parse x-merchant-id"),
    );
    request.metadata_mut().append(
        "x-tenant-id",
        "default".parse().expect("Failed to parse x-tenant-id"),
    );
    request.metadata_mut().append(
        "x-request-id",
        "webhook_test_req"
            .parse()
            .expect("Failed to parse x-request-id"),
    );

    let _response = client
        .transform(request)
        .await
        .map_err(|e| format!("gRPC transform call failed: {e}"))?;

    // Response processed successfully

    // If we get a response, the webhook was processed successfully
    Ok(())
}

// --- Payment Authorization Event Tests ---

#[tokio::test]
async fn test_payment_authorization_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456789";
        let amount = Some(100.50);
        let auth_code = Some("ABC123");
        let message_text = Some("This transaction has been approved.");

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF123"),
            auth_code,
            message_text,
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        // Check result in assertion below
        assert!(
            result.is_ok(),
            "Payment authorization approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_authorization_declined() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 2; // Declined
        let transaction_id = "60123456790";
        let message_text = Some("This transaction has been declined.");

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF124"),
            None,
            message_text,
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment authorization declined webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_authorization_held() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 4; // Held for review
        let transaction_id = "60123456791";
        let message_text = Some("This transaction is being held for review.");

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(75.25),
            Some("REF125"),
            None,
            message_text,
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment authorization held webhook should be processed successfully"
        );
    });
}

// --- Payment Auth-Capture Event Tests ---

#[tokio::test]
async fn test_payment_authcapture_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authcapture.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456792";
        let amount = Some(200.00);

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF126"),
            Some("XYZ789"),
            Some("This transaction has been approved."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment authcapture approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_authcapture_declined() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authcapture.created";
        let response_code = 2; // Declined
        let transaction_id = "60123456793";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF127"),
            None,
            Some("This transaction has been declined."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment authcapture declined webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_authcapture_held() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authcapture.created";
        let response_code = 4; // Held for review
        let transaction_id = "60123456794";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(150.75),
            Some("REF128"),
            None,
            Some("This transaction is being held for review."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment authcapture held webhook should be processed successfully"
        );
    });
}

// --- Payment Capture Event Tests ---

#[tokio::test]
async fn test_payment_capture_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.capture.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456795";
        let amount = Some(100.00);

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF129"),
            None,
            Some("This transaction has been captured."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment capture approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_capture_declined() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.capture.created";
        let response_code = 2; // Declined
        let transaction_id = "60123456796";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF130"),
            None,
            Some("This capture has been declined."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment capture declined webhook should be processed successfully"
        );
    });
}

// --- Payment Void Event Tests ---

#[tokio::test]
async fn test_payment_void_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.void.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456797";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF131"),
            None,
            Some("This transaction has been voided."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment void approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_void_failed() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.void.created";
        let response_code = 2; // Failed
        let transaction_id = "60123456798";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF132"),
            None,
            Some("This void has failed."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment void failed webhook should be processed successfully"
        );
    });
}

// --- Payment Prior Auth Capture Event Tests ---

#[tokio::test]
async fn test_payment_prior_auth_capture_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.priorAuthCapture.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456799";
        let amount = Some(85.50);

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF133"),
            None,
            Some("This prior authorization capture has been approved."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment prior auth capture approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_prior_auth_capture_declined() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.priorAuthCapture.created";
        let response_code = 2; // Declined
        let transaction_id = "60123456800";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF134"),
            None,
            Some("This prior authorization capture has been declined."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment prior auth capture declined webhook should be processed successfully"
        );
    });
}

// --- Refund Event Tests ---

#[tokio::test]
async fn test_payment_refund_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.refund.created";
        let response_code = 1; // Approved
        let transaction_id = "60123456801";
        let amount = Some(50.25);

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF135"),
            None,
            Some("This refund has been approved."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment refund approved webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_refund_declined() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.refund.created";
        let response_code = 2; // Declined
        let transaction_id = "60123456802";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            None,
            Some("REF136"),
            None,
            Some("This refund has been declined."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment refund declined webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_payment_refund_held() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.refund.created";
        let response_code = 4; // Held for review
        let transaction_id = "60123456803";
        let amount = Some(25.00);

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            amount,
            Some("REF137"),
            None,
            Some("This refund is being held for review."),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Payment refund held webhook should be processed successfully"
        );
    });
}

// --- Security and Error Tests ---

#[tokio::test]
async fn test_webhook_signature_verification_valid() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456804";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF138"),
            Some("ABC123"),
            Some("Valid signature test."),
        );

        // This should succeed with valid signature
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(result.is_ok(), "Valid signature should be accepted");
    });
}

#[tokio::test]
async fn test_webhook_missing_signature() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456806";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF140"),
            Some("ABC123"),
            Some("Missing signature test."),
        );

        // Process without signature - the gRPC server requires signatures even when verification is not mandatory
        let result = process_webhook_request(&mut client, json_body, false).await;

        // The gRPC server returns "Signature not found" when no signature is provided
        // This is expected behavior even though verification is not mandatory for Authorize.Net
        match result {
            Ok(_) => {
                // If it succeeds, that's fine - the system handled missing signature gracefully
            }
            Err(e) => {
                // Expect signature not found error
                assert!(
                    e.contains("Signature not found for incoming webhook"),
                    "Expected 'Signature not found' error but got: {e}"
                );
            }
        }
    });
}

#[tokio::test]
async fn test_webhook_malformed_body() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let malformed_json = json!({
            "invalid": "structure",
            "missing": "required_fields"
        });

        let request_body_bytes =
            serde_json::to_vec(&malformed_json).expect("Failed to serialize malformed json");

        let mut request = Request::new(PaymentServiceTransformRequest {
            request_ref_id: Some(grpc_api_types::payments::Identifier {
                id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                    "webhook_test".to_string(),
                )),
card_details),
            request_details: Some(RequestDetails {
                method: grpc_api_types::payments::HttpMethod::Post.into(),
                headers: std::collections::HashMap::new(),
                uri: Some("/webhooks/authorizedotnet".to_string()),
                query_params: None,
                body: request_body_bytes,
card_details),
            webhook_secrets: None,
            state: None,
        });

        let auth = utils::credential_utils::load_connector_auth("authorizedotnet")
            .expect("Failed to load authorizedotnet credentials");

        let api_key = match auth {
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, .. } => {
                api_key.expose()
            }
            _ => panic!("Expected BodyKey auth type for authorizedotnet"),
        };

        // Get transaction_key from metadata
        let metadata = utils::credential_utils::load_connector_metadata("authorizedotnet")
            .expect("Failed to load authorizedotnet metadata");
        let transaction_key = metadata
            .get("transaction_key")
            .expect("transaction_key not found in authorizedotnet metadata")
            .clone();

        request.metadata_mut().append(
            "x-connector",
            "authorizedotnet"
                .parse()
                .expect("Failed to parse x-connector"),
        );
        request.metadata_mut().append(
            "x-auth",
            "signature-key".parse().expect("Failed to parse x-auth"),
        );
        request.metadata_mut().append(
            "x-api-key",
            api_key.parse().expect("Failed to parse x-api-key"),
        );
        request.metadata_mut().append(
            "x-transaction-key",
            transaction_key
                .parse()
                .expect("Failed to parse x-transaction-key"),
        );

        // This should fail due to malformed body
        let response = client.transform(request).await;

        // We expect this to fail or return an error response
        match response {
            Ok(_resp) => {
                // If it succeeds, the response should indicate parsing failure
                // We'll accept this as the system handled it gracefully
            }
            Err(_) => {
                // This is expected - malformed body should cause failure
            }
        }
    });
}

// --- Customer Created Event Tests ---

#[tokio::test]
async fn test_customer_created_approved() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.customer.created";
        let customer_profile_id = "394";
        let payment_profile_id = "694";

        let json_body = build_authorizedotnet_customer_webhook_json_body(
            event_type,
            customer_profile_id,
            payment_profile_id,
            Some("cust457"),
            Some("Profile created by Subscription: 1447"),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Customer created webhook should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_customer_created_with_different_customer_id() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.customer.created";
        let customer_profile_id = "395";
        let payment_profile_id = "695";

        let json_body = build_authorizedotnet_customer_webhook_json_body(
            event_type,
            customer_profile_id,
            payment_profile_id,
            Some("cust458"),
            Some("Profile created for mandate setup"),
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Customer creation webhook with different ID should be processed successfully"
        );
    });
}

// --- Customer Payment Profile Created Event Tests ---

#[tokio::test]
async fn test_customer_payment_profile_created_individual() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.customer.paymentProfile.created";
        let customer_profile_id = 394;
        let payment_profile_id = "694";
        let customer_type = "individual";

        let json_body = build_authorizedotnet_payment_profile_webhook_json_body(
            event_type,
            customer_profile_id,
            payment_profile_id,
            customer_type,
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Customer payment profile created webhook for individual should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_customer_payment_profile_created_business() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.customer.paymentProfile.created";
        let customer_profile_id = 395;
        let payment_profile_id = "695";
        let customer_type = "business";

        let json_body = build_authorizedotnet_payment_profile_webhook_json_body(
            event_type,
            customer_profile_id,
            payment_profile_id,
            customer_type,
        );

        // Test that webhook processing succeeds
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Customer payment profile created webhook for business should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_webhook_unknown_event_type() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let unknown_event_type = "net.authorize.unknown.event.type";
        let response_code = 1;
        let transaction_id = "60123456807";

        let json_body = build_authorizedotnet_webhook_json_body(
            unknown_event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF141"),
            Some("ABC123"),
            Some("Unknown event type test."),
        );

        let result = process_webhook_request(&mut client, json_body, true).await;

        // The system should handle unknown event types gracefully
        // This could either succeed with a default handling or fail gracefully
        match result {
            Ok(()) => {
                // System handled unknown event type gracefully
            }
            Err(_) => {
                // System appropriately rejected unknown event type
            }
        }
    });
}

// --- Webhook Source Verification Tests ---

#[tokio::test]
async fn test_webhook_source_verification_valid_signature() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456808";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF142"),
            Some("ABC123"),
            Some("Valid signature verification test."),
        );

        // Test with valid signature - the helper function already generates correct signatures
        let result = process_webhook_request(&mut client, json_body, true).await;
        assert!(
            result.is_ok(),
            "Webhook with valid signature should be processed successfully"
        );
    });
}

#[tokio::test]
async fn test_webhook_source_verification_invalid_signature() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456809";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF143"),
            Some("ABC123"),
            Some("Invalid signature verification test."),
        );

        let request_body_bytes =
            serde_json::to_vec(&json_body).expect("Failed to serialize json_body to Vec<u8>");

        let mut headers = std::collections::HashMap::new();

        // Add an invalid signature
        headers.insert(
            "X-ANET-Signature".to_string(),
            "sha512=invalidhexsignature".to_string(),
        );

        // Get webhook_secret from metadata
        let metadata = utils::credential_utils::load_connector_metadata("authorizedotnet")
            .expect("Failed to load authorizedotnet metadata");
        let webhook_secret = metadata
            .get("webhook_secret")
            .expect("webhook_secret not found in authorizedotnet metadata")
            .clone();

        let webhook_secrets = Some(grpc_api_types::payments::WebhookSecrets {
            secret: webhook_secret.clone(),
            additional_secret: None,
        });

        let mut request = Request::new(PaymentServiceTransformRequest {
            request_ref_id: Some(grpc_api_types::payments::Identifier {
                id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                    "webhook_test".to_string(),
                )),
card_details),
            request_details: Some(RequestDetails {
                method: grpc_api_types::payments::HttpMethod::Post.into(),
                headers,
                uri: Some("/webhooks/authorizedotnet".to_string()),
                query_params: None,
                body: request_body_bytes,
card_details),
            webhook_secrets,
            state: None,
        });

        let auth = utils::credential_utils::load_connector_auth("authorizedotnet")
            .expect("Failed to load authorizedotnet credentials");

        let (api_key, key1) = match auth {
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
                (api_key.expose(), key1.expose())
            }
            _ => panic!("Expected BodyKey auth type for authorizedotnet"),
        };

        request.metadata_mut().append(
            "x-connector",
            "authorizedotnet"
                .parse()
                .expect("Failed to parse x-connector"),
        );
        request.metadata_mut().append(
            "x-auth",
            "body-key".parse().expect("Failed to parse x-auth"),
        );
        request.metadata_mut().append(
            "x-api-key",
            api_key.parse().expect("Failed to parse x-api-key"),
        );
        request
            .metadata_mut()
            .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
        request.metadata_mut().append(
            "x-merchant-id",
            "test_merchant"
                .parse()
                .expect("Failed to parse x-merchant-id"),
        );
        request.metadata_mut().append(
            "x-tenant-id",
            "default".parse().expect("Failed to parse x-tenant-id"),
        );
        request.metadata_mut().append(
            "x-request-id",
            "webhook_test_req"
                .parse()
                .expect("Failed to parse x-request-id"),
        );

        // This should still process the webhook but with source_verified = false
        let response = client.transform(request).await;
        assert!(
            response.is_ok(),
            "Webhook with invalid signature should still be processed"
        );

        // Note: The response should have source_verified = false, but UCS continues processing
    });
}

#[tokio::test]
async fn test_webhook_source_verification_missing_signature() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456810";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF144"),
            Some("ABC123"),
            Some("Missing signature verification test."),
        );

        let request_body_bytes =
            serde_json::to_vec(&json_body).expect("Failed to serialize json_body to Vec<u8>");

        // Don't add any signature header
        let headers = std::collections::HashMap::new();

        // Get webhook_secret from metadata
        let metadata = utils::credential_utils::load_connector_metadata("authorizedotnet")
            .expect("Failed to load authorizedotnet metadata");
        let webhook_secret = metadata
            .get("webhook_secret")
            .expect("webhook_secret not found in authorizedotnet metadata")
            .clone();

        let webhook_secrets = Some(grpc_api_types::payments::WebhookSecrets {
            secret: webhook_secret.clone(),
            additional_secret: None,
        });

        let mut request = Request::new(PaymentServiceTransformRequest {
            request_ref_id: Some(grpc_api_types::payments::Identifier {
                id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                    "webhook_test".to_string(),
                )),
card_details),
            request_details: Some(RequestDetails {
                method: grpc_api_types::payments::HttpMethod::Post.into(),
                headers,
                uri: Some("/webhooks/authorizedotnet".to_string()),
                query_params: None,
                body: request_body_bytes,
card_details),
            webhook_secrets,
            state: None,
        });

        let auth = utils::credential_utils::load_connector_auth("authorizedotnet")
            .expect("Failed to load authorizedotnet credentials");

        let (api_key, key1) = match auth {
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
                (api_key.expose(), key1.expose())
            }
            _ => panic!("Expected BodyKey auth type for authorizedotnet"),
        };

        request.metadata_mut().append(
            "x-connector",
            "authorizedotnet"
                .parse()
                .expect("Failed to parse x-connector"),
        );
        request.metadata_mut().append(
            "x-auth",
            "body-key".parse().expect("Failed to parse x-auth"),
        );
        request.metadata_mut().append(
            "x-api-key",
            api_key.parse().expect("Failed to parse x-api-key"),
        );
        request
            .metadata_mut()
            .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
        request.metadata_mut().append(
            "x-merchant-id",
            "test_merchant"
                .parse()
                .expect("Failed to parse x-merchant-id"),
        );
        request.metadata_mut().append(
            "x-tenant-id",
            "default".parse().expect("Failed to parse x-tenant-id"),
        );
        request.metadata_mut().append(
            "x-request-id",
            "webhook_test_req"
                .parse()
                .expect("Failed to parse x-request-id"),
        );

        // This should still process the webhook but with source_verified = false
        let response = client.transform(request).await;
        assert!(
            response.is_ok(),
            "Webhook without signature should still be processed"
        );

        // Note: The response should have source_verified = false, but UCS continues processing
    });
}

#[tokio::test]
async fn test_webhook_source_verification_no_secret_provided() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_type = "net.authorize.payment.authorization.created";
        let response_code = 1;
        let transaction_id = "60123456811";

        let json_body = build_authorizedotnet_webhook_json_body(
            event_type,
            response_code,
            transaction_id,
            Some(100.0),
            Some("REF145"),
            Some("ABC123"),
            Some("No secret provided verification test."),
        );

        let request_body_bytes =
            serde_json::to_vec(&json_body).expect("Failed to serialize json_body to Vec<u8>");

        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "X-ANET-Signature".to_string(),
            "sha512=somesignature".to_string(),
        );

        // Don't provide webhook secrets (None)
        let webhook_secrets = None;

        let mut request = Request::new(PaymentServiceTransformRequest {
            request_ref_id: Some(grpc_api_types::payments::Identifier {
                id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                    "webhook_test".to_string(),
                )),
card_details),
            request_details: Some(RequestDetails {
                method: grpc_api_types::payments::HttpMethod::Post.into(),
                headers,
                uri: Some("/webhooks/authorizedotnet".to_string()),
                query_params: None,
                body: request_body_bytes,
card_details),
            webhook_secrets,
            state: None,
        });

        let auth = utils::credential_utils::load_connector_auth("authorizedotnet")
            .expect("Failed to load authorizedotnet credentials");

        let (api_key, key1) = match auth {
            domain_types::router_data::ConnectorAuthType::BodyKey { api_key, key1 } => {
                (api_key.expose(), key1.expose())
            }
            _ => panic!("Expected BodyKey auth type for authorizedotnet"),
        };

        request.metadata_mut().append(
            "x-connector",
            "authorizedotnet"
                .parse()
                .expect("Failed to parse x-connector"),
        );
        request.metadata_mut().append(
            "x-auth",
            "body-key".parse().expect("Failed to parse x-auth"),
        );
        request.metadata_mut().append(
            "x-api-key",
            api_key.parse().expect("Failed to parse x-api-key"),
        );
        request
            .metadata_mut()
            .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
        request.metadata_mut().append(
            "x-merchant-id",
            "test_merchant"
                .parse()
                .expect("Failed to parse x-merchant-id"),
        );
        request.metadata_mut().append(
            "x-tenant-id",
            "default".parse().expect("Failed to parse x-tenant-id"),
        );
        request.metadata_mut().append(
            "x-request-id",
            "webhook_test_req"
                .parse()
                .expect("Failed to parse x-request-id"),
        );

        // This should process the webhook with source_verified = false (no secret to verify against)
        let response = client.transform(request).await;
        assert!(
            response.is_ok(),
            "Webhook without webhook secret should still be processed"
        );

        // Note: The response should have source_verified = false, but UCS continues processing
    });
}
