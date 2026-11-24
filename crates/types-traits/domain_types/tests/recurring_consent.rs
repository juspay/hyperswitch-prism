//! Connector-independent consent contracts must preserve agreement state without
//! treating agreement approval as settlement of a payment.
#![allow(clippy::expect_used)]

use std::marker::PhantomData;

use common_enums::{AttemptStatus, PaymentMethod};
use common_utils::metadata::MaskedMetadata;
use domain_types::{
    connector_flow::{GenerateQr, PushNotification},
    connector_types::{
        GenerateQrRequestData, MandateReference, PaymentFlowData, PaymentsResponseData,
        PushNotificationRequestData,
    },
    pix_automatico::{generate_recurring_confirmation_response, generate_recurring_qr_response},
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    types::Connectors,
    utils::ForeignTryFrom,
};
use grpc_api_types::payments as proto;
use hyperswitch_masking::{PeekInterface, Secret};

const MANDATE: &str = "RR1234567820261123abcdefghijk";

fn state() -> proto::ConnectorState {
    proto::ConnectorState {
        access_token: Some(proto::AccessToken {
            token: Some(Secret::new("test_consent_token".into())),
            expires_in_seconds: Some(300),
            token_type: Some("Bearer".into()),
        }),
        connector_customer_id: Some("customer_consent".into()),
    }
}

fn qr_request() -> proto::RecurringPaymentServiceGetQrRequest {
    proto::RecurringPaymentServiceGetQrRequest {
        merchant_request_id: Some("merchant_consent_request".into()),
        connector_recurring_payment_id: MANDATE.into(),
        initial_transaction_id: Some("a12345678901234567890123456".into()),
        state: Some(state()),
        ..Default::default()
    }
}

