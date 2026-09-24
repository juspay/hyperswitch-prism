//! Payhound — hosted crypto invoice gateway (Malta, MFSA / MiCAR licensed).
//!
//! # Flow set
//!
//! Payhound's invoices resource exposes exactly three operations — create, show and list — so this
//! integration implements **Authorize** (`POST /api/v1/invoices`), **PSync**
//! (`GET /api/v1/invoices/{id}`) and **incoming webhooks**, and nothing else. Refund, RSync,
//! Capture and Void are declared `not_supported` rather than `not_implemented`: the Payhound API
//! has no such endpoints at all. A merchant refunds a crypto invoice out-of-band from their
//! Payhound balance, which is a dashboard/payout operation, not an API call against the invoice.
//!
//! # Amounts
//!
//! Payhound's `price` is a **decimal major-unit string** (`"266.45"`), so the connector uses the
//! `StringMajorUnit` converter and reports `CurrencyUnit::Base`. Minor units would be read as a
//! wildly different amount.
//!
//! # Authentication
//!
//! Every request carries `X-MB-Key`, `X-MB-Nonce` and `X-MB-Signature`, where
//! `signature = hex(HMAC_SHA512(api_secret, uri_path ++ nonce ++ hex(SHA256(request_data))))`.
//! `request_data` is the exact JSON body bytes for POST and the URL-encoded query string (empty
//! when there is none) for GET, so the signature is always computed over the bytes that actually go
//! on the wire.
//!
//! The nonce must be unique *and* strictly increasing per API key, forever. It is produced by a
//! process-global monotonic counter (see `transformers::next_nonce`). That counter is
//! **per-process**: a horizontally scaled deployment sharing a single Payhound API key can still
//! emit a non-increasing nonce across pods, and Payhound offers no server-side mitigation — use one
//! API key per deployment. A `400 {"message":"Invalid nonce"}` surfaces as an ordinary connector
//! error and is deliberately not retried, because retrying would re-send a payment intent.
//!
//! # Webhooks
//!
//! Payhound signs callbacks with **the same `api_secret`** used for requests (there is no separate
//! webhook secret), over `X-MB-Callback-Id ++ hex(SHA256(raw_body))` — no path, no nonce. A
//! merchant who configures `connector_webhook_secret` explicitly must set it to that same Payhound
//! API secret.
//!
//! Payhound POSTs a callback on **any** change to **any** invoice attribute, not only on status
//! transitions, and retries 12 times over roughly 22 days until it receives a `2xx`. Webhook
//! handling must therefore be idempotent: deduplicate on `(id, status)` so a repeat callback for an
//! already-charged invoice is not double-counted.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    crypto::{self, VerifySignature},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::Method,
};
use domain_types::{
    connector_flow::{Authorize, PSync},
    connector_types::{
        ConnectorWebhookSecrets, EventContext, EventType, PaymentFlowData, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RequestDetails, WebhookDetailsResponse,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
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
    self as payhound, PayhoundInvoiceRequest, PayhoundInvoiceResponse,
    PayhoundInvoiceResponse as PayhoundInvoiceSyncResponse, PAYHOUND_CONNECTOR,
    PAYHOUND_CONTENT_TYPE, PAYHOUND_HEADER_CALLBACK_ID_LOWER, PAYHOUND_HEADER_KEY,
    PAYHOUND_HEADER_NONCE, PAYHOUND_HEADER_SIGNATURE, PAYHOUND_HEADER_SIGNATURE_LOWER,
    PAYHOUND_INVOICES_PATH,
};
use url::Url;

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    /// Standard `Content-Type` request header.
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    /// Standard `Accept` request header. Payhound requires it to name its vendor media type.
    pub(crate) const ACCEPT: &str = "Accept";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Payhound<T>
{
    fn id(&self) -> &'static str {
        PAYHOUND_CONNECTOR
    }

    /// Payhound prices invoices in the major unit of the settlement currency (`"266.45"`).
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        PAYHOUND_CONTENT_TYPE
    }

    /// Returns only the credential-derived header.
    ///
    /// `X-MB-Nonce` and `X-MB-Signature` cannot be produced here: the signature is computed over
    /// the request path and the exact body bytes, neither of which this trait method receives, and
    /// minting a nonce that were then discarded would waste the key's strictly-increasing nonce
    /// space. The complete header set is assembled in `build_headers`, which does have the request.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = payhound::PayhoundAuthType::try_from(auth_type)?;
        Ok(vec![(
            PAYHOUND_HEADER_KEY.to_string(),
            auth.api_key.peek().to_owned().into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.payhound.base_url.as_ref()
    }

    /// Payhound signals every failure with a real HTTP status code and the envelope
    /// `{"message": "<text>"}`. It never returns a 200 carrying an error payload, so there is
    /// deliberately no in-band error branch here.
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: payhound::PayhoundErrorResponse = res
            .response
            .parse_struct("PayhoundErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "payhound: error body did not match the documented `{\"message\": \"...\"}` \
                 envelope; confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            // Payhound returns no machine-readable error code; the HTTP status is the only stable
            // discriminator it offers.
            code: payhound::payhound_error_code(res.status_code),
            message: response.message.clone(),
            reason: Some(response.message),
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
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Payhound<T>
{
}

macros::macro_connector_payout_implementation!(
    connector: Payhound,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Payhound<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Payhound<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Payhound<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Payhound<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Payhound<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Payhound<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Payhound, amount_type: StringMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Payhound,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PayhoundInvoiceRequest,
            response_body: PayhoundInvoiceResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PayhoundInvoiceSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        /// Builds the complete Payhound header set, signing exactly the bytes that will be sent.
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let url = self.get_url(req)?;
            // The signed `uri_path` is taken from the parsed request URL rather than by subtracting
            // the configured base from it: Payhound documents its sandbox base *with* a trailing
            // slash, and string subtraction would then yield `api/v1/invoices` with no leading
            // slash and every request would fail with `403 "Invalid signature"`.
            let parsed_url = Url::parse(&url).change_context(IntegrationError::UrlParsingFailed {
                context: payhound::payhound_context(
                    "payhound: the request URL built from the configured base_url is not a valid \
                     absolute URL, so its path could not be signed",
                ),
            })?;
            let uri_path = parsed_url.path().to_owned();

            let request_data = match self.get_http_method() {
                Method::Get => match parsed_url.query() {
                    Some(query) => query.to_owned(),
                    // Payhound signs the URL-encoded query string, and an absent query is signed as
                    // the empty string. This is the documented contract, not a swallowed error:
                    // PSync is path-based and never carries a query.
                    None => String::new(),
                },
                Method::Post | Method::Put | Method::Delete | Method::Patch => self
                    .get_request_body(req)?
                    .map(|content| content.content.get_inner_value().peek().to_owned())
                    // Deliberately not `unwrap_or_default()`: signing the empty string while
                    // sending a real body yields an undiagnosable `403 "Invalid signature"`.
                    .ok_or_else(|| {
                        report!(IntegrationError::RequestEncodingFailed {
                            context: payhound::payhound_context(
                                "payhound: request body was absent while signing a body-carrying \
                                 request, so the signature could not cover the transmitted bytes",
                            ),
                        })
                    })?,
            };

            let auth = payhound::PayhoundAuthType::try_from(&req.connector_config)?;
            let nonce = payhound::next_nonce()?;
            let signature = payhound::payhound_request_signature(
                &auth.api_secret,
                &uri_path,
                nonce,
                &request_data,
            )?;

            Ok(vec![
                (
                    PAYHOUND_HEADER_KEY.to_string(),
                    auth.api_key.peek().to_owned().into_masked(),
                ),
                (
                    PAYHOUND_HEADER_NONCE.to_string(),
                    nonce.to_string().into_masked(),
                ),
                (
                    PAYHOUND_HEADER_SIGNATURE.to_string(),
                    signature.into_masked(),
                ),
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    PAYHOUND_CONTENT_TYPE.to_string().into(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.payhound.base_url
        }

        /// Joins the configured base URL with a Payhound path constant.
        ///
        /// The base is documented with a trailing slash, so it is trimmed here; the path constant
        /// always carries its own leading slash and is the same value that gets signed.
        pub fn build_payhound_url(base_url: &str, path: &str) -> String {
            format!("{}{}", base_url.trim_end_matches('/'), path)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payhound,
    curl_request: Json(PayhoundInvoiceRequest),
    curl_response: PayhoundInvoiceResponse,
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
            Ok(Self::build_payhound_url(
                self.connector_base_url_payments(req),
                PAYHOUND_INVOICES_PATH,
            ))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Payhound,
    curl_response: PayhoundInvoiceSyncResponse,
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
            // Payhound has no lookup-by-reference endpoint, so the invoice must be identified by
            // the connector transaction id Authorize returned. Falling back to
            // `connector_request_reference_id` would sync the wrong resource — or nothing at all.
            let invoice_id = req
                .request
                .connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: payhound::payhound_context(
                        "payhound: PSync requires the Payhound invoice id returned by Authorize; \
                         Payhound exposes no lookup-by-reference endpoint",
                    ),
                })?;

            Ok(format!(
                "{}/{invoice_id}",
                Self::build_payhound_url(
                    self.connector_base_url_payments(req),
                    PAYHOUND_INVOICES_PATH,
                )
            ))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Payhound<T>
{
    fn get_webhook_source_verification_signature(
        &self,
        request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        // The docs mark `X-MB-Signature` optional on callbacks. An unsigned callback is treated as
        // a verification failure, not as "no verification needed" — the docs themselves warn that
        // forging a callback is otherwise trivial.
        let signature = request
            .headers
            .get(PAYHOUND_HEADER_SIGNATURE_LOWER)
            .ok_or_else(|| report!(WebhookError::WebhookSignatureNotFound))
            .attach_printable("payhound: incoming callback carried no X-MB-Signature header")?;
        hex::decode(signature)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("payhound: X-MB-Signature is not valid lowercase hex")
    }

    fn get_webhook_source_verification_message(
        &self,
        request: &RequestDetails,
        _connector_webhook_secrets: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        let callback_id = request
            .headers
            .get(PAYHOUND_HEADER_CALLBACK_ID_LOWER)
            .ok_or_else(|| report!(WebhookError::WebhookSourceVerificationFailed))
            .attach_printable(
                "payhound: incoming callback carried no X-MB-Callback-Id header, which is part of \
                 the signing string",
            )?;

        // The raw bytes as received are signed — deserializing and re-serializing would reorder
        // keys or change whitespace and break the digest.
        payhound::payhound_callback_message(callback_id, &request.body)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("payhound: failed to build the callback verification message")
    }

    fn verify_webhook_source(
        &self,
        request: RequestDetails,
        connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        // Payhound signs callbacks with the API secret that created the resource, so the connector
        // credentials are the correct, zero-configuration source. A merchant-configured webhook
        // secret is honoured as a fallback and must be set to that same Payhound API secret.
        let secret: Vec<u8> =
            match (&connector_account_details, &connector_webhook_secret) {
                (Some(ConnectorSpecificConfig::Payhound { api_secret, .. }), _) => {
                    api_secret.peek().as_bytes().to_vec()
                }
                (_, Some(secrets)) => secrets.secret.clone(),
                (_, None) => return Err(report!(WebhookError::WebhookVerificationSecretNotFound))
                    .attach_printable(
                    "payhound: neither Payhound connector credentials nor a configured webhook \
                         secret were available to verify the callback signature",
                ),
            };

        // Both helpers derive everything from the request itself, so the secret bundle handed to
        // them exists only to satisfy the trait signature.
        let webhook_secrets = ConnectorWebhookSecrets {
            secret: secret.clone(),
            additional_secret: None,
        };

        let signature =
            self.get_webhook_source_verification_signature(&request, &webhook_secrets)?;
        let message = self.get_webhook_source_verification_message(&request, &webhook_secrets)?;

        crypto::HmacSha512
            .verify_signature(&secret, &signature, &message)
            .change_context(WebhookError::WebhookSourceVerificationFailed)
            .attach_printable("payhound: callback signature verification failed")
    }

    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"378d8ec6e305f469b009cb4e2deedf93","status":"completed","address":"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi","merchant_currency":"EUR","merchant_amount":"266.45","invoice_currency":"BTC","invoice_amount":"0.88613839","paid_currency":"BTC","paid_amount":"0.88613839","reference":"probe_ref","invoice_url":"/invoices/378d8ec6e305f469b009cb4e2deedf93","create_time":1398871897.0,"valid_until_time":1398872497.0}"#
    }

    fn get_event_type(
        &self,
        request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        // Payhound sends no event-type field: the callback body *is* the invoice object, so the
        // event is derived from its status.
        let invoice: PayhoundInvoiceResponse = request
            .body
            .parse_struct("PayhoundInvoiceResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        match invoice.status {
            payhound::PayhoundInvoiceStatus::Completed
            | payhound::PayhoundInvoiceStatus::Overpaid => Ok(EventType::PaymentIntentSuccess),
            payhound::PayhoundInvoiceStatus::Pending
            | payhound::PayhoundInvoiceStatus::Underpaid => Ok(EventType::PaymentActionRequired),
            payhound::PayhoundInvoiceStatus::Aborted | payhound::PayhoundInvoiceStatus::Timeout => {
                Ok(EventType::PaymentIntentFailure)
            }
            payhound::PayhoundInvoiceStatus::Unknown => {
                Ok(EventType::IncomingWebhookEventUnspecified)
            }
        }
    }

    fn process_payment_webhook(
        &self,
        request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        let invoice: PayhoundInvoiceResponse = request
            .body
            .parse_struct("PayhoundInvoiceResponse")
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        let mut response = payhound::payhound_webhook_details(&invoice)
            .change_context(WebhookError::WebhookBodyDecodingFailed)?;

        // Keeps an unrecognised status diagnosable: `#[serde(other)]` discards the literal value,
        // so the raw body is the only place it survives.
        response.raw_connector_response = Some(String::from_utf8_lossy(&request.body).to_string());
        Ok(response)
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Payhound,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_supported: [
        Refund,
        RSync,
        Capture,
        Void,
        VoidPC,
        VoidPostRefund,
        SetupMandate,
        RepeatPayment,
        MandateRevoke,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateOrder,
        PaymentMethodToken,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        ClientAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
    ],
);
