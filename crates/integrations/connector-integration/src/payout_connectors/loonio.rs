use domain_types::payment_method_data::PaymentMethodDataTypes;
pub mod transformers;

use common_enums::CurrencyUnit;
use common_utils::{consts::NO_ERROR_CODE, errors::CustomResult, events, ext_traits::ByteSliceExt};
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

use crate::finalize_connector_response;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use transformers::{
    LoonioAuthType, LoonioErrorResponse, LoonioPayoutGetResponse, LoonioPayoutTransferRequest,
    LoonioPayoutTransferResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const MERCHANTID: &str = "MerchantID";
    pub(crate) const MERCHANT_TOKEN: &str = "MerchantToken";
}

pub struct LoonioPayouts;

impl LoonioPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = LoonioAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            (
                headers::MERCHANTID.to_string(),
                auth.merchant_id.expose().into_masked(),
            ),
            (
                headers::MERCHANT_TOKEN.to_string(),
                auth.merchant_token.expose().into_masked(),
            ),
        ])
    }

    fn build_payout_headers(
        &self,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(connector_config)?;
        header.append(&mut api_key);
        Ok(header)
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for LoonioPayouts {
    fn id(&self) -> &'static str {
        "loonio"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.loonio.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.get_auth_header(auth_type)
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: LoonioErrorResponse = res
            .response
            .parse_struct("LoonioErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "loonio: response body did not match the expected format",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed = crate::connectors::macros::serialize_typed_connector_payload(
            &response,
            "typed_connector_response",
        );
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error_code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response.message.clone(),
            reason: Some(response.message.clone()),
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

// ===== SERVER AUTHENTICATION (not implemented) =====

impl ServerAuthentication for LoonioPayouts {}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for LoonioPayouts
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutServiceTrait<T> for LoonioPayouts
{
}

// ===== PAYOUT TRANSFER (REAL) =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutTransferV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest<T>,
        PayoutTransferResponse,
    > for LoonioPayouts
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
            PayoutTransferRequest<T>,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}api/v1/transactions/outgoing/send_to_interac",
            req.resource_common_data.connectors.loonio.base_url
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest<T>,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest<T>,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let connector_req = LoonioPayoutTransferRequest::try_from(req)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest<T>,
            PayoutTransferResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest<T>,
            PayoutTransferResponse,
        >,
        ConnectorError,
    > {
        let response: LoonioPayoutTransferResponse = res
            .response
            .parse_struct("LoonioPayoutTransferResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

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

// ===== PAYOUT GET (REAL) =====

impl PayoutGetV2 for LoonioPayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for LoonioPayouts
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
                context: Default::default(),
            },
        )?;
        Ok(format!(
            "{}api/v1/transactions/{}/details",
            req.resource_common_data.connectors.loonio.base_url, connector_payout_id
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
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
        let response: LoonioPayoutGetResponse = res
            .response
            .parse_struct("LoonioPayoutGetResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

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

// ===== PAYOUT STUB FLOWS =====

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutCreateV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutCreate,
        PayoutFlowData,
        PayoutCreateRequest<T>,
        PayoutCreateResponse,
    > for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest<T>,
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

impl PayoutVoidV2 for LoonioPayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for LoonioPayouts
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutStageV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest<T>, PayoutStageResponse>
    for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutStage,
            PayoutFlowData,
            PayoutStageRequest<T>,
            PayoutStageResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            Default::default(),
        )
        .into())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutCreateLinkV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest<T>,
        PayoutCreateLinkResponse,
    > for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateLink,
            PayoutFlowData,
            PayoutCreateLinkRequest<T>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutCreateRecipientV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest<T>,
        PayoutCreateRecipientResponse,
    > for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest<T>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutEnrollDisburseAccountV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest<T>,
        PayoutEnrollDisburseAccountResponse,
    > for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest<T>,
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

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    PayoutEligibilityV2<T> for LoonioPayouts
{
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Send + Sync + 'static + serde::Serialize>
    ConnectorIntegrationV2<
        PayoutEligibility,
        PayoutFlowData,
        PayoutEligibilityRequest<T>,
        PayoutEligibilityResponse,
    > for LoonioPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutEligibility,
            PayoutFlowData,
            PayoutEligibilityRequest<T>,
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
