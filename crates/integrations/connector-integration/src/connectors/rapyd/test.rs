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

#[test]
fn payment_webhook_returns_payment_state_and_preserves_routing() -> TestResult {
    use domain_types::connector_types::{EventType, ResponseId, WebhookResourceReference};
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    let body = json!({
        "type": "PAYMENT_COMPLETED",
        "data": {
            "id": "payment_test", "amount": 26.49, "currency_code": "USD",
            "status": "CLO", "next_action": "not_applicable",
            "merchant_reference_id": "attempt_test", "captured": true
        }
    });
    let request = webhook_request(serde_json::to_vec(&body)?);
    if connector.get_event_type(request.clone())? != EventType::PaymentIntentSuccess {
        return Err("expected a successful payment event".into());
    }
    let Some(WebhookResourceReference::Payment(reference)) =
        connector.get_webhook_event_reference(request.clone())?
    else {
        return Err("expected payment reference".into());
    };
    if reference.connector_transaction_id.as_deref() != Some("payment_test") {
        return Err("payment reference does not contain the provider payment ID".into());
    }
    if reference.merchant_transaction_id.is_some() {
        return Err("payment reference unexpectedly changes merchant routing".into());
    }
    let response = connector.process_payment_webhook(request, None, None, None)?;
    let Some(ResponseId::ConnectorTransactionId(id)) = response.resource_id else {
        return Err("expected payment ID".into());
    };
    if id != "payment_test" {
        return Err(format!("unexpected payment ID: {id}").into());
    }
    if response.status != common_enums::AttemptStatus::Charged {
        return Err(format!("unexpected payment status: {:?}", response.status).into());
    }
    if response.connector_request_reference_id.as_deref() != Some("attempt_test") {
        return Err("merchant payment reference was not preserved".into());
    }
    let amount = response.amount.ok_or("missing amount")?;
    if amount.amount().get_amount_as_i64() != 2649 {
        return Err("payment amount was not converted to minor units".into());
    }
    Ok(())
}

fn webhook_request(body: Vec<u8>) -> domain_types::connector_types::RequestDetails {
    domain_types::connector_types::RequestDetails {
        method: domain_types::connector_types::HttpMethod::Post,
        uri: Some("/webhooks/merchant/rapyd/mca_test?x=a%2Fb&z=1".to_string()),
        headers: [
            ("Host", "hooks.example.com"),
            ("Salt", "test_salt"),
            ("Timestamp", "1700000000"),
            ("Signature", "MDg0MGEyNDc2ZWMzNDM4MzZhNDgzMTRhMmJhNjNiMjExNzE3N2FmMDAwNzg1MGRkY2E2MzQyY2I3NWFlN2Q4NQ=="),
        ]
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .into(),
        body,
        query_params: Some("x=a%2Fb&z=1".to_string()),
    }
}

fn webhook_secrets() -> domain_types::connector_types::ConnectorWebhookSecrets {
    domain_types::connector_types::ConnectorWebhookSecrets {
        secret: br#"{"access_key":"test_access","secret_key":"test_secret"}"#.to_vec(),
        additional_secret: None,
    }
}

#[test]
fn refund_webhooks_keep_the_refund_and_parent_payment_distinct() -> TestResult {
    use domain_types::connector_types::{EventType, WebhookResourceReference};
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (event, expected_event, expected_status) in [
        (
            "REFUND_COMPLETED",
            EventType::RefundSuccess,
            common_enums::RefundStatus::Success,
        ),
        (
            "PAYMENT_REFUND_REJECTED",
            EventType::RefundFailure,
            common_enums::RefundStatus::Failure,
        ),
        (
            "PAYMENT_REFUND_FAILED",
            EventType::RefundFailure,
            common_enums::RefundStatus::Failure,
        ),
    ] {
        let request = webhook_request(serde_json::to_vec(&json!({
            "type": event,
            "data": {"id": "refund_test", "payment": "payment_test", "amount": 26.49,
                "currency": "USD", "status": "PENDING", "failure_reason": "declined"}
        }))?);
        if connector.get_event_type(request.clone())? != expected_event {
            return Err(format!("unexpected refund event type for {event}").into());
        }
        let Some(WebhookResourceReference::Refund(reference)) =
            connector.get_webhook_event_reference(request.clone())?
        else {
            return Err("expected refund reference".into());
        };
        if reference.connector_refund_id.as_deref() != Some("refund_test") {
            return Err("refund reference does not contain the provider refund ID".into());
        }
        if reference.connector_transaction_id.as_deref() != Some("payment_test") {
            return Err("refund reference does not contain the parent payment ID".into());
        }
        let response = connector.process_refund_webhook(request, None, None)?;
        if response.connector_refund_id.as_deref() != Some("refund_test") {
            return Err("refund response does not contain the provider refund ID".into());
        }
        if response.status != expected_status {
            return Err(format!(
                "unexpected refund status for {event}: {:?}",
                response.status
            )
            .into());
        }
    }
    Ok(())
}

