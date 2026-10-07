use common_utils::request::{Method, Request, RequestContent};
use domain_types::{
    connector_flow::Authorize,
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        ServerAuthenticationTokenResponseData,
    },
    payment_method_data::{
        Card, DefaultPCIHolder, GooglePayDecryptedData, GooglePayPaymentMethodInfo,
        GooglePayWalletData, GpayTokenizationData, PaymentMethodData, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    types::{AuthorizationRequest, Connectors},
    utils::ForeignTryFrom,
};
use grpc_api_types::payments;
use hyperswitch_masking::Secret;
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use serde_json::{json, Value};

type AuthorizeData = RouterDataV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<DefaultPCIHolder>,
    PaymentsResponseData,
>;
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn authorize_data() -> TestResult<AuthorizeData> {
    let number: cards::CardNumber = "378282246310005".parse()?;
    let request = payments::PaymentServiceAuthorizeRequest {
        amount: Some(payments::Money {
            minor_amount: 100,
            currency: payments::Currency::Usd.into(),
        }),
        address: Some(payments::PaymentAddress::default()),
        payment_method: Some(payments::PaymentMethod {
            payment_method: Some(payments::payment_method::PaymentMethod::Card(
                payments::CardDetails {
                    card_number: Some(number.clone()),
                    card_exp_month: Some(Secret::new("12".into())),
                    card_exp_year: Some(Secret::new("2029".into())),
                    card_cvc: Some(Secret::new("1234".into())),
                    card_network: Some(payments::CardNetwork::Amex.into()),
                    ..Default::default()
                },
            )),
        }),
        ..Default::default()
    };
    let authorization = AuthorizationRequest::from(request);
    let mut connectors = Connectors::default();
    connectors.jpmorgan.base_url = "https://example.com".into();
    let mut common = PaymentFlowData::foreign_try_from((
        authorization.clone(),
        connectors,
        &Default::default(),
    ))?;
    common.access_token = Some(ServerAuthenticationTokenResponseData {
        access_token: Secret::new("test-token".into()),
        token_type: None,
        expires_in: None,
    });
    let card = Card {
        card_number: domain_types::payment_method_data::RawCardNumber(number),
        card_exp_month: Secret::new("12".into()),
        card_exp_year: Secret::new("2029".into()),
        card_cvc: Secret::new("1234".into()),
        card_network: Some(common_enums::CardNetwork::AmericanExpress),
        ..Default::default()
    };
    Ok(AuthorizeData {
        flow: Default::default(),
        resource_common_data: common,
        connector_config: ConnectorSpecificConfig::Jpmorgan {
            client_id: Secret::new("test-client".into()),
            client_secret: Secret::new("test-secret".into()),
            company_name: Some(Secret::new("Test company".into())),
            product_name: Some(Secret::new("Test product".into())),
            base_url: None,
            secondary_base_url: None,
            merchant_purchase_description: None,
            statement_descriptor: None,
        },
        request: PaymentsAuthorizeData::foreign_try_from((
            authorization,
            PaymentMethodData::Card(card),
        ))?,
        response: Err(ErrorResponse::default()),
    })
}

fn build(data: &AuthorizeData) -> TestResult<Request> {
    Ok(ConnectorIntegrationV2::<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    >::build_request_v2(
        connector_integration::connectors::jpmorgan::Jpmorgan::new(),
        data,
    )?
    .ok_or("The connector must build a request")?)
}

fn json_body(request: Request) -> TestResult<Value> {
    match request.body.ok_or("A POST request must have a body")? {
        RequestContent::Json(value) => Ok(serde_json::to_value(value)?),
        _ => Err("Expected a JSON body".into()),
    }
}

fn check_eq<T: PartialEq + std::fmt::Debug>(actual: T, expected: T) -> TestResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected {expected:?}; received {actual:?}").into())
    }
}

