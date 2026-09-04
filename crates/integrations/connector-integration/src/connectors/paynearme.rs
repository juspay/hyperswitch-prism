//! PayNearMe connector (API v3.0, JSON).
//!
//! PayNearMe is an RPC-over-JSON gateway: every operation is a `POST` to
//! `{base_url}/{operation}` with a JSON body, and query strings are forbidden
//! ("Starting in API Version 3.0, all parameters must be passed via POST and
//! CANNOT be in the query string of the URL"). There is no `Authorization`
//! header — authentication is a per-request HMAC-SHA256 `signature` field inside
//! the body, so [`ConnectorCommon::get_auth_header`] emits no headers.
//!
//! PayNearMe models money movement in three layers — order, payment method
//! (token), payment — so an order must exist before anything can be charged.
//! `should_do_order_create()` therefore returns `true` and the Authorize flow
//! reads the `pnm_order_identifier` back out of `connector_order_id`.
//!
//! Implemented scope: Card, one-time payments.
//!
//! | UCS flow      | PayNearMe call |
//! |---------------|----------------|
//! | `CreateOrder` | `POST /create_order` |
//! | `Authorize`   | `POST /create_payment_method` with `send_payment=true` |
//! | `PSync`       | `POST /find_payment` |
//! | `Void`        | `POST /cancel_payment` |
//! | `Refund`      | `POST /refund_payment` |
//! | `RSync`       | `POST /find_payment` (nested `refund` object) |
//! | `Capture`     | *(none — see below)* |
//!
//! **Capture does not exist in this API.** All 41 endpoints were checked: there
//! is no `/capture_payment`, no `capture` parameter and no `capture_method`
//! concept anywhere. Card payments are sale / auto-capture. The flow is wired as
//! `not_implemented` below so the six-flow contract still compiles, and the
//! Authorize request builder rejects `CaptureMethod::Manual*`. Capture must never
//! be pointed at `/make_payment`: that would create a second charge.
//!
//! **3-D Secure does not exist in this API either** — no enrolment or
//! verification endpoint, no CAVV/ECI/XID/dsTransId field, no ACS redirect. A
//! `ThreeDs` authorize is rejected with `NotSupported` rather than silently
//! downgraded to a non-3DS charge.

pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use common_enums::{CaptureMethod, CurrencyUnit, PaymentMethod, PaymentMethodType};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        ConnectorSpecifications, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsResponseData,
        PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
        SupportedPaymentMethodsExt,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        ConnectorInfo, Connectors, FeatureStatus, PaymentMethodDetails, SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as paynearme, PaynearmeAuthorizeRequest, PaynearmeAuthorizeResponse,
    PaynearmeCreateOrderRequest, PaynearmeCreateOrderResponse, PaynearmeRefundRequest,
    PaynearmeRefundResponse, PaynearmeRefundSyncRequest, PaynearmeRefundSyncResponse,
    PaynearmeSyncRequest, PaynearmeSyncResponse, PaynearmeVoidRequest, PaynearmeVoidResponse,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
}

/// Endpoint paths, appended to the configured base URL (which already ends in
/// the `/json-api` representation selector).
const CREATE_ORDER_PATH: &str = "/create_order";
const CREATE_PAYMENT_METHOD_PATH: &str = "/create_payment_method";
const FIND_PAYMENT_PATH: &str = "/find_payment";
const CANCEL_PAYMENT_PATH: &str = "/cancel_payment";
const REFUND_PAYMENT_PATH: &str = "/refund_payment";

// Amounts are sent as decimal strings in base units ("500", "504.99"); the HMAC
// signature is computed over the string values, so nothing may be numeric.
macros::create_amount_converter_wrapper!(connector_name: Paynearme, amount_type: StringMajorUnit);

// ===== MACRO PREREQUISITES =====
macros::create_all_prerequisites!(
    connector_name: Paynearme,
    generic_type: T,
    api: [
        (
            flow: CreateOrder,
            request_body: PaynearmeCreateOrderRequest,
            response_body: PaynearmeCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: PaynearmeAuthorizeRequest,
            response_body: PaynearmeAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: PaynearmeSyncRequest,
            response_body: PaynearmeSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: PaynearmeVoidRequest,
            response_body: PaynearmeVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: PaynearmeRefundRequest,
            response_body: PaynearmeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: PaynearmeRefundSyncRequest,
            response_body: PaynearmeRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            // No auth headers: the credentials are the body's `site_identifier`
            // plus the HMAC `signature` computed from the API secret key.
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::ACCEPT.to_string(),
                    "application/json".to_string().into(),
                ),
            ])
        }

        pub fn payments_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paynearme.base_url
        }

        pub fn refunds_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.paynearme.base_url
        }
    }
);

