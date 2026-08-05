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
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
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

use crate::types::ResponseRouterData;
use transformers::{
    SantanderAccessTokenRequest, SantanderAccessTokenResponse, SantanderAuthType,
    SantanderCreateRequest, SantanderErrorResponse, SantanderPayoutResponse,
    SantanderStatusResponse, SantanderTransferRequest, SANTANDER_PIX_DOCS_URL,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_APPLICATION_KEY: &str = "X-Application-Key";
}

pub struct SantanderPayouts;

impl SantanderPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for SantanderPayouts {
    fn id(&self) -> &'static str {
        "santander"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.santander.base_url
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
        let response: Result<SantanderErrorResponse, _> =
            res.response.parse_struct("SantanderErrorResponse");

        match response {
            Ok(error_res) => {
                event_builder.map(|i| i.set_connector_response(&error_res));
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: error_res.code,
                    message: error_res.message,
                    reason: error_res.details,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            Err(error) => {
                tracing::error!(
                    "Failed to parse error response from Santander. Status: {}, Error: {:?}, Raw: {:?}",
                    res.status_code,
                    error,
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
                })
            }
        }
    }
}

fn get_workspace_id(auth: &SantanderAuthType) -> String {
    auth.workspace_id.clone().expose()
}

fn get_api_headers(access_token: &str, client_id: &str) -> Vec<(String, Maskable<String>)> {
    vec![
        (
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        ),
        (
            headers::ACCEPT.to_string(),
            "application/json".to_string().into(),
        ),
        (
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {access_token}").into_masked(),
        ),
        (
            headers::X_APPLICATION_KEY.to_string(),
            client_id.to_string().into_masked(),
        ),
    ]
}

// ===== SERVER AUTHENTICATION (access token) =====

impl ServerAuthentication for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for SantanderPayouts
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
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
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
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
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
        let base_url = self.base_url(&req.resource_common_data.connectors);
        Ok(format!("{base_url}/auth/oauth/v2/token"))
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
            (
                headers::ACCEPT.to_string(),
                "application/json".to_string().into(),
            ),
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
        let connector_req = SantanderAccessTokenRequest::try_from(req)?;
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
        let response: Result<SantanderAccessTokenResponse, _> =
            res.response.parse_struct("SantanderAccessTokenResponse");

        match response {
            Ok(token_res) => {
                event_builder.map(|i| i.set_connector_response(&token_res));
                Ok(RouterDataV2 {
                    response: Ok(ServerAuthenticationTokenResponseData {
                        access_token: token_res.access_token.into(),
                        token_type: token_res.token_type,
                        expires_in: token_res.expires_in,
                    }),
                    ..data.clone()
                })
            }
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    status_code = res.status_code,
                    "Failed to parse access token response from Santander"
                );
                Err(ConnectorError::ResponseDeserializationFailed {
                    context: ResponseTransformationErrorContext {
                        http_status_code: Some(res.status_code),
                        additional_context: Some(
                            "Santander access token - failed to deserialize response".to_string(),
                        ),
                    },
                }
                .into())
            }
        }
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

// ===== PAYOUT SERVICE TRAIT =====

impl PayoutEligibilityV2 for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
    > for SantanderPayouts
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
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support payout eligibility checks".to_string(),
                ),
                suggested_action: Some(
                    "Skip eligibility check; Santander validates recipient details during payout create".to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}

impl PayoutServiceTrait for SantanderPayouts {}

// ===== PAYOUT CREATE (POST — create payout with recipient) =====

