#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    pub mod base_url {
        use hyperswitch_masking::Secret;

        use crate::payout_connectors::checkout::get_checkout_payout_base_url;

        const SANDBOX_TEMPLATE: &str = "https://{{optional_client_id}}api.sandbox.checkout.com";

        #[test]
        fn client_id_contributes_account_prefix_subdomain() {
            let url = get_checkout_payout_base_url(
                SANDBOX_TEMPLATE,
                Some(&Secret::new(
                    "cli_x46dhqyzrrrhjrbnl5hyexcluderequest".to_string(),
                )),
            )
            .unwrap();
            assert_eq!(url, "https://x46dhqyz.api.sandbox.checkout.com");
        }

        #[test]
        fn absent_client_id_drops_optional_subdomain() {
            let url = get_checkout_payout_base_url(SANDBOX_TEMPLATE, None).unwrap();
            assert_eq!(url, "https://api.sandbox.checkout.com");
        }

        #[test]
        fn override_without_placeholder_is_used_literally() {
            // Same leniency as payment URL resolution: a plain override must not fail.
            let url = get_checkout_payout_base_url("http://localhost:8080", None).unwrap();
            assert_eq!(url, "http://localhost:8080");
        }

        #[test]
        fn override_without_placeholder_trailing_slash_is_trimmed() {
            let url = get_checkout_payout_base_url("https://payouts.example.com/", None).unwrap();
            assert_eq!(url, "https://payouts.example.com");
        }

        #[test]
        fn malformed_client_id_is_rejected() {
            // Missing the `cli_` prefix.
            assert!(get_checkout_payout_base_url(
                SANDBOX_TEMPLATE,
                Some(&Secret::new("x46dhqyzrrrh".to_string())),
            )
            .is_err());
            // Shorter than the 8-character account prefix.
            assert!(get_checkout_payout_base_url(
                SANDBOX_TEMPLATE,
                Some(&Secret::new("cli_x46".to_string())),
            )
            .is_err());
            // Non-alphanumeric account prefix.
            assert!(get_checkout_payout_base_url(
                SANDBOX_TEMPLATE,
                Some(&Secret::new("cli_x46-hqyzrrrh".to_string())),
            )
            .is_err());
        }
    }

    pub mod sender_reference {
        use hyperswitch_masking::Secret;

        use crate::payout_connectors::checkout::transformers::CheckoutPayoutReference;

        #[test]
        fn absent_reference_id_is_a_missing_field_error() {
            assert!(CheckoutPayoutReference::try_from_reference_id(None).is_err());
        }

        #[test]
        fn valid_reference_id_is_accepted() {
            assert!(
                CheckoutPayoutReference::try_from_reference_id(Some(&Secret::new(
                    "sender0001".to_string(),
                )))
                .is_ok()
            );
        }

        #[test]
        fn reference_id_longer_than_15_characters_is_rejected() {
            // Checkout caps `sender.reference` at 15 characters; the typical Hyperswitch
            // customer ID is 24 characters, so this must fail loudly.
            assert!(
                CheckoutPayoutReference::try_from_reference_id(Some(&Secret::new(
                    "cus_ABCDEFGHIJKLMNOP1234".to_string(),
                )))
                .is_err()
            );
            assert!(
                CheckoutPayoutReference::try_from_reference_id(Some(&Secret::new(
                    "123456789012345".to_string(),
                )))
                .is_ok()
            );
        }
    }

    pub mod status_mapping {
        use common_enums::PayoutStatus;

        use crate::payout_connectors::checkout::transformers::CheckoutPayoutStatus;

        fn map(wire_status: &str) -> PayoutStatus {
            let status: CheckoutPayoutStatus =
                serde_json::from_value(serde_json::Value::String(wire_status.to_string()))
                    .expect("wire status must deserialize");
            PayoutStatus::from(&status)
        }

        #[test]
        fn in_flight_statuses() {
            assert_eq!(map("Accepted"), PayoutStatus::Initiated);
            assert_eq!(map("Pending"), PayoutStatus::Pending);
        }

        #[test]
        fn terminal_success_statuses() {
            // `Approved` is terminal success for bank payouts, `Paid` for card payouts.
            assert_eq!(map("Approved"), PayoutStatus::Success);
            assert_eq!(map("Paid"), PayoutStatus::Success);
        }

        #[test]
        fn terminal_failure_statuses() {
            assert_eq!(map("Rejected"), PayoutStatus::Failure);
            assert_eq!(map("Declined"), PayoutStatus::Failure);
            // Failure states shared with the payment connector on GET /payments/{id}.
            assert_eq!(map("Canceled"), PayoutStatus::Failure);
            assert_eq!(map("Voided"), PayoutStatus::Failure);
            assert_eq!(map("Expired"), PayoutStatus::Failure);
        }

        #[test]
        fn shared_in_flight_statuses() {
            assert_eq!(map("Retry Scheduled"), PayoutStatus::Pending);
            assert_eq!(map("Authorized"), PayoutStatus::Pending);
            assert_eq!(map("Captured"), PayoutStatus::Pending);
            assert_eq!(map("Partially Captured"), PayoutStatus::Pending);
            assert_eq!(map("Card Verified"), PayoutStatus::Pending);
        }

        #[test]
        fn refund_like_statuses_map_to_reversed() {
            assert_eq!(map("Refunded"), PayoutStatus::Reversed);
            assert_eq!(map("Partially Refunded"), PayoutStatus::Reversed);
        }

        #[test]
        fn unknown_wire_status_deserializes_as_pending() {
            // A status checkout.com adds later must not fail deserialization on a transfer
            // the connector already holds.
            assert_eq!(map("Queued For Tomorrow"), PayoutStatus::Pending);
        }

        #[test]
        fn reversed_status() {
            assert_eq!(map("Returned"), PayoutStatus::Reversed);
        }

        #[test]
        fn lowercase_aliases_deserialize() {
            assert_eq!(map("pending"), PayoutStatus::Pending);
            assert_eq!(map("paid"), PayoutStatus::Success);
        }
    }

    pub mod payout_transfer {
        use std::{marker::PhantomData, str::FromStr};

        use cards::CardNumber;
        use common_enums::{BankHolderType, CountryAlpha2, Currency};
        use common_utils::{request::RequestContent, types::MinorUnit};
        use domain_types::{
            connector_flow::PayoutTransfer,
            payment_address::{Address, AddressDetails},
            payment_method_data::DefaultPCIHolder,
            payouts::{
                payout_method_data::{ApplePayDecrypt, Passthrough, PayoutMethodData, Wallet},
                payouts_types::{
                    AdditionalPayoutConnectorDetails, CheckoutAdditionalPayoutInformation,
                    CheckoutPayoutFundsTransferType, PayoutAddress, PayoutFlowData,
                    PayoutSourceOfFunds, PayoutTransferRequest, PayoutTransferResponse,
                    PayoutVendorAccountDetails, PayoutVendorDetails,
                },
            },
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };
        use hyperswitch_masking::Secret;
        use interfaces::connector_integration_v2::{
            BoxedConnectorIntegrationV2, ConnectorIntegrationAnyV2,
        };
        use serde_json::json;

        use crate::payout_connectors::CheckoutPayouts;

        const CHECKOUT_TEMPLATE_URL: &str =
            "https://{{optional_client_id}}api.sandbox.checkout.com";
        const CONNECTOR_REFERENCE: &str = "payout_btoF7KDgiTKWKEpCRHff";

        fn billing_address() -> AddressDetails {
            AddressDetails {
                first_name: Some(Secret::new("Hannah".to_string())),
                last_name: Some(Secret::new("Bret".to_string())),
                line1: Some(Secret::new("123 High Street".to_string())),
                city: Some(Secret::new("London".to_string())),
                zip: Some(Secret::new("SW1A 1AA".to_string())),
                country: Some(CountryAlpha2::GB),
                state: None,
                ..Default::default()
            }
        }

        fn corporate_sender() -> PayoutVendorAccountDetails {
            PayoutVendorAccountDetails {
                vendor_details: Some(PayoutVendorDetails {
                    vendor_type: Some(BankHolderType::Business),
                    vendor_name: Some(Secret::new("Example Corp".to_string())),
                    address: Some(Address {
                        address: Some(AddressDetails {
                            line1: Some(Secret::new("20 Example Road".to_string())),
                            city: Some(Secret::new("London".to_string())),
                            zip: Some(Secret::new("SW1A 2AA".to_string())),
                            country: Some(CountryAlpha2::GB),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    merchant_sender_reference_id: Some(Secret::new("sender0001".to_string())),
                    source_of_funds: Some(PayoutSourceOfFunds::Debit),
                    ..Default::default()
                }),
                individual_details: None,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn router_data(
            connector_request_reference_id: String,
            payout_method_data: Option<PayoutMethodData<DefaultPCIHolder>>,
            vendor_account_details: Option<PayoutVendorAccountDetails>,
            client_id: Option<Secret<String>>,
        ) -> RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest<DefaultPCIHolder>,
            PayoutTransferResponse,
        > {
            RouterDataV2 {
                flow: PhantomData::<PayoutTransfer>,
                resource_common_data: PayoutFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    payout_id: CONNECTOR_REFERENCE.to_string(),
                    connectors: Connectors {
                        checkout: ConnectorParams {
                            base_url: CHECKOUT_TEMPLATE_URL.to_string(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                    .into(),
                    connector_request_reference_id,
                    raw_connector_response: None,
                    typed_connector_response: None,
                    connector_response_headers: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    access_token: None,
                    test_mode: None,
                    description: None,
                    merchant_request_id: None,
                    vault_headers: None,
                },
                connector_config: ConnectorSpecificConfig::Checkout {
                    api_key: Secret::new("pk_test_publickey".to_string()),
                    api_secret: Secret::new("sk_test_secret".to_string()),
                    processing_channel_id: Secret::new("pc_test_channel".to_string()),
                    client_id,
                    currency_account_id: Some(Secret::new("ca_testcurrencyaccount".to_string())),
                    base_url: None,
                },
                request: PayoutTransferRequest {
                    merchant_payout_id: Some(CONNECTOR_REFERENCE.to_string()),
                    connector_quote_id: None,
                    connector_payout_id: None,
                    amount: MinorUnit::new(200),
                    source_currency: Currency::USD,
                    destination_currency: Currency::USD,
                    priority: None,
                    connector_payout_method_id: None,
                    webhook_url: None,
                    payout_method_data,
                    address: Some(PayoutAddress {
                        billing_address: Some(Address {
                            address: Some(billing_address()),
                            ..Default::default()
                        }),
                        shipping_address: None,
                    }),
                    source_bank_data: None,
                    customer: None,
                    connector_eligibility_reference_id: None,
                    payout_connector_metadata: None,
                    billing_descriptor: None,
                    additional_payout_connector_details: Some(
                        AdditionalPayoutConnectorDetails::Checkout(
                            CheckoutAdditionalPayoutInformation {
                                funds_transfer_type: Some(
                                    CheckoutPayoutFundsTransferType::AccountToAccount,
                                ),
                            },
                        ),
                    ),
                    vendor_account_details,
                },
                response: Err(ErrorResponse::default()),
            }
        }

        fn build_request_body(
            req: &RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            >,
        ) -> (common_utils::request::Request, serde_json::Value) {
            let connector = CheckoutPayouts::<DefaultPCIHolder>::new();
            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            > = connector.get_connector_integration_v2();
            let request = connector_integration
                .build_request_v2(req)
                .expect("request building must succeed")
                .expect("PayoutTransfer must produce a request");
            let body = match request.body.as_ref() {
                Some(
                    RequestContent::Json(body)
                    | RequestContent::FormUrlEncoded(body)
                    | RequestContent::Xml(body),
                ) => body
                    .masked_serialize()
                    .unwrap_or(json!({ "error": "failed to mask serialize connector request" })),
                Some(RequestContent::FormData(_)) => json!({"request_type": "FORM_DATA"}),
                Some(RequestContent::RawBytes(_)) => json!({"request_type": "RAW_BYTES"}),
                None => serde_json::Value::Null,
            };
            (request, body)
        }

        fn apple_pay_token() -> PayoutMethodData<DefaultPCIHolder> {
            PayoutMethodData::Wallet(Wallet::ApplePayDecrypt(ApplePayDecrypt {
                dpan: CardNumber::from_str("4242424242424242").unwrap(),
                expiry_month: Secret::new("07".to_string()),
                expiry_year: Secret::new("2027".to_string()),
                card_holder_name: Some(Secret::new("Hannah Bret".to_string())),
                card_network: None,
            }))
        }

        #[test]
        fn serialized_apple_pay_network_token_request() {
            let req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(apple_pay_token()),
                Some(corporate_sender()),
                None,
            );
            let (request, body) = build_request_body(&req);

            assert_eq!(request.url, "https://api.sandbox.checkout.com/payments");
            assert_eq!(request.method, common_utils::request::Method::Post);
            let idempotency_key = request
                .headers
                .iter()
                .find(|(name, _)| name == "Cko-Idempotency-Key")
                .map(|(_, value)| value.clone().into_inner());
            assert_eq!(idempotency_key, Some(CONNECTOR_REFERENCE.to_string()));

            assert_eq!(body["source"]["type"], "currency_account");
            assert_eq!(body["destination"]["type"], "network_token");
            assert_eq!(body["destination"]["token_type"], "applepay");
            assert_eq!(body["destination"]["account_holder"]["type"], "individual");
            assert_eq!(
                body["destination"]["account_holder"]["billing_address"]["country"],
                "GB"
            );
            assert_eq!(body["amount"], 200);
            assert_eq!(body["currency"], "USD");
            assert_eq!(body["reference"], CONNECTOR_REFERENCE);
            assert_eq!(body["instruction"]["funds_transfer_type"], "AA");
            assert_eq!(body["sender"]["type"], "corporate");
            assert_eq!(body["sender"]["reference_type"], "other");
            assert_eq!(body["sender"]["source_of_funds"], "debit");
        }

        #[test]
        fn client_id_resolves_into_request_url() {
            let req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(apple_pay_token()),
                Some(corporate_sender()),
                Some(Secret::new("cli_x46dhqyzrrrhjrbnl5hy".to_string())),
            );
            let (request, _) = build_request_body(&req);
            assert_eq!(
                request.url,
                "https://x46dhqyz.api.sandbox.checkout.com/payments"
            );
        }

        #[test]
        fn serialized_passthrough_destination() {
            let req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(PayoutMethodData::Passthrough(Passthrough {
                    psp_token: Secret::new("src_testtokentokentoken".to_string()),
                    psp_customer_id: None,
                    token_type: common_enums::PaymentMethodType::ApplePay,
                })),
                Some(corporate_sender()),
                None,
            );
            let (_, body) = build_request_body(&req);
            assert_eq!(body["destination"]["type"], "id");
            assert_eq!(body["destination"]["account_holder"]["type"], "individual");
        }

        #[test]
        fn missing_sender_reference_id_fails() {
            let mut sender = corporate_sender();
            sender
                .vendor_details
                .as_mut()
                .expect("sender fixture carries vendor details")
                .merchant_sender_reference_id = None;
            let req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(apple_pay_token()),
                Some(sender),
                None,
            );
            let connector = CheckoutPayouts::<DefaultPCIHolder>::new();
            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            > = connector.get_connector_integration_v2();
            assert!(connector_integration.build_request_v2(&req).is_err());
        }

        #[test]
        fn missing_currency_account_id_fails() {
            let mut req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(apple_pay_token()),
                Some(corporate_sender()),
                None,
            );
            req.connector_config = ConnectorSpecificConfig::Checkout {
                api_key: Secret::new("pk_test_publickey".to_string()),
                api_secret: Secret::new("sk_test_secret".to_string()),
                processing_channel_id: Secret::new("pc_test_channel".to_string()),
                client_id: None,
                currency_account_id: None,
                base_url: None,
            };
            let connector = CheckoutPayouts::<DefaultPCIHolder>::new();
            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            > = connector.get_connector_integration_v2();
            assert!(connector_integration.build_request_v2(&req).is_err());
        }

        #[test]
        fn empty_merchant_payout_id_fails() {
            // The reference ID doubles as the Cko-Idempotency-Key; empty must be rejected
            // before any money-moving request is built.
            let req = router_data(
                String::new(),
                Some(apple_pay_token()),
                Some(corporate_sender()),
                None,
            );
            let connector = CheckoutPayouts::<DefaultPCIHolder>::new();
            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            > = connector.get_connector_integration_v2();
            assert!(connector_integration.build_request_v2(&req).is_err());
        }

        #[test]
        fn missing_vendor_account_details_fails() {
            let req = router_data(
                CONNECTOR_REFERENCE.to_string(),
                Some(apple_pay_token()),
                None,
                None,
            );
            let connector = CheckoutPayouts::<DefaultPCIHolder>::new();
            let connector_integration: BoxedConnectorIntegrationV2<
                '_,
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest<DefaultPCIHolder>,
                PayoutTransferResponse,
            > = connector.get_connector_integration_v2();
            assert!(connector_integration.build_request_v2(&req).is_err());
        }
    }
}
