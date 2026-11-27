use std::{error::Error, marker::PhantomData};

use common_utils::metadata::MaskedMetadata;
use connector_integration::connectors::rapyd::Rapyd;
use domain_types::{
    connector_flow::{PSync, SetupMandate},
    connector_types::{
        PaymentFlowData, PaymentsResponseData, PaymentsSyncData, SetupMandateRequestData,
    },
    payment_method_data::DefaultPCIHolder,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{build_request_data_with_required_pmd, Connectors, SetupRecurringRequest},
    utils::ForeignTryFrom,
};
use grpc_api_types::payments as grpc;
use hyperswitch_masking::{ExposeInterface, Secret};
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use serde_json::{json, Value};

type TestResult = Result<(), Box<dyn Error>>;

#[track_caller]
fn check(condition: bool) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(format!("assertion failed at {}", std::panic::Location::caller()).into())
    }
}

#[track_caller]
fn check_eq<L: std::fmt::Debug + PartialEq<R>, R: std::fmt::Debug>(
    actual: L,
    expected: R,
) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "expected {expected:?}, got {actual:?} at {}",
            std::panic::Location::caller()
        )
        .into())
    }
}

type SetupData = RouterDataV2<
    SetupMandate,
    PaymentFlowData,
    SetupMandateRequestData<DefaultPCIHolder>,
    PaymentsResponseData,
>;
type SyncData = RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>;

fn setup_data() -> Result<SetupData, Box<dyn Error>> {
    let request = grpc::PaymentServiceSetupRecurringRequest {
        merchant_recurring_payment_id: "merchant_reference".into(),
        amount: Some(grpc::Money {
            minor_amount: 0,
            currency: grpc::Currency::Usd.into(),
        }),
        payment_method: Some(grpc::PaymentMethod {
            payment_method: Some(grpc::payment_method::PaymentMethod::Card(
                grpc::CardDetails {
                    card_number: Some("4111111111111111".parse()?),
                    card_exp_month: Some(Secret::new("12".into())),
                    card_exp_year: Some(Secret::new("30".into())),
                    card_cvc: Some(Secret::new("123".into())),
                    card_holder_name: Some(Secret::new("Test User".into())),
                    card_network: Some(grpc::CardNetwork::Visa.into()),
                    ..Default::default()
                },
            )),
        }),
        customer: Some(grpc::Customer {
            name: Some("Test User".into()),
            email: Some(Secret::new("test@example.com".into())),
            ..Default::default()
        }),
        address: Some(grpc::PaymentAddress::default()),
        return_url: Some("https://example.com/setup-return".into()),
        auth_type: grpc::AuthenticationType::ThreeDs.into(),
        setup_future_usage: Some(grpc::FutureUsage::OffSession.into()),
        ..Default::default()
    };
    let intermediate = SetupRecurringRequest::from(request.clone());
    let metadata = MaskedMetadata::default();
    let mut connectors = Connectors::default();
    connectors.rapyd.base_url = "https://sandboxapi.rapyd.net".into();
    let mut common =
        PaymentFlowData::foreign_try_from((intermediate.clone(), connectors, &metadata))?;
    common.attempt_id = "attempt_setup_1".into();
    let request = build_request_data_with_required_pmd(request.payment_method, intermediate)?;
    Ok(SetupData {
        flow: PhantomData,
        resource_common_data: common,
        connector_config: ConnectorSpecificConfig::Rapyd {
            access_key: Secret::new("test_access_key".into()),
            secret_key: Secret::new("test_secret_key".into()),
            country_of_incorporation: Some(common_enums::CountryAlpha2::IS),
            base_url: None,
        },
        request,
        response: Err(ErrorResponse::default()),
    })
}

fn wire(data: &SetupData) -> Result<(String, Value), Box<dyn Error>> {
    let request = Rapyd::<DefaultPCIHolder>::new()
        .build_request_v2(data)?
        .ok_or("missing HTTP request")?;
    let body = request
        .body
        .ok_or("missing request body")?
        .get_inner_value()
        .expose();
    Ok((request.url, serde_json::from_str(&body)?))
}

