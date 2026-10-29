// Tests exercise the public request-building interfaces of the Rapyd
// connector. They assert the exact wire translation of UCS inputs:
// external 3DS results, hosted 3DS flags, mandate setup, and saved-token
// repeat charges. Responses and live provider acceptance are covered
// separately.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::print_stdout)]
mod tests {
    use std::{marker::PhantomData, str::FromStr};

    use common_utils::{
        pii::Email,
        request::RequestContent,
        types::{MinorUnit, SemanticVersion},
    };
    use domain_types::{
        connector_flow::{Authorize, RepeatPayment, SetupMandate},
        connector_types::{
            ConnectorEnum, ConnectorMandateReferenceId, MandateReferenceId, PaymentFlowData,
            PaymentsAuthorizeData, PaymentsResponseData, RepeatPaymentData,
            SetupMandateRequestData,
        },
        errors::IntegrationError,
        payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData, RawCardNumber},
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        router_request_types::AuthenticationData,
        types::{ConnectorParams, Connectors},
    };
    use hyperswitch_masking::Secret;
    use interfaces::{
        connector_integration_v2::BoxedConnectorIntegrationV2, connector_types::BoxedConnector,
    };
    use serde_json::{json, Value};

    use crate::{connectors::Rapyd, types::ConnectorData};

    fn connectors_fixture() -> Connectors {
        Connectors {
            rapyd: ConnectorParams {
                base_url: "https://sandboxapi.rapyd.net/".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn connector_data() -> ConnectorData<DefaultPCIHolder> {
        let connector: BoxedConnector<DefaultPCIHolder> = Box::new(Rapyd::new());
        ConnectorData {
            connector,
            connector_name: ConnectorEnum::Rapyd,
        }
    }

    fn payment_flow_data(auth_type: common_enums::AuthenticationType) -> PaymentFlowData {
        PaymentFlowData {
            raw_connector_status: None,
            merchant_id: common_utils::id_type::MerchantId::default(),
            customer_id: None,
            connector_customer: None,
            payment_id: "pay_abcdef123456".to_string(),
            attempt_id: "attempt_123456abcdef".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: common_enums::PaymentMethod::Card,
            payment_method_type: Some(common_enums::PaymentMethodType::Card),
            description: Some("Payment for order #12345".to_string()),
            return_url: Some("https://www.example.com/return".to_string()),
            order_details: None,
            address: domain_types::payment_address::PaymentAddress::new(None, None, None, None),
            auth_type,
            connector_feature_data: None,
            amount_captured: None,
            amount_capturable: None,
            amount_authorized: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            connector_request_reference_id: "conn_ref_123456789".to_string(),
            test_mode: None,
            connector_http_status_code: None,
            connectors: connectors_fixture().into(),
            external_latency: None,
            connector_response_headers: None,
            raw_connector_response: None,
            typed_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            l2_l3_data: None,
            merchant_request_id: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
            settlement_status: None,
        }
    }

    fn rapyd_connector_config() -> ConnectorSpecificConfig {
        ConnectorSpecificConfig::Rapyd {
            access_key: Secret::new("test_rapyd_access_key".to_string()),
            secret_key: Secret::new("test_rapyd_secret_key".to_string()),
            base_url: None,
        }
    }

    /// Completed external 3DS authentication result. `message_version`
    /// and `ds_trans_id` are passed explicitly so individual tests can
    /// omit the mandatory fields.
    fn external_auth_data(
        message_version: Option<SemanticVersion>,
        ds_trans_id: Option<String>,
    ) -> AuthenticationData {
        AuthenticationData {
            trans_status: None,
            eci: Some("05".to_string()),
            cavv: Some(Secret::new("BwABBYcyOCdlg0QChDI4EHUURgE".to_string())),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: None,
            message_version,
            ds_trans_id,
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
        }
    }

    fn full_external_auth() -> AuthenticationData {
        external_auth_data(
            Some(SemanticVersion::new(2, 2, 0)),
            Some("1b20b229-8676-5812-8000-000000617fbb".to_string()),
        )
    }

    fn card(
        card_network: Option<common_enums::CardNetwork>,
        card_number: &str,
    ) -> PaymentMethodData<DefaultPCIHolder> {
        PaymentMethodData::Card(Card {
            card_number: RawCardNumber(cards::CardNumber::from_str(card_number).unwrap()),
            card_cvc: Secret::new("100".to_string()),
            card_exp_month: Secret::new("03".to_string()),
            card_exp_year: Secret::new("2030".to_string()),
            card_network,
            card_holder_name: Some(Secret::new("John Doe".to_string())),
            ..Default::default()
        })
    }

    fn authorize_request(
        auth_type: common_enums::AuthenticationType,
        payment_method_data: PaymentMethodData<DefaultPCIHolder>,
        authentication_data: Option<AuthenticationData>,
    ) -> RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: PhantomData::<Authorize>,
            resource_common_data: payment_flow_data(auth_type),
            connector_config: rapyd_connector_config(),
            request: PaymentsAuthorizeData {
                risk_assessment_data: None,
                split_settlement: None,
                customer_document_details: None,
                customer_date_of_birth: None,
                payment_channel: None,
                authentication_data,
                connector_testing_data: None,
                currency_conversion_data: None,
                payment_method_data,
                amount: common_utils::types::Money {
                    amount: MinorUnit::new(1000),
                    currency: common_enums::Currency::USD,
                },
                order_tax_amount: None,
                surcharge_amount: None,
                email: Some(Email::try_from("test@example.com".to_string()).unwrap()),
                customer_name: None,
                currency: common_enums::Currency::USD,
                confirm: true,
                capture_method: None,
                integrity_object: None,
                router_return_url: Some("https://www.example.com/return".to_string()),
                webhook_url: None,
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage: None,
                off_session: None,
                browser_info: None,
                order_category: None,
                session_token: None,
                enrolled_for_3ds: None,
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
            response: Err(ErrorResponse::default()),
        }
    }

    fn masked_json(body: &RequestContent) -> Value {
        match body {
            RequestContent::Json(value)
            | RequestContent::FormUrlEncoded(value)
            | RequestContent::Xml(value) => value.masked_serialize().unwrap_or(json!({
                "error": "failed to mask serialize connector request"
            })),
            RequestContent::FormData(_) => json!({ "request_type": "FORM_DATA" }),
            RequestContent::RawBytes(_) => json!({ "request_type": "RAW_BYTES" }),
        }
    }

    fn assert_missing_field(
        result: error_stack::Result<Option<common_utils::request::Request>, IntegrationError>,
        field_name: &'static str,
    ) {
        let report = result.unwrap_err();
        let error = report.current_context();
        assert!(
            matches!(
                error,
                IntegrationError::MissingRequiredField {
                    field_name: name,
                    ..
                } if *name == field_name
            ),
            "expected MissingRequiredField({field_name}), got {error:?}"
        );
    }

    #[test]
    fn test_build_request_external_3ds_mastercard_complete() {
        let req = authorize_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(
                Some(common_enums::CardNetwork::Mastercard),
                "5123456789012346",
            ),
            Some(full_external_auth()),
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        let options = &body["payment_method_options"];
        // External results must replace the hosted `3d_required` flag.
        assert!(options.get("3d_required").is_none());
        assert_eq!(options["3d_version"], "2.2.0");
        assert_eq!(options["eci"], "05");
        assert_eq!(
            options["ds_trans_id"],
            "1b20b229-8676-5812-8000-000000617fbb"
        );
        // cavv is forwarded but masked in typed request diagnostics.
        assert!(options.get("cavv").is_some());
    }

    #[test]
    fn test_build_request_external_3ds_mastercard_2_0_missing_ds_trans_id() {
        let req = authorize_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(
                Some(common_enums::CardNetwork::Mastercard),
                "5123456789012346",
            ),
            Some(external_auth_data(
                Some(SemanticVersion::new(2, 2, 0)),
                None,
            )),
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let result = connector_integration.build_request_v2(&req);
        assert_missing_field(result, "authentication_data.ds_trans_id");
    }

    #[test]
    fn test_build_request_external_3ds_missing_message_version() {
        let req = authorize_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(
                Some(common_enums::CardNetwork::Mastercard),
                "5123456789012346",
            ),
            Some(external_auth_data(
                None,
                Some("1b20b229-8676-5812-8000-000000617fbb".to_string()),
            )),
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let result = connector_integration.build_request_v2(&req);
        assert_missing_field(result, "authentication_data.message_version");
    }

    #[test]
    fn test_build_request_external_3ds_visa_omits_ds_trans_id() {
        let req = authorize_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(Some(common_enums::CardNetwork::Visa), "4111111111111111"),
            Some(external_auth_data(
                Some(SemanticVersion::new(2, 2, 0)),
                None,
            )),
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        let options = &body["payment_method_options"];
        assert!(options.get("3d_required").is_none());
        assert!(options.get("ds_trans_id").is_none());
        assert_eq!(options["3d_version"], "2.2.0");
        assert_eq!(options["eci"], "05");
    }

    #[test]
    fn test_build_request_hosted_3ds_true() {
        let req = authorize_request(
            common_enums::AuthenticationType::ThreeDs,
            card(
                Some(common_enums::CardNetwork::Mastercard),
                "5123456789012346",
            ),
            None,
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        let options = &body["payment_method_options"];
        assert_eq!(options["3d_required"], true);
        // Hosted requests must not carry external authentication fields.
        assert!(options.get("3d_version").is_none());
        assert!(options.get("cavv").is_none());
        assert!(options.get("eci").is_none());
        assert!(options.get("ds_trans_id").is_none());
    }

    #[test]
    fn test_build_request_hosted_3ds_none() {
        let req = authorize_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(None, "4111111111111111"),
            None,
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        assert_eq!(body["payment_method_options"]["3d_required"], false);
    }

    fn setup_mandate_request(
        auth_type: common_enums::AuthenticationType,
        payment_method_data: PaymentMethodData<DefaultPCIHolder>,
        authentication_data: Option<AuthenticationData>,
    ) -> RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: PhantomData::<SetupMandate>,
            resource_common_data: payment_flow_data(auth_type),
            connector_config: rapyd_connector_config(),
            request: SetupMandateRequestData {
                currency: common_enums::Currency::USD,
                payment_method_data,
                confirm: true,
                billing_descriptor: None,
                customer_acceptance: None,
                mandate_id: None,
                setup_future_usage: Some(common_enums::FutureUsage::OffSession),
                off_session: None,
                setup_mandate_details: None,
                router_return_url: Some("https://www.example.com/return".to_string()),
                webhook_url: None,
                browser_info: None,
                email: Some(Email::try_from("test@example.com".to_string()).unwrap()),
                customer_document_details: None,
                customer_name: Some("John Doe".to_string()),
                return_url: None,
                payment_method_type: Some(common_enums::PaymentMethodType::Card),
                request_incremental_authorization: false,
                metadata: None,
                complete_authorize_url: None,
                capture_method: None,
                merchant_order_id: None,
                amount: Some(common_utils::types::Money {
                    amount: MinorUnit::new(100),
                    currency: common_enums::Currency::USD,
                }),
                shipping_cost: None,
                customer_id: None,
                integrity_object: None,
                payment_channel: None,
                enable_partial_authorization: None,
                locale: None,
                connector_testing_data: None,
                mit_category: None,
                split_payments: None,
                authentication_data,
                partner_merchant_identifier_details: None,
                is_account_funding_transaction: None,
                recipient_details: None,
                additional_connector_details: None,
                customer: None,
                order_category: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn test_setup_mandate_external_3ds() {
        let req = setup_mandate_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(
                Some(common_enums::CardNetwork::Mastercard),
                "5123456789012346",
            ),
            Some(full_external_auth()),
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        let options = &body["payment_method_options"];
        assert!(options.get("3d_required").is_none());
        assert_eq!(options["3d_version"], "2.2.0");
        assert_eq!(options["eci"], "05");
        assert_eq!(
            options["ds_trans_id"],
            "1b20b229-8676-5812-8000-000000617fbb"
        );
        assert!(options.get("cavv").is_some());
        // Mandate setup always saves the card under an inline customer.
        // Email is PII-masked in masked serialization; the masked value
        // still proves the address was carried into the customer object.
        assert_eq!(body["save_payment_method"], true);
        assert_eq!(body["customer"]["email"], "****@example.com");
        assert_eq!(body["capture"], false);
    }

    #[test]
    fn test_setup_mandate_hosted_no_3ds() {
        let req = setup_mandate_request(
            common_enums::AuthenticationType::NoThreeDs,
            card(None, "4111111111111111"),
            None,
        );
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        assert_eq!(body["payment_method_options"]["3d_required"], false);
        assert_eq!(body["save_payment_method"], true);
    }

    fn repeat_payment_request(
        auth_type: common_enums::AuthenticationType,
    ) -> RouterDataV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: PhantomData::<RepeatPayment>,
            resource_common_data: payment_flow_data(auth_type),
            connector_config: rapyd_connector_config(),
            request: RepeatPaymentData {
                mandate_reference: MandateReferenceId::ConnectorMandateId(
                    ConnectorMandateReferenceId::new(
                        Some("card_saved_token".to_string()),
                        None,
                        None,
                        None,
                        None,
                    ),
                ),
                amount: common_utils::types::Money {
                    amount: MinorUnit::new(500),
                    currency: common_enums::Currency::USD,
                },
                currency: common_enums::Currency::USD,
                merchant_order_id: None,
                metadata: None,
                webhook_url: None,
                integrity_object: None,
                integrity_specifications: None,
                capture_method: None,
                browser_info: None,
                email: None,
                customer_document_details: None,
                payment_method_type: Some(common_enums::PaymentMethodType::Card),
                connector_feature_data: None,
                off_session: Some(true),
                router_return_url: Some("https://www.example.com/return".to_string()),
                complete_authorize_url: None,
                split_payments: None,
                split_settlement: None,
                recurring_mandate_payment_data: None,
                shipping_cost: None,
                payment_channel: None,
                mit_category: Some(common_enums::MitCategory::Recurring),
                enable_partial_authorization: None,
                billing_descriptor: None,
                payment_method_data: card(None, "4111111111111111"),
                authentication_data: None,
                locale: None,
                connector_testing_data: None,
                merchant_account_id: None,
                merchant_configured_currency: None,
                additional_payment_data: None,
                partner_merchant_identifier_details: None,
                is_account_funding_transaction: None,
                recipient_details: None,
                additional_connector_details: None,
                customer: None,
                currency_conversion_data: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn test_repeat_payment_hosted_no_3ds() {
        let req = repeat_payment_request(common_enums::AuthenticationType::NoThreeDs);
        let connector_data = connector_data();
        let connector_integration: BoxedConnectorIntegrationV2<
            '_,
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<DefaultPCIHolder>,
            PaymentsResponseData,
        > = connector_data.connector.get_connector_integration_v2();
        let request = connector_integration.build_request_v2(&req).unwrap();
        let body = request
            .as_ref()
            .and_then(|r| r.body.as_ref())
            .map(masked_json)
            .expect("request body should be JSON");
        assert_eq!(body["payment_method_options"]["3d_required"], false);
        assert_eq!(body["initiation_type"], "recurring");
        // The saved token is sent as the payment method string. It is
        // masked, so only assert that it is a plain string.
        assert!(body["payment_method"].is_string());
        assert!(body.get("customer").is_none());
        assert!(body.get("save_payment_method").is_none());
    }
}
