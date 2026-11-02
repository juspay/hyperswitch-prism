pub mod transformers;

use super::macros;
use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_enums::{PaymentMethod, PaymentMethodType};
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::MinorUnit,
};
use domain_types::{
    connector_flow::{
        Accept, Authenticate, Authorize, Capture, CreateConnectorCustomer, DefendDispute, PSync,
        PaymentMethodToken, PostAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        SubmitEvidence, Void,
    },
    connector_types::*,
    connector_types::{RefundFlowData, RefundSyncData, RefundsResponseData},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, AuthenticationStep, RedirectState},
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as finix, FinixAcceptDisputeRequest, FinixAcceptDisputeResponse,
    FinixAuthenticateResponse, FinixAuthorizeRequest, FinixAuthorizeResponse, FinixCaptureRequest,
    FinixCaptureResponse, FinixCreateIdentityRequest, FinixCreatePaymentInstrumentRequest,
    FinixDefendDisputeRequest, FinixDefendDisputeResponse, FinixIdentityResponse,
    FinixInstrumentResponse, FinixPSyncResponse, FinixPostAuthenticateResponse, FinixRSyncResponse,
    FinixRefundRequest, FinixRefundResponse, FinixRepeatPaymentRequest, FinixRepeatPaymentResponse,
    FinixSetupMandateRequest, FinixSetupMandateResponse, FinixSubmitEvidenceRequest,
    FinixSubmitEvidenceResponse, FinixThreeDSecureSessionRequest, FinixVoidRequest,
    FinixVoidResponse,
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const FINIX_VERSION: &str = "Finix-Version";
}

