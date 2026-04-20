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
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as fiservcommercehub;
use transformers::{
    FiservcommercehubAccessTokenRequest, FiservcommercehubAccessTokenResponse,
    FiservcommercehubAuthorizeRequest, FiservcommercehubAuthorizeResponse,
    FiservcommercehubCaptureRequest, FiservcommercehubCaptureResponse,
    FiservcommercehubPSyncRequest, FiservcommercehubPSyncResponse, FiservcommercehubRSyncRequest,
    FiservcommercehubRSyncResponse, FiservcommercehubRefundRequest,
    FiservcommercehubRefundResponse, FiservcommercehubRepeatPaymentRequest,
    FiservcommercehubRepeatResponse, FiservcommercehubSetupMandateRequest,
    FiservcommercehubSetupMandateResponse, FiservcommercehubVoidRequest,
    FiservcommercehubVoidResponse,
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
        ),
        (
            flow: Capture,
            request_body: FiservcommercehubCaptureRequest,
            response_body: FiservcommercehubCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: FiservcommercehubRepeatPaymentRequest,
            response_body: FiservcommercehubRepeatResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: FiservcommercehubSetupMandateRequest,
            response_body: FiservcommercehubSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
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

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };

            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
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

            auth.build_hmac_headers(
                self.common_get_content_type(),
                request_body_str,
            )
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

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };

            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
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

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };

            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )

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

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };

            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
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

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };
            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
        }

        /// Builds the HMAC-authenticated headers for the Capture endpoint.
        pub fn build_capture_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };
            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
        }

        /// Builds the HMAC-authenticated headers for the RepeatPayment endpoint.
        pub fn build_repeat_payment_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        {
            let auth =
                fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                    .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            let temp_request_body = self.get_request_body(req)?;
            let request_body_str = match temp_request_body {
                Some(RequestContent::Json(json_body)) => serde_json::to_string(&json_body)
                    .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?,
                None => String::new(),
                _ => return Err(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?
            };
            auth.build_hmac_headers(
                self.common_get_content_type(),
                &request_body_str,
            )
        }

        /// Builds the HMAC-authenticated headers for the SetupMandate endpoint.
        pub fn build_setup_mandate_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
            request_body_str: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        {
            let auth = fiservcommercehub::FiservcommercehubAuthType::try_from(&req.connector_config)
                      .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            auth.build_hmac_headers(
                self.common_get_content_type(),
                request_body_str,
            )
        }
    }
);

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
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
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>
    for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        MandateRevoke,
        PaymentFlowData,
        MandateRevokeRequestData,
        MandateRevokeResponseData,
    > for Fiservcommercehub<T>
{
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
    ConnectorIntegrationV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerSessionAuthenticationToken,
        PaymentFlowData,
        ServerSessionAuthenticationTokenRequestData,
        ServerSessionAuthenticationTokenResponseData,
    > for Fiservcommercehub<T>
{
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
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        VoidPC,
        PaymentFlowData,
        PaymentsCancelPostCaptureData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubCaptureRequest),
    curl_response: FiservcommercehubCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_capture_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}payments/v1/charges"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubRepeatPaymentRequest),
    curl_response: FiservcommercehubRepeatResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_repeat_payment_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}payments/v1/charges"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Fiservcommercehub,
    curl_request: Json(FiservcommercehubSetupMandateRequest),
    curl_response: FiservcommercehubSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
          fn build_request_v2(
            &self,
            req: &RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
        ) -> CustomResult<Option<common_utils::request::Request>, errors::IntegrationError> {
            use common_utils::request::{Method, RequestBuilder};

            let input_data = FiservcommercehubRouterData {
                connector: self.to_owned(),
                router_data: req.clone()
};
            let request_body: FiservcommercehubSetupMandateRequest =
                FiservcommercehubSetupMandateRequest::try_from(input_data)?;
            let request_body_str = serde_json::to_string(&request_body)
                .change_context(errors::IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            let headers = self.build_setup_mandate_headers(req, &request_body_str)?;

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

        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}payments-vas/v1/tokens"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ClientAuthenticationToken,
        PaymentFlowData,
        ClientAuthenticationTokenRequestData,
        PaymentsResponseData,
    > for Fiservcommercehub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>
    for Fiservcommercehub<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Fiservcommercehub<T>
{
}
