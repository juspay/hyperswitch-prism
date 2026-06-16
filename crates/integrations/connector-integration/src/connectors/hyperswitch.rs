pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PaymentMethodToken, RSync, Refund, RepeatPayment,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate, Void,
    },
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext, WebhookError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as hyperswitch, HyperswitchAccessTokenRequest, HyperswitchAccessTokenResponse,
    HyperswitchCaptureRequest, HyperswitchCaptureResponse, HyperswitchErrorResponse,
    HyperswitchPSyncResponse, HyperswitchPaymentsRequest, HyperswitchPaymentsResponse,
    HyperswitchRSyncResponse, HyperswitchRefundRequest, HyperswitchRefundResponse,
    HyperswitchRepeatPaymentRequest, HyperswitchRepeatPaymentResponse, HyperswitchSessionRequest,
    HyperswitchSessionResponse, HyperswitchSetupMandateRequest, HyperswitchSetupMandateResponse,
    HyperswitchTokenResponse, HyperswitchTokenizationRequest, HyperswitchVoidRequest,
    HyperswitchVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, utils, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const API_KEY: &str = "api-key";
}

macros::macro_connector_payout_implementation!(
    connector: Hyperswitch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Hyperswitch<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerSessionAuthentication for Hyperswitch<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Hyperswitch<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Hyperswitch, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Hyperswitch,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: HyperswitchPaymentsRequest<T>,
            response_body: HyperswitchPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: HyperswitchPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: HyperswitchCaptureRequest,
            response_body: HyperswitchCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: HyperswitchVoidRequest,
            response_body: HyperswitchVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: HyperswitchRefundRequest,
            response_body: HyperswitchRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: HyperswitchRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: HyperswitchSetupMandateRequest<T>,
            response_body: HyperswitchSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: HyperswitchRepeatPaymentRequest,
            response_body: HyperswitchRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: HyperswitchTokenizationRequest<T>,
            response_body: HyperswitchTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: ServerSessionAuthenticationToken,
            request_body: HyperswitchSessionRequest,
            response_body: HyperswitchSessionResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: ServerAuthenticationToken,
            request_body: HyperswitchAccessTokenRequest,
            response_body: HyperswitchAccessTokenResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.get_content_type().to_string().into(),
            )];
            let mut api_key = self
                .get_auth_header(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType {
                    context: Default::default(),
                })?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.hyperswitch.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.hyperswitch.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.hyperswitch.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Hyperswitch<T>
{
    fn id(&self) -> &'static str {
        "hyperswitch"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.hyperswitch.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = hyperswitch::HyperswitchAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Hyperswitch requires API key authentication".to_owned(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        Ok(vec![(
            headers::API_KEY.to_string(),
            auth.api_key.peek().to_string().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: HyperswitchErrorResponse = res
            .response
            .parse_struct("HyperswitchErrorResponse")
            .change_context(utils::response_deserialization_fail(
                res.status_code,
                "hyperswitch: response body did not match the expected error format.",
            ))?;

        with_error_response_body!(event_builder, response);

        let error = response.error;
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: error
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: error
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: error.message,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

// ===== AUTHORIZE =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchPaymentsRequest),
    curl_response: HyperswitchPaymentsResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_payments(req)))
        }
    }
);

// ===== PSYNC =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_response: HyperswitchPSyncResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/payments/{connector_payment_id}",
                self.connector_base_url_payments(req),
            ))
        }
    }
);

// ===== CAPTURE =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchCaptureRequest),
    curl_response: HyperswitchCaptureResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!(
                "{}/payments/{connector_payment_id}/capture",
                self.connector_base_url_payments(req)
            ))
        }
    }
);

// ===== VOID =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchVoidRequest),
    curl_response: HyperswitchVoidResponse,
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
            self.build_headers(req)
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

// ===== REFUND =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchRefundRequest),
    curl_response: HyperswitchRefundResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/refunds", self.connector_base_url_refunds(req)))
        }
    }
);

// ===== RSYNC =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_response: HyperswitchRSyncResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_refund_id = req.request.connector_refund_id.clone();
            Ok(format!(
                "{}/refunds/{connector_refund_id}",
                self.connector_base_url_refunds(req)
            ))
        }
    }
);

