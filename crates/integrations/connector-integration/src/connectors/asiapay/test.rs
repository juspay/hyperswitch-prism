// Tests for AsiaPay connector
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use std::{marker::PhantomData, str::FromStr};

    use common_utils::{request::RequestContent, types::MinorUnit};
    use domain_types::{
        connector_flow::{Authorize, PSync, RSync, Refund},
        connector_types::{
            ConnectorEnum, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
            PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
            ResponseId,
        },
        payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData, RawCardNumber},
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        types::{ConnectorParams, Connectors},
    };
    use hyperswitch_masking::Secret;
    use interfaces::{
        api::ConnectorCommon, connector_integration_v2::BoxedConnectorIntegrationV2,
        connector_types::BoxedConnector,
    };
    use serde_json::json;

    use crate::{connectors::Asiapay, types::ConnectorData};

    pub fn create_asiapay_connector_config() -> ConnectorSpecificConfig {
        ConnectorSpecificConfig::Asiapay {
            merchant_id: Secret::new("test_merchant_id".to_string()),
            secure_hash_secret: Secret::new("test_secure_hash_secret".to_string()),
            login_id: Secret::new("test_login_id".to_string()),
            password: Secret::new("test_password".to_string()),
            base_url: Some("https://test.asiapay.com".to_string()),
        }
    }

    pub fn create_test_router_data(
        connector_config: ConnectorSpecificConfig,
        request: PaymentsAuthorizeData<DefaultPCIHolder>,
    ) -> RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: PhantomData::<Authorize>,
            resource_common_data: PaymentFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_abcdef123456".to_string(),
                attempt_id: "attempt_123456abcdef".to_string(),
                status: common_enums::AttemptStatus::Pending,
                payment_method: common_enums::PaymentMethod::Card,
                description: Some("Payment for order #12345".to_string()),
                return_url: None,
                order_details: None,
                address: domain_types::payment_address::PaymentAddress::new(None, None, None, None),
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
                connector_request_reference_id: "conn_ref_123456789".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                connectors: Connectors {
                    asiapay: ConnectorParams {
                        base_url: "https://test.asiapay.com".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                external_latency: None,
                connector_response_headers: None,
                raw_connector_response: None,
                vault_headers: None,
                raw_connector_request: None,
                minor_amount_capturable: None,
                amount: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
            },
            connector_config,
            request,
            response: Err(ErrorResponse::default()),
        }
    }

    pub fn create_test_card() -> Card<DefaultPCIHolder> {
        Card {
            card_number: RawCardNumber(cards::CardNumber::from_str("5123456789012346").unwrap()),
            card_cvc: Secret::new("100".into()),
            card_exp_month: Secret::new("12".into()),
            card_exp_year: Secret::new("2030".into()),
            card_holder_name: Some(Secret::new("Test User".into())),
            ..Default::default()
        }
    }

    pub fn create_test_authorize_request() -> PaymentsAuthorizeData<DefaultPCIHolder> {
        PaymentsAuthorizeData {
            customer_document_details: None,
            payment_channel: None,
            authentication_data: None,
            connector_testing_data: None,
            payment_method_data: PaymentMethodData::Card(create_test_card()),
            amount: MinorUnit::new(1000),
            order_tax_amount: None,
            email: None,
            customer_name: None,
            currency: common_enums::Currency::USD,
            confirm: true,
            capture_method: Some(common_enums::CaptureMethod::Automatic),
            integrity_object: None,
            router_return_url: None,
            webhook_url: None,
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
            payment_method_type: Some(common_enums::PaymentMethodType::Card),
            customer_id: None,
            request_incremental_authorization: Some(false),
            metadata: None,
            minor_amount: MinorUnit::new(1000),
            merchant_order_id: None,
            shipping_cost: None,
            merchant_account_id: None,
            merchant_config_currency: None,
            all_keys_required: None,
            access_token: None,
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
        }
    }

    #[test]
    fn test_authorize_build_request_valid() {
        let config = create_asiapay_connector_config();
        let request = create_test_authorize_request();
        let router_data = create_test_router_data(config, request);

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Asiapay::new());
        let connector_data = ConnectorData {
            connector,
            connector_name: ConnectorEnum::Asiapay,
        };

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let result = connector_integration.build_request_v2(&router_data);
        assert!(result.is_ok(), "Expected successful request build");

        let request = result.unwrap();
        let req_body = request.as_ref().map(|request_val| {
            let masked_request = match request_val.body.as_ref() {
                Some(request_content) => match request_content {
                    RequestContent::Json(i)
                    | RequestContent::FormUrlEncoded(i)
                    | RequestContent::Xml(i) => i
                        .masked_serialize()
                        .unwrap_or(json!({ "error": "failed to mask serialize connector request"})),
                    RequestContent::FormData(_) => json!({"request_type": "FORM_DATA"}),
                    RequestContent::RawBytes(_) => json!({"request_type": "RAW_BYTES"}),
                },
                None => serde_json::Value::Null,
            };
            masked_request
        });

        assert_eq!(req_body.as_ref().unwrap()["orderRef"], "conn_ref_123456789");
    }

    #[test]
    fn test_authorize_build_request_manual_capture() {
        let config = create_asiapay_connector_config();
        let mut request = create_test_authorize_request();
        request.capture_method = Some(common_enums::CaptureMethod::Manual);
        let router_data = create_test_router_data(config, request);

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Asiapay::new());
        let connector_data = ConnectorData {
            connector,
            connector_name: ConnectorEnum::Asiapay,
        };

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let result = connector_integration.build_request_v2(&router_data);
        assert!(result.is_ok(), "Expected successful request build");
    }

    #[test]
    fn test_psync_build_request_valid() {
        let config = create_asiapay_connector_config();
        let req = RouterDataV2 {
            flow: PhantomData::<PSync>,
            resource_common_data: PaymentFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_abcdef123456".to_string(),
                attempt_id: "attempt_123456abcdef".to_string(),
                status: common_enums::AttemptStatus::Pending,
                payment_method: common_enums::PaymentMethod::Card,
                description: None,
                return_url: None,
                order_details: None,
                address: domain_types::payment_address::PaymentAddress::new(None, None, None, None),
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
                connector_request_reference_id: "conn_ref_123456789".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                connectors: Connectors {
                    asiapay: ConnectorParams {
                        base_url: "https://test.asiapay.com".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                external_latency: None,
                connector_response_headers: None,
                raw_connector_response: None,
                vault_headers: None,
                raw_connector_request: None,
                minor_amount_capturable: None,
                amount: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
            },
            connector_config: config,
            request: PaymentsSyncData {
                connector_transaction_id: ResponseId::ConnectorTransactionId(
                    "pay_ref_123".to_string(),
                ),
                encoded_data: None,
                capture_method: None,
                connector_feature_data: None,
                sync_type: domain_types::router_request_types::SyncRequestType::SinglePaymentSync,
                mandate_id: None,
                payment_method_type: None,
                currency: common_enums::Currency::USD,
                payment_experience: None,
                amount: MinorUnit::new(1000),
                all_keys_required: None,
                integrity_object: None,
                split_payments: None,
                setup_future_usage: None,
            },
            response: Err(ErrorResponse::default()),
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Asiapay::new());
        let connector_data = ConnectorData {
            connector,
            connector_name: ConnectorEnum::Asiapay,
        };

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            PSync,
            PaymentFlowData,
            PaymentsSyncData,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let result = connector_integration.build_request_v2(&req);
        assert!(result.is_ok(), "Expected successful PSync request build");
    }

    #[test]
    fn test_refund_build_request_valid() {
        let config = create_asiapay_connector_config();
        let req = RouterDataV2 {
            flow: PhantomData::<Refund>,
            resource_common_data: RefundFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                status: common_enums::RefundStatus::Pending,
                refund_id: Some("ref_123".to_string()),
                connectors: Connectors {
                    asiapay: ConnectorParams {
                        base_url: "https://test.asiapay.com".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                connector_request_reference_id: "ref_conn_ref".to_string(),
                raw_connector_response: None,
                connector_response_headers: None,
                raw_connector_request: None,
                access_token: None,
                connector_feature_data: None,
                test_mode: None,
                payment_method: None,
                merchant_request_id: None,
            },
            connector_config: config,
            request: RefundsData {
                refund_id: "ref_123".to_string(),
                connector_transaction_id: "pay_ref_123".to_string(),
                connector_refund_id: None,
                customer_id: None,
                currency: common_enums::Currency::USD,
                payment_amount: 1000,
                reason: None,
                webhook_url: None,
                refund_amount: 100,
                connector_feature_data: None,
                refund_connector_metadata: None,
                minor_payment_amount: MinorUnit::new(1000),
                minor_refund_amount: MinorUnit::new(100),
                refund_status: common_enums::RefundStatus::Pending,
                merchant_account_id: None,
                capture_method: None,
                integrity_object: None,
                browser_info: None,
                split_refunds: None,
                connector_order_id: None,
                payment_method_data: None,
            },
            response: Err(ErrorResponse::default()),
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Asiapay::new());
        let connector_data = ConnectorData {
            connector,
            connector_name: ConnectorEnum::Asiapay,
        };

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Refund,
            RefundFlowData,
            RefundsData,
            RefundsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let result = connector_integration.build_request_v2(&req);
        assert!(result.is_ok(), "Expected successful Refund request build");
    }

    #[test]
    fn test_rsync_build_request_valid() {
        let config = create_asiapay_connector_config();
        let req = RouterDataV2 {
            flow: PhantomData::<RSync>,
            resource_common_data: RefundFlowData {
                merchant_id: common_utils::id_type::MerchantId::default(),
                status: common_enums::RefundStatus::Pending,
                refund_id: Some("ref_123".to_string()),
                connectors: Connectors {
                    asiapay: ConnectorParams {
                        base_url: "https://test.asiapay.com".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                connector_request_reference_id: "ref_conn_ref".to_string(),
                raw_connector_response: None,
                connector_response_headers: None,
                raw_connector_request: None,
                access_token: None,
                connector_feature_data: None,
                test_mode: None,
                payment_method: None,
                merchant_request_id: None,
            },
            connector_config: config,
            request: RefundSyncData {
                connector_transaction_id: "pay_ref_123".to_string(),
                connector_refund_id: "ref_pay_123".to_string(),
                reason: None,
                refund_connector_metadata: None,
                refund_status: common_enums::RefundStatus::Pending,
                all_keys_required: None,
                integrity_object: None,
                browser_info: None,
                split_refunds: None,
                connector_feature_data: None,
                refund_money: None,
                connector_order_id: None,
            },
            response: Err(ErrorResponse::default()),
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Asiapay::new());
        let connector_data = ConnectorData {
            connector,
            connector_name: ConnectorEnum::Asiapay,
        };

        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            RSync,
            RefundFlowData,
            RefundSyncData,
            RefundsResponseData,
        > = connector_data.connector.get_connector_integration_v2();

        let result = connector_integration.build_request_v2(&req);
        assert!(result.is_ok(), "Expected successful RSync request build");
    }

    #[test]
    fn test_asiapay_connector_creation() {
        let connector = Asiapay::<DefaultPCIHolder>::new();
        assert_eq!(connector.id(), "asiapay");
        assert!(matches!(
            connector.get_currency_unit(),
            common_enums::CurrencyUnit::Base
        ));
        assert_eq!(
            connector.common_get_content_type(),
            "application/x-www-form-urlencoded"
        );
    }

    #[test]
    fn test_normalize_field_names_basic() {
        let mut input = std::collections::HashMap::new();
        input.insert("successcode".to_string(), "0".to_string());
        input.insert("payref".to_string(), "12345".to_string());
        input.insert("orderref".to_string(), "ORD001".to_string());
        input.insert("errmsg".to_string(), "Error".to_string());

        let result = super::super::normalize_asiapay_field_names(input);
        assert_eq!(result.get("successCode"), Some(&"0".to_string()));
        assert_eq!(result.get("payRef"), Some(&"12345".to_string()));
        assert_eq!(result.get("orderRef"), Some(&"ORD001".to_string()));
        assert_eq!(result.get("errMsg"), Some(&"Error".to_string()));
        assert_eq!(result.get("resultCode"), Some(&"0".to_string()));
    }

    #[test]
    fn test_normalize_field_names_unknown_keys() {
        let mut input = std::collections::HashMap::new();
        input.insert("unknownKey".to_string(), "value".to_string());

        let result = super::super::normalize_asiapay_field_names(input);
        assert_eq!(result.get("unknownKey"), Some(&"value".to_string()));
    }

    #[test]
    fn test_map_order_status_charged() {
        use super::super::transformers::map_order_status;
        use common_enums::AttemptStatus;

        assert_eq!(map_order_status("Accepted"), AttemptStatus::Charged);
        assert_eq!(map_order_status("Captured"), AttemptStatus::Charged);
        assert_eq!(map_order_status("Accepted_Adj"), AttemptStatus::Charged);
    }

    #[test]
    fn test_map_order_status_pending() {
        use super::super::transformers::map_order_status;
        use common_enums::AttemptStatus;

        assert_eq!(map_order_status("Pending"), AttemptStatus::Pending);
        assert_eq!(
            map_order_status("Pending_3D"),
            AttemptStatus::AuthenticationPending
        );
    }

    #[test]
    fn test_map_order_status_failure() {
        use super::super::transformers::map_order_status;
        use common_enums::AttemptStatus;

        assert_eq!(map_order_status("Rejected"), AttemptStatus::Failure);
        assert_eq!(map_order_status("UnknownStatus"), AttemptStatus::Failure);
    }

    #[test]
    fn test_map_order_status_voided() {
        use super::super::transformers::map_order_status;
        use common_enums::AttemptStatus;

        assert_eq!(map_order_status("Cancelled"), AttemptStatus::Voided);
        assert_eq!(map_order_status("Voided"), AttemptStatus::Voided);
        assert_eq!(map_order_status("Reverse Auth"), AttemptStatus::Voided);
    }

    #[test]
    fn test_map_refund_status_success() {
        use super::super::transformers::map_refund_status;
        use common_enums::RefundStatus;

        assert_eq!(map_refund_status("Refunded"), RefundStatus::Success);
        assert_eq!(map_refund_status("Partial Refunded"), RefundStatus::Success);
    }

    #[test]
    fn test_map_refund_status_pending() {
        use super::super::transformers::map_refund_status;
        use common_enums::RefundStatus;

        assert_eq!(map_refund_status("Pending"), RefundStatus::Pending);
        assert_eq!(map_refund_status("RequestRefund"), RefundStatus::Pending);
    }

    #[test]
    fn test_map_refund_status_failure() {
        use super::super::transformers::map_refund_status;
        use common_enums::RefundStatus;

        assert_eq!(map_refund_status("Unknown"), RefundStatus::Failure);
    }

    #[test]
    fn test_compute_webhook_hash() {
        use super::super::transformers::{compute_asiapay_webhook_hash, AsiapayWebhookBody};

        let body = AsiapayWebhookBody {
            success_code: Some("0".to_string()),
            order_ref: Some("ORD001".to_string()),
            pay_ref: Some("PAY001".to_string()),
            amt: Some("100.00".to_string()),
            cur: Some("702".to_string()),
            prc: Some("0".to_string()),
            src: Some("0".to_string()),
            order_status: Some("Accepted".to_string()),
            secure_hash: None,
            payer_auth: Some("U".to_string()),
        };

        let secret = Secret::new("test_secret".to_string());
        let result = compute_asiapay_webhook_hash(&body, &secret);
        assert!(result.is_ok(), "Expected successful hash computation");

        let hash = result.unwrap();
        assert_eq!(hash.len(), 64); // SHA256 hex length
    }

    #[test]
    fn test_webhook_event_type_success() {
        use super::super::transformers::{map_asiapay_webhook_event_type, AsiapayWebhookBody};

        let body = AsiapayWebhookBody {
            success_code: Some("0".to_string()),
            order_ref: None,
            pay_ref: None,
            amt: None,
            cur: None,
            prc: None,
            src: None,
            order_status: Some("Refunded".to_string()),
            secure_hash: None,
            payer_auth: None,
        };

        let result = map_asiapay_webhook_event_type(&body);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            domain_types::connector_types::EventType::RefundSuccess
        );
    }

    #[test]
    fn test_webhook_event_type_failure() {
        use super::super::transformers::{map_asiapay_webhook_event_type, AsiapayWebhookBody};

        let body = AsiapayWebhookBody {
            success_code: Some("1".to_string()),
            order_ref: None,
            pay_ref: None,
            amt: None,
            cur: None,
            prc: None,
            src: None,
            order_status: None,
            secure_hash: None,
            payer_auth: None,
        };

        let result = map_asiapay_webhook_event_type(&body);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            domain_types::connector_types::EventType::PaymentIntentFailure
        );
    }

    #[test]
    fn test_error_response_parsing() {
        use super::super::transformers::AsiapayErrorResponse;

        let error_response = AsiapayErrorResponse {
            success_code: Some("1".to_string()),
            err_msg: Some("Test error".to_string()),
            prc: Some("100".to_string()),
            src: Some("01".to_string()),
        };

        assert_eq!(error_response.get_error_message(), "Test error");
    }

    #[test]
    fn test_error_response_default() {
        use super::super::transformers::AsiapayErrorResponse;

        let error_response = AsiapayErrorResponse::default();
        assert_eq!(error_response.get_error_message(), "Unknown error");
    }

    #[test]
    fn test_direct_pay_response_successful() {
        use super::super::transformers::AsiapayDirectPayResponse;

        let response = AsiapayDirectPayResponse {
            prc: Some("0".to_string()),
            src: Some("0".to_string()),
            success_code: None,
            ..Default::default()
        };

        assert!(response.is_successful());
    }

    #[test]
    fn test_direct_pay_response_unsuccessful() {
        use super::super::transformers::AsiapayDirectPayResponse;

        let response = AsiapayDirectPayResponse {
            prc: Some("1".to_string()),
            src: Some("1".to_string()),
            success_code: None,
            ..Default::default()
        };

        assert!(!response.is_successful());
    }

    #[test]
    fn test_refund_response_successful() {
        use super::super::transformers::AsiapayRefundResponse;

        let response = AsiapayRefundResponse {
            result_code: Some("0".to_string()),
            ..Default::default()
        };

        assert!(response.is_successful());
    }

    #[test]
    fn test_refund_response_successful_with_order_status_only() {
        // Simulates Query API response: result_code is null, order_status is populated.
        use super::super::transformers::AsiapayRefundResponse;

        let response = AsiapayRefundResponse {
            result_code: None,
            order_status: Some("Voided".to_string()),
            ..Default::default()
        };

        assert!(response.is_successful());
    }

    #[test]
    fn test_refund_response_unsuccessful_when_result_code_non_zero() {
        use super::super::transformers::AsiapayRefundResponse;

        let response = AsiapayRefundResponse {
            result_code: Some("1".to_string()),
            ..Default::default()
        };

        assert!(!response.is_successful());
    }

    #[test]
    fn test_currency_conversion_usd() {
        use super::super::transformers::get_asiapay_currency_code;

        let result = get_asiapay_currency_code(common_enums::Currency::USD);
        assert_eq!(result.unwrap(), "840");
    }

    #[test]
    fn test_currency_conversion_sgd() {
        use super::super::transformers::get_asiapay_currency_code;

        let result = get_asiapay_currency_code(common_enums::Currency::SGD);
        assert_eq!(result.unwrap(), "702");
    }

    #[test]
    fn test_currency_reverse_lookup() {
        use super::super::transformers::get_currency_from_asiapay_code;

        assert_eq!(
            get_currency_from_asiapay_code("840"),
            Some(common_enums::Currency::USD)
        );
        assert_eq!(
            get_currency_from_asiapay_code("702"),
            Some(common_enums::Currency::SGD)
        );
        assert_eq!(get_currency_from_asiapay_code("999"), None);
    }

    #[test]
    fn test_connector_specifications() {
        use super::super::*;

        let connector = Asiapay::<DefaultPCIHolder>::new();
        assert_eq!(connector.id(), "asiapay");
        assert!(matches!(
            connector.get_currency_unit(),
            common_enums::CurrencyUnit::Base
        ));
    }

    #[test]
    fn test_preprocess_response_url_encoded() {
        use super::super::preprocess_xml_response;

        // Simple test to verify function exists and can be called
        // Testing with a minimal case would require more complex setup
        let result = std::panic::catch_unwind(|| {
            let _ = preprocess_xml_response("<?xml version=\"1.0\"?>\n<records>\n<record>\n<successcode>0</successcode>\n</record>\n</records>");
        });
        assert!(result.is_ok());
    }
}
