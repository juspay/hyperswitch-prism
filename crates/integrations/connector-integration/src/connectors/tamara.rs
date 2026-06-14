pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    consts,
    crypto::{HmacSha256, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, Eligibility, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentWebhookReference,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse, PaymentsResponseData, PaymentsSyncData,
        RedirectDetailsResponse, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId, WebhookResourceReference,
    },
    errors,
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
    decode::BodyDecoding, verification,
};
use serde::Serialize;
use transformers::{
    TamaraAuthType, TamaraCaptureRequest, TamaraCaptureResponse, TamaraEligibilityRequest,
    TamaraEligibilityResponse, TamaraErrorResponse, TamaraPSyncResponse, TamaraPaymentsRequest,
    TamaraPaymentsResponse, TamaraRSyncResponse, TamaraRefundRequest, TamaraRefundResponse,
    TamaraVoidRequest, TamaraVoidResponse, TamaraWebhookEventType,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

macros::create_amount_converter_wrapper!(connector_name: Tamara, amount_type: MinorUnit);

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

use domain_types::errors::IntegrationError;

macros::create_all_prerequisites!(
    connector_name: Tamara,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: TamaraPaymentsRequest,
            response_body: TamaraPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: TamaraPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: TamaraCaptureRequest,
            response_body: TamaraCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: TamaraVoidRequest,
            response_body: TamaraVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: TamaraRefundRequest,
            response_body: TamaraRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: TamaraRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Eligibility,
            request_body: TamaraEligibilityRequest,
            response_body: TamaraEligibilityResponse,
            router_data: RouterDataV2<Eligibility, PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
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

        pub fn build_headers_generic(
            &self,
            connector_config: &ConnectorSpecificConfig,
            _connectors: &Connectors,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tamara.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.tamara.base_url
        }
    }
);

/// Extract a query parameter value from either the explicit `query_params` field
/// or by parsing the URI's query string.
fn get_query_param(request: &RequestDetails, param_name: &str) -> Option<String> {
    if let Some(ref qp) = request.query_params {
        if let Some(val) = url::form_urlencoded::parse(qp.as_bytes())
            .find(|(k, _)| k == param_name)
            .map(|(_, v)| v.into_owned())
        {
            return Some(val);
        }
    }
    request.uri.as_ref().and_then(|uri_str| {
        uri_str.split('?').nth(1).and_then(|query_str| {
            url::form_urlencoded::parse(query_str.as_bytes())
                .find(|(k, _)| k == param_name)
                .map(|(_, v)| v.into_owned())
        })
    })
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentEligibilityV2 for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Tamara<T>
{
}

// ===== PAYOUT IMPLEMENTATION =====
macros::macro_connector_payout_implementation!(
    connector: Tamara,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== EMPTY TRAIT IMPLEMENTATIONS / STUBBED FLOWS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Tamara<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<errors::WebhookError>> {
        // Tamara sends a tamaraToken JWT in the Authorization header (Bearer) or query params.
        // The JWT is signed with HS256 using the Notification Token (stored as webhook secret).
        // Verify the JWT to confirm the webhook originated from Tamara.
        let token = request
            .headers
            .get("authorization")
            .and_then(|h| h.strip_prefix("Bearer ").map(String::from))
            .or_else(|| get_query_param(&request, "tamaraToken"))
            .ok_or_else(|| {
                error_stack::report!(errors::WebhookError::WebhookSignatureNotFound)
                    .attach_printable(
                        "Missing tamaraToken JWT in Authorization header or query params",
                    )
            })?;

        let secret = connector_webhook_secret.ok_or_else(|| {
            error_stack::report!(errors::WebhookError::WebhookVerificationSecretNotFound)
                .attach_printable("Webhook secret (Notification Token) not configured for Tamara")
        })?;

        // Split JWT into 3 parts: header.payload.signature
        let token_parts: Vec<&str> = token.splitn(3, '.').collect();
        let (header, payload, signature_b64) = match token_parts.as_slice() {
            [h, p, s] => (h, p, *s),
            _ => {
                return Err(error_stack::report!(
                    errors::WebhookError::WebhookSourceVerificationFailed
                )
                .attach_printable("Invalid JWT format: expected 3 dot-separated segments"))
            }
        };

        // JWT message = base64url(header).base64url(payload)
        let message = format!("{}.{}", header, payload);

        // Decode signature from URL-safe base64 without padding
        let signature = consts::BASE64_ENGINE_URL_SAFE_NO_PAD
            .decode(signature_b64)
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to decode JWT signature from base64url")?;

        // Verify HMAC-SHA256
        let computed = HmacSha256
            .sign_message(&secret.secret, message.as_bytes())
            .change_context(errors::WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Failed to sign JWT message with HMAC-SHA256")?;

        Ok(computed == signature)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<errors::WebhookError>> {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(EventType::from(event.event_type))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<errors::WebhookError>> {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: Some(event.order_id),
                merchant_transaction_id: event.order_reference_id,
            },
        )))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<errors::WebhookError>,
    > {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(event.order_id.clone())),
            status: AttemptStatus::from(event.event_type),
            connector_response_reference_id: Some(event.order_id),
            mandate_reference: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<errors::WebhookError>,
    > {
        let event: TamaraWebhookEventType = request
            .body
            .parse_struct("TamaraWebhookEventType")
            .change_context(errors::WebhookError::WebhookBodyDecodingFailed)?;

        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: None,
                status: RefundStatus::from(event.event_type),
                connector_response_reference_id: None,
                error_code: None,
                error_message: None,
                raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
                status_code: 200,
                response_headers: None,
            },
        )
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Tamara<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<verification::ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        let order_id = get_query_param(request, "orderId");

        Ok(RedirectDetailsResponse {
            resource_id: order_id.map(ResponseId::ConnectorTransactionId),
            status: None,
            connector_response_reference_id: None,
            error_code: None,
            error_message: None,
            error_reason: None,
            response_amount: None,
            raw_connector_response: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Tamara<T>
{
}

// Marker traits for flows with real macro_connector_implementation! impls.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Tamara<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Tamara<T>
{
}

macros::macro_connector_flow_status_impls!(
    connector: Tamara,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        CreateOrder,
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
        MandateRevoke,
        ServerAuthenticationToken,
    ],
    not_supported: [
        IncrementalAuthorization,
        VoidPC,
        Accept,
        DefendDispute,
        SubmitEvidence,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        ClientAuthenticationToken,
    ],
);

// ===== AUTHORIZE FLOW IMPLEMENTATION (creates order at /checkout) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_request: Json(TamaraPaymentsRequest),
    curl_response: TamaraPaymentsResponse,
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
            Ok(format!("{}/checkout", self.connector_base_url_payments(req)))
        }
    }
);

