pub mod transformers;

use std::fmt::Debug;

use base64::{engine::general_purpose, Engine as _};
use common_enums::{AttemptStatus, Currency, CurrencyUnit};
use common_utils::{errors::CustomResult, events, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund, Void, VoidPC,
    },
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentVoidData,
        PaymentWebhookReference, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundWebhookReference, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::WebhookError,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Mask, Maskable, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, AuthenticationStep, RedirectState},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers as authipay;
use transformers::{
    AuthipayAuthorizeResponse, AuthipayCaptureRequest, AuthipayCaptureResponse,
    AuthipayPaymentsRequest, AuthipayPostAuthenticateRequest, AuthipayPostAuthenticateResponse,
    AuthipayPreAuthenticateRequest, AuthipayPreAuthenticateResponse, AuthipayRefundRequest,
    AuthipayRefundResponse, AuthipayRefundSyncResponse, AuthipaySyncResponse,
    AuthipayVoidPCRequest, AuthipayVoidPCResponse, AuthipayVoidRequest, AuthipayVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

/// Constant-time comparison of two byte strings of unknown length. Disperses
/// the lengths into the accumulator so differing lengths also fail in constant
/// time. (ring::constant_time::verify_slices_are_equal is deprecated; the
/// common crypto wrappers only verify a fixed algorithm's signature shape —
/// the webhook path must compare a caller-decoded digest.)
fn constant_time_bytes_eq(left: &[u8], right: &[u8]) -> bool {
    let mut acc: u8 = u8::try_from((left.len() ^ right.len()) % 256).unwrap_or(u8::MAX);
    let max = left.len().max(right.len());
    for i in 0..max {
        let a = left.get(i).copied().unwrap_or(0);
        let b = right.get(i).copied().unwrap_or(0);
        acc |= a ^ b;
    }
    acc == 0
}

/// Resolve an ISO numeric currency code (e.g. "840") to a `Currency`, for
/// Connect-side notifications that carry the numeric code instead of the
/// alpha code. Returns None when no mapping exists — callers fail closed.
fn currency_from_numeric_code(code: &str) -> Option<Currency> {
    Some(match code {
        "840" => Currency::USD,
        "978" => Currency::EUR,
        "826" => Currency::GBP,
        "356" => Currency::INR,
        "036" => Currency::AUD,
        "124" => Currency::CAD,
        "392" => Currency::JPY,
        "756" => Currency::CHF,
        "946" => Currency::RON,
        "348" => Currency::HUF,
        "203" => Currency::CZK,
        "985" => Currency::PLN,
        "975" => Currency::BGN,
        "191" => Currency::HRK,
        "208" => Currency::DKK,
        "578" => Currency::NOK,
        "752" => Currency::SEK,
        "784" => Currency::AED,
        "702" => Currency::SGD,
        "764" => Currency::THB,
        "458" => Currency::MYR,
        "360" => Currency::IDR,
        "608" => Currency::PHP,
        "704" => Currency::VND,
        "410" => Currency::KRW,
        "156" => Currency::CNY,
        "344" => Currency::HKD,
        "901" => Currency::TWD,
        "554" => Currency::NZD,
        "710" => Currency::ZAR,
        "986" => Currency::BRL,
        "484" => Currency::MXN,
        "032" => Currency::ARS,
        "152" => Currency::CLP,
        "170" => Currency::COP,
        "604" => Currency::PEN,
        "858" => Currency::UYU,
        "376" => Currency::ILS,
        "949" => Currency::TRY,
        "682" => Currency::SAR,
        "400" => Currency::JOD,
        "414" => Currency::KWD,
        "512" => Currency::OMR,
        "048" => Currency::BHD,
        "634" => Currency::QAR,
        "818" => Currency::EGP,
        "504" => Currency::MAD,
        "566" => Currency::NGN,
        "404" => Currency::KES,
        _ => return None,
    })
}

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const API_KEY: &str = "Api-Key";
    pub(crate) const CLIENT_REQUEST_ID: &str = "Client-Request-Id";
    pub(crate) const AUTH_TOKEN_TYPE: &str = "Auth-Token-Type";
    pub(crate) const TIMESTAMP: &str = "Timestamp";
    pub(crate) const MESSAGE_SIGNATURE: &str = "Message-Signature";
}

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATIONS =====
// Main service trait - aggregates all other traits

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Authipay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Authipay<T>
{
}

// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Authipay<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Authipay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== REFUND FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Authipay<T>
{
}

// ===== ADVANCED FLOW TRAIT IMPLEMENTATIONS =====
// ===== AUTHENTICATION FLOW TRAIT IMPLEMENTATIONS =====
// ===== DISPUTE FLOW TRAIT IMPLEMENTATIONS =====
// ===== WEBHOOK TRAIT IMPLEMENTATIONS =====
// Spec `### IncomingWebhook`: one OMS server-to-server notification shape
// (form-urlencoded, `txntype` distinguishes payments from refunds), verified by
// an in-body `notification_hash` HMAC (algorithm carried by the notification
// itself) over `chargetotal|currency|txndatetime|storename|approval_code`
// keyed by the store's shared secret. Spec "verifier source": the shared
// secret lives in the connector auth block (creds `api_secret` per the spec's
// stated source field), resolved from connector account details — the same
// secret that signs outbound API requests (the harness delivers no
// webhook_secrets for authipay; missing or unresolvable secrets fail closed
// per G-IncomingWebhook-01 / TH-10).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Authipay<T>
{
    /// TH-10 / G-IncomingWebhook-01 fail-closed: when source verification
    /// returned `false` (or errored upstream), `HandleEvent` must surface the
    /// rejection as an error instead of processing the event. The shared
    /// `webhook_utils::process_webhook_event` is deliberately permissive —
    /// it surfaces `source_verified` and lets each connector decide (some
    /// connectors intentionally accept unverified events). For authipay the
    /// invalid-signature outcome reaches the gRPC caller as
    /// `WebhookSourceVerificationFailed` (unauthenticated); no event content
    /// is produced and no 2xx ack is returned.
    fn on_source_not_verified(
        &self,
        _request: &RequestDetails,
    ) -> Result<(), error_stack::Report<WebhookError>> {
        Err(
            error_stack::report!(WebhookError::WebhookSourceVerificationFailed).attach_printable(
                "authipay webhook: notification_hash mismatch — failing closed, \
             no event content produced and no ack returned",
            ),
        )
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // G-IncomingWebhook-01: secret must be resolvable or verification fails
        // closed. The named-webhook-secret slot wins when the caller supplied
        // one; otherwise the store shared secret comes from the connector auth
        // block (spec verifier source: creds api_secret).
        let secret: Vec<u8> = match connector_webhook_secret {
            Some(secrets) => secrets.secret,
            None => {
                let account_details = connector_account_details.ok_or_else(|| {
                    error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
                        .attach_printable(
                            "authipay webhook: no webhook secret supplied and no connector \
                             account details from which to resolve the store shared secret",
                        )
                })?;
                let auth = authipay::AuthipayAuthType::try_from(&account_details)
                    .change_context(WebhookError::WebhookVerificationSecretInvalid)
                    .attach_printable(
                        "authipay webhook: connector auth block did not carry api_secret, \
                         the store shared secret for notification_hash verification",
                    )?;
                auth.api_secret.expose().into_bytes()
            }
        };
        if secret.is_empty() {
            return Ok(false);
        }

        let body: authipay::AuthipayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .map_err(|err| {
                error_stack::report!(WebhookError::WebhookSourceVerificationFailed)
                    .attach_printable(format!(
                        "authipay webhook: notification body is not decodable form data: {err}"
                    ))
            })?;

        // G-IncomingWebhook-02: the algorithm comes from the notification, not
        // from config; an unresolvable or unsupported algorithm fails closed.
        let message = body.hash_preimage();
        let Some(computed) = body
            .hash_algorithm
            .sign(secret.as_slice(), message.as_bytes())
        else {
            return Ok(false);
        };

        let delivered = body.notification_hash.clone().expose();
        // UD-04: decode the delivered hash as Base64 when the store is
        // configured that way; else compare the string form directly (hex or
        // any string form the store emits). Fail closed on encodings that
        // decode to a digest whose length cannot match the declared algorithm.
        let delivered_bytes = match general_purpose::STANDARD.decode(delivered.trim()) {
            Ok(decoded) if decoded.len() == body.hash_algorithm.digest_len() => decoded,
            _ => match hex::decode(delivered.trim()) {
                Ok(decoded) if decoded.len() == body.hash_algorithm.digest_len() => decoded,
                _ => return Ok(false),
            },
        };

        // G-IncomingWebhook-03: constant-time compare of equal-length byte
        // strings without short-circuiting on the first difference.
        Ok(constant_time_bytes_eq(&computed, &delivered_bytes))
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        let body: authipay::AuthipayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .map_err(|err| {
                error_stack::report!(WebhookError::WebhookEventTypeNotFound).attach_printable(
                    format!(
                        "authipay webhook: notification body is not decodable form data: {err}"
                    ),
                )
            })?;

        if body.is_refund_notification() {
            return Ok(match body.status {
                authipay::AuthipayWebhookStatus::Approved => EventType::RefundSuccess,
                authipay::AuthipayWebhookStatus::Declined
                | authipay::AuthipayWebhookStatus::Failed => EventType::RefundFailure,
                authipay::AuthipayWebhookStatus::Waiting => EventType::RefundProcessing,
                authipay::AuthipayWebhookStatus::Unknown => {
                    EventType::IncomingWebhookEventUnspecified
                }
            });
        }

        // TH-07: non-terminal default — WAITING and unrecognised statuses map
        // to processing, never a terminal state.
        Ok(match body.status {
            authipay::AuthipayWebhookStatus::Approved => EventType::PaymentIntentSuccess,
            authipay::AuthipayWebhookStatus::Declined | authipay::AuthipayWebhookStatus::Failed => {
                EventType::PaymentIntentFailure
            }
            authipay::AuthipayWebhookStatus::Waiting | authipay::AuthipayWebhookStatus::Unknown => {
                EventType::PaymentIntentProcessing
            }
        })
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        let body: authipay::AuthipayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .map_err(|err| {
                error_stack::report!(WebhookError::WebhookEventTypeNotFound).attach_printable(
                    format!(
                        "authipay webhook: notification body is not decodable form data: {err}"
                    ),
                )
            })?;

        if body.is_refund_notification() {
            return Ok(Some(WebhookResourceReference::Refund(
                RefundWebhookReference {
                    connector_refund_id: body.ipg_transaction_id.clone(),
                    merchant_refund_id: body.refnumber.clone(),
                    connector_transaction_id: body.ipg_transaction_id.clone(),
                    merchant_transaction_id: body.merchant_reference(),
                },
            )));
        }

        Ok(Some(WebhookResourceReference::Payment(
            PaymentWebhookReference {
                connector_transaction_id: body.ipg_transaction_id.clone(),
                merchant_transaction_id: body.merchant_reference(),
            },
        )))
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body: authipay::AuthipayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .map_err(|err| {
                error_stack::report!(WebhookError::WebhookBodyDecodingFailed).attach_printable(
                    format!(
                        "authipay webhook: notification body is not decodable form data: {err}"
                    ),
                )
            })?;

        // Decode-only amounts (plan §4 amount: FloatMajorUnitForConnector;
        // the wire string is preserved verbatim — it already went through the
        // hash check — and merely converted).
        let currency = body.currency.parse::<Currency>().or_else(|_| {
            // Connect-side notifications carry ISO numeric codes ("840" = USD).
            currency_from_numeric_code(&body.currency)
                .ok_or_else(|| error_stack::report!(WebhookError::WebhookBodyDecodingFailed))
                .attach_printable(
                    "authipay webhook: unrecognised currency code in notification".to_string(),
                )
        })?;
        let minor_amount_captured =
            authipay::authipay_webhook_minor_amount(&body.charge_total, currency).ok();

        let error_message = body.fail_reason.clone();
        Ok(WebhookDetailsResponse {
            resource_id: body
                .ipg_transaction_id
                .clone()
                .map(ResponseId::ConnectorTransactionId),
            status: match body.status {
                authipay::AuthipayWebhookStatus::Approved => AttemptStatus::Charged,
                authipay::AuthipayWebhookStatus::Declined
                | authipay::AuthipayWebhookStatus::Failed => AttemptStatus::Failure,
                authipay::AuthipayWebhookStatus::Waiting
                | authipay::AuthipayWebhookStatus::Unknown => AttemptStatus::Pending,
            },
            connector_response_reference_id: body.tdate.clone(),
            connector_request_reference_id: body.merchant_reference(),
            mandate_reference: None,
            error_code: body.processor_response_code.clone(),
            error_message,
            error_reason: body.fail_reason.clone(),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let body: authipay::AuthipayWebhookBody = serde_urlencoded::from_bytes(&request.body)
            .map_err(|err| {
                error_stack::report!(WebhookError::WebhookBodyDecodingFailed).attach_printable(
                    format!(
                        "authipay webhook: notification body is not decodable form data: {err}"
                    ),
                )
            })?;

        Ok(RefundWebhookDetailsResponse {
            connector_refund_id: body.ipg_transaction_id.clone(),
            merchant_transaction_id: body.merchant_reference(),
            status: match body.status {
                authipay::AuthipayWebhookStatus::Approved => common_enums::RefundStatus::Success,
                authipay::AuthipayWebhookStatus::Declined
                | authipay::AuthipayWebhookStatus::Failed => common_enums::RefundStatus::Failure,
                authipay::AuthipayWebhookStatus::Waiting
                | authipay::AuthipayWebhookStatus::Unknown => common_enums::RefundStatus::Pending,
            },
            connector_response_reference_id: body.refnumber.clone().or_else(|| body.tdate.clone()),
            error_code: body.processor_response_code.clone(),
            error_message: body.fail_reason.clone(),
            raw_connector_response: Some(String::from_utf8_lossy(&request.body).to_string()),
            status_code: 200,
            response_headers: None,
        })
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Authipay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Authipay<T>
{
}

// ===== VALIDATION TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Authipay<T>
{
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: RedirectState,
        completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        // G-ThreeDS__Card-01/02: only 3DS card payments enter the authentication trio.
        // legs_used = [PreAuthenticate, PostAuthenticate] — AuthenticationStep::Authenticate
        // is never returned (the payer-auth challenge round trip is browser-side, and the
        // PATCH validate call is the PostAuthenticate leg).
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
                // Frictionless: PreAuthenticate completed without a redirect → charge directly.
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authorize
                }
                // Challenge: the ACS posted the CRes back → validate it (INV-25: guard this
                // arm before the PostAuthenticate-complete arm so the return journey starts
                // at PostAuthenticate and never re-runs it).
                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::PostAuthenticate),
                ) => AuthenticationStep::Authorize,
                (RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams, _) => {
                    AuthenticationStep::PostAuthenticate
                }
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

// ===== CONNECTOR CUSTOMER TRAIT IMPLEMENTATIONS =====
// ===== MACRO PREREQUISITES =====
// Define connector struct and bridge types for all flows
macros::create_all_prerequisites!(
    connector_name: Authipay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: AuthipayPaymentsRequest<T>,
            response_body: AuthipayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: AuthipaySyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: AuthipayVoidRequest,
            response_body: AuthipayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: AuthipayCaptureRequest,
            response_body: AuthipayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: AuthipayRefundRequest,
            response_body: AuthipayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: AuthipayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: AuthipayVoidPCRequest,
            response_body: AuthipayVoidPCResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: AuthipayPreAuthenticateRequest<T>,
            response_body: AuthipayPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: AuthipayPostAuthenticateRequest,
            response_body: AuthipayPostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        /// Build headers with HMAC-SHA256 signature
        /// This is a helper function used by all flows that need request body signing
        fn build_headers_with_signature(
            &self,
            auth: &authipay::AuthipayAuthType,
            request_body_str: &str,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Generate client request ID and timestamp
            let client_request_id = authipay::AuthipayAuthType::generate_client_request_id();
            let timestamp = authipay::AuthipayAuthType::generate_timestamp();

            // Generate HMAC signature
            let api_key_value = auth.api_key.clone().expose();
            let message_signature = auth.generate_hmac_signature(
                &api_key_value,
                &client_request_id,
                &timestamp,
                request_body_str,
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::API_KEY.to_string(),
                    Secret::new(api_key_value).into_masked(),
                ),
                (
                    headers::CLIENT_REQUEST_ID.to_string(),
                    client_request_id.into(),
                ),
                (
                    headers::AUTH_TOKEN_TYPE.to_string(),
                    "HMAC".to_string().into(),
                ),
                (headers::TIMESTAMP.to_string(), timestamp.into()),
                (
                    headers::MESSAGE_SIGNATURE.to_string(),
                    message_signature.into(),
                ),
            ])
        }

        /// Build headers for GET requests (no request body)
        fn build_headers_for_get(
            &self,
            auth: &authipay::AuthipayAuthType,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // For GET requests, use empty body for signature generation
            self.build_headers_with_signature(auth, "")
        }

        /// Helper to get base URL for payment flows
        fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.authipay.base_url
        }

        /// Helper to get base URL for refund flows
        fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.authipay.base_url
        }

        /// Build common headers for all flows
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            // This will be overridden by each flow's custom get_headers implementation
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )])
        }
    }
);

