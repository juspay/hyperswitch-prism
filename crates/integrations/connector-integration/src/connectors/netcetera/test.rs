use std::{error::Error, marker::PhantomData};

use common_utils::types::{MinorUnit, Money};
use domain_types::{
    connector_flow::{Authenticate, PostAuthenticate},
    connector_types::{
        PaymentFlowData, PaymentsAuthenticateData, PaymentsPostAuthenticateData,
        PaymentsResponseData,
    },
    errors::IntegrationError,
    payment_method_data::{
        Card, DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        VaultTokenHolder,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    utils::ForeignTryFrom,
};
use grpc_api_types::payments;
use hyperswitch_masking::{PeekInterface, Secret};
use interfaces::connector_integration_v2::ConnectorIntegrationV2;
use serde_json::json;

use super::{transformers::NetceteraAuthenticateRequest, Netcetera, NetceteraRouterData};

type AuthRouterData = RouterDataV2<
    Authenticate,
    PaymentFlowData,
    PaymentsAuthenticateData<DefaultPCIHolder>,
    PaymentsResponseData,
>;
type TestResult = Result<(), Box<dyn Error>>;

fn common_data() -> PaymentFlowData {
    PaymentFlowData {
        merchant_id: Default::default(),
        customer_id: None,
        connector_customer: None,
        payment_id: "pay_test".to_string(),
        attempt_id: "attempt_test".to_string(),
        status: common_enums::AttemptStatus::AuthenticationPending,
        payment_method: common_enums::PaymentMethod::Card,
        payment_method_type: None,
        description: None,
        return_url: Some("https://merchant.example/3ds/return".to_string()),
        address: domain_types::payment_address::PaymentAddress::new(None, None, None, None),
        auth_type: common_enums::AuthenticationType::ThreeDs,
        connector_feature_data: None,
        amount_captured: None,
        amount_capturable: None,
        amount: None,
        access_token: None,
        session_token: None,
        reference_id: None,
        connector_order_id: None,
        preprocessing_id: None,
        connector_api_version: None,
        connector_request_reference_id: "reference_test".to_string(),
        test_mode: None,
        connector_http_status_code: None,
        connector_response_headers: None,
        external_latency: None,
        connectors: domain_types::types::Connectors::default().into(),
        raw_connector_response: None,
        typed_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
        vault_headers: None,
        connector_response: None,
        recurring_mandate_payment_data: None,
        order_details: None,
        amount_authorized: None,
        l2_l3_data: None,
        merchant_request_id: None,
        sender_payment_instrument_id: None,
        connector_returned_payment_method_details: None,
        settlement_status: None,
        raw_connector_status: None,
    }
}

fn config(certificate: Option<&str>, private_key: Option<&str>) -> ConnectorSpecificConfig {
    ConnectorSpecificConfig::Netcetera {
        certificate: certificate.map(|value| Secret::new(value.to_string())),
        private_key: private_key.map(|value| Secret::new(value.to_string())),
        base_url: None,
        merchant_configuration_id: Some("merchant_configuration".to_string()),
        three_ds_requestor_id: Some("requestor_id".to_string()),
        three_ds_requestor_name: Some("requestor_name".to_string()),
    }
}

fn proto_auth() -> payments::PaymentMethodAuthenticationServiceAuthenticateRequest {
    payments::PaymentMethodAuthenticationServiceAuthenticateRequest {
        amount: Some(payments::Money {
            minor_amount: 2999,
            currency: payments::Currency::Usd.into(),
        }),
        ..Default::default()
    }
}

fn auth_data(
    proto: payments::PaymentMethodAuthenticationServiceAuthenticateRequest,
) -> Result<AuthRouterData, String> {
    let card = Card::<DefaultPCIHolder> {
        card_number: RawCardNumber(
            "4242424242424242"
                .parse()
                .map_err(|error| format!("{error:?}"))?,
        ),
        card_exp_month: Secret::new("12".to_string()),
        card_exp_year: Secret::new("2030".to_string()),
        card_cvc: Secret::new("123".to_string()),
        ..Default::default()
    };
    Ok(RouterDataV2 {
        flow: PhantomData,
        resource_common_data: common_data(),
        connector_config: config(None, None),
        request: PaymentsAuthenticateData::foreign_try_from((
            proto,
            Some(PaymentMethodData::Card(card)),
        ))
        .map_err(|error| format!("{error:?}"))?,
        response: Err(ErrorResponse::default()),
    })
}

fn wire_request(data: AuthRouterData) -> Result<serde_json::Value, String> {
    let request = NetceteraAuthenticateRequest::try_from(NetceteraRouterData {
        connector: (*Netcetera::<DefaultPCIHolder>::new()).clone(),
        router_data: data,
    })
    .map_err(|error| format!("{error:?}"))?;
    serde_json::to_value(request).map_err(|error| error.to_string())
}

fn post_auth_data<T: PaymentMethodDataTypes>(
    config: ConnectorSpecificConfig,
) -> RouterDataV2<
    PostAuthenticate,
    PaymentFlowData,
    PaymentsPostAuthenticateData<T>,
    PaymentsResponseData,
> {
    RouterDataV2 {
        flow: PhantomData,
        resource_common_data: common_data(),
        connector_config: config,
        request: PaymentsPostAuthenticateData {
            payment_method_data: None,
            amount: Money {
                amount: MinorUnit::new(2999),
                currency: common_enums::Currency::USD,
            },
            email: None,
            currency: Some(common_enums::Currency::USD),
            payment_method_type: None,
            router_return_url: None,
            continue_redirection_url: None,
            browser_info: None,
            enrolled_for_3ds: true,
            redirect_response: None,
            capture_method: None,
            connector_order_reference_id: None,
        },
        response: Err(ErrorResponse::default()),
    }
}

#[test]
fn browser_defaults_and_absent_sdk_are_preserved() -> TestResult {
    let mut data = auth_data(proto_auth())?;
    data.request.browser_info = Some(Default::default());
    let request = wire_request(data)?;
    assert_eq!(request["deviceChannel"], "02");
    assert_eq!(request["messageCategory"], "01");
    assert_eq!(request["threeDSCompInd"], "U");
    assert_eq!(
        request["threeDSRequestor"]["threeDSRequestorAuthenticationInd"],
        "01"
    );
    assert!(request["threeDSRequestor"]["threeDSRequestorChallengeInd"].is_null());
    assert!(!request["browserInformation"].is_null());
    assert!(request["sdkInformation"].is_null());
    Ok(())
}

#[test]
fn merchant_acquirer_and_notification_urls_keep_their_wire_values() -> TestResult {
    let mut proto = proto_auth();
    proto.merchant_details = Some(payments::MerchantDetails {
        merchant_category_code: Some(742),
        merchant_name: Some("Test merchant".to_string()),
        merchant_country_code: Some(payments::CountryAlpha2::Us.into()),
        ..Default::default()
    });
    proto.acquirer_details = Some(payments::AcquirerDetails {
        acquirer_bin: Some("438309".to_string()),
        acquirer_merchant_id: Some("00002000000".to_string()),
        acquirer_country_code: Some(payments::CountryAlpha2::Us.into()),
    });
    proto.webhook_url = Some("https://merchant.example/3ds/results".to_string());
    proto.threeds_completion_indicator = Some(payments::ThreeDsCompletionIndicator::Success.into());
    let request = wire_request(auth_data(proto)?)?;
    assert_eq!(
        request["merchant"],
        json!({
            "merchantConfigurationId": "merchant_configuration",
            "mcc": "0742", "merchantCountryCode": "840", "merchantName": "Test merchant",
            "notificationURL": "https://merchant.example/3ds/return",
            "threeDSRequestorId": "requestor_id", "threeDSRequestorName": "requestor_name",
            "resultsResponseNotificationUrl": "https://merchant.example/3ds/results"
        })
    );
    assert_eq!(
        request["acquirer"],
        json!({"acquirerBin": "438309", "acquirerMerchantId": "00002000000", "acquirerCountryCode": "840"})
    );
    assert_eq!(
        request["threeDSRequestorURL"],
        "https://merchant.example/3ds/return"
    );
    assert_eq!(request["threeDSCompInd"], "Y");
    Ok(())
}

#[test]
fn challenge_and_authentication_indicators_are_forwarded() -> TestResult {
    for (indicator, expected) in [
        (
            payments::ThreeDsRequestorChallengeIndicator::ChallengeMandated,
            "04",
        ),
        (
            payments::ThreeDsRequestorChallengeIndicator::NoChallengeTra,
            "05",
        ),
    ] {
        let mut proto = proto_auth();
        proto.three_ds_requestor_challenge_indicator = Some(indicator.into());
        proto.three_ds_requestor_authentication_indicator =
            Some(payments::ThreeDsRequestorAuthenticationIndicator::BillingAgreement.into());
        let request = wire_request(auth_data(proto)?)?;
        assert_eq!(
            request["threeDSRequestor"]["threeDSRequestorChallengeInd"],
            expected
        );
        assert_eq!(
            request["threeDSRequestor"]["threeDSRequestorAuthenticationInd"],
            "07"
        );
    }
    Ok(())
}

#[test]
fn app_without_sdk_information_fails_before_request_encoding() -> TestResult {
    let mut proto = proto_auth();
    proto.device_channel = Some(payments::DeviceChannel::App.into());
    let result = NetceteraAuthenticateRequest::try_from(NetceteraRouterData {
        connector: (*Netcetera::<DefaultPCIHolder>::new()).clone(),
        router_data: auth_data(proto)?,
    });
    assert!(matches!(
        result.as_ref().err().map(|error| error.current_context()),
        Some(IntegrationError::MissingRequiredField {
            field_name: "sdk_information",
            ..
        })
    ));
    Ok(())
}

#[test]
fn three_ri_is_explicitly_unsupported() -> TestResult {
    let mut proto = proto_auth();
    proto.device_channel = Some(payments::DeviceChannel::ThreeRi.into());
    let result = NetceteraAuthenticateRequest::try_from(NetceteraRouterData {
        connector: (*Netcetera::<DefaultPCIHolder>::new()).clone(),
        router_data: auth_data(proto)?,
    });
    assert!(matches!(
        result.as_ref().err().map(|error| error.current_context()),
        Some(IntegrationError::NotSupported {
            connector: "netcetera",
            ..
        })
    ));
    Ok(())
}

#[test]
fn optional_enum_omission_differs_from_invalid_explicit_values() -> TestResult {
    assert!(auth_data(proto_auth()).is_ok());
    for raw in [0, i32::MAX] {
        let mut proto = proto_auth();
        proto.device_channel = Some(raw);
        assert!(auth_data(proto).is_err());
        let mut proto = proto_auth();
        proto.three_ds_requestor_challenge_indicator = Some(raw);
        assert!(auth_data(proto).is_err());
        let mut proto = proto_auth();
        proto.three_ds_requestor_authentication_indicator = Some(raw);
        assert!(auth_data(proto).is_err());
        let mut proto = proto_auth();
        proto.message_category = Some(raw);
        assert!(auth_data(proto).is_err());
    }
    Ok(())
}

#[test]
fn post_authenticate_vault_proxy_does_not_require_a_local_certificate() -> TestResult {
    let data = post_auth_data::<VaultTokenHolder>(config(None, None));
    let connector = Netcetera::<VaultTokenHolder>::new();
    let certificate = ConnectorIntegrationV2::get_certificate(connector, &data)
        .map_err(|error| format!("{error:?}"))?;
    let private_key = ConnectorIntegrationV2::get_certificate_key(connector, &data)
        .map_err(|error| format!("{error:?}"))?;
    assert!(certificate.is_none());
    assert!(private_key.is_none());
    Ok(())
}

#[test]
fn post_authenticate_direct_calls_require_both_credentials() {
    let connector = Netcetera::<DefaultPCIHolder>::new();
    for cfg in [
        config(None, None),
        config(Some("certificate"), None),
        config(None, Some("key")),
    ] {
        let data = post_auth_data::<DefaultPCIHolder>(cfg);
        assert!(ConnectorIntegrationV2::get_certificate(connector, &data).is_err());
        assert!(ConnectorIntegrationV2::get_certificate_key(connector, &data).is_err());
    }
}

#[test]
fn post_authenticate_preserves_a_supplied_certificate_pair() -> TestResult {
    let data = post_auth_data::<DefaultPCIHolder>(config(Some("certificate"), Some("key")));
    let connector = Netcetera::<DefaultPCIHolder>::new();
    let certificate = ConnectorIntegrationV2::get_certificate(connector, &data)
        .map_err(|error| format!("{error:?}"))?;
    let private_key = ConnectorIntegrationV2::get_certificate_key(connector, &data)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        certificate.as_ref().map(|value| value.peek().as_str()),
        Some("certificate")
    );
    assert_eq!(
        private_key.as_ref().map(|value| value.peek().as_str()),
        Some("key")
    );
    Ok(())
}

