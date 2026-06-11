pub mod transformers;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount,
        PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
    },
    errors::{ConnectorError, IntegrationError},
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
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEnrollDisburseAccountV2,
        PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2, PayoutVoidV2,
    },
};

use crate::types::ResponseRouterData;
use transformers as stripe;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const STRIPE_API_VERSION: &str = "stripe-version";
    pub(crate) const STRIPE_VERSION: &str = "2022-11-15";
    pub(crate) const STRIPE_COMPATIBLE_CONNECT_ACCOUNT: &str = "Stripe-Account";
}

pub struct StripePayouts;

impl StripePayouts {
    pub const fn new() -> &'static Self {
        &Self
    }

    fn base_url<'a>(&self, req_connectors: &'a Connectors) -> &'a str {
        req_connectors.stripe.base_url.as_ref()
    }

    /// Base headers shared by every payout flow: content type + bearer auth + api version.
    fn build_payout_headers(
        &self,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = stripe::StripeAuthType::try_from(connector_config)?;
        Ok(vec![
            (
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ),
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", auth.api_key.peek()).into_masked(),
            ),
            (
                headers::STRIPE_API_VERSION.to_string(),
                headers::STRIPE_VERSION.to_string().into_masked(),
            ),
        ])
    }
}

// ===== CONNECTOR COMMON =====

impl ConnectorCommon for StripePayouts {
    fn id(&self) -> &'static str {
        "stripe"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.stripe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = stripe::StripeAuthType::try_from(auth_type)?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", auth.api_key.peek()).into_masked(),
            ),
            (
                headers::STRIPE_API_VERSION.to_string(),
                headers::STRIPE_VERSION.to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: stripe::StripeConnectErrorResponse = res
            .response
            .parse_struct("StripeConnectErrorResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

        event_builder.map(|i| i.set_connector_response(&response));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: if response.error.message.is_empty() {
                NO_ERROR_MESSAGE.to_string()
            } else {
                response.error.message.clone()
            },
            reason: response.error.decline_code.clone(),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: response.error.decline_code,
        })
    }
}

// ===== PAYOUT SERVICE TRAIT =====

impl PayoutServiceTrait for StripePayouts {}

// ===== PAYOUT CREATE (TRANSFER CREATE) =====

impl PayoutCreateV2 for StripePayouts {}

impl ConnectorIntegrationV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
    for StripePayouts
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}v1/transfers",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = stripe::StripeConnectPayoutCreateRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
        )))
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
        let response: stripe::StripeConnectPayoutCreateResponse = res
            .response
            .parse_struct("StripeConnectPayoutCreateResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT TRANSFER (PAYOUT CREATE) =====

impl PayoutTransferV2 for StripePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for StripePayouts
{
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
        Ok(format!(
            "{}v1/payouts",
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
        let mut headers = self.build_payout_headers(&req.connector_config)?;
        if let Some(ref account_id) = req.request.connector_payout_method_id {
            headers.push((
                headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                Secret::new(account_id.clone()).into_masked(),
            ));
        }
        Ok(headers)
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
        let connector_req = stripe::StripeConnectPayoutFulfillRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
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
        let response: stripe::StripeConnectPayoutFulfillResponse = res
            .response
            .parse_struct("StripeConnectPayoutFulfillResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT GET (PAYOUT RETRIEVE) =====

impl PayoutGetV2 for StripePayouts {}

impl ConnectorIntegrationV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
    for StripePayouts
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let payout_id = req.request.connector_payout_id.clone().ok_or_else(|| {
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }
        })?;
        Ok(format!(
            "{}v1/payouts/{}",
            self.base_url(&req.resource_common_data.connectors),
            payout_id
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut headers = self.build_payout_headers(&req.connector_config)?;
        if let Some(ref account_id) = req.request.connector_payout_method_id {
            headers.push((
                headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                Secret::new(account_id.clone()).into_masked(),
            ));
        }
        Ok(headers)
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
        let response: stripe::StripeConnectPayoutRetrieveResponse = res
            .response
            .parse_struct("StripeConnectPayoutRetrieveResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT VOID (TRANSFER REVERSAL) =====

impl PayoutVoidV2 for StripePayouts {}

impl ConnectorIntegrationV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>
    for StripePayouts
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<String, IntegrationError> {
        let transfer_id = req.request.connector_payout_id.clone().ok_or_else(|| {
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }
        })?;
        Ok(format!(
            "{}v1/transfers/{}/reversals",
            self.base_url(&req.resource_common_data.connectors),
            transfer_id
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = stripe::StripeConnectReversalRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
        )))
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
        let response: stripe::StripeConnectReversalResponse = res
            .response
            .parse_struct("StripeConnectReversalResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT CREATE RECIPIENT (CONNECTED ACCOUNT) =====

impl PayoutCreateRecipientV2 for StripePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for StripePayouts
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}v1/accounts",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = stripe::StripeConnectRecipientCreateRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PayoutCreateRecipient,
            PayoutFlowData,
            PayoutCreateRecipientRequest,
            PayoutCreateRecipientResponse,
        >,
        ConnectorError,
    > {
        let response: stripe::StripeConnectRecipientCreateResponse = res
            .response
            .parse_struct("StripeConnectRecipientCreateResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT ENROLL DISBURSE ACCOUNT (EXTERNAL ACCOUNT) =====

impl PayoutEnrollDisburseAccountV2 for StripePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for StripePayouts
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        let account_id = req.request.connector_payout_id.clone().ok_or_else(|| {
            IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            }
        })?;
        Ok(format!(
            "{}v1/accounts/{}/external_accounts",
            self.base_url(&req.resource_common_data.connectors),
            account_id
        ))
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_payout_headers(&req.connector_config)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
    ) -> CustomResult<Option<common_utils::request::RequestContent>, IntegrationError> {
        let connector_req = stripe::StripeConnectRecipientAccountCreateRequest::try_from(req)?;
        Ok(Some(common_utils::request::RequestContent::FormUrlEncoded(
            Box::new(connector_req),
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PayoutEnrollDisburseAccount,
            PayoutFlowData,
            PayoutEnrollDisburseAccountRequest,
            PayoutEnrollDisburseAccountResponse,
        >,
        ConnectorError,
    > {
        let response: stripe::StripeConnectRecipientAccountCreateResponse = res
            .response
            .parse_struct("StripeConnectRecipientAccountCreateResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

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

// ===== PAYOUT STUB FLOWS (NOT SUPPORTED BY STRIPE) =====

impl PayoutStageV2 for StripePayouts {}

impl ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for StripePayouts
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

impl PayoutCreateLinkV2 for StripePayouts {}

impl
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for StripePayouts
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
