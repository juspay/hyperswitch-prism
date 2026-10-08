#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::print_stdout)]
mod tests {
    pub mod link_token {
        use std::marker::PhantomData;

        use common_enums::CountryAlpha2;
        use common_utils::{id_type::CustomerId, request::RequestContent};
        use domain_types::{
            connector_flow::ClientAuthenticationToken,
            connector_types::{
                ClientAuthenticationTokenRequestData, CustomerInfo, PaymentsResponseData,
            },
            merchant_authentication_flow_data::MerchantAuthenticationFlowData,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::Connectors,
        };
        use hyperswitch_masking::Secret;
        use interfaces::connector_integration_v2::{
            BoxedConnectorIntegrationV2, ConnectorIntegrationAnyV2,
        };
        use serde_json::json;

        use crate::authenticator_connectors::Plaid;
        use domain_types::payment_method_data::DefaultPCIHolder;

        fn make_req(
            client_name: Option<&str>,
            customer: Option<CustomerInfo>,
            country_codes: Vec<CountryAlpha2>,
        ) -> RouterDataV2<
            ClientAuthenticationToken,
            MerchantAuthenticationFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        > {
            RouterDataV2 {
                flow: PhantomData,
                resource_common_data: MerchantAuthenticationFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    connectors: Connectors::default().into(),
                    connector_request_reference_id: "ref_test".to_owned(),
                    test_mode: None,
                    return_url: None,
                    connector_feature_data: None,
                    order_details: None,
                    merchant_request_id: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    typed_connector_response: None,
                    connector_response_headers: None,
                },
                connector_config: ConnectorSpecificConfig::Plaid {
                    client_id: Secret::new("test_client_id".to_owned()),
                    secret: Secret::new("test_secret".to_owned()),
                    client_name: client_name.map(str::to_owned),
                    base_url: None,
                },
                request: ClientAuthenticationTokenRequestData {
                    amount: common_utils::types::MinorUnit::new(0),
                    currency: common_enums::Currency::USD,
                    country: None,
                    order_details: None,
                    customer,
                    order_tax_amount: None,
                    shipping_cost: None,
                    payment_method_type: None,
                    webhook_url: None,
                    country_codes,
                    locale: None,
                    permissions: None,
                    native_app_identifier: None,
                },
                response: Err(ErrorResponse::default()),
            }
        }

        fn customer() -> CustomerInfo {
            CustomerInfo {
                customer_id: Some(
                    CustomerId::try_from(std::borrow::Cow::from("cus_test".to_owned())).unwrap(),
                ),
                customer_email: None,
                customer_name: None,
                first_name: None,
                last_name: None,
                customer_phone_number: None,
                customer_phone_country_code: None,
                salutation: None,
                date_of_birth: None,
            }
        }

        #[test]
        fn test_build_request_valid() {
            let req = make_req(Some("My App"), Some(customer()), vec![CountryAlpha2::US]);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            > = connector.get_connector_integration_v2();

            let request = integration.build_request_v2(&req).unwrap();
            let body = request.as_ref().map(|r| match r.body.as_ref() {
                Some(RequestContent::Json(v)) => v.masked_serialize().unwrap_or(json!({})),
                _ => json!({}),
            });
            println!("link_token request body: {body:?}");
            assert_eq!(body.as_ref().unwrap()["client_name"], "My App");
            assert_eq!(body.as_ref().unwrap()["country_codes"], json!(["US"]));
            assert_eq!(body.as_ref().unwrap()["products"], json!(["auth"]));
        }

        #[test]
        fn test_build_request_missing_client_name() {
            let req = make_req(None, Some(customer()), vec![CountryAlpha2::US]);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            > = connector.get_connector_integration_v2();

            let result = integration.build_request_v2(&req);
            assert!(result.is_err(), "expected error for missing client_name");
        }

        #[test]
        fn test_build_request_missing_customer() {
            let req = make_req(Some("My App"), None, vec![CountryAlpha2::US]);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            > = connector.get_connector_integration_v2();

            let result = integration.build_request_v2(&req);
            assert!(result.is_err(), "expected error for missing customer");
        }

