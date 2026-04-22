pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::RequestContent, FloatMajorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, ClientAuthenticationToken,
        CreateConnectorCustomer, CreateOrder, DefendDispute, IncrementalAuthorization,
        MandateRevoke, PSync, PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund,
        RepeatPayment, ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence, Void, VoidPC,
    },
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData,
    },
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as fiservcommercehub;
use transformers::{
    FiservcommercehubAccessTokenRequest, FiservcommercehubAccessTokenResponse,
    FiservcommercehubAuthorizeRequest, FiservcommercehubAuthorizeResponse,
    FiservcommercehubPSyncRequest, FiservcommercehubPSyncResponse, FiservcommercehubRSyncRequest,
    FiservcommercehubRSyncResponse, FiservcommercehubRefundRequest,
    FiservcommercehubRefundResponse, FiservcommercehubVoidRequest, FiservcommercehubVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const API_KEY: &str = "Api-Key";
    pub(crate) const TIMESTAMP: &str = "Timestamp";
    pub(crate) const CLIENT_REQUEST_ID: &str = "Client-Request-Id";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const AUTH_TOKEN_TYPE: &str = "Auth-Token-Type";
    pub(crate) const ACCEPT_LANGUAGE: &str = "Accept-Language";
    pub(crate) const AUTH_TOKEN_TYPE_HMAC: &str = "HMAC";
    pub(crate) const ACCEPT_LANGUAGE_EN: &str = "en";
}

fn fiservcommercehub_flow_not_supported(
    flow: &str,
) -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::FlowNotSupported {
        flow: flow.to_string(),
        connector: "Fiservcommercehub".to_string(),
        context: Default::default(),
    })
}

// =============================================================================
// MACRO PREREQUISITES — creates Fiservcommercehub<T> struct, FiservcommercehubRouterData,
// bridge types, Clone impl, and new()
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Fiservcommercehub,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: FiservcommercehubAccessTokenRequest,
            response_body: FiservcommercehubAccessTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, PaymentFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: FiservcommercehubAuthorizeRequest,
            response_body: FiservcommercehubAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: FiservcommercehubPSyncRequest,
            response_body: FiservcommercehubPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: FiservcommercehubRefundRequest,
            response_body: FiservcommercehubRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: FiservcommercehubVoidRequest,
            response_body: FiservcommercehubVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            request_body: FiservcommercehubRSyncRequest,
            response_body: FiservcommercehubRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_access_token_headers(
            &self,
            req: &RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
};

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                &request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }

        pub fn build_authorize_headers(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            request_body_str: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data
                .connectors
                .fiservcommercehub
                .base_url
                .clone()
        }

        pub fn build_psync_headers(
            &self,
            req: &RouterDataV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<
                PSync,
                PaymentFlowData,
                PaymentsSyncData,
                PaymentsResponseData,
            >,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
};

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                &request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }

        pub fn build_void_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
};

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                &request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }

        /// Builds the HMAC-authenticated headers for the Refund endpoint.
        pub fn build_refund_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
};

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                &request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }

        /// Builds the HMAC-authenticated headers for the RSync (refund transaction-inquiry) endpoint.
        pub fn build_rsync_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let api_key = auth.api_key.clone().expose();
            let client_request_id =
                fiservcommercehub::FiservcommercehubAuthType::generate_client_request_id();
            let timestamp =
                fiservcommercehub::FiservcommercehubAuthType::generate_timestamp();

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
};

            let authorization = auth.generate_hmac_signature(
                &api_key,
                &client_request_id,
                &timestamp,
                &request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key).into_masked(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    authorization.into_masked(),
                ),
                (headers::AUTH_TOKEN_TYPE.to_string(), headers::AUTH_TOKEN_TYPE_HMAC.into()),
                (headers::ACCEPT_LANGUAGE.to_string(), headers::ACCEPT_LANGUAGE_EN.into()),
            ])
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================

