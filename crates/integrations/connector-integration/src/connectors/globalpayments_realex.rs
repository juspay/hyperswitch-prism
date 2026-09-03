//! Global Payments **Ecommerce XML API** (legacy *Realex Payments* "XML API" / "Remote API").
//!
//! Not to be confused with the [`super::globalpay`] connector, which integrates the modern GP-API
//! JSON product (`apis.sandbox.globalpay.com/ucp`, OAuth bearer). This one is the older CGI
//! endpoint that speaks XML in both directions and authenticates with a two-stage SHA-1 digest.
//! The two products share no types, transformers, status maps or configuration.
//!
//! A single endpoint serves every operation; the operation is chosen by the `type` attribute on
//! the `<request>` root element: `auth` (Authorize), `settle` (Capture), `void` (Void), `rebate`
//! (Refund) and `query` (PSync and RSync). There is no refund-status request type: RSync reuses
//! `query`, addressing the synthetic `_rebate_<orderid>` transaction the gateway stores for a
//! successful rebate (tech spec §12.6). A `query` on the *original* order id only ever echoes the
//! authorization leg, which is why PSync cannot see refund state — but the rebate leg can.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt, types::MinorUnit};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PostAuthenticate, PreAuthenticate, RSync, Refund,
        Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsPostAuthenticateData, PaymentsPreAuthenticateData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    GlobalpaymentsRealexCaptureRequest, GlobalpaymentsRealexCaptureResponse,
    GlobalpaymentsRealexPSyncRequest, GlobalpaymentsRealexPSyncResponse,
    GlobalpaymentsRealexPaymentsRequest, GlobalpaymentsRealexPaymentsResponse,
    GlobalpaymentsRealexRSyncRequest, GlobalpaymentsRealexRSyncResponse,
    GlobalpaymentsRealexRefundRequest, GlobalpaymentsRealexRefundResponse,
    GlobalpaymentsRealexVoidRequest, GlobalpaymentsRealexVoidResponse,
    Gp3ds2AuthenticationResponse, Gp3ds2AuthenticationsRequest, Gp3ds2Digest, Gp3ds2ErrorResponse,
    Gp3ds2PostAuthenticationResponse, Gp3ds2ProtocolVersionsRequest,
    Gp3ds2ProtocolVersionsResponse,
};

use super::macros::{self, GetSoapXml};
use crate::{types::ResponseRouterData, utils, with_error_response_body};

/// The gateway accepts `application/xml` and `text/xml`; the former is what the spec verified.
const CONTENT_TYPE_XML: &str = "application/xml";

/// Every operation posts to this one path under the configured base URL.
const EPAGE_REMOTE_PATH: &str = "epage-remote.cgi";

/// The 3DS2 JSON API's content type. Unrelated to [`CONTENT_TYPE_XML`], which the XML gateway
/// uses — the two APIs share a connector but not a wire format.
const CONTENT_TYPE_JSON: &str = "application/json";

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    /// 3DS2 only: `Authorization: securehash <two-stage SHA-1 digest>`.
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    /// 3DS2 only: the EMVCo protocol version the request and response are shaped for.
    pub(crate) const X_GP_VERSION: &str = "X-GP-VERSION";
}

// RealEx amounts are integers in the smallest unit of the currency. The JPY x100 special case is
// applied in the transformer, not here.
macros::create_amount_converter_wrapper!(
    connector_name: GlobalpaymentsRealex,
    amount_type: MinorUnit
);