#[test]
fn hosted_setup_registers_a_card_without_creating_a_payment() -> TestResult {
    let data = setup_data()?;
    let (url, body) = wire(&data)?;
    check_eq(url.as_str(), "https://sandboxapi.rapyd.net/v1/customers")?;
    check_eq(body.get("name"), Some(&json!("Test User")))?;
    check_eq(
        body.pointer("/payment_method/complete_payment_url"),
        Some(&json!("https://example.com/setup-return")),
    )?;
    check_eq(
        body.pointer("/payment_method/error_payment_url"),
        Some(&json!("https://example.com/setup-return")),
    )?;
    check_eq(
        body.get("payment_method_options"),
        Some(&json!({"3d_required": true, "currency": "USD"})),
    )?;
    check(body.get("amount").is_none())?;
    check(body.get("capture").is_none())?;
    check(
        body.pointer("/payment_method/metadata/ucs_setup_attempt")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("rapyd_setup_v1_")),
    )?;
    check_eq(
        body.pointer("/payment_method/metadata/ucs_setup_mode"),
        Some(&json!("hosted_3ds")),
    )?;
    check_eq(wire(&data)?, (url, body))?;
    Ok(())
}

fn response(body: Value) -> Result<Response, Box<dyn Error>> {
    Ok(Response {
        headers: None,
        response: serde_json::to_vec(&body)?.into(),
        status_code: 200,
    })
}

fn external_setup() -> Result<SetupData, Box<dyn Error>> {
    let mut data = setup_data()?;
    data.request.authentication_data = Some(serde_json::from_value(json!({
        "trans_status": "Y", "message_version": "2.2.0", "eci": "05",
        "cavv": "test_cavv", "ds_trans_id": "test_directory_transaction"
    }))?);
    Ok(data)
}

#[test]
fn pending_hosted_registration_does_not_activate_the_card() -> TestResult {
    let data = setup_data()?;
    let mapped = Rapyd::<DefaultPCIHolder>::new().handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""},
            "data": {"id": "cus_test", "payment_methods": {"data": [{
                "id": "card_not_verified", "next_action": "3d_verification",
                "redirect_url": "https://sandboxcheckout.rapyd.net/3ds",
                "network_reference_id": 12345
            }]}}
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::AuthenticationPending,
    )?;
    check_eq(
        mapped.resource_common_data.connector_customer.as_deref(),
        Some("cus_test"),
    )?;
    let PaymentsResponseData::TransactionResponse {
        resource_id,
        mandate_reference,
        redirection_data,
        connector_metadata,
        network_txn_id,
        ..
    } = mapped.response.map_err(|_| "unexpected provider error")?
    else {
        return Err("expected setup response".into());
    };
    check_eq(
        Some(resource_id.get_connector_transaction_id()?.as_str()),
        wire(&data)?
            .1
            .pointer("/payment_method/metadata/ucs_setup_attempt")
            .and_then(Value::as_str),
    )?;
    check(mandate_reference.is_none())?;
    check(network_txn_id.is_none())?;
    check(redirection_data.is_some())?;
    check_eq(
        connector_metadata
            .ok_or("missing setup state")?
            .pointer("/rapyd_setup/customer_id"),
        Some(&json!("cus_test")),
    )?;
    Ok(())
}

#[test]
fn existing_customer_adds_and_verifies_a_different_card() -> TestResult {
    let mut data = external_setup()?;
    data.resource_common_data.connector_customer = Some("cus_existing".into());
    let (url, body) = wire(&data)?;
    check_eq(
        url.as_str(),
        "https://sandboxapi.rapyd.net/v1/customers/cus_existing/payment_methods",
    )?;
    check(body.get("payment_method").is_none())?;
    check_eq(
        body.pointer("/payment_method_options/cavv"),
        Some(&json!("test_cavv")),
    )?;
    check(
        body.pointer("/payment_method_options/3d_required")
            .is_none(),
    )?;
    let mapped = Rapyd::<DefaultPCIHolder>::new().handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""},
            "data": {"id": "card_new", "next_action": "not_applicable",
                "redirect_url": "", "network_reference_id": 98765}
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::Charged,
    )?;
    let PaymentsResponseData::TransactionResponse {
        mandate_reference,
        network_txn_id,
        ..
    } = mapped.response.map_err(|_| "unexpected provider error")?
    else {
        return Err("expected setup response".into());
    };
    check_eq(
        mandate_reference
            .ok_or("missing card reference")?
            .connector_mandate_id
            .as_deref(),
        Some("card_new"),
    )?;
    check_eq(network_txn_id.as_deref(), Some("98765"))?;
    Ok(())
}

