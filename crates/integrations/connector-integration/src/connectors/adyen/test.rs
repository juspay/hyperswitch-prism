#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::print_stdout)]
mod tests {
    pub mod authorize {
        use std::{borrow::Cow, marker::PhantomData, str::FromStr};

        use common_utils::{pii::Email, request::RequestContent, types::MinorUnit};
        use domain_types::{
            connector_flow::Authorize,
            connector_types::{
                ConnectorEnum, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
            },
            payment_method_data::{DefaultPCIHolder, PaymentMethodData, RawCardNumber},
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };
        use hyperswitch_masking::Secret;
        use interfaces::{
            connector_integration_v2::BoxedConnectorIntegrationV2, connector_types::BoxedConnector,
        };
        use serde_json::json;

        use crate::{connectors::Adyen, types::ConnectorData};
        #[test]
        fn test_build_request_valid() {
            let api_key = "test_adyen_api_key".to_string(); // Hardcoded dummy value
            let key1 = "test_adyen_key1".to_string(); // Hardcoded dummy value
            let req: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: Some("conn_cust_987654".to_string()),
                    payment_id: "pay_abcdef123456".to_string(),
                    attempt_id: "attempt_123456abcdef".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    description: Some("Payment for order #12345".to_string()),
                    return_url: Some("www.google.com".to_string()),
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
                    auth_type: common_enums::AuthenticationType::ThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: None,
                    payment_method_token: None,
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "conn_ref_123456789".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors {
                        adyen: ConnectorParams {
                            base_url: "https://checkout-test.adyen.com/".to_string(),
                            dispute_base_url: Some("https://ca-test.adyen.com/ca/services/DisputeService/v30/defendDispute".to_string()),
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
                    l2_l3_data: None
},
                connector_config: ConnectorSpecificConfig::Adyen {
                    api_key: Secret::new(api_key),
                    merchant_account: Secret::new(key1),
                    review_key: None,
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None
},
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(
                        domain_types::payment_method_data::Card {
                            card_number: RawCardNumber(cards::CardNumber::from_str(
                                "5123456789012346",
                            )
                            .unwrap()),
                            card_cvc: Secret::new("100".into()),
                            card_exp_month: Secret::new("03".into()),
                            card_exp_year: Secret::new("2030".into()),
                            ..Default::default()
                        },
                    ),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: Some(
                        Email::try_from("test@example.com".to_string())
                            .expect("Failed to parse email"),
                    ),
                    customer_name: None,
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    capture_method: None,
                    integrity_object: None,
                    router_return_url: Some("www.google.com".to_string()),
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: Some(
                        domain_types::router_request_types::BrowserInformation {
                            color_depth: Some(24),
                            java_enabled: Some(false),
                            screen_height: Some(1080),
                            screen_width: Some(1920),
                            user_agent: Some(
                                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string(),
                            ),
                            accept_header: Some(
                                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                                    .to_string(),
                            ),
                            java_script_enabled: Some(false),
                            language: Some("en-US".to_string()),
                            time_zone: Some(-480),
                            referer: None,
                            ip_address: None,
                            os_type: None,
                            os_version: None,
                            device_model: None,
                            accept_language: None
},
                    ),
                    order_category: None,
                    session_token: None,
                    enrolled_for_3ds: Some(true),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(common_enums::PaymentMethodType::Card),
                    customer_id: Some(
                        common_utils::id_type::CustomerId::try_from(Cow::from(
                            "cus_123456789".to_string(),
                        ))
                        .unwrap(),
                    ),
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
                    tokenization: None
},
                response: Err(ErrorResponse::default())
};

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Adyen::new());
            let connector_data = ConnectorData {
                connector,
                connector_name: ConnectorEnum::Adyen,
            };

            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = connector_data.connector.get_connector_integration_v2();

