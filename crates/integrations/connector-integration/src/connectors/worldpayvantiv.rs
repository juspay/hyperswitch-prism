pub mod transformers;

use base64::Engine;
use common_enums;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::{deserialize_xml_to_struct, BytesExt},
    request::RequestContent,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        ServerSessionAuthenticationToken, SetupMandate, SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, EventType,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RepeatPaymentData, RequestDetails, ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData, SubmitEvidenceData,
        WebhookDetailsResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        AcceptDispute, ClientAuthentication, ConnectorServiceTrait,
        CreateConnectorCustomer as CreateConnectorCustomerTrait, DisputeDefend, IncomingWebhook,
        MandateRevokeV2, PaymentAuthenticateV2, PaymentAuthorizeV2, PaymentCapture,
        PaymentOrderCreate, PaymentPostAuthenticateV2, PaymentPreAuthenticateV2, PaymentSyncV2,
        PaymentVoidPostCaptureV2, PaymentVoidV2, RefundSyncV2, RefundV2, RepeatPaymentV2,
        ServerSessionAuthentication, SetupMandateV2, SubmitEvidenceV2, ValidationTrait,
        VerifyRedirectResponse,
    },
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;

use self::transformers::{
    CnpOnlineResponse, VantivSyncResponse, WorldpayvantivAuthType, WorldpayvantivPaymentsRequest,
    BASE64_ENGINE,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_response_body};
use domain_types::errors::{ConnectorError, IntegrationError, WebhookError};
use error_stack::report;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

/// Helper function to unwrap JSON-wrapped XML responses
/// Helper function to unwrap JSON-wrapped XML responses.
/// Some responses might come as a JSON string containing XML, this function handles that case.
fn unwrap_json_wrapped_xml(
    response_bytes: &[u8],
    status_code: u16,
) -> CustomResult<String, ConnectorError> {
    let response_str = std::str::from_utf8(response_bytes)
        .change_context(utils::response_handling_fail_for_connector(
            status_code,
            "worldpayvantiv",
        ))
        .attach_printable("Failed to convert response bytes to UTF-8 string")?;

    // Handle JSON-wrapped XML response (response might be a JSON string containing XML)
    let xml_str = if response_str.trim().starts_with('"') {
        // Try to parse as JSON string first to unwrap the XML
        serde_json::from_str::<String>(response_str)
            .change_context(utils::response_handling_fail_for_connector(
                status_code,
                "worldpayvantiv",
            ))
            .attach_printable("Failed to parse JSON-wrapped XML response")?
    } else {
        response_str.to_string()
    };

    Ok(xml_str)
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentPreAuthenticateV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ClientAuthentication for Worldpayvantiv<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Worldpayvantiv,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentAuthenticateV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentPostAuthenticateV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorServiceTrait<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ServerSessionAuthentication for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    CreateConnectorCustomerTrait for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SetupMandateV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ValidationTrait for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    VerifyRedirectResponse for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SourceVerification for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    IncomingWebhook for Worldpayvantiv<T>
{
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(false) // WorldpayVantiv doesn't support webhooks
    }

    fn get_event_type(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        Err(report!(WebhookError::WebhooksNotImplemented {
            operation: "get_event_type",
        }))
    }

    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(report!(WebhookError::WebhooksNotImplemented {
            operation: "process_payment_webhook",
        }))
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(report!(WebhookError::WebhooksNotImplemented {
            operation: "process_refund_webhook",
        }))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    SubmitEvidenceV2 for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> DisputeDefend
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundSyncV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> AcceptDispute
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentIncrementalAuthorization for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    RepeatPaymentV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentOrderCreate for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentAuthorizeV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentSyncV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentVoidV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentVoidPostCaptureV2 for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentTokenV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::ServerAuthentication for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentCapture
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    MandateRevokeV2 for Worldpayvantiv<T>
{
}

// Basic connector implementation
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for Worldpayvantiv<T>
{
    fn id(&self) -> &'static str {
        "worldpayvantiv"
    }

    fn common_get_content_type(&self) -> &'static str {
        "text/xml"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.worldpayvantiv.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;

        with_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.response_code,
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }
}

