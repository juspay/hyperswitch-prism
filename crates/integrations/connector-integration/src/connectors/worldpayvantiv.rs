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
        Authorize, Capture, IncrementalAuthorization, PSync, RSync, Refund, RepeatPayment, Void,
        VoidPC,
    },
    connector_types::{
        ConnectorSpecifications, ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails, WebhookDetailsResponse,
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
        ConnectorServiceTrait, PaymentAuthorizeV2, PaymentCapture, PaymentSyncV2,
        PaymentVoidPostCaptureV2, PaymentVoidV2, RefundSyncV2, RefundV2, RepeatPaymentV2,
        ValidationTrait, VerifyRedirectResponse,
    },
    decode::BodyDecoding,
    verification::SourceVerification,
    webhooks::IncomingWebhook,
};
use serde::Serialize;

use self::transformers::{
    CnpOnlineResponse, VantivSyncResponse, WorldpayvantivAuthType, WorldpayvantivAuthorizeRequest,
    WorldpayvantivPaymentsRequest, WorldpayvantivRepeatPaymentRequest,
    WorldpayvantivRepeatPaymentResponse, BASE64_ENGINE,
};

use super::macros::{self, GetSoapXml};
use crate::{finalize_connector_response, types::ResponseRouterData, utils, with_response_body};
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
    fn get_headers(
        &self,
        req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
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
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Xml(Box::new(request)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }
}

macros::macro_connector_payout_implementation!(
    connector: Worldpayvantiv,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorServiceTrait<T> for Worldpayvantiv<T>
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
        _event_context: Option<EventContext>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> RefundSyncV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    interfaces::connector_types::PaymentIncrementalAuthorization for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentAuthorizeV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    RepeatPaymentV2<T> for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentSyncV2
    for Worldpayvantiv<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        interfaces::connector_types::validate_psync_connector_transaction_id(data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> PaymentVoidV2
    for Worldpayvantiv<T>
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    PaymentVoidPostCaptureV2 for Worldpayvantiv<T>
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
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let xml_str = unwrap_json_wrapped_xml(&res.response, res.status_code)?;

        let response: CnpOnlineResponse = deserialize_xml_to_struct(&xml_str).change_context(
            utils::response_handling_fail_for_connector(res.status_code, "worldpayvantiv"),
        )?;

        with_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
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
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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
            request_body: WorldpayvantivAuthorizeRequest<T>,
            response_body: CnpOnlineResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: WorldpayvantivRepeatPaymentRequest<T>,
            response_body: WorldpayvantivRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
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
    curl_request: SoapXml(WorldpayvantivRepeatPaymentRequest<T>),
    curl_response: WorldpayvantivRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![("Content-Type".into(), self.common_get_content_type().into())])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Worldpayvantiv,
    curl_request: SoapXml(WorldpayvantivAuthorizeRequest<T>),
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![("Content-Type".into(), self.common_get_content_type().into())])
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Xml(Box::new(request)),
            typed,
        )))
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
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Xml(Box::new(request)),
            typed,
        )))
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
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Xml(Box::new(request)),
            typed,
        )))
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
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let request = WorldpayvantivPaymentsRequest::try_from(WorldpayvantivRouterData {
            router_data: req.clone(),
            connector: self.clone(),
        })?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            RequestContent::Xml(Box::new(request)),
            typed,
        )))
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
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
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
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
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
        finalize_connector_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }
}

// Explicit not implemented dispute flow placeholders

// Explicit not implemented order flow placeholders

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    ConnectorSpecifications for Worldpayvantiv<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Worldpayvantiv,
    generic_type: T,
    [PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        SetupMandate,
        Accept,
        SubmitEvidence,
        DefendDispute,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        MandateRevoke,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        ServerAuthenticationToken,
    ],
);

