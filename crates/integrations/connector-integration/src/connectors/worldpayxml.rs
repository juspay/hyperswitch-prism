pub(crate) mod requests;
pub(crate) mod responses;
pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::{AttemptStatus, CaptureMethod, CurrencyUnit, RefundStatus};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, StringMinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{self as worldpayxml};

use requests::{
    WorldpayxmlCaptureRequest, WorldpayxmlPSyncRequest, WorldpayxmlPaymentsRequest,
    WorldpayxmlRSyncRequest, WorldpayxmlRefundRequest, WorldpayxmlRepeatPaymentRequest,
    WorldpayxmlSetupMandateRequest, WorldpayxmlVoidPCRequest, WorldpayxmlVoidRequest,
};
use responses::{
    WorldpayxmlAuthorizeResponse, WorldpayxmlCaptureResponse, WorldpayxmlRefundResponse,
    WorldpayxmlRepeatPaymentResponse, WorldpayxmlRsyncResponse, WorldpayxmlSetupMandateResponse,
    WorldpayxmlTransactionResponse, WorldpayxmlVoidPCResponse, WorldpayxmlVoidResponse,
};

use super::macros::{self, GetSoapXml};
use crate::{types::ResponseRouterData, utils, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

const CONTENT_TYPE_XML: &str = "text/xml";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

macros::create_amount_converter_wrapper!(connector_name: Worldpayxml, amount_type: StringMinorUnit);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Worldpayxml<T>
{
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        _completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        // Card 3DS starts with Cardinal device data collection; both the DDC return and
        // the challenge return re-enter Authorize, which branches on the redirect payload.
        // Wallets authorize directly: decrypted wallet tokens carry a network cryptogram,
        // so they are already authenticated. Google Pay FPAN 3DS is supported on the
        // granular path only, where the caller routes it through PreAuthenticate.
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
            && matches!(redirect_state, RedirectState::InitialRequest)
        {
            AuthenticationStep::PreAuthenticate
        } else {
            AuthenticationStep::Authorize
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Worldpayxml<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Worldpayxml<T>
{
}

macros::create_all_prerequisites!(
    connector_name: Worldpayxml,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: WorldpayxmlPaymentsRequest,
            response_body: WorldpayxmlAuthorizeResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: WorldpayxmlCaptureRequest,
            response_body: WorldpayxmlCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: WorldpayxmlVoidRequest,
            response_body: WorldpayxmlVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: WorldpayxmlPSyncRequest,
            response_body: WorldpayxmlTransactionResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: WorldpayxmlRefundRequest,
            response_body: WorldpayxmlRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: WorldpayxmlRSyncRequest,
            response_body: WorldpayxmlRsyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: WorldpayxmlSetupMandateRequest,
            response_body: WorldpayxmlSetupMandateResponse,
            response_format: xml,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: WorldpayxmlRepeatPaymentRequest,
            response_body: WorldpayxmlRepeatPaymentResponse,
            response_format: xml,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: WorldpayxmlVoidPCRequest,
            response_body: WorldpayxmlVoidPCResponse,
            response_format: xml,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayxml.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.worldpayxml.base_url
        }

        pub fn build_auth_header(
            &self,
            auth: worldpayxml::WorldpayxmlAuthType,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let credentials = format!("{}:{}",
                auth.api_username.expose(),
                auth.api_password.expose()
            );
            let encoded = BASE64_ENGINE.encode(credentials.as_bytes());
            Ok(vec![
                (headers::AUTHORIZATION.to_string(), format!("Basic {}", encoded).into_masked()),
            ])
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlPaymentsRequest),
    curl_response: WorldpayxmlAuthorizeResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            // The 3ds challenge-completion leg must reach the same Worldpay machine that
            // issued the challenge, so replay the cookie captured from that response.
            if worldpayxml::parse_worldpayxml_challenge_return(req.request.redirect_response.as_ref())
                .is_some()
            {
                let cookie =
                    worldpayxml::get_worldpayxml_cookie(req.request.connector_feature_data.as_ref())?;
                headers.push(("Cookie".to_string(), cookie.into_masked()));
            }
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlSetupMandateRequest),
    curl_response: WorldpayxmlSetupMandateResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlRepeatPaymentRequest),
    curl_response: WorldpayxmlRepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlCaptureRequest),
    curl_response: WorldpayxmlCaptureResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlVoidRequest),
    curl_response: WorldpayxmlVoidResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlPSyncRequest),
    curl_response: WorldpayxmlTransactionResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlRefundRequest),
    curl_response: WorldpayxmlRefundResponse,
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
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlRSyncRequest),
    curl_response: WorldpayxmlRsyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayxml,
    curl_request: SoapXml(WorldpayxmlVoidPCRequest),
    curl_response: WorldpayxmlVoidPCResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = worldpayxml::WorldpayxmlAuthType::try_from(&req.connector_config)?;
            let mut headers = vec![
                (headers::CONTENT_TYPE.to_string(), CONTENT_TYPE_XML.to_string().into()),
            ];
            headers.extend(self.build_auth_header(auth)?);
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Source verification implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> Worldpayxml<T> {
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        // WorldPay XML responses are kept as-is
        // The macros will handle XML deserialization using parse_xml()
        Ok(bytes)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Worldpayxml<T>
{
    fn id(&self) -> &'static str {
        "worldpayxml"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.worldpayxml.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = worldpayxml::WorldpayxmlAuthType::try_from(auth_type)?;
        self.build_auth_header(auth)
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: responses::WorldpayxmlErrorResponse = res
            .response
            .parse_struct("WorldpayxmlErrorResponse")
            .change_context(
                utils::response_deserialization_fail(
                    res.status_code,
                "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        match response {
            responses::WorldpayxmlErrorResponse::Standard(error_response) => {
                with_error_response_body!(event_builder, error_response);

                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_response
                        .code
                        .unwrap_or(common_utils::consts::NO_ERROR_CODE.to_string()),
                    message: error_response
                        .message
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: None,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Worldpayxml<T>
{
}

macros::macro_connector_local_flow_implementation!(
    connector: Worldpayxml,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    handle_response: worldpayxml::handle_pre_authenticate_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);

// Mirrors `map_worldpayxml_authorize_status`. A 3DS `challengeRequired` reply and an
// inquiry-level `<error>` reply are handled before the mapper runs in the Authorize TryFrom,
// so the extractor surfaces them as flags alongside the payment `lastEvent`. The mapper's
// `previous_status` retention for `Unknown`/out-of-journey events is not reachable from the
// request/response pair, so those collapse to `Pending`.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: Authorize,
    source: (bool, bool, Option<responses::WorldpayxmlLastEvent>),
    context: bool,
    params: [status, is_auto_capture],
    success: _ => [Authorized, Charged],
    failure: none,
    extractors: {
        request: PaymentsAuthorizeData<T>,
        response: WorldpayxmlAuthorizeResponse,
        source: |response| {
            let reply = &response.reply;
            let order_status = reply.order_status.as_ref();
            let error = reply.error.is_some()
                || order_status.is_some_and(|os| os.error.is_some());
            let challenge = order_status.is_some_and(|os| os.challenge_required.is_some());
            let last_event = order_status
                .and_then(|os| os.payment.as_ref())
                .map(|payment| payment.last_event);
            (error, challenge, last_event)
        },
        context: |request, _response| {
            request.capture_method != Some(CaptureMethod::Manual)
                && request.capture_method != Some(CaptureMethod::ManualMultiple)
        },
    },
    {
        match status {
            (true, _, _) => AttemptStatus::Failure,
            (false, true, _) => AttemptStatus::AuthenticationPending,
            (false, false, None) => AttemptStatus::Pending,
            (false, false, Some(last_event)) => match last_event {
                responses::WorldpayxmlLastEvent::Authorised => {
                    if is_auto_capture {
                        AttemptStatus::Charged
                    } else {
                        AttemptStatus::Authorized
                    }
                }
                responses::WorldpayxmlLastEvent::Refused
                | responses::WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
                responses::WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
                responses::WorldpayxmlLastEvent::Captured
                | responses::WorldpayxmlLastEvent::Settled
                | responses::WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
                responses::WorldpayxmlLastEvent::SentForAuthorisation => {
                    AttemptStatus::Authorizing
                }
                _ => AttemptStatus::Pending,
            },
        }
    }
}

domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: PSync,
    source: (bool, Option<responses::WorldpayxmlLastEvent>),
    context: bool,
    params: [status, is_auto_capture],
    success: _ => [Authorized, Charged, Voided],
    failure: none,
    extractors: {
        request: PaymentsSyncData,
        response: WorldpayxmlTransactionResponse,
        source: |response| match response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                let reply = &xml_response.reply;
                let order_status = reply.order_status.as_ref();
                let error = reply.error.is_some()
                    || order_status.is_some_and(|os| {
                        os.error.is_some() && os.payment.is_some()
                    });
                let last_event = order_status
                    .and_then(|os| os.payment.as_ref())
                    .map(|payment| payment.last_event);
                (error, last_event)
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                (false, Some(webhook_response.payment_status))
            }
        },
        context: |request, _response| {
            request.capture_method != Some(CaptureMethod::Manual)
                && request.capture_method != Some(CaptureMethod::ManualMultiple)
        },
    },
    {
        match status {
            (true, _) => AttemptStatus::Failure,
            (false, None) => AttemptStatus::Pending,
            (false, Some(last_event)) => match last_event {
                responses::WorldpayxmlLastEvent::Authorised => {
                    if is_auto_capture {
                        AttemptStatus::Charged
                    } else {
                        AttemptStatus::Authorized
                    }
                }
                responses::WorldpayxmlLastEvent::Refused
                | responses::WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
                responses::WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
                responses::WorldpayxmlLastEvent::Captured
                | responses::WorldpayxmlLastEvent::Settled
                | responses::WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
                responses::WorldpayxmlLastEvent::SentForAuthorisation => {
                    AttemptStatus::Authorizing
                }
                _ => AttemptStatus::Pending,
            },
        }
    }
}

// Mirrors `map_worldpayxml_setup_mandate_status`.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: SetupMandate,
    source: (bool, Option<responses::WorldpayxmlLastEvent>),
    context: (),
    params: [status, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: SetupMandateRequestData<T>,
        response: WorldpayxmlSetupMandateResponse,
        source: |response| {
            let reply = &response.reply;
            let order_status = reply.order_status.as_ref();
            let error = reply.error.is_some()
                || order_status.is_some_and(|os| os.error.is_some());
            let last_event = order_status
                .and_then(|os| os.payment.as_ref())
                .map(|payment| payment.last_event);
            (error, last_event)
        },
        context: |_request, _response| (),
    },
    {
        match status {
            (true, _) => AttemptStatus::Failure,
            (false, None) => AttemptStatus::Pending,
            (false, Some(last_event)) => match last_event {
                responses::WorldpayxmlLastEvent::Refused
                | responses::WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
                responses::WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
                responses::WorldpayxmlLastEvent::Authorised
                | responses::WorldpayxmlLastEvent::Captured
                | responses::WorldpayxmlLastEvent::Settled
                | responses::WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
                responses::WorldpayxmlLastEvent::SentForAuthorisation => {
                    AttemptStatus::Authorizing
                }
                _ => AttemptStatus::Pending,
            },
        }
    }
}