// ===== SETUP MANDATE (POST /payments + setup_future_usage + customer_acceptance) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchSetupMandateRequest),
    curl_response: HyperswitchSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_payments(req)))
        }
    }
);

// ===== REPEAT PAYMENT (MIT recurring charge, POST /payments + recurring_details) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchRepeatPaymentRequest),
    curl_response: HyperswitchRepeatPaymentResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_payments(req)))
        }
    }
);

// ===== PAYMENT METHOD TOKEN (Tokenization, POST /payment_methods) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchTokenizationRequest),
    curl_response: HyperswitchTokenResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payment_methods", self.connector_base_url_payments(req)))
        }
    }
);

// ===== SESSION (ServerSessionAuthenticationToken, POST /payments → client_secret) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchSessionRequest),
    curl_response: HyperswitchSessionResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_merchant_auth(req)))
        }
    }
);

// ===== ACCESS TOKEN (ServerAuthenticationToken) — DEGENERATE / non-functional =====
// Hyperswitch uses a static `api-key` header and has NO token-mint endpoint
// (tech spec section 11). This flow exists only for surface parity: the macro
// requires an HTTP call, so we POST a benign body to `/payments`; the permissive
// response struct accepts any JSON object (success or error) and the response
// transformer echoes the configured `api-key` as the "access token". NOT for
// production token exchange.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Hyperswitch,
    curl_request: Json(HyperswitchAccessTokenRequest),
    curl_response: HyperswitchAccessTokenResponse,
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
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/payments", self.connector_base_url_merchant_auth(req)))
        }
    }
);

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Hyperswitch<T>
{
}

// Hyperswitch signs outgoing webhooks with HMAC-SHA512 over the raw JSON body,
// hex-encoded in the `X-Webhook-Signature-512` header, keyed by the merchant's
// `payment_response_hash_key` (delivered here as `ConnectorWebhookSecrets.secret`,
// used as raw bytes — NOT hex/base64-decoded). Verification is advisory: the
// gRPC handler processes the webhook regardless and surfaces `source_verified`.
const HYPERSWITCH_WEBHOOK_SIGNATURE_HEADER: &str = "x-webhook-signature-512";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Hyperswitch<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_hex = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(HYPERSWITCH_WEBHOOK_SIGNATURE_HEADER))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))?;
        hex::decode(signature_hex).change_context(WebhookError::WebhookSignatureNotFound)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        // Hyperswitch signs the raw request body verbatim.
        Ok(request.body.clone())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Fail-closed at the flag level (never use a default secret): no secret =>
        // not verified. The handler still processes the webhook.
        let secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => return Ok(false),
        };
        // Missing/garbled signature header => unverified (truthful Ok(false)).
        let signature = match self.get_webhook_source_verification_signature(&request, &secrets) {
            Ok(signature) => signature,
            Err(_) => return Ok(false),
        };
        let message = self.get_webhook_source_verification_message(&request, &secrets)?;
        Ok(crypto::HmacSha512
            .verify_signature(&secrets.secret, &signature, &message)
            .unwrap_or(false))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = hyperswitch::get_webhook_object_from_body(&request.body)?;
        Ok(hyperswitch::map_webhook_event_type(&body.event_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body = hyperswitch::get_webhook_object_from_body(&request.body)?;
        hyperswitch::get_webhook_reference(&body)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = hyperswitch::get_webhook_object_from_body(&request.body)?;
        hyperswitch::build_webhook_payment_response(&body, &request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = hyperswitch::get_webhook_object_from_body(&request.body)?;
        hyperswitch::build_webhook_refund_response(&body, &request.body)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let body = hyperswitch::get_webhook_object_from_body(&request.body)?;
        let resource: Box<dyn hyperswitch_masking::ErasedMaskSerialize> = Box::new(body);
        Ok(resource)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"merchant_id":"probe_merchant","event_id":"evt_probe_001","event_type":"payment_succeeded","content":{"type":"payment_details","object":{"payment_id":"pay_probe001","status":"succeeded","connector_transaction_id":"txn_probe001"}}}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Hyperswitch<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Hyperswitch<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Hyperswitch<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Hyperswitch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        CreateConnectorCustomer,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        MandateRevoke,
    ],
    not_supported: [
        IncrementalAuthorization,
        SubmitEvidence,
        DefendDispute,
        Accept,
        VoidPC,
    ],
);