/// Finix API version pinned on every request. Docs: the header "must be included with
/// every request"; omitted, Finix silently serves base version 2018-01-01.
const FINIX_VERSION_VALUE: &str = "2022-02-01";

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const FINIX_REFERRER_SOURCE_HEADER: &str = "X-Finix-Referrer-Source";
const FINIX_REFERRER_SOURCE_VALUE: &str = "PLUGIN_HYPERSWITCH";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Finix<T>
{
    fn id(&self) -> &'static str {
        "finix"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.finix.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = finix::FinixAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let encoded_auth = auth.generate_basic_auth();
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Basic {}", encoded_auth).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: finix::FinixErrorResponse = res
            .response
            .parse_struct("FinixErrorResponse")
            .change_context(
            crate::utils::response_deserialization_fail(res.status_code, "finix: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.get_code(),
            message: response.get_message(),
            reason: response.get_failure_message(),
            attempt_status: None,
            connector_transaction_id: response.get_resource_id(),
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

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Finix<T>
{
}

// =============================================================================
// VALIDATION TRAIT IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Finix<T>
{
    /// Enable automatic connector customer creation before payment
    /// When enabled, UCS will automatically call CreateConnectorCustomer before Authorize
    fn should_create_connector_customer(&self) -> bool {
        true
    }

    /// Enable automatic payment method tokenization before payment
    /// When enabled, UCS will automatically call PaymentMethodToken before Authorize
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        payment_method_type: Option<PaymentMethodType>,
        _is_wallet_decrypted_network_token: bool,
    ) -> bool {
        // Apple Pay and Google Pay are vaulted as APPLE_PAY / GOOGLE_PAY Payment
        // Instruments before the charge, exactly like cards.
        let is_finix_wallet = payment_method == PaymentMethod::Wallet
            && matches!(
                payment_method_type,
                Some(PaymentMethodType::GooglePay | PaymentMethodType::ApplePay)
            );

        // Card and BankDebit always need tokenization
        matches!(
            payment_method,
            PaymentMethod::Card | PaymentMethod::BankDebit
        ) || is_finix_wallet
    }

    /// Finix 3D Secure Sessions: Authenticate creates the session (POST
    /// /3d_secure_sessions, redirects the buyer to the hosted page), PostAuthenticate
    /// reads the trusted result (GET /3d_secure_sessions/{id}), Authorize charges with
    /// `3d_secure_session`. There is no device-data-collection leg, so no PreAuthenticate.
    /// Wallet is gated in because a tokenized instrument (the PI Finix needs) arrives
    /// as PaymentMethod::Wallet.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: PaymentMethod,
        redirect_state: RedirectState,
        completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        if auth_type != common_enums::AuthenticationType::ThreeDs
            || !matches!(payment_method, PaymentMethod::Card | PaymentMethod::Wallet)
        {
            return AuthenticationStep::Authorize;
        }
        match (redirect_state, completed_step) {
            (RedirectState::InitialRequest, None) => AuthenticationStep::Authenticate,
            (RedirectState::InitialRequest, Some(AuthenticationStep::Authenticate)) => {
                AuthenticationStep::PostAuthenticate
            }
            (RedirectState::InitialRequest, Some(AuthenticationStep::PostAuthenticate)) => {
                AuthenticationStep::Authorize
            }
            (
                RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                Some(AuthenticationStep::PostAuthenticate),
            ) => AuthenticationStep::Authorize,
            (RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams, None) => {
                AuthenticationStep::PostAuthenticate
            }
            _ => AuthenticationStep::Authorize,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Finix<T>
{
}

// ===== FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Finix<T>
{
}

// Ported 1:1 from hyperswitch `impl webhooks::IncomingWebhook for Finix`
// (crates/hyperswitch_connectors/src/connectors/finix.rs). Hyperswitch (Direct
// gateway) is the source of truth: HMAC-SHA256 over `"{timestamp}:{raw body}"`
// with the hex-encoded signature carried in the comma-separated
// `Finix-Signature: timestamp=...,sig=...` header.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Finix<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            tracing::warn!(
                target: "finix_webhook",
                "no webhook secret configured for Finix source verification"
            );
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        // Constant-time compare (ring `hmac::verify`), never `Vec::eq`.
        crypto::HmacSha256
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("failed to verify the HMAC-SHA256 over the Finix webhook message")
    }

    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = finix::decode_finix_signature(&request.headers)?;

        // HS hex-decodes the `sig` value before comparing
        // (get_webhook_source_verification_signature).
        hex::decode(&signature_header.sig)
            .change_context(WebhookError::WebhookSignatureNotFound)
            .attach_printable("Finix-Signature `sig` value is not valid hex")
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let signature_header = finix::decode_finix_signature(&request.headers)?;

        // HS signs `"{timestamp}:{raw body}"` — the RAW request bytes, never a
        // re-serialization (get_webhook_source_verification_message).
        Ok(format!(
            "{}:{}",
            signature_header.timestamp,
            String::from_utf8_lossy(&request.body)
        )
        .into_bytes())
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // HS: an empty body or a body that trims to `{}` is Finix's
        // endpoint-verification probe (get_webhook_event_type).
        if finix::is_endpoint_verification_body(&request.body) {
            return Ok(EventType::EndpointVerification);
        }
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::get_finix_webhook_event_type(&webhook_body)
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        // Endpoint-verification probes carry no resource; HS's Direct gateway
        // also resolves no reference for them (`get_webhook_object_reference_id`
        // fails to parse `{}` and the router `.ok()`s it away).
        if finix::is_endpoint_verification_body(&request.body) {
            return Ok(None);
        }
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::get_finix_webhook_reference(&webhook_body)
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_payment_webhook_response(self, &webhook_body, &request.body)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_refund_webhook_response(&webhook_body, &request.body)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        finix::build_finix_dispute_webhook_response(&webhook_body, &request.body)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        let webhook_body = finix::parse_finix_webhook_body(&request.body)?;
        Ok(Box::new(webhook_body))
    }

    /// A minimal, structurally valid Finix transfer webhook (field-probe input).
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"type":"updated","entity":"transfer","_embedded":{"transfers":[{"id":"TRsample000000000000000","amount":1000,"currency":"USD","state":"SUCCEEDED","tags":{},"type":"DEBIT"}]}}"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Finix<T>
{
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _payment_flow_data: &PaymentFlowData,
    ) -> CustomResult<(), IntegrationError> {
        connector_types::validate_psync_connector_transaction_id(data)
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::AcceptDispute for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SubmitEvidenceV2 for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::DisputeDefend for Finix<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Finix,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Finix<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Finix<T>
{
}

// ===== CONNECTOR INTEGRATION V2 IMPLEMENTATIONS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Finix<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Finix, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Finix,
    generic_type: T,
    api: [
        (
            flow: CreateConnectorCustomer,
            request_body: FinixCreateIdentityRequest,
            response_body: FinixIdentityResponse,
            router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ),
        (
            flow: PaymentMethodToken,
            request_body: FinixCreatePaymentInstrumentRequest,
            response_body: FinixInstrumentResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: Authorize,
            request_body: FinixAuthorizeRequest,
            response_body: FinixAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: FinixThreeDSecureSessionRequest,
            response_body: FinixAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            response_body: FinixPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: FinixPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: FinixCaptureRequest,
            response_body: FinixCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: RSync,
            response_body: FinixRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: FinixVoidRequest,
            response_body: FinixVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: FinixRefundRequest,
            response_body: FinixRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: FinixSetupMandateRequest,
            response_body: FinixSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: FinixRepeatPaymentRequest,
            response_body: FinixRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Accept,
            request_body: FinixAcceptDisputeRequest,
            response_body: FinixAcceptDisputeResponse,
            router_data: RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ),
        (
            flow: SubmitEvidence,
            request_body: FinixSubmitEvidenceRequest,
            response_body: FinixSubmitEvidenceResponse,
            router_data: RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ),
        (
            flow: DefendDispute,
            request_body: FinixDefendDisputeRequest,
            response_body: FinixDefendDisputeResponse,
            router_data: RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        /// Headers every Finix request carries, without a Content-Type (multipart
        /// requests must let the client set their own boundary).
        pub fn build_auth_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![
                (
                    FINIX_REFERRER_SOURCE_HEADER.to_string(),
                    FINIX_REFERRER_SOURCE_VALUE.to_string().into(),
                ),
                (
                    headers::FINIX_VERSION.to_string(),
                    FINIX_VERSION_VALUE.to_string().into(),
                ),
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            headers.append(&mut self.build_auth_headers(req)?);
            Ok(headers)
        }

        /// Error response for the charging flows (Authorize, RepeatPayment). A decline
        /// envelope that names the Transfer / Authorization it created is a terminal
        /// failure of that resource; any other non-2xx leaves the attempt status unset.
        pub fn build_charge_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let attempt_status = res
                .response
                .parse_struct::<finix::FinixErrorResponse>("FinixErrorResponse")
                .ok()
                .and_then(|response| response.get_charge_attempt_status())
                .map(FlowStatus::Payment);
            let error_response = self.build_error_response(res, event_builder, connector_config)?;
            Ok(ErrorResponse {
                attempt_status,
                ..error_response
            })
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.finix.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.finix.base_url
        }

        pub fn connector_base_url_disputes<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, DisputeFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.finix.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCreateIdentityRequest),
    curl_response: FinixIdentityResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/identities", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCreatePaymentInstrumentRequest),
    curl_response: FinixInstrumentResponse,
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
            Ok(format!("{}/payment_instruments", self.connector_base_url_payments(req)))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Finix,
    curl_request: Json(FinixAuthorizeRequest),
    curl_response: FinixAuthorizeResponse,
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

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_charge_error_response(res, event_builder, connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Route based on capture_method: AUTOMATIC -> /transfers, MANUAL -> /authorizations
            let endpoint = match req.request.capture_method {
                Some(common_enums::CaptureMethod::Automatic) |
                Some(common_enums::CaptureMethod::SequentialAutomatic) => "transfers",
                _ => "authorizations"
};
            Ok(format!("{}/{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_response: FinixPSyncResponse,
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
            let connector_transaction_id = req.request.get_connector_transaction_id()?;
            // Route based on ID type using FinixId enum
            let finix_id = transformers::FinixId::from(connector_transaction_id);
            let endpoint = match finix_id {
                transformers::FinixId::Auth(_) => "authorizations",
                transformers::FinixId::Transfer(_) => "transfers"
};
            Ok(format!("{}/{}/{}", self.connector_base_url_payments(req), endpoint, finix_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixVoidRequest),
    curl_response: FinixVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/authorizations/{}", self.connector_base_url_payments(req), connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_response: FinixRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(&self, req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_refund_id = req.request.connector_refund_id.clone();
            Ok(format!("{}/transfers/{}", self.connector_base_url_refunds(req), connector_refund_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixCaptureRequest),
    curl_response: FinixCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Put,
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
            let connector_transaction_id = req.request.get_connector_transaction_id()?;
            Ok(format!("{}/authorizations/{}", self.connector_base_url_payments(req), connector_transaction_id))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixRefundRequest),
    curl_response: FinixRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(&self, req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>) -> CustomResult<String, IntegrationError> {
            let connector_transaction_id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/transfers/{}/reversals", self.connector_base_url_refunds(req), connector_transaction_id))
        }
    }
);

// SetupMandate (SetupRecurring) - stores payment instrument for recurring payments
// Finix requires an identity (connector customer) to create a payment instrument
// The flow creates a payment instrument and returns its ID as the connector_mandate_id
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixSetupMandateRequest),
    curl_response: FinixSetupMandateResponse,
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
            Ok(format!("{}/payment_instruments", self.connector_base_url_payments(req)))
        }
    }
);