// Mirrors `map_worldpayxml_authorize_status` as used by the RepeatPayment TryFrom.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: RepeatPayment,
    source: (bool, Option<responses::WorldpayxmlLastEvent>),
    context: bool,
    params: [status, is_auto_capture],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: RepeatPaymentData<T>,
        response: WorldpayxmlRepeatPaymentResponse,
        source: |response| {
            let reply = &response.reply;
            let order_status = reply.order_status.as_ref();
            let error = reply.error.is_some()
                || order_status.is_some_and(|os| os.error.is_some());
            let last_event = order_status
                .and_then(|os| os.payment.as_ref())
                .map(|payment| payment.last_event);
            (error, last_event)
        },
        context: |request, _response| {
            request.capture_method != Some(CaptureMethod::Manual)
                && request.capture_method != Some(CaptureMethod::ManualMultiple)
        },
    },
    {
        match status {
            (true, _) => AttemptStatus::Failure,
            (false, None) => AttemptStatus::Pending,
            (false, Some(last_event)) => match last_event {
                responses::WorldpayxmlLastEvent::Authorised => {
                    if is_auto_capture {
                        AttemptStatus::Charged
                    } else {
                        AttemptStatus::Authorized
                    }
                }
                responses::WorldpayxmlLastEvent::Refused
                | responses::WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
                responses::WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
                responses::WorldpayxmlLastEvent::Captured
                | responses::WorldpayxmlLastEvent::Settled
                | responses::WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
                responses::WorldpayxmlLastEvent::SentForAuthorisation => {
                    AttemptStatus::Authorizing
                }
                _ => AttemptStatus::Pending,
            },
        }
    }
}

