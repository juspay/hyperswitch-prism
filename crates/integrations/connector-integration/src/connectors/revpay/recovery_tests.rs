//! Wire-availability recovery tests: end-to-end Authorize/Refund request
//! bodies built from proto-level fixtures, asserted field-by-field.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::as_conversions)]

use super::*;
use common_utils::metadata::MaskedMetadata;
use domain_types::{
    payment_method_data::DefaultPCIHolder,
    types::{AuthorizationRequest, Connectors},
    utils::ForeignTryFrom,
};
use grpc_api_types::payments as proto;
use serde_json::json;

type AuthorizeRouter = RouterDataV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<DefaultPCIHolder>,
    PaymentsResponseData,
>;

fn config() -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Revpay {
        merchant_id: Secret::new("MER00000000001".into()),
        merchant_key: Secret::new("test-key".into()),
        key_index: Secret::new("1".into()),
        base_url: None,
    }
}

fn authorize_router(method: serde_json::Value) -> AuthorizeRouter {
    // Deserialize the HTTP payment-method envelope before using the same domain
    // conversions as the service. This catches schema and dispatch regressions.
    let payment_method: proto::PaymentMethod = serde_json::from_value(json!({
        "payment_method": method
    }))
    .expect("HTTP payment method");
    let pmd = PaymentMethodData::<DefaultPCIHolder>::foreign_try_from(payment_method.clone())
        .expect("domain payment method");
    let wire = proto::PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some("payment-unique-reference".into()),
        amount: Some(proto::Money {
            minor_amount: 1234,
            currency: proto::Currency::Myr as i32,
        }),
        payment_method: Some(payment_method),
        address: Some(Default::default()),
        return_url: Some("https://merchant.example/return".into()),
        webhook_url: Some("https://merchant.example/webhook".into()),
        description: Some("".into()),
        ..Default::default()
    };
    let authorization = AuthorizationRequest::from(wire);
    let resource_common_data = PaymentFlowData::foreign_try_from((
        authorization.clone(),
        Connectors::default(),
        &MaskedMetadata::default(),
    ))
    .expect("payment flow");
    let request =
        PaymentsAuthorizeData::foreign_try_from((authorization, pmd)).expect("authorize request");
    RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data,
        connector_config: config(),
        request,
        response: Err(ErrorResponse::default()),
    }
}

#[test]
fn boost_http_request_reaches_revpay_as_wallet_11() {
    let router = authorize_router(json!({"boost_redirect": {}}));
    assert_eq!(
        router.resource_common_data.payment_method_type,
        Some(common_enums::PaymentMethodType::Boost)
    );
    let request = RevpayPaymentsRequest::try_from(&router).expect("BOOST request");
    let body = serde_json::to_value(request).expect("wire body");
    assert_eq!(body["Payment_ID"], 11);
    assert_eq!(body["Amount"], "12.34");
    assert_eq!(body["Reference_Number"], "payment-unique-reference");
    assert_eq!(
        body["Transaction_Description"],
        "Payment payment-unique-reference"
    );
    assert!(body.get("Bank_Code").is_none());
    assert!(body.get("Funding_Pan").is_none());
    assert!(body.get("Source_Of_Funds").is_none());
}

#[test]
fn fpx_http_issuer_maps_to_revpay_bank_code_without_card_fields() {
    let router = authorize_router(json!({"online_banking_fpx": {"issuer": "Maybank"}}));
    let request = RevpayPaymentsRequest::try_from(&router).expect("FPX request");
    let body = serde_json::to_value(request).expect("wire body");
    assert_eq!(body["Payment_ID"], 3);
    assert_eq!(body["Bank_Code"], "MB2U0227");
    assert_eq!(body["Currency"], "MYR");
    assert_eq!(body["Amount"], "12.34");
    assert_eq!(
        body["Signature"],
        generate_revpay_signature(
            "test-key",
            &[
                &"MER00000000001",
                &"payment-unique-reference",
                &"12.34",
                &"MYR",
            ]
        )
        .peek()
        .as_str()
    );
    for field in [
        "Funding_Pan",
        "Funding_CVV",
        "Funding_Exp_Date",
        "Card_Type",
        "Source_Of_Funds",
    ] {
        assert!(body.get(field).is_none(), "unexpected card field: {field}");
    }
}

