pub(crate) mod requests;
pub(crate) mod responses;
pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, StringMinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void, VoidPC,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData,
    },
    payment_method_data::{
        GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::{RedirectForm, Response},
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
        // Wallets authorize directly: the initial order arms additional3DSData itself.
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayxml<T>
{
    fn get_call_connector_action(&self) -> common_enums::CallConnectorAction {
        common_enums::CallConnectorAction::HandleResponseWithoutBuildRequest
    }

    fn build_request_v2(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        // No outbound call: the device-data-collection page is built locally in
        // `handle_response_v2` and the shopper's browser talks to Cardinal directly.
        Ok(None)
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        // Unreachable: build_request_v2 returns None, so the framework never asks for a URL.
        Err(IntegrationError::NotImplemented(
            "worldpayxml pre_authenticate makes no outbound call".to_string(),
            Default::default(),
        )
        .into())
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        _event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        // Fail before rendering the DDC page when the browser data the authorize leg
        // will need is absent, matching the hyperswitch PreAuthenticate validations.
        let browser_info =
            data.request
                .browser_info
                .as_ref()
                .ok_or(utils::response_handling_fail(
                    res.status_code,
                    "worldpayxml: browser_info is required for device data collection.",
                ))?;
        if browser_info.accept_header.is_none() {
            return Err(utils::response_handling_fail(
                res.status_code,
                "worldpayxml: browser_info.accept_header is required for device data collection.",
            )
            .into());
        }
        if browser_info.user_agent.is_none() {
            return Err(utils::response_handling_fail(
                res.status_code,
                "worldpayxml: browser_info.user_agent is required for device data collection.",
            )
            .into());
        }

        let bin = match &data.request.payment_method_data {
            Some(PaymentMethodData::Card(card)) => {
                card.card_number.peek().chars().take(6).collect::<String>()
            }
            Some(PaymentMethodData::Wallet(WalletData::GooglePay(gpay))) => {
                match &gpay.tokenization_data {
                    GpayTokenizationData::Decrypted(decrypt_data) => decrypt_data
                        .application_primary_account_number
                        .get_card_isin(),
                    GpayTokenizationData::Encrypted(_) => {
                        return Err(utils::response_handling_fail(
                            res.status_code,
                            "worldpayxml: device data collection needs the card bin; an encrypted google pay token does not carry it.",
                        )
                        .into())
                    }
                }
            }
            _ => {
                return Err(utils::response_handling_fail(
                    res.status_code,
                    "worldpayxml: device data collection is only supported for cards and decrypted google pay.",
                )
                .into())
            }
        };

        let (iss, org_unit_id, jwt_mac_key) = match &data.connector_config {
            ConnectorSpecificConfig::Worldpayxml {
                issuer_id: Some(issuer_id),
                organizational_unit_id: Some(organizational_unit_id),
                jwt_mac_key: Some(jwt_mac_key),
                ..
            } => (
                issuer_id.clone(),
                organizational_unit_id.clone(),
                jwt_mac_key.clone(),
            ),
            _ => {
                return Err(utils::response_handling_fail(
                    res.status_code,
                    "worldpayxml: issuer_id, organizational_unit_id and jwt_mac_key must be configured in the connector metadata for 3ds.",
                )
                .into())
            }
        };
        let iat =
            u64::try_from(time::OffsetDateTime::now_utc().unix_timestamp()).map_err(|_| {
                utils::response_handling_fail(
                    res.status_code,
                    "worldpayxml: system time is before the unix epoch.",
                )
            })?;
        let jwt = worldpayxml::sign_worldpayxml_jwt(
            &requests::WorldpayxmlDdcJwt {
                jti: uuid::Uuid::new_v4().to_string(),
                iat,
                iss,
                org_unit_id,
            },
            &jwt_mac_key,
            res.status_code,
        )?;

        let collect_base = data
            .resource_common_data
            .connectors
            .worldpayxml
            .secondary_base_url
            .as_deref()
            .ok_or_else(|| {
                utils::response_handling_fail(
                    res.status_code,
                    "worldpayxml: secondary_base_url must be configured for device data collection.",
                )
            })?;
        let collect_url = format!("{}/V2/Cruise/Collect", collect_base.trim_end_matches('/'));
        let html_data = worldpayxml::build_worldpayxml_ddc_page(&collect_url, &bin, &jwt);

        let mut router_data = data.clone();
        router_data.resource_common_data.status =
            common_enums::AttemptStatus::DeviceDataCollectionPending;
        router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            resource_id: None,
            authentication_data: None,
            redirection_data: Some(Box::new(RedirectForm::Html { html_data })),
            connector_response_reference_id: Some(
                data.resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            status_code: res.status_code,
        });
        Ok(router_data)
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