// ===== MAIN CONNECTOR INTEGRATION IMPLEMENTATIONS =====

// Authorize flow - Payment authorization with HMAC signature
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayPaymentsRequest<T>),
    curl_response: AuthipayAuthorizeResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayPaymentsRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// PSync flow - Payment status retrieval (GET request, no body)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_response: AuthipaySyncResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // For GET requests, use empty body for HMAC signature
            self.build_headers_for_get(&auth)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract transaction ID from connector_transaction_id
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;

            let base_url = self.connector_base_url_payments(req);
            // Append transaction ID to base URL for GET request
            Ok(format!("{base_url}/{transaction_id}"))
        }
    }
);

// Void flow - Cancel/void a payment authorization
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayVoidRequest),
    curl_response: AuthipayVoidResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayVoidRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract transaction ID from connector_transaction_id
            let transaction_id = &req.request.connector_transaction_id;
            let base_url = self.connector_base_url_payments(req);
            // Secondary transaction pattern: {base_url}/{transaction_id}
            Ok(format!("{base_url}/{transaction_id}"))
        }
    }
);

// VoidPC flow - Reverse/VoidPostCapture: cancel a captured (PostAuth) payment before settlement
// Uses requestType: VoidTransaction (distinct from Void which uses VoidPreAuthTransactions)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayVoidPCRequest),
    curl_response: AuthipayVoidPCResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayVoidPCRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = &req.request.connector_transaction_id;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/{transaction_id}"))
        }
    }
);

