//! Ilixium — Direct API (card, one-time payments).
//!
//! Scope: the **Authorize** flow only, for **Card** payment methods, covering both the
//! no-3DS and 3DS variants. The 3DS variant needs two round trips, and UCS drives both of
//! them through this same `Authorize` flow:
//!
//! * `POST /direct/auth` — the authorisation. On a 3DS-enabled account it answers
//!   `status.code = PENDING` with the ACS redirect data instead of a final result.
//! * `POST /direct/threedcomplete` — the finalisation, sent when UCS re-invokes Authorize
//!   after the ACS posts `md`/`paRes` back to `TermUrl`. Selected by
//!   [`transformers::is_three_ds_completion`], which is also what the request-body builder
//!   uses, so the URL and the body cannot disagree about which leg is being sent.
//!
//! Two things about this API drive the whole design:
//!
//! 1. **Every** business failure — validation rejection, decline, 3DS-required, internal
//!    error — comes back as **HTTP 200** with the outcome in `status.code`. Nothing here
//!    branches on the HTTP status.
//! 2. Authentication is a body-derived digest (`X-MERCHANT-DIGEST`), not a static key, so it
//!    is computed in `build_headers` from the exact serialised body rather than in
//!    `ConnectorCommon::get_auth_header`.
//!
//! Reference: `grace/rulesbook/codegen/references/ilixium/technical_specification.md`

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, request::Method, types::StringMinorUnit,
};
use domain_types::{
    connector_flow::Authorize,
    connector_types::*,
    errors,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{self as ilixium, IlixiumAuthorizeRequest, IlixiumPaymentResponse};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const X_MERCHANT_DIGEST: &str = "X-MERCHANT-DIGEST";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
}

/// The Direct API defaults to `text/xml`, so both `Content-Type` and `Accept` must be sent
/// explicitly for a JSON integration.
const ILIXIUM_JSON_MEDIA_TYPE: &str = "application/json";

const AUTH_ENDPOINT: &str = "/direct/auth";
const THREE_DS_COMPLETE_ENDPOINT: &str = "/direct/threedcomplete";

// =============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Ilixium<T>
{
    fn id(&self) -> &'static str {
        "ilixium"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        ILIXIUM_JSON_MEDIA_TYPE
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.ilixium.base_url.as_ref()
    }

    // NOTE: Ilixium has no static auth header. Every request is authenticated with
    // `X-MERCHANT-DIGEST`, a two-round SHA-512/Base64 digest over the *exact* request body
    // salted with the merchant's Digest Calculation Password — a value this trait method
    // cannot compute, because it never sees the body. The real header is built in
    // `build_headers` below, which every flow's `get_headers` delegates to. Merchant identity
    // travels in the body (`merchant.merchantId` / `merchant.accountId`), not in a header.
    //
    // This impl exists only to satisfy the trait. It still resolves the auth type so a
    // misconfigured merchant account fails loudly here rather than silently sending an
    // unauthenticated request, and it deliberately returns no headers.
    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        ilixium::IlixiumAuthType::try_from(auth_type)?;
        Ok(Vec::new())
    }

    // Reached only for genuine transport/infrastructure failures: Ilixium answers HTTP 200
    // for every documented business outcome, so declines and validation rejections are
    // handled by the Authorize response mapping, not here. A non-2xx body is therefore most
    // likely an intermediary (proxy/WAF/gateway) response, which may not be JSON at all.
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        match res
            .response
            .parse_struct::<IlixiumPaymentResponse>("IlixiumPaymentResponse")
        {
            Ok(response) => {
                with_error_response_body!(event_builder, response);

                let reason_codes = response.reason_codes();
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: reason_codes
                        .first()
                        .cloned()
                        .unwrap_or_else(|| format!("{:?}", response.status.code).to_uppercase()),
                    message: response
                        .status
                        .message
                        .clone()
                        .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
                    reason: if reason_codes.is_empty() {
                        response.status.message.clone()
                    } else {
                        Some(reason_codes.join(", "))
                    },
                    attempt_status: None,
                    connector_transaction_id: response.gateway_ref(),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
            Err(_) => {
                let raw_body = String::from_utf8_lossy(&res.response).to_string();
                tracing::warn!(
                    status_code = res.status_code,
                    body = %raw_body,
                    "Ilixium returned a body that is not a paymentResponse envelope — the \
                     Direct API answers HTTP 200 with a paymentResponse for every documented \
                     outcome, so this is most likely an intermediary proxy/WAF/gateway response"
                );
                Ok(ErrorResponse {
                    status_code: res.status_code,
                    code: common_utils::consts::NO_ERROR_CODE.to_string(),
                    message: common_utils::consts::NO_ERROR_MESSAGE.to_string(),
                    reason: Some(raw_body.chars().take(500).collect()),
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                })
            }
        }
    }
}

// =============================================================================
// BODY DECODING IMPLEMENTATION
// =============================================================================
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Ilixium<T>
{
}

// =============================================================================
// AMOUNT CONVERTER
// =============================================================================
// `transaction.amount` is the minor-unit value, digits only, and the schema types it as a
// JSON string (`^[\d]{1,12}$`).
macros::create_amount_converter_wrapper!(connector_name: Ilixium, amount_type: StringMinorUnit);