// RepeatPayment (MIT) - charge a previously stored Payment Instrument.
// Routes to /transfers for auto-capture and /authorizations for manual capture,
// matching the Authorize flow's behaviour.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Finix,
    curl_request: Json(FinixRepeatPaymentRequest),
    curl_response: FinixRepeatPaymentResponse,
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

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_charge_error_response(res, event_builder, connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let endpoint = match req.request.capture_method {
                Some(common_enums::CaptureMethod::Manual)
                | Some(common_enums::CaptureMethod::ManualMultiple)
                | Some(common_enums::CaptureMethod::Scheduled) => "authorizations",
                _ => "transfers",
            };
            Ok(format!("{}/{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

// Authenticate - create a 3D Secure Session; the response carries the Finix-hosted
// authentication URL the buyer is redirected to.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixThreeDSecureSessionRequest),
    curl_response: FinixAuthenticateResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}/3d_secure_sessions", self.connector_base_url_payments(req)))
        }
    }
);

// PostAuthenticate - retrieve the 3D Secure Session after the buyer returns. The
// redirect's `status` query parameter is browser-supplied, so the result is always
// confirmed server-side. Read-only: this leg never moves money.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_response: FinixPostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let session_id = finix::get_three_d_secure_session_id(&req.request)?;
            Ok(format!("{}/3d_secure_sessions/{}", self.connector_base_url_payments(req), session_id))
        }
    }
);