macros::create_all_prerequisites!(
    connector_name: GlobalpaymentsRealex,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: GlobalpaymentsRealexPaymentsRequest,
            response_body: GlobalpaymentsRealexPaymentsResponse,
            response_format: xml,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: GlobalpaymentsRealexPSyncRequest,
            response_body: GlobalpaymentsRealexPSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: GlobalpaymentsRealexCaptureRequest,
            response_body: GlobalpaymentsRealexCaptureResponse,
            response_format: xml,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: GlobalpaymentsRealexVoidRequest,
            response_body: GlobalpaymentsRealexVoidResponse,
            response_format: xml,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: GlobalpaymentsRealexRefundRequest,
            response_body: GlobalpaymentsRealexRefundResponse,
            response_format: xml,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: GlobalpaymentsRealexRSyncRequest,
            response_body: GlobalpaymentsRealexRSyncResponse,
            response_format: xml,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        // The three 3DS2 flows register a response bridge only. Their requests are built inside
        // `build_request_v2` so that the `request_timestamp` in the body/query and the one inside
        // the `securehash` header come from a single clock read — see the blocking comments there.
        (
            flow: PreAuthenticate,
            response_body: Gp3ds2ProtocolVersionsResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: Authenticate,
            response_body: Gp3ds2AuthenticationResponse,
            router_data: RouterDataV2<Authenticate, PaymentFlowData, PaymentsAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: PostAuthenticate,
            response_body: Gp3ds2PostAuthenticationResponse,
            router_data: RouterDataV2<PostAuthenticate, PaymentFlowData, PaymentsPostAuthenticateData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: MinorUnit
    ],
    member_functions: {
        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_realex.base_url
        }

        /// The Refund flow hangs off `RefundFlowData` rather than `PaymentFlowData`, so it needs
        /// its own accessor; the URL it resolves to is identical.
        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.globalpayments_realex.base_url
        }

        /// The single CGI endpoint shared by `auth`, `settle`, `void`, `rebate` and `query`.
        pub fn build_endpoint_url(&self, base_url: &str) -> String {
            format!("{}{}", base_url.trim_end_matches('/'), format_args!("/{EPAGE_REMOTE_PATH}"))
        }

        /// There is no header-based authentication: the only credential material on the wire is
        /// the `<sha1hash>` element inside the body.
        pub fn build_xml_headers(&self) -> Vec<(String, Maskable<String>)> {
            vec![(
                headers::CONTENT_TYPE.to_string(),
                CONTENT_TYPE_XML.to_string().into(),
            )]
        }

        /// The 3DS2 JSON API host — a **different** host from the XML gateway, configured as
        /// `globalpayments_realex.secondary_base_url`.
        ///
        /// It is a hard error rather than a fallback to `base_url`: pointing 3DS2 traffic at the
        /// CGI endpoint would produce a confusing HTML/XML error instead of a clear config
        /// failure, and there is no sensible default that works for both sandbox and production.
        pub fn connector_3ds2_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> CustomResult<String, IntegrationError> {
            req.resource_common_data
                .connectors
                .globalpayments_realex
                .secondary_base_url
                .as_ref()
                .cloned()
                .ok_or_else(|| {
                    error_stack::Report::new(IntegrationError::InvalidConnectorConfig {
                        config: "globalpayments_realex.secondary_base_url",
                        context: Default::default(),
                    })
                })
        }

        /// `{secondary_base_url}/{path}`.
        pub fn build_3ds2_url(&self, base_url: &str, path: &str) -> String {
            format!("{}/{}", base_url.trim_end_matches('/'), path)
        }

        /// The 3DS2 request headers.
        ///
        /// Unlike the XML API — where the credential travels as a `<sha1hash>` element inside the
        /// body — the JSON API authenticates with `Authorization: securehash <digest>`. The digest
        /// covers the `request_timestamp`, so the header and the body must be built together;
        /// see the blocking comments on each flow's `build_request_v2`.
        pub fn build_3ds2_headers(
            &self,
            securehash: &Secret<String>,
        ) -> Vec<(String, Maskable<String>)> {
            vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    CONTENT_TYPE_JSON.to_string().into(),
                ),
                (
                    headers::X_GP_VERSION.to_string(),
                    transformers::GP_3DS2_VERSION.to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    Secret::new(format!("securehash {}", securehash.peek())).into_masked(),
                ),
            ]
        }

        /// Parse a 3DS2 error body, degrading gracefully when there is none.
        ///
        /// A `400` carries a structured EMVCo error document; a `401` (a bad `securehash`, or a
        /// rotated Shared Secret) carries **no body at all**, and a `502` may carry an upstream
        /// document in an entirely different shape. Insisting on the structured shape would turn
        /// any of those into an opaque deserialization failure, so anything unparsable falls
        /// through to the shared handler rather than erroring out.
        pub fn build_3ds2_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            match res
                .response
                .parse_struct::<Gp3ds2ErrorResponse>("Gp3ds2ErrorResponse")
            {
                Ok(error_response) if error_response.is_populated() => {
                    with_error_response_body!(event_builder, error_response);
                    let typed = macros::serialize_typed_connector_payload(
                        &error_response,
                        "typed_connector_response",
                    );
                    Ok(error_response.into_error_response(res.status_code, typed))
                }
                _ => {
                    // Verified live: `401` (bad securehash) returns a **zero-length** body, and
                    // `403` ("That Account ID is not configured for 3D Secure") / `404`
                    // ("Merchant information not found.") return **plain text**, not JSON. The
                    // shared handler surfaces those verbatim in `reason`, but it cannot know that
                    // a non-2xx here means the authentication is over — so re-stamp the attempt
                    // status, exactly as the structured branch does. `ensure_liability_shift` in
                    // the Authorize transformer is the backstop if a caller ignores it anyway.
                    let status_code = res.status_code;
                    let mut error_response = utils::handle_json_response_deserialization_failure(
                        res,
                        "globalpayments_realex",
                    )?;
                    if !(200..300).contains(&status_code) {
                        error_response.attempt_status = Some(FlowStatus::Payment(
                            common_enums::AttemptStatus::AuthenticationFailed,
                        ));
                    }
                    Ok(error_response)
                }
            }
        }
    }
);