            let request = connector_integration.build_request_v2(&req).unwrap();
            let req_body = request.as_ref().map(|request_val| {
                let masked_request = match request_val.body.as_ref() {
                    Some(request_content) => match request_content {
                        RequestContent::Json(i)
                        | RequestContent::FormUrlEncoded(i)
                        | RequestContent::Xml(i) => i.masked_serialize().unwrap_or(
                            json!({ "error": "failed to mask serialize connector request"}),
                        ),
                        RequestContent::FormData(_) => json!({"request_type": "FORM_DATA"}),
                        RequestContent::RawBytes(_) => json!({"request_type": "RAW_BYTES"}),
                    },
                    None => serde_json::Value::Null,
                };
                masked_request
            });
            println!("request: {req_body:?}");
            assert_eq!(
                req_body.as_ref().unwrap()["reference"],
                "conn_ref_123456789"
            );
        }
        #[test]
        fn test_build_request_missing() {
            let api_key = "test_adyen_api_key_missing".to_string(); // Hardcoded dummy value
            let key1 = "test_adyen_key1_missing".to_string(); // Hardcoded dummy value
            let req: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "".to_string(),
                    attempt_id: "".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
                    auth_type: common_enums::AuthenticationType::ThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: None,
                    payment_method_token: None,
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors {
                        adyen: ConnectorParams {
                            base_url: "https://checkout-test.adyen.com/".to_string(),
                            dispute_base_url: Some("https://ca-test.adyen.com/ca/services/DisputeService/v30/defendDispute".to_string()),
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
                    l2_l3_data: None
},
                connector_config: ConnectorSpecificConfig::Adyen {
                    api_key: Secret::new(api_key),
                    merchant_account: Secret::new(key1),
                    review_key: None,
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None
},
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Default::default()),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: None,
                    customer_name: None,
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: None,
                    integrity_object: None,
                    order_category: None,
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: None,
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    minor_amount: MinorUnit::new(0),
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
                    tokenization: None
},
                response: Err(ErrorResponse::default())
};

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Adyen::new());
            let connector_data = ConnectorData {
                connector,
                connector_name: ConnectorEnum::Adyen,
            };

            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = connector_data.connector.get_connector_integration_v2();

            let result = connector_integration.build_request_v2(&req);
            assert!(result.is_err(), "Expected error for missing fields");
        }
        // #[test]
        // fn test_build_request_invalid() {
        //     let api_key = env::var("API_KEY").expect("API_KEY not set");
        //     let key1 = env::var("KEY1").expect("KEY1 not set");
        //     let req: RouterDataV2<
        //         Authorize,
        //         PaymentFlowData,
        //         PaymentsAuthorizeData,
        //         PaymentsResponseData
        //     > = RouterDataV2 {
        //         flow: PhantomData::<Authorize>,
        //         resource_common_data: PaymentFlowData {
        //             merchant_id: common_utils::id_type::MerchantId::default(),
        //             customer_id: None,
        //             connector_customer: None,
        //             payment_id: "pay_invalid".to_string(),
        //             attempt_id: "attempt_invalid".to_string(),
        //             status: common_enums::AttemptStatus::Pending,
        //             payment_method: common_enums::PaymentMethod::Card,
        //             description: Some("Invalid test".to_string()),
        //             return_url: None,
        //             address: domain_types::payment_address::PaymentAddress::new(
        //                 None,
        //                 None,
        //                 None,
        //                 None
        //             ),
        //             auth_type: common_enums::AuthenticationType::ThreeDs,
        //             connector_meta_data: None,
        //             amount_captured: None,
        //             minor_amount_captured: None,
        //             access_token: None,
        //             session_token: None,
        //             reference_id: None,
        //             payment_method_token: None,
        //             preprocessing_id: None,
        //             connector_api_version: None,
        //             connector_request_reference_id: "invalid_ref".to_string(),
        //             test_mode: None,
        //             connector_http_status_code: None,
        //             connectors: Connectors {
        //                 adyen: ConnectorParams {
        //                     base_url: "https://checkout-test.adyen.com/".to_string(),
        //                 },
        //                 razorpay: ConnectorParams {
        //                     base_url: "https://sandbox.juspay.in/".to_string(),
        //                 },
        //             },
        //             external_latency: None,
        //         },
        //         connector_config: ConnectorSpecificConfig::BodyKey {
        //             api_key: Secret::new(api_key.into()),
        //             key1: Secret::new(key1.into()),
        //,
        //         },
        //         request: PaymentsAuthorizeData {
        //             payment_method_data: PaymentMethodData::Card(
        //                 (domain_types::payment_method_data::Card {
        //                     card_number: cards::CardNumber
        //                         ::from_str("1234567890123456")
        //                         .unwrap(),
        //                     card_cvc: Secret::new("12".into()),
        //                     card_exp_month: Secret::new("00".into()), // invalid month
        //                     card_exp_year: Secret::new("1999".into()), // past year
        //                     ..Default::default()
        //                 }).into()
        //             ),
        //             amount: 100,
        //             order_tax_amount: None,
        //             email: Some("invalid-email".to_string())
        //                 .map(|email_str| Email::try_from(email_str))
        //                 .transpose()
        //                 .unwrap_or(None),
        //             customer_name: None,
        //             currency: common_enums::Currency::USD,
        //             confirm: true,
        //             statement_descriptor_suffix: None,
        //             statement_descriptor: None,
        //             capture_method: None,
        //             router_return_url: None,
        //             webhook_url: None,
        //             complete_authorize_url: None,
        //             mandate_id: None,
        //             setup_future_usage: None,
        //             off_session: None,
        //             browser_info: None,
        //             order_category: None,
        //             session_token: None,
        //             enrolled_for_3ds: Some(false),
        //             related_transaction_id: None,
        //             payment_experience: None,
        //             payment_method_type: None,
        //             customer_id: None,
        //             request_incremental_authorization: Some(false),
        //             metadata: None,
        //             minor_amount: MinorUnit::new(100),
        //             merchant_order_id: None,
        //             shipping_cost: None,
        //             merchant_account_id: None,
        //             merchant_config_currency: None,
        //         },
        //         response: Err(ErrorResponse::default()),
        //     };

        //     let connector: BoxedConnector = Box::new(Adyen::new());
        //     let connector_data = ConnectorData {
        //         connector,
        //         connector_name: ConnectorEnum::Adyen,
        //     };

        //     let connector_integration: BoxedConnectorIntegrationV2<
        //         '_,
        //         Authorize,
        //         PaymentFlowData,
        //         PaymentsAuthorizeData,
        //         PaymentsResponseData
        //     > = connector_data.connector.get_connector_integration_v2();

        //     let result = connector_integration.build_request_v2(&req);
        //     assert!(result.is_err(), "Expected error for invalid fields");
        // }
    }
}
