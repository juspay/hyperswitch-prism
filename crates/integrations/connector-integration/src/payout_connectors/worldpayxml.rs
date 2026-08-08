pub mod transformers;

use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine as _};
use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::{ByteSliceExt, XmlExt},
    request::RequestContent,
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEligibility,
        PayoutEnrollDisburseAccount, PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
        ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest, PayoutTransferResponse,
        PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEligibilityV2,
        PayoutEnrollDisburseAccountV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2,
        PayoutTransferV2, PayoutVoidV2, ServerAuthentication,
    },
};

use crate::{
    connectors::worldpayxml::{requests, responses},
    set_typed_response,
    types::ResponseRouterData,
    with_error_response_body,
};
use transformers::WorldpayxmlAuthType;

const CONTENT_TYPE_XML: &str = "text/xml";

mod headers {
    pub(super) const CONTENT_TYPE: &str = "Content-Type";
    pub(super) const AUTHORIZATION: &str = "Authorization";
}

pub struct WorldpayxmlPayouts;

impl WorldpayxmlPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn build_auth_header(
        auth: WorldpayxmlAuthType,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let credentials = format!(
            "{}:{}",
            auth.api_username.expose(),
            auth.api_password.expose()
        );
        let encoded = BASE64_ENGINE.encode(credentials.as_bytes());
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", encoded).into_masked(),
        )])
    }

    fn xml_headers(
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = WorldpayxmlAuthType::try_from(connector_config)?;
        let mut headers = vec![(
            headers::CONTENT_TYPE.to_string(),
            CONTENT_TYPE_XML.to_string().into(),
        )];
        headers.extend(Self::build_auth_header(auth)?);
        Ok(headers)
    }

    fn parse_xml_response<T>(
        bytes: bytes::Bytes,
        status_code: u16,
    ) -> CustomResult<T, ConnectorError>
    where
        T: serde::de::DeserializeOwned,
    {
        if bytes.is_empty() {
            return Err(crate::utils::response_handling_fail_for_connector(
                status_code,
                "worldpayxml",
            )
            .into());
        }
        let response_str = String::from_utf8(bytes.to_vec())
            .change_context(crate::utils::response_handling_fail_for_connector(
                status_code,
                "worldpayxml",
            ))
            .attach_printable("Failed to convert response bytes to UTF-8 string")?;
        response_str
            .as_str()
            .parse_xml::<T>()
            .change_context(crate::utils::response_handling_fail_for_connector(
                status_code,
                "worldpayxml",
            ))
            .attach_printable("Failed to parse XML response")
    }

    fn encode_soap_xml<R>(request: &R) -> CustomResult<RequestContent, IntegrationError>
    where
        R: crate::connectors::macros::GetSoapXml,
    {
        let soap_xml = R::to_soap_xml(request);
        crate::connectors::macros::validate_xml_structure(&soap_xml).map_err(|e| {
            Report::new(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })
            .attach_printable(e)
        })?;
        Ok(RequestContent::RawBytes(soap_xml.into_bytes()))
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for WorldpayxmlPayouts {
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
        let auth = WorldpayxmlAuthType::try_from(auth_type)?;
        Self::build_auth_header(auth)
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
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "worldpayxml: response body did not match the expected format; confirm API version and connector documentation.",
            ))?;

        let typed = crate::connectors::macros::serialize_typed_connector_payload(
            &response,
            "typed_connector_response",
        );
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

// ===== SERVER AUTHENTICATION (not implemented) =====

impl ServerAuthentication for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            ConnectorCommon::id(self),
            "server_authentication_token",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}

impl PayoutServiceTrait for WorldpayxmlPayouts {}

// ===== PAYOUT TRANSFER (REAL) =====

impl PayoutTransferV2 for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for WorldpayxmlPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(req
            .resource_common_data
            .connectors
            .worldpayxml
            .base_url
            .to_string())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Self::xml_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = requests::WorldpayxmlPayoutTransferRequest::try_from(req)?;
        Ok(Some(Self::encode_soap_xml(&connector_req)?))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ConnectorError,
    > {
        let response: responses::WorldpayxmlPayoutTransferResponse =
            Self::parse_xml_response(res.response, res.status_code)?;
        set_typed_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT GET (REAL) =====

impl PayoutGetV2 for WorldpayxmlPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for WorldpayxmlPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(req
            .resource_common_data
            .connectors
            .worldpayxml
            .base_url
            .to_string())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Self::xml_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = requests::WorldpayxmlPayoutGetRequest::try_from(req)?;
        Ok(Some(Self::encode_soap_xml(&connector_req)?))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ConnectorError,
    > {
        let response: responses::WorldpayxmlPayoutGetResponse =
            Self::parse_xml_response(res.response, res.status_code)?;
        set_typed_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT VOID (REAL) =====

impl PayoutVoidV2 for WorldpayxmlPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for WorldpayxmlPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(req
            .resource_common_data
            .connectors
            .worldpayxml
            .base_url
            .to_string())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Self::xml_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = requests::WorldpayxmlPayoutVoidRequest::try_from(req)?;
        Ok(Some(Self::encode_soap_xml(&connector_req)?))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ConnectorError,
    > {
        let response: responses::WorldpayxmlPayoutVoidResponse =
            Self::parse_xml_response(res.response, res.status_code)?;
        set_typed_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT STUB FLOWS =====

impl PayoutCreateV2 for WorldpayxmlPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create",
            Default::default(),
        )
        .into())
    }
}

impl PayoutStageV2 for WorldpayxmlPayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateLinkV2 for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateLink,
            PayoutFlowData,
            PayoutCreateLinkRequest,
            PayoutCreateLinkResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_link",
            Default::default(),
        )
        .into())
    }
}

impl PayoutCreateRecipientV2 for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_recipient",
            Default::default(),
        )
        .into())
    }
}

impl PayoutEnrollDisburseAccountV2 for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_enroll_disburse_account",
            Default::default(),
        )
        .into())
    }
}

impl PayoutEligibilityV2 for WorldpayxmlPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for WorldpayxmlPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest,
            PayoutEligibilityResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_eligibility",
            Default::default(),
        )
        .into())
    }
}