#[test]
fn external_customer_registration_preserves_the_customer_network_reference() -> TestResult {
    let mapped = Rapyd::<DefaultPCIHolder>::new().handle_response_v2(
        &external_setup()?,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""},
            "data": {
                "id": "cus_test", "network_reference_id": 937015,
                "payment_methods": {"data": [{
                    "id": "card_registered", "next_action": "not_applicable", "redirect_url": ""
                }]}
            }
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::Charged,
    )?;
    let PaymentsResponseData::TransactionResponse { network_txn_id, .. } =
        mapped.response.map_err(|_| "unexpected provider error")?
    else {
        return Err("expected setup response".into());
    };
    check_eq(network_txn_id.as_deref(), Some("937015"))?;
    Ok(())
}

#[test]
fn unsupported_or_incomplete_setup_requests_fail_before_provider_submission() -> TestResult {
    let mut data = setup_data()?;
    data.resource_common_data.auth_type = common_enums::AuthenticationType::NoThreeDs;
    check(wire(&data).is_err())?;
    let mut data = external_setup()?;
    data.request
        .authentication_data
        .as_mut()
        .ok_or("missing external data")?
        .trans_status = Some(common_enums::TransactionStatus::ChallengeRequired);
    check(wire(&data).is_err())?;
    let mut data = setup_data()?;
    data.request.money = None;
    check(wire(&data).is_err())?;
    let mut data = setup_data()?;
    data.resource_common_data.attempt_id = "IRRELEVANT_ATTEMPT_ID".into();
    check(wire(&data).is_err())?;
    Ok(())
}

fn sync_data(data: &SetupData) -> Result<SyncData, Box<dyn Error>> {
    let mapped = Rapyd::<DefaultPCIHolder>::new().handle_response_v2(
        data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""},
            "data": {"id": "cus_test", "payment_methods": {"data": [{
                "id": null, "next_action": "3d_verification",
                "redirect_url": "https://sandboxcheckout.rapyd.net/3ds"
            }]}}
        }))?,
    )?;
    let PaymentsResponseData::TransactionResponse {
        resource_id,
        connector_metadata,
        ..
    } = mapped.response.map_err(|_| "unexpected setup failure")?
    else {
        return Err("expected setup response".into());
    };
    Ok(SyncData {
        flow: PhantomData,
        resource_common_data: mapped.resource_common_data,
        connector_config: data.connector_config.clone(),
        request: PaymentsSyncData {
            connector_transaction_id: resource_id,
            connector_feature_data: connector_metadata.map(Secret::new),
            money: data.request.money.clone().ok_or("missing amount")?,
            ..Default::default()
        },
        response: Err(ErrorResponse::default()),
    })
}

fn verification_payment(data: &SetupData) -> Result<Value, Box<dyn Error>> {
    Ok(json!({
        "id": "payment_generated", "customer_token": "cus_test",
        "amount": 0, "original_amount": 0, "currency_code": "USD",
        "status": "ACT", "next_action": "pending_capture",
        "payment_method": "card_verified",
        "payment_method_data": {"network_reference_id": 123456},
        "authentication_result": {"result": "A", "eci": "05", "version": "2.2.0"},
        "metadata": wire(data)?.1.pointer("/payment_method/metadata").ok_or("missing card metadata")?
    }))
}

#[test]
fn setup_sync_finds_the_exact_verification_and_keeps_its_stable_reference() -> TestResult {
    let setup = setup_data()?;
    let data = sync_data(&setup)?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    check_eq(
        connector.get_url(&data)?,
        "https://sandboxapi.rapyd.net/v1/payments?customer=cus_test&limit=100",
    )?;
    let payment = verification_payment(&setup)?;
    let mut unrelated = payment.clone();
    *unrelated.get_mut("metadata").ok_or("missing metadata")? = json!({});
    let mapped = connector.handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""}, "data": [unrelated, payment]
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::Charged,
    )?;
    let PaymentsResponseData::TransactionResponse {
        resource_id,
        mandate_reference,
        network_txn_id,
        ..
    } = mapped.response.map_err(|_| "unexpected sync failure")?
    else {
        return Err("expected payment response".into());
    };
    check_eq(
        resource_id.get_connector_transaction_id()?,
        data.request
            .connector_transaction_id
            .get_connector_transaction_id()?,
    )?;
    check_eq(
        mandate_reference
            .ok_or("missing mandate")?
            .connector_mandate_id
            .as_deref(),
        Some("card_verified"),
    )?;
    check_eq(network_txn_id.as_deref(), Some("123456"))?;
    Ok(())
}