fn fiservcommercehub_not_implemented(flow: &str) -> error_stack::Report<errors::IntegrationError> {
    error_stack::report!(errors::IntegrationError::not_implemented(format!(
        "{flow} flow for fiservcommercehub"
    )))
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Fiservcommercehub<T>
{
    fn id(&self) -> &'static str {
        "fiservcommercehub"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, _connectors: &'a Connectors) -> &'a str {
        &_connectors.fiservcommercehub.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: fiservcommercehub::FiservcommercehubErrorResponse = res
            .response
            .parse_struct("FiservcommercehubErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "fiservcommercehub: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error
            .first()
            .and_then(|e| e.code.clone())
            .or_else(|| response.error.first().map(|e| e.error_type.clone()))
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let message = response
            .error
            .first()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "Unknown error occurred".to_string());

        let reason = {
            let reasons: Vec<String> = response
                .error
                .iter()
                .map(|e| {
                    let error_type = &e.error_type;
                    let mut parts = vec![format!("[{error_type}]")];
                    if let Some(code) = &e.code {
                        parts.push(format!("Code: {code}"));
                    }
                    if let Some(field) = &e.field {
                        parts.push(format!("Field: {field}"));
                    }
                    parts.push(e.message.clone());
                    parts.join(" | ")
                })
                .collect();
            if reasons.is_empty() {
                None
            } else {
                Some(reasons.join("; "))
            }
        };

        let connector_transaction_id = response
            .gateway_response
            .as_ref()
            .and_then(|gr| gr.transaction_processing_details.as_ref())
            .and_then(|tpd| tpd.transaction_id.clone());

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason,
            attempt_status: None,
            connector_transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Fiservcommercehub<T>
{
}

// =============================================================================
// ACCESS TOKEN FLOW IMPLEMENTATION
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubAccessTokenRequest),
    curl_response: FiservcommercehubAccessTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: PaymentFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_access_token_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<
                ServerAuthenticationToken,
                PaymentFlowData,
                ServerAuthenticationTokenRequestData,
                ServerAuthenticationTokenResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}security/v1/keys/generate"))
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Fiservcommercehub<T>
{
}

// ===== FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentIncrementalAuthorization for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Fiservcommercehub<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Fiservcommercehub,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Fiservcommercehub<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Fiservcommercehub<T>
{
}

// ===== CONNECTOR INTEGRATION V2 IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>
    for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("accept_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateConnectorCustomer,
            PaymentFlowData,
            ConnectorCustomerData,
            ConnectorCustomerResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported(
            "create_connector_customer",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("defend_dispute"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            MandateRevoke,
            PaymentFlowData,
            MandateRevokeRequestData,
            MandateRevokeResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("mandate_revoke"))
    }
}

// ServerAuthenticationToken is implemented via macro_connector_implementation! above.

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("authenticate"))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubAuthorizeRequest),
    curl_response: FiservcommercehubAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}payments/v1/charges"))
        }

        fn build_request_v2(
            &self,
            req: &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Option<common_utils::request::Request>, errors::IntegrationError> {
            use common_utils::request::{Method, RequestBuilder};

            let input_data = FiservcommercehubRouterData {
                connector: self.to_owned(),
                router_data: req.clone()
};
            let request_body: FiservcommercehubAuthorizeRequest =
                FiservcommercehubAuthorizeRequest::try_from(input_data)?;
            let request_body_str = serde_json::to_string(&request_body)
                .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let headers = self.build_authorize_headers(req, &request_body_str)?;

            let url = self.get_url(req)?;
            let request_content = RequestContent::Json(Box::new(request_body));

            Ok(Some(
                RequestBuilder::new()
                    .method(Method::Post)
                    .url(&url)
                    .attach_default_headers()
                    .headers(headers)
                    .set_optional_body(Some(request_content))
                    .build(),
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_not_implemented("capture"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            IncrementalAuthorization,
            PaymentFlowData,
            PaymentsIncrementalAuthorizationData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported(
            "incremental_authorization",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            CreateOrder,
            PaymentFlowData,
            PaymentCreateOrderData,
            PaymentCreateOrderResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("create_order"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("post_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("pre_authenticate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerSessionAuthenticationToken,
            PaymentFlowData,
            ServerSessionAuthenticationTokenRequestData,
            ServerSessionAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported(
            "create_server_session_authentication_token",
        ))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubPSyncRequest),
    curl_response: FiservcommercehubPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_psync_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!(
                "{base_url}payments/v1/transaction-inquiry"
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PaymentMethodToken,
            PaymentFlowData,
            PaymentMethodTokenizationData<T>,
            PaymentMethodTokenResponse,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("payment_method_token"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            VoidPC,
            PaymentFlowData,
            PaymentsCancelPostCaptureData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("void_post_capture"))
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubVoidRequest),
    curl_response: FiservcommercehubVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_void_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}payments/v1/cancels"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubRSyncRequest),
    curl_response: FiservcommercehubRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_rsync_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = req
                .resource_common_data
                .connectors
                .fiservcommercehub
                .base_url
                .clone();
            Ok(format!("{base_url}payments/v1/transaction-inquiry"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubRefundRequest),
    curl_response: FiservcommercehubRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_refund_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = req
                .resource_common_data
                .connectors
                .fiservcommercehub
                .base_url
                .clone();
            Ok(format!("{base_url}payments/v1/refunds"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            RepeatPayment,
            PaymentFlowData,
            RepeatPaymentData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_not_implemented("repeat_payment"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ClientAuthenticationToken,
            PaymentFlowData,
            ClientAuthenticationTokenRequestData,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported(
            "create_client_authentication_token",
        ))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_not_implemented("setup_mandate"))
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Fiservcommercehub<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            SubmitEvidence,
            DisputeFlowData,
            SubmitEvidenceData,
            DisputeResponseData,
        >,
    ) -> CustomResult<String, errors::IntegrationError> {
        Err(fiservcommercehub_flow_not_supported("submit_evidence"))
    }
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Fiservcommercehub<T>
{
}
