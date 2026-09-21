#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    pub mod authorize {
        use std::{borrow::Cow, marker::PhantomData};

        use common_utils::types::MinorUnit;
        use domain_types::{
            connector_flow::Authorize,
            connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
            payment_address::{Address, AddressDetails, PaymentAddress},
            payment_method_data::{
                DefaultPCIHolder, PaymentMethodData, PaymentMethodToken, TokenPaymentMethod,
            },
            router_data::{ConnectorSpecificConfig, ErrorResponse},
            router_data_v2::RouterDataV2,
            types::{ConnectorParams, Connectors},
        };
        use hyperswitch_masking::Secret;

        use crate::connectors::{
            stripe::{
                transformers::{PaymentIntentRequest, StripePaymentMethodType},
                StripeRouterData,
            },
            Stripe,
        };

        fn address(line1: &str, city: &str, zip: &str) -> Address {
            Address {
                address: Some(AddressDetails {
                    first_name: Some(Secret::new("John".to_string())),
                    last_name: Some(Secret::new("Doe".to_string())),
                    line1: Some(Secret::new(line1.to_string())),
                    city: Some(Secret::new(city.to_string())),
                    zip: Some(Secret::new(zip.to_string())),
                    country: Some(common_enums::CountryAlpha2::US),
                    ..Default::default()
                }),
                phone: None,
                email: None,
            }
        }

        /// An ordinary (non split-payment) wallet token authorize, the shape the router sends for
        /// Apple Pay and Google Pay once it populates `token_payment_method_type`.
        fn wallet_token_intent(
            wallet: TokenPaymentMethod,
        ) -> PaymentIntentRequest<DefaultPCIHolder> {
            let router_data: RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<DefaultPCIHolder>,
                PaymentsResponseData,
            > = RouterDataV2 {
                flow: PhantomData::<Authorize>,
                resource_common_data: PaymentFlowData {
                    merchant_id: common_utils::id_type::MerchantId::default(),
                    payment_id: "pay_wallet_token".to_string(),
                    attempt_id: "attempt_wallet_token".to_string(),
                    status: common_enums::AttemptStatus::Pending,
                    payment_method: common_enums::PaymentMethod::Wallet,
                    address: PaymentAddress::new(
                        Some(address("9 Shipping Ln", "Shiptown", "99999")),
                        Some(address("123 Main St", "Anytown", "12345")),
                        None,
                        None,
                    ),
                    auth_type: common_enums::AuthenticationType::NoThreeDs,
                    connector_request_reference_id: "conn_ref_wallet_token".to_string(),
                    connectors: Connectors {
                        stripe: ConnectorParams {
                            base_url: "https://api.stripe.com/".to_string(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                    .into(),
                    raw_connector_status: None,
                    vault_headers: None,
                    customer_id: None,
                    connector_customer: None,
                    payment_method_type: None,
                    description: None,
                    return_url: None,
                    order_details: None,
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
                    test_mode: None,
                    connector_http_status_code: None,
                    external_latency: None,
                    connector_response_headers: None,
                    raw_connector_response: None,
                    typed_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                    minor_amount_capturable: None,
                    amount: None,
                    connector_response: None,
                    recurring_mandate_payment_data: None,
                    l2_l3_data: None,
                    merchant_request_id: None,
                    sender_payment_instrument_id: None,
                    connector_returned_payment_method_details: None,
                    settlement_status: None,
                },
                connector_config: ConnectorSpecificConfig::Stripe {
                    api_key: Secret::new("test_stripe_api_key".to_string()),
                    base_url: None,
                },
                request: PaymentsAuthorizeData {
                    payment_method_data: PaymentMethodData::PaymentMethodToken(
                        PaymentMethodToken {
                            token: Secret::new("tok_wallet_123".to_string()),
                            token_payment_method_type: Some(wallet),
                        },
                    ),
                    amount: MinorUnit::new(1000),
                    minor_amount: MinorUnit::new(1000),
                    currency: common_enums::Currency::USD,
                    confirm: true,
                    router_return_url: Some("https://juspay.in/".to_string()),
                    customer_id: Some(
                        common_utils::id_type::CustomerId::try_from(Cow::from(
                            "cus_wallet_token".to_string(),
                        ))
                        .unwrap(),
                    ),
                    // the merchant is NOT on Stripe split payments, so this is not the tokenize flow
                    split_payments: None,
                    split_settlement: None,
                    customer_document_details: None,
                    customer_date_of_birth: None,
                    authentication_data: None,
                    connector_testing_data: None,
                    currency_conversion_data: None,
                    access_token: None,
                    order_tax_amount: None,
                    surcharge_amount: None,
                    email: None,
                    customer_name: None,
                    capture_method: None,
                    integrity_object: None,
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
                    payment_method_type: None,
                    request_incremental_authorization: Some(false),
                    metadata: None,
                    merchant_order_id: None,
                    shipping_cost: None,
                    merchant_account_id: None,
                    merchant_config_currency: None,
                    all_keys_required: None,
                    customer_acceptance: None,
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
                    is_account_funding_transaction: None,
                    recipient_details: None,
                    business_country: None,
                    additional_connector_details: None,
                    customer: None,
                    mit_category: None,
                    payment_channel: None,
                    domain_data: None,
                    partner_merchant_identifier_details: None,
                },
                response: Err(ErrorResponse::default()),
            };

            PaymentIntentRequest::try_from(StripeRouterData {
                connector: Stripe::<DefaultPCIHolder>::new().to_owned(),
                router_data,
            })
            .unwrap()
        }

        /// Hyperswitch gates the address and browser blocks on `is_payment_method_tokenize_flow_required`,
        /// which also requires a Stripe split payment. A wallet token on an ordinary merchant must keep
        /// its billing and shipping blocks, or Stripe loses AVS on every Apple Pay / Google Pay payment.
        #[test]
        fn wallet_token_keeps_billing_and_shipping() {
            for wallet in [TokenPaymentMethod::ApplePay, TokenPaymentMethod::GooglePay] {
                let intent = wallet_token_intent(wallet);
                assert_eq!(
                    intent.billing.zip_code,
                    Some(Secret::new("12345".to_string())),
                    "{wallet:?}: billing must survive the wallet token branch"
                );
                assert_eq!(
                    intent.billing.city,
                    Some(Secret::new("Anytown".to_string())),
                    "{wallet:?}: billing city must survive the wallet token branch"
                );
                assert!(
                    intent.shipping.is_some(),
                    "{wallet:?}: shipping must survive the wallet token branch"
                );
            }
        }

        /// Stripe derives the wallet from the token, so `payment_method_types[0]` follows the wallet:
        /// none for Apple Pay, `card` for Google Pay. Mirrors
        /// `get_stripe_payment_method_type_from_wallet_data` on the hyperswitch side.
        #[test]
        fn payment_method_types_follows_the_wallet() {
            assert_eq!(
                wallet_token_intent(TokenPaymentMethod::ApplePay).payment_method_types,
                None,
                "Apple Pay must not send payment_method_types[0]"
            );
            assert_eq!(
                wallet_token_intent(TokenPaymentMethod::GooglePay).payment_method_types,
                Some(StripePaymentMethodType::Card),
                "Google Pay must send payment_method_types[0]=card"
            );
        }
    }
}