// =============================================================================
// MARKER TRAITS
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthenticateV2<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPostAuthenticateV2<T> for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for GlobalpaymentsRealex<T>
{
    /// Sequence the merchant-driven 3DS2 legs for a `three_ds` card payment.
    ///
    /// Without this override the default answers `Authorize` for everything, the three
    /// authentication flows never fire, and a payment marked `three_ds` goes out as a plain
    /// non-3DS `auth` wearing a 3DS label. Modelled on `netcetera.rs`.
    ///
    /// The `RedirectWithParams` / `RedirectWithoutParams` split is what tells the two browser
    /// returns apart: the ACS's device-profiling return carries our `?gp3ds=method` query marker
    /// (see [`transformers::METHOD_RETURN_MARKER`]), the post-challenge `cres` return does not.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};

        if auth_type != common_enums::AuthenticationType::ThreeDs
            || payment_method != common_enums::PaymentMethod::Card
        {
            return AuthenticationStep::Authorize;
        }

        match (redirect_state, completed_step) {
            // Nothing has happened yet: check the card's 3DS2 enrolment and the ACS Method URL.
            (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,

            // Defensive: PreAuthenticate always returns a redirect, so this pairing should be
            // unreachable. Falling through to Authorize rather than looping keeps a future change
            // to that invariant from spinning.
            (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                AuthenticationStep::Authorize
            }

            // Back from device profiling (or from the synthesised no-DDC self-POST): run the AReq.
            (RedirectState::RedirectWithParams, None) => AuthenticationStep::Authenticate,

            // Frictionless: the AReq already carried eci / cavv / ds_trans_id.
            (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                AuthenticationStep::Authorize
            }

            // Back from the ACS challenge with a `cres` and no query string: fetch the result.
            (RedirectState::RedirectWithoutParams, None) => AuthenticationStep::PostAuthenticate,

            (RedirectState::RedirectWithoutParams, Some(AuthenticationStep::PostAuthenticate)) => {
                AuthenticationStep::Authorize
            }

            _ => AuthenticationStep::Authorize,
        }
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for GlobalpaymentsRealex<T>
{
}

// The XML card flows have no webhooks (tech spec §1).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for GlobalpaymentsRealex<T>
{
}

// Like the other XML connectors, the response body is parsed through the macro layer's XML
// pattern rather than through `BodyDecoding`.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for GlobalpaymentsRealex<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    GlobalpaymentsRealex<T>
{
    pub fn preprocess_response_bytes<F, FCD, Req, Res>(
        &self,
        _req: &RouterDataV2<F, FCD, Req, Res>,
        bytes: bytes::Bytes,
        _status_code: u16,
    ) -> CustomResult<bytes::Bytes, IntegrationError> {
        // The XML is passed through untouched; the macro layer deserializes it.
        Ok(bytes)
    }
}

// =============================================================================
// AUTHORIZE FLOW
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexPaymentsRequest),
    curl_response: GlobalpaymentsRealexPaymentsResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// PSYNC FLOW (`type="query"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexPSyncRequest),
    curl_response: GlobalpaymentsRealexPSyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // A status enquiry is a POST like everything else on this API: the same single CGI
            // endpoint, with `type="query"` on the request root.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// CAPTURE FLOW (`type="settle"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexCaptureRequest),
    curl_response: GlobalpaymentsRealexCaptureResponse,
    flow_name: Capture,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsCaptureData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as Authorize; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// VOID FLOW (`type="void"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexVoidRequest),
    curl_response: GlobalpaymentsRealexVoidResponse,
    flow_name: Void,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentVoidData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as Authorize and Capture; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_payments(req)))
        }
    }
);

