use super::macros;
pub mod transformers;
use crate::{types::ResponseRouterData, with_error_response_body};
use common_enums::{
    AttemptStatus, CaptureMethod, CardNetwork, CurrencyUnit, PaymentMethod, PaymentMethodType,
    RefundStatus,
};
use common_utils::{errors::CustomResult, events, StringMinorUnit};
use hyperswitch_masking::Maskable;
use serde_json::Value;
use std::{
    fmt::Debug,
    marker::{Send, Sync},
    sync::LazyLock,
};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        ConnectorSpecifications, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundsData,
        RefundsResponseData, RepeatPaymentData, SetupMandateRequestData,
        SupportedPaymentMethodsExt,
    },
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        self, CardSpecificFeatures, ConnectorInfo, Connectors, FeatureStatus, PaymentMethodDetails,
        PaymentMethodSpecificFeatures, SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, ConnectorValidation},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as zift, ZiftAuthPaymentsResponse, ZiftAuthPaymentsResponse as ZiftSetupMandateResponse,
    ZiftAuthPaymentsResponse as ZiftRepeatPaymentResponse, ZiftCaptureRequest, ZiftCaptureResponse,
    ZiftErrorResponse, ZiftPaymentsRequest, ZiftRefundRequest, ZiftRefundResponse,
    ZiftRepeatPaymentsRequest, ZiftSetupMandateRequest, ZiftSyncRequest, ZiftSyncResponse,
    ZiftVoidRequest, ZiftVoidResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

macros::macro_connector_payout_implementation!(
    connector: Zift,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Zift<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Zift<T>
{
    fn validate_psync_reference_id(
        &self,
        _data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        Ok(())
    }
}

macros::create_amount_converter_wrapper!(connector_name: Zift, amount_type: StringMinorUnit);

macros::create_all_prerequisites!(
    connector_name: Zift,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ZiftPaymentsRequest<T>,
            response_body: ZiftAuthPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: ZiftSyncRequest,
            response_body: ZiftSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: ZiftCaptureRequest,
            response_body: ZiftCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: ZiftSetupMandateRequest<T>,
            response_body: ZiftSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: ZiftVoidRequest,
            response_body: ZiftVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ZiftRefundRequest,
            response_body: ZiftRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: ZiftRepeatPaymentsRequest<T>,
            response_body: ZiftRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )

    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            let url_encoded_response: Value = serde_urlencoded::from_bytes(&bytes)
                    .change_context(crate::utils::response_deserialization_fail(status_code, "zift: response body did not match the expected format; confirm API version and connector documentation."))
                    .attach_printable("Failed to parse URL-encoded response from Zift")
                    ?;

            let json_bytes = serde_json::to_vec(&url_encoded_response)
                    .change_context(crate::utils::response_deserialization_fail(status_code, "zift: response body did not match the expected format; confirm API version and connector documentation."))
                    .attach_printable("Failed to convert URL-encoded response to JSON")
                    ?;

                Ok(bytes::Bytes::from(json_bytes))
        }
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string().into(),
        )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.zift.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.zift.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Zift<T>
{
    fn id(&self) -> &'static str {
        "zift"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.zift.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: ZiftErrorResponse = serde_urlencoded::from_bytes(&res.response)
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "zift: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.response_code,
            message: response.response_message.clone(),
            reason: Some(response.response_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

static ZIFT_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> = LazyLock::new(|| {
    let zift_supported_capture_methods = vec![
        CaptureMethod::Automatic,
        CaptureMethod::Manual,
        CaptureMethod::SequentialAutomatic,
    ];

    let zift_supported_card_network = vec![
        CardNetwork::AmericanExpress,
        CardNetwork::DinersClub,
        CardNetwork::JCB,
        CardNetwork::Mastercard,
        CardNetwork::Visa,
        CardNetwork::Discover,
    ];

    let mut zift_supported_payment_methods = SupportedPaymentMethods::new();

    zift_supported_payment_methods.add(
        PaymentMethod::Card,
        PaymentMethodType::Card,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: zift_supported_capture_methods.clone(),
            specific_features: Some(PaymentMethodSpecificFeatures::Card({
                CardSpecificFeatures {
                    three_ds: FeatureStatus::NotSupported,
                    no_three_ds: FeatureStatus::Supported,
                    supported_card_networks: zift_supported_card_network.clone(),
                }
            })),
        },
    );

    zift_supported_payment_methods
});

static ZIFT_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Zift",
    description: "Zift connector",
    connector_type: types::PaymentConnectorCategory::PaymentGateway,
};

static ZIFT_SUPPORTED_WEBHOOK_FLOWS: [common_enums::EventClass; 0] = [];

impl ConnectorSpecifications for Zift<DefaultPCIHolder> {
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&ZIFT_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&*ZIFT_SUPPORTED_PAYMENT_METHODS)
    }

    fn get_supported_webhook_flows(&self) -> Option<&'static [common_enums::EventClass]> {
        Some(&ZIFT_SUPPORTED_WEBHOOK_FLOWS)
    }
}

