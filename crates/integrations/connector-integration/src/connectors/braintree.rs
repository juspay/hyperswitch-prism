pub mod transformers;
use std::fmt::Debug;

use base64::Engine;
use common_enums::{CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    crypto::{self, SignMessage},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{StringMajorUnit, StringMinorUnit},
    ParsingError,
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, ClientAuthenticationToken, MandateRevoke, PSync,
        PaymentMethodToken, PostAuthenticate, PreAuthenticate, RSync, Refund, RepeatPayment,
        SetupMandate, Void, VoidPC,
    },
    connector_types::{
        ClientAuthenticationTokenRequestData, ConnectorWebhookSecrets,
        DisputeWebhookDetailsResponse, EventType, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsPreAuthenticateData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundWebhookDetailsResponse,
        RefundsData, RefundsResponseData, RepeatPaymentData, RequestDetails,
        SetupMandateRequestData, WebhookDetailsResponse, WebhookResourceReference,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::Report;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as braintree, BraintreeAuthResponse, BraintreeAuthenticateRequest,
    BraintreeAuthenticateResponse, BraintreeCancelRequest, BraintreeCancelResponse,
    BraintreeCaptureRequest, BraintreeCaptureResponse, BraintreeClientTokenRequest,
    BraintreePSyncRequest, BraintreePSyncResponse, BraintreePaymentsRequest,
    BraintreePaymentsResponse, BraintreePostAuthenticateRequest, BraintreePostAuthenticateResponse,
    BraintreePreAuthenticateRequest, BraintreePreAuthenticateResponse, BraintreeRSyncRequest,
    BraintreeRSyncResponse, BraintreeRefundRequest, BraintreeRefundResponse,
    BraintreeRepeatPaymentRequest, BraintreeRepeatPaymentResponse, BraintreeRevokeMandateRequest,
    BraintreeRevokeMandateResponse, BraintreeSessionResponse, BraintreeSetupMandateRequest,
    BraintreeSetupMandateResponse, BraintreeTokenRequest, BraintreeTokenResponse,
    BraintreeVoidPCRequest, BraintreeVoidPCResponse,
};

use super::macros;
use crate::{finalize_connector_response, types::ResponseRouterData, with_error_response_body};
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;
use domain_types::errors::WebhookError;
use error_stack::ResultExt;
pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

pub const BRAINTREE_VERSION: &str = "Braintree-Version";
pub const BRAINTREE_VERSION_VALUE: &str = "2019-01-01";

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Braintree<T>
{
    fn id(&self) -> &'static str {
        "braintree"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.braintree.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = braintree::BraintreeAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let auth_key = format!("{}:{}", auth.public_key.peek(), auth.private_key.peek());
        let auth_header = format!("Basic {}", BASE64_ENGINE.encode(auth_key));
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth_header.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: Result<braintree::ErrorResponses, Report<ParsingError>> =
            res.response.parse_struct("Braintree Error Response");

        match response {
            Ok(braintree::ErrorResponses::BraintreeApiErrorResponse(response)) => {
                with_error_response_body!(event_builder, response);

                let typed = macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                let error_object = response.api_error_response.errors;
                let error = error_object.errors.first().or(error_object
                    .transaction
                    .as_ref()
                    .and_then(|transaction_error| {
                        transaction_error.errors.first().or(transaction_error
                            .credit_card
                            .as_ref()
                            .and_then(|credit_card_error| credit_card_error.errors.first()))
                    }));
                let (code, message) = error.map_or(
                    (NO_ERROR_CODE.to_string(), NO_ERROR_MESSAGE.to_string()),
                    |error| (error.code.clone(), error.message.clone()),
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code,
                    message,
                    reason: Some(response.api_error_response.message),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Ok(braintree::ErrorResponses::BraintreeErrorResponse(response)) => {
                with_error_response_body!(event_builder, response);
                let typed = macros::serialize_typed_connector_payload(
                    &response,
                    "typed_connector_response",
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: NO_ERROR_CODE.to_string(),
                    message: NO_ERROR_MESSAGE.to_string(),
                    reason: Some(response.errors),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: typed,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
            Err(_) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({"error": "Error response parsing failed", "status_code": res.status_code}));
                }
                domain_types::utils::handle_json_response_deserialization_failure(res, "braintree")
            }
        }
    }
}

