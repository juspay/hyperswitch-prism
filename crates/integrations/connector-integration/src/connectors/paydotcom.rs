//! Pay.com (`paydotcom`) — REST/JSON connector for cards, one-time payments.
//!
//! Flows: Authorize, PSync, Capture, Void, Refund, RSync.
//!
//! # Endpoint map
//!
//! | Flow | Method | Path |
//! |---|---|---|
//! | Authorize, auto capture | POST | `/v1/charges` |
//! | Authorize, manual capture | POST | `/v1/holds` |
//! | PSync | GET | `/v1/charges/{id}` or `/v1/holds/{id}` (routed by id prefix) |
//! | Capture | POST | `/v1/holds/{id}/capture` (answers with a **new** `chrg_` id) |
//! | Void | POST | `/v1/holds/{id}/cancel` (no body) |
//! | Refund | POST | `/v1/refunds` |
//! | RSync | GET | `/v1/refunds/{id}` |
//!
//! # 3-D Secure
//!
//! Three shapes are supported, all card-only:
//!
//! * **No 3DS** — one call, `request_threed_secure: "none"`.
//! * **External-MPI 3DS** — one call; the merchant's `eci`/`cavv` are replayed in
//!   `source_data.card.three_ds`.
//! * **Gateway-driven 3DS with a challenge redirect** — Pay.com needs three HTTP calls and
//!   `ConnectorIntegrationV2` issues exactly one per flow execution, so the journey is
//!   split across three flow executions, mirroring the Saferpay precedent:
//!
//!   | # | UCS flow | HTTP call | Result |
//!   |---|---|---|---|
//!   | 1 | `PreAuthenticate` | `POST /v1/charges\|/v1/holds` with `authentication_context` and `request_threed_secure: "challenge"` | `requires_authentication` + the `chrg_`/`hld_` id |
//!   | 2 | `Authorize` | `POST /v1/sessions/authentication/linked` `{resource, return_url, confirm:false}` | the challenge `url` → `RedirectForm` |
//!   | 3 | `Authorize` | `POST /v1/{charges\|holds}/{id}/confirm` | the final status, read synchronously |
//!
//!   The resource id travels between legs on `PaymentFlowData::connector_feature_data`:
//!   the connector publishes it, the caller persists it as the attempt's
//!   `connector_metadata`, and hands it back on the next Authorize. This is exactly the
//!   channel Saferpay uses for its session token
//!   (`saferpay/transformers.rs::settle_token`), and it is read back here by
//!   `transformers::pending_resource_id`.
//!
//!   `confirm: false` is deliberate: with `confirm: true` Pay.com authorizes on its own and
//!   reports only via webhooks, which are out of scope here. `false` keeps the settle on
//!   our side and synchronous.
//!
//!   Each Authorize leg is chosen from the **request alone**
//!   (`transformers::authorize_leg`), never from a previous response, so `get_url` and
//!   `get_request_body` can never disagree about which call is being made.
//!
//! `CreateOrder` is **not** used for leg 1: `PaymentCreateOrderData`
//! (`domain_types/src/connector_types.rs:2107`) carries no `payment_method_data`, and the
//! wire message `PaymentServiceCreateOrderRequest` (`payment.proto:3332`) carries only a
//! `payment_method_type` enum — neither can deliver the PAN that
//! `source_data.card` requires. `PreAuthenticate` can, via
//! `PaymentsPreAuthenticateData::payment_method_data`.
//!
//! A 3DS Authorize that was *not* opened by PreAuthenticate still works when the
//! authentication turns out frictionless: it sends `authentication_context` with
//! `request_threed_secure: "automatic"` and settles in one call. If Pay.com answers
//! `requires_authentication` there, the attempt is reported `AuthenticationPending` and the
//! caller should drive the PreAuthenticate journey instead.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    types::{MinorUnit, StringMinorUnit},
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
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
    decode::BodyDecoding,
};
use serde::Serialize;
use transformers::{
    self as paydotcom, PaydotcomAuthorizeRequest, PaydotcomAuthorizeResponse,
    PaydotcomCaptureRequest, PaydotcomCaptureResponse, PaydotcomPSyncResponse,
    PaydotcomPreAuthenticateRequest, PaydotcomPreAuthenticateResponse, PaydotcomRefundRequest,
    PaydotcomRefundResponse, PaydotcomRefundSyncResponse, PaydotcomVoidResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    /// The single API key credential. `test_…` on sandbox, `live_…` in production.
    pub(crate) const X_PAYCOM_API_KEY: &str = "x-paycom-api-key";
    /// Required on every POST. Pay.com caches the first response for a key — including
    /// 5xx — so it must be freshly generated per HTTP attempt and never derived from the
    /// payment or attempt id.
    pub(crate) const IDEMPOTENCY_KEY: &str = "idempotency-key";
}

