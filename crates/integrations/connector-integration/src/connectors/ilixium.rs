//! Ilixium — Direct API (card, one-time payments).
//!
//! Scope: the **PreAuthenticate**, **Authorize**, **Capture**, **Void**, **PSync**, **Refund**
//! and **RSync** flows, for **Card** payment methods.
//!
//! Ilixium has exactly **one** authorisation endpoint, and it performs 3-D Secure inside it
//! rather than through standalone authentication endpoints. A card payment is therefore one or
//! two round trips depending on whether the issuer challenges:
//!
//! * `POST /direct/auth` — the authorisation. Runs as **PreAuthenticate** when the payment is
//!   3DS (see the flow block below) and as **Authorize** when it is not. On a 3DS-enabled
//!   account it *may* answer `status.code = PENDING` with the ACS redirect data instead of a
//!   final result — but it may equally answer `SUCCESS`, because Ilixium decides at response
//!   time whether a challenge is needed. A not-enrolled or frictionless card is charged by this
//!   single call and has no second leg.
//! * `POST /direct/threedcomplete` — the finalisation, sent as **Authorize** when UCS re-invokes
//!   the flow after the ACS posts `md`/`paRes` back to `TermUrl`. Selected by
//!   [`transformers::is_three_ds_completion`], which is also what the request-body builder
//!   uses, so the URL and the body cannot disagree about which leg is being sent.
//!
//! This is why `/direct/threedcomplete` is modelled as `Authorize` and not as
//! `PostAuthenticate`: it is what moves the money (the vendor is explicit that "no customer
//! funds have been ring-fenced prior to this stage"), and the authentication flows are defined
//! to produce authentication data for a *following* Authorize to spend. Modelling it as
//! `PostAuthenticate` would leave an Authorize still to run, re-sending `/direct/auth` and
//! earning response code 102, "Duplicate Merchant Ref".
//!
//! Capture and Void are each a single round trip:
//!
//! * `POST /direct/capture` — completes a deferred-capture authorisation. It is bound to the
//!   original payment by `transaction.merchantRef` (the authorisation's own reference), not by
//!   a gateway id, so the reference is recomputed from the same UCS value both legs used.
//! * `POST /direct/reversal` — Ilixium's name for a void. A separate endpoint, *not* a capture
//!   for zero, with a body field-for-field identical to the capture body and the same
//!   `merchantRef` binding. Partial reversals are not supported, so the amount must be the
//!   original transaction's in full (see `transformers::IlixiumVoidRequest`).
//!
//! Refund is likewise a single round trip, and the first flow here built on `RefundFlowData`:
//!
//! * `POST /direct/refund` — variant A only (a refund that references a previous transaction),
//!   bound to the payment by the same `transaction.merchantRef`. Partial refunds are supported
//!   and accumulate; the payment must already be captured. `/direct/credit` is a payout, not a
//!   refund, and is never called. Ilixium returns no refund identifier of any kind — see
//!   `transformers::refund_identifier`.
//!
//! PSync and RSync are the outliers, and they are the *same* call:
//!
//! * `POST /history/operations` — note the path is **not** under `/direct/`. Ilixium publishes no
//!   per-payment status endpoint and no refund-status endpoint at all, so a sync is a **bulk,
//!   time-windowed reconciliation report** (max 24 hours, no server-side filter of any kind) that
//!   the connector then filters client-side. It is a POST with a body, unlike the GET-based
//!   PSync/RSync most connectors have. See `transformers::IlixiumHistoryRequest`.
//!
//!   PSync keeps the entries whose `type` is a payment-lifecycle operation; RSync keeps those
//!   whose `type` is `REFUND` — never `CREDIT`, which is a `/direct/credit` payout. Both filter on
//!   the **original payment's** `transaction.merchantRef`, which on the refund side has to be
//!   resolved out of `RefundSyncData` rather than out of the flow's common data. Refund entries
//!   echo the payment's `merchantRef` *and* `gatewayRef`, so distinguishing one partial refund
//!   from another is a known, documented limitation — see
//!   `transformers::IlixiumHistoryResponse::match_refund_operation`.
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
    connector_flow::{Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, Void},
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
use transformers::{
    self as ilixium, IlixiumAuthorizeRequest, IlixiumCaptureRequest, IlixiumCaptureResponse,
    IlixiumHistoryRequest, IlixiumHistoryResponse, IlixiumPaymentResponse,
    IlixiumPreAuthenticateRequest, IlixiumPreAuthenticateResponse, IlixiumRefundHistoryRequest,
    IlixiumRefundHistoryResponse, IlixiumRefundRequest, IlixiumRefundResponse, IlixiumVoidRequest,
    IlixiumVoidResponse,
};

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
const CAPTURE_ENDPOINT: &str = "/direct/capture";
/// Ilixium calls a void a *reversal*; there is no `void`/`cancel` endpoint on the Direct API.
const VOID_ENDPOINT: &str = "/direct/reversal";
/// The refund endpoint. **Not** `/direct/credit`: that is a money-out *payout* to a payment
/// method (`creditRequest`, response `type: CREDIT`, its own error codes 126 and 205–208) which
/// happens to have an almost identical body, and it must never be used for a refund.
const REFUND_ENDPOINT: &str = "/direct/refund";
/// The operation-history report, and the **only** query the Direct API offers — there is no
/// per-payment status endpoint (see the module docs). Note the path is `/history/…`, *not* under
/// `/direct/` like every other endpoint above.
const HISTORY_OPERATIONS_ENDPOINT: &str = "/history/operations";

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

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        ilixium::IlixiumAuthType::try_from(auth_type)?;
        Ok(Vec::new())
    }

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
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
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
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                })
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Ilixium<T>
{
}

