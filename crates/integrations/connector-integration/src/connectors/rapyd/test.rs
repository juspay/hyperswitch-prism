use std::{error::Error, marker::PhantomData};

use common_utils::metadata::MaskedMetadata;
use domain_types::{
    connector_flow::Authorize,
    connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
    errors::IntegrationError,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodToken},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    types::{build_request_data_with_required_pmd, AuthorizationRequest, Connectors},
    utils::ForeignTryFrom,
};
use error_stack::Report;
use grpc_api_types::payments as grpc;
use hyperswitch_masking::Secret;
use serde_json::{json, Value};

use super::{transformers::RapydPaymentsRequest, Rapyd, RapydRouterData};

type AuthorizeData = RouterDataV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<DefaultPCIHolder>,
    PaymentsResponseData,
>;
type TestResult = Result<(), Box<dyn Error>>;

fn card_request() -> Result<grpc::PaymentServiceAuthorizeRequest, Box<dyn Error>> {
    Ok(grpc::PaymentServiceAuthorizeRequest {
        merchant_transaction_id: Some("rapyd_request_test".to_string()),
        amount: Some(grpc::Money {
            minor_amount: 1000,
            currency: grpc::Currency::Eur.into(),
        }),
        payment_method: Some(grpc::PaymentMethod {
            payment_method: Some(grpc::payment_method::PaymentMethod::Card(
                grpc::CardDetails {
                    card_number: Some("4111111111111111".parse()?),
                    card_exp_month: Some(Secret::new("12".to_string())),
                    card_exp_year: Some(Secret::new("30".to_string())),
                    card_cvc: Some(Secret::new("123".to_string())),
                    card_network: Some(grpc::CardNetwork::Visa.into()),
                    ..Default::default()
                },
            )),
        }),
        address: Some(grpc::PaymentAddress {
            billing_address: Some(grpc::Address {
                first_name: Some(Secret::new("Test".to_string())),
                last_name: Some(Secret::new("User".to_string())),
                country_alpha2_code: Some(grpc::CountryAlpha2::Us.into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        return_url: Some("https://example.com/return".to_string()),
        auth_type: grpc::AuthenticationType::NoThreeDs.into(),
        capture_method: Some(grpc::CaptureMethod::Automatic.into()),
        ..Default::default()
    })
}

fn authorize_data(
    request: grpc::PaymentServiceAuthorizeRequest,
) -> Result<AuthorizeData, Box<dyn Error>> {
    let common = PaymentFlowData::foreign_try_from((
        request.clone(),
        Connectors::default(),
        &MaskedMetadata::default(),
    ))?;
    let request = build_request_data_with_required_pmd(
        request.payment_method.clone(),
        AuthorizationRequest::from(request),
    )?;
    Ok(AuthorizeData {
        flow: PhantomData,
        resource_common_data: common,
        connector_config: ConnectorSpecificConfig::Rapyd {
            access_key: Secret::new("test_access_key".to_string()),
            secret_key: Secret::new("test_secret_key".to_string()),
            country_of_incorporation: Some(common_enums::CountryAlpha2::IS),
            base_url: None,
        },
        request,
        response: Err(ErrorResponse::default()),
    })
}

fn convert(
    data: AuthorizeData,
) -> Result<RapydPaymentsRequest<DefaultPCIHolder>, Report<IntegrationError>> {
    RapydPaymentsRequest::try_from(RapydRouterData {
        connector: (*Rapyd::<DefaultPCIHolder>::new()).clone(),
        router_data: data,
    })
}

fn external_authentication() -> grpc::AuthenticationData {
    grpc::AuthenticationData {
        message_version: Some("2.2.0".to_string()),
        cavv: Some("test_cavv".to_string()),
        eci: Some("05".to_string()),
        ds_transaction_id: Some("test_ds_transaction_id".to_string()),
        ..Default::default()
    }
}

#[test]
fn hosted_authentication_follows_the_requested_authentication_type() -> TestResult {
    for (auth_type, required) in [
        (grpc::AuthenticationType::ThreeDs, true),
        (grpc::AuthenticationType::NoThreeDs, false),
    ] {
        let mut request = card_request()?;
        request.auth_type = auth_type.into();
        let wire = serde_json::to_value(convert(authorize_data(request)?)?)?;
        if wire.get("payment_method_options") != Some(&json!({ "3d_required": required })) {
            return Err(format!("unexpected hosted authentication options: {wire}").into());
        }
        if wire.get("capture") != Some(&Value::Bool(true)) {
            return Err("automatic capture was not preserved".into());
        }
    }
    Ok(())
}

#[test]
fn external_authentication_replaces_hosted_authentication_options() -> TestResult {
    let mut request = card_request()?;
    request.authentication_data = Some(external_authentication());
    let wire = serde_json::to_value(convert(authorize_data(request)?)?)?;
    if wire.get("payment_method_options")
        != Some(&json!({
            "3d_version": "2.2.0",
            "cavv": "test_cavv",
            "eci": "05",
            "ds_trans_id": "test_ds_transaction_id"
        }))
    {
        return Err(format!("unexpected external authentication options: {wire}").into());
    }
    Ok(())
}

#[test]
fn absent_optional_authentication_fields_are_omitted() -> TestResult {
    let mut authentication = external_authentication();
    authentication.eci = None;
    authentication.ds_transaction_id = None;
    let mut request = card_request()?;
    request.authentication_data = Some(authentication);
    let wire = serde_json::to_value(convert(authorize_data(request)?)?)?;
    if wire.get("payment_method_options")
        != Some(&json!({ "3d_version": "2.2.0", "cavv": "test_cavv" }))
    {
        return Err(
            format!("absent optional authentication fields were not omitted: {wire}").into(),
        );
    }
    Ok(())
}

#[test]
fn incomplete_external_authentication_reports_the_missing_field() -> TestResult {
    for field in ["message_version", "cavv"] {
        let mut authentication = external_authentication();
        if field == "message_version" {
            authentication.message_version = None;
        } else {
            authentication.cavv = None;
        }
        let mut request = card_request()?;
        request.authentication_data = Some(authentication);
        let error = convert(authorize_data(request)?)
            .err()
            .ok_or("incomplete authentication unexpectedly succeeded")?;
        match error.current_context() {
            IntegrationError::MissingRequiredField { field_name, .. } => {
                if *field_name != format!("authentication_data.{field}") {
                    return Err(
                        format!("unexpected missing authentication field: {field_name}").into(),
                    );
                }
            }
            other => return Err(format!("unexpected authentication error: {other:?}").into()),
        }
    }
    Ok(())
}

#[test]
fn merchant_country_and_network_select_the_method_independently_of_funding() -> TestResult {
    for (country, network, expected) in [
        (
            common_enums::CountryAlpha2::IS,
            grpc::CardNetwork::Visa,
            "is_visa_card",
        ),
        (
            common_enums::CountryAlpha2::CY,
            grpc::CardNetwork::Visa,
            "cy_visa_card",
        ),
        (
            common_enums::CountryAlpha2::DE,
            grpc::CardNetwork::Mastercard,
            "de_mastercard_card",
        ),
        (
            common_enums::CountryAlpha2::IS,
            grpc::CardNetwork::Amex,
            "is_amex_card",
        ),
    ] {
        for funding in [None, Some("credit"), Some("debit"), Some("prepaid")] {
            let mut request = card_request()?;
            let Some(grpc::payment_method::PaymentMethod::Card(card)) = request
                .payment_method
                .as_mut()
                .and_then(|method| method.payment_method.as_mut())
            else {
                return Err("card fixture is missing".into());
            };
            card.card_network = Some(network.into());
            card.card_type = funding.map(str::to_string);
            card.card_issuing_country_alpha2 = Some("IN".to_string());
            let mut data = authorize_data(request)?;
            let ConnectorSpecificConfig::Rapyd {
                country_of_incorporation,
                ..
            } = &mut data.connector_config
            else {
                return Err("Rapyd fixture is missing".into());
            };
            *country_of_incorporation = Some(country);
            let wire = serde_json::to_value(convert(data)?)?;
            if wire.pointer("/payment_method/type") != Some(&json!(expected)) {
                return Err(format!("expected {expected} for funding {funding:?}: {wire}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn mastercard_bin_resolves_a_missing_network() -> TestResult {
    let mut request = card_request()?;
    let Some(grpc::payment_method::PaymentMethod::Card(card)) = request
        .payment_method
        .as_mut()
        .and_then(|method| method.payment_method.as_mut())
    else {
        return Err("card fixture is missing".into());
    };
    card.card_number = Some("5123456789012346".parse()?);
    card.card_network = None;
    let wire = serde_json::to_value(convert(authorize_data(request)?)?)?;
    if wire.pointer("/payment_method/type") != Some(&json!("is_mastercard_card")) {
        return Err(format!("Mastercard BIN was not translated to its method type: {wire}").into());
    }
    Ok(())
}

#[test]
fn fresh_cards_require_the_merchant_country() -> TestResult {
    let mut data = authorize_data(card_request()?)?;
    let ConnectorSpecificConfig::Rapyd {
        country_of_incorporation,
        ..
    } = &mut data.connector_config
    else {
        return Err("Rapyd fixture is missing".into());
    };
    *country_of_incorporation = None;
    let error = convert(data)
        .err()
        .ok_or("missing country unexpectedly succeeded")?;
    match error.current_context() {
        IntegrationError::MissingRequiredField { field_name, .. } => {
            if *field_name != "country_of_incorporation" {
                return Err(format!("unexpected missing configuration field: {field_name}").into());
            }
        }
        other => return Err(format!("unexpected country error: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn saved_payment_method_ids_do_not_require_the_merchant_country() -> TestResult {
    let mut data = authorize_data(card_request()?)?;
    let ConnectorSpecificConfig::Rapyd {
        country_of_incorporation,
        ..
    } = &mut data.connector_config
    else {
        return Err("Rapyd fixture is missing".into());
    };
    *country_of_incorporation = None;
    data.request.payment_method_data = PaymentMethodData::PaymentMethodToken(PaymentMethodToken {
        token: Secret::new("card_saved_test".to_string()),
        token_payment_method_type: None,
    });
    let wire = serde_json::to_value(convert(data)?)?;
    if wire.get("payment_method") != Some(&json!("card_saved_test")) {
        return Err(format!("saved payment method ID was not preserved: {wire}").into());
    }
    Ok(())
}
