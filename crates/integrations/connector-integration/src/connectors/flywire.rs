mod transformers;
use super::macros;

use domain_types::{
    connector_flow::{Authenticate, Authorize, PSync, RSync, Refund},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsResponseData, PaymentsSyncData,
        RedirectDetailsResponse, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RequestDetails, WebhookDetailsResponse,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};

use std::fmt::Debug;

use common_enums::{AttemptStatus, CurrencyUnit, RefundStatus};
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};

use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, WebhookError};
use error_stack::{report, ResultExt};
use hyperswitch_masking::{Maskable, PeekInterface};
use interfaces::connector_types::{AuthenticationStep, RedirectState};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};
use serde::Serialize;
use transformers as flywire;
use transformers::{
    FlywireCheckoutSessionRequest, FlywireCheckoutSessionResponse, FlywireConfirmRequest,
    FlywireConfirmResponse, FlywirePayment as FlywirePSyncResponse,
    FlywireRefundRequest, FlywireRefundResponse,
    FlywireRefundResponse as FlywireRSyncResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_AUTHENTICATION_KEY: &str = "X-Authentication-Key";
}

macros::macro_connector_payout_implementation!(
    connector: Flywire,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Flywire<T>
{
    fn verify_redirect_response_source(
        &self,
        _request: &RequestDetails,
        _secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, IntegrationError> {
        Ok(false)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, IntegrationError> {
        Ok(RedirectDetailsResponse {
            resource_id: None,
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Flywire<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Flywire<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Flywire<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        // Spec: https://developers.flywire.com/education/Content/notifications-from-flywire.htm
        // Header is `X-Flywire-Digest`, value is Base64(HMAC-SHA256(raw_body, shared_secret)).
        let signature_header = request
            .headers
            .get("x-flywire-digest")
            .or_else(|| request.headers.get("X-Flywire-Digest"))
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("Missing X-Flywire-Digest header on incoming Flywire webhook")?;

        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(signature_header.trim())
            .attach_printable("Failed to base64-decode X-Flywire-Digest header")
            .change_context(WebhookError::WebhookSourceVerificationFailed)
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(request.body.clone())
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        let connector_webhook_secrets = match connector_webhook_secret {
            Some(secrets) => secrets,
            None => {
                let auth = flywire::FlywireAuthType::try_from(
                    connector_account_details
                        .as_ref()
                        .ok_or_else(|| report!(WebhookError::WebhookVerificationSecretNotFound))?,
                )
                .map_err(|e| e.change_context(WebhookError::WebhookSourceVerificationFailed))?;

                // No shared secret configured — merchant has not set up webhook verification.
                // Return source_verified=false rather than blindly trusting the webhook.
                let Some(shared_secret) = auth.shared_secret else {
                    return Ok(false);
                };

                ConnectorWebhookSecrets {
                    secret: shared_secret.peek().as_bytes().to_vec(),
                    additional_secret: None,
                }
            }
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &connector_webhook_secrets)?;
        let message =
            self.get_webhook_source_verification_message(&request, &connector_webhook_secrets)?;

        crypto::HmacSha256
            .verify_signature(&connector_webhook_secrets.secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("Webhook source verification failed for Flywire")
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"event_type":"guaranteed","event_date":"2026-01-01T00:00:00Z","event_resource":"payments","data":{"payment_id":"PTU000000000","status":"guaranteed","external_reference":"sample"}}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let notif: flywire::FlywireWebhookBody = request
            .body
            .parse_struct("FlywireWebhookBody")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        Ok(flywire::webhook_event_type(&notif))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let notif: flywire::FlywireWebhookBody = request
            .body
            .parse_struct("FlywireWebhookBody")
            .attach_printable("Failed to parse Flywire webhook body")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response = WebhookDetailsResponse::try_from(&notif)
            .change_context(WebhookError::WebhookResponseEncodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let notif: flywire::FlywireWebhookBody = request
            .body
            .parse_struct("FlywireWebhookBody")
            .attach_printable("Failed to parse Flywire webhook body")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;
        let response = RefundWebhookDetailsResponse::try_from(&notif)
            .change_context(WebhookError::WebhookResponseEncodingFailed);

        response.map(|mut response| {
            response.raw_connector_response =
                Some(String::from_utf8_lossy(&request.body).to_string());
            response
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Flywire<T>
{
    /// Flywire's flow: Authenticate (POST /checkout/sessions → iframe) on the
    /// initial request; Authorize (POST /checkout/sessions/{id}/confirm) after
    /// the customer completes payment and the caller redirects back.
    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: common_enums::PaymentMethod,
        redirect_state: RedirectState,
        _completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        match redirect_state {
            RedirectState::InitialRequest => AuthenticationStep::Authenticate,
            RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams => {
                AuthenticationStep::Authorize
            }
        }
    }

    fn requires_verify_redirect_response(&self) -> bool {
        true
    }

    fn requires_authorize_post_redirect(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Flywire<T>
{
    fn id(&self) -> &'static str {
        "flywire"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = flywire::FlywireAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![(
            headers::X_AUTHENTICATION_KEY.to_string(),
            auth.api_key.peek().to_string().into(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.flywire.base_url
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: flywire::FlywireErrorResponse = res
            .response
            .parse_struct("FlywireErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "flywire: response body did not match the expected RFC 7807 problem+json shape",
            ))?;

        with_error_response_body!(event_builder, response);

        let title = response
            .title
            .clone()
            .unwrap_or_else(|| "Error".to_string());
        let detail = response.detail.clone().unwrap_or_default();
        let first_detail_msg = response
            .errors
            .first()
            .and_then(|e| e.message.clone())
            .unwrap_or_default();
        let combined_msg = if !first_detail_msg.is_empty() {
            format!("{title}: {detail} ({first_detail_msg})")
        } else if !detail.is_empty() {
            format!("{title}: {detail}")
        } else {
            title.clone()
        };

        // Classify whether this error is a known terminal refund failure.
        // FLYWIRE refund endpoints return RFC 7807 problem+json with an inner
        // `errors[*].type` tag. Each of these means the refund will never
        // succeed; we surface that as FlowStatus::Refund(Failure) so the
        // framework marks the refund row FAILED rather than leaving it
        // transient/PENDING.
        //
        // Reason codes per FLYWIRE refund docs:
        // https://developers.flywire.com/education/Content/resource_refunds.htm
        //   not_found                      — parent payment missing
        //   already_refund                 — one-active-refund rule violated
        //   status                         — payment in non-refundable state
        //   amount_over_remaining_balance  — over-refund
        //   minimum_threshold              — below recipient's configured floor
        //   currency                       — currency mismatch
        //   multiple                       — payment already fully refunded
        // Plus the wire-observed `invalid_status` (used in 422 responses for
        // the same conditions; not formally listed but actually emitted).
        let is_refund_failure = response.errors.iter().any(|e| {
            matches!(
                e.detail_type.as_deref(),
                Some("invalid_status")
                    | Some("not_found")
                    | Some("already_refund")
                    | Some("status")
                    | Some("amount_over_remaining_balance")
                    | Some("minimum_threshold")
                    | Some("currency")
                    | Some("multiple")
            )
        });

        let attempt_status: FlowStatus = if is_refund_failure {
            FlowStatus::Refund(RefundStatus::Failure)
        } else {
            let payment_status = match response.status {
                Some(401) | Some(403) => AttemptStatus::AuthenticationFailed,
                Some(404) | Some(422) => AttemptStatus::Failure,
                _ => AttemptStatus::Pending,
            };
            FlowStatus::Payment(payment_status)
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            message: combined_msg.clone(),
            reason: Some(combined_msg),
            attempt_status: Some(attempt_status),
            connector_transaction_id: None,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
        })
    }
}

macros::create_amount_converter_wrapper!(connector_name: Flywire, amount_type: MinorUnit);

macros::create_all_prerequisites!(
    connector_name: Flywire,
    generic_type: T,
    api: [
        (
            flow: Authenticate,
            request_body: FlywireCheckoutSessionRequest,
            response_body: FlywireCheckoutSessionResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authorize,
            request_body: FlywireConfirmRequest,
            response_body: FlywireConfirmResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: FlywirePSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: FlywireRefundRequest,
            response_body: FlywireRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: FlywireRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.flywire.base_url.to_string()
        }

        pub fn connector_base_url_refund<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.flywire.base_url.to_string()
        }

        fn psync_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url(req);
            let payment_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            Ok(format!("{base_url}/payments/v1/payments/{payment_id}"))
        }
    }
);

// Authenticate — creates the hosted-checkout session on Flywire.
// POST {base}/payments/v1/checkout/sessions
//
// Returns the session UUID as `connector_transaction_id` and the iframe HTML
// as `redirection_data`. The caller renders the iframe; after the customer pays
// they are redirected back and a second CompositeAuthorize call (with
// connector_order_id = session_id) triggers the Authorize (confirm) step.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Flywire,
    curl_request: Json(FlywireCheckoutSessionRequest),
    curl_response: FlywireCheckoutSessionResponse,
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
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}/payments/v1/checkout/sessions"))
        }
    }
);

// Authorize — confirms the checkout session and returns the Flywire payment_id.
// POST {base}/payments/v1/checkout/sessions/{session_id}/confirm  (empty body)
//
// The session_id is read from `resource_common_data.connector_order_id`,
// which was set by the prior CreateOrder call. Flywire's /confirm endpoint
// is one-shot — subsequent calls return 404.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Flywire,
    curl_request: Json(FlywireConfirmRequest),
    curl_response: FlywireConfirmResponse,
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
            let base_url = self.connector_base_url(req);
            let session_id = req
                .resource_common_data
                .connector_order_id
                .clone()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_order_id (Flywire session id; call CreateOrder first)",
                    context: Default::default(),
                })?;
            Ok(format!(
                "{base_url}/payments/v1/checkout/sessions/{session_id}/confirm"
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Flywire,
    curl_response: FlywirePSyncResponse,
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
            self.psync_url(req)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Flywire,
    curl_request: Json(FlywireRefundRequest),
    curl_response: FlywireRefundResponse,
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
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let payment_id = req.request.connector_transaction_id.clone();
            let base_url = self.connector_base_url_refund(req);
            Ok(format!("{base_url}/payments/v1/payments/{payment_id}/refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Flywire,
    curl_response: FlywireRSyncResponse,
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
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut api_key);
            Ok(headers)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // RSync hits FLYWIRE's per-refund GET endpoint (docs:
            // https://developers.flywire.com/education/Content/resource_refunds.htm).
            let refund_id = req.request.connector_refund_id.clone();
            let base_url = self.connector_base_url_refund(req);
            Ok(format!("{base_url}/payments/v1/refunds/{refund_id}"))
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Flywire,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Capture,
        Void,
        IncrementalAuthorization,
        ServerAuthenticationToken,
        ClientAuthenticationToken,
        PreAuthenticate,
        PostAuthenticate,
        DefendDispute,
        Accept,
        SubmitEvidence,
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
    ],
    not_supported: [
        VoidPC,
        PaymentMethodToken,
        CreateOrder,
        VoidPostRefund,
        PaymentMethodEligibility,
    ],
);