//marker traits
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ClientAuthentication for Braintree<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Braintree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidPostCaptureV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for Braintree<T>
{
}
/// Braintree revokes a stored credential by deleting the vaulted payment method
/// (`deletePaymentMethodFromVault`). Without this impl the flow stays in the `not_implemented`
/// bucket and a merchant migrating from Hyperswitch loses the ability to revoke a mandate it can
/// still charge.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::MandateRevokeV2 for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Braintree<T>
{
    /// Braintree charges a `paymentMethodId`; none of its transaction mutations accepts card
    /// credentials inline. So every instrument whose credentials prism can actually see has to be
    /// exchanged for one first.
    ///
    /// * **Card** — `tokenizeCreditCard`.
    /// * **Wallet carrying a DECRYPTED network token** (Apple Pay with cleartext DPAN +
    ///   cryptogram) — `tokenizeNetworkToken`. Gated on `is_wallet_decrypted_network_token` and
    ///   not on the wallet alone: an `ApplePayThirdPartySdk` / `GooglePayThirdPartySdk` payload is
    ///   already a Braintree-minted nonce charged through `chargePaymentMethod`, and tokenizing it
    ///   would be a round trip that can only fail. An ENCRYPTED Apple Pay blob is excluded by the
    ///   same flag — prism holds no decryption certificate for it.
    fn should_do_payment_method_token(
        &self,
        payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
        is_wallet_decrypted_network_token: bool,
    ) -> bool {
        match payment_method {
            PaymentMethod::Card => true,
            PaymentMethod::Wallet => is_wallet_decrypted_network_token,
            _ => false,
        }
    }

    /// Drives the composite authorize loop through Braintree-HOSTED 3D Secure for card + ThreeDs:
    /// PreAuthenticate (`tokenizeCreditCard` + `createClientToken`) -> Authenticate
    /// (`performThreeDSecureLookup`) -> [PostAuthenticate (`node(id:)` readback) once the
    /// cardholder returns from the ACS challenge] -> Authorize.
    ///
    /// Without this override the trait default returns `AuthenticationStep::Authorize` and all
    /// three legs are unreachable from `CompositePaymentService/Authorize`.
    ///
    /// Two arms are load-bearing and neither is arbitrary:
    ///
    /// * `(InitialRequest, Some(PreAuthenticate)) => Authenticate`. PreAuthenticate emits no
    ///   redirect — Braintree's lookup is genuinely server-side and is satisfied by
    ///   `transactionInformation.browserInformation`, so there is nothing to send the browser away
    ///   for. The composite loop's PreAuthenticate arm breaks only on a redirect or a failure, so
    ///   it falls through to here and the lookup runs in the same call.
    /// * `(RedirectWithParams | RedirectWithoutParams, None) => PostAuthenticate`. The ACS posts
    ///   its PaRes to BRAINTREE's own `termUrl`, not to UCS, so the cardholder returns to the
    ///   merchant carrying nothing. Both return states are handled because the caller — not this
    ///   connector — decides whether it puts anything in `redirection_response.params`.
    ///
    /// Termination: PreAuthenticate advances to Authenticate; Authenticate either breaks the loop
    /// on a challenge redirect / a terminal status or advances to Authorize; PostAuthenticate never
    /// breaks on its own, so the `Some(PostAuthenticate)` arms resolve it to Authorize. Every other
    /// pair falls to the catch-all.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authenticate
                }
                // Frictionless: the lookup resolved without an ACS challenge.
                (RedirectState::InitialRequest, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }
                // `Some(..)` arms precede the catch-all redirect arm below: an earlier `_` in the
                // completed_step position would shadow them and the loop would re-run
                // PostAuthenticate forever, since that arm never breaks.
                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::PostAuthenticate),
                ) => AuthenticationStep::Authorize,
                (
                    RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams,
                    None,
                ) => AuthenticationStep::PostAuthenticate,
                _ => AuthenticationStep::Authorize,
            }
        } else {
            // Load-bearing: an unset proto `auth_type` resolves to `NoThreeDs`, so the gate must be
            // an explicit equality test and everything else must charge directly.
            AuthenticationStep::Authorize
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Braintree<T>
{
    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, Report<WebhookError>> {
        let connector_webhook_secrets = connector_webhook_secret.ok_or_else(|| {
            tracing::warn!(
                target: "braintree_webhook",
                "no webhook secret configured for Braintree source verification"
            );
            error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
        })?;

        let notif = braintree::get_webhook_object_from_body(&request.body)
            .inspect_err(|error| {
                tracing::warn!(
                    target: "braintree_webhook",
                    ?error,
                    "failed to decode the Braintree webhook body for source verification"
                );
            })
            .change_context(WebhookError::WebhookSourceVerificationFailed)?;

        // `bt_signature` is `pubkey1|sig1&pubkey2|sig2&...`; split into (public_key, signature) pairs.
        let signature_pairs: Vec<(&str, &str)> = notif
            .bt_signature
            .split('&')
            .map(|pair| pair.split_once('|').unwrap_or(("", "")))
            .collect();

        // `additional_secret` holds the merchant's Braintree public key.
        let public_key = connector_webhook_secrets
            .additional_secret
            .as_ref()
            .ok_or_else(|| {
                tracing::warn!(
                    target: "braintree_webhook",
                    "missing Braintree public key (additional_secret) for source verification"
                );
                error_stack::report!(WebhookError::WebhookVerificationSecretNotFound)
            })?;

        let extracted_signature =
            braintree::get_matching_webhook_signature(&signature_pairs, public_key.peek())
                .ok_or_else(|| {
                    tracing::warn!(
                        target: "braintree_webhook",
                        "no bt_signature entry matched the merchant Braintree public key"
                    );
                    error_stack::report!(WebhookError::WebhookSignatureNotFound)
                })?;

        // Signing key is the SHA1 digest of the private key (`secret`), then HMAC-SHA1 over
        // the payload. Braintree's scheme is HMAC-SHA1 keyed by the *SHA-1 digest of the
        // private key*, not by the private key itself; collapsing the two steps into a plain
        // HMAC silently fails every verification.
        let sha1_hash_key = ring::digest::digest(
            &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
            &connector_webhook_secrets.secret,
        );

        // The message is the RAW, newline-inclusive `bt_payload` — not the newline-stripped
        // form `decode_from_request` builds for base64 decoding. Keep the two asymmetric.
        //
        // Braintree's own SDK accepts a match against either the payload as received or the
        // payload with one trailing "\n" appended: Ruby's `Base64.encode64` always terminates
        // its output with a newline and signs that, but form parsers, proxies and replay
        // harnesses routinely trim it before the body reaches the merchant. Normalising to
        // either one alone rejects half of real traffic, so try both.
        let candidates = [notif.bt_payload.clone(), format!("{}\n", notif.bt_payload)];

        for candidate in candidates {
            let signed_message = crypto::HmacSha1
                .sign_message(sha1_hash_key.as_ref(), candidate.as_bytes())
                .inspect_err(|error| {
                    tracing::warn!(
                        target: "braintree_webhook",
                        ?error,
                        "failed to compute the HMAC-SHA1 signature over the Braintree bt_payload"
                    );
                })
                .change_context(WebhookError::WebhookSourceVerificationFailed)?;

            // Constant-time comparison: a byte-by-byte early return leaks the position of the
            // first mismatch to anyone who can time the endpoint.
            #[allow(deprecated)] // ring 0.17 renamed the module; the function is still sound
            if ring::constant_time::verify_slices_are_equal(
                hex::encode(signed_message).as_bytes(),
                extracted_signature.as_bytes(),
            )
            .is_ok()
            {
                return Ok(true);
            }
        }

        tracing::warn!(
            target: "braintree_webhook",
            "Braintree webhook signature did not match the computed HMAC-SHA1 digest"
        );
        Ok(false)
    }

    fn get_event_type(&self, request: RequestDetails) -> Result<EventType, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        Ok(braintree::get_status(notif.kind.as_str()))
    }

    fn get_webhook_event_reference(
        &self,
        request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::get_webhook_reference(&notif)
    }

    /// Reached for the two settlement kinds and — via the misc-event fall-through in
    /// `webhook_utils::process_webhook_event`, which routes anything that is not a payment,
    /// refund, dispute or payout event to this handler — for `check` and for every kind UCS
    /// does not model. It must therefore tolerate a subject it cannot read and report no
    /// status change rather than erroring or inventing one.
    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::build_webhook_payment_response(&notif)
    }

    fn process_refund_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::build_webhook_refund_response(&notif)
    }

    fn process_dispute_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        braintree::build_webhook_dispute_response(&notif, &request.body)
    }

    fn get_webhook_resource_object(
        &self,
        request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, Report<WebhookError>> {
        let notif = braintree::decode_from_request(&request)?;
        Ok(Box::new(notif))
    }

    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<connector_types::IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<interfaces::api::EventAckResponse, Report<WebhookError>> {
        Ok(interfaces::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: Some(b"[accepted]".to_vec()),
        })
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        // form-urlencoded `bt_signature=<pubkey>|<sig>&bt_payload=<base64 dispute_opened XML>`.
        //
        // This is a PARSE probe, not a verification probe, so `bt_signature` is a dummy pair.
        // The XML is shaped from Braintree's own SDK sample generator, not from these structs:
        // it carries the `<subject>` wrapper, kebab-case element names, a `type=` attribute, a
        // `nil="true"` element, a lowercase `chargeback` stage and a decimal major-unit amount.
        // A fixture derived from the parser under test cannot falsify the parser.
        br#"bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B"#
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Braintree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for Braintree<T>
{
}
macros::create_all_prerequisites!(
    connector_name: Braintree,
    generic_type: T,
    api: [
        (
            flow: PaymentMethodToken,
            request_body: BraintreeTokenRequest<T>,
            response_body: BraintreeTokenResponse,
            router_data: RouterDataV2<PaymentMethodToken, PaymentFlowData, PaymentMethodTokenizationData<T>, PaymentMethodTokenResponse>,
        ),
        (
            flow: PSync,
            request_body: BraintreePSyncRequest,
            response_body: BraintreePSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: BraintreeCaptureRequest,
            response_body: BraintreeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: BraintreeCancelRequest,
            response_body: BraintreeCancelResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: VoidPC,
            request_body: BraintreeVoidPCRequest,
            response_body: BraintreeVoidPCResponse,
            router_data: RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>,
        ),
        (
            flow: ClientAuthenticationToken,
            request_body: BraintreeClientTokenRequest,
            response_body: BraintreeSessionResponse,
            router_data: RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: BraintreeRefundRequest,
            response_body: BraintreeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: BraintreeRSyncRequest,
            response_body: BraintreeRSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: BraintreeRepeatPaymentRequest,
            response_body: BraintreeRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: BraintreeSetupMandateRequest,
            response_body: BraintreeSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: MandateRevoke,
            request_body: BraintreeRevokeMandateRequest,
            response_body: BraintreeRevokeMandateResponse,
            router_data: RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: BraintreePreAuthenticateRequest<T>,
            response_body: BraintreePreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            request_body: BraintreeAuthenticateRequest,
            response_body: BraintreeAuthenticateResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            request_body: BraintreePostAuthenticateRequest,
            response_body: BraintreePostAuthenticateResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit,
        amount_converter_webhooks: StringMinorUnit
        ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut header = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    BRAINTREE_VERSION.to_string(),
                    BRAINTREE_VERSION_VALUE.to_string().into(),
                ),
            ];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }

        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.braintree.base_url
        }
    }
);