// WorldpayXML acknowledges a capture with `<captureReceived>`; completion is confirmed via
// PSync, so an ack maps to `CaptureInitiated` and a reply-level error to `CaptureFailed`,
// mirroring the Capture TryFrom.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: Capture,
    source: bool,
    context: (),
    params: [acknowledged, _ctx],
    success: _ => [Charged],
    failure: none,
    extractors: {
        request: PaymentsCaptureData,
        response: WorldpayxmlCaptureResponse,
        source: |response| {
            if response.reply.error.is_some() {
                false
            } else {
                response.reply.ok.is_some()
            }
        },
        context: |_request, _response| (),
    },
    {
        if acknowledged {
            AttemptStatus::CaptureInitiated
        } else {
            AttemptStatus::CaptureFailed
        }
    }
}

// WorldpayXML acknowledges a void with `<cancelReceived>`; completion is confirmed via PSync,
// so an ack maps to `VoidInitiated` and a reply-level error to `VoidFailed`, mirroring the
// Void TryFrom.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: Void,
    source: bool,
    context: (),
    params: [acknowledged, _ctx],
    success: _ => [Voided],
    failure: none,
    extractors: {
        request: PaymentVoidData,
        response: WorldpayxmlVoidResponse,
        source: |response| {
            if response.reply.error.is_some() {
                false
            } else {
                response.reply.ok.is_some()
            }
        },
        context: |_request, _response| (),
    },
    {
        if acknowledged {
            AttemptStatus::VoidInitiated
        } else {
            AttemptStatus::VoidFailed
        }
    }
}