#[test]
fn zero_verification_uses_the_provider_currency_without_changing_the_setup_currency() -> TestResult
{
    let setup = setup_data()?;
    let data = sync_data(&setup)?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (next_action, result, expected) in [
        (
            "3d_verification",
            "R",
            common_enums::AttemptStatus::AuthenticationPending,
        ),
        ("pending_capture", "A", common_enums::AttemptStatus::Charged),
    ] {
        let mut payment = verification_payment(&setup)?;
        *payment.get_mut("currency_code").ok_or("missing currency")? = json!("ISK");
        *payment.get_mut("next_action").ok_or("missing action")? = json!(next_action);
        *payment
            .pointer_mut("/authentication_result/result")
            .ok_or("missing result")? = json!(result);
        let mapped = connector.handle_response_v2(
            &data,
            None,
            response(json!({
                "status": {"status": "SUCCESS", "error_code": ""}, "data": [payment]
            }))?,
        )?;
        check_eq(mapped.resource_common_data.status, expected)?;
        check_eq(mapped.request.money, data.request.money.clone())?;
        let PaymentsResponseData::TransactionResponse {
            mandate_reference, ..
        } = mapped
            .response
            .map_err(|_| "unexpected zero verification failure")?
        else {
            return Err("expected setup sync response".into());
        };
        check_eq(
            mandate_reference.is_some(),
            expected == common_enums::AttemptStatus::Charged,
        )?;
    }
    Ok(())
}

#[test]
fn setup_sync_does_not_promote_pending_or_failed_verification() -> TestResult {
    let setup = setup_data()?;
    let data = sync_data(&setup)?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    let mut payment = verification_payment(&setup)?;
    *payment.get_mut("next_action").ok_or("missing action")? = json!("3d_verification");
    *payment
        .pointer_mut("/authentication_result/result")
        .ok_or("missing result")? = json!("R");
    let mapped = connector.handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""}, "data": [payment.clone()]
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::AuthenticationPending,
    )?;
    let PaymentsResponseData::TransactionResponse {
        mandate_reference, ..
    } = mapped.response.map_err(|_| "unexpected sync failure")?
    else {
        return Err("expected response".into());
    };
    check(mandate_reference.is_none())?;
    *payment.get_mut("status").ok_or("missing status")? = json!("ERR");
    payment
        .as_object_mut()
        .ok_or("expected payment object")?
        .insert("failure_code".into(), json!("ERROR_CREATE_PAYMENT"));
    let mapped = connector.handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""}, "data": [payment]
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::Failure,
    )?;
    check_eq(
        mapped
            .response
            .err()
            .ok_or("missing verification failure")?
            .code,
        "ERROR_CREATE_PAYMENT",
    )?;
    Ok(())
}

#[test]
fn setup_sync_rejects_ambiguous_or_nonzero_verification_records() -> TestResult {
    let setup = setup_data()?;
    let data = sync_data(&setup)?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    let payment = verification_payment(&setup)?;
    for records in [
        vec![payment.clone(), payment.clone()],
        vec![payment.clone(); 100],
    ] {
        check(
            connector
                .handle_response_v2(
                    &data,
                    None,
                    response(json!({
                        "status": {"status": "SUCCESS", "error_code": ""}, "data": records
                    }))?,
                )
                .is_err(),
        )?;
    }
    for (field, value) in [
        ("amount", json!(1)),
        ("original_amount", json!(1)),
        ("currency_code", Value::Null),
    ] {
        let mut invalid = payment.clone();
        *invalid.get_mut(field).ok_or("missing verification field")? = value;
        check(
            connector
                .handle_response_v2(
                    &data,
                    None,
                    response(json!({
                        "status": {"status": "SUCCESS", "error_code": ""}, "data": [invalid]
                    }))?,
                )
                .is_err(),
        )?;
    }
    Ok(())
}