// Accept - concede the dispute. Finix answers 202 with the Dispute resource
// (response_state ACCEPTED); the final state may move asynchronously.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixAcceptDisputeRequest),
    curl_response: FinixAcceptDisputeResponse,
    flow_name: Accept,
    resource_common_data: DisputeFlowData,
    flow_request: AcceptDisputeData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Accept, DisputeFlowData, AcceptDisputeData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_dispute_id = finix::get_connector_dispute_id(&req.request.connector_dispute_id)?;
            Ok(format!("{}/disputes/{}/accept", self.connector_base_url_disputes(req), connector_dispute_id))
        }
    }
);

// SubmitEvidence - upload one evidence file (multipart field `file`). Finix answers 201
// with the evidence resource in PENDING; validation to SUCCEEDED is asynchronous.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: FormData(FinixSubmitEvidenceRequest),
    curl_response: FinixSubmitEvidenceResponse,
    flow_name: SubmitEvidence,
    resource_common_data: DisputeFlowData,
    flow_request: SubmitEvidenceData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        // No Content-Type here: the HTTP client sets multipart/form-data with its boundary.
        fn get_headers(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_auth_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<SubmitEvidence, DisputeFlowData, SubmitEvidenceData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_dispute_id = finix::get_connector_dispute_id(&req.request.connector_dispute_id)?;
            Ok(format!("{}/disputes/{}/evidence", self.connector_base_url_disputes(req), connector_dispute_id))
        }
    }
);

// DefendDispute - submit the uploaded evidence as a challenge. Finix answers 202 with the
// Dispute resource (response_state RESPONDED), or 422 while any uploaded evidence file is
// not yet SUCCEEDED.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Finix,
    curl_request: Json(FinixDefendDisputeRequest),
    curl_response: FinixDefendDisputeResponse,
    flow_name: DefendDispute,
    resource_common_data: DisputeFlowData,
    flow_request: DisputeDefendData,
    flow_response: DisputeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<DefendDispute, DisputeFlowData, DisputeDefendData, DisputeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let connector_dispute_id = finix::get_connector_dispute_id(&req.request.connector_dispute_id)?;
            Ok(format!("{}/disputes/{}/submit", self.connector_base_url_disputes(req), connector_dispute_id))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Finix,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        GetConnectorCustomer,
        VoidPC,
    ],
    not_supported: [
        VoidPostRefund,
        MandateRevoke,
        ServerAuthenticationToken,
        IncrementalAuthorization,
        CreateOrder,
        PreAuthenticate,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
    ],
);