impl PayoutCreateV2 for SantanderPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for SantanderPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_certificate(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        let workspace_id = get_workspace_id(&auth);
        Ok(format!(
            "{base_url}/management_payments_partners/v1/workspaces/{workspace_id}/pix_payments"
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.get_access_token()?;
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(get_api_headers(&access_token, &auth.client_id.expose()))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = SantanderCreateRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ConnectorError,
    > {
        let response: SantanderPayoutResponse = res
            .response
            .parse_struct("SantanderPayoutResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize Santander payout response".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

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
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT TRANSFER (PATCH — fulfill/authorize payout) =====

impl PayoutTransferV2 for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for SantanderPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Patch
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
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
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
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
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
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        let workspace_id = get_workspace_id(&auth);
        let connector_payout_id = req
            .request
            .connector_payout_id
            .clone()
            .ok_or(IntegrationError::MissingConnectorTransactionID {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "connector_payout_id is required to authorize a payout transfer".to_string(),
                ),
                suggested_action: Some(
                    "Ensure the payout create step succeeded and returned a connector_payout_id"
                        .to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        })?;
        Ok(format!(
            "{base_url}/management_payments_partners/v1/workspaces/{workspace_id}/pix_payments/{connector_payout_id}"
        ))
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
        let access_token = req.resource_common_data.get_access_token()?;
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(get_api_headers(&access_token, &auth.client_id.expose()))
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
        let connector_req = SantanderTransferRequest::try_from(req)?;
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
        let response: SantanderPayoutResponse = res
            .response
            .parse_struct("SantanderPayoutResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize Santander payout response".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

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
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT VOID — not supported by Santander =====

impl PayoutVoidV2 for SantanderPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for SantanderPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_void",
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support voiding Pix payouts".to_string(),
                ),
                suggested_action: Some(
                    "Contact Santander support to cancel a payout after it has been authorized"
                        .to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}

// ===== PAYOUT GET (GET — status sync) =====

impl PayoutGetV2 for SantanderPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for SantanderPayouts
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
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(auth.certificates)
    }

    fn get_certificate_key(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Option<hyperswitch_masking::Secret<String>>, IntegrationError> {
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(auth.private_key)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let base_url = self.base_url(&req.resource_common_data.connectors);
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        let workspace_id = get_workspace_id(&auth);
        let connector_payout_id = req
            .request
            .connector_payout_id
            .clone()
            .ok_or(IntegrationError::MissingConnectorTransactionID {
            context: IntegrationErrorContext {
                additional_context: Some(
                    "connector_payout_id is required to fetch payout status".to_string(),
                ),
                suggested_action: Some(
                    "Ensure the payout create step succeeded and returned a connector_payout_id"
                        .to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        })?;
        Ok(format!(
            "{base_url}/management_payments_partners/v1/workspaces/{workspace_id}/pix_payments/{connector_payout_id}"
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = req.resource_common_data.get_access_token()?;
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(get_api_headers(&access_token, &auth.client_id.expose()))
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
        let response: SantanderStatusResponse = res
            .response
            .parse_struct("SantanderStatusResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    additional_context: Some(
                        "Failed to deserialize Santander payout response".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

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
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

// ===== PAYOUT STUB FLOWS =====

impl PayoutStageV2 for SantanderPayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for SantanderPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support staged payout flows".to_string(),
                ),
                suggested_action: Some(
                    "Use the payout create and transfer flows directly".to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}

impl PayoutCreateLinkV2 for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for SantanderPayouts
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
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support creating payout links".to_string(),
                ),
                suggested_action: Some(
                    "Use Pix direct bank transfer instead of payout links".to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}

impl PayoutCreateRecipientV2 for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for SantanderPayouts
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
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support a separate recipient creation step".to_string(),
                ),
                suggested_action: Some(
                    "Provide recipient details directly in the payout create request".to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}

impl PayoutEnrollDisburseAccountV2 for SantanderPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for SantanderPayouts
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
            IntegrationErrorContext {
                additional_context: Some(
                    "Santander does not support enrolling disburse accounts".to_string(),
                ),
                suggested_action: Some(
                    "Configure the debit account via source_bank_data in the transfer request"
                        .to_string(),
                ),
                doc_url: Some(SANTANDER_PIX_DOCS_URL.to_string()),
            },
        )
        .into())
    }
}