#[cfg(test)]
mod macro_tests {
    use super::*;
    use domain_types::payment_method_data::DefaultPCIHolder;
    use hyperswitch_masking::Secret;
    use transformers::{
        Authentication, CnpOnlineRequest, OrderSource, PaymentInfo, Sale, TokenData,
        TokenizationData,
    };

    fn authorize_data(
        capture_method: common_enums::CaptureMethod,
        amount: i64,
    ) -> RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        use domain_types::{
            payment_method_data::{Card, PaymentMethodData, RawCardNumber},
            utils::ForeignTryFrom,
        };
        use grpc_api_types::payments as proto;
        let mut request = PaymentsAuthorizeData::foreign_try_from((
            domain_types::types::AuthorizationRequest::from(
                proto::PaymentServiceAuthorizeRequest {
                    amount: Some(proto::Money {
                        minor_amount: amount,
                        currency: proto::Currency::Usd.into(),
                    }),
                    payment_method: Some(proto::PaymentMethod {
                        payment_method: Some(proto::payment_method::PaymentMethod::Card(
                            Default::default(),
                        )),
                    }),
                    ..Default::default()
                },
            ),
            PaymentMethodData::Card(Card::<DefaultPCIHolder> {
                card_number: RawCardNumber("4111111111111111".parse().unwrap()),
                card_exp_month: Secret::new("01".into()),
                card_exp_year: Secret::new("2030".into()),
                card_cvc: Secret::new("123".into()),
                ..Default::default()
            }),
        ))
        .unwrap();
        request.capture_method = Some(capture_method);
        let mut connectors = Connectors::default();
        connectors.worldpayvantiv.base_url = "https://vantiv.example/online".into();
        RouterDataV2 {
            flow: PhantomData,
            resource_common_data: PaymentFlowData {
                merchant_id: serde_json::from_value(serde_json::json!("merchant_123")).unwrap(),
                customer_id: None,
                connector_customer: None,
                payment_id: "payment_123".into(),
                attempt_id: "attempt_123".into(),
                status: common_enums::AttemptStatus::Pending,
                payment_method: common_enums::PaymentMethod::Card,
                payment_method_type: None,
                description: None,
                return_url: None,
                address: Default::default(),
                auth_type: common_enums::AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                captured_money: None,
                capturable_money: None,
                money: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "reference_123".into(),
                test_mode: None,
                connector_http_status_code: None,
                connector_response_headers: None,
                external_latency: None,
                connectors: std::sync::Arc::new(connectors),
                raw_connector_response: None,
                typed_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
                vault_headers: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                order_details: None,
                authorized_money: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                connector_returned_payment_method_details: None,
                settlement_status: None,
                raw_connector_status: None,
            },
            connector_config: ConnectorSpecificConfig::Worldpayvantiv {
                user: Secret::new("user&test".into()),
                password: Secret::new("secret_password".into()),
                merchant_id: Some(Secret::new("merchant_123".into())),
                merchant_id_by_currency: None,
                base_url: None,
                secondary_base_url: None,
                report_group: Some("test_group".into()),
                merchant_config_currency: None,
            },
            request,
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn authorize_soapxml_preserves_requests_responses_and_masking() {
        use common_enums::{AttemptStatus, CaptureMethod};
        for (capture, amount, element, prefix, status, failure) in [
            (
                CaptureMethod::Automatic,
                1000,
                "sale",
                "sale",
                AttemptStatus::Charged,
                AttemptStatus::Failure,
            ),
            (
                CaptureMethod::Manual,
                1000,
                "authorization",
                "auth",
                AttemptStatus::Authorized,
                AttemptStatus::AuthorizationFailed,
            ),
            (
                CaptureMethod::Automatic,
                0,
                "authorization",
                "sale",
                AttemptStatus::Charged,
                AttemptStatus::Failure,
            ),
        ] {
            let data = authorize_data(capture, amount);
            let connector = Worldpayvantiv::<DefaultPCIHolder>::new();
            let request = connector.build_request_v2(&data).unwrap().unwrap();
            assert_eq!(request.url, "https://vantiv.example/online");
            assert_eq!(request.method, common_utils::request::Method::Post);
            assert!(request
                .headers
                .contains(&("Content-Type".to_string(), "text/xml".to_string().into())));
            let xml = String::from_utf8(request.body.unwrap().get_body_bytes().unwrap().0.unwrap())
                .unwrap();
            assert!(xml.starts_with("<?xml version="));
            assert!(xml.contains(&format!("<{element} id=\"{prefix}_reference_123\"")));
            assert!(xml.contains(&format!("<amount>{amount}</amount>")));
            assert!(xml.contains("<number>4111111111111111</number>"));
            assert!(xml.contains("<expDate>0130</expDate>"));
            assert!(xml.contains("<cardValidationNum>123</cardValidationNum>"));
            assert!(xml.contains("<user>user&amp;test</user>"));
            assert!(!xml.contains("&amp;amp;"));
            assert!(!xml.contains("&lt;cnpOnlineRequest"));
            assert!(!xml.contains("Envelope"));
            macros::validate_xml_structure(&xml).unwrap();
            let typed_request =
                WorldpayvantivAuthorizeRequest::try_from(WorldpayvantivRouterData {
                    router_data: data.clone(),
                    connector: connector.clone(),
                })
                .unwrap();
            let masked =
                events::MaskedSerdeValue::from_masked_optional(&typed_request, "test_request")
                    .unwrap();
            let logged = masked.inner().to_string();
            assert!(!logged.contains("4111111111111111"));
            assert!(!logged.contains("secret_password"));
            assert!(!logged.contains("<?xml"));
            for (code, expected) in [("000", status), ("601", failure)] {
                let response_xml = format!(
                    r#"<cnpOnlineResponse version="12.30" response="0" message="Valid Format"><{element}Response id="reference_123" reportGroup="test_group"><cnpTxnId>txn_123</cnpTxnId><orderId>reference_123</orderId><response>{code}</response><message>Result</message><responseTime>2026-11-18T00:00:00</responseTime><networkTransactionId>ntid_123</networkTransactionId><tokenResponse><cnpToken>returned_token_123</cnpToken><tokenResponseCode>801</tokenResponseCode><tokenMessage>Registered</tokenMessage><type>VI</type><bin>411111</bin></tokenResponse></{element}Response></cnpOnlineResponse>"#
                );
                for body in [
                    response_xml.clone(),
                    serde_json::to_string(&response_xml).unwrap(),
                ] {
                    let result = connector
                        .handle_response_v2(
                            &data,
                            None,
                            Response {
                                headers: None,
                                response: body.into(),
                                status_code: 200,
                            },
                        )
                        .unwrap();
                    assert_eq!(result.resource_common_data.status, expected);
                    if code == "000" {
                        let PaymentsResponseData::TransactionResponse {
                            mandate_reference,
                            network_txn_id,
                            ..
                        } = result.response.unwrap()
                        else {
                            panic!("expected transaction response");
                        };
                        let mandate = mandate_reference.unwrap();
                        assert_eq!(
                            mandate.connector_mandate_id.as_deref(),
                            Some("returned_token_123")
                        );
                        assert_eq!(
                            mandate.mandate_metadata.unwrap().peek()["networkTransactionId"],
                            "ntid_123"
                        );
                        assert_eq!(network_txn_id.as_deref(), Some("ntid_123"));
                    } else {
                        assert_eq!(result.response.unwrap_err().code, "601");
                    }
                }
            }
        }
    }

    #[test]
    fn repeat_payment_macro_xml_preserves_payload_and_masks_logs() {
        let request =
            WorldpayvantivRepeatPaymentRequest::try_from(CnpOnlineRequest::<DefaultPCIHolder> {
                version: "12.30".into(),
                xmlns: "http://www.vantivcnp.com/schema".into(),
                merchant_id: Secret::new("merchant_123".into()),
                authentication: Authentication {
                    user: Secret::new("test_user".into()),
                    password: Secret::new("secret_password".into()),
                },
                authorization: None,
                sale: Some(Sale {
                    id: "sale_reference_123".into(),
                    report_group: "test_group".into(),
                    customer_id: None,
                    order_id: "reference_123".into(),
                    amount: MinorUnit::new(1000),
                    order_source: OrderSource::Ecommerce,
                    bill_to_address: None,
                    ship_to_address: None,
                    payment_info: PaymentInfo::Token(TokenData {
                        token: TokenizationData {
                            cnp_token: Secret::new("processor_token_123".into()),
                            exp_date: Secret::new("0130".into()),
                        },
                        original_network_transaction_id: Some(Secret::new("original_ntid".into())),
                    }),
                    enhanced_data: None,
                    processing_instructions: None,
                    cardholder_authentication: None,
                }),
                capture: None,
                auth_reversal: None,
                void: None,
                credit: None,
            })
            .unwrap();
        let xml = request.to_soap_xml();
        assert!(xml.starts_with("<?xml version="));
        assert!(xml.contains("<cnpOnlineRequest "));
        assert!(xml.contains("<sale "));
        assert!(xml.contains("<cnpToken>processor_token_123</cnpToken>"));
        assert!(xml.contains("<expDate>0130</expDate>"));
        assert!(xml.contains(
            "<originalNetworkTransactionId>original_ntid</originalNetworkTransactionId>"
        ));
        assert!(!xml.contains("&lt;cnpOnlineRequest"));
        assert!(!xml.contains("Envelope"));
        macros::validate_xml_structure(&xml).unwrap();
        let masked =
            events::MaskedSerdeValue::from_masked_optional(&request, "test_request").unwrap();
        let logged = masked.inner().to_string();
        assert!(logged.contains("cnpToken"));
        for secret in ["processor_token_123", "secret_password", "original_ntid"] {
            assert!(!logged.contains(secret));
        }
        assert!(!logged.contains("<?xml"));
    }

    #[test]
    fn repeat_payment_macro_preprocesses_plain_and_wrapped_xml() {
        let data = RouterDataV2::<(), (), (), ()> {
            flow: PhantomData,
            resource_common_data: (),
            connector_config: ConnectorSpecificConfig::Worldpayvantiv {
                user: Secret::new("test_user".into()),
                password: Secret::new("test_password".into()),
                merchant_id: Some(Secret::new("merchant_123".into())),
                merchant_id_by_currency: None,
                base_url: None,
                secondary_base_url: None,
                report_group: None,
                merchant_config_currency: None,
            },
            request: (),
            response: Ok(()),
        };
        let connector = Worldpayvantiv::<DefaultPCIHolder>::new();
        for element in ["saleResponse", "authorizationResponse"] {
            for code in ["000", "601"] {
                let xml = format!(
                    r#"<cnpOnlineResponse version="12.30" response="0" message="Valid Format"><{element} id="reference_123" reportGroup="test_group"><cnpTxnId>txn_123</cnpTxnId><orderId>reference_123</orderId><response>{code}</response><message>Result</message><responseTime>2026-11-18T00:00:00</responseTime></{element}></cnpOnlineResponse>"#
                );
                for body in [xml.clone(), serde_json::to_string(&xml).unwrap()] {
                    let bytes = connector
                        .preprocess_response_bytes(&data, body.into(), 200)
                        .unwrap();
                    let response: WorldpayvantivRepeatPaymentResponse =
                        serde_json::from_slice(&bytes).unwrap();
                    let response = response
                        .sale_response
                        .or(response.authorization_response)
                        .unwrap();
                    assert_eq!(response.response.to_string(), code);
                    assert_eq!(response.cnp_txn_id, "txn_123");
                }
            }
        }
    }
}