macros::create_amount_converter_wrapper!(connector_name: Ilixium, amount_type: StringMinorUnit);

macros::create_all_prerequisites!(
    connector_name: Ilixium,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: IlixiumAuthorizeRequest<T>,
            response_body: IlixiumPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (

            flow: PreAuthenticate,
            request_body: IlixiumPreAuthenticateRequest<T>,
            response_body: IlixiumPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: IlixiumCaptureRequest,
            response_body: IlixiumCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: IlixiumVoidRequest,
            response_body: IlixiumVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: IlixiumHistoryRequest,
            response_body: IlixiumHistoryResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: IlixiumRefundRequest,
            response_body: IlixiumRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: IlixiumRefundHistoryRequest,
            response_body: IlixiumRefundHistoryResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
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
        /// `content.content.get_inner_value()` returns here, so the digest can never drift
        /// from the payload.
        ///
        /// Note the `.content` hop: `ConnectorRequestData` carries both the wire body
        /// (`content`) and a *masked* typed copy kept for observability (`typed_request`).
        /// The digest must be taken over the former — digesting the masked copy would sign
        /// redacted JSON and every request would be rejected by the gateway.
        ///
        /// Generic over the flow-common-data type (`FCD`) rather than pinned to
        /// `PaymentFlowData`: the digest is computed from the serialised body alone, so the
        /// same code serves the payment flows and the refund flows (`RefundFlowData`) without
        /// duplication.
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let method = self.get_http_method();
            let body = match method {
                Method::Get => String::default(),
                Method::Post | Method::Put | Method::Delete | Method::Patch => self
                    .get_request_body(req)?
                    .map(|content| content.content.get_inner_value().peek().to_owned())
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

        /// Refund-side counterpart of [`Self::connector_base_url_payments`]. Ilixium serves
        /// every Direct API operation from one host, so both resolve to the same value; the
        /// split exists only because `RefundFlowData` is a different common-data type.
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.ilixium.base_url
        }
    }
);

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
            let endpoint = if ilixium::is_three_ds_completion(&req.request) {
                THREE_DS_COMPLETE_ENDPOINT
            } else {
                AUTH_ENDPOINT
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), endpoint))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumPreAuthenticateRequest),
    curl_response: IlixiumPreAuthenticateResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), AUTH_ENDPOINT))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumCaptureRequest),
    curl_response: IlixiumCaptureResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), CAPTURE_ENDPOINT))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumVoidRequest),
    curl_response: IlixiumVoidResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), VOID_ENDPOINT))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumHistoryRequest),
    curl_response: IlixiumHistoryResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), HISTORY_OPERATIONS_ENDPOINT))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumRefundRequest),
    curl_response: IlixiumRefundResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), REFUND_ENDPOINT))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Ilixium,
    curl_request: Json(IlixiumRefundHistoryRequest),
    curl_response: IlixiumRefundHistoryResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_refunds(req), HISTORY_OPERATIONS_ENDPOINT))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Ilixium<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Ilixium<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Ilixium,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Ilixium,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
        SetupMandate,
        RepeatPayment
    ],
    not_supported: [
        VoidPC,
        VoidPostRefund,
        IncrementalAuthorization,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        MandateRevoke,
        Authenticate,
        PostAuthenticate,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        ClientAuthenticationToken,
        Accept,
        SubmitEvidence,
        DefendDispute
    ],
);
