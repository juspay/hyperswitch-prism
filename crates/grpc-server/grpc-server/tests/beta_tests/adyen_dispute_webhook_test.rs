#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
use grpc_server::app;
use ucs_env::configs;
use hyperswitch_masking::ExposeInterface;
mod common;
mod utils;
use grpc_api_types::payments::{
    payment_service_client::PaymentServiceClient,
    webhook_response_content::Content as GrpcWebhookContent, DisputeStage as GrpcDisputeStage,
    DisputeStatus as GrpcDisputeStatus, DisputesSyncResponse, PaymentServiceTransformRequest,
    RequestDetails,
};
use serde_json::json;
use tonic::{transport::Channel, Request};

// Helper function to construct Adyen webhook JSON body
fn build_adyen_webhook_json_body(
    event_code: &str,
    reason: &str,
    adyen_dispute_status: Option<&str>,
) -> serde_json::Value {
    let mut additional_data = serde_json::Map::new();
    if let Some(status) = adyen_dispute_status {
        additional_data.insert("disputeStatus".to_string(), json!(status));
    }
    let psp_reference = "9915555555555555"; // Default
    let original_reference = "9913333333333333"; // Default
    let merchant_account_code = "YOUR_MERCHANT_ACCOUNT"; // Default
    let merchant_reference = "YOUR_REFERENCE"; // Default
    let payment_method = "mc"; // Default
    let amount_currency = "EUR"; // Default
    let amount_value = 1000; // Default
    let event_date = "2023-12-01T12:00:00Z"; // Default
    let success_status = "true";

    json!({
        "live": "false",
        "notificationItems": [
            {
                "NotificationRequestItem": {
                    "eventCode": event_code,
                    "success": success_status,
                    "pspReference": psp_reference,
                    "originalReference": original_reference,
                    "merchantAccountCode": merchant_account_code,
                    "merchantReference": merchant_reference,
                    "paymentMethod": payment_method,
                    "eventDate": event_date,
                    "additionalData": additional_data,
                    "reason": reason,
                    "amount": {
                        "value": amount_value,
                        "currency": amount_currency
                    }
                }
            }
        ]
    })
}

// Helper to make the gRPC call and return the DisputesSyncResponse
async fn process_webhook_and_get_response(
    client: &mut PaymentServiceClient<Channel>,
    json_body: serde_json::Value,
) -> DisputesSyncResponse {
    let request_body_bytes =
        serde_json::to_vec(&json_body).expect("Failed to serialize json_body to Vec<u8>");

    let mut request = Request::new(PaymentServiceTransformRequest {
        request_ref_id: Some(grpc_api_types::payments::Identifier {
            id_type: Some(grpc_api_types::payments::identifier::IdType::Id(
                "webhook_test".to_string(),
            )),
        }),
        request_details: Some(RequestDetails {
            method: grpc_api_types::payments::HttpMethod::Post.into(),
            headers: std::collections::HashMap::new(),
            uri: Some("/webhooks/adyen".to_string()),
            query_params: None,
            body: request_body_bytes,
        }),
        webhook_secrets: None,
        state: None,
    });

    let auth = utils::credential_utils::load_connector_auth("adyen")
        .expect("Failed to load adyen credentials");

    let (api_key, key1, api_secret) = match auth {
        domain_types::router_data::ConnectorAuthType::SignatureKey {
            api_key,
            key1,
            api_secret,
        } => (api_key.expose(), key1.expose(), api_secret.expose()),
        _ => panic!("Expected SignatureKey auth type for adyen"),
    };

    request.metadata_mut().append(
        "x-connector",
        "adyen".parse().expect("Failed to parse x-connector"),
    );
    request.metadata_mut().append(
        "x-auth",
        "signature-key".parse().expect("Failed to parse x-auth"),
    );
    request.metadata_mut().append(
        "x-api-key",
        api_key.parse().expect("Failed to parse x-api-key"),
    );
    request
        .metadata_mut()
        .append("x-key1", key1.parse().expect("Failed to parse x-key1"));
    request.metadata_mut().append(
        "x-api-secret",
        api_secret.parse().expect("Failed to parse x-api-secret"),
    );

    let response = client
        .transform(request)
        .await
        .expect("gRPC transform call failed")
        .into_inner();

    match response.content.and_then(|c| c.content) {
        Some(GrpcWebhookContent::DisputesResponse(dispute_response)) => {
            DisputesSyncResponse {
                stage: dispute_response.dispute_stage,
                status: dispute_response.dispute_status,
                dispute_message: dispute_response.dispute_message,
                dispute_id: dispute_response.dispute_id.unwrap_or("".to_string()), // TODO need a fix
                connector_response_reference_id: None, // TODO need a fix
            }
        }
        _ => {
            //if the content is not a DisputesResponse, return a dummy DisputesSyncResponse
            DisputesSyncResponse {
                stage: 0,
                status: 0,
                dispute_message: None,
                dispute_id: "".to_string(),
                connector_response_reference_id: None,
            }
        }
    }
}