#[test]
fn fpx_b2c_bank_codes_match_specification() {
    for (bank, code) in [
        (BankNames::AffinBank, "ABB0233"),
        (BankNames::AllianceBank, "ABMB0212"),
        (BankNames::AgroBank, "AGRO01"),
        (BankNames::AmBank, "AMBB0209"),
        (BankNames::BankIslam, "BIMB0340"),
        (BankNames::BankMuamalat, "BMMB0341"),
        (BankNames::BankRakyat, "BKRM0602"),
        (BankNames::BankSimpananNasional, "BSN0601"),
        (BankNames::CimbBank, "BCBB0235"),
        (BankNames::HongLeongBank, "HLB0224"),
        (BankNames::HsbcBank, "HSBC0223"),
        (BankNames::KuwaitFinanceHouse, "KFH0346"),
        (BankNames::Maybank, "MB2U0227"),
        (BankNames::OcbcBank, "OCBC0229"),
        (BankNames::PublicBank, "PBB0233"),
        (BankNames::RhbBank, "RHB0218"),
        (BankNames::StandardCharteredBank, "SCB0216"),
        (BankNames::UobBank, "UOB0226"),
        (BankNames::BankOfChina, "BOCM01"),
    ] {
        assert_eq!(fpx_bank_code(&bank).expect("supported issuer"), code);
    }
    assert!(fpx_bank_code(&BankNames::Chase).is_err());
}

#[test]
fn fpx_unknown_bank_is_rejected_before_gateway_call() {
    let router = authorize_router(json!({"online_banking_fpx": {"issuer": "Chase"}}));
    assert!(RevpayPaymentsRequest::try_from(&router).is_err());
    assert!(serde_json::from_value::<proto::PaymentMethod>(json!({
        "payment_method": {"online_banking_fpx": {"issuer": "NOT_A_BANK"}}
    }))
    .is_err());
}

#[test]
fn existing_redirect_wallet_routes_are_preserved() {
    for (method, id) in [
        (json!({"ali_pay_redirect": {}}), 15),
        (json!({"we_chat_pay_redirect": {}}), 14),
        (json!({"touch_n_go_redirect": {}}), 28),
        (json!({"grabpay_redirect": {}}), 17),
    ] {
        let request = RevpayPaymentsRequest::try_from(&authorize_router(method)).expect("wallet");
        assert_eq!(request.payment_id, id);
        assert!(request.bank_code.is_none());
    }
}

#[test]
fn boost_payment_method_type_round_trips_through_shared_mappings() {
    let pm: proto::PaymentMethod = serde_json::from_value(json!({
        "payment_method": {"boost_redirect": {}}
    }))
    .expect("BOOST JSON");
    assert_eq!(
        common_enums::PaymentMethod::foreign_try_from(pm).expect("category"),
        common_enums::PaymentMethod::Wallet
    );
    assert_eq!(
        common_enums::PaymentMethod::foreign_try_from(proto::PaymentMethodType::BoostWallet)
            .expect("category"),
        common_enums::PaymentMethod::Wallet
    );
    assert_eq!(
        common_enums::PaymentMethodType::foreign_try_from(proto::PaymentMethodType::BoostWallet)
            .expect("method type"),
        common_enums::PaymentMethodType::Boost
    );
}

