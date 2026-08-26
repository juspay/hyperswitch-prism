pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RepeatPaymentData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as moneris, MonerisAuthRequest, MonerisAuthResponse, MonerisAuthorizeResponse,
    MonerisCancelRequest, MonerisCaptureResponse, MonerisPaymentsCaptureRequest,
    MonerisPaymentsRequest, MonerisPaymentsResponse as MonerisRepeatPaymentResponse,
    MonerisPaymentsResponse as MonerisPaymentSyncResponse,
    MonerisPaymentsResponse as MonerisPaymentVoidResponse, MonerisPostAuthenticateRequest,
    MonerisPostAuthenticateResponse, MonerisPreAuthenticateRequest, MonerisPreAuthenticateResponse,
    MonerisRefundRequest, MonerisRefundResponse,
    MonerisRefundResponse as MonerisRefundSyncResponse, MonerisRepeatPaymentRequest,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Moneris<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }

    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        _redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::AuthenticationStep;

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && matches!(payment_method, common_enums::PaymentMethod::Card)
        {
            match completed_step {
                None => AuthenticationStep::PreAuthenticate,
                Some(AuthenticationStep::PreAuthenticate) => AuthenticationStep::Authorize,
                Some(AuthenticationStep::PostAuthenticate) => AuthenticationStep::Authorize,
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Moneris<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Moneris<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Moneris<T>
{
    fn id(&self) -> &'static str {
        "moneris"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.moneris.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Moneris returns JSON errors for application-level failures, but
        // WAF/CDN layers (e.g. Cloudflare) may return HTML for blocked requests.
        // Fall back to a raw-body error when JSON parsing fails so the error
        // propagates cleanly instead of crashing with a parse error.
        let parsed_error = res
            .response
            .parse_struct::<moneris::MonerisErrorResponse>("MonerisErrorResponse");

        let (code, message, reason) = match parsed_error {
            Ok(ref r) => {
                with_error_response_body!(event_builder, r);
                let reason = match &r.errors {
                    Some(error_list) => error_list
                        .iter()
                        .map(|e| format!("{}: {}", e.parameter_name, e.reason_code))
                        .collect::<Vec<_>>()
                        .join(" & "),
                    None => r.title.clone(),
                };
                (r.category.clone(), r.title.clone(), reason)
            }
            Err(_) => {
                let body_preview = String::from_utf8_lossy(&res.response)
                    .chars()
                    .take(300)
                    .collect::<String>();
                let msg = format!("HTTP {}", res.status_code);
                (
                    format!("HTTP_{}", res.status_code),
                    msg.clone(),
                    body_preview,
                )
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: Some(reason),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

macros::create_all_prerequisites!(
    connector_name: Moneris,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: MonerisAuthRequest,
            response_body: MonerisAuthResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: MonerisPaymentsRequest<T>,
            response_body: MonerisAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: MonerisPreAuthenticateRequest<T>,
            response_body: MonerisPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: MonerisPostAuthenticateRequest,
            response_body: MonerisPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: MonerisRepeatPaymentRequest<T>,
            response_body: MonerisRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: MonerisPaymentSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: MonerisPaymentsCaptureRequest,
            response_body: MonerisCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: MonerisCancelRequest,
            response_body: MonerisPaymentVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: MonerisRefundRequest,
            response_body: MonerisRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: MonerisRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers(
            &self,
            access_token: &str,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = moneris::MonerisAuthType::try_from(connector_config)?;
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    moneris::auth_headers::API_VERSION.to_string(),
                    moneris::auth_headers::API_VERSION_VALUE
                        .to_string()
                        .into(),
                ),
                (
                    moneris::auth_headers::X_MERCHANT_ID.to_string(),
                    auth.merchant_id.expose().into_masked(),
                ),
            ];
            let auth_header = (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", access_token).into_masked(),
            );
            header.push(auth_header);
            Ok(header)
        }

        pub fn get_headers_from_access_token(
            &self,
            access_token: Option<ServerAuthenticationTokenResponseData>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = access_token.ok_or_else(|| IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Ensure the OAuth access token is obtained via the \
                         ServerAuthenticationToken flow before initiating this operation."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "Moneris requires an OAuth access token on \
                         `resource_common_data.access_token`, but it was None."
                            .to_string(),
                    ),
                },
            })?;
            self.build_headers(&token.access_token.expose(), connector_config)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.moneris.base_url
        }
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.moneris.base_url
        }
        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.moneris.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [],
    connector: Moneris,
    curl_request: FormUrlEncoded(MonerisAuthRequest),
    curl_response: MonerisAuthResponse,
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
            _req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/x-www-form-urlencoded".to_string().into(),
            )])
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/oauth2/token", self.connector_base_url_merchant_auth(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            // auth error have different structure than common error
            let response: moneris::MonerisAuthErrorResponse = res
                .response
                .parse_struct("MonerisAuthErrorResponse")
                .change_context(utils::response_handling_fail_for_connector(
                    res.status_code,
                    "moneris",
                ))?;
            with_error_response_body!(event_builder, response);
            Ok(ErrorResponse {
                status_code: res.status_code,
                code: response.error.to_string(),
                message: response.error.clone(),
                reason: response.error_description,
                attempt_status: None,
                connector_transaction_id: None,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisPaymentsRequest<T>),
    curl_response: MonerisAuthorizeResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisRepeatPaymentRequest),
    curl_response: MonerisRepeatPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_response: MonerisPaymentSyncResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.get_connector_transaction_id()?;
            Ok(format!(
                "{}/payments/{connector_payment_id}",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisPaymentsCaptureRequest),
    curl_response: MonerisCaptureResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.get_connector_transaction_id()?;
            Ok(format!(
                "{}/payments/{connector_payment_id}/complete",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisCancelRequest),
    curl_response: MonerisPaymentVoidResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req.request.connector_transaction_id.clone();
            Ok(format!(
                "{}/payments/{connector_payment_id}/cancel",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisRefundRequest),
    curl_response: MonerisRefundResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/refunds", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_response: MonerisRefundSyncResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/refunds/{refund_id}", self.connector_base_url_refunds(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisPreAuthenticateRequest<T>),
    curl_response: MonerisPreAuthenticateResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/three-d-secure/authentications",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Moneris,
    curl_request: Json(MonerisPostAuthenticateRequest),
    curl_response: MonerisPostAuthenticateResponse,
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
            self.get_headers_from_access_token(
                req.resource_common_data.access_token.clone(),
                &req.connector_config,
            )
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/three-d-secure/authentication-value-lookups",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Moneris,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
    ],
    not_supported: [
        ServerSessionAuthenticationToken,
        VoidPostRefund,
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        Authenticate,
        ClientAuthenticationToken,
        MandateRevoke,
        VoidPC,
    ],
);
