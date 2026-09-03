pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{ConnectorRequestData, RequestContent},
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

use crate::{types::ResponseRouterData, with_error_response_body};
use transformers::{
    MifinityAuthType, MifinityErrorResponse, MifinityPmcRequest, MifinityPmcResponse,
    MifinityStatusResponse,
};

const API_VERSION: &str = "1";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
    pub(crate) const KEY: &str = "key";
    pub(crate) const API_VERSION: &str = "api-version";
}

pub struct MifinityPayouts;

impl MifinityPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn build_headers(
        &self,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = MifinityAuthType::try_from(connector_config)?;
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::ACCEPT.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (headers::KEY.to_string(), auth.key.expose().into_masked()),
            (
                headers::API_VERSION.to_string(),
                API_VERSION.to_string().into(),
            ),
        ])
    }
}

impl ConnectorCommon for MifinityPayouts {
    fn id(&self) -> &'static str {
        "mifinity"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.mifinity.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: MifinityErrorResponse = res
            .response
            .parse_struct("MifinityErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "MiFinity payouts - failed to deserialize error response".to_string(),
                    ),
                },
            })?;

        with_error_response_body!(event_builder, response);

        let typed_connector_response = crate::connectors::macros::serialize_typed_connector_payload(
            &response,
            "typed_connector_response",
        );

        let first_error = response.errors.first();
        let code = first_error
            .and_then(|e| e.error_code.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.to_string());
        let message = first_error
            .and_then(|e| e.message.clone())
            .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message: message.clone(),
            reason: Some(message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

impl PayoutServiceTrait for MifinityPayouts {}
impl ServerAuthentication for MifinityPayouts {}

// ===== SERVER AUTHENTICATION (not implemented — MiFinity uses static key auth) =====

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for MifinityPayouts
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
            self.id(),
            "server_authentication_token",
            Default::default(),
        )
        .into())
    }
}

// ===== PAYOUT TRANSFER (REAL — PayMyCard / PMC card payout) =====

impl PayoutTransferV2 for MifinityPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for MifinityPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
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
        let base_url = self
            .base_url(&req.resource_common_data.connectors)
            .trim_end_matches('/');
        Ok(format!("{base_url}/api/payments/pmc"))
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
        self.build_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<ConnectorRequestData>, IntegrationError> {
        let connector_req = MifinityPmcRequest::try_from(req)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(ConnectorRequestData::new(
            RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
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
        let response: MifinityPmcResponse = res
            .response
            .parse_struct("MifinityPmcResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "MiFinity PayoutTransfer response deserialization failed".to_string(),
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
                    "MiFinity PayoutTransfer response mapping failed".to_string(),
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

// ===== PAYOUT GET / STATUS SYNC (REAL — GET /api/transactions/status/{traceId}) =====

impl PayoutGetV2 for MifinityPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for MifinityPayouts
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        // MiFinity's status endpoint is keyed by `traceId` — the caller-assigned
        // correlation id sent on the original PayoutTransfer request. That value
        // travels here as `connector_request_reference_id` (derived from the
        // merchant payout id); fall back to the connector payout id if unset.
        let trace_id = {
            let reference = req
                .resource_common_data
                .connector_request_reference_id
                .clone();
            if reference.is_empty() {
                req.request.connector_payout_id.clone().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_payout_id",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "MiFinity payout sync requires the traceId (merchant_payout_id) used on the original transfer, or a connector_payout_id."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    },
                )?
            } else {
                reference
            }
        };

        let base_url = self
            .base_url(&req.resource_common_data.connectors)
            .trim_end_matches('/');
        Ok(format!("{base_url}/api/transactions/status/{trace_id}"))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(&req.connector_config)
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
        let response: MifinityStatusResponse = res
            .response
            .parse_struct("MifinityStatusResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some(
                        "MiFinity PayoutGet response deserialization failed".to_string(),
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
                    "MiFinity PayoutGet response mapping failed".to_string(),
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

macro_rules! impl_unimplemented_payout_flow {
    ($trait_name:ident, $flow:ty, $request:ty, $response:ty, $flow_name:literal) => {
        impl $trait_name for MifinityPayouts {}

        impl ConnectorIntegrationV2<$flow, PayoutFlowData, $request, $response> for MifinityPayouts {
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