#[test]
fn amex_three_ds_does_not_require_transaction_id_or_protocol() -> TestResult<()> {
    let mut data = authorize_data()?;
    data.request.authentication_data = Some(serde_json::from_value(
        json!({"cavv":"AAAAAA==","eci":"05"}),
    )?);
    let request = build(&data)?;
    check_eq(request.method, Method::Post)?;
    let body = json_body(request)?;
    let authentication = body
        .pointer("/paymentMethodType/card/authentication")
        .ok_or("Expected card authentication")?;
    check_eq(
        authentication.pointer("/threeDS/authenticationValue"),
        Some(&json!("AAAAAA==")),
    )?;
    check_eq(
        authentication.get("electronicCommerceIndicator"),
        Some(&json!("05")),
    )?;
    check_eq(
        authentication
            .pointer("/threeDS/authenticationTransactionId")
            .is_none(),
        true,
    )?;
    check_eq(
        authentication
            .pointer("/threeDS/threeDSProgramProtocol")
            .is_none(),
        true,
    )?;
    Ok(())
}

#[test]
fn short_wallet_cryptogram_is_preserved_and_missing_mastercard_eci_uses_token_default(
) -> TestResult<()> {
    let mut data = authorize_data()?;
    data.request.payment_method_data = decrypted_google_pay()?;
    let body = json_body(build(&data)?)?;
    let card = body
        .pointer("/paymentMethodType/card")
        .ok_or("Expected card data")?;
    check_eq(
        card.pointer("/authentication/tokenAuthenticationValue"),
        Some(&json!("ABCD")),
    )?;
    check_eq(
        card.pointer("/authentication/electronicCommerceIndicator"),
        Some(&json!("5")),
    )?;
    check_eq(card.pointer("/expiry/year"), Some(&json!(2029)))?;
    Ok(())
}

fn decrypted_google_pay() -> TestResult<PaymentMethodData<DefaultPCIHolder>> {
    Ok(PaymentMethodData::Wallet(WalletData::GooglePay(
        GooglePayWalletData {
            pm_type: "CARD".into(),
            description: "Public test wallet".into(),
            info: GooglePayPaymentMethodInfo {
                card_network: "MASTERCARD".into(),
                card_details: "0005".into(),
                assurance_details: None,
            },
            tokenization_data: GpayTokenizationData::Decrypted(GooglePayDecryptedData {
                card_exp_month: Secret::new("12".into()),
                card_exp_year: Secret::new("29".into()),
                application_primary_account_number: "5555555555554444".parse()?,
                cryptogram: Some(Secret::new("ABCD".into())),
                eci_indicator: None,
                auth_method: Some(common_enums::GooglePayAuthMethod::Cryptogram),
            }),
        },
    )))
}

#[test]
fn wallet_three_ds_preserves_both_cryptograms_and_uses_authentication_eci() -> TestResult<()> {
    let mut data = authorize_data()?;
    data.request.payment_method_data = decrypted_google_pay()?;
    data.request.authentication_data = Some(serde_json::from_value(json!({
        "cavv": "AAAAAA==", "eci": "02", "ds_trans_id": "original-directory-id", "message_version": "2.2.0"
    }))?);
    let body = json_body(build(&data)?)?;
    let authentication = body
        .pointer("/paymentMethodType/card/authentication")
        .ok_or("Expected card authentication")?;
    check_eq(
        authentication.get("tokenAuthenticationValue"),
        Some(&json!("ABCD")),
    )?;
    check_eq(
        authentication.pointer("/threeDS/authenticationValue"),
        Some(&json!("AAAAAA==")),
    )?;
    check_eq(
        authentication.pointer("/threeDS/authenticationTransactionId"),
        Some(&json!("original-directory-id")),
    )?;
    check_eq(
        authentication.pointer("/threeDS/threeDSProgramProtocol"),
        Some(&json!("2.2.0")),
    )?;
    check_eq(
        authentication.get("electronicCommerceIndicator"),
        Some(&json!("02")),
    )?;
    Ok(())
}

