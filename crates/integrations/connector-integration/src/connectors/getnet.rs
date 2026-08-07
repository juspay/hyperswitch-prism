pub mod transformers;

use domain_types::router_data::ConnectorSpecificConfig;
use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PaymentMethodToken, PostAuthenticate,
        PreAuthenticate, RSync, Refund, ServerAuthenticationToken, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as getnet, GetnetAccessTokenRequest, GetnetAccessTokenResponse, GetnetAuthenticateRequest,
    GetnetAuthenticateResponse, GetnetAuthorizeRequest, GetnetAuthorizeResponse,
    GetnetCaptureRequest, GetnetCaptureResponse, GetnetPostAuthenticateRequest,
    GetnetPostAuthenticateResponse, GetnetPreAuthenticateRequest, GetnetPreAuthenticateResponse,
    GetnetRefundRequest, GetnetRefundResponse, GetnetRefundSyncResponse, GetnetSyncResponse,
    GetnetTokenizeRequest, GetnetTokenizeResponse, GetnetVoidRequest, GetnetVoidResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_TRANSACTION_CHANNEL_ENTRY: &str = "x-transaction-channel-entry";
}

const TRANSACTION_CHANNEL_ENTRY_DEFAULT: &str = "XX";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Getnet<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Getnet,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Getnet<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Getnet<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }

    /// Globalgetnet cards are charged via the Cofre vault token rather than the raw
    /// PAN, so card payments are auto-tokenized (`POST /dpm/cofre-gw-proxy/v1/tokens/card`)
    /// before Authorize. The resulting `number_token` is then sent on `/payments`
    /// (and is mandatory for the 3DS-authenticated final Authorize).
    fn should_do_payment_method_token(
        &self,
        payment_method: common_enums::PaymentMethod,
        _payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        matches!(payment_method, common_enums::PaymentMethod::Card)
    }

    /// Drive the composite authorize flow through Globalgetnet's 3DS chain for card +
    /// ThreeDs: PreAuthenticate (`enrolments-initial`) → Authenticate (`enrolments-continue`)
    /// → [PostAuthenticate (`validations`) when a browser challenge is returned] → Authorize.
    /// The frictionless sandbox path skips PostAuthenticate and goes straight to Authorize
    /// once Authenticate completes. Any other payment method / auth type goes directly to
    /// Authorize.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authenticate
                }
                // Frictionless: Authenticate resolved without a challenge → charge.
                (RedirectState::InitialRequest, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }
                // Challenge: the customer posted the ACS result back → validate it.
                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::PostAuthenticate
                }
                (RedirectState::RedirectWithParams, Some(AuthenticationStep::PostAuthenticate)) => {
                    AuthenticationStep::Authorize
                }
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

