#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::print_stdout)]
mod tests {

    use cards::CardNumber;
    use common_enums::{AttemptStatus, AuthenticationType, PaymentMethod};
    use domain_types::{
        connector_types::{PaymentFlowData, PaymentsAuthorizeData},
        payment_address::{Address, PhoneDetails},
        payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData, RawCardNumber},
        router_request_types::BrowserInformation,
        router_response_types::Response,
    };
    use interfaces::{
        connector_integration_v2::ConnectorIntegrationV2,
        connector_types::{BoxedConnector, ConnectorServiceTrait},
    };
    use serde_json::{json, to_value};

    use crate::connectors::Razorpay;

    mod authorize {
        use std::str::FromStr;

        use cards::CardNumber;
        use common_enums::{
            AttemptStatus, AuthenticationType, Currency, PaymentMethod, PaymentMethodType,
        };
        use common_utils::{
            id_type::MerchantId, pii::Email, request::RequestContent, types::MinorUnit,
        };
        use domain_types::{
            connector_types::{PaymentFlowData, PaymentsAuthorizeData},
            payment_address::{Address, PaymentAddress, PhoneDetails},
            payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData, RawCardNumber},
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            router_request_types::BrowserInformation,
            router_response_types::Response,
            types::{ConnectorParams, Connectors},
        };
        use interfaces::{
            connector_integration_v2::ConnectorIntegrationV2,
            connector_types::{BoxedConnector, ConnectorServiceTrait},
        };
        use serde_json::{json, to_value, Value};

        use crate::connectors::Razorpay;

        #[test]
        fn test_build_request_valid() {
            let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();

            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                    attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                    status: AttemptStatus::Pending,
                    payment_method: PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: PaymentAddress::new(
                        None,
                        Some(Address {
                            address: None,
                            phone: Some(PhoneDetails {
                                number: Some("1234567890".to_string().into()),
                                country_code: Some("+1".to_string()),
                            }),
                            email: Some(email.clone()),
                        }),
                        None,
                        None,
                    ),
                    auth_type: AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_capturable: None,
                    amount: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("order_QMSVrXxHS9sBmu".to_string()),
                    connector_order_id: Some("order_QMSVrXxHS9sBmu".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_12345".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Card {
                        card_number: RawCardNumber(
                            CardNumber::from_str("5123456789012346").unwrap(),
                        ),
                        card_exp_month: "12".to_string().into(),
                        card_exp_year: "2026".to_string().into(),
                        card_cvc: "123".to_string().into(),
                        card_issuer: None,
                        card_network: None,
                        card_type: None,
                        card_issuing_country: None,
                        bank_code: None,
                        nick_name: None,
                        card_holder_name: Some("Test User".to_string().into()),
                        co_badged_card_data: None,
                    }),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: Some(email.clone()),
                    customer_name: None,
                    currency: Currency::USD,
                    confirm: true,
                    billing_descriptor: None,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    integrity_object: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: Some(BrowserInformation {
                        color_depth: None,
                        java_enabled: Some(false),
                        java_script_enabled: None,
                        language: Some("en-US".to_string()),
                        screen_height: Some(1080),
                        screen_width: Some(1920),
                        time_zone: None,
                        ip_address: None,
                        accept_header: Some(
                            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                                .to_string(),
                        ),
                        user_agent: Some(
                            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string(),
                        ),
                        referer: None,
                        os_type: None,
                        os_version: None,
                        device_model: None,
                        accept_language: None,
                    }),
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    order_category: None,
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
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                },
                response: Err(ErrorResponse {
                    code: "HE_00".to_string(),
                    message: "Something went wrong".to_string(),
                    reason: None,
                    status_code: 500,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data);
            let request_content = result.unwrap();

            let actual_json: Value = match request_content {
                Some(RequestContent::Json(payload)) => {
                    to_value(&payload).expect("Failed to serialize payload to JSON")
                }
                _ => panic!("Expected JSON payload"),
            };
            let expected_json: Value = json!({
                "amount": 1000,
                "currency": "USD",
                "contact": "1234567890",
                "email": "testuser@gmail.com",
                "order_id": "order_QMSVrXxHS9sBmu",
                "method": "card",
                "card": {
                    "number": "5123456789012346",
                    "expiry_month": "12",
                    "expiry_year": "2026",
                    "cvv": "123"
                },
                "authentication": {
                    "authentication_channel": "browser"
                },
                "browser": {
                    "java_enabled": false,
                    "language": "en-US",
                    "screen_height": 1080,
                    "screen_width": 1920
                },
                "ip": "127.0.0.1",
                "referer": "https://example.com",
                "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"
            });
            assert_eq!(actual_json, expected_json);
        }

        #[test]
        fn test_build_request_missing() {
            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "MISSING_EMAIL_ID".to_string(),
                    attempt_id: "MISSING_CARD_ID".to_string(),
                    status: AttemptStatus::Pending,
                    payment_method: PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::new(None, None, None, None),
                    auth_type: AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("order_missing".to_string()),
                    connector_order_id: Some("order_missing".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_missing".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    minor_amount_capturable: None,
                    amount: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Card {
                        card_number: RawCardNumber(CardNumber::from_str("").unwrap_or_default()),
                        card_exp_month: "".to_string().into(),
                        card_exp_year: "".to_string().into(),
                        card_cvc: "".to_string().into(),
                        card_issuer: None,
                        card_network: None,
                        card_type: None,
                        card_issuing_country: None,
                        bank_code: None,
                        nick_name: None,
                        card_holder_name: Some("Test User".to_string().into()),
                        co_badged_card_data: None,
                    }),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: None,
                    customer_name: None,
                    currency: Currency::USD,
                    confirm: true,
                    billing_descriptor: None,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    integrity_object: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: None,
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    order_category: None,
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
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                },
                response: Err(ErrorResponse {
                    code: "HE_01".to_string(),
                    message: "Missing required fields".to_string(),
                    reason: None,
                    status_code: 400,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data);

            assert!(
                result.is_err(),
                "Expected error for missing required fields, but got: {result:?}"
            );
        }

        #[test]
        fn test_build_request_invalid() {
            use common_utils::pii::Email;

            let email = Email::try_from("invalid-email@nowhere.com".to_string()).unwrap();

            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "INVALID_PAYMENT".to_string(),
                    attempt_id: "INVALID_ATTEMPT".to_string(),
                    status: AttemptStatus::Pending,
                    payment_method: PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::new(None, None, None, None),
                    auth_type: AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("invalid_id".to_string()),
                    connector_order_id: Some("invalid_id".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_invalid".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    minor_amount_capturable: None,
                    amount: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Card {
                        card_number: RawCardNumber(CardNumber::from_str("123").unwrap_or_default()),
                        card_exp_month: "99".to_string().into(),
                        card_exp_year: "1999".to_string().into(),
                        card_cvc: "1".to_string().into(),
                        card_issuer: None,
                        card_network: None,
                        card_type: None,
                        card_issuing_country: None,
                        bank_code: None,
                        nick_name: None,
                        card_holder_name: Some("Test User".to_string().into()),
                        co_badged_card_data: None,
                    }),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: Some(email),
                    customer_name: None,
                    currency: Currency::USD,
                    confirm: true,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    integrity_object: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: None,
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    order_category: None,
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
                },
                response: Err(ErrorResponse {
                    code: "HE_02".to_string(),
                    message: "Invalid format".to_string(),
                    reason: None,
                    status_code: 422,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data);

            assert!(
                result.is_err(),
                "Expected error for invalid field values, but got: {result:?}"
            );
        }

        #[test]
        fn test_handle_response_v2_valid_authorize_response() {
            use std::str::FromStr;

            use common_enums::Currency;
            use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
            use domain_types::{
                connector_types::PaymentFlowData,
                payment_address::PaymentAddress,
                router_data::{ConnectorSpecificConfig, ErrorResponse},
                router_data_v2::RouterDataV2,
                types::{ConnectorParams, Connectors},
            };
            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();

            let data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                    attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                    status: AttemptStatus::Pending,
                    payment_method: PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::new(
                        None,
                        Some(Address {
                            address: None,
                            phone: Some(PhoneDetails {
                                number: Some("1234567890".to_string().into()),
                                country_code: Some("+1".to_string()),
                            }),
                            email: Some(email.clone()),
                        }),
                        None,
                        None,
                    ),
                    auth_type: AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    connector_order_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_12345".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    minor_amount_capturable: None,
                    amount: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Card {
                        card_number: RawCardNumber(
                            CardNumber::from_str("5123450000000008").unwrap(),
                        ),
                        card_exp_month: "12".to_string().into(),
                        card_exp_year: "2025".to_string().into(),
                        card_cvc: "123".to_string().into(),
                        card_issuer: None,
                        card_network: None,
                        card_type: None,
                        card_issuing_country: None,
                        bank_code: None,
                        nick_name: None,
                        card_holder_name: Some("Test User".to_string().into()),
                        co_badged_card_data: None,
                    }),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: Some(email),
                    customer_name: None,
                    currency: Currency::USD,
                    confirm: true,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    integrity_object: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: Some(BrowserInformation {
                        color_depth: None,
                        java_enabled: Some(false),
                        java_script_enabled: None,
                        language: Some("en-US".to_string()),
                        screen_height: Some(1080),
                        screen_width: Some(1920),
                        time_zone: None,
                        ip_address: None,
                        accept_header: Some(
                            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                                .to_string(),
                        ),
                        user_agent: Some(
                            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string(),
                        ),
                        referer: None,
                        os_type: None,
                        os_version: None,
                        device_model: None,
                        accept_language: None,
                    }),
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    order_category: None,
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
                },
                response: Err(ErrorResponse {
                    code: "HE_00".to_string(),
                    message: "Something went wrong".to_string(),
                    reason: None,
                    status_code: 500,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let http_response = Response {
                headers: None,
                response: br#"{
            "razorpay_payment_id":"pay_QMsUsXCDy9sX3b",
            "next":[
                {
                    "action":"redirect",
                    "url":"https://api.razorpay.com/v1/payments/QMsUsXCDy9sX3b/authenticate"
                }
            ]
        }"#
                .to_vec()
                .into(),
                status_code: 200,
            };

            let result = connector
                .handle_response_v2(&data, None, http_response)
                .unwrap();

            assert!(matches!(
                result.resource_common_data.status,
                AttemptStatus::AuthenticationPending
            ));
        }

        #[test]
        fn test_handle_authorize_error_response() {
            use domain_types::{
                connector_flow::Authorize,
                connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
            };

            let http_response = Response {
                headers: None,
                response: br#"{
                    "error": {
                        "code": "BAD_REQUEST_ERROR",
                        "description": "The id provided does not exist",
                        "source": "internal",
                        "step": "payment_initiation",
                        "reason": "input_validation_failed",
                        "metadata": {}
                    }
                }"#
                .to_vec()
                .into(),
                status_code: 400,
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

            let result =
                <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<DefaultPCIHolder>,
                    PaymentsResponseData,
                >>::get_error_response_v2(&**connector, http_response, None)
                .unwrap();

            let actual_json = to_value(&result).unwrap();

            let expected_json = json!({
                "code": "BAD_REQUEST_ERROR",
                "message": "The id provided does not exist",
                "reason": "input_validation_failed",
                "status_code": 400,
                "attempt_status": "failure",
                "connector_transaction_id": null,
                "network_advice_code": null,
                "network_decline_code": null,
                "network_error_message": null
            });

            assert_eq!(actual_json, expected_json);
        }

        #[test]
        fn test_handle_authorize_missing_required_fields() {
            use domain_types::{
                connector_flow::Authorize,
                connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
            };

            let http_response = Response {
                headers: None,
                response: br#"{
                    "error": {
                        "description": "Missing required fields",
                        "step": "payment_initiation",
                        "reason": "input_validation_failed"
                    }
                }"#
                .to_vec()
                .into(),
                status_code: 400,
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

            let result =
                <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                    Authorize,
                    PaymentFlowData,
                    PaymentsAuthorizeData<DefaultPCIHolder>,
                    PaymentsResponseData,
                >>::get_error_response_v2(&**connector, http_response, None);

            assert!(
                result.is_err(),
                "Expected panic due to missing required fields",
            );
        }
    }

    #[test]
    fn test_handle_authorize_invalid_error_fields() {
        use domain_types::{
            connector_flow::Authorize,
            connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
        };

        let http_response = Response {
            headers: None,
            response: br#"{
            "error": {
                "code": 500,
                "description": "Card number is invalid.",
                "step": "payment_authorization",
                "reason": "input_validation_failed",
                "source": "business",
                "metadata": {}
            }
        }"#
            .to_vec()
            .into(),
            status_code: 400,
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let result =
            <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            >>::get_error_response_v2(&**connector, http_response, None);

        assert!(
            result.is_err(),
            "Expected panic due to missing required fields"
        );
    }

    #[test]
    fn test_handle_response_v2_missing_fields_authorize_response() {
        use std::str::FromStr;

        use common_enums::Currency;
        use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
        use domain_types::{
            connector_types::PaymentFlowData,
            payment_address::PaymentAddress,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
        let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();

        let data = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Card,
                description: None,
                return_url: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: None,
                        phone: Some(PhoneDetails {
                            number: Some("1234567890".to_string().into()),
                            country_code: Some("+1".to_string()),
                        }),
                        email: Some(email.clone()),
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                amount: None,
                minor_amount_captured: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                connector_order_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_12345".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                connectors: Connectors {
                    razorpay: ConnectorParams {
                        base_url: "https://api.razorpay.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                minor_amount_capturable: None,
                l2_l3_data: None,
            },
            connector_config: ConnectorSpecificConfig::Razorpay {
                api_key: "dummy_api_key".to_string().into(),
                api_secret: Some("dummy_key1".to_string().into()),
                base_url: None,
            },
            request: PaymentsAuthorizeData {
                payment_channel: None,
                authentication_data: None,
                connector_testing_data: None,
                payment_method_data: PaymentMethodData::Card(Card {
                    card_number: RawCardNumber(CardNumber::from_str("5123450000000008").unwrap()),
                    card_exp_month: "12".to_string().into(),
                    card_exp_year: "2025".to_string().into(),
                    card_cvc: "123".to_string().into(),
                    card_issuer: None,
                    card_network: None,
                    card_type: None,
                    card_issuing_country: None,
                    bank_code: None,
                    nick_name: None,
                    card_holder_name: Some("Test User".to_string().into()),
                    co_badged_card_data: None,
                }),
                amount: MinorUnit::new(1000),
                order_tax_amount: None,
                email: Some(email),
                customer_name: None,
                currency: Currency::USD,
                confirm: true,
                capture_method: None,
                router_return_url: None,
                webhook_url: None,
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage: None,
                off_session: None,
                browser_info: Some(BrowserInformation {
                    color_depth: None,
                    java_enabled: Some(false),
                    java_script_enabled: None,
                    language: Some("en-US".to_string()),
                    screen_height: Some(1080),
                    screen_width: Some(1920),
                    time_zone: None,
                    ip_address: None,
                    accept_header: Some(
                        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                            .to_string(),
                    ),
                    user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string()),
                    referer: None,
                    os_type: None,
                    os_version: None,
                    device_model: None,
                    accept_language: None,
                }),
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
                order_category: None,
                shipping_cost: None,
                integrity_object: None,
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
            },
            response: Err(ErrorResponse {
                code: "HE_00".to_string(),
                message: "Something went wrong".to_string(),
                reason: None,
                status_code: 500,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        let http_response = Response {
            headers: None,
            response: br#"{
            "next":[
                {
                    "action":"redirect",
                    "url":"https://api.razorpay.com/v1/payments/QMsUsXCDy9sX3b/authenticate"
                }
            ]
        }"#
            .to_vec()
            .into(),
            status_code: 200,
        };

        let result = connector.handle_response_v2(&data, None, http_response);

        assert!(
            result.is_err(),
            "Expected error due to missing razorpay_payment_id, but got success."
        );
    }

    #[test]
    fn test_handle_response_v2_invalid_json_authorize_response() {
        use std::str::FromStr;

        use common_enums::Currency;
        use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
        use domain_types::{
            connector_types::PaymentFlowData,
            payment_address::PaymentAddress,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
        let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();

        let data = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Card,
                description: None,
                return_url: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: None,
                        phone: Some(PhoneDetails {
                            number: Some("1234567890".to_string().into()),
                            country_code: Some("+1".to_string()),
                        }),
                        email: Some(email.clone()),
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                connector_order_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_12345".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                connectors: Connectors {
                    razorpay: ConnectorParams {
                        base_url: "https://api.razorpay.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                minor_amount_capturable: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                l2_l3_data: None,
            },
            connector_config: ConnectorSpecificConfig::Razorpay {
                api_key: "dummy_api_key".to_string().into(),
                api_secret: Some("dummy_key1".to_string().into()),
                base_url: None,
            },
            request: PaymentsAuthorizeData {
                payment_channel: None,
                authentication_data: None,
                connector_testing_data: None,
                payment_method_data: PaymentMethodData::Card(Card {
                    card_number: RawCardNumber(CardNumber::from_str("5123450000000008").unwrap()),
                    card_exp_month: "12".to_string().into(),
                    card_exp_year: "2025".to_string().into(),
                    card_cvc: "123".to_string().into(),
                    card_issuer: None,
                    card_network: None,
                    card_type: None,
                    card_issuing_country: None,
                    bank_code: None,
                    nick_name: None,
                    card_holder_name: Some("Test User".to_string().into()),
                    co_badged_card_data: None,
                }),
                amount: MinorUnit::new(1000),
                order_tax_amount: None,
                email: Some(email),
                customer_name: None,
                currency: Currency::USD,
                confirm: true,
                capture_method: None,
                router_return_url: None,
                webhook_url: None,
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage: None,
                off_session: None,
                browser_info: Some(BrowserInformation {
                    color_depth: None,
                    java_enabled: Some(false),
                    java_script_enabled: None,
                    language: Some("en-US".to_string()),
                    screen_height: Some(1080),
                    screen_width: Some(1920),
                    time_zone: None,
                    ip_address: None,
                    accept_header: Some(
                        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                            .to_string(),
                    ),
                    user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".to_string()),
                    referer: None,
                    os_type: None,
                    os_version: None,
                    device_model: None,
                    accept_language: None,
                }),
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
                order_category: None,
                shipping_cost: None,
                integrity_object: None,
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
            },
            response: Err(ErrorResponse {
                code: "HE_00".to_string(),
                message: "Something went wrong".to_string(),
                reason: None,
                status_code: 500,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        let http_response = Response {
        headers: None,
        response: br#"{"razorpay_payment_id": "pay_xyz", "next": [ { "action": "redirect" "url": "https://api.razorpay.com/v1/payments/xyz/authenticate" } ]"#.to_vec().into(),
        status_code: 200
};

        let result = connector.handle_response_v2(&data, None, http_response);

        assert!(
            result.is_err(),
            "Expected error due to missing razorpay_payment_id, but got success."
        );
    }

    mod order {

        use common_utils::{pii::Email, request::RequestContent};
        use domain_types::{
            payment_address::{Address, PhoneDetails},
            payment_method_data::DefaultPCIHolder,
            router_data::ConnectorSpecificConfig,
            types::{ConnectorParams, Connectors},
        };
        use interfaces::connector_types::BoxedConnector;
        use serde_json::{to_value, Value};

        use crate::connectors::Razorpay;

        #[test]
        fn test_build_request_valid_order() {
            use common_enums::Currency;
            use common_utils::{id_type::MerchantId, request::RequestContent, types::MinorUnit};
            use domain_types::{
                connector_types::PaymentCreateOrderData,
                payment_address::PaymentAddress,
                router_data::{ConnectorSpecificConfig, ErrorResponse},
                router_data_v2::RouterDataV2,
            };
            use serde_json::{to_value, Value};

            let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();

            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: domain_types::connector_types::PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                    attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::new(
                        None,
                        Some(Address {
                            address: None,
                            phone: Some(PhoneDetails {
                                number: Some("1234567890".to_string().into()),
                                country_code: Some("+1".to_string()),
                            }),
                            email: Some(email.clone()),
                        }),
                        None,
                        None,
                    ),
                    auth_type: common_enums::AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_capturable: None,
                    amount: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: None,
                    connector_order_id: None,
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_12345".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentCreateOrderData {
                    amount: MinorUnit::new(1000),
                    currency: Currency::USD,
                    merchant_order_id: None,
                    order_details: None,
                    integrity_object: None,
                    metadata: None,
                    webhook_url: None,
                    payment_method_type: None,
                },
                response: Err(ErrorResponse {
                    code: "HE_00".to_string(),
                    message: "Something went wrong".to_string(),
                    reason: None,
                    status_code: 500,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data).unwrap();

            let actual_json: Value = match result {
                Some(RequestContent::Json(payload)) => {
                    to_value(&payload).expect("Failed to serialize payload")
                }
                Some(RequestContent::RawBytes(bytes)) => {
                    // Handle raw bytes - try to parse as JSON
                    let json_str =
                        String::from_utf8(bytes).expect("Failed to convert bytes to string");
                    serde_json::from_str(&json_str).expect("Failed to parse bytes as JSON")
                }
                Some(RequestContent::FormUrlEncoded(form_data)) => {
                    // Convert form data to JSON for comparison
                    to_value(&form_data).expect("Failed to serialize form data")
                }
                None => panic!("Expected some request content"),
                Some(other) => panic!("Unexpected RequestContent type: {other:?}"),
            };

            assert_eq!(actual_json["amount"], 1000);
            assert_eq!(actual_json["currency"], "USD");

            let receipt_value = &actual_json["receipt"];
            assert!(
                receipt_value.is_string(),
                "Expected receipt to be a string, got: {receipt_value:?}"
            );
            let receipt_str = receipt_value.as_str().unwrap();
            assert!(!receipt_str.is_empty(), "Expected non-empty receipt string");
        }

        #[test]
        fn test_build_request_missing() {
            use common_enums::Currency;
            use common_utils::{id_type::MerchantId, types::MinorUnit};
            use domain_types::{
                connector_types::PaymentCreateOrderData,
                payment_address::PaymentAddress,
                router_data::{ConnectorSpecificConfig, ErrorResponse},
                router_data_v2::RouterDataV2,
            };

            use crate::connectors::Razorpay;

            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: domain_types::connector_types::PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "".to_string(),
                    attempt_id: "".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::default(),
                    auth_type: common_enums::AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_capturable: None,
                    amount: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    connector_order_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_12345".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "dummy_api_key".to_string().into(),
                    api_secret: Some("dummy_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentCreateOrderData {
                    amount: MinorUnit::new(0),
                    currency: Currency::default(),
                    merchant_order_id: None,
                    order_details: None,
                    integrity_object: None,
                    metadata: None,
                    webhook_url: None,
                    payment_method_type: None,
                },
                response: Err(ErrorResponse {
                    code: "HE_01".to_string(),
                    message: "Missing required fields".to_string(),
                    reason: None,
                    status_code: 400,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data);
            let req = result.unwrap();

            let actual_json: Value = match req {
                Some(RequestContent::Json(payload)) => {
                    to_value(&payload).expect("Failed to serialize payload")
                }
                None => {
                    return;
                }
                Some(RequestContent::RawBytes(bytes)) => {
                    let json_str =
                        String::from_utf8(bytes).expect("Failed to convert bytes to string");
                    serde_json::from_str(&json_str).expect("Failed to parse bytes as JSON")
                }
                Some(RequestContent::FormUrlEncoded(form_data)) => {
                    to_value(&form_data).expect("Failed to serialize form data")
                }
                Some(other) => panic!("Unexpected RequestContent type: {other:?}"),
            };

            assert_eq!(actual_json["amount"], 0);
            assert_eq!(actual_json["currency"], "USD");

            let receipt_value = &actual_json["receipt"];
            assert!(
                receipt_value.is_string(),
                "Expected receipt to be a string, got: {receipt_value:?}"
            );
        }

        #[test]
        fn test_build_request_invalid() {
            use common_enums::{
                AttemptStatus, AuthenticationType, Currency, PaymentMethod, PaymentMethodType,
            };
            use common_utils::{id_type::MerchantId, types::MinorUnit};
            use domain_types::{
                connector_types::{PaymentFlowData, PaymentsAuthorizeData},
                payment_address::PaymentAddress,
                payment_method_data::{Card, PaymentMethodData},
                router_data::ErrorResponse,
                router_data_v2::RouterDataV2,
                types::{ConnectorParams, Connectors},
            };

            use crate::connectors::Razorpay;

            let test_router_data = RouterDataV2 {
                flow: std::marker::PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "invalid_payment_id".to_string(),
                    attempt_id: "invalid_attempt_id".to_string(),
                    status: AttemptStatus::Pending,
                    payment_method: PaymentMethod::Card,
                    description: None,
                    return_url: None,
                    address: PaymentAddress::new(None, None, None, None),
                    auth_type: AuthenticationType::NoThreeDs,
                    connector_feature_data: None,
                    amount_captured: None,
                    minor_amount_captured: None,
                    minor_amount_capturable: None,
                    amount: None,
                    minor_amount_authorized: None,
                    access_token: None,
                    session_token: None,
                    reference_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    connector_order_id: Some("order_QMsUrrLPdwNxPG".to_string()),
                    preprocessing_id: None,
                    connector_api_version: None,
                    connector_request_reference_id: "ref_12345".to_string(),
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    raw_connector_response: None,
                    connectors: Connectors {
                        razorpay: ConnectorParams {
                            base_url: "https://api.razorpay.com/".to_string(),
                            dispute_base_url: None,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    vault_headers: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    order_details: None,
                    l2_l3_data: None,
                },
                connector_config: ConnectorSpecificConfig::Razorpay {
                    api_key: "invalid_key".to_string().into(),
                    api_secret: Some("invalid_key1".to_string().into()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_channel: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    payment_method_data: PaymentMethodData::Card(Card {
                        card_number: Default::default(),
                        card_exp_month: "".to_string().into(),
                        card_exp_year: "".to_string().into(),
                        card_cvc: "".to_string().into(),
                        card_issuer: None,
                        card_network: None,
                        card_type: None,
                        card_issuing_country: None,
                        bank_code: None,
                        nick_name: None,
                        card_holder_name: Some("Test User".to_string().into()),
                        co_badged_card_data: None,
                    }),
                    amount: MinorUnit::new(1000),
                    order_tax_amount: None,
                    email: None,
                    customer_name: None,
                    currency: Currency::USD,
                    confirm: true,
                    billing_descriptor: None,
                    capture_method: None,
                    router_return_url: None,
                    webhook_url: None,
                    complete_authorize_url: None,
                    mandate_id: None,
                    setup_future_usage: None,
                    off_session: None,
                    browser_info: None,
                    session_token: None,
                    enrolled_for_3ds: Some(false),
                    related_transaction_id: None,
                    payment_experience: None,
                    payment_method_type: Some(PaymentMethodType::Card),
                    customer_id: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    integrity_object: None,
                    minor_amount: MinorUnit::new(1000),
                    merchant_order_id: None,
                    order_category: None,
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
                    enable_partial_authorization: None,
                    locale: None,
                    continue_redirection_url: None,
                    redirect_response: None,
                    threeds_method_comp_ind: None,
                    tokenization: None,
                },
                response: Err(ErrorResponse {
                    code: "HE_INVALID".to_string(),
                    message: "Invalid request body".to_string(),
                    reason: None,
                    status_code: 422,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
            };

            let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());
            let result = connector.get_request_body(&test_router_data);

            assert!(
                result.is_err(),
                "Expected error for invalid request data, but got: {result:?}"
            );
        }
    }

    #[test]
    fn test_handle_response_v2_valid_order_response() {
        use common_enums::Currency;
        use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
        use domain_types::{
            connector_types::{PaymentCreateOrderData, PaymentFlowData},
            payment_address::PaymentAddress,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };
        let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();
        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let data = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Card,
                description: None,
                return_url: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: None,
                        phone: Some(PhoneDetails {
                            number: Some("1234567890".to_string().into()),
                            country_code: Some("+1".to_string()),
                        }),
                        email: Some(email.clone()),
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_12345".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                connectors: Connectors {
                    razorpay: ConnectorParams {
                        base_url: "https://api.razorpay.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                minor_amount_capturable: None,
                l2_l3_data: None,
            },
            connector_config: ConnectorSpecificConfig::Razorpay {
                api_key: "dummy_api_key".to_string().into(),
                api_secret: Some("dummy_key1".to_string().into()),
                base_url: None,
            },
            request: PaymentCreateOrderData {
                amount: MinorUnit::new(1000),
                currency: Currency::USD,
                merchant_order_id: None,
                order_details: None,
                integrity_object: None,
                metadata: None,
                webhook_url: None,
                payment_method_type: None,
            },
            response: Err(ErrorResponse {
                code: "HE_00".to_string(),
                message: "Something went wrong".to_string(),
                reason: None,
                status_code: 500,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        let http_response = Response {
            headers: None,
            response: br#"{
                "amount":1000,
                "amount_due":1000,
                "amount_paid":0,
                "attempts":0,
                "created_at":1745490447,
                "currency":"USD",
                "entity":"order",
                "id":"order_QMrTOdLWvEHsXz",
                "notes":[],
                "offer_id":null,
                "receipt":"141674f6-30d3-4a17-b904-27fe6ca085c7",
                "status":"created"
            }"#
            .to_vec()
            .into(),
            status_code: 200,
        };

        let result = connector
            .handle_response_v2(&data, None, http_response)
            .unwrap();

        assert_eq!(
            result.response.unwrap().connector_order_id,
            "order_QMrTOdLWvEHsXz".to_string()
        );
    }

    #[test]
    fn test_handle_response_missing() {
        use common_enums::Currency;
        use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
        use domain_types::{
            connector_types::PaymentCreateOrderData,
            payment_address::PaymentAddress,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };

        let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();
        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let data = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Card,
                description: None,
                return_url: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: None,
                        phone: Some(PhoneDetails {
                            number: Some("1234567890".to_string().into()),
                            country_code: Some("+1".to_string()),
                        }),
                        email: Some(email.clone()),
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_capturable: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_12345".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                connectors: Connectors {
                    razorpay: ConnectorParams {
                        base_url: "https://api.razorpay.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                l2_l3_data: None,
            },
            connector_config: ConnectorSpecificConfig::Razorpay {
                api_key: "dummy_api_key".to_string().into(),
                api_secret: Some("dummy_key1".to_string().into()),
                base_url: None,
            },
            request: PaymentCreateOrderData {
                amount: MinorUnit::new(1000),
                currency: Currency::USD,
                merchant_order_id: None,
                order_details: None,
                integrity_object: None,
                metadata: None,
                webhook_url: None,
                payment_method_type: None,
            },
            response: Err(ErrorResponse {
                code: "HE_00".to_string(),
                message: "Something went wrong".to_string(),
                reason: None,
                status_code: 500,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        let http_response = Response {
            headers: None,
            response: br#"{
            "amount":1000,
            "currency":"USD",
            "status":"created"
        }"#
            .to_vec()
            .into(),
            status_code: 200,
        };

        let result = connector.handle_response_v2(&data, None, http_response);

        assert!(
            result.is_err(),
            "Expected error due to missing order_id or receipt"
        );
    }

    #[test]
    fn test_handle_response_invalid() {
        use common_enums::Currency;
        use common_utils::{id_type::MerchantId, pii::Email, types::MinorUnit};
        use domain_types::{
            connector_types::PaymentCreateOrderData,
            payment_address::PaymentAddress,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };

        let email = Email::try_from("testuser@gmail.com".to_string()).unwrap();
        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let data = RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "IRRELEVANT_PAYMENT_ID".to_string(),
                attempt_id: "IRRELEVANT_ATTEMPT_ID".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Card,
                description: None,
                return_url: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: None,
                        phone: Some(PhoneDetails {
                            number: Some("1234567890".to_string().into()),
                            country_code: Some("+1".to_string()),
                        }),
                        email: Some(email.clone()),
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_capturable: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "ref_12345".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                connectors: Connectors {
                    razorpay: ConnectorParams {
                        base_url: "https://api.razorpay.com/".to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                l2_l3_data: None,
            },
            connector_config: ConnectorSpecificConfig::Razorpay {
                api_key: "dummy_api_key".to_string().into(),
                api_secret: Some("dummy_key1".to_string().into()),
                base_url: None,
            },
            request: PaymentCreateOrderData {
                amount: MinorUnit::new(1000),
                currency: Currency::USD,
                merchant_order_id: None,
                order_details: None,
                integrity_object: None,
                metadata: None,
                webhook_url: None,
                payment_method_type: None,
            },
            response: Err(ErrorResponse {
                code: "HE_00".to_string(),
                message: "Something went wrong".to_string(),
                reason: None,
                status_code: 500,
                attempt_status: None,
                connector_transaction_id: None,
                network_decline_code: None,
                network_advice_code: None,
                network_error_message: None,
            }),
        };

        let http_response = Response {
            headers: None,
            response: br#"{
            "amount":1000,
            "currency":"USD",
            "status":"created"
        }"#
            .to_vec()
            .into(),
            status_code: 500,
        };

        let result = connector.handle_response_v2(&data, None, http_response);

        assert!(
            result.is_err(),
            "Expected error due to invalid response format"
        );
    }

    #[test]
    fn test_handle_error_response_valid() {
        let http_response = Response {
            headers: None,
            response: br#"{
                "error": {
                    "code": "BAD_REQUEST_ERROR",
                    "description": "Order receipt should be unique.",
                    "step": "payment_initiation",
                    "reason": "input_validation_failed",
                    "source": "business",
                    "metadata": {
                        "order_id": "order_OL0t841dI8F9NV"
                    }
                }
            }"#
            .to_vec()
            .into(),
            status_code: 400,
        };
        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let result =
            <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >>::get_error_response_v2(&**connector, http_response, None)
            .unwrap();

        let actual_json = to_value(&result).unwrap();
        let expected_json = json!({
            "code": "BAD_REQUEST_ERROR",
            "message": "Order receipt should be unique.",
            "reason": "input_validation_failed",
            "status_code": 400,
            "attempt_status": "failure",
            "connector_transaction_id": null,
            "network_advice_code": null,
            "network_decline_code": null,
            "network_error_message": null
        });
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn test_handle_error_response_invalid_json() {
        let http_response = Response {
            headers: None,
            response: br#"{ "error": { "code": "BAD_REQUEST_ERROR" "#.to_vec().into(),
            status_code: 400,
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let result =
            <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >>::get_error_response_v2(&**connector, http_response, None);

        assert!(result.is_err(), "Expected error for invalid JSON");
    }
    #[test]
    fn test_handle_error_response_missing_error_field() {
        let http_response = Response {
            headers: None,
            response: br#"{
            "message": "Some generic message",
            "status": "failed"
        }"#
            .to_vec()
            .into(),
            status_code: 400,
        };

        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Razorpay::new());

        let result =
            <dyn ConnectorServiceTrait<DefaultPCIHolder> + Sync as ConnectorIntegrationV2<
                domain_types::connector_flow::CreateOrder,
                PaymentFlowData,
                domain_types::connector_types::PaymentCreateOrderData,
                domain_types::connector_types::PaymentCreateOrderResponse,
            >>::get_error_response_v2(&**connector, http_response, None)
            .unwrap();

        let actual_json = to_value(&result).unwrap();
        let expected_json = json!({
            "code": "ROUTE_ERROR",
            "message": "Some generic message",
            "reason": "Some generic message",
            "status_code": 400,
            "attempt_status": "failure",
            "connector_transaction_id": null,
            "network_advice_code": null,
            "network_decline_code": null,
            "network_error_message": null
        });
        assert_eq!(actual_json, expected_json);
    }
}
