pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{consts::NO_ERROR_CODE, errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount,
        PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid, ServerAuthenticationToken,
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
        PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse, PayoutFlowData,
        PayoutGetRequest, PayoutGetResponse, PayoutStageRequest, PayoutStageResponse,
        PayoutTransferRequest, PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEnrollDisburseAccountV2,
        PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2, PayoutVoidV2,
        ServerAuthentication,
    },
};

use crate::types::ResponseRouterData;
use transformers as paypal;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const PREFER: &str = "Prefer";
    pub(crate) const PAYPAL_REQUEST_ID: &str = "PayPal-Request-Id";
    pub(crate) const PAYPAL_PARTNER_ATTRIBUTION_ID: &str = "PayPal-Partner-Attribution-Id";
}

pub struct PaypalPayouts;

impl PaypalPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn build_payout_headers(
        &self,
        resource_common_data: &PayoutFlowData,
        connector_config: &ConnectorSpecificConfig,
        flow_name: &str,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(format!(
                        "PayPal {} - Failed to obtain OAuth access token",
                        flow_name
                    )),
                    suggested_action: None,
                    doc_url: None,
                },
            }
        })?;

        let auth = paypal::PaypalAuthType::try_from(connector_config)?;

        let mut headers = vec![
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into(),
            ),
            (
                headers::PREFER.to_string(),
                "return=representation".to_string().into(),
            ),
            (
                headers::PAYPAL_REQUEST_ID.to_string(),
                resource_common_data
                    .connector_request_reference_id
                    .clone()
                    .into(),
            ),
        ];

        // Add partner attribution header for partner integration
        if let paypal::PaypalAuthType::AuthWithDetails(
            paypal::PaypalConnectorCredentials::PartnerIntegration(creds),
        ) = auth
        {
            let legacy_attribution_id = creds.payer_id.peek().clone();
            if !legacy_attribution_id.is_empty() {
                headers.push((
                    headers::PAYPAL_PARTNER_ATTRIBUTION_ID.to_string(),
                    legacy_attribution_id.into(),
                ));
            }
        }

        Ok(headers)
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for PaypalPayouts {
    fn id(&self) -> &'static str {
        "paypal"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paypal.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = paypal::PaypalAuthType::try_from(auth_type)?;
        let credentials = auth.get_credentials()?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            credentials.get_client_secret().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: paypal::PaypalPaymentErrorResponse = res
            .response
            .parse_struct("Paypal ErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "PayPal - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

        let reason = response.details.as_ref().map(|details| {
            details
                .iter()
                .filter_map(|d| d.description.clone())
                .collect::<Vec<_>>()
                .join("; ")
        });

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .name
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response.message.clone(),
            reason,
            attempt_status: None,
            connector_transaction_id: response.debug_id,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ===== SERVER AUTHENTICATION (not implemented) =====

impl ServerAuthentication for PaypalPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for PaypalPayouts
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

// ===== PAYOUT SERVICE TRAIT =====

impl PayoutServiceTrait for PaypalPayouts {}

// ===== PAYOUT TRANSFER (REAL) =====

impl PayoutTransferV2 for PaypalPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for PaypalPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
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
        Ok(format!(
            "{}v1/payments/payouts",
            self.base_url(&req.resource_common_data.connectors)
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
        self.build_payout_headers(
            &req.resource_common_data,
            &req.connector_config,
            "Payout Transfer",
        )
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = paypal::PaypalFulfillRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::Json(Box::new(
            connector_req,
        ))))
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
        let response: paypal::PaypalFulfillResponse = res
            .response
            .parse_struct("PaypalFulfillResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "PayPal Fulfill (PayoutTransfer) - failed to deserialize response"
                            .to_string(),
                    ),
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "PayPal Fulfill (PayoutTransfer) - failed to map response to RouterData"
                        .to_string(),
                ),
            },
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

// ===== PAYOUT GET (REAL) =====

impl PayoutGetV2 for PaypalPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for PaypalPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let connector_payout_id = req.request.connector_payout_id.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PayPal Payout Get - Missing connector payout ID".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            },
        )?;

        let base = self.base_url(&req.resource_common_data.connectors);
        let url = url::Url::parse(base)
            .map_err(|_| IntegrationError::InvalidDataFormat {
                field_name: "base_url",
                context: IntegrationErrorContext {
                    additional_context: Some("PayPal Payout Get - Invalid base URL".to_string()),
                    suggested_action: None,
                    doc_url: None,
                },
            })?
            .join(&format!("v1/payments/payouts/{}", connector_payout_id))
            .map_err(|_| IntegrationError::InvalidDataFormat {
                field_name: "connector_payout_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "PayPal Payout Get - Invalid connector payout ID for URL".to_string(),
                    ),
                    suggested_action: None,
                    doc_url: None,
                },
            })?;

        Ok(url.to_string())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(
            &req.resource_common_data,
            &req.connector_config,
            "Payout Get",
        )
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
        let response: paypal::PaypalPayoutSyncResponse = res
            .response
            .parse_struct("PaypalPayoutSyncResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "PayPal Get (PayoutGet) - failed to deserialize response".to_string(),
                    ),
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "PayPal Get (PayoutGet) - failed to map response to RouterData".to_string(),
                ),
            },
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

impl PayoutCreateV2 for PaypalPayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for PaypalPayouts
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

impl PayoutVoidV2 for PaypalPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for PaypalPayouts
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

impl PayoutStageV2 for PaypalPayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for PaypalPayouts
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

impl PayoutCreateLinkV2 for PaypalPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for PaypalPayouts
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

impl PayoutCreateRecipientV2 for PaypalPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for PaypalPayouts
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

impl PayoutEnrollDisburseAccountV2 for PaypalPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for PaypalPayouts
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
