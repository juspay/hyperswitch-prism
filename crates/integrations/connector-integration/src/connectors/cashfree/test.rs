// Test file placeholder for Cashfree connector
// Tests will be implemented once the basic connector is working

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use domain_types::payment_method_data::DefaultPCIHolder;
    use interfaces::api::ConnectorCommon;

    use crate::connectors;

    #[test]
    fn test_cashfree_connector_creation() {
        // Basic test to ensure connector can be created
        let connector: &connectors::cashfree::Cashfree<DefaultPCIHolder> =
            super::super::Cashfree::new();
        assert_eq!(connector.id(), "cashfree");
    }

    /// The customer email must still reach Cashfree verbatim, while the
    /// structured logs (which serialize the typed struct via
    /// `masked_serialize`) must never carry the address.
    #[test]
    fn test_customer_email_masked_in_logs_but_verbatim_on_the_wire() {
        use hyperswitch_masking::Secret;

        use crate::connectors::cashfree::transformers::CashfreeCustomerDetails;

        let details = CashfreeCustomerDetails {
            customer_id: "cust_1".to_string(),
            customer_email: Some(Secret::new("jane.doe@example.com".to_string())),
            customer_phone: Secret::new("9999999999".to_string()),
            customer_name: Some("Jane Doe".to_string()),
        };

        let wire = serde_json::to_string(&details).expect("wire serialization");
        assert!(
            wire.contains("jane.doe@example.com"),
            "connector payload must be unchanged, got: {wire}"
        );

        let masked = hyperswitch_masking::masked_serialize(&details)
            .expect("masked serialization")
            .to_string();
        assert!(
            !masked.contains("jane.doe@example.com"),
            "email leaked into the log view: {masked}"
        );
        assert!(
            masked.contains("@example.com"),
            "EmailStrategy should retain the domain, got: {masked}"
        );
    }

    /// CreateOrder now carries the caller's `customer.id` into
    /// `PaymentFlowData.customer_id`, but Cashfree's order request keeps sending
    /// the value it always sent, `"guest"` (Cashfree accepts only alphanumeric ids
    /// of 3 to 50 characters, which `cus_...` ids fail).
    #[test]
    #[allow(clippy::panic, clippy::indexing_slicing)]
    fn create_order_keeps_sending_guest_as_the_customer_id() {
        use std::marker::PhantomData;

        use common_utils::{metadata::MaskedMetadata, request::RequestContent};
        use domain_types::{
            connector_flow::CreateOrder,
            connector_types::{
                PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData,
            },
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::Connectors,
            utils::ForeignTryFrom,
        };
        use grpc_api_types::payments as grpc;
        use hyperswitch_masking::Secret;
        use interfaces::connector_integration_v2::ConnectorIntegrationV2;

        for customer_id in [Some("cus_4f9a_0001"), Some("cust123"), None] {
            let proto = grpc::PaymentServiceCreateOrderRequest {
                merchant_order_id: Some("order_cashfree_0001".to_string()),
                amount: Some(grpc::Money {
                    minor_amount: 1000,
                    currency: grpc::Currency::from_str_name("INR").expect("INR").into(),
                }),
                customer: customer_id.map(|id| grpc::Customer {
                    id: Some(id.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let request =
                PaymentCreateOrderData::foreign_try_from(proto.clone()).expect("order data");
            let common = PaymentFlowData::foreign_try_from((
                proto,
                Connectors::default(),
                &MaskedMetadata::default(),
            ))
            .expect("flow data");
            assert_eq!(
                common.customer_id.as_ref().map(|id| id.get_string_repr()),
                customer_id
            );

            let router_data: RouterDataV2<
                CreateOrder,
                PaymentFlowData,
                PaymentCreateOrderData,
                PaymentCreateOrderResponse,
            > = RouterDataV2 {
                flow: PhantomData,
                resource_common_data: common,
                connector_config: ConnectorSpecificConfig::Cashfree {
                    app_id: Secret::new("dummy_app_id".to_string()),
                    secret_key: Secret::new("dummy_secret_key".to_string()),
                    base_url: None,
                },
                request,
                response: Err(ErrorResponse::default()),
            };

            let connector: &connectors::cashfree::Cashfree<DefaultPCIHolder> =
                super::super::Cashfree::new();
            let content = connector
                .get_request_body(&router_data)
                .expect("request body")
                .expect("some request body")
                .content;
            let body = match content {
                RequestContent::Json(payload) => serde_json::to_value(&payload).expect("json"),
                _ => panic!("expected a JSON body"),
            };
            assert_eq!(body["customer_details"]["customer_id"], "guest");
        }
    }
}