#[test]
fn redirect_continuation_retrieves_the_original_resource_without_a_payment_body() -> TestResult<()>
{
    for (kind, path) in [("PAYMENT", "payments"), ("VERIFICATION", "verifications")] {
        let mut data = authorize_data()?;
        data.request.connector_feature_data = Some(Secret::new(json!({"jpmorgan": {
            "continueThreeDs": true,
            "threeDsResource": {"kind": kind, "id": "original-resource", "merchantId": data.resource_common_data.merchant_id.get_string_repr()}
        }})));
        let request = build(&data)?;
        check_eq(request.method, Method::Get)?;
        check_eq(
            request.url,
            format!("https://example.com/{path}/original-resource"),
        )?;
        check_eq(request.body.is_none(), true)?;
    }
    Ok(())
}

#[test]
fn redirect_continuation_rejects_a_resource_from_another_merchant() -> TestResult<()> {
    let mut data = authorize_data()?;
    data.request.connector_feature_data = Some(Secret::new(json!({"jpmorgan": {
        "continueThreeDs": true,
        "threeDsResource": {"kind": "PAYMENT", "id": "original-resource", "merchantId": "another-merchant"}
    }})));
    let error = ConnectorIntegrationV2::<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    >::build_request_v2(
        connector_integration::connectors::jpmorgan::Jpmorgan::new(),
        &data,
    )
    .err()
    .ok_or("A resource from another merchant must not be retrieved")?;
    if !matches!(
        error.current_context(),
        domain_types::errors::IntegrationError::InvalidDataFormat {
            field_name: "connector_feature_data.jpmorgan.threeDsResource",
            ..
        }
    ) {
        return Err(format!("Expected InvalidDataFormat for connector_feature_data.jpmorgan.threeDsResource; received {error:?}").into());
    }
    Ok(())
}

#[test]
fn ach_preserves_supplied_contact_details_without_requiring_a_complete_address() -> TestResult<()> {
    let mut data = authorize_data()?;
    data.request.payment_method_data = PaymentMethodData::BankDebit(
        domain_types::payment_method_data::BankDebitData::AchBankDebit {
            account_number: Secret::new("123456789".into()),
            routing_number: Secret::new("021000021".into()),
            bank_account_holder_name: Some(Secret::new("Test Customer".into())),
            card_holder_name: None,
            bank_name: None,
            bank_type: None,
            bank_holder_type: None,
        },
    );
    data.resource_common_data.address = domain_types::payment_address::PaymentAddress::new(
        None,
        Some(serde_json::from_value(json!({
            "email": "billing@example.com", "address": {"city": "New York"},
            "phone": {"number": "2125550100", "country_code": "+1"}
        }))?),
        None,
        None,
    );
    if let ConnectorSpecificConfig::Jpmorgan {
        statement_descriptor,
        ..
    } = &mut data.connector_config
    {
        *statement_descriptor = Some(Secret::new("Test merchant".into()));
    }
    let body = json_body(build(&data)?)?;
    let holder = body
        .get("accountHolder")
        .ok_or("Expected an account holder")?;
    check_eq(holder.get("email"), Some(&json!("billing@example.com")))?;
    check_eq(
        holder.pointer("/billingAddress/city"),
        Some(&json!("New York")),
    )?;
    check_eq(holder.pointer("/billingAddress/line1").is_none(), true)?;
    check_eq(
        holder.pointer("/phone/phoneNumber"),
        Some(&json!("2125550100")),
    )?;
    check_eq(holder.pointer("/phone/countryCode"), Some(&json!(1)))?;
    Ok(())
}

