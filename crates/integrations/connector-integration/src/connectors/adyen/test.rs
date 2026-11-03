#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::print_stdout)]
#[allow(clippy::unwrap_in_result)]
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
            let address_with_full_state = domain_types::payment_address::Address {
                address: Some(domain_types::payment_address::AddressDetails {
                    city: Some(Secret::new("London".to_string())),
                    country: Some(common_enums::CountryAlpha2::GB),
                    line1: Some(Secret::new("10".to_string())),
                    line2: Some(Secret::new("Main Street".to_string())),
                    zip: Some(Secret::new("SW1A 1AA".to_string())),
                    state: Some(Secret::new("A Region With A Long Name".to_string())),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let address_with_short_state = domain_types::payment_address::Address {
                address: Some(domain_types::payment_address::AddressDetails {
                    city: Some(Secret::new("Zurich".to_string())),
                    country: Some(common_enums::CountryAlpha2::CH),
                    line1: Some(Secret::new("1".to_string())),
                    line2: Some(Secret::new("Main Street".to_string())),
                    zip: Some(Secret::new("8001".to_string())),
                    state: Some(Secret::new("ZH".to_string())),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let req: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    raw_connector_status: None,
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: Some("conn_cust_987654".to_string()),
                    payment_id: "pay_abcdef123456".to_string(),
                    attempt_id: "attempt_123456abcdef".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    payment_method_type: None,
                    description: Some("Payment for order #12345".to_string()),
                    return_url: Some("https://example.com/return".to_string()),
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        Some(address_with_short_state),
                        Some(address_with_full_state),
                        None,
                        None,
                    ),
                    auth_type: common_enums::AuthenticationType::ThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    amount_authorized: None,
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
                        adyen: ConnectorParams {
                            base_url: "https://checkout-test.adyen.com/".to_string(),
                            dispute_base_url: Some("https://ca-test.adyen.com/ca/services/DisputeService/v30/defendDispute".to_string()),
                            ..Default::default()
                        },
                        ..Default::default()
                    }.into(),
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    vault_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    connector_returned_payment_method_details: None,
                    settlement_status: None,
                    typed_connector_response: None,
},
                connector_config: ConnectorSpecificConfig::Adyen {
                    api_key: Secret::new(api_key),
                    merchant_account: Secret::new(key1),
                    review_key: None,
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None,
                    native_three_ds: None
},
                request: PaymentsAuthorizeData {
                    risk_assessment_data: None,
                    split_settlement: None,
                    customer_document_details: None,
                    customer_date_of_birth: None,
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    currency_conversion_data: None,
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
                    amount: common_utils::types::Money { amount: MinorUnit::new(1000), currency: common_enums::Currency::USD },
                    order_tax_amount: None,
                    surcharge_amount: None,
                    email: Some(
                        Email::try_from("test@example.com".to_string())
                            .expect("Failed to parse email"),
                    ),
                    customer_name: None,
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    capture_method: None,
                    integrity_object: None,
                    router_return_url: Some("https://example.com/return".to_string()),
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    is_stored_credential: None,
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
                    integrity_specifications: None,
                    connector_feature_data: None,
                    billing_descriptor: None,
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                    mit_category: None,
                    domain_data: None,
                    partner_merchant_identifier_details: None,
                    is_account_funding_transaction: None,
                    recipient_details: None,
                    business_country: None,
                    enable_avs_check: None,
                    additional_connector_details: None,
                    customer: None,
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
            assert_eq!(
                request.as_ref().unwrap().url,
                "https://checkout-test.adyen.com/v72/payments"
            );
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
            assert_eq!(
                req_body.as_ref().unwrap()["returnUrl"],
                "https://example.com/return"
            );
            let body = req_body.as_ref().unwrap();
            assert_eq!(body["billingAddress"]["country"], "GB");
            assert!(body["billingAddress"].get("stateOrProvince").is_none());
            assert!(body["deliveryAddress"]["stateOrProvince"].is_string());
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
                    raw_connector_status: None,
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "".to_string(),
                    attempt_id: "".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    payment_method_type: None,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
                    auth_type: common_enums::AuthenticationType::ThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: None,
                    connector_order_id: None,
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
                    }.into(),
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    vault_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    connector_returned_payment_method_details: None,
                    settlement_status: None,
                    typed_connector_response: None,
},
                connector_config: ConnectorSpecificConfig::Adyen {
                    api_key: Secret::new(api_key),
                    merchant_account: Secret::new(key1),
                    review_key: None,
                    base_url: None,
                    dispute_base_url: None,
                    endpoint_prefix: None,
                    native_three_ds: None
},
                request: PaymentsAuthorizeData {
                    risk_assessment_data: None,
                    split_settlement: None,
                    customer_document_details: None,
                    customer_date_of_birth: None,
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    currency_conversion_data: None,
                    payment_method_data: PaymentMethodData::Card(Default::default()),
                    amount: common_utils::types::Money { amount: MinorUnit::new(1000), currency: common_enums::Currency::USD },
                    order_tax_amount: None,
                    surcharge_amount: None,
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
                    is_stored_credential: None,
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
                    integrity_specifications: None,
                    connector_feature_data: None,
                    billing_descriptor: None,
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                    mit_category: None,
                    domain_data: None,
                    partner_merchant_identifier_details: None,
                    is_account_funding_transaction: None,
                    recipient_details: None,
                    business_country: None,
                    enable_avs_check: None,
                    additional_connector_details: None,
                    customer: None,
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
        //             amount_authorized: None,
        //             access_token: None,
        //             session_token: None,
        //             reference_id: None,
        //
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
        //             minor_amount: common_utils::types::Money { amount: MinorUnit::new(100), currency: common_enums::Currency::USD },
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

    /// `RouterDataV2` scaffolding shared by the native 3DS2 and Level 2/3 tests. The flow data
    /// has fifty-odd fields and at most two matter per test, so it is built once here.
    pub mod fixtures {
        use domain_types::{
            connector_types::PaymentFlowData,
            router_data::ConnectorSpecificConfig,
            types::{ConnectorParams, Connectors},
        };
        use hyperswitch_masking::Secret;

        pub fn payment_flow_data() -> PaymentFlowData {
            PaymentFlowData {
                raw_connector_status: None,
                merchant_id: common_utils::id_type::MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_adyen_test".to_string(),
                attempt_id: "attempt_adyen_test".to_string(),
                status: common_enums::AttemptStatus::Pending,
                payment_method: common_enums::PaymentMethod::Card,
                payment_method_type: None,
                description: None,
                return_url: Some("https://hs.example/return".to_string()),
                order_details: None,
                address: domain_types::payment_address::PaymentAddress::new(None, None, None, None),
                auth_type: common_enums::AuthenticationType::ThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "conn_ref_adyen_test".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                connectors: Connectors {
                    adyen: ConnectorParams {
                        base_url: "https://checkout-test.adyen.com/".to_string(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                external_latency: None,
                connector_response_headers: None,
                raw_connector_response: None,
                vault_headers: None,
                raw_connector_request: None,
                typed_connector_request: None,
                amount_capturable: None,
                amount: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                connector_returned_payment_method_details: None,
                settlement_status: None,
                typed_connector_response: None,
            }
        }

        pub fn adyen_config() -> ConnectorSpecificConfig {
            ConnectorSpecificConfig::Adyen {
                api_key: Secret::new("test_adyen_api_key".to_string()),
                merchant_account: Secret::new("test_adyen_merchant_account".to_string()),
                review_key: None,
                base_url: None,
                dispute_base_url: None,
                endpoint_prefix: None,
                native_three_ds: Some(true),
            }
        }
    }

    /// Native (SDK-less) 3DS2 standalone authentication: the untagged variant order that keeps a
    /// `threeDS2` action from parsing as a redirect, the `action.token` decode, and the outcome
    /// table of each of the three legs — PostAuthenticate included, which the integration
    /// harness cannot reach.
    ///
    /// Everything here goes through the public surface: a response body is `from_str`-parsed into
    /// the leg's response type and handed to its `TryFrom<ResponseRouterData<..>>`, which is the
    /// only caller of the private decoder, state helpers and outcome table.
    pub mod native_three_ds {
        use std::{collections::HashMap, marker::PhantomData};

        use base64::Engine;
        use common_enums::{AttemptStatus, CaptureMethod};
        use common_utils::types::{MinorUnit, Money};
        use domain_types::{
            connector_flow::{Authenticate, PostAuthenticate, PreAuthenticate},
            connector_types::{
                PaymentFlowData, PaymentsAuthenticateData, PaymentsPostAuthenticateData,
                PaymentsPreAuthenticateData, PaymentsResponseData,
            },
            payment_method_data::DefaultPCIHolder,
            router_data::ErrorResponse,
            router_data_v2::RouterDataV2,
            router_request_types::AuthenticationData,
            router_response_types::RedirectForm,
        };

        use super::fixtures::{adyen_config, payment_flow_data};
        use crate::{
            connectors::adyen::transformers::{
                AdyenAuthenticationResponse, AdyenPaymentResponse, AdyenPostAuthenticationResponse,
                AdyenPreAuthenticateRequest, AdyenPreAuthenticationResponse,
            },
            connectors::adyen::AdyenRouterData,
            types::ResponseRouterData,
        };

        type PreAuthRouterData = RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<DefaultPCIHolder>,
            PaymentsResponseData,
        >;
        type AuthRouterData = RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<DefaultPCIHolder>,
            PaymentsResponseData,
        >;
        type PostAuthRouterData = RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<DefaultPCIHolder>,
            PaymentsResponseData,
        >;
        type LegResponse = Result<PaymentsResponseData, ErrorResponse>;

        // ===== inputs =====

        fn b64url(payload: &str) -> String {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload)
        }

        /// `action.token` of a `fingerprint` action: base64url(no pad) JSON.
        fn fingerprint_token() -> String {
            b64url(
                r#"{"threeDSMethodUrl":"https://acs.example/3ds-method","threeDSMethodNotificationURL":"https://hs.example/notify","threeDSServerTransID":"srv-1","threeDSMessageVersion":"2.2.0"}"#,
            )
        }

        /// `action.token` of a `challenge` action.
        fn challenge_token() -> String {
            b64url(
                r#"{"acsURL":"https://acs.example/challenge","acsTransID":"acs-1","acsReferenceNumber":"ref-1","messageVersion":"2.2.0","threeDSNotificationURL":"https://adyen.example/notify","threeDSServerTransID":"srv-1"}"#,
            )
        }

        /// A `resultCode` + `threeDS2` action body, as adyen sends it.
        fn action_body(result_code: &str, subtype: &str, token: &str) -> String {
            format!(
                r#"{{"resultCode":"{result_code}","action":{{"type":"threeDS2","subtype":"{subtype}","token":"{token}","paymentData":"Ab02b4c0!payment-data","authorisationToken":"Ab02b4c0!auth-token","paymentMethodType":"scheme"}},"pspReference":"psp-1","merchantReference":"ref-1"}}"#
            )
        }

        /// An `AuthenticationFinished` body with the `threeDS2Result` adyen returns.
        fn authentication_finished_body(trans_status: &str, eci: &str) -> String {
            format!(
                r#"{{"resultCode":"AuthenticationFinished","pspReference":"psp-1","merchantReference":"ref-1","threeDS2Result":{{"authenticationValue":"cavv-value","eci":"{eci}","transStatus":"{trans_status}","dsTransID":"ds-1","threeDSServerTransID":"srv-1","messageVersion":"2.2.0"}}}}"#
            )
        }

        /// A body carrying only a `resultCode`, plus a refusal reason where adyen sends one.
        fn result_code_body(result_code: &str) -> String {
            format!(
                r#"{{"resultCode":"{result_code}","pspReference":"psp-1","merchantReference":"ref-1","refusalReason":"Refused","refusalReasonCode":"2"}}"#
            )
        }

        // ===== the three legs, through their public response transforms =====

        fn pre_authenticate_request(capture_method: Option<CaptureMethod>) -> PreAuthRouterData {
            RouterDataV2 {
                flow: PhantomData::<PreAuthenticate>,
                resource_common_data: payment_flow_data(),
                connector_config: adyen_config(),
                request: PaymentsPreAuthenticateData {
                    payment_method_data: None,
                    amount: Money {
                        amount: MinorUnit::new(1000),
                        currency: common_enums::Currency::USD,
                    },
                    email: None,
                    currency: Some(common_enums::Currency::USD),
                    payment_method_type: None,
                    router_return_url: None,
                    continue_redirection_url: None,
                    browser_info: None,
                    enrolled_for_3ds: true,
                    redirect_response: None,
                    capture_method,
                    mandate_reference: None,
                    merchant_transaction_id: None,
                    metadata: None,
                    customer_date_of_birth: None,
                },
                response: Err(ErrorResponse::default()),
            }
        }

        fn adyen_config_without_native_three_ds(
        ) -> domain_types::router_data::ConnectorSpecificConfig {
            match adyen_config() {
                domain_types::router_data::ConnectorSpecificConfig::Adyen {
                    api_key,
                    merchant_account,
                    review_key,
                    base_url,
                    dispute_base_url,
                    endpoint_prefix,
                    ..
                } => domain_types::router_data::ConnectorSpecificConfig::Adyen {
                    api_key,
                    merchant_account,
                    review_key,
                    base_url,
                    dispute_base_url,
                    endpoint_prefix,
                    native_three_ds: None,
                },
                other => other,
            }
        }

        fn pre_authenticate(
            body: &str,
            capture_method: Option<CaptureMethod>,
        ) -> PreAuthRouterData {
            let response = serde_json::from_str::<AdyenPreAuthenticationResponse>(body).unwrap();
            let router_data = pre_authenticate_request(capture_method);
            RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data,
                http_code: 200,
            })
            .unwrap()
        }

        fn authenticate(body: &str) -> AuthRouterData {
            let response = serde_json::from_str::<AdyenAuthenticationResponse>(body).unwrap();
            let router_data: AuthRouterData = RouterDataV2 {
                flow: PhantomData::<Authenticate>,
                resource_common_data: payment_flow_data(),
                connector_config: adyen_config(),
                request: PaymentsAuthenticateData {
                    payment_method_data: None,
                    amount: Money {
                        amount: MinorUnit::new(1000),
                        currency: common_enums::Currency::USD,
                    },
                    email: None,
                    currency: Some(common_enums::Currency::USD),
                    payment_method_type: None,
                    router_return_url: None,
                    continue_redirection_url: None,
                    browser_info: None,
                    enrolled_for_3ds: true,
                    redirect_response: None,
                    capture_method: None,
                    authentication_data: None,
                    webhook_url: None,
                    domain_data: None,
                    sdk_information: None,
                    device_channel: None,
                    setup_future_usage: None,
                    customer_acceptance: None,
                    enable_partial_authorization: None,
                    payment_channel: None,
                    billing_descriptor: None,
                },
                response: Err(ErrorResponse::default()),
            };
            RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data,
                http_code: 200,
            })
            .unwrap()
        }

        fn post_authenticate(body: &str) -> PostAuthRouterData {
            let response = serde_json::from_str::<AdyenPostAuthenticationResponse>(body).unwrap();
            let router_data: PostAuthRouterData = RouterDataV2 {
                flow: PhantomData::<PostAuthenticate>,
                resource_common_data: payment_flow_data(),
                connector_config: adyen_config(),
                request: PaymentsPostAuthenticateData {
                    payment_method_data: None,
                    amount: Money {
                        amount: MinorUnit::new(1000),
                        currency: common_enums::Currency::USD,
                    },
                    email: None,
                    currency: Some(common_enums::Currency::USD),
                    payment_method_type: None,
                    router_return_url: None,
                    continue_redirection_url: None,
                    browser_info: None,
                    enrolled_for_3ds: true,
                    redirect_response: None,
                    capture_method: None,
                    connector_order_reference_id: None,
                },
                response: Err(ErrorResponse::default()),
            };
            RouterDataV2::try_from(ResponseRouterData {
                response,
                router_data,
                http_code: 200,
            })
            .unwrap()
        }

        // ===== reading an outcome =====

        fn authentication_data(data: &PaymentsResponseData) -> Option<&AuthenticationData> {
            match data {
                PaymentsResponseData::PreAuthenticateResponse {
                    authentication_data,
                    ..
                }
                | PaymentsResponseData::AuthenticateResponse {
                    authentication_data,
                    ..
                }
                | PaymentsResponseData::PostAuthenticateResponse {
                    authentication_data,
                    ..
                } => authentication_data.as_ref(),
                _ => None,
            }
        }

        /// `(endpoint, form_fields)` of the browser form the leg handed back.
        fn redirect_form(data: &PaymentsResponseData) -> Option<(String, HashMap<String, String>)> {
            let form = match data {
                PaymentsResponseData::PreAuthenticateResponse {
                    redirection_data, ..
                }
                | PaymentsResponseData::AuthenticateResponse {
                    redirection_data, ..
                } => redirection_data.as_deref(),
                _ => None,
            }?;
            match form {
                RedirectForm::Form {
                    endpoint,
                    form_fields,
                    ..
                } => Some((endpoint.clone(), form_fields.clone())),
                _ => None,
            }
        }

        /// `(status, produced authentication data, produced a shopper form)`.
        fn outcome(status: AttemptStatus, response: &LegResponse) -> (AttemptStatus, bool, bool) {
            let data = response.as_ref().ok();
            (
                status,
                data.and_then(authentication_data).is_some(),
                data.and_then(redirect_form).is_some(),
            )
        }

        /// `(code, message, reason)` of a refused leg. `None` when the leg did not fail.
        fn refusal(response: &LegResponse) -> Option<(String, String, Option<String>)> {
            response.as_ref().err().map(|error| {
                (
                    error.code.clone(),
                    error.message.clone(),
                    error.reason.clone(),
                )
            })
        }

        fn form_field(fields: &HashMap<String, String>, key: &str) -> String {
            fields.get(key).cloned().unwrap_or_default()
        }

        /// The EMVCo blobs the forms carry are base64url(no pad) JSON.
        fn decode_b64url(value: &str) -> serde_json::Value {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(value)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or(serde_json::Value::Null)
        }

        // ===== (a) untagged variant order =====

        /// `ThreeDs2Action` must stay ahead of `RedirectionResponse` in the untagged enum:
        /// `AdyenRedirectAction`'s url / method / data are all optional, so a `threeDS2` body
        /// parses cleanly as a redirect, yields `redirection_data: None` and hangs the payment.
        /// Without the opt-in Adyen answers the redirect variant, which these legs cannot drive, so
        /// no request is built. Refused ahead of any other validation.
        #[test]
        fn pre_authenticate_is_refused_without_the_native_three_ds_flag() {
            let connector = crate::connectors::Adyen::<DefaultPCIHolder>::new().clone();
            let mut router_data = pre_authenticate_request(None);
            router_data.connector_config = adyen_config_without_native_three_ds();
            let result = AdyenPreAuthenticateRequest::try_from(AdyenRouterData {
                connector,
                router_data,
            });
            assert!(matches!(
                result.map_err(|err| err.current_context().clone()),
                Err(domain_types::errors::IntegrationError::NotSupported { .. })
            ));
        }

        #[test]
        fn three_ds2_action_body_is_not_a_redirection_response() {
            let body = format!(
                r#"{{"resultCode":"IdentifyShopper","action":{{"type":"threeDS2","subtype":"fingerprint","token":"{}","paymentData":"Ab02b4c0!payment-data","authorisationToken":"Ab02b4c0!auth-token","paymentMethodType":"scheme"}},"pspReference":"X"}}"#,
                fingerprint_token()
            );
            assert!(matches!(
                serde_json::from_str::<AdyenPaymentResponse>(&body),
                Ok(AdyenPaymentResponse::ThreeDs2Action(_))
            ));
        }

        /// ... and a real redirect action must still reach `RedirectionResponse`: the required
        /// `subtype` + `token` on the native action are what keep the two apart.
        #[test]
        fn redirect_action_body_still_parses_as_a_redirection_response() {
            let body = r#"{"resultCode":"RedirectShopper","action":{"paymentMethodType":"scheme","type":"redirect","url":"https://acs.example/redirect","method":"GET"},"pspReference":"X"}"#;
            assert!(matches!(
                serde_json::from_str::<AdyenPaymentResponse>(body),
                Ok(AdyenPaymentResponse::RedirectionResponse(_))
            ));
        }

        // ===== (b) action.token decode =====

        /// The decoded fingerprint token becomes the hidden-iframe 3DS-method form. The field
        /// spellings are the ones hyperswitch's converter sniffs for, so they are asserted
        /// verbatim.
        #[test]
        fn fingerprint_action_token_decodes_into_the_three_ds_method_form() {
            let out = pre_authenticate(
                &action_body("IdentifyShopper", "fingerprint", &fingerprint_token()),
                None,
            );
            let (endpoint, fields) = out
                .response
                .as_ref()
                .ok()
                .and_then(redirect_form)
                .unwrap_or_default();
            assert_eq!(endpoint, "https://acs.example/3ds-method");
            assert_eq!(
                (
                    form_field(&fields, "threeDsMethodUrl"),
                    form_field(&fields, "threeDSServerTransID"),
                    form_field(&fields, "messageVersion"),
                    form_field(&fields, "threeDsMethodDataSubmission"),
                ),
                (
                    "https://acs.example/3ds-method".to_string(),
                    "srv-1".to_string(),
                    "2.2.0".to_string(),
                    "true".to_string(),
                )
            );
            // `threeDSMethodData` is the EMVCo blob the ACS reads back.
            let method_data = decode_b64url(&form_field(&fields, "threeDsMethodData"));
            assert_eq!(
                (
                    method_data
                        .get("threeDSServerTransID")
                        .and_then(|value| value.as_str()),
                    method_data
                        .get("threeDSMethodNotificationURL")
                        .and_then(|value| value.as_str()),
                ),
                (Some("srv-1"), Some("https://hs.example/notify"))
            );
        }

        /// The decoded challenge token becomes the `creq` posted to `acsURL`.
        #[test]
        fn challenge_action_token_decodes_into_the_creq_form() {
            let out = pre_authenticate(
                &action_body("ChallengeShopper", "challenge", &challenge_token()),
                None,
            );
            let (endpoint, fields) = out
                .response
                .as_ref()
                .ok()
                .and_then(redirect_form)
                .unwrap_or_default();
            assert_eq!(endpoint, "https://acs.example/challenge");
            assert_eq!(fields.len(), 1);
            let creq = decode_b64url(&form_field(&fields, "creq"));
            assert_eq!(
                (
                    creq.get("threeDSServerTransID")
                        .and_then(|value| value.as_str()),
                    creq.get("acsTransID").and_then(|value| value.as_str()),
                    creq.get("messageVersion").and_then(|value| value.as_str()),
                    creq.get("messageType").and_then(|value| value.as_str()),
                ),
                (Some("srv-1"), Some("acs-1"), Some("2.2.0"), Some("CReq"))
            );
        }

        /// A token that is not base64url JSON, and one that decodes to the wrong shape, are both
        /// errors rather than panics. The transform itself still returns `Ok`: the failure
        /// surfaces as `response: Err(..)` with an `AuthenticationFailed` status.
        #[test]
        fn malformed_action_token_is_an_error_not_a_panic() {
            let not_base64 = pre_authenticate(
                &action_body("IdentifyShopper", "fingerprint", "!!!not base64!!!"),
                None,
            );
            assert_eq!(
                (
                    not_base64.resource_common_data.status,
                    not_base64.response.is_err()
                ),
                (AttemptStatus::AuthenticationFailed, true)
            );
            // `e30` is base64url for `{}`: valid base64, wrong JSON shape.
            let wrong_shape =
                pre_authenticate(&action_body("ChallengeShopper", "challenge", "e30"), None);
            assert_eq!(
                (
                    wrong_shape.resource_common_data.status,
                    wrong_shape.response.is_err()
                ),
                (AttemptStatus::AuthenticationFailed, true)
            );
        }

        // ===== (c) outcome table, per leg =====

        /// `IdentifyShopper` is the fingerprint step, and only the first leg can ask for one.
        /// The state the next leg needs is left in `connector_feature_data`, stamped with the
        /// step the shopper was handed.
        #[test]
        fn identify_shopper_hands_back_the_fingerprint_step_on_the_pre_authenticate_leg() {
            let out = pre_authenticate(
                &action_body("IdentifyShopper", "fingerprint", &fingerprint_token()),
                None,
            );
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::AuthenticationPending, false, true)
            );
            let state = out
                .resource_common_data
                .connector_feature_data
                .as_ref()
                .map(|carrier| {
                    use hyperswitch_masking::PeekInterface;
                    carrier.peek().clone()
                })
                .unwrap_or(serde_json::Value::Null);
            assert_eq!(
                (
                    state.pointer("/three_ds_data/paymentData"),
                    state.pointer("/three_ds_data/authorisationToken"),
                    state.pointer("/three_ds_data/stage"),
                ),
                (
                    Some(&serde_json::json!("Ab02b4c0!payment-data")),
                    Some(&serde_json::json!("Ab02b4c0!auth-token")),
                    Some(&serde_json::json!("fingerprint")),
                )
            );
        }

        /// A fingerprint that arrives after the fingerprint leg has nowhere to run: fail closed
        /// rather than drop the action and leave the payment pending with no next step.
        #[test]
        fn identify_shopper_is_refused_on_the_later_legs() {
            let body = action_body("IdentifyShopper", "fingerprint", &fingerprint_token());
            let on_authenticate = authenticate(&body);
            assert_eq!(
                outcome(
                    on_authenticate.resource_common_data.status,
                    &on_authenticate.response
                ),
                (AttemptStatus::AuthenticationFailed, false, false)
            );
            let on_post_authenticate = post_authenticate(&body);
            assert_eq!(
                on_post_authenticate.resource_common_data.status,
                AttemptStatus::AuthenticationFailed
            );
        }

        /// `ChallengeShopper` is the ACS challenge: legs 1 and 2 surface it, leg 3 cannot —
        /// it is the leg that carries the challenge result back.
        #[test]
        fn challenge_shopper_is_surfaced_by_the_first_two_legs_only() {
            let body = action_body("ChallengeShopper", "challenge", &challenge_token());
            let on_pre = pre_authenticate(&body, None);
            assert_eq!(
                outcome(on_pre.resource_common_data.status, &on_pre.response),
                (AttemptStatus::AuthenticationPending, false, true)
            );
            let on_auth = authenticate(&body);
            assert_eq!(
                outcome(on_auth.resource_common_data.status, &on_auth.response),
                (AttemptStatus::AuthenticationPending, false, true)
            );
            let on_post = post_authenticate(&body);
            assert_eq!(
                outcome(on_post.resource_common_data.status, &on_post.response),
                (AttemptStatus::AuthenticationFailed, false, false)
            );
        }

        /// A `threeDS2Result` whose ECI corroborates its `transStatus` becomes the
        /// `AuthenticationData` that Authorize sends on as `mpiData`. Asserted on every leg,
        /// PostAuthenticate included.
        #[test]
        fn authentication_finished_with_a_corroborated_result_becomes_authentication_data() {
            let body = authentication_finished_body("Y", "05");
            for (status, response) in [
                {
                    let out = pre_authenticate(&body, None);
                    (out.resource_common_data.status, out.response)
                },
                {
                    let out = authenticate(&body);
                    (out.resource_common_data.status, out.response)
                },
                {
                    let out = post_authenticate(&body);
                    (out.resource_common_data.status, out.response)
                },
            ] {
                assert_eq!(
                    outcome(status, &response),
                    (AttemptStatus::AuthenticationSuccessful, true, false)
                );
                let mapped = response
                    .as_ref()
                    .ok()
                    .and_then(authentication_data)
                    .map(|data| {
                        use hyperswitch_masking::PeekInterface;
                        (
                            data.trans_status.clone(),
                            data.eci.clone(),
                            data.cavv.as_ref().map(|cavv| cavv.peek().clone()),
                            data.ds_trans_id.clone(),
                            data.threeds_server_transaction_id.clone(),
                            data.message_version.is_some(),
                        )
                    });
                assert_eq!(
                    mapped,
                    Some((
                        Some(common_enums::TransactionStatus::Success),
                        Some("05".to_string()),
                        Some("cavv-value".to_string()),
                        Some("ds-1".to_string()),
                        Some("srv-1".to_string()),
                        true,
                    ))
                );
            }
        }

        /// A `transStatus` its ECI does not corroborate is refused, not passed on as a
        /// successful authentication: `Y` with a "not authenticated" ECI, `N` with a success
        /// ECI, and an unlisted `transStatus` all fail closed.
        #[test]
        fn authentication_finished_without_corroboration_is_refused() {
            for (trans_status, eci) in [("Y", "07"), ("N", "05"), ("Z", "05")] {
                let out = post_authenticate(&authentication_finished_body(trans_status, eci));
                assert_eq!(
                    outcome(out.resource_common_data.status, &out.response),
                    (AttemptStatus::AuthenticationFailed, false, false)
                );
                assert_eq!(
                    refusal(&out.response).and_then(|(_, _, reason)| reason),
                    Some("threeDS2Result transStatus is not corroborated by its ECI".to_string())
                );
            }
            // A missing `threeDS2Result` is the same refusal.
            let no_result = post_authenticate(
                r#"{"resultCode":"AuthenticationFinished","pspReference":"psp-1"}"#,
            );
            assert_eq!(
                no_result.resource_common_data.status,
                AttemptStatus::AuthenticationFailed
            );
        }

        /// `Refused` / `Error` / `Cancelled` report adyen's own refusal reason.
        #[test]
        fn refused_error_and_cancelled_are_authentication_failures() {
            for result_code in ["Refused", "Error", "Cancelled"] {
                let out = post_authenticate(&result_code_body(result_code));
                assert_eq!(
                    outcome(out.resource_common_data.status, &out.response),
                    (AttemptStatus::AuthenticationFailed, false, false)
                );
                assert_eq!(
                    refusal(&out.response),
                    Some((
                        "2".to_string(),
                        "Refused".to_string(),
                        Some("Refused".to_string()),
                    ))
                );
            }
        }

        /// `AuthenticationNotRequired` is not a failure: there was nothing to authenticate, so
        /// Authorize proceeds without `mpiData` and the leg succeeds with neither a form nor
        /// authentication data.
        #[test]
        fn authentication_not_required_is_pending_without_authentication_data() {
            let out = pre_authenticate(&result_code_body("AuthenticationNotRequired"), None);
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::Pending, false, false)
            );
        }

        /// `Received` / `Pending` mean adyen has not finished authenticating. Continuing would
        /// authorise with no `mpiData` - no liability shift, and adyen free to start a 3DS flow
        /// of its own on the money call and challenge the shopper a second time. Only a finished
        /// result, or an explicit `AuthenticationNotRequired`, may continue.
        #[test]
        fn an_unfinished_authentication_does_not_authorise() {
            for result_code in ["Received", "Pending"] {
                let out = pre_authenticate(&result_code_body(result_code), None);
                assert_eq!(
                    outcome(out.resource_common_data.status, &out.response),
                    (AttemptStatus::AuthenticationFailed, false, false),
                    "{result_code} must not let the money call run"
                );
            }
        }

        /// The `serde(other)` `Unknown` arm: a `resultCode` this integration has never seen
        /// fails closed. Grouping it with `AuthenticationNotRequired` would authorise the
        /// payment with no `mpiData` on the strength of a response nobody has read.
        #[test]
        fn an_unrecognised_result_code_fails_closed() {
            let out = post_authenticate(
                r#"{"resultCode":"SomethingAdyenInventedLastTuesday","pspReference":"psp-1"}"#,
            );
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::AuthenticationFailed, false, false)
            );
            assert_eq!(
                refusal(&out.response).and_then(|(_, _, reason)| reason),
                Some("adyen returned a resultCode this integration does not recognise".to_string())
            );
        }

        /// `nativeThreeDS` was asked for, so the redirect 3DS2 variant is not something these
        /// legs can run.
        #[test]
        fn redirect_shopper_is_refused_on_the_native_legs() {
            let out = pre_authenticate(&result_code_body("RedirectShopper"), None);
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::AuthenticationFailed, false, false)
            );
        }

        /// Adyen answers the same standalone-authentication call with a `redirect` action when
        /// native 3DS2 was not requested, or the account cannot do it. That body carries no
        /// `subtype` and no `token`: while the field was typed as `threeDS2` alone the whole
        /// response failed to deserialize, so the leg reported "response body did not match the
        /// expected format" instead of the reason, and the `RedirectShopper` arm above could
        /// never be reached. It must parse, and the leg must say what happened.
        #[test]
        fn a_redirect_action_parses_rather_than_failing_to_deserialize() {
            let redirect_action = r#""action":{"type":"redirect","url":"https://test.adyen.com/hpp/3d/validate.shtml","method":"POST","paymentMethodType":"scheme","data":{"PaReq":"pa","MD":"md","TermUrl":"term"}}"#;

            // The realistic shape: adyen picked the redirect variant and says so.
            let body = format!(
                r#"{{"resultCode":"RedirectShopper","pspReference":"psp-1",{redirect_action}}}"#
            );
            assert!(
                serde_json::from_str::<AdyenPreAuthenticationResponse>(&body).is_ok(),
                "a redirect action must still deserialize"
            );
            let out = pre_authenticate(&body, None);
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::AuthenticationFailed, false, false)
            );

            // A shopper-action resultCode carrying a non-native action: the leg names the
            // action type rather than trying to decode a token that is not there.
            let mismatched = format!(
                r#"{{"resultCode":"IdentifyShopper","pspReference":"psp-1",{redirect_action}}}"#
            );
            let out = pre_authenticate(&mismatched, None);
            assert_eq!(
                outcome(out.resource_common_data.status, &out.response),
                (AttemptStatus::AuthenticationFailed, false, false)
            );
            let reason = refusal(&out.response).and_then(|(_, _, reason)| reason);
            assert!(
                reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("Redirect")),
                "the reason should name the action adyen sent, got {reason:?}"
            );
        }

        /// Money moved on an `authenticationOnly` call: reported as an error, but with the
        /// honest money status so the composite loop does not authorise the payment twice.
        #[test]
        fn authorised_on_an_authentication_only_leg_reports_the_money_status() {
            let auto_capture = pre_authenticate(&result_code_body("Authorised"), None);
            assert_eq!(
                (
                    auto_capture.resource_common_data.status,
                    auto_capture.response.is_err()
                ),
                (AttemptStatus::Charged, true)
            );
            let manual_capture =
                pre_authenticate(&result_code_body("Authorised"), Some(CaptureMethod::Manual));
            assert_eq!(
                (
                    manual_capture.resource_common_data.status,
                    manual_capture.response.is_err()
                ),
                (AttemptStatus::Authorized, true)
            );
        }
    }

    /// Level 2/3 data on the wire: the legacy flat `additionalData.enhancedSchemeData.*` keys,
    /// including one `itemDetailLine{n}` group per order line numbered from 1.
    pub mod level_two_three {
        use std::{marker::PhantomData, str::FromStr};

        use common_utils::{request::RequestContent, types::MinorUnit};
        use domain_types::{
            connector_flow::Authorize,
            connector_types::{
                ConnectorEnum, L2L3Data, OrderInfo, PaymentFlowData, PaymentsAuthorizeData,
                PaymentsResponseData, TaxInfo,
            },
            payment_address::OrderDetailsWithAmount,
            payment_method_data::{DefaultPCIHolder, PaymentMethodData, RawCardNumber},
            router_data::ErrorResponse,
            router_data_v2::RouterDataV2,
            router_request_types::AuthenticationData,
        };
        use hyperswitch_masking::Secret;
        use interfaces::{
            connector_integration_v2::BoxedConnectorIntegrationV2, connector_types::BoxedConnector,
        };

        use super::fixtures::{adyen_config, payment_flow_data};
        use crate::{
            connectors::{adyen::transformers::AdditionalData, Adyen},
            types::ConnectorData,
        };

        fn money(minor_units: i64) -> common_utils::types::Money {
            common_utils::types::Money {
                amount: MinorUnit::new(minor_units),
                currency: common_enums::Currency::USD,
            }
        }

        fn order_line(
            product_name: &str,
            product_id: &str,
            quantity: u16,
            unit_price: i64,
        ) -> OrderDetailsWithAmount {
            OrderDetailsWithAmount {
                product_name: product_name.to_string(),
                product_id: Some(product_id.to_string()),
                quantity,
                amount: MinorUnit::new(unit_price),
                unit_of_measure: Some("EA".to_string()),
                ..Default::default()
            }
        }

        /// Two order lines, a shipping cost and an order tax amount — the fields that become
        /// `itemDetailLine1` / `itemDetailLine2`, `freightAmount` and `totalTaxAmount`.
        fn level_two_three_data() -> L2L3Data {
            L2L3Data {
                order_info: Some(OrderInfo {
                    order_date: None,
                    order_details: Some(vec![
                        order_line("widget", "SKU-1", 2, 1000),
                        order_line("gadget", "SKU-2", 3, 250),
                    ]),
                    merchant_order_reference_id: Some("order-7".to_string()),
                    discount_amount: None,
                    shipping_cost: Some(money(500)),
                    duty_amount: None,
                    order_commodity_code: None,
                }),
                tax_info: Some(TaxInfo {
                    tax_status: None,
                    customer_tax_registration_id: None,
                    merchant_tax_registration_id: None,
                    shipping_amount_tax: None,
                    order_tax_amount: Some(money(250)),
                    merchant_vat_registration_id: None,
                    value_added_tax_amount: None,
                    value_added_tax_percent: None,
                    vat_invoice_reference_number: None,
                }),
                customer_info: None,
                shipping_details: None,
                billing_details: None,
            }
        }

        /// The Authorize request body the connector would send, as JSON.
        fn authorize_request(
            l2_l3_data: Option<Box<L2L3Data>>,
            authentication_data: Option<AuthenticationData>,
        ) -> common_utils::errors::CustomResult<
            Option<common_utils::request::Request>,
            domain_types::errors::IntegrationError,
        > {
            let req: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    l2_l3_data,
                    ..payment_flow_data()
                },
                connector_config: adyen_config(),
                request: PaymentsAuthorizeData {
                    risk_assessment_data: None,
                    split_settlement: None,
                    customer_document_details: None,
                    customer_date_of_birth: None,
                    payment_channel: None,
                    authentication_data,
                    connector_testing_data: None,
                    currency_conversion_data: None,
                    payment_method_data: PaymentMethodData::Card(
                        domain_types::payment_method_data::Card {
                            card_number: RawCardNumber(
                                cards::CardNumber::from_str("5123456789012346").unwrap(),
                            ),
                            card_cvc: Secret::new("100".into()),
                            card_exp_month: Secret::new("03".into()),
                            card_exp_year: Secret::new("2030".into()),
                            ..Default::default()
                        },
                    ),
                    amount: money(3250),
                    order_tax_amount: None,
                    surcharge_amount: None,
                    email: None,
                    customer_name: None,
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    capture_method: None,
                    integrity_object: None,
                    router_return_url: Some("https://hs.example/return".to_string()),
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: Some(domain_types::router_request_types::BrowserInformation {
                        color_depth: Some(24),
                        java_enabled: Some(false),
                        screen_height: Some(1080),
                        screen_width: Some(1920),
                        user_agent: Some("Mozilla/5.0".to_string()),
                        accept_header: Some("text/html".to_string()),
                        java_script_enabled: Some(false),
                        language: Some("en-US".to_string()),
                        time_zone: Some(-480),
                        referer: None,
                        ip_address: None,
                        os_type: None,
                        os_version: None,
                        device_model: None,
                        accept_language: None,
                    }),
                    order_category: None,
                    session_token: None,
                    enrolled_for_3ds: Some(true),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(common_enums::PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
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
                    mit_category: None,
                    domain_data: None,
                    partner_merchant_identifier_details: None,
                    is_account_funding_transaction: None,
                    recipient_details: None,
                    business_country: None,
                    enable_avs_check: None,
                    additional_connector_details: None,
                    customer: None,
                    is_stored_credential: None,
                    integrity_specifications: None,
                },
                response: Err(ErrorResponse::default()),
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

            connector_integration.build_request_v2(&req)
        }

        fn authorize_body(l2_l3_data: Option<Box<L2L3Data>>) -> serde_json::Value {
            authorize_request(l2_l3_data, None)
                .unwrap()
                .and_then(|request| request.body)
                .and_then(|content| match content {
                    RequestContent::Json(body)
                    | RequestContent::FormUrlEncoded(body)
                    | RequestContent::Xml(body) => body.masked_serialize().ok(),
                    _ => None,
                })
                .unwrap_or(serde_json::Value::Null)
        }

        /// Every L2/L3 key reaches adyen as a flat dotted `additionalData` key with a string
        /// value, one `itemDetailLine{n}` group per order line numbered from 1, and
        /// `totalAmount` = quantity x unitPrice (less any line discount, of which there is none
        /// here).
        #[test]
        fn level_two_three_data_serialises_to_flat_dotted_item_detail_line_keys() {
            let body = authorize_body(Some(Box::new(level_two_three_data())));
            let additional_data = body
                .get("additionalData")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let key = |name: &str| additional_data.get(name).cloned();
            assert_eq!(
                (
                    key("enhancedSchemeData.customerReference"),
                    key("enhancedSchemeData.totalTaxAmount"),
                    key("enhancedSchemeData.freightAmount"),
                ),
                (
                    Some(serde_json::json!("order-7")),
                    Some(serde_json::json!("250")),
                    Some(serde_json::json!("500")),
                )
            );
            // Line numbering starts at 1, and the amounts and the quantity are strings: every
            // `additionalData` value is a string on the wire.
            assert_eq!(
                (
                    key("enhancedSchemeData.itemDetailLine1.description"),
                    key("enhancedSchemeData.itemDetailLine1.productCode"),
                    key("enhancedSchemeData.itemDetailLine1.quantity"),
                    key("enhancedSchemeData.itemDetailLine1.unitPrice"),
                    key("enhancedSchemeData.itemDetailLine1.totalAmount"),
                    key("enhancedSchemeData.itemDetailLine1.unitOfMeasure"),
                ),
                (
                    Some(serde_json::json!("widget")),
                    Some(serde_json::json!("SKU-1")),
                    Some(serde_json::json!("2")),
                    Some(serde_json::json!("1000")),
                    Some(serde_json::json!("2000")),
                    Some(serde_json::json!("EA")),
                )
            );
            assert_eq!(
                (
                    key("enhancedSchemeData.itemDetailLine2.description"),
                    key("enhancedSchemeData.itemDetailLine2.productCode"),
                    key("enhancedSchemeData.itemDetailLine2.quantity"),
                    key("enhancedSchemeData.itemDetailLine2.unitPrice"),
                    key("enhancedSchemeData.itemDetailLine2.totalAmount"),
                ),
                (
                    Some(serde_json::json!("gadget")),
                    Some(serde_json::json!("SKU-2")),
                    Some(serde_json::json!("3")),
                    Some(serde_json::json!("250")),
                    Some(serde_json::json!("750")),
                )
            );
            // There is no third line, and an absent optional key is omitted, never null.
            assert_eq!(key("enhancedSchemeData.itemDetailLine3.description"), None);
            assert_eq!(
                key("enhancedSchemeData.itemDetailLine1.commodityCode"),
                None
            );
            assert_eq!(key("enhancedSchemeData.dutyAmount"), None);

            // No L2/L3 data on the request, no `enhancedSchemeData.*` keys at all.
            let without = authorize_body(None);
            assert_eq!(
                without
                    .get("additionalData")
                    .and_then(|data| data.as_object())
                    .map(|data| data
                        .keys()
                        .any(|name| name.starts_with("enhancedSchemeData."))),
                Some(false)
            );
        }

        /// The keys are request-only (`#[serde(flatten, skip_deserializing)]`): a response that
        /// echoes one must not populate the field, and must not have it absorbed by the
        /// flattened line map either.
        #[test]
        fn a_response_never_populates_the_request_only_keys() {
            let parsed = serde_json::from_str::<AdditionalData>(
                r#"{"cardSummary":"0000","expiryDate":"03/2030","authCode":"012345","avsResult":"7 Both postal code and address match","recurring.recurringDetailReference":"tok_1","enhancedSchemeData.customerReference":"echoed-by-nobody","enhancedSchemeData.itemDetailLine1.description":"echoed-by-nobody"}"#,
            )
            .unwrap_or_default();
            assert_eq!(parsed.auth_code, Some("012345".to_string()));
            // Round-tripping is the public view of the skipped field: nothing was captured, so
            // nothing comes back out.
            let round_tripped = serde_json::to_value(&parsed).unwrap_or_default();
            assert_eq!(
                round_tripped.as_object().map(|data| data
                    .keys()
                    .any(|name| name.starts_with("enhancedSchemeData."))),
                Some(false)
            );
        }

        /// Adyen judges an external 3DS result, not this connector: an ECI outside the usual table
        /// (no leading zero) still goes out as `mpiData`.
        #[test]
        fn an_external_three_ds_result_is_passed_through_as_mpi_data() {
            let request = authorize_request(
                None,
                Some(AuthenticationData {
                    trans_status: Some(common_enums::TransactionStatus::NotVerified),
                    eci: Some("2".to_string()),
                    cavv: Some(Secret::new("cavv".to_string())),
                    ucaf_collection_indicator: None,
                    threeds_server_transaction_id: None,
                    message_version: None,
                    ds_trans_id: None,
                    acs_transaction_id: None,
                    transaction_id: None,
                    network_params: None,
                    exemption_indicator: None,
                    created_at: None,
                    challenge_code: None,
                    challenge_cancel: None,
                    challenge_code_reason: None,
                    message_extension: None,
                    authentication_type: None,
                }),
            );
            let body = request
                .unwrap()
                .and_then(|request| request.body)
                .and_then(|content| match content {
                    RequestContent::Json(body) => body.masked_serialize().ok(),
                    _ => None,
                })
                .unwrap();
            assert_eq!(body["mpiData"]["eci"], "2");
        }

        /// An amount in another currency would reach Adyen as a bare number and change value, so
        /// it is refused for each of the three `Money` fields rather than converted.
        #[test]
        fn an_amount_in_another_currency_is_refused() {
            let euros = common_utils::types::Money {
                amount: MinorUnit::new(500),
                currency: common_enums::Currency::EUR,
            };
            let mut shipping = level_two_three_data();
            if let Some(order_info) = shipping.order_info.as_mut() {
                order_info.shipping_cost = Some(euros.clone());
            }
            let mut duty = level_two_three_data();
            if let Some(order_info) = duty.order_info.as_mut() {
                order_info.duty_amount = Some(euros.clone());
            }
            let mut tax = level_two_three_data();
            if let Some(tax_info) = tax.tax_info.as_mut() {
                tax_info.order_tax_amount = Some(euros);
            }
            for (case, data) in [("shipping", shipping), ("duty", duty), ("tax", tax)] {
                assert!(
                    authorize_request(Some(Box::new(data)), None).is_err(),
                    "{case} in EUR on a USD payment must be refused"
                );
            }
        }
    }

    /// Pure rules the flows lean on: what an Adyen error means for the attempt, how an
    /// `Idempotency-Key` is derived, and which webhook keys and 3DS outcomes are accepted.
    pub mod rules {
        use std::collections::HashMap;

        use common_enums::{AttemptStatus, RefundStatus, TransactionStatus};
        use domain_types::{
            connector_types::{ConnectorWebhookSecrets, RequestDetails},
            payment_method_data::DefaultPCIHolder,
            router_data::FlowStatus,
        };
        use hyperswitch_masking::{PeekInterface, Secret};
        use interfaces::connector_types::IncomingWebhook;

        use crate::connectors::{
            adyen::{
                adyen_authorize_error_attempt_status, adyen_error_attempt_status,
                adyen_idempotency_key,
                transformers::{
                    get_adyen_redirect_return, is_three_ds_outcome_corroborated, AdyenErrorType,
                    AdyenMdPaResFieldNames, AdyenRedirectReturn, AdyenRefundRequestReason,
                },
                AdyenErrorFlow,
            },
            Adyen,
        };

        const PENDING: FlowStatus = FlowStatus::Payment(AttemptStatus::Pending);
        const FAILURE: FlowStatus = FlowStatus::Payment(AttemptStatus::Failure);

        #[test]
        fn authorize_error_status_follows_http_status_and_body_presence() {
            for (status_code, error_code, expected) in [
                // Transient codes stay pending whatever the HTTP status says.
                (422, Some("703"), PENDING),
                (422, Some("704"), PENDING),
                (422, Some("705"), PENDING),
                // Rate limited, timed out or conflicting: the original may still be in flight.
                (408, None, PENDING),
                (409, Some("100"), PENDING),
                (429, None, PENDING),
                // Adyen does not store a request that fails validation.
                (400, None, FAILURE),
                (422, Some("100"), FAILURE),
                // A 500 with a ServiceError body was rejected; without one nothing is known.
                (500, Some("701"), FAILURE),
                (500, None, PENDING),
                (502, Some("701"), PENDING),
                (503, None, PENDING),
            ] {
                assert_eq!(
                    adyen_authorize_error_attempt_status(status_code, error_code),
                    expected,
                    "HTTP {status_code}, code {error_code:?}"
                );
            }
        }

        #[test]
        fn already_processing_stays_pending_only_on_money_moving_flows() {
            let status = |flow| adyen_error_attempt_status(flow, 422, Some("704"), None);
            assert_eq!(status(AdyenErrorFlow::Payment), Some(PENDING));
            assert_eq!(status(AdyenErrorFlow::Capture), Some(PENDING));
            assert_eq!(status(AdyenErrorFlow::Void), Some(PENDING));
            assert_eq!(
                status(AdyenErrorFlow::Refund),
                Some(FlowStatus::Refund(RefundStatus::Pending))
            );
            assert_eq!(status(AdyenErrorFlow::Sync), None);
            assert_eq!(status(AdyenErrorFlow::Authentication), None);
            assert_eq!(status(AdyenErrorFlow::Unclassified), None);
        }

        #[test]
        fn transient_errors_outside_authorize_are_not_failures() {
            let flow = AdyenErrorFlow::Capture;
            for (status_code, error_code, error_type) in [
                (429, None, None),
                (500, Some("100"), Some(&AdyenErrorType::Validation)),
                (503, None, None),
                (400, Some("703"), None),
                (400, Some("705"), None),
                (400, None, Some(&AdyenErrorType::Internal)),
            ] {
                assert_eq!(
                    adyen_error_attempt_status(flow, status_code, error_code, error_type),
                    Some(PENDING),
                    "HTTP {status_code}, code {error_code:?}, type {error_type:?}"
                );
            }
        }

        #[test]
        fn an_explicit_rejection_never_fails_the_payment_from_sync() {
            let rejected = |flow| {
                adyen_error_attempt_status(flow, 422, None, Some(&AdyenErrorType::Validation))
            };
            assert_ne!(rejected(AdyenErrorFlow::Sync), Some(FAILURE));
            // An unrecognised errorType on a 4xx is not a rejection Adyen has told us about.
            assert_eq!(
                adyen_error_attempt_status(
                    AdyenErrorFlow::Unclassified,
                    422,
                    None,
                    Some(&AdyenErrorType::Unknown),
                ),
                None
            );
        }

        // The expected keys are the SHA-256 of `operation:identity`, computed outside this code.
        const KEY_PAYMENTS_REQUEST: &str =
            "b2033ffdb360109d9ad2645fc6ff627a81d9a45c6172d4942ee4f75a58a8bacc";
        const KEY_PAYMENTS_ATTEMPT: &str =
            "69b79edd530e2a5c20884858e49fbf756572b1c25193b92fae8afbe4cf0146b2";
        const KEY_CAPTURES_REQUEST: &str =
            "a2daeee13c8e8009e5adfba1e18b3dc54433f3cea96758f94931ebd89b60a33d";

        fn key(
            operation: &str,
            merchant_request_id: Option<&str>,
            reference: &str,
        ) -> Option<String> {
            adyen_idempotency_key(operation, merchant_request_id, reference).unwrap()
        }

        #[test]
        fn merchant_request_id_wins_over_the_reference() {
            let derived = key("payments", Some("req-1"), "attempt-1").unwrap();
            assert_eq!(derived, KEY_PAYMENTS_REQUEST);
            assert_eq!(derived.len(), 64, "Adyen's maximum key length");
        }

        /// Two attempts on one payment share the reference, so only their own request ids keep
        /// Adyen from replaying the first attempt's response for the second.
        #[test]
        fn two_attempts_sharing_a_reference_get_distinct_keys_from_their_request_ids() {
            assert_ne!(
                key("payments", Some("req-1"), "pay_1"),
                key("payments", Some("req-2"), "pay_1")
            );
        }

        #[test]
        fn a_missing_or_blank_request_id_falls_back_to_the_reference() {
            for merchant_request_id in [None, Some(""), Some("   ")] {
                assert_eq!(
                    key("payments", merchant_request_id, "attempt-1").as_deref(),
                    Some(KEY_PAYMENTS_ATTEMPT),
                    "request id {merchant_request_id:?}"
                );
            }
        }

        #[test]
        fn the_operation_keeps_two_calls_on_one_identity_apart() {
            assert_eq!(
                key("captures", Some("req-1"), "attempt-1").as_deref(),
                Some(KEY_CAPTURES_REQUEST)
            );
        }

        #[test]
        fn no_key_is_sent_without_a_stable_identity() {
            assert_eq!(key("refunds", None, ""), None);
            assert_eq!(key("refunds", Some(" "), "  "), None);
        }

        // `SIGNATURE_*` are the Base64 HMAC-SHA256 of `WEBHOOK_MESSAGE` under each hex key,
        // computed outside this code. The message is Adyen's documented eight-field preimage.
        const WEBHOOK_MESSAGE_FIELDS: (&str, &str, &str) = ("psp_1", "Acct", "order_1");
        const CURRENT_KEY: &str = "00112233445566778899aabbccddeeff";
        const PREVIOUS_KEY: &str = "ffeeddccbbaa99887766554433221100";
        const SIGNATURE_CURRENT: &str = "MkBGZ8cc0uEYsTr2RBMIxZNZMN9gxy7H4H+PnSra63g=";
        const SIGNATURE_PREVIOUS: &str = "xzfIFR9m2zGvu0Au6jX+zPEk3LCnf/ifDRp+7NSGv98=";

        fn signed_webhook(signature: &str) -> RequestDetails {
            let (psp_reference, merchant_account, merchant_reference) = WEBHOOK_MESSAGE_FIELDS;
            let body = serde_json::json!({
                "notificationItems": [{
                    "NotificationRequestItem": {
                        "pspReference": psp_reference,
                        "merchantAccountCode": merchant_account,
                        "merchantReference": merchant_reference,
                        "eventCode": "AUTHORISATION",
                        "success": "true",
                        "amount": { "value": 1000, "currency": "USD" },
                        "additionalData": { "hmacSignature": signature },
                    }
                }]
            });
            RequestDetails {
                method: domain_types::connector_types::HttpMethod::Post,
                uri: None,
                headers: HashMap::new(),
                body: body.to_string().into_bytes(),
                query_params: None,
            }
        }

        fn verify(
            signature: &str,
            secret: &str,
            additional_secret: Option<&str>,
        ) -> Result<bool, error_stack::Report<domain_types::errors::WebhookError>> {
            let connector = Adyen::<DefaultPCIHolder>::new();
            IncomingWebhook::verify_webhook_source(
                connector,
                signed_webhook(signature),
                Some(ConnectorWebhookSecrets {
                    secret: secret.as_bytes().to_vec(),
                    additional_secret: additional_secret.map(|key| Secret::new(key.to_string())),
                }),
                None,
            )
        }

        #[test]
        fn a_webhook_signed_with_the_current_key_verifies() {
            assert!(verify(SIGNATURE_CURRENT, CURRENT_KEY, None).unwrap());
        }

        #[test]
        fn the_previous_key_verifies_only_while_it_is_configured_for_rotation() {
            assert!(verify(SIGNATURE_PREVIOUS, CURRENT_KEY, Some(PREVIOUS_KEY)).unwrap());
            assert!(!verify(SIGNATURE_PREVIOUS, CURRENT_KEY, None).unwrap());
        }

        #[test]
        fn a_non_hex_additional_secret_is_not_treated_as_a_key() {
            assert!(!verify(SIGNATURE_PREVIOUS, CURRENT_KEY, Some("not-hex")).unwrap());
            assert!(verify(SIGNATURE_CURRENT, CURRENT_KEY, Some("not-hex")).unwrap());
        }

        #[test]
        fn an_empty_or_non_hex_primary_key_is_an_error() {
            assert!(verify(SIGNATURE_CURRENT, "", None).is_err());
            assert!(verify(SIGNATURE_CURRENT, "not-hex", None).is_err());
        }

        #[test]
        fn three_ds_outcome_needs_an_eci_that_corroborates_the_status() {
            for (status, eci, expected) in [
                (TransactionStatus::Success, "05", true),
                (TransactionStatus::Success, "02", true),
                (TransactionStatus::Success, "07", false),
                (TransactionStatus::NotVerified, "06", true),
                (TransactionStatus::NotVerified, "01", true),
                (TransactionStatus::NotVerified, "05", false),
                (TransactionStatus::InformationOnly, "07", true),
                (TransactionStatus::InformationOnly, "01", false),
                (TransactionStatus::Failure, "05", false),
                (TransactionStatus::Rejected, "02", false),
                (TransactionStatus::ChallengeRequired, "05", false),
            ] {
                assert_eq!(
                    is_three_ds_outcome_corroborated(Some(&status), Some(eci)),
                    expected,
                    "status {status:?}, eci {eci}"
                );
            }
            assert!(is_three_ds_outcome_corroborated(
                Some(&TransactionStatus::Success),
                Some(" 05 ")
            ));
            assert!(!is_three_ds_outcome_corroborated(
                Some(&TransactionStatus::Success),
                None
            ));
            assert!(!is_three_ds_outcome_corroborated(None, Some("05")));
        }

        #[test]
        fn refund_reason_text_maps_onto_adyen_values_or_other() {
            for (reason, expected) in [
                ("fraud", AdyenRefundRequestReason::Fraud),
                (
                    "customer_requested",
                    AdyenRefundRequestReason::CustomerRequest,
                ),
                (
                    "CUSTOMER REQUEST",
                    AdyenRefundRequestReason::CustomerRequest,
                ),
                (" return ", AdyenRefundRequestReason::Return),
                ("Duplicate", AdyenRefundRequestReason::Duplicate),
                ("changed my mind", AdyenRefundRequestReason::Other),
                ("", AdyenRefundRequestReason::Other),
            ] {
                assert_eq!(
                    AdyenRefundRequestReason::from_reason(reason),
                    expected,
                    "reason {reason:?}"
                );
            }
        }

        const FIELD_NAMES: AdyenMdPaResFieldNames = AdyenMdPaResFieldNames {
            md: "encoded_data.MD",
            pa_res: "encoded_data.PaRes",
        };

        fn redirect_return(
            params: &[(&str, &str)],
            allow_payload: bool,
        ) -> Result<
            Option<AdyenRedirectReturn>,
            error_stack::Report<domain_types::errors::IntegrationError>,
        > {
            let params: HashMap<String, String> = params
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect();
            get_adyen_redirect_return(|key| params.get(key).cloned(), allow_payload, FIELD_NAMES)
        }

        #[test]
        fn redirect_return_picks_one_shape_in_precedence_order() {
            let all = [
                ("redirectResult", "rr"),
                ("threeDSResult", "tds"),
                ("payload", "pl"),
                ("MD", "md"),
                ("PaRes", "pares"),
            ];
            assert!(matches!(
                redirect_return(&all, true).unwrap(),
                Some(AdyenRedirectReturn::RedirectResult(value)) if value == "rr"
            ));
            assert!(matches!(
                redirect_return(&all[1..], true).unwrap(),
                Some(AdyenRedirectReturn::ThreeDsResult(value)) if value == "tds"
            ));
            assert!(matches!(
                redirect_return(&all[2..], true).unwrap(),
                Some(AdyenRedirectReturn::Payload(value)) if value == "pl"
            ));
            assert!(matches!(
                redirect_return(&all[2..], false).unwrap(),
                Some(AdyenRedirectReturn::MdPaRes { md, pa_res })
                    if md.peek() == "md" && pa_res.peek() == "pares"
            ));
        }

        #[test]
        fn redirect_return_treats_blank_as_absent_and_refuses_half_a_pair() {
            assert!(redirect_return(&[], true).unwrap().is_none());
            assert!(redirect_return(&[("redirectResult", "  ")], true)
                .unwrap()
                .is_none());
            assert!(redirect_return(&[("MD", "md")], true).is_err());
            assert!(redirect_return(&[("PaRes", "pares")], true).is_err());
        }
    }
}