// Capture flow - Capture an authorized payment
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayCaptureRequest),
    curl_response: AuthipayCaptureResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayCaptureRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract transaction ID from connector_transaction_id
            let transaction_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID { context: Default::default() })?;

            let base_url = self.connector_base_url_payments(req);
            // Secondary transaction pattern: {base_url}/{transaction_id}
            Ok(format!("{base_url}/{transaction_id}"))
        }
    }
);

// Refund flow - Process a refund for a payment
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayRefundRequest),
    curl_response: AuthipayRefundResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayRefundRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract transaction ID from connector_transaction_id
            // This is the ipgTransactionId from the original payment transaction
            let transaction_id = req.request.connector_transaction_id.clone();
            let base_url = self.connector_base_url_refunds(req);
            // Secondary transaction pattern: {base_url}/{transaction_id}
            Ok(format!("{base_url}/{transaction_id}"))
        }
    }
);

// RSync flow - Refund status retrieval (GET request, no body)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_response: AuthipayRefundSyncResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // For GET requests, use empty body for HMAC signature
            self.build_headers_for_get(&auth)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Extract refund transaction ID from connector_refund_id
            // This is the ipgTransactionId from the refund transaction response
            let refund_id = req.request.connector_refund_id.clone();
            let base_url = self.connector_base_url_refunds(req);
            // GET request to retrieve refund transaction state
            Ok(format!("{base_url}/{refund_id}"))
        }
    }
);

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
// Pre Authentication — 3DS payer-auth initiate (POST /payments, PayerAuth transaction)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayPreAuthenticateRequest<T>),
    curl_response: AuthipayPreAuthenticateResponse,
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
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayPreAuthenticateRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same POST /payments endpoint as Authorize; the payer-auth wire is
            // distinguished by requestType=PaymentCardPayerAuthTransaction in the body.
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Authentication — not implemented: the 2-leg payer-auth layout uses the browser ACS round
// trip plus the PostAuthenticate PATCH instead of a server-side Authenticate call.