fn google_pay_proto(auth_method: Option<i32>) -> TestResult<payments::PaymentMethod> {
    Ok(payments::PaymentMethod {
        payment_method: Some(payments::payment_method::PaymentMethod::GooglePaySdk(
            payments::GoogleWallet {
                r#type: "CARD".into(),
                info: Some(payments::google_wallet::PaymentMethodInfo {
                    card_network: "MASTERCARD".into(),
                    ..Default::default()
                }),
                tokenization_data: Some(payments::google_wallet::TokenizationData {
                    tokenization_data: Some(
                        payments::google_wallet::tokenization_data::TokenizationData::DecryptedData(
                            payments::GooglePayDecryptedData {
                                card_exp_month: Some(Secret::new("12".into())),
                                card_exp_year: Some(Secret::new("2029".into())),
                                application_primary_account_number: Some(
                                    "5555555555554444".parse()?,
                                ),
                                auth_method,
                                ..Default::default()
                            },
                        ),
                    ),
                }),
                ..Default::default()
            },
        )),
    })
}

#[test]
fn google_pay_pan_only_keeps_the_json_contract_and_sends_no_token_cryptogram() -> TestResult<()> {
    let method = google_pay_proto(Some(payments::GooglePayAuthMethod::PanOnly.into()))?;
    let json = serde_json::to_value(&method)?;
    check_eq(
        json.pointer("/payment_method/google_pay_sdk/tokenization_data/tokenization_data/decrypted_data/auth_method"),
        Some(&json!("PAN_ONLY"))
    )?;
    let mut data = authorize_data()?;
    data.request.payment_method_data =
        PaymentMethodData::convert_to_domain_model_for_non_card_payment_methods(method)?;
    let body = json_body(build(&data)?)?;
    let card = body
        .pointer("/paymentMethodType/card")
        .ok_or("Expected card data")?;
    check_eq(card.get("accountNumberType"), Some(&json!("PAN")))?;
    check_eq(card.get("authentication").is_none(), true)?;
    Ok(())
}

#[test]
fn google_pay_missing_classification_is_not_inferred_and_unknown_values_are_rejected(
) -> TestResult<()> {
    for classification in [
        None,
        Some(payments::GooglePayAuthMethod::Unspecified.into()),
    ] {
        let mut data = authorize_data()?;
        data.request.payment_method_data =
            PaymentMethodData::convert_to_domain_model_for_non_card_payment_methods(
                google_pay_proto(classification)?,
            )?;
        let error = ConnectorIntegrationV2::<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<DefaultPCIHolder>,
            PaymentsResponseData,
        >::build_request_v2(
            connector_integration::connectors::jpmorgan::Jpmorgan::new(),
            &data,
        )
        .err()
        .ok_or("JPMorgan must reject a missing wallet classification")?;
        if !matches!(
            error.current_context(),
            domain_types::errors::IntegrationError::MissingRequiredField {
                field_name: "wallet.google_pay.auth_method",
                ..
            }
        ) {
            return Err(format!("Expected MissingRequiredField for wallet.google_pay.auth_method; received {error:?}").into());
        }
    }
    let error = PaymentMethodData::<DefaultPCIHolder>::convert_to_domain_model_for_non_card_payment_methods(google_pay_proto(Some(99))?).err().ok_or("Unknown classifications must be rejected")?;
    if !matches!(
        error.current_context(),
        domain_types::errors::IntegrationError::InvalidDataFormat {
            field_name: "payment_method.google_pay.auth_method",
            ..
        }
    ) {
        return Err(format!("Expected InvalidDataFormat for payment_method.google_pay.auth_method; received {error:?}").into());
    }
    Ok(())
}