// =============================================================================
// PREREQUISITES: struct, flow bridges, shared digest/header helpers
// =============================================================================
macros::create_all_prerequisites!(
    connector_name: Ilixium,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: IlixiumAuthorizeRequest<T>,
            response_body: IlixiumPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        /// Builds the three Ilixium request headers.
        ///
        /// `X-MERCHANT-DIGEST` must be computed over the **exact** bytes that will be
        /// transmitted, so the body is taken from `get_request_body` and digested verbatim.
        /// That is the same value the framework puts on the wire: `RequestContent::
        /// get_body_bytes` serialises through `get_inner_value`, which is what
        /// `content.get_inner_value()` returns here, so the digest can never drift from the
        /// payload.
        pub fn build_headers<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
        {
            let method = self.get_http_method();
            let body = match method {
                // Every in-scope Ilixium endpoint is a POST; the digest covers the body only
                // (no headers, no URL, no timestamp, no nonce), so a bodyless method digests
                // the empty string.
                Method::Get => String::default(),
                Method::Post | Method::Put | Method::Delete | Method::Patch => self
                    .get_request_body(req)?
                    .map(|content| content.get_inner_value().peek().to_owned())
                    .unwrap_or_default(),
            };

            let auth = ilixium::IlixiumAuthType::try_from(&req.connector_config)?;
            let digest = auth.compute_merchant_digest(&body).change_context(
                errors::IntegrationError::RequestEncodingFailed {
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: Some(
                            "https://docs.ilixium.com/docs/direct/digest".to_string(),
                        ),
                        additional_context: Some(
                            "Failed to compute the X-MERCHANT-DIGEST authentication header \
                             for an Ilixium payment request. An incorrect digest is rejected \
                             by the platform with response code 101, 'Invalid Merchant \
                             Credentials'."
                                .to_string(),
                        ),
                    },
                },
            )?;

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    ILIXIUM_JSON_MEDIA_TYPE.to_string().into(),
                ),
                (headers::X_MERCHANT_DIGEST.to_string(), digest.into_masked()),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.ilixium.base_url
        }
    }
);

// =============================================================================
// AUTHORIZE — POST /direct/auth, and POST /direct/threedcomplete on the 3DS return leg
// =============================================================================
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumAuthorizeRequest),
    curl_response: IlixiumPaymentResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            // Same gate as the request-body builder in transformers.rs, so the endpoint and
            // the payload always describe the same leg of the 3DS flow.
            let endpoint = if ilixium::is_three_ds_completion(&req.request) {
                THREE_DS_COMPLETE_ENDPOINT
            } else {
                AUTH_ENDPOINT
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

// =============================================================================
// TRAIT REGISTRATION
// =============================================================================
// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
// Aggregate trait - composes all other connector traits.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Ilixium<T>
{
}

// ===== FLOW MARKER TRAIT IMPLEMENTATIONS =====
// Required by ConnectorServiceTrait's supertrait bounds; not auto-generated by
// create_all_prerequisites! itself.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Ilixium<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
// These are simple marker traits that are NOT flows and therefore have no arm
// in expand_flow_status_impl!. They must be impl'd manually.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Ilixium<T>
{
}

// ===== INCOMING WEBHOOK IMPLEMENTATION =====
// The Ilixium Direct API is fully synchronous for card payments: the outcome of
// /direct/auth (or /direct/threedcomplete for 3DS) is returned inline in the HTTP
// response, and the vendor documents no notification callback for this surface. The
// default no-op implementation is therefore the correct one.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Ilixium<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
// Non-generic marker trait required by VerifyRedirectResponse for webhook
// signature verification.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Ilixium<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
// Emits payout marker-trait impls and default no-op ConnectorIntegrationV2
// impls for all PayoutXxxV2 flows.
crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Ilixium,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// `not_implemented` = Ilixium documents an API for it, this pass just doesn't cover it.
// `not_supported`   = the documented Direct API has no equivalent concept at all.
//
// Authorize is implemented for real above (card, no-3DS and 3DS) and appears in neither
// list.
crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Ilixium,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        // Documented at /docs/api/capture, /docs/api/reversal and /docs/api/refund;
        // deliberately out of scope for this Authorize-only pass.
        Capture,
        Void,
        Refund,
        RSync,
        // The Direct API is synchronous and publishes no status-retrieval endpoint for
        // card payments, but a merchant-ref lookup exists in the wider platform docs.
        PSync,
        // Card tokens are returned on every payment attempt
        // (paymentHistory.paymentAttempt[].token) and can drive stored-card payments;
        // tokenization-as-a-flow and mandates/MIT are out of scope here.
        PaymentMethodToken,
        SetupMandate,
        RepeatPayment
    ],
    not_supported: [
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        // Ilixium has no order/intent object to create before authorising, and no
        // customer-object endpoint: customer.customerId is merchant-supplied and travels
        // inline in the authorisation body.
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
        // 3-D Secure is performed by the platform inside /direct/auth +
        // /direct/threedcomplete, not through standalone authentication endpoints.
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        // The Direct API publishes no dispute/chargeback surface.
        Accept,
        SubmitEvidence,
        DefendDispute
    ],
);
