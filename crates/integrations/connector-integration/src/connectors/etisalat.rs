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
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Etisalat<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Etisalat<T>
{
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