#[test]
fn refund_uses_original_payment_reference_and_a_separate_refund_reference() {
    let wire = proto::PaymentServiceRefundRequest {
        merchant_refund_id: Some("refund-unique-reference".into()),
        connector_transaction_id: "gateway-transaction-id".into(),
        connector_order_id: Some("payment-unique-reference".into()),
        refund_amount: Some(proto::Money {
            minor_amount: 500,
            currency: proto::Currency::Myr as i32,
        }),
        payment_amount: 1234,
        ..Default::default()
    };
    let mut router = RouterDataV2 {
        flow: std::marker::PhantomData,
        resource_common_data: RefundFlowData::foreign_try_from((
            wire.clone(),
            Connectors::default(),
            &MaskedMetadata::default(),
        ))
        .expect("refund flow"),
        connector_config: config(),
        request: RefundsData::foreign_try_from(wire).expect("refund data"),
        response: Err(ErrorResponse::default()),
    };
    let request = RevpayRefundRequest::try_from(&router).expect("refund");
    assert_eq!(request.reference_number, "refund-unique-reference");
    assert_eq!(
        request.original_reference_number,
        "payment-unique-reference"
    );
    assert_eq!(
        serde_json::to_value(&request.refund_amount).expect("amount serialization"),
        "5.00"
    );
    assert_eq!(
        request.signature.peek(),
        generate_revpay_signature(
            "test-key",
            &[
                &"MER00000000001",
                &"refund-unique-reference",
                &"5.00",
                &"payment-unique-reference",
            ]
        )
        .peek()
        .as_str()
    );
    router.request.connector_order_id = None;
    assert!(
        RevpayRefundRequest::try_from(&router).is_err(),
        "never substitute the refund or gateway transaction ID"
    );
    router.request.connector_order_id = Some("  ".into());
    assert!(RevpayRefundRequest::try_from(&router).is_err());
}

#[test]
fn composite_authorize_http_json_accepts_boost_and_fpx() {
    for method in [
        json!({"boost_redirect": {}}),
        json!({"online_banking_fpx": {"issuer": "Maybank"}}),
    ] {
        let request: proto::CompositeAuthorizeRequest = serde_json::from_value(json!({
            "merchant_transaction_id": "unique-payment-reference",
            "amount": {"minor_amount": 1234, "currency": "MYR"},
            "payment_method": {"payment_method": method},
            "address": {},
            "auth_type": "NO_THREE_DS",
            "return_url": "https://merchant.example/return",
            "webhook_url": "https://merchant.example/webhook",
            "order_details": []
        }))
        .expect("composite HTTP JSON must deserialize");
        let pmd = PaymentMethodData::<DefaultPCIHolder>::foreign_try_from(
            request.payment_method.expect("payment method"),
        )
        .expect("payment-method dispatch");
        assert!(matches!(
            pmd,
            PaymentMethodData::Wallet(WalletData::BoostRedirect {})
                | PaymentMethodData::BankRedirect(BankRedirectData::OnlineBankingFpx {
                    issuer: BankNames::Maybank
                })
        ));
    }
}

#[test]
fn alipay_plus_and_duitnow_use_distinct_hosted_payment_routes() {
    for (method, id) in [
        (json!({"ali_pay_plus_redirect": {}}), 36),
        (json!({"duit_now": {}}), 37),
    ] {
        let router = authorize_router(method);
        let request = RevpayPaymentsRequest::try_from(&router).expect("hosted request");
        let body = serde_json::to_value(request).expect("wire body");
        assert_eq!(body["Payment_ID"], id);
        assert_eq!(body["Return_URL"], "https://merchant.example/return");
        assert!(body.get("Funding_Pan").is_none());
        assert!(body.get("Bank_Code").is_none());
    }
    // Alipay Online keeps its existing ID rather than silently becoming Alipay+.
    assert_eq!(
        RevpayPaymentsRequest::try_from(&authorize_router(json!({"ali_pay_redirect": {}})))
            .unwrap()
            .payment_id,
        15
    );
}

#[test]
fn mydebit_and_unionpay_explicit_networks_use_dedicated_card_rails() {
    for (network, id, card_type) in [("MYDEBIT", 31, "05"), ("UNIONPAY", 38, "06")] {
        let router = authorize_router(json!({"card": {
            "card_number": "4111111111111111",
            "card_exp_month": "12", "card_exp_year": "2030", "card_cvc": "123",
            "card_network": network
        }}));
        let request = RevpayPaymentsRequest::try_from(&router).expect("explicit card rail");
        let body = serde_json::to_value(request).expect("card request");
        assert_eq!(body["Payment_ID"], id);
        assert_eq!(body["Card_Type"], card_type);
        assert_eq!(body["Source_Of_Funds"], "CARD");
        assert_eq!(body["Funding_Exp_Date"], "203012");
        assert!(body.get("Bank_Code").is_none());
    }
}