#[test]
fn webhook_signature_authenticates_the_exact_callback_and_body() -> TestResult {
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    let request = webhook_request(
        " {\"type\":\"PAYMENT_COMPLETED\",\"data\":{\"id\":\"payment_test\"},\"note\":\"café\"}\n"
            .as_bytes()
            .to_vec(),
    );
    if !connector.verify_webhook_source(request.clone(), Some(webhook_secrets()), None)? {
        return Err("valid signature was not verified".into());
    }

    let mut absolute = request.clone();
    absolute.uri = Some(format!(
        "https://hooks.example.com{}",
        absolute.uri.as_deref().ok_or("missing URI")?
    ));
    absolute.headers.remove("Host");
    if !connector.verify_webhook_source(absolute, Some(webhook_secrets()), None)? {
        return Err("signature for the absolute callback URL was not verified".into());
    }

    let mut separate_query = request.clone();
    separate_query.uri = Some("/webhooks/merchant/rapyd/mca_test".to_string());
    if !connector.verify_webhook_source(separate_query, Some(webhook_secrets()), None)? {
        return Err("signature with separate query parameters was not verified".into());
    }

    let mut changed_url = request.clone();
    changed_url.uri = Some("/webhooks/merchant/rapyd/another_account?x=a%2Fb&z=1".to_string());
    if connector.verify_webhook_source(changed_url, Some(webhook_secrets()), None)? {
        return Err("signature verified after the callback URL changed".into());
    }

    let mut changed = request;
    changed.body.pop();
    if connector.verify_webhook_source(changed, Some(webhook_secrets()), None)? {
        return Err("signature verified after the raw body changed".into());
    }
    Ok(())
}

#[test]
fn webhook_verification_rejects_missing_or_invalid_inputs() -> TestResult {
    use interfaces::webhooks::IncomingWebhook;
    let connector = Rapyd::<DefaultPCIHolder>::new();
    let request = webhook_request(b"{}".to_vec());
    if connector
        .verify_webhook_source(request.clone(), None, None)
        .is_ok()
    {
        return Err("missing webhook secret was accepted".into());
    }
    for key in ["Host", "Salt", "Timestamp", "Signature"] {
        let mut missing = request.clone();
        missing.headers.remove(key);
        if connector
            .verify_webhook_source(missing, Some(webhook_secrets()), None)
            .is_ok()
        {
            return Err(format!("missing {key} header was accepted").into());
        }
    }
    for signature in ["not base64", "bm90IGhleA=="] {
        let mut invalid = request.clone();
        invalid
            .headers
            .insert("Signature".to_string(), signature.to_string());
        if connector
            .verify_webhook_source(invalid, Some(webhook_secrets()), None)
            .is_ok()
        {
            return Err("malformed signature was accepted".into());
        }
    }
    for secret in [
        b"not json".as_slice(),
        br#"{"access_key":"","secret_key":"test_secret"}"#,
    ] {
        let mut secrets = webhook_secrets();
        secrets.secret = secret.to_vec();
        if connector
            .verify_webhook_source(request.clone(), Some(secrets), None)
            .is_ok()
        {
            return Err("invalid webhook secret was accepted".into());
        }
    }
    Ok(())
}

