pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult, events, ext_traits::ByteSliceExt, types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, RepeatPayment, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundsData, RefundsResponseData, RepeatPaymentData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
    utils,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, AuthenticationStep, RedirectState},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as etisalat, EtisalatAuthorizeRequest, EtisalatCaptureRequest, EtisalatRefundRequest,
    EtisalatRepeatPaymentRequest, EtisalatResponse, EtisalatReversalRequest,
};

// Per-flow aliases so the `create_all_prerequisites!` macro can synthesise
// distinct `<Response>Templating` types for each flow — every flow re-uses
// the shared `EtisalatResponse` wire shape.
use transformers::EtisalatResponse as EtisalatAuthorizeResponse;
use transformers::EtisalatResponse as EtisalatCaptureResponse;
use transformers::EtisalatResponse as EtisalatReversalResponse;
use transformers::EtisalatResponse as EtisalatRepeatPaymentResponse;
use transformers::EtisalatResponse as EtisalatRefundResponse;

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
}

const APPLICATION_JSON: &str = "application/json";

// Marker trait wiring — every flow the macro-based connector framework relies on
// must have an impl block here. Flows we do not implement are still required to
// be listed via `macro_connector_flow_status_impls!` below.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Etisalat<T>
{
}
// Mirrors `map_attempt_status` (transformers.rs:741) as the Authorize TryFrom
// (transformers.rs:800) calls it: `is_auto_capture` chooses the success leg
// (`Charged` auto / `Authorized` manual), `"0"` → that leg, a pending response
// code → `Pending`, anything else → `Failure`. The response code string is
// parsed into `EtisalatResponseVerdict` (transformers.rs) — the macro body
// matches enum variants, never raw strings. The ctx bool is the request's
// `is_auto_capture()` (Default false = manual capture, matching a `None`
// capture_method).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Etisalat<T>,
    flow: Authorize,
    source: etisalat::EtisalatResponseVerdict,
    context: bool,
    mapping: |verdict, is_auto_capture| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        use etisalat::EtisalatResponseVerdict;
        match verdict {
            EtisalatResponseVerdict::Success => {
                if is_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            EtisalatResponseVerdict::Pending => AttemptStatus::Pending,
            EtisalatResponseVerdict::Failure => AttemptStatus::Failure,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Authorized => success!(Authorized),
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::PartiallyAuthorized => success!(PartiallyAuthorized),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::AutoRefunded => non_terminal!(AutoRefunded),
            common_enums::AttemptStatus::ConfirmationAwaited => non_terminal!(ConfirmationAwaited),
            common_enums::AttemptStatus::DeviceDataCollectionPending => non_terminal!(DeviceDataCollectionPending),
            common_enums::AttemptStatus::Expired => non_terminal!(Expired),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::PaymentMethodAwaited => non_terminal!(PaymentMethodAwaited),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            common_enums::AttemptStatus::Unknown => non_terminal!(Unknown),
            common_enums::AttemptStatus::Unresolved => non_terminal!(Unresolved),
            common_enums::AttemptStatus::Unspecified => non_terminal!(Unspecified),
            common_enums::AttemptStatus::Voided => non_terminal!(Voided),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsAuthorizeData<T>,
        response: EtisalatResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(etisalat::EtisalatResponseVerdict::from(&response.transaction)) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(request.is_auto_capture()) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Etisalat<T>
{
}
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Etisalat<T>,
    flow: Capture,
    source: etisalat::EtisalatResponseVerdict,
    context: bool,
    mapping: |verdict, is_partial| { match verdict {
        etisalat::EtisalatResponseVerdict::Success if is_partial => success!(PartialCharged),
        etisalat::EtisalatResponseVerdict::Success => success!(Charged),
        etisalat::EtisalatResponseVerdict::Pending => non_terminal!(Pending),
        etisalat::EtisalatResponseVerdict::Failure => failure!(CaptureFailed),
    } },
    runtime: {
        request: PaymentsCaptureData,
        response: EtisalatResponse,
        source: |_resource_common_data, _request, response, _http_status_code| {
            Ok(etisalat::EtisalatResponseVerdict::from(&response.transaction))
        },
        context: |resource_common_data, request, _response, _http_status_code| {
            Ok(domain_types::flow_status::FlowStatusReader::payment_amount(resource_common_data)
                .is_some_and(|authorized| request.amount_to_capture.amount < authorized))
        },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Etisalat<T>
{
}
// The refund leg (transformers.rs:1031) keys its `RefundsResponseData.refund_status`
// off the same `is_success()`/`is_pending()` response-code split as
// `map_attempt_status` — `RefundStatus::Success` on `"0"`, `Pending` on the
// PENDING_RESPONSE_CODES, `Failure` otherwise. The connector status string is
// parsed into `EtisalatResponseVerdict` at the trust boundary (transformers.rs);
// the macro body matches its variants, no raw string matching.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Etisalat<T>,
    flow: Refund,
    source: etisalat::EtisalatResponseVerdict,
    mapping: |status| {
        match status {
            etisalat::EtisalatResponseVerdict::Success => success!(Success),
            etisalat::EtisalatResponseVerdict::Failure => failure!(Failure),
            etisalat::EtisalatResponseVerdict::Pending => non_terminal!(Pending),
        }
    },
    runtime: {
        request: RefundsData,
        response: EtisalatResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(etisalat::EtisalatResponseVerdict::from(&response.transaction)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Etisalat<T>
{
}
// Mirrors the Void TryFrom (transformers.rs:940): `map_attempt_status(&body,
// AttemptStatus::Voided, AttemptStatus::VoidFailed)` — all Void targets land in
// Void::ALLOWED (Voided / Pending / VoidFailed).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Etisalat<T>,
    flow: Void,
    source: etisalat::EtisalatResponseVerdict,
    mapping: |status| {
        match status {
            etisalat::EtisalatResponseVerdict::Success => success!(Voided),
            etisalat::EtisalatResponseVerdict::Failure => failure!(VoidFailed),
            etisalat::EtisalatResponseVerdict::Pending => non_terminal!(Pending),
        }
    },
    runtime: {
        request: PaymentVoidData,
        response: EtisalatResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(etisalat::EtisalatResponseVerdict::from(&response.transaction)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Etisalat<T>
{
}
// Mirrors the RepeatPayment TryFrom (transformers.rs:999): payer-not-present
// recurring charges always auto-capture on Etisalat, so the call is
// `map_attempt_status(&body, AttemptStatus::Charged, AttemptStatus::Failure)`
// — a fixed success leg with no request-side split.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Etisalat<T>,
    flow: RepeatPayment,
    source: etisalat::EtisalatResponseVerdict,
    mapping: |status| {
        match status {
            etisalat::EtisalatResponseVerdict::Success => success!(Charged),
            etisalat::EtisalatResponseVerdict::Failure => failure!(Failure),
            etisalat::EtisalatResponseVerdict::Pending => non_terminal!(Pending),
        }
    },
    runtime: {
        request: RepeatPaymentData<T>,
        response: EtisalatResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(etisalat::EtisalatResponseVerdict::from(&response.transaction)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Etisalat<T>
{
}
// PSync, RSync, ServerAuthentication, PreAuthenticate, PostAuthenticate marker
// traits are supplied by `macro_connector_flow_status_impls!` below (they must
// not be duplicated here, or coherence checks reject the build).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Etisalat<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        // Etisalat carries UserName/Password/Customer inside every request body;
        // no server-side OAuth exchange is required.
        false
    }

    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: common_enums::PaymentMethod,
        _redirect_state: RedirectState,
        _completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        AuthenticationStep::Authorize
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Etisalat<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Etisalat<T>
{
    fn id(&self) -> &'static str {
        "etisalat"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // Amounts are sent as decimal-string major units ("10.00"), even
        // though prism internally carries minor units — the amount_converter
        // handles the conversion at request-building time.
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        APPLICATION_JSON
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.etisalat.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Etisalat always returns a `{"Transaction": {...}}` envelope, even for
        // application-level failures. Fall back to a raw-body error only if the
        // JSON is malformed (e.g. gateway/WAF served HTML).
        let parsed = res
            .response
            .parse_struct::<EtisalatResponse>("EtisalatResponse");

        match parsed {
            Ok(response) => {
                with_error_response_body!(event_builder, response);
                Ok(etisalat::build_error_response(
                    &response.transaction,
                    res.status_code,
                    None,
                ))
            }
            Err(error_msg) => {
                if let Some(event) = event_builder {
                    event.set_connector_response(&serde_json::json!({
                        "error": "Error response parsing failed",
                        "status_code": res.status_code,
                    }));
                }
                tracing::error!(deserialization_error =? error_msg);
                utils::handle_json_response_deserialization_failure(res, "etisalat")
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Struct scaffolding + per-flow bridge templates for the five flows we
// implement (Authorize, Capture, Void, Refund, RepeatPayment).
// ----------------------------------------------------------------------------

macros::create_all_prerequisites!(
    connector_name: Etisalat,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: EtisalatAuthorizeRequest<T>,
            response_body: EtisalatAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: EtisalatCaptureRequest,
            response_body: EtisalatCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: EtisalatReversalRequest,
            response_body: EtisalatReversalResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: EtisalatRepeatPaymentRequest,
            response_body: EtisalatRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: EtisalatRefundRequest,
            response_body: EtisalatRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        fn body_only_headers(&self) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // Auth is embedded in the JSON body; only content negotiation belongs
            // in the HTTP headers.
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    APPLICATION_JSON.to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    APPLICATION_JSON.to_string().into(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.etisalat.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.etisalat.base_url
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Etisalat,
    curl_request: Json(EtisalatAuthorizeRequest<T>),
    curl_response: EtisalatAuthorizeResponse,
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
            _req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.body_only_headers()
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Etisalat,
    curl_request: Json(EtisalatCaptureRequest),
    curl_response: EtisalatCaptureResponse,
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
            _req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.body_only_headers()
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
    connector: Etisalat,
    curl_request: Json(EtisalatReversalRequest),
    curl_response: EtisalatReversalResponse,
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
            _req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.body_only_headers()
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
    connector: Etisalat,
    curl_request: Json(EtisalatRepeatPaymentRequest),
    curl_response: EtisalatRepeatPaymentResponse,
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
            _req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.body_only_headers()
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
    connector: Etisalat,
    curl_request: Json(EtisalatRefundRequest),
    curl_response: EtisalatRefundResponse,
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
            _req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.body_only_headers()
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// Flows this connector deliberately does not offer. Etisalat has no per-txn
// sync endpoint and its recurrence setup requires a 3DS redirect flow that is
// out of scope here; webhooks only cover Central Bank offline payments.
macros::macro_connector_flow_status_impls!(
    connector: Etisalat,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PSync,
        RSync,
        SetupMandate,
        PreAuthenticate,
        PostAuthenticate,
        ServerAuthenticationToken,
    ],
    not_supported: [
        ServerSessionAuthenticationToken,
        VoidPostRefund,
        IncrementalAuthorization,
        Accept,
        SubmitEvidence,
        DefendDispute,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        Authenticate,
        ClientAuthenticationToken,
        MandateRevoke,
        VoidPC,
    ],
);