#[test]
fn external_sync_accepts_a_null_provider_authentication_result_only_for_external_setup(
) -> TestResult {
    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (setup, expected) in [
        (external_setup()?, common_enums::AttemptStatus::Charged),
        (
            setup_data()?,
            common_enums::AttemptStatus::AuthenticationPending,
        ),
    ] {
        let data = sync_data(&setup)?;
        let mut payment = verification_payment(&setup)?;
        *payment.get_mut("next_action").ok_or("missing action")? = json!("not_applicable");
        *payment
            .pointer_mut("/authentication_result/result")
            .ok_or("missing result")? = Value::Null;
        let mapped = connector.handle_response_v2(
            &data,
            None,
            response(json!({
                "status": {"status": "SUCCESS", "error_code": ""}, "data": [payment]
            }))?,
        )?;
        check_eq(mapped.resource_common_data.status, expected)?;
    }
    Ok(())
}

fn signed_webhook(
    body: Value,
) -> Result<domain_types::connector_types::RequestDetails, Box<dyn Error>> {
    use base64::Engine;
    let body = serde_json::to_vec(&body)?;
    let mut message =
        b"https://hooks.example.com/rapydtest_salt1700000000test_accesstest_secret".to_vec();
    message.extend_from_slice(&body);
    let signature = ring::hmac::sign(
        &ring::hmac::Key::new(ring::hmac::HMAC_SHA256, b"test_secret"),
        &message,
    );
    Ok(domain_types::connector_types::RequestDetails {
        method: domain_types::connector_types::HttpMethod::Post,
        uri: Some("https://hooks.example.com/rapyd".into()),
        headers: [
            ("salt".into(), "test_salt".into()),
            ("timestamp".into(), "1700000000".into()),
            (
                "signature".into(),
                base64::engine::general_purpose::URL_SAFE.encode(hex::encode(signature.as_ref())),
            ),
        ]
        .into(),
        body,
        query_params: None,
    })
}

#[test]
fn verified_hosted_webhook_uses_the_setup_reference_not_the_generated_payment_id() -> TestResult {
    use domain_types::connector_types::{
        ConnectorWebhookSecrets, EventType, WebhookResourceReference,
    };
    use interfaces::webhooks::IncomingWebhook;
    let data = setup_data()?;
    let marker = wire(&data)?
        .1
        .pointer("/payment_method/metadata")
        .ok_or("missing card metadata")?
        .clone();
    let connector = Rapyd::<DefaultPCIHolder>::new();
    let request =
        signed_webhook(json!({"type": "PAYMENT_UPDATED", "data": verification_payment(&data)?}))?;
    check(connector.verify_webhook_source(
        request.clone(),
        Some(ConnectorWebhookSecrets {
            secret: br#"{"access_key":"test_access","secret_key":"test_secret"}"#.to_vec(),
            additional_secret: None,
        }),
        None,
    )?)?;
    check_eq(
        connector.get_event_type(request.clone())?,
        EventType::PaymentIntentSuccess,
    )?;
    let Some(WebhookResourceReference::Payment(reference)) =
        connector.get_webhook_event_reference(request.clone())?
    else {
        return Err("missing setup event reference".into());
    };
    check_eq(
        reference.connector_transaction_id.as_deref(),
        marker.get("ucs_setup_attempt").and_then(Value::as_str),
    )?;
    check(reference.merchant_transaction_id.is_none())?;
    let response = connector.process_payment_webhook(request, None, None, None)?;
    check_eq(response.status, common_enums::AttemptStatus::Charged)?;
    check_eq(
        response
            .resource_id
            .ok_or("missing setup reference")?
            .get_connector_transaction_id()?,
        marker
            .get("ucs_setup_attempt")
            .and_then(Value::as_str)
            .ok_or("missing marker")?,
    )?;
    check_eq(
        response
            .mandate_reference
            .ok_or("missing verified card")?
            .connector_mandate_id
            .as_deref(),
        Some("card_verified"),
    )?;
    check_eq(response.network_txn_id.as_deref(), Some("123456"))?;
    Ok(())
}