#[test]
fn explicit_mydebit_overrides_unionpay_bin_inference() {
    let (id, card_type) = card_payment_details(Some(&CardNetwork::MyDebit), "6250941234567890");
    assert_eq!(id, 31);
    assert!(matches!(card_type, Some(RevpayCardType::MyDebit)));
    let (id, card_type) = card_payment_details(None, "6250941234567890");
    assert_eq!(id, 38);
    assert!(matches!(card_type, Some(RevpayCardType::UnionPay)));
}

#[test]
fn new_shared_payment_identifiers_round_trip() {
    use domain_types::utils::ForeignFrom;
    assert_eq!(
        CardNetwork::foreign_try_from(proto::CardNetwork::Mydebit).unwrap(),
        CardNetwork::MyDebit
    );
    assert_eq!(
        proto::CardNetwork::foreign_from(CardNetwork::MyDebit),
        proto::CardNetwork::Mydebit
    );
    assert_eq!(
        common_enums::PaymentMethodType::foreign_try_from(proto::PaymentMethodType::AliPayPlus)
            .unwrap(),
        common_enums::PaymentMethodType::AliPayPlus
    );
    assert_eq!(
        common_enums::PaymentMethod::foreign_try_from(proto::PaymentMethodType::AliPayPlus)
            .unwrap(),
        common_enums::PaymentMethod::Wallet
    );
}

#[test]
fn hosted_duitnow_and_alipay_plus_response_preserves_gateway_redirect() {
    for (method, payment_id) in [
        (json!({"duit_now": {}}), 37),
        (json!({"ali_pay_plus_redirect": {}}), 36),
    ] {
        let router = authorize_router(method);
        let response: RevpayPaymentsResponse = serde_json::from_value(json!({
            "Payment_ID": payment_id,
            "Reference_Number": "payment-unique-reference",
            "Transaction_ID": null,
            "Amount": "12.34", "Currency": "MYR", "Response_Code": "09",
            "Bank_Redirect_URL": "https://stg-mpg.revpay-sandbox.com.my/v1/payment/intents/test"
        }))
        .unwrap();
        let result = AuthorizeRouter::try_from(ResponseRouterData {
            response,
            router_data: router,
            http_code: 200,
        })
        .expect("hosted response");
        match result.response.expect("payment response") {
            PaymentsResponseData::TransactionResponse {
                redirection_data: Some(redirect),
                ..
            } => {
                assert!(
                    matches!(*redirect, RedirectForm::Uri { uri } if uri == "https://stg-mpg.revpay-sandbox.com.my/v1/payment/intents/test")
                );
            }
            other => panic!("expected hosted redirect, got {other:?}"),
        }
    }
}

#[test]
fn authorize_uses_connector_converter() {
    struct RejectAmount;
    impl AmountConvertor for RejectAmount {
        type Output = StringMajorUnitWithTwoDecimals;
        fn convert(&self, _: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
            Err(ParsingError::DecimalToI64ConversionFailure.into())
        }
        fn convert_back(
            &self,
            amount: Self::Output,
            currency: Currency,
        ) -> Result<Money, error_stack::Report<ParsingError>> {
            StringMajorUnitWithTwoDecimalsForConnector.convert_back(amount, currency)
        }
    }
    let mut connector = crate::connectors::revpay::Revpay::<DefaultPCIHolder>::new().clone();
    connector.amount_converter = &RejectAmount;
    let router_data = authorize_router(json!({"online_banking_fpx": {"issuer": "Maybank"}}));
    let result = RevpayPaymentsRequest::try_from(RevpayRouterData {
        router_data,
        connector,
    });
    assert!(result.is_err(), "the configured converter must be used");
}

#[test]
fn request_signature_is_masked_in_debug_but_present_on_wire() {
    let router = authorize_router(json!({"online_banking_fpx": {"issuer": "Maybank"}}));
    let request = RevpayPaymentsRequest::try_from(&router).expect("request");
    let signature = request.signature.peek().clone();
    assert!(!format!("{request:?}").contains(&signature));
    let body = serde_json::to_value(&request).expect("wire body");
    assert_eq!(body["Signature"], signature);
}
