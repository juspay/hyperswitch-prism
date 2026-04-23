pub mod transformers;

use std::{
    fmt::Debug,
    marker::{Send, Sync},
};

use common_utils::{errors::CustomResult, events, ext_traits::XmlExt};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    BamboraapacAuthorizeResponse, BamboraapacCaptureRequest, BamboraapacCaptureResponse,
    BamboraapacErrorResponse, BamboraapacPSyncRequest, BamboraapacPSyncResponse,
    BamboraapacPaymentRequest, BamboraapacRSyncRequest, BamboraapacRSyncResponse,
    BamboraapacRefundRequest, BamboraapacRefundResponse, BamboraapacRepeatPaymentRequest,
    BamboraapacRepeatPaymentResponse, BamboraapacSetupMandateRequest,
    BamboraapacSetupMandateResponse,
};

use super::macros;
use super::macros::GetSoapXml;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// Type alias for non-generic trait implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_supported(
            self,
            "incremental_authorization",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ClientAuthentication for Bamboraapac<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Bamboraapac,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ConnectorServiceTrait<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentAuthorizeV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentSyncV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentCapture for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::RefundV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::RefundSyncV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::SetupMandateV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::RepeatPaymentV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentPreAuthenticateV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentAuthenticateV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentPostAuthenticateV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::SubmitEvidenceV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::DisputeDefend for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::AcceptDispute for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::IncomingWebhook for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentIncrementalAuthorization for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentVoidPostCaptureV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentVoidV2 for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentTokenV2<T> for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::CreateConnectorCustomer for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ServerAuthentication for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ServerSessionAuthentication for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentOrderCreate for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::VerifyRedirectResponse for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Bamboraapac<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ValidationTrait for Bamboraapac<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::MandateRevokeV2 for Bamboraapac<T>
{
}

// Create all prerequisites for the connector using macros
macros::create_all_prerequisites!(
    connector_name: Bamboraapac,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: BamboraapacPaymentRequest<T>,
            response_body: BamboraapacAuthorizeResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: BamboraapacPSyncRequest,
            response_body: BamboraapacPSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BamboraapacCaptureRequest,
            response_body: BamboraapacCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BamboraapacRefundRequest,
            response_body: BamboraapacRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: BamboraapacRSyncRequest,
            response_body: BamboraapacRSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: BamboraapacSetupMandateRequest,
            response_body: BamboraapacSetupMandateResponse,
            response_format: xml,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: BamboraapacRepeatPaymentRequest,
            response_body: BamboraapacRepeatPaymentResponse,
            response_format: xml,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "text/xml".to_string().into(),
                ),
            ];
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.bamboraapac.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.bamboraapac.base_url
        }
    }
);

// Implement helper methods for Bamboraapac
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> Bamboraapac<T> {
    /// Minimal preprocessing - only removes namespace prefixes
    /// Keeps full XML structure for deserialization
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _data: &RouterDataV2<F, FCD, Req, Res>,
        response_bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        use error_stack::ResultExt;

        let response_str = String::from_utf8(response_bytes.to_vec()).change_context(
            IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            },
        )?;

        // Only remove namespace prefixes for easier deserialization
        // Keep the full structure including Envelope
        let xml_response = response_str
            .replace("soap:", "")
            .replace(
                " xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"",
                "",
            )
            .replace(
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
                "",
            )
            .replace(" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\"", "")
            .replace(
                " xmlns=\"http://www.ippayments.com.au/interface/api/dts\"",
                "",
            );

        Ok(bytes::Bytes::from(xml_response))
    }
}

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Bamboraapac<T>
{
    fn id(&self) -> &'static str {
        "bamboraapac"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.bamboraapac.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Bambora APAC includes auth in the request body (SOAP), not headers
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        _event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: BamboraapacErrorResponse = if res.response.is_empty() {
            BamboraapacErrorResponse::default()
        } else {
            String::from_utf8(res.response.to_vec())
                .ok()
                .and_then(|s| s.parse_xml().ok())
                .unwrap_or_default()
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.unwrap_or_else(|| "UNKNOWN".to_string()),
            message: response
                .error_message
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string()),
            reason: response.error_message,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// Implement Authorize flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacPaymentRequest<T>),
    curl_response: BamboraapacAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_payments(req)))
        }
    }
);

// Implement PSync flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacPSyncRequest),
    curl_response: BamboraapacPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_payments(req)))
        }
    }
);

// Implement Capture flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacCaptureRequest),
    curl_response: BamboraapacCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_payments(req)))
        }
    }
);

// Implement Refund flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacRefundRequest),
    curl_response: BamboraapacRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_refunds(req)))
        }
    }
);

// Implement RSync flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacRSyncRequest),
    curl_response: BamboraapacRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_refunds(req)))
        }
    }
);

// Implement SetupMandate flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacSetupMandateRequest),
    curl_response: BamboraapacSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/sipp.asmx", self.connector_base_url_payments(req)))
        }
    }
);

// Implement RepeatPayment flow using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_error_response_v2, get_content_type],
    connector: Bamboraapac,
    curl_request: SoapXml(BamboraapacRepeatPaymentRequest),
    curl_response: BamboraapacRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dts.asmx", self.connector_base_url_payments(req)))
        }
    }
);

// Explicit not implemented flow placeholders

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "pre_authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "post_authenticate",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_supported(
            self,
            "accept_dispute",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SubmitEvidence,
            DisputeFlowData,
            SubmitEvidenceData,
            DisputeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_supported(
            self,
            "submit_evidence",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_supported(
            self,
            "defend_dispute",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "mandate_revoke",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self, "void",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "void_post_capture",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<T>,
            PaymentMethodTokenResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "payment_method_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_connector_customer",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        PaymentFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_server_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_server_session_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_order",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Bamboraapac<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(crate::utils::ConnectorFlowStatusExt::flow_not_implemented(
            self,
            "create_client_authentication_token",
        ))
    }
}