// ===== PSYNC FLOW IMPLEMENTATION (calls GET /orders/{order_id}) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_response: TamaraPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!("{}/orders/{}", self.connector_base_url_payments(req), order_id))
        }
    }
);

// ===== CAPTURE FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_request: Json(TamaraCaptureRequest),
    curl_response: TamaraCaptureResponse,
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
            Ok(format!("{}/payments/capture", self.connector_base_url_payments(req)))
        }
    }
);

// ===== VOID FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_request: Json(TamaraVoidRequest),
    curl_response: TamaraVoidResponse,
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
            let order_id = &req.request.connector_transaction_id;
            Ok(format!("{}/orders/{}/cancel", self.connector_base_url_payments(req), order_id))
        }
    }
);

// ===== REFUND FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_request: Json(TamaraRefundRequest),
    curl_response: TamaraRefundResponse,
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
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = &req.request.connector_transaction_id;
            Ok(format!(
                "{}/payments/simplified-refund/{}",
                self.connector_base_url_refunds(req),
                order_id
            ))
        }
    }
);

// ===== RSYNC FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_response: TamaraRSyncResponse,
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
            let mut header = vec![];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/orders/{}", self.connector_base_url_refunds(req), order_id))
        }
    }
);

// ===== ELIGIBILITY FLOW IMPLEMENTATION =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_headers, get_content_type, get_error_response_v2],
    connector: Tamara,
    curl_request: Json(TamaraEligibilityRequest),
    curl_response: TamaraEligibilityResponse,
    flow_name: Eligibility,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodEligibilityData,
    flow_response: PaymentMethodEligibilityResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Eligibility, PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/pre-checkout/v1/eligibility", self.connector_base_url_payments(req)))
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Tamara<T>
{
    fn id(&self) -> &'static str {
        "tamara"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.tamara.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = TamaraAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.expose()).into(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: TamaraErrorResponse = res
            .response
            .parse_struct("TamaraErrorResponse")
            .change_context(errors::ConnectorError::ResponseDeserializationFailed {
                context: Default::default(),
            })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .errors
                .and_then(|e: Vec<_>| e.into_iter().next().map(|e| e.error_code))
                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
            message: response.message,
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

// ===== BODY DECODING IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Tamara<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    verification::SourceVerification for Tamara<T>
{
}
