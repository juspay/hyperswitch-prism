pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::BytesExt,
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
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEligibilityV2,
        PayoutEnrollDisburseAccountV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2,
        PayoutTransferV2, PayoutVoidV2, ServerAuthentication,
    },
};

use crate::{finalize_connector_response, types::ResponseRouterData, with_error_response_body};
use transformers::{NuveiPayoutRequest, NuveiPayoutResponse};

pub struct NuveiPayouts;

impl NuveiPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }
}

impl ConnectorCommon for NuveiPayouts {
    fn id(&self) -> &'static str {
        "nuvei"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.nuvei.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: NuveiPayoutResponse = res
            .response
            .parse_struct("NuveiPayoutResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "nuvei",
            ))?;
        with_error_response_body!(event_builder, response);
        Ok(response.error_response(res.status_code))
    }
}

impl PayoutServiceTrait for NuveiPayouts {}
impl PayoutTransferV2 for NuveiPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for NuveiPayouts
{
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
            "{}/payout.do",
            self.base_url(&req.resource_common_data.connectors)
                .trim_end_matches('/')
        ))
    }

    fn get_headers(
        &self,
        _req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        Ok(vec![(
            "Content-Type".to_owned(),
            self.common_get_content_type().to_owned().into(),
        )])
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
        let request = NuveiPayoutRequest::try_from(req)?;
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&request, "typed_connector_request");
        Ok(Some(ConnectorRequestData::new(
            RequestContent::Json(Box::new(request)),
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
        let response: NuveiPayoutResponse = res
            .response
            .parse_struct("NuveiPayoutResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "nuvei",
            ))?;
        finalize_connector_response!(event_builder, response, data, res.status_code)
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

macro_rules! unsupported_payout_flows {
    ($(($flow:ty, $request:ty, $response:ty, $trait:ident, $name:literal)),+ $(,)?) => {
        $(impl $trait for NuveiPayouts {}
        impl ConnectorIntegrationV2<$flow, PayoutFlowData, $request, $response> for NuveiPayouts {
            fn get_url(&self, _req: &RouterDataV2<$flow, PayoutFlowData, $request, $response>) -> CustomResult<String, IntegrationError> {
                Err(IntegrationError::connector_flow_not_implemented(self.id(), $name, Default::default()).into())
            }
        })+
    };
}

unsupported_payout_flows!(
    (
        PayoutCreate,
        PayoutCreateRequest,
        PayoutCreateResponse,
        PayoutCreateV2,
        "payout_create"
    ),
    (
        PayoutGet,
        PayoutGetRequest,
        PayoutGetResponse,
        PayoutGetV2,
        "payout_get"
    ),
    (
        PayoutVoid,
        PayoutVoidRequest,
        PayoutVoidResponse,
        PayoutVoidV2,
        "payout_void"
    ),
    (
        PayoutStage,
        PayoutStageRequest,
        PayoutStageResponse,
        PayoutStageV2,
        "payout_stage"
    ),
    (
        PayoutCreateLink,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
        PayoutCreateLinkV2,
        "payout_create_link"
    ),
    (
        PayoutCreateRecipient,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
        PayoutCreateRecipientV2,
        "payout_create_recipient"
    ),
    (
        PayoutEnrollDisburseAccount,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
        PayoutEnrollDisburseAccountV2,
        "payout_enroll_disburse_account"
    ),
    (
        PayoutEligibility,
        PayoutEligibilityRequest,
        PayoutEligibilityResponse,
        PayoutEligibilityV2,
        "payout_eligibility"
    ),
);

impl ServerAuthentication for NuveiPayouts {}
impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for NuveiPayouts
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