// =============================================================================
// REFUND FLOW (`type="rebate"`)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexRefundRequest),
    curl_response: GlobalpaymentsRealexRefundResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Same single CGI endpoint as every other flow; only the `type` attribute differs.
            Ok(self.build_endpoint_url(self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// RSYNC FLOW (`type="query"` on the `_rebate_` leg)
// =============================================================================

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: GlobalpaymentsRealex,
    curl_request: SoapXml(GlobalpaymentsRealexRSyncRequest),
    curl_response: GlobalpaymentsRealexRSyncResponse,
    flow_name: RSync,
    resource_common_data: RefundFlowData,
    flow_request: RefundSyncData,
    flow_response: RefundsResponseData,
    http_method: Post,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            _req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(self.build_xml_headers())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // A refund status enquiry is the same `type="query"` operation PSync uses, against the
            // same single CGI endpoint; only the `<orderid>` differs.
            Ok(self.build_endpoint_url(self.connector_base_url_refunds(req)))
        }
    }
);

// =============================================================================
// 3DS2 AUTHENTICATION FLOWS (the separate JSON API — see the 3DS2 section of transformers.rs)
// =============================================================================
//
// ┌──────────────────────────────────────────────────────────────────────────────────────────┐
// │ READ THIS BEFORE TOUCHING ANY OF THE THREE `build_request_v2` OVERRIDES BELOW.            │
// │                                                                                          │
// │ The 3DS2 API authenticates with `Authorization: securehash <digest>`, and the digest is   │
// │ computed over the `request_timestamp` that the request body (or, for PostAuthenticate,    │
// │ the query string) also carries. The two MUST be the same string.                          │
// │                                                                                          │
// │ The default `build_request_v2` (interfaces/src/connector_integration_v2.rs) calls         │
// │ `get_url` and `get_headers` as *separate* calls. If the timestamp were read independently │
// │ in each, the hash would cover a different microsecond than the body does and every single │
// │ call would come back `401 Unauthorized` — with no response body, and with code that reads │
// │ perfectly correctly. So each flow reads the clock ONCE, threads that one string into both │
// │ halves, and builds the whole request itself.                                              │
// │                                                                                          │
// │ `get_url` deliberately returns an error on all three flows: if anyone ever "simplifies"   │
// │ these overrides away, the flow fails immediately and says why, instead of silently 401ing.│
// └──────────────────────────────────────────────────────────────────────────────────────────┘

/// Decode a 3DS2 JSON response through the flow's registered bridge and map it onto the router
/// data, mirroring what `macro_connector_implementation!` would have generated.
///
/// A macro rather than a function because the bridge's associated `ConnectorInputData` names the
/// flow's own `RouterDataV2`, which cannot be abstracted over without spelling out the whole
/// `dyn BridgeRequestResponse<..>` type three times.
macro_rules! handle_3ds2_response {
    ($connector:expr, $bridge:ident, $data:expr, $event_builder:expr, $res:expr) => {{
        use domain_types::connector_types::RawConnectorRequestResponse;

        let bridge = $connector.$bridge;
        let response_body = bridge.response($res.response, $res.status_code)?;
        let masked = events::MaskedSerdeValue::from_masked_optional(
            &response_body,
            "connector_response",
        );
        if let Some(ref msv) = masked {
            if let Some(evt) = $event_builder {
                evt.response_data = Some(msv.clone());
            }
        }
        tracing::info!(response = ?response_body, "response from connector");

        let response_router_data = ResponseRouterData {
            response: response_body,
            router_data: $data.clone(),
            http_code: $res.status_code,
        };
        let mut result = bridge.router_data(response_router_data, $res.status_code)?;
        result
            .resource_common_data
            .set_typed_connector_response(masked.as_ref().map(|m| m.inner().to_string()));
        Ok(result)
    }};
}

/// The error `get_url` raises on a 3DS2 flow — see the block comment above.
fn three_ds_two_url_is_built_in_build_request_v2(
    flow: &'static str,
) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotImplemented(
        format!(
            "globalpayments_realex: the 3DS2 {flow} URL and its `Authorization: securehash` \
             header must be built together from a single `request_timestamp` read, so they are \
             produced inside `build_request_v2` and `get_url` is never called. If you are seeing \
             this error, a `build_request_v2` override was removed — restore it rather than \
             implementing `get_url`, or every call will fail with HTTP 401."
        ),
        domain_types::errors::IntegrationErrorContext::default(),
    ))
}