fn confirmation_request() -> proto::RecurringPaymentServiceRequestConfirmationRequest {
    proto::RecurringPaymentServiceRequestConfirmationRequest {
        merchant_request_id: Some("merchant_consent_request".into()),
        connector_recurring_payment_id: MANDATE.into(),
        state: Some(state()),
        expires_at: "2030-12-01T10:00:00Z".into(),
        payer: Some(proto::PixAutomaticoPayerAccount {
            document_number: Some(Secret::new("12345678901".into())),
            document_type: proto::pix_automatico_payer_account::DocumentType::Cpf.into(),
            account_number: Some(Secret::new("123456".into())),
            bank_ispb: "12345678".into(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn common() -> PaymentFlowData {
    PaymentFlowData::foreign_try_from((
        qr_request(),
        Connectors::default(),
        &MaskedMetadata::default(),
    ))
    .expect("valid consent common data")
}

fn transaction(mandate: Option<&str>, redirect: Option<RedirectForm>) -> PaymentsResponseData {
    PaymentsResponseData::TransactionResponse {
        resource_id: domain_types::connector_types::ResponseId::ConnectorTransactionId(
            "confirmation_123".into(),
        ),
        redirection_data: redirect.map(Box::new),
        connector_metadata: Some(serde_json::json!({"consent_version": 2})),
        mandate_reference: mandate.map(|id| {
            Box::new(MandateReference {
                connector_mandate_id: Some(id.into()),
                payment_method_id: None,
                connector_mandate_request_reference_id: None,
                mandate_metadata: None,
            })
        }),
        network_txn_id: None,
        network_txn_link_id: None,
        connector_response_reference_id: Some("vendor_correlation_123".into()),
        incremental_authorization_allowed: None,
        splits: None,
        status_code: 201,
        payment_account_reference: None,
    }
}

fn qr_data(
    response: Result<PaymentsResponseData, ErrorResponse>,
    status: AttemptStatus,
) -> RouterDataV2<GenerateQr, PaymentFlowData, GenerateQrRequestData, PaymentsResponseData> {
    let mut common = common();
    common.status = status;
    RouterDataV2 {
        flow: PhantomData,
        resource_common_data: common,
        connector_config: ConnectorSpecificConfig::NoKey,
        request: GenerateQrRequestData::foreign_try_from(qr_request()).expect("valid QR request"),
        response,
    }
}

#[test]
fn consent_adapters_preserve_correlation_and_bearer_state_without_amounts() {
    let qr = GenerateQrRequestData::foreign_try_from(qr_request()).expect("QR request");
    assert_eq!(qr.connector_mandate_id, MANDATE);
    assert_eq!(
        qr.initial_transaction_id.as_deref(),
        Some("a12345678901234567890123456")
    );
    let push = PaymentFlowData::foreign_try_from((
        confirmation_request(),
        Connectors::default(),
        &MaskedMetadata::default(),
    ))
    .expect("valid push request");
    for data in [common(), push] {
        assert_eq!(data.payment_method, PaymentMethod::BankTransfer);
        assert_eq!(data.status, AttemptStatus::Pending);
        assert!(data.money.is_none());
        assert_eq!(
            data.connector_request_reference_id,
            "merchant_consent_request"
        );
        assert_eq!(
            data.merchant_request_id.as_deref(),
            Some("merchant_consent_request")
        );
        assert_eq!(data.connector_customer.as_deref(), Some("customer_consent"));
        let token = data.access_token.expect("bearer state preserved");
        assert_eq!(token.access_token.peek(), "test_consent_token");
        assert_eq!(token.expires_in, Some(300));
        assert_eq!(token.token_type.as_deref(), Some("Bearer"));
    }
}

#[test]
fn qr_response_preserves_payload_mandate_and_connector_state() {
    let response = generate_recurring_qr_response(qr_data(
        Ok(transaction(
            None,
            Some(RedirectForm::Qr {
                payload: Some("000201pix-consent".into()),
                image_base64: Some("cXJfaW1hZ2U=".into()),
                image_url: Some("https://example.test/qr".into()),
                fallback_url: Some("https://example.test/consent".into()),
                expires_at: Some(1_922_345_678),
            }),
        )),
        AttemptStatus::AuthenticationPending,
    ))
    .expect("valid QR response");
    assert_eq!(
        response.connector_recurring_payment_id.as_deref(),
        Some(MANDATE)
    );
    assert!(response.connector_confirmation_id.is_none());
    assert_eq!(
        response.status,
        i32::from(proto::PaymentStatus::AuthenticationPending)
    );
    assert_eq!(response.status_code, 201);
    let state = response.state.expect("state");
    assert_eq!(
        state.connector_customer_id.as_deref(),
        Some("customer_consent")
    );
    assert_eq!(
        state
            .access_token
            .expect("bearer")
            .token
            .expect("token")
            .peek(),
        "test_consent_token"
    );
    assert_eq!(
        response
            .connector_feature_data
            .expect("feature data")
            .peek(),
        "{\"consent_version\":2}"
    );
    match response
        .redirection_data
        .expect("redirect")
        .form_type
        .expect("QR form")
    {
        proto::redirect_form::FormType::QrData(qr) => {
            assert_eq!(qr.payload.as_deref(), Some("000201pix-consent"));
            assert_eq!(qr.image_base64.as_deref(), Some("cXJfaW1hZ2U="));
            assert_eq!(qr.image_url.as_deref(), Some("https://example.test/qr"));
            assert_eq!(
                qr.fallback_url.as_deref(),
                Some("https://example.test/consent")
            );
            assert_eq!(qr.expires_at, Some(1_922_345_678));
        }
        other => panic!("unexpected QR representation: {other:?}"),
    }
}

#[test]
fn confirmation_response_keeps_confirmation_and_agreement_ids_separate() {
    let mut common = common();
    common.status = AttemptStatus::Authorized;
    let data = RouterDataV2::<PushNotification, _, _, _> {
        flow: PhantomData,
        resource_common_data: common,
        connector_config: ConnectorSpecificConfig::NoKey,
        request: PushNotificationRequestData::foreign_try_from(confirmation_request())
            .expect("valid push"),
        response: Ok(transaction(Some("updated_mandate"), None)),
    };
    let response = generate_recurring_confirmation_response(data).expect("approved agreement");
    assert_eq!(
        response.connector_confirmation_id.as_deref(),
        Some("confirmation_123")
    );
    assert_eq!(
        response.connector_recurring_payment_id.as_deref(),
        Some("updated_mandate")
    );
    assert_eq!(response.status, i32::from(proto::PaymentStatus::Authorized));
}

#[test]
fn consent_rejects_cash_statuses_on_success_and_error_paths() {
    for status in [
        AttemptStatus::Charged,
        AttemptStatus::PartialCharged,
        AttemptStatus::CaptureInitiated,
    ] {
        assert!(
            generate_recurring_qr_response(qr_data(Ok(transaction(None, None)), status)).is_err()
        );
        assert!(
            generate_recurring_qr_response(qr_data(Err(ErrorResponse::default()), status)).is_err()
        );
        let error = ErrorResponse {
            attempt_status: Some(FlowStatus::Payment(status)),
            ..Default::default()
        };
        assert!(
            generate_recurring_qr_response(qr_data(Err(error), AttemptStatus::Pending)).is_err()
        );
    }
}

#[test]
fn consent_errors_preserve_connector_details_and_mandate() {
    let unclassified = generate_recurring_qr_response(qr_data(
        Err(ErrorResponse::default()),
        AttemptStatus::Pending,
    ))
    .expect("connector errors without an attempt status keep pending consent");
    assert_eq!(
        unclassified.status,
        i32::from(proto::PaymentStatus::Pending)
    );
    assert!(unclassified.error.is_some());
    let error = ErrorResponse {
        code: "CONSENT_REJECTED".into(),
        message: "Payer declined agreement".into(),
        reason: Some("bank app rejection".into()),
        status_code: 422,
        attempt_status: Some(FlowStatus::Payment(AttemptStatus::AuthenticationFailed)),
        ..Default::default()
    };
    let response =
        generate_recurring_qr_response(qr_data(Err(error), AttemptStatus::AuthenticationFailed))
            .expect("consent error response");
    assert_eq!(
        response.connector_recurring_payment_id.as_deref(),
        Some(MANDATE)
    );
    assert_eq!(response.status_code, 422);
    let details = response
        .error
        .expect("error")
        .connector_details
        .expect("connector details");
    assert_eq!(details.code.as_deref(), Some("CONSENT_REJECTED"));
    assert_eq!(details.message.as_deref(), Some("Payer declined agreement"));
}

fn mandate_settings() -> proto::PixAutomaticoMandateDetails {
    proto::PixAutomaticoMandateDetails {
        start_date: "2030-12-01".into(),
        frequency: proto::pix_automatico_mandate_details::Frequency::Monthly.into(),
        retry_policy: proto::pix_automatico_mandate_details::RetryPolicy::NoRetry.into(),
        ..Default::default()
    }
}

fn pix_instrument() -> domain_types::payment_method_data::PaymentMethodData<
    domain_types::payment_method_data::DefaultPCIHolder,
> {
    domain_types::payment_method_data::PaymentMethodData::BankTransfer(Box::new(
        domain_types::payment_method_data::BankTransferData::PixAutomaticoQr {},
    ))
}

fn setup_request() -> proto::PaymentServiceSetupRecurringRequest {
    proto::PaymentServiceSetupRecurringRequest {
        setup_future_usage: Some(proto::FutureUsage::OffSession.into()),
        payment_method: Some(proto::PaymentMethod {
            payment_method: Some(proto::payment_method::PaymentMethod::PixAutomaticoQr(
                proto::PixAutomaticoPayment {},
            )),
        }),
        amount: Some(proto::Money {
            minor_amount: 100,
            currency: proto::Currency::Brl.into(),
        }),
        pix_automatico: Some(mandate_settings()),
        ..Default::default()
    }
}

#[test]
fn pix_setup_preserves_typed_settings_without_card_acceptance() {
    use domain_types::{
        connector_types::SetupMandateRequestData, payment_method_data::DefaultPCIHolder,
    };
    let data = SetupMandateRequestData::<DefaultPCIHolder>::foreign_try_from((
        setup_request(),
        pix_instrument(),
    ))
    .expect("Pix approval happens in bank app");
    assert!(data.customer_acceptance.is_none());
    assert_eq!(
        data.pix_automatico.expect("typed settings").start_date,
        "2030-12-01"
    );
    let mut missing = setup_request();
    missing.pix_automatico = None;
    assert!(
        SetupMandateRequestData::<DefaultPCIHolder>::foreign_try_from((missing, pix_instrument()))
            .is_err()
    );
}

#[test]
fn legacy_setup_adapter_preserves_pix_contract_and_qr_location() {
    use domain_types::{
        connector_types::SetupMandateRequestData, payment_method_data::DefaultPCIHolder,
        types::SetupRecurringRequest,
    };
    let mut request = setup_request();
    request.session_token = Some("qr_location_123".into());
    request.address = Some(proto::PaymentAddress::default());
    let intermediate = SetupRecurringRequest::from(request);
    let common = PaymentFlowData::foreign_try_from((
        intermediate.clone(),
        Connectors::default(),
        &MaskedMetadata::default(),
    ))
    .expect("legacy setup common data");
    let data = SetupMandateRequestData::<DefaultPCIHolder>::foreign_try_from((
        intermediate,
        pix_instrument(),
    ))
    .expect("legacy Pix setup adapter");
    assert_eq!(
        data.pix_automatico.expect("settings").start_date,
        "2030-12-01"
    );
    assert_eq!(common.session_token.as_deref(), Some("qr_location_123"));
}

#[test]
fn card_setup_retains_acceptance_requirement_and_rejects_pix_configuration() {
    use domain_types::{
        connector_types::SetupMandateRequestData,
        payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData},
    };
    let mut request = setup_request();
    request.pix_automatico = None;
    let error = SetupMandateRequestData::<DefaultPCIHolder>::foreign_try_from((
        request,
        PaymentMethodData::Card(Card::default()),
    ))
    .expect_err("card acceptance remains required");
    assert!(matches!(
        error.current_context(),
        domain_types::errors::IntegrationError::MissingRequiredField {
            field_name: "customer_acceptance",
            ..
        }
    ));
    assert!(
        SetupMandateRequestData::<DefaultPCIHolder>::foreign_try_from((
            setup_request(),
            PaymentMethodData::Card(Card::default())
        ))
        .is_err()
    );
}

#[test]
fn session_token_request_preserves_bearer_and_connector_customer_state() {
    use domain_types::connector_types::ServerSessionAuthenticationTokenRequestData;
    let request = proto::MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
        state: Some(state()),
        domain_context: Some(proto::merchant_authentication_service_create_server_session_authentication_token_request::DomainContext::Payment(proto::PaymentSessionContext {
            amount: Some(proto::Money { minor_amount: 0, currency: proto::Currency::Brl.into() }),
            ..Default::default()
        })),
        ..Default::default()
    };
    let data = ServerSessionAuthenticationTokenRequestData::foreign_try_from(request)
        .expect("session context");
    assert_eq!(
        data.access_token.expect("bearer").access_token.peek(),
        "test_consent_token"
    );
    assert_eq!(
        data.connector_customer_id.as_deref(),
        Some("customer_consent")
    );
}

#[test]
fn pix_charge_preserves_receiver_settings_and_rejects_mismatched_instrument() {
    use domain_types::{
        connector_types::RepeatPaymentData,
        payment_method_data::{Card, DefaultPCIHolder, PaymentMethodData},
    };
    let request = proto::RecurringPaymentServiceChargeRequest {
        amount: Some(proto::Money {
            minor_amount: 100,
            currency: proto::Currency::Brl.into(),
        }),
        payment_method_type: Some(proto::PaymentMethodType::PixAutomaticoQr.into()),
        connector_recurring_payment_id: Some(proto::MandateReference {
            mandate_id_type: Some(proto::mandate_reference::MandateIdType::ConnectorMandateId(
                proto::ConnectorMandateReferenceId {
                    connector_mandate_id: Some(MANDATE.into()),
                    ..Default::default()
                },
            )),
        }),
        pix_automatico: Some(proto::PixAutomaticoChargeDetails {
            due_date: "2030-12-01".into(),
            receiver: Some(proto::PixAutomaticoReceiverAccount {
                account_number: Some(Secret::new("123456".into())),
                account_type: proto::pix_automatico_receiver_account::AccountType::Checking.into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let data = RepeatPaymentData::<DefaultPCIHolder>::foreign_try_from((
        request.clone(),
        Some(pix_instrument()),
    ))
    .expect("valid recurring charge");
    let settings = data.pix_automatico.expect("typed charge settings");
    assert_eq!(settings.due_date, "2030-12-01");
    assert_eq!(settings.receiver.account_number.peek(), "123456");
    assert!(RepeatPaymentData::<DefaultPCIHolder>::foreign_try_from((
        request.clone(),
        Some(PaymentMethodData::Card(Card::default()))
    ))
    .is_err());
    let mut missing = request;
    missing.pix_automatico = None;
    assert!(RepeatPaymentData::<DefaultPCIHolder>::foreign_try_from((
        missing,
        Some(pix_instrument())
    ))
    .is_err());
}