#[test]
fn wallet_repeat_payment_preserves_original_references_without_replaying_a_cryptogram(
) -> TestResult<()> {
    use domain_types::{connector_flow::RepeatPayment, connector_types::RepeatPaymentData};

    let original_id = "original-network-transaction";
    let original_link = "1234567890123456789012";
    let base = authorize_data()?;
    let request = payments::RecurringPaymentServiceChargeRequest {
        amount: Some(payments::Money { minor_amount: 100, currency: payments::Currency::Usd.into() }),
        capture_method: Some(payments::CaptureMethod::Automatic.into()),
        connector_recurring_payment_id: Some(payments::MandateReference {
            mandate_id_type: Some(payments::mandate_reference::MandateIdType::NetworkMandateId(payments::NetworkMandateId {
                network_transaction_id: original_id.into(), transaction_link_id: Some(original_link.into()),
            })),
        }),
        connector_feature_data: Some(Secret::new(json!({"jpmorgan": {
            "walletProvider": "GOOGLE_PAY", "accountNumberType": "DEVICE_TOKEN",
            "originalNetworkTransactionId": original_id, "originalTransactionLinkId": original_link
        }}).to_string())),
        ..Default::default()
    };
    let mut data = RouterDataV2 {
        flow: Default::default(),
        resource_common_data: base.resource_common_data,
        connector_config: base.connector_config,
        response: Err(ErrorResponse::default()),
        request: RepeatPaymentData::foreign_try_from((request, Some(decrypted_google_pay()?)))?,
    };
    let connector = connector_integration::connectors::jpmorgan::Jpmorgan::new();
    let request = ConnectorIntegrationV2::<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<DefaultPCIHolder>,
        PaymentsResponseData,
    >::build_request_v2(connector, &data)?
    .ok_or("The connector must build a request")?;
    check_eq(request.method, Method::Post)?;
    let body = json_body(request)?;
    let card = body
        .pointer("/paymentMethodType/card")
        .ok_or("Expected card data")?;
    check_eq(
        card.get("originalNetworkTransactionId"),
        Some(&json!(original_id)),
    )?;
    check_eq(
        card.get("originalTransactionLinkId"),
        Some(&json!(original_link)),
    )?;
    check_eq(card.get("authentication").is_none(), true)?;
    check_eq(body.get("initiatorType"), Some(&json!("MERCHANT")))?;

    if let domain_types::connector_types::MandateReferenceId::NetworkMandateId(reference) =
        &mut data.request.mandate_reference
    {
        reference.network_transaction_id = "different-network-transaction".into();
    }
    let error = ConnectorIntegrationV2::<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<DefaultPCIHolder>,
        PaymentsResponseData,
    >::build_request_v2(connector, &data)
    .err()
    .ok_or("A repeat payment must not replace the original network reference")?;
    if !matches!(
        error.current_context(),
        domain_types::errors::IntegrationError::InvalidDataFormat {
            field_name: "original_network_transaction_id",
            ..
        }
    ) {
        return Err(format!(
            "Expected InvalidDataFormat for original_network_transaction_id; received {error:?}"
        )
        .into());
    }
    Ok(())
}

#[test]
fn visa_wallet_three_ds_requires_the_original_authentication_transaction_id() -> TestResult<()> {
    let mut data = authorize_data()?;
    let mut wallet = decrypted_google_pay()?;
    if let PaymentMethodData::Wallet(WalletData::GooglePay(wallet)) = &mut wallet {
        wallet.info.card_network = "VISA".to_owned();
    }
    data.request.payment_method_data = wallet;
    data.request.authentication_data = Some(serde_json::from_value(
        json!({"cavv": "AAAAAA==", "eci": "05"}),
    )?);
    let error = ConnectorIntegrationV2::<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    >::build_request_v2(
        connector_integration::connectors::jpmorgan::Jpmorgan::new(),
        &data,
    )
    .err()
    .ok_or("Visa must reject a missing original authentication ID")?;
    if !matches!(
        error.current_context(),
        domain_types::errors::IntegrationError::MissingRequiredField {
            field_name: "authentication_data.ds_trans_id",
            ..
        }
    ) {
        return Err(format!(
            "Expected MissingRequiredField for authentication_data.ds_trans_id; received {error:?}"
        )
        .into());
    }
    Ok(())
}