/// Auto-capture Authorize: creates a Charge, terminal success `succeeded`.
const PATH_CHARGES: &str = "/v1/charges";
/// Manual-capture Authorize: creates a Hold, terminal success `requires_capture`.
const PATH_HOLDS: &str = "/v1/holds";
/// Refund creation. RSync appends `/{id}`.
const PATH_REFUNDS: &str = "/v1/refunds";
/// Mints the challenge URL for a resource parked on `requires_authentication`.
const PATH_LINKED_AUTH_SESSION: &str = "/v1/sessions/authentication/linked";

// Every amount Pay.com accepts is in the currency's smallest unit, and every request
// amount except `amount_to_capture` / `amount_to_refund` is a JSON integer.
macros::create_all_prerequisites!(
    connector_name: Paydotcom,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: PaydotcomAuthorizeRequest<T>,
            response_body: PaydotcomAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: PaydotcomPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: PaydotcomCaptureRequest,
            response_body: PaydotcomCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            response_body: PaydotcomVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PaydotcomRefundRequest,
            response_body: PaydotcomRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: PaydotcomRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: PaydotcomPreAuthenticateRequest<T>,
            response_body: PaydotcomPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit,
        string_amount_converter: StringMinorUnit
    ],
    member_functions: {
        /// Headers for bodyless `GET` requests. `idempotency-key` is deliberately absent:
        /// Pay.com documents it as having no effect on GET and advises against sending it.
        pub fn build_get_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = paydotcom::PaydotcomAuthType::try_from(&req.connector_config)?;
            Ok(vec![(
                headers::X_PAYCOM_API_KEY.to_string(),
                auth.api_key.into_masked(),
            )])
        }

        /// Headers for `POST` requests: adds `Content-Type` and a fresh idempotency key.
        pub fn build_post_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = self.build_get_headers(req)?;
            header.push((
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            ));
            header.push((
                headers::IDEMPOTENCY_KEY.to_string(),
                uuid::Uuid::new_v4().to_string().into(),
            ));
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.paydotcom.base_url.trim_end_matches('/')
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            req.resource_common_data.connectors.paydotcom.base_url.trim_end_matches('/')
        }

        pub fn build_refund_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: paydotcom::PaydotcomErrorResponse = res
                .response
                .parse_struct("PaydotcomErrorResponse")
                .change_context(crate::utils::response_deserialization_fail(
                    res.status_code,
                    "paydotcom: refund error body did not match the documented \
                     {\"error\":{type,code,message}} envelope.",
                ))?;

            with_error_response_body!(event_builder, response);
            Ok(response.to_refund_error_response(res.status_code))
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paydotcom<T>
{
    fn id(&self) -> &'static str {
        "paydotcom"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.paydotcom.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = paydotcom::PaydotcomAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::X_PAYCOM_API_KEY.to_string(),
            auth.api_key.into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Every 4xx/5xx carries `{"error": {"type", "code", "message", …}}`.
        let response: paydotcom::PaydotcomErrorResponse = res
            .response
            .parse_struct("PaydotcomErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "paydotcom: response body did not match the documented \
                 {\"error\":{type,code,message}} envelope.",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(response.to_error_response(res.status_code))
    }
}

// ===== FLOW-SPECIFIC CONNECTOR INTEGRATION IMPLEMENTATIONS =====

// Authorize — `POST /v1/charges` (auto capture) or `POST /v1/holds` (manual capture).
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paydotcom,
    curl_request: Json(PaydotcomAuthorizeRequest),
    curl_response: PaydotcomAuthorizeResponse,
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
            self.build_post_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Keyed off the same `authorize_leg` helper the request builder uses, so the URL
            // and the body can never describe different calls.
            let path = match paydotcom::authorize_leg(&req.request) {
                paydotcom::PaydotcomAuthorizeLeg::Confirm => {
                    let resource = paydotcom::pending_resource_id(
                        req.request.connector_feature_data.as_ref(),
                    )
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "connector_feature_data.paydotcom_resource",
                        context: Default::default(),
                    })?;
                    paydotcom::confirm_path(&resource)?
                }
                paydotcom::PaydotcomAuthorizeLeg::LinkedSession => {
                    PATH_LINKED_AUTH_SESSION.to_string()
                }
                paydotcom::PaydotcomAuthorizeLeg::Create => {
                    // The body is identical for both endpoints, so the same guard that
                    // rejects ManualMultiple / Scheduled runs here and in the transformer.
                    if paydotcom::is_manual_capture(req.request.capture_method)? {
                        PATH_HOLDS.to_string()
                    } else {
                        PATH_CHARGES.to_string()
                    }
                }
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
        }
    }
);

