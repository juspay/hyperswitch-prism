pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::FloatMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, PSync, Refund, ServerAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundsData, RefundsResponseData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    PaylaterAuthType, PaylaterAuthorizeRequest, PaylaterAuthorizeResponse, PaylaterErrorResponse,
    PaylaterPSyncResponse, PaylaterRefundRequest, PaylaterRefundResponse,
    PaylaterWebCheckoutRequest, PaylaterWebCheckoutResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

pub(crate) const FORM_URL_ENCODED: &str = "application/x-www-form-urlencoded";

use domain_types::errors::IntegrationError;

macros::create_amount_converter_wrapper!(connector_name: Paylater, amount_type: FloatMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Paylater,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: PaylaterAuthorizeRequest,
            response_body: PaylaterAuthorizeResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: PaylaterWebCheckoutRequest,
            response_body: PaylaterWebCheckoutResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaylaterPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PaylaterRefundRequest,
            response_body: PaylaterRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paylater.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paylater.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paylater.base_url
        }

        /// Extract the short-lived Bearer JWT obtained via the
        /// `ServerAuthenticationToken` flow. The composite flow runs the
        /// token call first and stores the result on
        /// `resource_common_data.access_token`.
        pub fn extract_access_token(
            &self,
            access_token: Option<&ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<hyperswitch_masking::Secret<String>, IntegrationError> {
            access_token
                .map(|t| t.access_token.clone())
                .ok_or_else(|| {
                    IntegrationError::FailedToObtainAuthType {
                        context: Default::default(),
                    }
                    .into()
                })
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paylater<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Paylater<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paylater<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Paylater,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: Paylater,
    curl_request: FormUrlEncoded(PaylaterAuthorizeRequest),
    curl_response: PaylaterAuthorizeResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            FORM_URL_ENCODED
        }

        fn get_headers(
            &self,
            _req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // This call ITSELF is the token-fetch — credentials go in the
            // form-urlencoded body (client_id/client_secret), not in headers.
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                FORM_URL_ENCODED.to_string().into(),
            )])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/auth/realms/api/protocol/openid-connect/token",
                self.connector_base_url_merchant_auth(req),
            ))
        }
    }
);

// ===== AUTHORIZE — Generate Payment Link (hosted web checkout) =====
// POST /api/paylater/merchant-portal/v2/web-checkout
// Response carries `paymentLinkUrl` → returned as `redirection_data` for the
// shopper redirect. Status → AttemptStatus::AuthenticationPending.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paylater,
    curl_request: Json(PaylaterWebCheckoutRequest),
    curl_response: PaylaterWebCheckoutResponse,
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
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/api/paylater/merchant-portal/v2/web-checkout",
                self.connector_base_url_payments(req),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", token.expose()).into(),
                ),
            ])
        }
    }
);

// ===== PSYNC — Check Payment Status =====
// GET /api/paylater/merchant-portal/v2/web-checkout/status?order_id=<order_id>
// No request body — the GET carries the merchant's `order_id` as a URL-encoded
// query parameter. Bearer auth reuses the access_token cached by the
// ServerAuthenticationToken step of the composite PSync flow.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paylater,
    curl_response: PaylaterPSyncResponse,
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
            // Same Bearer-access-token pattern as Authorize. The OAuth JWT is
            // stored on `resource_common_data.access_token` by the
            // ServerAuthenticationToken step of the composite PSync flow.
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", token.expose()).into(),
                ),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // `order_id` is the merchant's reference — the same value sent in
            // the Authorize body. UCS stores it on
            // `resource_common_data.connector_request_reference_id`.
            // URL-encode defensively: merchants sometimes embed characters
            // like `+`, `/`, or whitespace in their order ids which would
            // otherwise corrupt the query string.
            let order_id = &req.resource_common_data.connector_request_reference_id;
            if order_id.is_empty() {
                return Err(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_request_reference_id (order_id)",
                        context: Default::default(),
                    }
                ));
            }
            Ok(format!(
                "{}/api/paylater/merchant-portal/v2/web-checkout/status?order_id={}",
                self.connector_base_url_payments(req),
                urlencoding::encode(order_id),
            ))
        }
    }
);

// ===== REFUND =====
// POST /api/paylater/merchant-portal/v2/web-checkout/refund
// Body: `{ "order_id": "<merchant order id>" }`. Full-refund only — PayLater
// treats every refund as full; any partial amount supplied by the caller is
// ignored by the gateway. See the rustdoc on `PaylaterRefundRequest` for the
// 29-day / 10-minute gateway-side windows and idempotency notes.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paylater,
    curl_request: Json(PaylaterRefundRequest),
    curl_response: PaylaterRefundResponse,
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
            // Reuse the same Bearer-access-token pattern as Authorize. The OAuth
            // JWT is stored on `resource_common_data.access_token` by the
            // ServerAuthenticationToken step of the composite Refund flow.
            let token =
                self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", token.expose()).into(),
                ),
            ])
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}/api/paylater/merchant-portal/v2/web-checkout/refund",
                self.connector_base_url_refunds(req),
            ))
        }
    }
);

// ===== PAYLATER SPEC: Flows implemented are ServerAuthenticationToken,
// Authorize (redirect BNPL), PSync, and Refund. Capture/Void/RSync
// are NOT supported by the gateway.
macros::macro_connector_flow_status_impls!(
    connector: Paylater,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        SetupMandate,
        PaymentMethodToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        RepeatPayment,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        ClientAuthenticationToken,
        ServerSessionAuthenticationToken,
        MandateRevoke,
    ],
    not_supported: [
        Capture,
        Void,
        VoidPC,
        RSync,
        VoidPostRefund,
        IncrementalAuthorization,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ],
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paylater<T>
{
    fn id(&self) -> &'static str {
        "paylater"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.paylater.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // NOTE: this generic auth-header helper is for direct Bearer-API connectors;
        // PayLater is OAuth client_credentials — its access token is obtained through
        // the ServerAuthenticationToken flow and carried in `state.access_token` on
        // subsequent calls. We do NOT want to surface the long-lived client_secret
        // as the Authorization header, so this helper is intentionally only used
        // where the integration explicitly invokes `get_auth_header` (e.g. tests,
        // any helper that wants a Bearer). For now, sign with the client_id so
        // the result is well-formed but not the oauth secret.
        let auth = PaylaterAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.client_id.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: PaylaterErrorResponse = res
            .response
            .parse_struct("PaylaterErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        // OAuth token endpoint failures ship `error_description` instead of
        // `message`; refund/API failures ship `message`. Prefer `message` and
        // fall back to `error_description` so callers always see the verbatim
        // gateway detail (e.g. "Transaction happened less than 10 minutes ago").
        let message = response
            .message
            .or(response.error_description)
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message,
            reason: None,
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
