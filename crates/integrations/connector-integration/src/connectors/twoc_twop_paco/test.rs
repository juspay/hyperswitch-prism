#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod void_pc {
    use std::{marker::PhantomData, sync::Arc};

    use common_enums::{AttemptStatus, PostCaptureVoidStatus};
    use domain_types::{
        connector_flow::VoidPC,
        connector_types::{PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData},
        payment_address::PaymentAddress,
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        types::{generate_payment_void_post_capture_response, Connectors},
    };
    use grpc_api_types::payments::{
        PaymentServiceReverseResponse, PaymentStatus,
        PostCaptureVoidStatus as GrpcPostCaptureVoidStatus,
    };
    use hyperswitch_masking::Secret;
    use serde_json::json;

    use crate::{
        connectors::twoc_twop_paco::transformers::{
            map_post_capture_void_status, PacoPaymentStatus, PacoPaymentStatusInfo,
            PacoPaymentStep, TwocTwopPacoNonUiResponse,
        },
        types::ResponseRouterData,
    };

    type VoidPCRouterData =
        RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>;

    const ORIGINAL_TXN_ID: &str = "120091712847";

    fn void_pc_router_data() -> VoidPCRouterData {
        RouterDataV2 {
            flow: PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_123".to_string(),
                attempt_id: "attempt_123".to_string(),
                status: AttemptStatus::Charged,
                payment_method: common_enums::PaymentMethod::Card,
                payment_method_type: None,
                description: None,
                return_url: None,
                address: PaymentAddress::default(),
                auth_type: common_enums::AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                amount_capturable: None,
                amount: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_123".to_string(),
                test_mode: Some(true),
                connector_http_status_code: None,
                connector_response_headers: None,
                external_latency: None,
                connectors: Arc::new(Connectors::default()),
                raw_connector_response: None,
                typed_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
                vault_headers: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                amount_authorized: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                connector_returned_payment_method_details: None,
                settlement_status: None,
                raw_connector_status: None,
            },
            connector_config: ConnectorSpecificConfig::TwocTwopPaco {
                access_token: Secret::new("access_token".to_string()),
                office_id: Secret::new("office_id".to_string()),
                paco_kid: Secret::new("paco_kid".to_string()),
                merchant_signing_private_key: Secret::new("signing_key".to_string()),
                merchant_encryption_private_key: Secret::new("encryption_key".to_string()),
                paco_signing_public_key: Secret::new("paco_signing_key".to_string()),
                paco_encryption_public_key: Secret::new("paco_encryption_key".to_string()),
                response_audience: None,
                base_url: None,
            },
            request: PaymentsCancelPostCaptureData {
                connector_transaction_id: ORIGINAL_TXN_ID.to_string(),
                cancellation_reason: None,
                integrity_object: None,
                raw_connector_response: None,
                browser_info: None,
                amount: None,
                payment_method_data: None,
                capture_method: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    /// Runs a PACO VoidPC response body through the connector's response transformer.
    fn transform(body: serde_json::Value, http_code: u16) -> VoidPCRouterData {
        let response: TwocTwopPacoNonUiResponse = serde_json::from_value(body).unwrap();
        VoidPCRouterData::try_from(ResponseRouterData {
            response,
            router_data: void_pc_router_data(),
            http_code,
        })
        .unwrap()
    }

    fn paco_body(payment_status: &str, payment_step: &str) -> serde_json::Value {
        json!({
            "data": {
                "paymentStatusInfo": {
                    "paymentStatus": payment_status,
                    "paymentStep": payment_step
                },
                "invoiceNo2C2P": ORIGINAL_TXN_ID,
                "orderNo": "order_123"
            },
            "apiResponse": {
                "responseCode": "4200",
                "responseDescription": "Void rejected by issuer"
            }
        })
    }

    fn reverse_response(router_data: VoidPCRouterData) -> PaymentServiceReverseResponse {
        generate_payment_void_post_capture_response(router_data).unwrap()
    }

    fn connector_error(status_code: u16) -> ErrorResponse {
        ErrorResponse {
            code: "E01".to_string(),
            message: "connector error".to_string(),
            reason: Some("connector error".to_string()),
            status_code,
            connector_transaction_id: Some("void_456".to_string()),
            ..ErrorResponse::default()
        }
    }

    fn status_info(
        payment_status: PacoPaymentStatus,
        payment_step: PacoPaymentStep,
    ) -> PacoPaymentStatusInfo {
        PacoPaymentStatusInfo {
            payment_status,
            payment_step,
        }
    }

    #[test]
    fn maps_voided_at_vd_step_to_succeeded() {
        let status =
            map_post_capture_void_status(&status_info(PacoPaymentStatus::V, PacoPaymentStep::VD));
        assert!(matches!(status, Some(PostCaptureVoidStatus::Succeeded)));
    }

    #[test]
    fn maps_failure_at_any_step_to_failed() {
        for payment_step in [
            PacoPaymentStep::VD,
            PacoPaymentStep::PA,
            PacoPaymentStep::ST,
        ] {
            let info = status_info(PacoPaymentStatus::F, payment_step);
            assert!(
                matches!(
                    map_post_capture_void_status(&info),
                    Some(PostCaptureVoidStatus::Failed)
                ),
                "expected Failed for {info:?}"
            );
        }
    }

    #[test]
    fn non_terminal_or_non_void_results_have_no_void_status() {
        for (payment_status, payment_step) in [
            (PacoPaymentStatus::P, PacoPaymentStep::VD),
            (PacoPaymentStatus::V, PacoPaymentStep::ST),
            (PacoPaymentStatus::A, PacoPaymentStep::PA),
            (PacoPaymentStatus::S, PacoPaymentStep::ST),
        ] {
            let info = status_info(payment_status, payment_step);
            assert!(
                map_post_capture_void_status(&info).is_none(),
                "expected no void status for {info:?}"
            );
        }
    }

    #[test]
    fn successful_void_returns_post_capture_void_response() {
        let router_data = transform(paco_body("V", "VD"), 200);

        match &router_data.response {
            Ok(PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status,
                connector_reference_id,
                status_code,
            }) => {
                assert!(matches!(
                    post_capture_void_status,
                    PostCaptureVoidStatus::Succeeded
                ));
                assert_eq!(connector_reference_id.as_deref(), Some(ORIGINAL_TXN_ID));
                assert_eq!(*status_code, 200);
            }
            other => panic!("expected PostCaptureVoidResponse, got {other:?}"),
        }
        // The void outcome never changes the payment's attempt status.
        assert_eq!(
            router_data.resource_common_data.status,
            AttemptStatus::Charged
        );
    }

    #[test]
    fn declined_void_on_2xx_returns_connector_error_response() {
        let router_data = transform(paco_body("F", "VD"), 200);

        let error = router_data
            .response
            .as_ref()
            .expect_err("a declined void must be an ErrorResponse");
        assert_eq!(error.code, "4200");
        assert_eq!(error.message, "Void rejected by issuer");
        assert_eq!(error.reason.as_deref(), Some("Void rejected by issuer"));
        assert_eq!(error.status_code, 200);
        assert!(error.attempt_status.is_none());
        assert_eq!(
            error.connector_transaction_id.as_deref(),
            Some(ORIGINAL_TXN_ID)
        );
        assert_eq!(
            router_data.resource_common_data.status,
            AttemptStatus::Charged
        );
    }

    #[test]
    fn void_without_final_status_is_pending() {
        for body in [
            paco_body("P", "VD"),
            json!({ "apiResponse": { "responseCode": "0000", "responseDescription": "Success" } }),
        ] {
            let router_data = transform(body, 200);
            assert!(matches!(
                router_data.response,
                Ok(PaymentsResponseData::PostCaptureVoidResponse {
                    post_capture_void_status: PostCaptureVoidStatus::Pending,
                    ..
                })
            ));
        }
    }

    #[test]
    fn reverse_response_for_successful_void_has_no_error() {
        let response = reverse_response(transform(paco_body("V", "VD"), 200));

        assert_eq!(
            response.post_capture_void_status(),
            GrpcPostCaptureVoidStatus::Succeeded
        );
        #[allow(deprecated)]
        let status = response.status();
        assert_eq!(status, PaymentStatus::Unspecified);
        assert_eq!(response.connector_transaction_id, ORIGINAL_TXN_ID);
        assert_eq!(
            response.connector_reference_id.as_deref(),
            Some(ORIGINAL_TXN_ID)
        );
        assert!(response.error.is_none());
    }

    #[test]
    fn reverse_response_for_declined_2xx_void_is_failed_with_connector_error() {
        let response = reverse_response(transform(paco_body("F", "VD"), 200));

        assert_eq!(
            response.post_capture_void_status(),
            GrpcPostCaptureVoidStatus::Failed
        );
        assert_eq!(response.connector_transaction_id, ORIGINAL_TXN_ID);
        assert!(response.connector_reference_id.is_none());
        let details = response.error.unwrap().connector_details.unwrap();
        assert_eq!(details.code.as_deref(), Some("4200"));
        assert_eq!(details.message.as_deref(), Some("Void rejected by issuer"));
    }

    #[test]
    fn reverse_response_for_4xx_error_is_failed() {
        let mut router_data = void_pc_router_data();
        router_data.response = Err(connector_error(400));

        let response = reverse_response(router_data);

        assert_eq!(
            response.post_capture_void_status(),
            GrpcPostCaptureVoidStatus::Failed
        );
        assert_eq!(response.status_code, 400);
        assert_eq!(response.connector_transaction_id, ORIGINAL_TXN_ID);
        assert!(response.connector_reference_id.is_none());
        let details = response.error.unwrap().connector_details.unwrap();
        assert_eq!(details.code.as_deref(), Some("E01"));
        assert_eq!(
            details.connector_transaction_id.as_deref(),
            Some("void_456")
        );
    }

    #[test]
    fn reverse_response_for_5xx_error_is_pending() {
        for status_code in [500, 503, 511] {
            let mut router_data = void_pc_router_data();
            router_data.response = Err(connector_error(status_code));

            let response = reverse_response(router_data);

            assert_eq!(
                response.post_capture_void_status(),
                GrpcPostCaptureVoidStatus::Pending,
                "status code {status_code}"
            );
            assert!(response.connector_reference_id.is_none());
            assert!(response.error.is_some());
        }
    }
}