// Manual implementation for Authorize with conditional response body
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    > for Braintree<T>
{
    fn get_content_type(&self) -> &'static str {
        self.common_get_content_type()
    }

    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        self.build_headers(req)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::ConnectorRequestData>, IntegrationError> {
        let connector_router_data = BraintreeRouterData {
            connector: self.to_owned(),
            router_data: req.to_owned(),
        };
        let connector_req: BraintreePaymentsRequest =
            BraintreePaymentsRequest::try_from(connector_router_data)?;
        let typed = events::MaskedSerdeValue::from_masked_optional(
            &connector_req,
            "typed_connector_request",
        );
        Ok(Some(common_utils::request::ConnectorRequestData::new(
            common_utils::request::RequestContent::Json(Box::new(connector_req)),
            typed,
        )))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ConnectorError,
    > {
        match data.request.is_auto_capture() {
            true => {
                let response: BraintreePaymentsResponse = res
                    .response
                    .parse_struct("Braintree PaymentsResponse")
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "braintree: response body did not match the expected format; confirm API version and connector documentation."),
                    )?;
                finalize_connector_response!(event_builder, response, data, res.status_code)
            }
            false => {
                let response: BraintreeAuthResponse = res
                    .response
                    .parse_struct("Braintree AuthResponse")
                    .change_context(
                        crate::utils::response_deserialization_fail(
                            res.status_code,
                        "braintree: response body did not match the expected format; confirm API version and connector documentation."),
                    )?;
                finalize_connector_response!(event_builder, response, data, res.status_code)
            }
        }
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, _connector_config)
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRepeatPaymentRequest),
    curl_response: BraintreeRepeatPaymentResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreePSyncRequest),
    curl_response: BraintreePSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
        Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeCaptureRequest),
    curl_response: BraintreeCaptureResponse,
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
         Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeCancelRequest),
    curl_response: BraintreeCancelResponse,
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
             Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeVoidPCRequest),
    curl_response: BraintreeVoidPCResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeClientTokenRequest),
    curl_response: BraintreeSessionResponse,
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
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<ClientAuthenticationToken, MerchantAuthenticationFlowData, ClientAuthenticationTokenRequestData , PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(self.connector_base_url_merchant_auth(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeTokenRequest),
    curl_response: BraintreeTokenResponse,
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
             Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// SetupMandate (SetupRecurring) - a real zero-amount card verification.
// `vaultCreditCard` verifies the card against the network and vaults it in one
// mutation; the attempt status is mapped from `verification.status`, the AVS/CVV
// verdicts land on `connector_response`, and the vaulted `paymentMethod.id` is
// returned as `connector_mandate_id`, which RepeatPayment consumes via its
// existing MandatePayment request path.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeSetupMandateRequest),
    curl_response: BraintreeSetupMandateResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// `deletePaymentMethodFromVault` — the revoke. `http_method: Post` like every other leg: Braintree
// is a single GraphQL endpoint and every operation, mutation or query, is POSTed to it.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRevokeMandateRequest),
    curl_response: BraintreeRevokeMandateResponse,
    flow_name: MandateRevoke,
    resource_common_data: PaymentFlowData,
    flow_request: MandateRevokeRequestData,
    flow_response: MandateRevokeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<MandateRevoke, PaymentFlowData, MandateRevokeRequestData, MandateRevokeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRefundRequest),
    curl_response: BraintreeRefundResponse,
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
          Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeRSyncRequest),
    curl_response: BraintreeRSyncResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
             Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// Braintree-hosted 3D Secure, leg 1 of 3. Every leg posts to the same GraphQL endpoint with the
// same headers as every other Braintree flow — there is no per-flow path and no version override.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreePreAuthenticateRequest<T>),
    curl_response: BraintreePreAuthenticateResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Leg 2 of 3 — the server-side 3D Secure lookup.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreeAuthenticateRequest),
    curl_response: BraintreeAuthenticateResponse,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Leg 3 of 3 — the `node(id:)` readback. `http_method: Post` despite being a GraphQL *query*:
// GraphQL over HTTP posts queries and mutations alike to the same endpoint.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Braintree,
    curl_request: Json(BraintreePostAuthenticateRequest),
    curl_response: BraintreePostAuthenticateResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// ConnectorIntegrationV2 implementations for authentication flows

macros::macro_connector_flow_status_impls!(
    connector: Braintree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        IncrementalAuthorization,
        CreateOrder,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        SubmitEvidence,
        DefendDispute,
        Accept,
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