// ===== CONNECTOR COMMON =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Paynearme<T>
{
    fn id(&self) -> &'static str {
        "paynearme"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // Amounts travel as decimal strings in major units, e.g. "504.99".
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.paynearme.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Validate the credential shape but emit nothing: PayNearMe documents no
        // `Authorization` header, no API-key header and no idempotency header.
        paynearme::PaynearmeAuthType::try_from(auth_type)?;
        Ok(Vec::new())
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // The docs enumerate only 200 / 201 / 400. Signature failures, expired
        // keys, rate limiting and 5xx bodies are unspecified, so the documented
        // error envelope is attempted first and anything else falls through to
        // the generic message.
        let response: paynearme::PaynearmeErrorResponse = res
            .response
            .parse_struct("PaynearmeErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "paynearme: response body did not match the documented {status, errors[]} error envelope.",
            ))?;

        with_error_response_body!(event_builder, response);

        // No `attempt_status` is forced here: this handler runs for every flow
        // (`get_error_response_v2` is in `connector_default_implementations` on
        // all six) and sees only transport-level failures, which say nothing
        // about the payment. See `PaynearmeErrorResponse::to_error_response`.
        Ok(response.to_error_response(res.status_code))
    }
}

// ===== CREATE ORDER — POST /create_order =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeCreateOrderRequest),
    curl_response: PaynearmeCreateOrderResponse,
    flow_name: CreateOrder,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentCreateOrderData,
    flow_response: PaymentCreateOrderResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.payments_base_url(req), CREATE_ORDER_PATH))
        }
    }
);

// ===== AUTHORIZE — POST /create_payment_method (send_payment=true) =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeAuthorizeRequest),
    curl_response: PaynearmeAuthorizeResponse,
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
            Ok(format!(
                "{}{}",
                self.payments_base_url(req),
                CREATE_PAYMENT_METHOD_PATH
            ))
        }
    }
);

// ===== PSYNC — POST /find_payment =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeSyncRequest),
    curl_response: PaynearmeSyncResponse,
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
            Ok(format!("{}{}", self.payments_base_url(req), FIND_PAYMENT_PATH))
        }
    }
);

// ===== VOID — POST /cancel_payment =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeVoidRequest),
    curl_response: PaynearmeVoidResponse,
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
            Ok(format!(
                "{}{}",
                self.payments_base_url(req),
                CANCEL_PAYMENT_PATH
            ))
        }
    }
);

// ===== REFUND — POST /refund_payment =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeRefundRequest),
    curl_response: PaynearmeRefundResponse,
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
            Ok(format!(
                "{}{}",
                self.refunds_base_url(req),
                REFUND_PAYMENT_PATH
            ))
        }
    }
);

// ===== RSYNC — POST /find_payment, keyed on the payment id =====
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Paynearme,
    curl_request: Json(PaynearmeRefundSyncRequest),
    curl_response: PaynearmeRefundSyncResponse,
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
            Ok(format!("{}{}", self.refunds_base_url(req), FIND_PAYMENT_PATH))
        }
    }
);

// ===== CONNECTOR SERVICE TRAITS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Paynearme<T>
{
}

// PayNearMe requires an order before any money can move, so this flow is real
// rather than a stub (`should_do_order_create()` returns `true` below).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Paynearme<T>
{
    fn should_do_order_create(&self) -> bool {
        // "With PayNearMe, an order is required any time money moves." The
        // Authorize flow reads the resulting `pnm_order_identifier` back out of
        // `connector_order_id`.
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Paynearme<T>
{
}

// PayNearMe signs its Authorization and Confirmation callbacks with the same
// HMAC-SHA256 scheme, but neither callback is consumed here: outcomes are
// resolved by polling `/find_payment` (PSync).
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Paynearme<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Paynearme<T>
{
}

// ===== PAYOUTS (not offered on the card product in scope) =====
macros::macro_connector_payout_implementation!(
    connector: Paynearme,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS STUBS =====
// `Capture` is listed here deliberately: the PayNearMe API has no capture
// endpoint at all, so the flow is wired (the trait impl exists and the connector
// compiles) but every call returns `NotImplemented`. It must never be routed at
// `/make_payment`, which would charge the card a second time.
macros::macro_connector_flow_status_impls!(
    connector: Paynearme,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Capture,
        Accept,
        Authenticate,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        GetConnectorCustomer,
        MandateRevoke,
        PaymentMethodToken,
        PostAuthenticate,
        PreAuthenticate,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
    ],
    not_supported: [
        IncrementalAuthorization,
        VoidPC,
        VoidPostRefund,
    ],
);

// ===== SUPPORTED PAYMENT METHODS =====
static PAYNEARME_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> =
    LazyLock::new(|| {
        // Sale / auto-capture only: there is no capture endpoint.
        let supported_capture_methods = vec![CaptureMethod::Automatic];
        let mut supported_payment_methods = SupportedPaymentMethods::new();

        // One entry: the request always sends `payment_method_type: "card"` and
        // PayNearMe classifies credit vs debit from the BIN, reporting it back
        // in the response's `payment_type`.
        supported_payment_methods.add(
            PaymentMethod::Card,
            PaymentMethodType::Card,
            PaymentMethodDetails {
                // Mandates / autopay are out of scope for this integration.
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods,
                specific_features: None,
            },
        );

        supported_payment_methods
    });

static PAYNEARME_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "PayNearMe",
    description:
        "PayNearMe is a US payments platform for bill pay and iGaming, offering card, ACH and cash payments.",
    connector_type: domain_types::types::PaymentConnectorCategory::PaymentGateway,
};

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Paynearme<T>
{
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&PAYNEARME_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&PAYNEARME_SUPPORTED_PAYMENT_METHODS)
    }
}