// -----------------------------------------------------------------------------
// PreAuthenticate — POST /3ds2/protocol-versions ("check version")
// -----------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    > for GlobalpaymentsRealex<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_JSON
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(three_ds_two_url_is_built_in_build_request_v2(
            "PreAuthenticate",
        ))
    }

    /// **Do not replace this with the default builder.** See the block comment above: the
    /// `request_timestamp` is read exactly once here and threaded into both the JSON body and the
    /// `securehash` header. Two independent clock reads produce a hash over a different timestamp
    /// than the body carries and the gateway answers `401` on every call.
    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        // ── the single clock read ────────────────────────────────────────────────────────────
        let request_timestamp = transformers::current_3ds2_timestamp()?;

        let auth = transformers::GlobalpaymentsRealexAuthType::try_from(&req.connector_config)?;
        let body = Gp3ds2ProtocolVersionsRequest::try_from((req, request_timestamp.as_str()))?;

        // Hash inputs are taken off the *body that is about to be sent*, never re-derived, so the
        // digest cannot drift from the payload.
        let securehash = transformers::build_3ds2_securehash(
            Gp3ds2Digest::CheckVersion {
                timestamp: &body.request_timestamp,
                merchant_id: body.merchant_id.peek(),
                card_number: body.number.peek(),
            },
            auth.shared_secret.peek(),
        );

        let url = self.build_3ds2_url(
            &self.connector_3ds2_base_url(req)?,
            transformers::PATH_PROTOCOL_VERSIONS,
        );
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&body, "typed_connector_request");

        Ok(Some(
            common_utils::request::RequestBuilder::new()
                .method(common_utils::request::Method::Post)
                .url(&url)
                .attach_default_headers()
                .headers(self.build_3ds2_headers(&securehash))
                .set_body(common_utils::request::RequestContent::Json(Box::new(body)))
                .set_typed_connector_request(typed.map(|msv| msv.inner().clone()))
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PreAuthenticate,
            PaymentFlowData,
            PaymentsPreAuthenticateData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        handle_3ds2_response!(self, pre_authenticate, data, event_builder, res)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // `ConnectorCommon::build_error_response` is the XML gateway's parser and cannot be
        // widened: it has one signature shared by all nine flows and the two APIs disagree on
        // both the wire format and the meaning of an HTTP status.
        self.build_3ds2_error_response(res, event_builder)
    }
}

// -----------------------------------------------------------------------------
// Authenticate — POST /3ds2/authentications ("initiate authentication", AReq)
// -----------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    > for GlobalpaymentsRealex<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Post
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_JSON
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(three_ds_two_url_is_built_in_build_request_v2(
            "Authenticate",
        ))
    }

    /// **Do not replace this with the default builder.** See the block comment above: one clock
    /// read feeds both the JSON body and the `securehash` header. Splitting them yields a `401`
    /// on every call that no amount of code review will reveal.
    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        // ── the single clock read ────────────────────────────────────────────────────────────
        let request_timestamp = transformers::current_3ds2_timestamp()?;

        let auth = transformers::GlobalpaymentsRealexAuthType::try_from(&req.connector_config)?;
        let body = Gp3ds2AuthenticationsRequest::try_from((req, request_timestamp.as_str()))?;

        let securehash = transformers::build_3ds2_securehash(
            Gp3ds2Digest::InitiateAuthentication {
                timestamp: &body.request_timestamp,
                merchant_id: body.merchant_id.peek(),
                card_number: body.card_detail.number.peek(),
                server_trans_id: &body.server_trans_id,
            },
            auth.shared_secret.peek(),
        );

        let url = self.build_3ds2_url(
            &self.connector_3ds2_base_url(req)?,
            transformers::PATH_AUTHENTICATIONS,
        );
        let typed =
            events::MaskedSerdeValue::from_masked_optional(&body, "typed_connector_request");

        Ok(Some(
            common_utils::request::RequestBuilder::new()
                .method(common_utils::request::Method::Post)
                .url(&url)
                .attach_default_headers()
                .headers(self.build_3ds2_headers(&securehash))
                .set_body(common_utils::request::RequestContent::Json(Box::new(body)))
                .set_typed_connector_request(typed.map(|msv| msv.inner().clone()))
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            Authenticate,
            PaymentFlowData,
            PaymentsAuthenticateData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        handle_3ds2_response!(self, authenticate, data, event_builder, res)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_3ds2_error_response(res, event_builder)
    }
}