#[test]
fn card_events_complete_only_external_registration_and_route_failures_without_a_card_id(
) -> TestResult {
    use domain_types::connector_types::{EventType, WebhookResourceReference};
    use interfaces::webhooks::IncomingWebhook;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (setup, event, card, action, expected) in [
        (
            setup_data()?,
            "CARD_ADDED_SUCCESSFULLY",
            Value::Null,
            "3d_verification",
            common_enums::AttemptStatus::AuthenticationPending,
        ),
        (
            external_setup()?,
            "CARD_ADDED_SUCCESSFULLY",
            json!("card_external"),
            "not_applicable",
            common_enums::AttemptStatus::Charged,
        ),
        (
            setup_data()?,
            "CARD_ADDED_FAILURE",
            Value::Null,
            "not_applicable",
            common_enums::AttemptStatus::Failure,
        ),
    ] {
        let marker = wire(&setup)?
            .1
            .pointer("/payment_method/metadata")
            .ok_or("missing card metadata")?
            .clone();
        let request = signed_webhook(json!({"type": event, "data": {
            "id": card, "next_action": action, "metadata": marker
        }}))?;
        let Some(WebhookResourceReference::Payment(reference)) =
            connector.get_webhook_event_reference(request.clone())?
        else {
            return Err("missing card-event reference".into());
        };
        check_eq(
            reference.connector_transaction_id.as_deref(),
            marker.get("ucs_setup_attempt").and_then(Value::as_str),
        )?;
        let response = connector.process_payment_webhook(request.clone(), None, None, None)?;
        check_eq(response.status, expected)?;
        check_eq(
            response.mandate_reference.is_some(),
            expected == common_enums::AttemptStatus::Charged,
        )?;
        if expected == common_enums::AttemptStatus::Failure {
            check_eq(
                connector.get_event_type(request)?,
                EventType::PaymentIntentFailure,
            )?;
            check(response.error_code.is_some())?;
        }
    }
    let request = signed_webhook(json!({"type": "CUSTOMER_CREATED", "data": {
        "id": "cus_pending", "default_payment_method": "card_unverified"
    }}))?;
    check_eq(
        connector.get_event_type(request.clone())?,
        EventType::IncomingWebhookEventUnspecified,
    )?;
    check(connector.get_webhook_event_reference(request)?.is_none())?;
    Ok(())
}

#[test]
fn malformed_or_failed_customer_registration_does_not_return_a_mandate() -> TestResult {
    let data = external_setup()?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    for body in [
        json!({"status": {"status": "SUCCESS", "error_code": ""}, "data": null}),
        json!({"status": {"status": "SUCCESS", "error_code": ""}, "data": {"id": "cus_test", "payment_methods": {"data": []}}}),
    ] {
        check(
            connector
                .handle_response_v2(&data, None, response(body)?)
                .is_err(),
        )?;
    }
    let mapped = connector.handle_response_v2(
        &data,
        None,
        response(json!({
            "status": {"status": "SUCCESS", "error_code": ""},
            "data": {"id": "cus_test", "payment_methods": {"data": [{
                "id": "card_failed", "next_action": "not_applicable",
                "failure_code": "CARD_VERIFICATION_FAILED", "failure_message": "Verification failed"
            }]}}
        }))?,
    )?;
    check_eq(
        mapped.resource_common_data.status,
        common_enums::AttemptStatus::Failure,
    )?;
    check_eq(
        mapped
            .response
            .err()
            .ok_or("missing card verification error")?
            .code,
        "CARD_VERIFICATION_FAILED",
    )?;
    Ok(())
}

#[test]
fn setup_events_with_conflicting_context_never_activate_a_card() -> TestResult {
    use interfaces::webhooks::IncomingWebhook;
    let setup = setup_data()?;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    let mut payment = verification_payment(&setup)?;
    *payment.get_mut("amount").ok_or("missing amount")? = json!(1);
    let request = signed_webhook(json!({"type": "PAYMENT_UPDATED", "data": payment}))?;
    check(connector.get_event_type(request).is_err())?;
    let mut payment = verification_payment(&setup)?;
    *payment
        .pointer_mut("/metadata/ucs_setup_mode")
        .ok_or("missing setup mode")? = json!("unknown_mode");
    let request = signed_webhook(json!({"type": "PAYMENT_UPDATED", "data": payment}))?;
    check(connector.get_webhook_event_reference(request).is_err())?;
    let request =
        signed_webhook(json!({"type": "PAYMENT_FAILED", "data": verification_payment(&setup)?}))?;
    let response = connector.process_payment_webhook(request, None, None, None)?;
    check_eq(response.status, common_enums::AttemptStatus::Failure)?;
    check(response.mandate_reference.is_none())?;
    Ok(())
}