impl ConnectorValidation for Zift<DefaultPCIHolder> {
    fn validate_mandate_payment(
        &self,
        _pm_type: Option<PaymentMethodType>,
        pm_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> CustomResult<(), IntegrationError> {
        let connector = self.id();
        match pm_data {
            PaymentMethodData::Card(_) => Ok(()),
            _ => Err(IntegrationError::NotSupported {
                message: " mandate payment".to_string(),
                connector,
                context: Default::default(),
            }
            .into()),
        }
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftPaymentsRequest<T>),
    curl_response: ZiftAuthPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftSyncRequest),
    curl_response: ZiftSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
    }

    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftCaptureRequest),
    curl_response: ZiftCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftMandatePaymentRequest<T>),
    curl_response: ZiftSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftVoidRequest),
    curl_response: ZiftVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftRefundRequest),
    curl_response: ZiftRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Zift,
    curl_request: FormUrlEncoded(ZiftRepeatPaymentsRequest<T>),
    curl_response: ZiftRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>,IntegrationError>{
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}gates/xurl", self.connector_base_url_payments(req)))
        }
    }
);

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: Authorize,
    source: String,
    context: bool,
    params: [response_code, is_auto_capture],
    success: _ => [Authorized, Charged],
    failure: none,
    extractors: {
        request: PaymentsAuthorizeData<T>,
        response: ZiftAuthPaymentsResponse,
        source: |response| response.response_code.clone(),
        context: |request, _response| request.is_auto_capture(),
    },
    {
        use zift::ResponseCodeExt;
        match (response_code.is_approved(), is_auto_capture) {
            (true, true) => AttemptStatus::Charged,
            (true, false) => AttemptStatus::Authorized,
            _ if response_code.is_pending() => AttemptStatus::Pending,
            _ => AttemptStatus::Failure,
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: PSync,
    source: (zift::PaymentRequestType, zift::TransactionStatus),
    context: (),
    params: [transaction, _ctx],
    success: _ => [Authorized, Charged],
    failure: none,
    extractors: {
        request: PaymentsSyncData,
        response: ZiftSyncResponse,
        source: |response| (response.transaction_type.clone(), response.transaction_status),
        context: |_request, _response| (),
    },
    {
        match transaction {
            (zift::PaymentRequestType::Sale, zift::TransactionStatus::Processed) => {
                AttemptStatus::Charged
            }
            (
                zift::PaymentRequestType::Sale,
                zift::TransactionStatus::Pending | zift::TransactionStatus::InRebill,
            ) => AttemptStatus::Pending,
            (zift::PaymentRequestType::Sale, zift::TransactionStatus::Cancelled) => {
                AttemptStatus::Failure
            }
            (zift::PaymentRequestType::Auth, zift::TransactionStatus::Processed) => {
                AttemptStatus::Authorized
            }
            (
                zift::PaymentRequestType::Auth,
                zift::TransactionStatus::Pending | zift::TransactionStatus::InRebill,
            ) => AttemptStatus::Pending,
            (zift::PaymentRequestType::Auth, zift::TransactionStatus::Cancelled) => {
                AttemptStatus::Failure
            }
            (zift::PaymentRequestType::Capture, zift::TransactionStatus::Processed) => {
                AttemptStatus::Charged
            }
            (
                zift::PaymentRequestType::Capture,
                zift::TransactionStatus::Pending | zift::TransactionStatus::InRebill,
            ) => AttemptStatus::CaptureInitiated,
            (zift::PaymentRequestType::Capture, zift::TransactionStatus::Cancelled) => {
                AttemptStatus::CaptureFailed
            }
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: Capture,
    source: String,
    context: (),
    params: [response_code, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: PaymentsCaptureData,
        response: ZiftCaptureResponse,
        source: |response| response.response_code.clone(),
        context: |_request, _response| (),
    },
    {
        use zift::ResponseCodeExt;
        if response_code.is_approved() {
            AttemptStatus::Charged
        } else {
            AttemptStatus::CaptureFailed
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: SetupMandate,
    source: String,
    context: (),
    params: [response_code, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: SetupMandateRequestData<T>,
        response: ZiftSetupMandateResponse,
        source: |response| response.response_code.clone(),
        context: |_request, _response| (),
    },
    {
        use zift::ResponseCodeExt;
        if response_code.is_approved() {
            AttemptStatus::Charged
        } else if response_code.is_pending() {
            AttemptStatus::Pending
        } else {
            AttemptStatus::Failure
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: Void,
    source: String,
    context: (),
    params: [response_code, _ctx],
    success: _ => [Voided],
    failure: none,
    extractors: {
        request: PaymentVoidData,
        response: ZiftVoidResponse,
        source: |response| response.response_code.clone(),
        context: |_request, _response| (),
    },
    {
        use zift::ResponseCodeExt;
        if response_code.is_approved() {
            AttemptStatus::Voided
        } else {
            AttemptStatus::Failure
        }
    }
}

domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: Refund,
    source: String,
    context: (),
    params: [response_code, _ctx],
    success: _ => [Success],
    failure: none,
    extractors: {
        request: RefundsData,
        response: ZiftRefundResponse,
        source: |response| response.response_code.clone(),
        context: |_request, _response| (),
    },
    {
        use zift::ResponseCodeExt;
        if response_code.is_approved() {
            RefundStatus::Success
        } else if response_code.is_pending() {
            RefundStatus::Pending
        } else {
            RefundStatus::Failure
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Zift<T>,
    flow: RepeatPayment,
    source: String,
    context: bool,
    params: [response_code, is_auto_capture],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: RepeatPaymentData<T>,
        response: ZiftRepeatPaymentResponse,
        source: |response| response.response_code.clone(),
        context: |request, _response| request.is_auto_capture(),
    },
    {
        use zift::ResponseCodeExt;
        match (response_code.is_approved(), is_auto_capture) {
            (true, true) => AttemptStatus::Charged,
            (true, false) => AttemptStatus::Authorized,
            _ if response_code.is_pending() => AttemptStatus::Pending,
            _ => AttemptStatus::Failure,
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Zift,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        RSync,
        VoidPC,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
        ClientAuthenticationToken,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        CreateOrder,
        Accept,
        DefendDispute,
        SubmitEvidence,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