        #[test]
        fn test_build_request_missing_country_codes() {
            let req = make_req(Some("My App"), Some(customer()), vec![]);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                ClientAuthenticationToken,
                MerchantAuthenticationFlowData,
                ClientAuthenticationTokenRequestData,
                PaymentsResponseData,
            > = connector.get_connector_integration_v2();

            let result = integration.build_request_v2(&req);
            assert!(result.is_err(), "expected error for empty country_codes");
        }
    }

    pub mod token_exchange {
        use std::marker::PhantomData;

        use common_utils::request::RequestContent;
        use domain_types::{
            connector_flow::PaymentMethodToken,
            connector_types::{
                PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
            },
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::Connectors,
        };
        use hyperswitch_masking::Secret;
        use interfaces::connector_integration_v2::{
            BoxedConnectorIntegrationV2, ConnectorIntegrationAnyV2,
        };
        use serde_json::json;

        use crate::authenticator_connectors::Plaid;
        use domain_types::payment_method_data::DefaultPCIHolder;

        fn make_req(
            metadata: Option<&str>,
        ) -> RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<DefaultPCIHolder>,
            PaymentMethodTokenResponse,
        > {
            RouterDataV2 {
                flow: PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "pay_test".to_owned(),
                    attempt_id: "attempt_test".to_owned(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::BankDebit,
                    payment_method_type: None,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
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
                    connector_request_reference_id: "ref_test".to_owned(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors::default().into(),
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    vault_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    minor_amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    settlement_status: None,
                    raw_connector_status: None,
                    connector_returned_payment_method_details: None,
                    typed_connector_response: None,
                },
                connector_config: ConnectorSpecificConfig::Plaid {
                    client_id: Secret::new("test_client_id".to_owned()),
                    secret: Secret::new("test_secret".to_owned()),
                    client_name: Some("My App".to_owned()),
                    base_url: None,
                },
                request: PaymentMethodTokenizationData {
                    payment_method_data:
                        domain_types::payment_method_data::PaymentMethodData::BankDebit(
                            domain_types::payment_method_data::BankDebitData::AchBankDebit {
                                account_number: Secret::new("000123456789".to_owned()),
                                routing_number: Secret::new("021000021".to_owned()),
                                card_holder_name: None,
                                bank_account_holder_name: None,
                                bank_name: None,
                                bank_type: None,
                                bank_holder_type: None,
                            },
                        ),
                    amount: common_utils::types::MinorUnit::new(0),
                    currency: common_enums::Currency::USD,
                    metadata: metadata.map(|s| Secret::new(s.to_owned())),
                    split_payments: None,
                    connector_feature_data: None,
                    browser_info: None,
                    customer_acceptance: None,
                    setup_future_usage: None,
                    setup_mandate_details: None,
                    mandate_id: None,
                    integrity_object: None,
                    capture_method: None,
                },
                response: Err(ErrorResponse::default()),
            }
        }

        #[test]
        fn test_build_request_valid() {
            let metadata = r#"{"public_token": "public-sandbox-token"}"#;
            let req = make_req(Some(metadata));

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<DefaultPCIHolder>,
                PaymentMethodTokenResponse,
            > = connector.get_connector_integration_v2();

            let request = integration.build_request_v2(&req).unwrap();
            let body = request.as_ref().map(|r| match r.body.as_ref() {
                Some(RequestContent::Json(v)) => v.masked_serialize().unwrap_or(json!({})),
                _ => json!({}),
            });
            println!("token_exchange request body: {body:?}");
            // public_token is masked — check the key exists
            assert!(body.as_ref().unwrap().get("public_token").is_some());
        }

        #[test]
        fn test_build_request_missing_public_token() {
            let req = make_req(None);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<DefaultPCIHolder>,
                PaymentMethodTokenResponse,
            > = connector.get_connector_integration_v2();

            let result = integration.build_request_v2(&req);
            assert!(
                result.is_err(),
                "expected error for missing metadata.public_token"
            );
        }
    }

    pub mod auth_get {
        use std::marker::PhantomData;

        use common_enums::{BankHolderType, BankType};
        use domain_types::{
            connector_flow::GetPaymentMethod,
            connector_types::{
                GetPaymentMethodData, GetPaymentMethodResponseData, PaymentFlowData,
            },
            payment_method_data::{BankAccountRoutingDetails, PaymentMethodDetails},
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::Connectors,
        };
        use hyperswitch_masking::Secret;

        use crate::{
            authenticator_connectors::plaid::transformers::{
                PlaidAccount, PlaidAchNumbers, PlaidAuthGetResponse, PlaidBacsNumbers,
                PlaidBalances, PlaidHolderCategory, PlaidInternationalNumbers, PlaidItem,
                PlaidNumbers,
            },
            types::ResponseRouterData,
        };

        fn make_router_data() -> RouterDataV2<
            GetPaymentMethod,
            PaymentFlowData,
            GetPaymentMethodData,
            GetPaymentMethodResponseData,
        > {
            RouterDataV2 {
                flow: PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "pay_test".to_owned(),
                    attempt_id: "attempt_test".to_owned(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::BankDebit,
                    payment_method_type: None,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
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
                    connector_request_reference_id: "ref_test".to_owned(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors::default().into(),
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    vault_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    minor_amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    settlement_status: None,
                    raw_connector_status: None,
                    connector_returned_payment_method_details: None,
                    typed_connector_response: None,
                },
                connector_config: ConnectorSpecificConfig::Plaid {
                    client_id: Secret::new("test_client_id".to_owned()),
                    secret: Secret::new("test_secret".to_owned()),
                    client_name: Some("My App".to_owned()),
                    base_url: None,
                },
                request: GetPaymentMethodData {
                    merchant_payment_method_id: None,
                    connector_payment_method_id: None,
                    customer: None,
                    payment_method_type: common_enums::PaymentMethodType::Ach,
                    connector_feature_data: None,
                    payment_method_token: Some(Secret::new("access_token_xxx".to_owned())),
                },
                response: Err(ErrorResponse::default()),
            }
        }

        fn account(
            id: &str,
            subtype: Option<&str>,
            holder_category: Option<PlaidHolderCategory>,
            current: Option<f64>,
            currency: Option<common_enums::Currency>,
        ) -> PlaidAccount {
            PlaidAccount {
                account_id: Secret::new(id.to_owned()),
                name: Secret::new(format!("Account {id}")),
                subtype: subtype.map(str::to_owned),
                holder_category,
                mask: None,
                official_name: None,
                account_type: None,
                balances: PlaidBalances {
                    current: current.map(common_utils::types::FloatMajorUnit),
                    available: None,
                    limit: None,
                    iso_currency_code: currency,
                    unofficial_currency_code: None,
                },
            }
        }

        fn parse(
            accounts: Vec<PlaidAccount>,
            numbers: PlaidNumbers,
        ) -> domain_types::payment_method_data::BankAccountDetails {
            let response = PlaidAuthGetResponse {
                accounts,
                numbers,
                item: PlaidItem {
                    item_id: "item_001".to_owned(),
                    institution_name: Some("Test Bank".to_owned()),
                },
                request_id: "req_001".to_owned(),
            };
            let wrapped = ResponseRouterData {
                response,
                router_data: make_router_data(),
                http_code: 200,
            };
            let result = RouterDataV2::<
                GetPaymentMethod,
                PaymentFlowData,
                GetPaymentMethodData,
                GetPaymentMethodResponseData,
            >::try_from(wrapped)
            .expect("try_from failed");

            match result.response.expect("response Ok") {
                GetPaymentMethodResponseData {
                    payment_method_details: Some(PaymentMethodDetails::BankAccount(details)),
                    ..
                } => details,
                other => panic!("unexpected: {other:?}"),
            }
        }

        #[test]
        fn test_ach_account_parsed() {
            let accounts = vec![account(
                "acct_1",
                Some("checking"),
                Some(PlaidHolderCategory::Personal),
                Some(100.0),
                Some(common_enums::Currency::USD),
            )];
            let numbers = PlaidNumbers {
                ach: vec![PlaidAchNumbers {
                    account_id: Secret::new("acct_1".to_owned()),
                    account: Secret::new("000123456789".to_owned()),
                    routing: Secret::new("021000021".to_owned()),
                }],
                ..Default::default()
            };

            let details = parse(accounts, numbers);
            assert_eq!(details.accounts.len(), 1);
            let acct = details
                .accounts
                .first()
                .expect("expected at least one account");
            assert!(matches!(acct.bank_type, Some(BankType::Checking)));
            assert!(matches!(
                acct.bank_holder_type,
                Some(BankHolderType::Personal)
            ));
            assert!(matches!(
                acct.account_details,
                Some(BankAccountRoutingDetails::Ach(_))
            ));
        }

        #[test]
        fn test_bacs_account_parsed() {
            let accounts = vec![account(
                "acct_2",
                Some("savings"),
                Some(PlaidHolderCategory::Business),
                Some(200.0),
                Some(common_enums::Currency::GBP),
            )];
            let numbers = PlaidNumbers {
                bacs: vec![PlaidBacsNumbers {
                    account_id: Secret::new("acct_2".to_owned()),
                    account: Secret::new("12345678".to_owned()),
                    sort_code: Secret::new("200000".to_owned()),
                }],
                ..Default::default()
            };

            let details = parse(accounts, numbers);
            assert_eq!(details.accounts.len(), 1);
            let acct = details
                .accounts
                .first()
                .expect("expected at least one account");
            assert!(matches!(acct.bank_type, Some(BankType::Savings)));
            assert!(matches!(
                acct.bank_holder_type,
                Some(BankHolderType::Business)
            ));
            assert!(matches!(
                acct.account_details,
                Some(BankAccountRoutingDetails::Bacs(_))
            ));
        }

        #[test]
        fn test_international_account_parsed() {
            let accounts = vec![account(
                "acct_3",
                None,
                None,
                Some(300.0),
                Some(common_enums::Currency::EUR),
            )];
            let numbers = PlaidNumbers {
                international: vec![PlaidInternationalNumbers {
                    account_id: Secret::new("acct_3".to_owned()),
                    iban: Secret::new("DE89370400440532013000".to_owned()),
                    bic: Some(Secret::new("COBADEFFXXX".to_owned())),
                }],
                ..Default::default()
            };

            let details = parse(accounts, numbers);
            assert_eq!(details.accounts.len(), 1);
            let acct = details
                .accounts
                .first()
                .expect("expected at least one account");
            assert!(matches!(
                acct.account_details,
                Some(BankAccountRoutingDetails::Sepa(_))
            ));
        }

        #[test]
        fn test_account_absent_from_numbers_is_dropped() {
            let accounts = vec![account("acct_orphan", Some("checking"), None, None, None)];
            let details = parse(accounts, PlaidNumbers::default());
            assert!(
                details.accounts.is_empty(),
                "account with no routing numbers entry must be dropped"
            );
        }

        #[test]
        fn test_unknown_subtype_yields_no_bank_type() {
            let accounts = vec![account("acct_4", Some("money_market"), None, None, None)];
            let numbers = PlaidNumbers {
                ach: vec![PlaidAchNumbers {
                    account_id: Secret::new("acct_4".to_owned()),
                    account: Secret::new("000000001".to_owned()),
                    routing: Secret::new("021000021".to_owned()),
                }],
                ..Default::default()
            };
            let details = parse(accounts, numbers);
            assert_eq!(details.accounts.len(), 1);
            assert!(details
                .accounts
                .first()
                .expect("expected at least one account")
                .bank_type
                .is_none());
        }
    }

    pub mod balance_get {
        use std::marker::PhantomData;

        use common_utils::request::RequestContent;
        use domain_types::{
            connector_flow::PaymentMethodEligibility,
            connector_types::{
                PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse,
            },
            payment_method_data::PaymentMethodDetails,
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::Connectors,
        };
        use hyperswitch_masking::Secret;
        use interfaces::connector_integration_v2::{
            BoxedConnectorIntegrationV2, ConnectorIntegrationAnyV2,
        };
        use serde_json::json;

        use domain_types::payment_method_data::DefaultPCIHolder;

        use crate::{
            authenticator_connectors::{
                plaid::transformers::{
                    PlaidAccount, PlaidBalanceGetResponse, PlaidBalances, PlaidItem,
                },
                Plaid,
            },
            types::ResponseRouterData,
        };

        type BalanceRouterData = RouterDataV2<
            PaymentMethodEligibility,
            PaymentFlowData,
            PaymentMethodEligibilityData,
            PaymentMethodEligibilityResponse,
        >;

        fn make_router_data(connector_payment_method_id: Option<&str>) -> BalanceRouterData {
            RouterDataV2 {
                flow: PhantomData,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    customer_id: None,
                    connector_customer: None,
                    payment_id: "pay_test".to_owned(),
                    attempt_id: "attempt_test".to_owned(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::BankDebit,
                    payment_method_type: None,
                    description: None,
                    return_url: None,
                    order_details: None,
                    address: domain_types::payment_address::PaymentAddress::new(
                        None, None, None, None,
                    ),
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
                    connector_request_reference_id: "ref_test".to_owned(),
                    test_mode: None,
                    connector_http_status_code: None,
                    connectors: Connectors::default().into(),
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    vault_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    minor_amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    settlement_status: None,
                    raw_connector_status: None,
                    connector_returned_payment_method_details: None,
                    typed_connector_response: None,
                },
                connector_config: ConnectorSpecificConfig::Plaid {
                    client_id: Secret::new("test_client_id".to_owned()),
                    secret: Secret::new("test_secret".to_owned()),
                    client_name: Some("My App".to_owned()),
                    base_url: None,
                },
                request: PaymentMethodEligibilityData {
                    amount: common_utils::types::Money {
                        amount: common_utils::types::MinorUnit::new(0),
                        currency: common_enums::Currency::USD,
                    },
                    customer: None,
                    connector_payment_method_id: connector_payment_method_id.map(str::to_owned),
                    country_code: Some(common_enums::CountryAlpha2::US),
                    payment_method_types: vec![grpc_api_types::payments::PaymentMethodType::Ach],
                    description: None,
                    metadata: None,
                    connector_feature_data: None,
                    test_mode: None,
                },
                response: Err(ErrorResponse::default()),
            }
        }

        fn plaid_account(id: &str, available: Option<f64>, current: Option<f64>) -> PlaidAccount {
            PlaidAccount {
                account_id: Secret::new(id.to_owned()),
                name: Secret::new(format!("Account {id}")),
                subtype: Some("checking".to_owned()),
                holder_category: None,
                mask: Some(Secret::new("0000".to_owned())),
                official_name: None,
                account_type: Some("depository".to_owned()),
                balances: PlaidBalances {
                    available: available.map(common_utils::types::FloatMajorUnit),
                    current: current.map(common_utils::types::FloatMajorUnit),
                    limit: None,
                    iso_currency_code: Some(common_enums::Currency::USD),
                    unofficial_currency_code: None,
                },
            }
        }

        fn plaid_response(
            accounts: Vec<PlaidAccount>,
        ) -> ResponseRouterData<PlaidBalanceGetResponse, BalanceRouterData> {
            ResponseRouterData {
                response: PlaidBalanceGetResponse {
                    accounts,
                    item: PlaidItem {
                        item_id: "item_001".to_owned(),
                        institution_name: Some("Test Bank".to_owned()),
                    },
                    request_id: "req_001".to_owned(),
                },
                router_data: make_router_data(Some("access-sandbox-xxx")),
                http_code: 200,
            }
        }

        #[test]
        fn test_build_request_serializes_in_body_auth() {
            let req = make_router_data(Some("access-sandbox-xxx"));

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            > = connector.get_connector_integration_v2();

            let request = integration.build_request_v2(&req).unwrap();
            assert!(request.as_ref().is_some(), "expected a built request");
            let body = request.as_ref().map(|r| match r.body.as_ref() {
                Some(RequestContent::Json(v)) => v.masked_serialize().unwrap_or(json!({})),
                _ => json!({}),
            });
            println!("balance_get request body: {body:?}");
            let body = body.as_ref().expect("request body");
            // client_id/secret/access_token are Secret — masked in serialize, but the keys exist
            assert!(body.get("client_id").is_some());
            assert!(body.get("secret").is_some());
            assert!(body.get("access_token").is_some());
            // options is None and skipped
            assert!(body.get("options").is_none());
        }

        #[test]
        fn test_build_request_missing_connector_payment_method_id() {
            let req = make_router_data(None);

            let connector = Plaid::<DefaultPCIHolder>::new();
            let integration: BoxedConnectorIntegrationV2<
                '_,
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            > = connector.get_connector_integration_v2();

            let result = integration.build_request_v2(&req);
            assert!(
                result.is_err(),
                "expected error for missing connector_payment_method_id"
            );
        }

        fn parse_response(
            wrapped: ResponseRouterData<PlaidBalanceGetResponse, BalanceRouterData>,
        ) -> PaymentMethodEligibilityResponse {
            let result = BalanceRouterData::try_from(wrapped).expect("try_from failed");
            result.response.expect("response Ok")
        }

        #[test]
        fn test_response_with_accounts_is_eligible() {
            let wrapped = plaid_response(vec![plaid_account("acct_1", Some(100.0), Some(110.0))]);
            let res = parse_response(wrapped);

            assert_eq!(res.status_code, 200);
            assert_eq!(res.results.len(), 1);
            let result = res.results.first().expect("one result");
            assert_eq!(result.eligibility, common_enums::EligibilityStatus::Eligible);
            assert!(result.error_info.is_none());
            assert_eq!(
                result.payment_method_type,
                grpc_api_types::payments::PaymentMethodType::Ach
            );
            match &result.payment_method_details {
                Some(PaymentMethodDetails::BankAccount(details)) => {
                    assert_eq!(details.accounts.len(), 1);
                    let acct = details.accounts.first().expect("one account");
                    assert_eq!(
                        acct.balance.as_ref().map(|m| m.amount),
                        Some(common_utils::types::MinorUnit::new(110_00))
                    );
                    assert_eq!(
                        acct.available_balance.as_ref().map(|m| m.amount),
                        Some(common_utils::types::MinorUnit::new(100_00))
                    );
                    assert!(acct.account_details.is_none());
                }
                other => panic!("expected bank account details, got {other:?}"),
            }
        }

        #[test]
        fn test_response_with_no_accounts_is_ineligible() {
            let wrapped = plaid_response(vec![]);
            let res = parse_response(wrapped);

            assert_eq!(res.results.len(), 1);
            let result = res.results.first().expect("one result");
            assert_eq!(
                result.eligibility,
                common_enums::EligibilityStatus::Ineligible
            );
            assert!(result.payment_method_details.is_none());
            let error_info = result.error_info.as_ref().expect("error_info present");
            assert_eq!(error_info.code, "NO_ACCOUNTS");
        }
    }

    pub mod transformer {
        use crate::authenticator_connectors::plaid::transformers::{
            PlaidAuthType, PlaidLinkTokenResponse, PlaidNumbers,
        };
        use domain_types::{errors::IntegrationError, router_data::ConnectorSpecificConfig};
        use hyperswitch_masking::Secret;

        fn plaid_config(client_name: Option<&str>) -> ConnectorSpecificConfig {
            ConnectorSpecificConfig::Plaid {
                client_id: Secret::new("test_client_id".to_owned()),
                secret: Secret::new("test_secret".to_owned()),
                client_name: client_name.map(str::to_owned),
                base_url: None,
            }
        }

        #[test]
        fn test_auth_type_succeeds_for_plaid_config() {
            let auth = PlaidAuthType::try_from(&plaid_config(Some("My App")));
            assert!(auth.is_ok());
            assert_eq!(auth.unwrap().client_name.as_deref(), Some("My App"));
        }

        #[test]
        fn test_auth_type_fails_for_non_plaid_config() {
            let config = ConnectorSpecificConfig::Stripe {
                api_key: Secret::new("sk_test_xxx".to_owned()),
                base_url: None,
            };
            let err = PlaidAuthType::try_from(&config).unwrap_err();
            assert!(
                matches!(
                    err.current_context(),
                    IntegrationError::FailedToObtainAuthType { .. }
                ),
                "unexpected error: {err:?}"
            );
        }

        #[test]
        fn test_numbers_missing_keys_default_to_empty_vecs() {
            // Plaid omits number-type keys that have no entries; #[serde(default)] handles this.
            let json = r#"{ "ach": [] }"#;
            let nums: PlaidNumbers = serde_json::from_str(json).expect("deserialize");
            assert!(nums.bacs.is_empty());
            assert!(nums.international.is_empty());
            assert!(nums.eft.is_empty());
        }

        #[test]
        fn test_numbers_entirely_absent_defaults_to_all_empty() {
            let json = r#"{}"#;
            let nums: PlaidNumbers = serde_json::from_str(json).expect("deserialize");
            assert!(nums.ach.is_empty() && nums.bacs.is_empty() && nums.international.is_empty());
        }

        #[test]
        fn test_link_token_response_valid_rfc3339_round_trips() {
            let json = r#"{"link_token":"link-sandbox-abc","expiration":"2099-01-01T00:00:00Z","request_id":"req1"}"#;
            let res: PlaidLinkTokenResponse = serde_json::from_str(json).unwrap();
            assert_eq!(res.expiration.as_deref(), Some("2099-01-01T00:00:00Z"));
        }

        #[test]
        fn test_link_token_response_unparsable_expiration_does_not_fail() {
            let json = r#"{"link_token":"link-sandbox-xyz","expiration":"not-a-date","request_id":"req2"}"#;
            let res: PlaidLinkTokenResponse = serde_json::from_str(json).unwrap();
            assert_eq!(res.expiration.as_deref(), Some("not-a-date"));
        }

        #[test]
        fn test_link_token_response_absent_expiration_is_none() {
            let json = r#"{"link_token":"link-sandbox-xyz","request_id":"req3"}"#;
            let res: PlaidLinkTokenResponse = serde_json::from_str(json).unwrap();
            assert!(res.expiration.is_none());
        }
    }
}
