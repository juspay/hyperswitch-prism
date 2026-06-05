#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::marker::PhantomData;

    use common_enums::Currency;
    use common_utils::types::MinorUnit;
    use domain_types::{
        connector_flow::PaymentMethodToken,
        connector_types::{
            PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        },
        payment_method_data::{DefaultPCIHolder, PaymentMethodData},
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        types::Connectors,
    };

    use crate::{connectors::hipay::transformers::HipayTokenResponse, types::ResponseRouterData};

    fn default_payment_flow_data() -> PaymentFlowData {
        PaymentFlowData {
            merchant_id: common_utils::id_type::MerchantId::default(),
            customer_id: None,
            connector_customer: None,
            payment_id: "pay_test".to_string(),
            attempt_id: "attempt_test".to_string(),
            status: common_enums::AttemptStatus::Pending,
            payment_method: common_enums::PaymentMethod::Card,
            description: None,
            return_url: None,
            address: Default::default(),
            auth_type: common_enums::AuthenticationType::NoThreeDs,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: None,
            preprocessing_id: None,
            connector_api_version: None,
            connector_request_reference_id: "ref_test".to_string(),
            test_mode: None,
            connector_http_status_code: None,
            connector_response_headers: None,
            external_latency: None,
            connectors: Connectors::default(),
            raw_connector_response: None,
            raw_connector_request: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
            merchant_request_id: None,
            sender_payment_instrument_id: None,
        }
    }

    #[test]
    fn test_parity_15803_connector_response() {
        let raw = include_str!("parity_fixtures/15803.json");
        let response: HipayTokenResponse = serde_json::from_str(raw).unwrap();

        assert_eq!(response.brand, "VISA");

        let router_data: RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<DefaultPCIHolder>,
            PaymentMethodTokenResponse,
        > = RouterDataV2 {
            flow: PhantomData,
            resource_common_data: default_payment_flow_data(),
            connector_config: ConnectorSpecificConfig::NoKey,
            request: PaymentMethodTokenizationData {
                payment_method_data: PaymentMethodData::MandatePayment,
                browser_info: None,
                currency: Currency::EUR,
                amount: MinorUnit::new(6000),
                capture_method: None,
                customer_acceptance: None,
                setup_future_usage: None,
                setup_mandate_details: None,
                mandate_id: None,
                integrity_object: None,
                split_payments: None,
                connector_feature_data: None,
            },
            response: Err(ErrorResponse::default()),
        };

        let response_router_data = ResponseRouterData {
            response,
            router_data,
            http_code: 201,
        };

        let result: RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<DefaultPCIHolder>,
            PaymentMethodTokenResponse,
        > = response_router_data.try_into().unwrap();

        let token_response = result.response.unwrap();
        assert_eq!(
            token_response.token,
            "8d97bfb7cf0141e09f56fd7337dca4131c3b550d3e367d5752e7fba54ef69cc4"
        );

        let connector_response = result
            .resource_common_data
            .connector_response
            .expect("connector_response should be Some");
        let additional_data = connector_response
            .additional_payment_method_data
            .expect("additional_payment_method_data should be Some");

        match additional_data {
            domain_types::router_data::AdditionalPaymentMethodConnectorResponse::Card {
                card_network,
                domestic_network,
                authentication_data,
                payment_checks,
                auth_code,
            } => {
                assert_eq!(card_network, Some("VISA".to_string()));
                assert_eq!(domestic_network, None);
                assert_eq!(authentication_data, None);
                assert_eq!(payment_checks, None);
                assert_eq!(auth_code, None);
            }
            _ => panic!("Expected Card variant"),
        }
    }
}