// Post Authentication — 3DS payer-auth validate (PATCH /payments/{payer-auth-id})
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Authipay,
    curl_request: Json(AuthipayPostAuthenticateRequest),
    curl_response: AuthipayPostAuthenticateResponse,
    flow_name: PostAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPostAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    http_method: Patch,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = authipay::AuthipayAuthType::try_from(&req.connector_config)
                .change_context(IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

            // Build the request to get the body for HMAC signature
            let connector_req = AuthipayPostAuthenticateRequest::try_from(req)?;
            let request_body_str = serde_json::to_string(&connector_req)
                .change_context(IntegrationError::RequestEncodingFailed { context: Default::default() })?;

            // Generate headers with HMAC signature
            self.build_headers_with_signature(
                &auth,
                &request_body_str,
            )
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // PATCH /payments/{payer-auth-ipgTransactionId}: the id of the PreAuthenticate
            // payer-auth transaction to validate. It arrives as connector_order_reference_id
            // (from an Authenticate response on full-trio paths) or as the
            // connector_feature_data the caller persisted after PreAuthenticate. INV-23:
            // method+URL are distinct from Authorize's POST /payments.
            let payer_auth_id = req
                .request
                .connector_order_reference_id
                .clone()
                .or_else(|| {
                    req.resource_common_data
                        .connector_feature_data
                        .as_ref()
                        .and_then(|feature_data| {
                            feature_data.clone().expose().as_str().map(|id| id.to_string())
                        })
                })
                .ok_or(error_stack::report!(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_order_reference_id",
                        context: domain_types::errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Pass the payer-auth ipgTransactionId returned by PreAuthenticate \
                                 as connector_order_reference_id or connector_feature_data so the \
                                 PATCH validate call can address it.".to_string(),
                            ),
                            doc_url: None,
                            additional_context: None,
                        },
                    }
                ))?;

            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/{payer_auth_id}"))
        }
    }
);