#[test]
fn dispute_webhooks_use_the_token_and_currency_precision() -> TestResult {
    use domain_types::connector_types::{EventType, WebhookResourceReference};
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (event, status, expected_event, expected_status) in [
        (
            "PAYMENT_DISPUTE_CREATED",
            "ACT",
            EventType::DisputeOpened,
            common_enums::DisputeStatus::DisputeOpened,
        ),
        (
            "PAYMENT_DISPUTE_UPDATED",
            "RVW",
            EventType::DisputeChallenged,
            common_enums::DisputeStatus::DisputeChallenged,
        ),
        (
            "PAYMENT_DISPUTE_UPDATED",
            "LOS",
            EventType::DisputeLost,
            common_enums::DisputeStatus::DisputeLost,
        ),
        (
            "PAYMENT_DISPUTE_UPDATED",
            "WIN",
            EventType::DisputeWon,
            common_enums::DisputeStatus::DisputeWon,
        ),
    ] {
        for (currency, amount, expected_amount) in [
            ("USD", 26.49, "2649"),
            ("JPY", 26.0, "26"),
            ("BHD", 26.491, "26491"),
        ] {
            let request = webhook_request(serde_json::to_vec(&json!({
                "type": event,
                "data": {"id": "reserved_id", "token": "dispute_test", "original_transaction_id": "payment_test",
                    "status": status, "amount": amount, "currency": currency,
                    "dispute_reason_description": "service not provided", "created_at": 1700000000}
            }))?);
            if connector.get_event_type(request.clone())? != expected_event {
                return Err(format!("unexpected dispute event type for {event}/{status}").into());
            }
            let Some(WebhookResourceReference::Dispute(reference)) =
                connector.get_webhook_event_reference(request.clone())?
            else {
                return Err("expected dispute reference".into());
            };
            if reference.connector_dispute_id.as_deref() != Some("dispute_test") {
                return Err("dispute reference does not use the dispute token".into());
            }
            if reference.connector_transaction_id.as_deref() != Some("payment_test") {
                return Err("dispute reference does not contain the parent payment ID".into());
            }
            let response = connector.process_dispute_webhook(request, None, None)?;
            if response.dispute_id != "dispute_test" {
                return Err("dispute response does not use the dispute token".into());
            }
            if response.status != expected_status {
                return Err(format!("unexpected dispute status for {event}/{status}").into());
            }
            if serde_json::to_value(response.amount)? != json!(expected_amount) {
                return Err(format!("incorrect dispute amount for {currency}").into());
            }
            if response.dispute_message.as_deref() != Some("service not provided") {
                return Err("dispute reason description was not preserved".into());
            }
            if response.connector_reason_code.is_some() {
                return Err("dispute description was incorrectly used as a reason code".into());
            }
        }
    }
    Ok(())
}

#[test]
fn payment_webhooks_preserve_authorization_and_failure_details() -> TestResult {
    use domain_types::connector_types::EventType;
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    for (event, status, next_action, expected_event, expected_status) in [
        (
            "PAYMENT_CAPTURED",
            "CLO",
            "not_applicable",
            EventType::PaymentIntentSuccess,
            common_enums::AttemptStatus::Charged,
        ),
        (
            "PAYMENT_COMPLETED",
            "ACT",
            "pending_capture",
            EventType::PaymentIntentSuccess,
            common_enums::AttemptStatus::Authorized,
        ),
        (
            "PAYMENT_FAILED",
            "ERR",
            "not_applicable",
            EventType::PaymentIntentFailure,
            common_enums::AttemptStatus::Failure,
        ),
    ] {
        let request = webhook_request(serde_json::to_vec(&json!({
            "type": event, "data": {"id": "payment_test", "amount": 10,
                "currency_code": "USD", "status": status, "next_action": next_action,
                "failure_code": "DECLINED", "failure_message": "issuer declined",
                "payment_method": "card_saved", "payment_method_data": {"network_reference_id": "network_test"}}
        }))?);
        if connector.get_event_type(request.clone())? != expected_event {
            return Err(format!("unexpected payment event type for {event}").into());
        }
        let response = connector.process_payment_webhook(request, None, None, None)?;
        if response.status != expected_status {
            return Err(format!(
                "unexpected payment status for {event}: {:?}",
                response.status
            )
            .into());
        }
        if response.error_code.as_deref() != (status == "ERR").then_some("DECLINED") {
            return Err(format!("incorrect payment error code for {event}").into());
        }
        if response.network_txn_id.as_deref() != Some("network_test") {
            return Err("network transaction reference was not preserved".into());
        }
        if response
            .mandate_reference
            .and_then(|reference| reference.connector_mandate_id)
            .as_deref()
            != Some("card_saved")
        {
            return Err("saved payment method was not preserved".into());
        }
    }
    Ok(())
}

