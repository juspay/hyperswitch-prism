pub mod transformers;

use std::collections::BTreeMap;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
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

use crate::{types::ResponseRouterData, with_error_response_body};
use transformers as truelayer;
use transformers::{
    TruelayerAccessTokenErrorResponse, TruelayerErrorResponse, TruelayerPayoutRequest,
    TruelayerPayoutResponse, TruelayerPayoutSyncType, TruelayerServerAuthenticationTokenRequest,
    TruelayerServerAuthenticationTokenResponse,
};

const TL_SIGNATURE: &str = "Tl-Signature";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const IDEMPOTENCY_KEY: &str = "Idempotency-Key";
}

pub struct TruelayerPayouts;

impl TruelayerPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn build_signed_headers(
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
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "TrueLayer PayoutTransfer requires an OAuth access token".to_string(),
                    ),
                    ..Default::default()
                },
            }
        })?;
        let metadata = truelayer::TruelayerPayoutMetadata::try_from(&req.connector_config)?;
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        let request_body = self
            .get_request_body(req)?
            .map(|body| body.get_inner_value().expose().clone());

        let mut signed_headers = BTreeMap::new();
        signed_headers.insert(
            headers::IDEMPOTENCY_KEY.to_string(),
            idempotency_key.clone(),
        );

        let tl_signature = truelayer::generate_tl_signature(
            common_utils::request::Method::Post.to_string(),
            "/v3/payouts",
            &signed_headers,
            request_body.as_deref(),
            metadata.private_key.expose().clone(),
            metadata.kid.expose().as_str(),
        )?;

        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ),
            (TL_SIGNATURE.to_string(), tl_signature.into_masked()),
            (headers::IDEMPOTENCY_KEY.to_string(), idempotency_key.into()),
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
        ])
    }

    fn build_bearer_headers(
        &self,
        resource_common_data: &PayoutFlowData,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let access_token = resource_common_data.get_access_token().map_err(|_| {
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "TrueLayer PayoutGet requires an OAuth access token".to_string(),
                    ),
                    ..Default::default()
                },
            }
        })?;

        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {access_token}").into_masked(),
            ),
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
        ])
    }
}

impl ConnectorCommon for TruelayerPayouts {
    fn id(&self) -> &'static str {
        "truelayer"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json; charset=UTF-8"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.truelayer.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: TruelayerErrorResponse = res
            .response
            .parse_struct("TruelayerErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "TrueLayer payouts - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.title.clone(),
            message: response
                .errors
                .unwrap_or_else(|| serde_json::Value::String(response.title.clone()))
                .to_string(),
            reason: Some(response.detail),
            attempt_status: None,
            connector_transaction_id: Some(response.trace_id),
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

impl PayoutServiceTrait for TruelayerPayouts {}
impl ServerAuthentication for TruelayerPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for TruelayerPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
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
        let base_url = req
            .resource_common_data
            .connectors
            .truelayer
            .secondary_base_url
            .as_ref()
            .ok_or(IntegrationError::FailedToObtainIntegrationUrl {
                context: Default::default(),
            })?;
        Ok(format!("{base_url}/connect/token"))
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
        Ok(vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/x-www-form-urlencoded".to_string().into(),
        )])
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = TruelayerServerAuthenticationTokenRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
        )))
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
        let response: TruelayerServerAuthenticationTokenResponse = res
            .response
            .parse_struct("TruelayerServerAuthenticationTokenResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "TrueLayer access-token response deserialization failed".to_string(),
                    ),
                },
            })?;

        event_builder.map(|event| event.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "TrueLayer access-token response mapping failed".to_string(),
                ),
            },
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: TruelayerAccessTokenErrorResponse = res
            .response
            .parse_struct("TruelayerAccessTokenErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "TrueLayer access-token error deserialization failed".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error,
            message: response
                .error_description
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.error_details.and_then(|details| details.reason),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

impl PayoutTransferV2 for TruelayerPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for TruelayerPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
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
            "{}/v3/payouts",
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
        self.build_signed_headers(req)
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
        let connector_req = TruelayerPayoutRequest::try_from(req)?;
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
        let response: TruelayerPayoutResponse = res
            .response
            .parse_struct("TruelayerPayoutResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "TrueLayer PayoutTransfer response deserialization failed".to_string(),
                    ),
                },
            })?;

        event_builder.map(|event| event.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some(
                    "TrueLayer PayoutTransfer response mapping failed".to_string(),
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

impl PayoutGetV2 for TruelayerPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for TruelayerPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let connector_payout_id = req.request.connector_payout_id.as_ref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_payout_id",
                context: Default::default(),
            },
        )?;

        Ok(format!(
            "{}/v3/payouts/{connector_payout_id}",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_bearer_headers(&req.resource_common_data)
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
        let response: TruelayerPayoutSyncType = res
            .response
            .parse_struct("TruelayerPayoutSyncType")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "TrueLayer PayoutGet response deserialization failed".to_string(),
                    ),
                },
            })?;

        event_builder.map(|event| event.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
        .change_context(ConnectorError::ResponseDeserializationFailed {
            context: ResponseTransformationErrorContext {
                http_status_code: Some(res.status_code),
                additional_context: Some("TrueLayer PayoutGet response mapping failed".to_string()),
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

macro_rules! impl_unimplemented_payout_flow {
    ($trait_name:ident, $flow:ty, $request:ty, $response:ty, $flow_name:literal) => {
        impl $trait_name for TruelayerPayouts {}

        impl ConnectorIntegrationV2<$flow, PayoutFlowData, $request, $response>
            for TruelayerPayouts
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<$flow, PayoutFlowData, $request, $response>,
            ) -> CustomResult<String, IntegrationError> {
                Err(IntegrationError::connector_flow_not_implemented(
                    self.id(),
                    $flow_name,
                    Default::default(),
                )
                .into())
            }
        }
    };
}

impl_unimplemented_payout_flow!(
    PayoutCreateV2,
    PayoutCreate,
    PayoutCreateRequest,
    PayoutCreateResponse,
    "payout_create"
);
impl_unimplemented_payout_flow!(
    PayoutVoidV2,
    PayoutVoid,
    PayoutVoidRequest,
    PayoutVoidResponse,
    "payout_void"
);
impl_unimplemented_payout_flow!(
    PayoutStageV2,
    PayoutStage,
    PayoutStageRequest,
    PayoutStageResponse,
    "payout_stage"
);
impl_unimplemented_payout_flow!(
    PayoutCreateLinkV2,
    PayoutCreateLink,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
    "payout_create_link"
);
impl_unimplemented_payout_flow!(
    PayoutCreateRecipientV2,
    PayoutCreateRecipient,
    PayoutCreateRecipientRequest,
    PayoutCreateRecipientResponse,
    "payout_create_recipient"
);
impl_unimplemented_payout_flow!(
    PayoutEnrollDisburseAccountV2,
    PayoutEnrollDisburseAccount,
    PayoutEnrollDisburseAccountRequest,
    PayoutEnrollDisburseAccountResponse,
    "payout_enroll_disburse_account"
);
impl_unimplemented_payout_flow!(
    PayoutEligibilityV2,
    PayoutEligibility,
    PayoutEligibilityRequest,
    PayoutEligibilityResponse,
    "payout_eligibility"
);
