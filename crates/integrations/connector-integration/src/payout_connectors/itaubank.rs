pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::RequestContent,
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
    errors::{ConnectorError, IntegrationError},
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
use error_stack::ResultExt;
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

use crate::{set_typed_response, types::ResponseRouterData};
use transformers::{
    ItaubankAccessTokenRequest, ItaubankAccessTokenResponse, ItaubankAuthType,
    ItaubankErrorResponse, ItaubankPayoutGetResponse, ItaubankTransferRequest,
    ItaubankTransferResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const USER_AGENT: &str = "User-Agent";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_ITAU_API_KEY: &str = "x-itau-apikey";
}

pub struct ItaubankPayouts;

impl ItaubankPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }
}

fn construct_itaubank_error_message(error_res: &ItaubankErrorResponse) -> String {
    let campos_msg = if error_res.campos.is_empty() {
        None
    } else {
        Some(
            error_res
                .campos
                .iter()
                .map(|c| format!("{}: {}", c.campo, c.mensagem))
                .collect::<Vec<String>>()
                .join(", "),
        )
    };

    match (error_res.mensagem.clone(), campos_msg) {
        (Some(msg), Some(campos)) => format!("{} | {}", msg, campos),
        (Some(msg), None) => msg,
        (None, Some(campos)) => campos,
        (None, None) => "Unknown error".to_string(),
    }
}

fn build_env_specific_endpoint(base_url: &str, test_mode: Option<bool>) -> String {
    if test_mode.unwrap_or(true) {
        format!("{base_url}/itau-ep9-gtw-sispag-ext")
    } else {
        format!("{base_url}/sispag")
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for ItaubankPayouts {
    fn id(&self) -> &'static str {
        "itaubank"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.itaubank.base_url
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
        let response: Result<ItaubankErrorResponse, _> =
            res.response.parse_struct("ItaubankErrorResponse");

        match response {
            Ok(error_res) => {
                event_builder.map(|i| i.set_connector_response(&error_res));
                let message = construct_itaubank_error_message(&error_res);
                let typed = crate::connectors::macros::serialize_typed_connector_payload(
                    &error_res,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_res.codigo,
                    message,
                    reason: error_res.mensagem,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                })
            }
            Err(_) => {
                tracing::error!(
                    "Failed to parse error response from Itaubank. Status: {}, Raw: {:?}",
                    res.status_code,
                    res.response
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: res.status_code.to_string(),
                    message: "Failed to parse error response from connector".to_string(),
                    reason: Some(format!("Raw response: {:?}", res.response)),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                })
            }
        }
    }
}

// ===== SERVER AUTHENTICATION (merchant-auth / access token) =====

impl ServerAuthentication for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for ItaubankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        // Itaú exposes the OAuth token endpoint on secondary_base_url for
        // payout configs; older configs use the JWT endpoint on base_url.
        if let Some(secondary_base_url) = req
            .resource_common_data
            .connectors
            .itaubank
            .secondary_base_url
            .as_deref()
        {
            Ok(format!("{}/api/oauth/token", secondary_base_url))
        } else {
            let base_url = self.base_url(&req.resource_common_data.connectors);
            Ok(format!("{}/api/oauth/jwt", base_url))
        }
    }

    fn get_headers(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            ),
            (headers::ACCEPT.to_string(), "*/*".to_string().into()),
        ])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = ItaubankAccessTokenRequest::try_from(req)?;
        Ok(Some(RequestContent::FormUrlEncoded(Box::new(
            connector_req,
        ))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
        ConnectorError,
    > {
        let response: ItaubankAccessTokenResponse = res
            .response
            .parse_struct("ItaubankAccessTokenResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        set_typed_response!(event_builder, response, data, res.status_code)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

// ===== PAYOUT SERVICE TRAIT =====

impl PayoutServiceTrait for ItaubankPayouts {}

// ===== PAYOUT TRANSFER (REAL) =====

impl PayoutTransferV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for ItaubankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
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
        let base_url = build_env_specific_endpoint(
            self.base_url(&req.resource_common_data.connectors),
            req.resource_common_data.test_mode,
        );
        Ok(format!("{base_url}/v1/transferencias"))
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
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;

        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (headers::ACCEPT.to_string(), "*/*".to_string().into()),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ),
            (
                headers::USER_AGENT.to_string(),
                "Hyperswitch".to_string().into(),
            ),
            (
                headers::X_ITAU_API_KEY.to_string(),
                auth.client_id.expose().into_masked(),
            ),
        ])
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
        let connector_req = ItaubankTransferRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
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
        let response: ItaubankTransferResponse = res
            .response
            .parse_struct("ItaubankTransferResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

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

impl PayoutGetV2 for ItaubankPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for ItaubankPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = build_env_specific_endpoint(
            self.base_url(&req.resource_common_data.connectors),
            req.resource_common_data.test_mode,
        );
        let connector_payout_id = req.request.connector_payout_id.clone().ok_or(
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            },
        )?;
        Ok(format!(
            "{}v1/pagamentos_sispag/{}",
            base_url, connector_payout_id
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
        })?;
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;

        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (headers::ACCEPT.to_string(), "*/*".to_string().into()),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ),
            (
                headers::USER_AGENT.to_string(),
                "Hyperswitch".to_string().into(),
            ),
            (
                headers::X_ITAU_API_KEY.to_string(),
                auth.client_id.expose().into_masked(),
            ),
        ])
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
        let response: ItaubankPayoutGetResponse = res
            .response
            .parse_struct("ItaubankPayoutGetResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

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

impl PayoutCreateV2 for ItaubankPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for ItaubankPayouts
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

impl PayoutVoidV2 for ItaubankPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for ItaubankPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_void",
            Default::default(),
        )
        .into())
    }
}

impl PayoutStageV2 for ItaubankPayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for ItaubankPayouts
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

impl PayoutCreateLinkV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for ItaubankPayouts
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

impl PayoutCreateRecipientV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for ItaubankPayouts
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

impl PayoutEnrollDisburseAccountV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for ItaubankPayouts
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

impl PayoutEligibilityV2 for ItaubankPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for ItaubankPayouts
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