#[test]
fn unsupported_and_malformed_webhooks_do_not_produce_payment_updates() -> TestResult {
    use domain_types::connector_types::EventType;
    use interfaces::webhooks::IncomingWebhook;

    let connector = Rapyd::<DefaultPCIHolder>::new();
    for body in [
        json!({"type": "FUTURE_EVENT", "data": {"id": "unrelated"}}),
        json!({"type": "FUTURE_EVENT", "data": [1, 2, 3]}),
    ] {
        let request = webhook_request(serde_json::to_vec(&body)?);
        if connector.get_event_type(request.clone())? != EventType::IncomingWebhookEventUnspecified
        {
            return Err("unknown webhook was assigned a supported event type".into());
        }
        if connector
            .get_webhook_event_reference(request.clone())?
            .is_some()
        {
            return Err("unknown webhook was assigned a payment reference".into());
        }
        if connector
            .process_payment_webhook(request, None, None, None)
            .is_ok()
        {
            return Err("unknown webhook produced a payment update".into());
        }
    }
    for body in [b"not json".as_slice(), br#"{"data":{}}"#] {
        if connector
            .get_event_type(webhook_request(body.to_vec()))
            .is_ok()
        {
            return Err("malformed webhook was accepted".into());
        }
    }
    for event in [
        "PAYMENT_COMPLETED",
        "REFUND_COMPLETED",
        "PAYMENT_DISPUTE_UPDATED",
    ] {
        let request = webhook_request(serde_json::to_vec(&json!({"type":event,"data":{}}))?);
        if connector.get_webhook_event_reference(request).is_ok() {
            return Err(format!("malformed {event} payload was accepted").into());
        }
    }
    let unknown_dispute = webhook_request(serde_json::to_vec(&json!({
        "type": "PAYMENT_DISPUTE_UPDATED", "data": {
            "token": "dispute_test", "original_transaction_id": "payment_test",
            "status": "FUTURE_STATUS", "amount": 10, "currency": "USD",
            "dispute_reason_description": "unknown state"
        }
    }))?);
    if connector.get_event_type(unknown_dispute.clone())?
        != EventType::IncomingWebhookEventUnspecified
    {
        return Err("unknown dispute status was assigned a supported event type".into());
    }
    if connector
        .process_dispute_webhook(unknown_dispute, None, None)
        .is_ok()
    {
        return Err("unknown dispute status produced a dispute update".into());
    }
    Ok(())
}

#[test]
fn webhook_dispatch_returns_the_correct_public_response() -> TestResult {
    use crate::{types::ConnectorData, webhook_utils};
    use domain_types::connector_types::ConnectorEnum;
    use grpc::event_content::Content;

    for body in [
        json!({"type":"PAYMENT_COMPLETED", "data":{"id":"payment_test", "amount":26.49,
            "currency_code":"USD", "status":"CLO", "next_action":"not_applicable", "merchant_reference_id":"attempt_test"}}),
        json!({"type":"REFUND_COMPLETED", "data":{"id":"refund_test", "payment":"payment_test",
            "amount":26.49, "currency":"USD", "status":"COMPLETED"}}),
        json!({"type":"PAYMENT_DISPUTE_CREATED", "data":{"token":"dispute_test", "original_transaction_id":"payment_test",
            "amount":26.49, "currency":"USD", "status":"ACT", "dispute_reason_description":"service not provided"}}),
    ] {
        let connector_data =
            ConnectorData::<DefaultPCIHolder>::get_connector_by_name(&ConnectorEnum::Rapyd);
        let request = webhook_request(serde_json::to_vec(&body)?);
        let parsed = webhook_utils::parse_webhook_event(connector_data.clone(), request.clone())?;
        if parsed.reference.is_none() {
            return Err("parsed webhook has no resource reference".into());
        }
        let response = webhook_utils::process_webhook_event(
            connector_data,
            request,
            None,
            None,
            true,
            None,
            None,
        )?;
        if parsed.event_type != Some(response.event_type) {
            return Err("parsed and processed webhook event types differ".into());
        }
        if !response.source_verified {
            return Err("source verification result was not preserved".into());
        }
        if response
            .event_ack_response
            .ok_or("missing acknowledgment")?
            .status_code
            != 200
        {
            return Err("webhook acknowledgment does not return HTTP 200".into());
        }
        match response
            .event_content
            .and_then(|content| content.content)
            .ok_or("missing event content")?
        {
            Content::PaymentsResponse(payment) => {
                if body.get("type") != Some(&json!("PAYMENT_COMPLETED")) {
                    return Err("non-payment webhook produced payment content".into());
                }
                if payment.connector_transaction_id != "payment_test" {
                    return Err("public payment response has an incorrect payment ID".into());
                }
                if payment.merchant_transaction_id.as_deref() != Some("attempt_test") {
                    return Err("public payment response loses the merchant reference".into());
                }
                if payment.amount.ok_or("missing payment amount")?.minor_amount != 2649 {
                    return Err("public payment response has an incorrect minor amount".into());
                }
            }
            Content::RefundsResponse(refund) => {
                if body.get("type") != Some(&json!("REFUND_COMPLETED")) {
                    return Err("non-refund webhook produced refund content".into());
                }
                if refund.connector_refund_id != "refund_test" {
                    return Err("public refund response has an incorrect refund ID".into());
                }
            }
            Content::DisputesResponse(dispute) => {
                if body.get("type") != Some(&json!("PAYMENT_DISPUTE_CREATED")) {
                    return Err("non-dispute webhook produced dispute content".into());
                }
                if dispute.connector_dispute_id.as_deref() != Some("dispute_test") {
                    return Err("public dispute response has an incorrect dispute ID".into());
                }
            }
            _ => return Err("unexpected webhook content".into()),
        }
    }
    Ok(())
}

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