// --- Test Cases ---
// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#notification_of_chargeback
#[tokio::test]
async fn test_notification_of_chargeback_undefended() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "NOTIFICATION_OF_CHARGEBACK";
        let reason = "Fraudulent transaction";
        let adyen_dispute_status = Some("Undefended");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            grpc_api_types::payments::DisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            grpc_api_types::payments::DisputeStage::PreDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeOpened
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#notification_of_chargeback
#[tokio::test]
async fn test_notification_of_chargeback_pending() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "NOTIFICATION_OF_CHARGEBACK";
        let reason = "Product not received";
        let adyen_dispute_status = Some("Pending");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::PreDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeOpened
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback
#[tokio::test]
async fn test_chargeback_undefended() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK";
        let reason = "Service not rendered";
        let adyen_dispute_status = Some("Undefended");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeOpened
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback
#[tokio::test]
async fn test_chargeback_pending() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK";
        let reason = "Credit not processed";
        let adyen_dispute_status = Some("Pending");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeOpened
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback
#[tokio::test]
async fn test_chargeback_lost() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK";
        let reason = "Duplicate transaction";
        let adyen_dispute_status = Some("Lost");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeLost
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback
#[tokio::test]
async fn test_chargeback_accepted() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK";
        let reason = "Fraudulent transaction - merchant accepted";
        let adyen_dispute_status = Some("Accepted");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeAccepted
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback_reversed
#[tokio::test]
async fn test_chargeback_reversed_pending() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK_REVERSED";
        let reason = "Defense successful, awaiting bank review";
        let adyen_dispute_status = Some("Pending");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeChallenged
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#chargeback_reversed
#[tokio::test]
async fn test_chargeback_reversed_won() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "CHARGEBACK_REVERSED";
        let reason = "Defense accepted by bank";
        let adyen_dispute_status = Some("Won");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::ActiveDispute
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeWon
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#second_chargeback
#[tokio::test]
async fn test_second_chargeback_lost() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "SECOND_CHARGEBACK";
        let reason = "Defense declined after initial reversal";
        let adyen_dispute_status = Some("Lost");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::PreArbitration
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeLost
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#prearbitration_won
#[tokio::test]
async fn test_prearbitration_won_with_status_won() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "PREARBITRATION_WON";
        let reason = "Pre-arbitration won by merchant";
        let adyen_dispute_status = Some("Won");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::PreArbitration
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeWon
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#prearbitration_won
#[tokio::test]
async fn test_prearbitration_won_with_status_pending() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "PREARBITRATION_WON";
        let reason = "Pre-arbitration outcome pending";
        let adyen_dispute_status = Some("Pending");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::PreArbitration
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeOpened
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}

// Adyen Doc: https://docs.adyen.com/risk-management/disputes-api/dispute-notifications/#prearbitration_lost
#[tokio::test]
async fn test_prearbitration_lost() {
    grpc_test!(client, PaymentServiceClient<Channel>, {
        let event_code = "PREARBITRATION_LOST";
        let reason = "Pre-arbitration lost by merchant";
        let adyen_dispute_status = Some("Lost");

        let json_body = build_adyen_webhook_json_body(event_code, reason, adyen_dispute_status);
        let dispute_response = process_webhook_and_get_response(&mut client, json_body).await;

        assert_eq!(
            GrpcDisputeStage::try_from(dispute_response.stage)
                .expect("Failed to convert i32 to DisputeStage"),
            GrpcDisputeStage::PreArbitration
        );
        assert_eq!(
            GrpcDisputeStatus::try_from(dispute_response.status)
                .expect("Failed to convert i32 to DisputeStatus"),
            GrpcDisputeStatus::DisputeLost
        );
        assert_eq!(dispute_response.dispute_message, Some(reason.to_string()));
    });
}