// The VoidPC TryFrom confirms the post-capture void synchronously
// (`PostCaptureVoidStatus::Succeeded` ⇔ `VoidedPostCapture`, `Failed` ⇔ `Failure`).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: VoidPC,
    source: bool,
    context: (),
    params: [acknowledged, _ctx],
    success: _ => [VoidedPostCapture],
    failure: none,
    extractors: {
        request: PaymentsCancelPostCaptureData,
        response: WorldpayxmlVoidPCResponse,
        source: |response| {
            if response.reply.error.is_some() {
                false
            } else {
                response.reply.ok.is_some()
            }
        },
        context: |_request, _response| (),
    },
    {
        if acknowledged {
            AttemptStatus::VoidedPostCapture
        } else {
            AttemptStatus::Failure
        }
    }
}

// WorldpayXML acknowledges a refund with `<refundReceived>` — the refund stays `Pending`
// until RSync — while a reply-level error fails the refund in the Refund TryFrom.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: Refund,
    source: bool,
    context: (),
    params: [acknowledged, _ctx],
    success: _ => [Success],
    failure: none,
    extractors: {
        request: RefundsData,
        response: WorldpayxmlRefundResponse,
        source: |response| {
            if response.reply.error.is_some() {
                false
            } else {
                response.reply.ok.is_some()
            }
        },
        context: |_request, _response| (),
    },
    {
        if acknowledged {
            RefundStatus::Pending
        } else {
            RefundStatus::Failure
        }
    }
}

// Mirrors `map_worldpayxml_refund_status`, driven by the inquiry `lastEvent`. The mapper's
// `Unknown` retention of the previous refund status is mirrored through the request's
// `refund_status`.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Worldpayxml<T>,
    flow: RSync,
    source: (bool, Option<responses::WorldpayxmlLastEvent>),
    context: RefundStatus,
    params: [status, previous_status],
    success: _ => [Success],
    failure: none,
    extractors: {
        request: RefundSyncData,
        response: WorldpayxmlRsyncResponse,
        source: |response| match response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                let reply = &xml_response.reply;
                let order_status = reply.order_status.as_ref();
                let error = reply.error.is_some();
                let last_event = order_status
                    .and_then(|os| os.payment.as_ref())
                    .map(|payment| payment.last_event);
                (error, last_event)
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                (false, Some(webhook_response.payment_status))
            }
        },
        context: |request, _response| request.refund_status,
    },
    {
        match status {
            (true, _) => RefundStatus::Failure,
            (false, None) => RefundStatus::Pending,
            (false, Some(last_event)) => match last_event {
                responses::WorldpayxmlLastEvent::Refunded
                | responses::WorldpayxmlLastEvent::RefundedByMerchant => RefundStatus::Success,
                responses::WorldpayxmlLastEvent::SentForRefund
                | responses::WorldpayxmlLastEvent::RefundRequested
                | responses::WorldpayxmlLastEvent::SentForFastRefund
                | responses::WorldpayxmlLastEvent::Captured
                | responses::WorldpayxmlLastEvent::Settled => RefundStatus::Pending,
                responses::WorldpayxmlLastEvent::RefundFailed
                | responses::WorldpayxmlLastEvent::Expired => RefundStatus::Failure,
                responses::WorldpayxmlLastEvent::Unknown => previous_status,
                _ => RefundStatus::Pending,
            },
        }
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Worldpayxml,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        PostAuthenticate,
        Authenticate,
        SubmitEvidence,
        DefendDispute,
        PaymentMethodToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        MandateRevoke,
        CreateOrder,
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
    ],
);