// -----------------------------------------------------------------------------
// PostAuthenticate — GET /3ds2/authentications/{sid} ("obtain authentication data")
// -----------------------------------------------------------------------------

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    > for GlobalpaymentsRealex<T>
{
    fn get_http_method(&self) -> common_utils::request::Method {
        common_utils::request::Method::Get
    }

    fn get_content_type(&self) -> &'static str {
        CONTENT_TYPE_JSON
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(three_ds_two_url_is_built_in_build_request_v2(
            "PostAuthenticate",
        ))
    }

    /// **Do not replace this with the default builder.** This flow has no body at all: the
    /// `request_timestamp` travels in the **query string**, and the `securehash` is computed over
    /// that same string. `get_url` and `get_headers` as separate calls would read the clock twice
    /// and the gateway would answer `401` every time.
    fn build_request_v2(
        &self,
        req: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
    ) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        // ── the single clock read ────────────────────────────────────────────────────────────
        let request_timestamp = transformers::current_3ds2_timestamp()?;

        let auth = transformers::GlobalpaymentsRealexAuthType::try_from(&req.connector_config)?;

        // The 3DS Server transaction id comes out of the CRes the ACS posted to the challenge
        // notification URL — hyperswitch builds this leg's request with `authentication_data:
        // None`, so the browser POST is the only place it can be read from.
        let payload = req.request.get_redirect_response_payload()?;
        let server_trans_id = transformers::read_challenge_return(payload.peek())?;

        let securehash = transformers::build_3ds2_securehash(
            Gp3ds2Digest::ObtainAuthenticationData {
                timestamp: &request_timestamp,
                merchant_id: auth.merchant_id.peek(),
                server_trans_id: &server_trans_id,
            },
            auth.shared_secret.peek(),
        );

        // Sent verbatim, exactly as the vendor's own worked example does — `:` and `.` are legal
        // in a query component and the gateway hashes the unencoded value.
        let url = format!(
            "{}/{}?merchant_id={}&request_timestamp={}",
            self.connector_3ds2_base_url(req)?.trim_end_matches('/'),
            format_args!("{}/{server_trans_id}", transformers::PATH_AUTHENTICATIONS),
            auth.merchant_id.peek(),
            request_timestamp,
        );

        Ok(Some(
            common_utils::request::RequestBuilder::new()
                .method(common_utils::request::Method::Get)
                .url(&url)
                .attach_default_headers()
                .headers(self.build_3ds2_headers(&securehash))
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<
            PostAuthenticate,
            PaymentFlowData,
            PaymentsPostAuthenticateData<T>,
            PaymentsResponseData,
        >,
        ConnectorError,
    > {
        handle_3ds2_response!(self, post_authenticate, data, event_builder, res)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_3ds2_error_response(res, event_builder)
    }
}

// =============================================================================
// CONNECTOR COMMON
// =============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for GlobalpaymentsRealex<T>
{
    fn id(&self) -> &'static str {
        "globalpayments_realex"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        CONTENT_TYPE_XML
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.globalpayments_realex.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Authentication is the `<sha1hash>` element in the body; there are no auth headers.
        Ok(vec![])
    }

    /// In practice this is never reached for a business failure: the gateway answers **every**
    /// outcome — declines, bad digests, malformed XML — with HTTP 200, so failures are classified
    /// from `<result>` in the response transformer. This path only covers a genuine transport-level
    /// non-200 that still carried a RealEx document.
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: GlobalpaymentsRealexPaymentsResponse = res
            .response
            .parse_struct("GlobalpaymentsRealexPaymentsResponse")
            .change_context(utils::response_deserialization_fail(
                res.status_code,
                "globalpayments_realex: response body did not match the expected format; confirm \
                 API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.result.clone(),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.to_string()),
            reason: response.message,
            attempt_status: None,
            connector_transaction_id: response.pasref,
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

// All six core flows are wired: Authorize (`auth`), PSync (`query`), Capture (`settle`), Void
// (`void`), Refund (`rebate`) and RSync (`query` on the `_rebate_` leg). A `query` on the original
// order id does only ever echo the authorization leg, so refund state is not readable *there*
// (tech spec §12.4.5) — but a successful rebate is stored as its own transaction under the
// synthetic order id `_rebate_<orderid>`, and querying that leg is exactly what RSync does
// (tech spec §12.6).
macros::macro_connector_flow_status_impls!(
    connector: GlobalpaymentsRealex,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        MandateRevoke,
        SetupMandate,
        RepeatPayment,
        PaymentMethodToken,
    ],
    not_supported: [
        VoidPostRefund,
        VoidPC,
        CreateOrder,
        SubmitEvidence,
        DefendDispute,
        Accept,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        IncrementalAuthorization,
    ],
);