macros::create_all_prerequisites!(
    connector_name: Getnet,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GetnetAuthorizeRequest<T>,
            response_body: GetnetAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GetnetCaptureRequest,
            response_body: GetnetCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: GetnetSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GetnetRefundRequest,
            response_body: GetnetRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: GetnetRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: GetnetVoidRequest,
            response_body: GetnetVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: GetnetAccessTokenRequest,
            response_body: GetnetAccessTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: GetnetPreAuthenticateRequest,
            response_body: GetnetPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: GetnetAuthenticateRequest,
            response_body: GetnetAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: GetnetPostAuthenticateRequest,
            response_body: GetnetPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: GetnetTokenizeRequest<T>,
            response_body: GetnetTokenizeResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers(
            &self,
            access_token: &str,
        ) -> Vec<(String, Maskable<String>)> {
            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {access_token}").into(),
                ),
                (
                    headers::X_TRANSACTION_CHANNEL_ENTRY.to_string(),
                    TRANSACTION_CHANNEL_ENTRY_DEFAULT.to_string().into(),
                ),
            ]
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.getnet.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.getnet.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.getnet.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Getnet<T>
{
    fn id(&self) -> &'static str {
        "getnet"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.getnet.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: getnet::GetnetErrorResponse = res
            .response
            .parse_struct("GetnetErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "getnet: response body did not match the expected format; confirm API version and connector documentation."),
            )
            .attach_printable("Failed to deserialize Getnet error response")?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.unwrap_or_else(|| "UNKNOWN_ERROR".to_string()),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
            typed_connector_response: typed,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetAuthorizeRequest),
    curl_response: GetnetAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            use domain_types::payment_method_data::{BankTransferData, PaymentMethodData, VoucherData};

            // Boleto, Pix QR, and the canonical `/payments` endpoint all use
            // different subpaths with *different* request schemas. The
            // discriminator must match the one used inside `TryFrom<...> for
            // GetnetAuthorizeRequest` so URL and body never drift.
            //
            // Note: Pix Automatico (`Pix` PM with `setup_future_usage ==
            // OffSession`) still falls through to `/payments` here, but the
            // body-side TryFrom returns `NotSupported` before the request fires,
            // so this branch is unreachable at runtime for that case.
            let path = match &req.request.payment_method_data {
                PaymentMethodData::Voucher(VoucherData::Boleto(_)) => {
                    "/dpm/payments-gwproxy/v2/payments/boleto"
                }
                PaymentMethodData::BankTransfer(bt)
                    if matches!(bt.as_ref(), BankTransferData::Pix { .. })
                        && !matches!(
                            req.request.setup_future_usage,
                            Some(common_enums::FutureUsage::OffSession)
                        ) =>
                {
                    "/dpm/payments-gwproxy/v2/payments/qrcode/pix"
                }
                _ => "/dpm/payments-gwproxy/v2/payments",
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetCaptureRequest),
    curl_response: GetnetCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/payments-gwproxy/v2/payments/capture", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_response: GetnetSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })
                .attach_printable("Missing connector transaction ID")?;
            Ok(format!("{}/dpm/hub-payment-info/v1/payments/info/{}", self.connector_base_url_payments(req), payment_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetRefundRequest),
    curl_response: GetnetRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/payments-gwproxy/v2/payments/cancel", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetVoidRequest),
    curl_response: GetnetVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/payments-gwproxy/v2/payments/cancel", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_response: GetnetRefundSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/dpm/hub-payment-info/v1/payments/info/{}", self.connector_base_url_refunds(req), payment_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: FormUrlEncoded(GetnetAccessTokenRequest),
    curl_response: GetnetAccessTokenResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = getnet::GetnetAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;

            // Generate Base64(client_id:client_secret) for Basic Auth
            let auth_value = format!("{}:{}", auth.api_key.peek(), auth.api_secret.peek());
            let encoded_auth = base64::engine::general_purpose::STANDARD.encode(auth_value.as_bytes());

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/x-www-form-urlencoded".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {encoded_auth}").into_masked(),
                ),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/authentication/oauth2/access_token", self.connector_base_url_merchant_auth(req)))
        }

    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetPreAuthenticateRequest),
    curl_response: GetnetPreAuthenticateResponse,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/security-gwproxy/v2/enrolments-initial", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetAuthenticateRequest),
    curl_response: GetnetAuthenticateResponse,
    flow_name: Authenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/security-gwproxy/v2/enrolments-continue", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetPostAuthenticateRequest),
    curl_response: GetnetPostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/security-gwproxy/v2/validations", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Getnet,
    curl_request: Json(GetnetTokenizeRequest),
    curl_response: GetnetTokenizeResponse,
    flow_name: PaymentMethodToken,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodTokenizationData<T>,
    flow_response: PaymentMethodTokenResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let access_token = req.resource_common_data.get_access_token()
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })
                .attach_printable("Failed to obtain access token")?;
            Ok(self.build_headers(&access_token))
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/dpm/cofre-gw-proxy/v1/tokens/card", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Getnet,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        VoidPC,
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        CreateOrder,
        ServerSessionAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        ClientAuthenticationToken,
    ],
);