// ===== CONNECTOR CUSTOMER CONNECTOR INTEGRATIONS =====
// Create Connector Customer

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Authipay<T>
{
    fn id(&self) -> &'static str {
        "authipay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.authipay.base_url
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Authentication is handled in get_headers for Authipay
        // because we need the request body to generate the HMAC signature
        Ok(vec![])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: authipay::AuthipayErrorResponse = if res.response.is_empty() {
            authipay::AuthipayErrorResponse::default()
        } else {
            // Tolerates both documented error shapes: the standard {code, message,
            // details} object and the Apigee {"errors": [...]} envelope whose first
            // entry maps into code/message (spec shape-(a) recommendation).
            authipay::AuthipayErrorResponse::from_bytes(&res.response).ok_or_else(|| {
                crate::utils::response_deserialization_fail(
                    res.status_code,
                    "authipay: response body did not match the expected format; confirm API version and connector documentation.",
                )
            })?
        };

        with_error_response_body!(event_builder, response);
        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.unwrap_or_default(),
            message: response.message.unwrap_or_default(),
            reason: response.api_trace_id,
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

macros::macro_connector_flow_status_impls!(
    connector: Authipay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        SetupMandate,
        RepeatPayment,
        ServerSessionAuthenticationToken,
        PaymentMethodToken,
        ServerAuthenticationToken,
        Authenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        CreateOrder,
        ClientAuthenticationToken,
        Accept,
        DefendDispute,
        SubmitEvidence,
        CreateConnectorCustomer,
        GetConnectorCustomer,
    ],
);
