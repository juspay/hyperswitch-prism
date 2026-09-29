pub mod transformers;

use std::fmt::Debug;

use common_enums::{CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{
    crypto::VerifySignature, errors::CustomResult, events, ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        Authorize, Capture, ClientAuthenticationToken, PSync, PaymentMethodToken, RSync, Refund,
        RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets, EventContext, EventType,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as datatrans, DatatransCaptureRequest, DatatransCaptureResponse,
    DatatransClientAuthRequest, DatatransClientAuthResponse, DatatransPaymentsRequest,
    DatatransPaymentsResponse, DatatransRefundRequest, DatatransRefundResponse,
    DatatransRefundSyncResponse, DatatransRepeatPaymentRequest, DatatransRepeatPaymentResponse,
    DatatransSetupMandateRequest, DatatransSetupMandateResponse, DatatransSyncResponse,
    DatatransTokenizeRequest, DatatransTokenizeResponse, DatatransVoidPCRequest,
    DatatransVoidPCResponse, DatatransVoidRequest, DatatransVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Datatrans<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Datatrans<T>
{
}

// ===== PAYMENT METHOD TOKEN FLOW TRAIT IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Datatrans<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Datatrans,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Datatrans<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Datatrans<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Datatrans<T>
{
    /// Verifies `Datatrans-Signature: t=<ts>,s0=<hex>`.
    ///
    /// spec: "Webhook Authentication & Signature Verification" —
    /// https://docs.datatrans.ch/docs/api-webhooks: HMAC-SHA256 with the dashboard key
    /// **hex-decoded** to bytes, over the preimage `String(t) + rawPayload`, compared with `s0`.
    /// Fails closed (`Ok(false)`) when the secret is absent or not hex, or the header is missing
    /// or malformed. The comparison is constant time (`HmacSha256::verify_signature`).
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let Some(secrets) = connector_webhook_secret else {
            return Ok(false);
        };
        let key = match hex::decode(&secrets.secret) {
            Ok(key) if !key.is_empty() => key,
            _ => return Ok(false),
        };
        let Some(signature) = datatrans::DatatransWebhookSignature::from_headers(&request.headers)
        else {
            return Ok(false);
        };
        let message = signature.message(&request.body);

        common_utils::crypto::HmacSha256
            .verify_signature(&key, &signature.s0, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body = datatrans::parse_datatrans_webhook_body(&request.body)?;
        Ok(datatrans::datatrans_webhook_event_type(&body))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body = datatrans::parse_datatrans_webhook_body(&request.body)?;
        Ok(Some(datatrans::datatrans_webhook_reference(&body)))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = datatrans::parse_datatrans_webhook_body(&request.body)?;
        datatrans::build_datatrans_payment_webhook_details(&body, &request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body = datatrans::parse_datatrans_webhook_body(&request.body)?;
        datatrans::build_datatrans_refund_webhook_details(&body, &request.body)
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"transactionId":"260928150333612088","type":"payment","status":"settled","refno":"Test-1234"}"#
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Datatrans<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Datatrans<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Datatrans<T>
{
}
// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Datatrans<T>
{
    /// Google Pay payments are tokenized into a Datatrans alias
    /// (`POST /v1/aliases/tokenize`) before Authorize / SetupMandate. Charging or
    /// registering the alias as an `ALIAS` card is the Datatrans path that supports a
    /// native 3DS challenge for Google Pay (the raw `PAY` payload cannot be
    /// 3DS-authenticated).
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        payment_method_type: Option<PaymentMethodType>,
        _is_wallet_decrypted_network_token: bool,
    ) -> bool {
        matches!(
            (payment_method, payment_method_type),
            (PaymentMethod::Wallet, Some(PaymentMethodType::GooglePay))
        )
    }
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
macros::create_all_prerequisites!(
    connector_name: Datatrans,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: DatatransPaymentsRequest<T>,
            response_body: DatatransPaymentsResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: DatatransSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: DatatransCaptureRequest,
            response_body: DatatransCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: DatatransVoidRequest,
            response_body: DatatransVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: DatatransVoidPCRequest,
            response_body: DatatransVoidPCResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: DatatransRefundRequest,
            response_body: DatatransRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: DatatransRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: DatatransClientAuthRequest,
            response_body: DatatransClientAuthResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: DatatransSetupMandateRequest<T>,
            response_body: DatatransSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: DatatransRepeatPaymentRequest<T>,
            response_body: DatatransRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: DatatransTokenizeRequest,
            response_body: DatatransTokenizeResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![(
                "Content-Type".to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn build_headers_without_content_type<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            // For GET requests - only need auth header, no Content-Type
            let auth_header = self.get_auth_header(&req.connector_config)?;
            Ok(auth_header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.datatrans.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.datatrans.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.datatrans.base_url
        }

        /// Parses the error body via `build_error_response` and sets the flow-aware
        /// `attempt_status` (see `datatrans::datatrans_error_attempt_status`).
        fn build_flow_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
            error_flow: datatrans::DatatransErrorFlow,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let status_code = res.status_code;
            let mut error_response = self.build_error_response(res, event_builder, connector_config)?;
            error_response.attempt_status = datatrans::datatrans_error_attempt_status(error_flow, status_code);
            Ok(error_response)
        }
    }
);

// ===== STUB IMPLEMENTATIONS FOR UNSUPPORTED FLOWS =====

// Payment Void Post Capture

// Setup Mandate

// Repeat Payment

// Order Create

// Session Token

// Dispute Accept

// Dispute Defend

// Submit Evidence

// Payment Token (required by PaymentTokenV2 trait)

// Access Token (required by ServerAuthentication trait)

// ===== AUTHENTICATION FLOW CONNECTOR INTEGRATIONS =====
// Pre Authentication

// Authentication

// Post Authentication

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== SOURCE VERIFICATION IMPLEMENTATIONS =====

// ===== AUTHENTICATION FLOW SOURCE VERIFICATION =====

// ===== CONNECTOR CUSTOMER SOURCE VERIFICATION =====

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Datatrans<T>
{
    fn id(&self) -> &'static str {
        "datatrans"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        // Return the configured base_url (prod vs sandbox per environment), like HS Direct,
        // instead of a hardcoded sandbox literal. Flow URLs are built via
        // `connector_base_url_payments/refunds/merchant_auth`, which read the same config value.
        &connectors.datatrans.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = datatrans::DatatransAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.generate_basic_auth().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: datatrans::DatatransErrorResponse = if res.response.is_empty() {
            datatrans::DatatransErrorResponse::default()
        } else {
            // Datatrans normally returns `{"error":{"code","message"}}`, but some gateway errors
            // come back as a non-JSON (HTML) page. Fall back to surfacing the raw body text
            // instead of failing with a deserialization error (mirrors HS Direct).
            res.response
                .parse_struct("DatatransErrorResponse")
                .ok()
                .unwrap_or_else(|| {
                    datatrans::DatatransErrorResponse::from_non_json_body(res.response.as_ref())
                })
        };

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code(),
            message: response.message(),
            reason: Some(response.message()),
            // Flow-agnostic default (always `None`); flows that own a terminal failure
            // override `get_error_response_v2` via `build_flow_error_response`.
            attempt_status: datatrans::datatrans_error_attempt_status(
                datatrans::DatatransErrorFlow::Other,
                res.status_code,
            ),
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransPaymentsRequest<T>),
    curl_response: DatatransPaymentsResponse,
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
            let base_url = self.connector_base_url_payments(req);
            // Endpoint selection mirrors HS Direct (see `authorize_uses_init_endpoint`):
            // native 3DS or a raw-card CIT alias registration use the redirect-capable
            // `/v1/transactions` init endpoint; passthrough external 3DS and no-3DS payments
            // use `/v1/transactions/authorize`, which authorizes immediately with no redirect.
            // (MIT is served by the separate RepeatPayment flow -> `/v1/transactions/authorize`.)
            if datatrans::authorize_uses_init_endpoint(req) {
                Ok(format!("{base_url}/v1/transactions"))
            } else {
                Ok(format!("{base_url}/v1/transactions/authorize"))
            }
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Payment)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_response: DatatransSyncResponse,
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
            self.build_headers_without_content_type(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{}/v1/transactions/{}", self.connector_base_url_payments(req), transaction_id))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Sync)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransCaptureRequest),
    curl_response: DatatransCaptureResponse,
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
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;
            Ok(format!("{}/v1/transactions/{}/settle", self.connector_base_url_payments(req), transaction_id))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Capture)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransVoidRequest),
    curl_response: DatatransVoidResponse,
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
            let transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/transactions/{}/cancel", self.connector_base_url_payments(req), transaction_id))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Void)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Datatrans,
    curl_request: Json(DatatransVoidPCRequest),
    curl_response: DatatransVoidPCResponse,
    flow_name: VoidPC,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCancelPostCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/transactions/{}/cancel", self.connector_base_url_payments(req), transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransRefundRequest),
    curl_response: DatatransRefundResponse,
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
            let transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/transactions/{}/credit", self.connector_base_url_refunds(req), transaction_id))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Refund)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_response: DatatransRefundSyncResponse,
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
            self.build_headers_without_content_type(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let refund_transaction_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/v1/transactions/{}", self.connector_base_url_refunds(req), refund_transaction_id))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Sync)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Datatrans,
    curl_request: Json(DatatransClientAuthRequest),
    curl_response: DatatransClientAuthResponse,
    flow_name: ClientAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ClientAuthenticationTokenRequestData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/v1/transactions/secureFields", self.connector_base_url_merchant_auth(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransSetupMandateRequest<T>),
    curl_response: DatatransSetupMandateResponse,
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
            // Endpoint selection follows the request body (see `setup_mandate_uses_validate`):
            // a raw card without 3DS uses the zero-amount card check `/v1/transactions/validate`,
            // which returns `card.alias` synchronously; native 3DS or a Google Pay alias
            // registration uses the redirect-capable `/v1/transactions` init endpoint.
            let base_url = self.connector_base_url_payments(req);
            if datatrans::setup_mandate_uses_validate(req) {
                Ok(format!("{base_url}/v1/transactions/validate"))
            } else {
                Ok(format!("{base_url}/v1/transactions"))
            }
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Payment)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Datatrans,
    curl_request: Json(DatatransRepeatPaymentRequest<T>),
    curl_response: DatatransRepeatPaymentResponse,
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
            // MIT charges the stored alias via the split authorize endpoint (no redirect):
            // the alias was already 3DS-authenticated at SetupMandate time.
            Ok(format!("{}/v1/transactions/authorize", self.connector_base_url_payments(req)))
        }
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_flow_error_response(res, event_builder, connector_config, datatrans::DatatransErrorFlow::Payment)
        }
    }
);

// PaymentMethodToken (Google Pay alias tokenization) Flow — converts the Google Pay
// wallet payload into a Datatrans alias via POST /v1/aliases/tokenize, so the
// subsequent Authorize can charge it as an `ALIAS` card with native 3DS support.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Datatrans,
    curl_request: Json(DatatransTokenizeRequest),
    curl_response: DatatransTokenizeResponse,
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
            Ok(format!("{}/v1/aliases/tokenize", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Datatrans,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        CreateOrder,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        DefendDispute,
        SubmitEvidence,
    ],
);