// PreAuthenticate — gateway-3DS leg 1. Creates the Charge/Hold with
// `authentication_context` and `request_threed_secure: "challenge"`, which parks it on
// `requires_authentication` and yields the id the challenge will authenticate.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paydotcom,
    curl_request: Json(PaydotcomPreAuthenticateRequest),
    curl_response: PaydotcomPreAuthenticateResponse,
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
            self.build_post_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let path = if paydotcom::is_manual_capture(req.request.capture_method)? {
                PATH_HOLDS
            } else {
                PATH_CHARGES
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
        }
    }
);

// PSync — a bodyless `GET`, routed by the `chrg_` / `hld_` prefix of the stored
// connector transaction id.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paydotcom,
    curl_response: PaydotcomPSyncResponse,
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
            self.build_get_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.get_connector_transaction_id()?;
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                paydotcom::payment_resource_path(&transaction_id)?
            ))
        }
    }
);

// Capture — `POST /v1/holds/{id}/capture`. The response is a **Charge with a new id**,
// which the transformer promotes to the attempt's connector transaction id.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paydotcom,
    curl_request: Json(PaydotcomCaptureRequest),
    curl_response: PaydotcomCaptureResponse,
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
            self.build_post_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let transaction_id = req.request.get_connector_transaction_id()?;
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                paydotcom::hold_capture_path(&transaction_id)?
            ))
        }
    }
);

// Void — `POST /v1/holds/{id}/cancel`. The OpenAPI defines no request body for it, so
// none is sent; the idempotency key still rides along because it is a POST.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paydotcom,
    curl_response: PaydotcomVoidResponse,
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
            self.build_post_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                paydotcom::hold_cancel_path(&req.request.connector_transaction_id)?
            ))
        }
    }
);

// Refund — `POST /v1/refunds`. `charge` must be a `chrg_` id; the transformer rejects a
// hold id up front.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paydotcom,
    curl_request: Json(PaydotcomRefundRequest),
    curl_response: PaydotcomRefundResponse,
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
            self.build_post_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_refunds(req),
                PATH_REFUNDS
            ))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_refund_error_response(res, event_builder)
        }
    }
);

// RSync — `GET /v1/refunds/{id}`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Paydotcom,
    curl_response: PaydotcomRefundSyncResponse,
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
            self.build_get_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}/{}",
                self.connector_base_url_refunds(req),
                PATH_REFUNDS,
                req.request.connector_refund_id
            ))
        }

        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
            _connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            self.build_refund_error_response(res, event_builder)
        }
    }
);

// ===== CONNECTOR SERVICE TRAIT IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paydotcom<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Paydotcom<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paydotcom<T>
{
}

// Pay.com does publish webhooks, but they are out of scope for this card-only,
// one-time-payment integration.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paydotcom<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paydotcom<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Paydotcom<T>
{
}

// ===== BODY DECODING IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paydotcom<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: Paydotcom,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Everything outside Authorize / PSync / Capture / Void / Refund / RSync is stubbed:
// wallets, mandates/MIT, tokenization for reuse, disputes, webhooks and payouts are all
// out of scope for this integration.
macros::macro_connector_flow_status_impls!(
    connector: Paydotcom,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        // Pay.com's challenge journey is PreAuthenticate -> Authorize -> Authorize; the
        // middle `Authenticate` flow has no channel for the resource id, so it is unused.
        Authenticate,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        CreateOrder,
        DefendDispute,
        GetConnectorCustomer,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodToken,
        PostAuthenticate,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
        VoidPC,
        VoidPostRefund
    ],
);