// Define connector prerequisites for payment flows (XML-based)
// Group flows by unique request/response combinations to avoid duplicate templating structs
macros::create_all_prerequisites!(
    connector_name: Worldpayvantiv,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: WorldpayvantivPaymentsRequest<T>,
            response_body: CnpOnlineResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: VantivSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            // Convert XML responses to JSON format for the macro's JSON parser
            let xml_str = unwrap_json_wrapped_xml(&bytes, status_code)?;

            // Parse XML to struct, then serialize back to JSON
            if xml_str.trim().starts_with("<?xml") || xml_str.trim().starts_with("<") {
                // This is an XML response - convert to JSON
                let xml_response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str)
                    .change_context(utils::response_deserialization_fail(
                        status_code,
                        "worldpayvantiv: failed to parse XML response in preprocess"
                    ))
                    .attach_printable("Failed to parse XML response in preprocess")?;

                let json_bytes = serde_json::to_vec(&xml_response)
                    .change_context(utils::response_deserialization_fail(
                        status_code,
                        "worldpayvantiv: failed to serialize XML response to JSON in preprocess"
                    ))
                    .attach_printable("Failed to serialize XML response to JSON in preprocess")?;

                Ok(bytes::Bytes::from(json_bytes))
            } else {
                // This is already JSON or another format
                Ok(bytes)
            }
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // For XML-based flows (Authorize, Capture, Void, VoidPC, Refund, SetupMandate),
            // we don't send authorization header - it's included in the XML body
            Ok(vec![])
        }

        pub fn connector_base_url_payments<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            let base_url = &req.resource_common_data.connectors.worldpayvantiv.base_url;
            base_url.to_string()
        }

        pub fn connector_base_url_refunds<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.worldpayvantiv.base_url.to_string()
        }

        pub fn get_auth_header(
            &self,
            auth_type: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = WorldpayvantivAuthType::try_from(auth_type)?;
            let auth_key = format!("{}:{}", auth.user.peek(), auth.password.peek());
            let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
            Ok(vec![(
                headers::AUTHORIZATION.to_string(),
                auth_header.into(),
            )])
        }
    }
);

// Implement the specific flows
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayvantiv,
    curl_request: Xml(WorldpayvantivPaymentsRequest<T>),
    curl_response: CnpOnlineResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayvantiv,
    curl_response: VantivSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.get_auth_header(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let txn_id = req.request.get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            let secondary_base_url = req.resource_common_data.connectors.worldpayvantiv.secondary_base_url
                .as_ref()
                .unwrap_or(&req.resource_common_data.connectors.worldpayvantiv.base_url);
            Ok(format!(
                "{secondary_base_url}/reports/dtrPaymentStatus/{txn_id}"
            ))
        }
    }
);

// Manual implementations for flows that share request/response types
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
    for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_refunds(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.get_auth_header(&req.connector_config)
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<String, IntegrationError> {
        let refund_id = req.request.connector_refund_id.clone();
        let secondary_base_url = req
            .resource_common_data
            .connectors
            .worldpayvantiv
            .secondary_base_url
            .as_ref()
            .unwrap_or(&req.resource_common_data.connectors.worldpayvantiv.base_url);
        Ok(format!(
            "{secondary_base_url}/reports/dtrPaymentStatus/{refund_id}"
        ))
    }

    fn get_request_body(
        &self,
        _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        // GET request doesn't need a body
        Ok(None)
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ConnectorError,
    > {
        let response: VantivSyncResponse = res
            .response
            .parse_struct("VantivSyncResponse")
            .change_context(utils::response_handling_fail_for_connector(
                res.status_code,
                "worldpayvantiv",
            ))?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }
}

// SetupMandate (SetupRecurring) — zero-dollar Authorization that returns
// a cnpToken in the tokenResponse element. That token is surfaced as
// MandateReference.connector_mandate_id so subsequent RepeatPayments can
// charge the saved card.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

// RepeatPayment (MIT) — posts a cnpAPI <sale> using the saved cnpToken from
// SetupMandate plus `processingType=merchantInitiatedCOF` and the original
// NTI. Shares CnpOnlineRequest/CnpOnlineResponse with the other XML flows.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
    fn get_headers(
        &self,
        req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        Ok(Some(RequestContent::Xml(Box::new(request))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        if let Some(i) = event_builder {
            i.set_connector_response(&response)
        }
        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

// Empty implementations for dispute flows
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Worldpayvantiv<T>
{
}

// Empty implementations for order flows
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::ServerAuthenticationToken,
        PaymentFlowData,
        domain_types::connector_types::ServerAuthenticationTokenRequestData,
        domain_types::connector_types::ServerAuthenticationTokenResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        domain_types::connector_flow::PaymentMethodToken,
        PaymentFlowData,
        domain_types::connector_types::PaymentMethodTokenizationData<T>,
        domain_types::connector_types::PaymentMethodTokenResponse,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Worldpayvantiv<T>
{
}