fn sdk_information() -> payments::ThreeDsSdkInformation {
    payments::ThreeDsSdkInformation {
        sdk_app_id: "app_test".to_string(),
        sdk_enc_data: Some("encrypted_data".to_string().into()),
        sdk_ephem_pub_key: Some(payments::ThreeDsSdkEphemeralPublicKey {
            kty: "EC".to_string(),
            crv: "P-256".to_string(),
            x: "coordinate_x".to_string(),
            y: "coordinate_y".to_string(),
        }),
        sdk_max_timeout: 5,
        sdk_reference_number: "sdk_reference".to_string(),
        sdk_trans_id: "sdk_transaction".to_string(),
        ..Default::default()
    }
}

#[test]
fn valid_app_request_keeps_sdk_data_and_omits_browser_information() -> TestResult {
    let mut proto = proto_auth();
    proto.device_channel = Some(payments::DeviceChannel::App.into());
    proto.sdk_information = Some(sdk_information());
    let mut data = auth_data(proto)?;
    data.request.browser_info = Some(Default::default());
    let request = wire_request(data)?;
    assert_eq!(request["deviceChannel"], "01");
    assert_eq!(request["enforcePreferredProtocolVersion"], true);
    assert!(request["browserInformation"].is_null());
    assert_eq!(request["sdkInformation"]["sdkAppID"], "app_test");
    assert_eq!(request["sdkInformation"]["sdkEncData"], "encrypted_data");
    assert_eq!(
        request["sdkInformation"]["sdkEphemPubKey"]["x"],
        "coordinate_x"
    );
    Ok(())
}

#[test]
fn sdk_conversion_rejects_missing_or_incomplete_ephemeral_keys() {
    for key in [
        None,
        Some(payments::ThreeDsSdkEphemeralPublicKey {
            kty: "EC".to_string(),
            crv: "P-256".to_string(),
            x: "coordinate_x".to_string(),
            y: String::new(),
        }),
    ] {
        let mut sdk = sdk_information();
        sdk.sdk_ephem_pub_key = key;
        let result = domain_types::connector_types::SdkInformation::foreign_try_from(sdk);
        assert!(matches!(
            result.as_ref().err().map(|error| error.current_context()),
            Some(IntegrationError::MissingRequiredField {
                field_name: "sdk_information.sdk_ephem_pub_key",
                ..
            })
        ));
    }
}

#[test]
fn sdk_conversion_rejects_empty_encrypted_data_and_explicit_unspecified_type() {
    let mut sdk = sdk_information();
    sdk.sdk_enc_data = Some(String::new().into());
    assert!(domain_types::connector_types::SdkInformation::foreign_try_from(sdk).is_err());
    let mut sdk = sdk_information();
    sdk.sdk_type = Some(0);
    assert!(domain_types::connector_types::SdkInformation::foreign_try_from(sdk).is_err());
}
