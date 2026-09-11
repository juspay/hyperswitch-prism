pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData,
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
    self as saferpay, SaferpayAuthorizeRequest, SaferpayAuthorizeResponse, SaferpayCaptureRequest,
    SaferpayCaptureResponse, SaferpayPSyncRequest, SaferpayPSyncResponse,
    SaferpayPreAuthenticateRequest, SaferpayPreAuthenticateResponse, SaferpayRefundRequest,
    SaferpayRefundResponse, SaferpayRefundSyncRequest, SaferpayRefundSyncResponse,
    SaferpayRepeatPaymentRequest, SaferpayRepeatPaymentResponse, SaferpaySetupMandateRequest,
    SaferpaySetupMandateResponse, SaferpayVoidRequest, SaferpayVoidResponse,
};

use super::macros;
use crate::types::ResponseRouterData;
use crate::with_error_response_body;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const ACCEPT: &str = "Accept";
}

/// One-shot authorization with raw card data. Never performs 3-D Secure.
const PATH_AUTHORIZE_DIRECT: &str = "/Payment/v1/Transaction/AuthorizeDirect";
/// Starts a redirect (3-D Secure) transaction and returns a session `Token`.
const PATH_INITIALIZE: &str = "/Payment/v1/Transaction/Initialize";
/// Finalises a redirect transaction from its session `Token`.
const PATH_AUTHORIZE: &str = "/Payment/v1/Transaction/Authorize";
/// Reads the current state of a payment or refund transaction.
const PATH_INQUIRE: &str = "/Payment/v1/Transaction/Inquire";
/// Settles an authorized payment or refund transaction.
const PATH_CAPTURE: &str = "/Payment/v1/Transaction/Capture";
/// Releases an authorized transaction without capturing it.
const PATH_CANCEL: &str = "/Payment/v1/Transaction/Cancel";
/// Creates a refund against a capture.
const PATH_REFUND: &str = "/Payment/v1/Transaction/Refund";
/// Registers raw card data as a Secure Card Data alias in a single call. Saferpay
/// has no zero-amount authorization, so this — not a 0-value charge — is how a
/// mandate is set up.
const PATH_ALIAS_INSERT_DIRECT: &str = "/Payment/v1/Alias/InsertDirect";

// `Amount.Value` is a string in the currency's minor units; Saferpay rejects a
// numeric value.
macros::create_amount_converter_wrapper!(connector_name: Saferpay, amount_type: StringMinorUnit);

// ===== MACRO PREREQUISITES =====
macros::create_all_prerequisites!(
    connector_name: Saferpay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: SaferpayAuthorizeRequest<T>,
            response_body: SaferpayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: SaferpayPSyncRequest,
            response_body: SaferpayPSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: SaferpayCaptureRequest,
            response_body: SaferpayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: SaferpayVoidRequest,
            response_body: SaferpayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: SaferpayRefundRequest,
            response_body: SaferpayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: SaferpayRefundSyncRequest,
            response_body: SaferpayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: PreAuthenticate,
            request_body: SaferpayPreAuthenticateRequest<T>,
            response_body: SaferpayPreAuthenticateResponse,
            router_data: RouterDataV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: SaferpaySetupMandateRequest<T>,
            response_body: SaferpaySetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: SaferpayRepeatPaymentRequest<T>,
            response_body: SaferpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = saferpay::SaferpayAuthType::try_from(&req.connector_config)?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                // A wrong Accept header is answered with HTTP 406.
                (
                    headers::ACCEPT.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    auth.basic_auth_value().into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.saferpay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.saferpay.base_url
        }

        pub fn build_refund_error_response(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, ConnectorError> {
            let response: saferpay::SaferpayErrorResponse = res
                .response
                .parse_struct("SaferpayErrorResponse")
                .change_context(crate::utils::response_deserialization_fail(
                    res.status_code,
                    "saferpay: refund error body did not match the expected format.",
                ))?;

            with_error_response_body!(event_builder, response);
            Ok(response.to_refund_error_response(res.status_code))
        }
    }
);

// ===== CONNECTOR COMMON IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Saferpay<T>
{
    fn id(&self) -> &'static str {
        "saferpay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.saferpay.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = saferpay::SaferpayAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            auth.basic_auth_value().into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Every non-200 answer carries the same error envelope; `ErrorName` is the
        // code, `ErrorMessage` the message and `ErrorDetail` (an array) the reason.
        let response: saferpay::SaferpayErrorResponse = res
            .response
            .parse_struct("SaferpayErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "saferpay: response body did not match the expected error format.",
            ))?;

        with_error_response_body!(event_builder, response);

        Ok(response.to_error_response(res.status_code))
    }
}

// ===== FLOW-SPECIFIC CONNECTOR INTEGRATION IMPLEMENTATIONS =====

// Authorize Flow — `AuthorizeDirect` for non-3DS, `Initialize` for 3DS.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayAuthorizeRequest),
    curl_response: SaferpayAuthorizeResponse,
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
            // 3DS never reaches `AuthorizeDirect`: `PreAuthenticate` opens the journey
            // with `Initialize`, and this flow finalises it with the token-based
            // `Authorize` once the shopper returns. Non-3DS goes straight to
            // `AuthorizeDirect`.
            let path = if saferpay::is_three_ds_settlement(&req.request) {
                PATH_AUTHORIZE
            } else {
                PATH_AUTHORIZE_DIRECT
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
        }
    }
);

// PSync Flow — a read-only `Inquire`. The 3DS second leg is the settle Authorize, so a
// sync never mutates state.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayPSyncRequest),
    curl_response: SaferpayPSyncResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PATH_INQUIRE
            ))
        }
    }
);

// PreAuthenticate Flow — opens the 3-D Secure journey. `Initialize` returns a session
// `Token` plus the URL of Saferpay's hosted DCC + 3DS pages; the browser goes there and
// comes back to `continue_redirection_url`, which is what routes the caller into
// `PostAuthenticate`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayPreAuthenticateRequest<T>),
    curl_response: SaferpayPreAuthenticateResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PATH_INITIALIZE
            ))
        }
    }
);

// SetupMandate Flow — standalone Secure Card Data registration.
//
// Saferpay sanctions no zero-amount authorization, and the docs forbid the 0.01 EUR
// registration workaround for Visa/Mastercard, so a mandate is set up by registering
// the card as an alias instead of by charging it. `Alias/InsertDirect` does that in a
// single call with no redirect, matching the raw-PAN posture `AuthorizeDirect`
// already has. The returned `Alias.Id` becomes the `connector_mandate_id`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpaySetupMandateRequest<T>),
    curl_response: SaferpaySetupMandateResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PATH_ALIAS_INSERT_DIRECT
            ))
        }
    }
);

// RepeatPayment Flow — merchant-initiated charge of a stored credential.
//
// Same endpoint as a non-3DS Authorize (`AuthorizeDirect`); what makes it an MIT is
// the body: `Initiator: MERCHANT` plus the stored alias in `PaymentMeans.Alias.Id`
// instead of a PAN. Saferpay has no dedicated recurring endpoint, and
// `Transaction/AuthorizeReferenced` — the alternative, which chains off a previous
// `TransactionId` rather than an alias — is out of scope: `SetupMandate` emits only
// a `connector_mandate_id`, and `AuthorizeReferenced` accepts neither `Initiator`
// nor `IssuerReference`.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayRepeatPaymentRequest<T>),
    curl_response: SaferpayRepeatPaymentResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PATH_AUTHORIZE_DIRECT
            ))
        }
    }
);

// Capture Flow — settles an authorized transaction and yields the `CaptureId` that
// a later Refund must reference.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayCaptureRequest),
    curl_response: SaferpayCaptureResponse,
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_payments(req),
                PATH_CAPTURE
            ))
        }
    }
);

// Void Flow — `Cancel`. The response has no `Status`: HTTP 200 is the success signal.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayVoidRequest),
    curl_response: SaferpayVoidResponse,
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
                self.connector_base_url_payments(req),
                PATH_CANCEL
            ))
        }
    }
);

// Refund Flow — creates a refund against a **capture**. Saferpay answers with a
// `Type: REFUND` transaction at `Status: AUTHORIZED`, which is reported as `Pending`
// because no money has moved until that refund transaction is itself captured.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Saferpay,
    curl_request: Json(SaferpayRefundRequest),
    curl_response: SaferpayRefundResponse,
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
                self.connector_base_url_refunds(req),
                PATH_REFUND
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

// RSync Flow — the same `Inquire` as PSync, keyed on the refund transaction id.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type],
    connector: Saferpay,
    curl_request: Json(SaferpayRefundSyncRequest),
    curl_response: SaferpayRefundSyncResponse,
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
            // A Saferpay refund is created `AUTHORIZED` and never settles by itself;
            // it needs an explicit Capture against the refund's own transaction id.
            // There is no "capture a refund" flow in UCS, so RSync issues it while
            // the refund is still pending, then falls back to Inquire once settled.
            let path = if saferpay::refund_needs_settlement(&req.request) {
                PATH_CAPTURE
            } else {
                PATH_INQUIRE
            };
            Ok(format!("{}{}", self.connector_base_url_refunds(req), path))
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
    connector_types::ConnectorServiceTrait<T> for Saferpay<T>
{
}

// ===== PAYMENT FLOW TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentPreAuthenticateV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Saferpay<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Saferpay<T>
{
}

// Saferpay ships no consumable webhook, so this impl stays empty and every
// `IncomingWebhook` method keeps its trait default. `get_event_type` /
// `process_*_webhook` therefore return `WebhooksNotImplemented`, which is the
// intended fail-closed answer: overriding them to hand back
// `IncomingWebhookEventUnspecified` + `Ok(None)` would advertise webhook support
// that does not exist, both to the caller and to the capability probe.
//
// This was re-verified against live sources rather than inherited from the tech
// spec, which is stale on other points.
//
// 1. There is no webhook, event-subscription or push product at all.
//    The complete published specification <https://saferpay.github.io/jsonapi/>
//    (633 KB, all endpoints) contains zero occurrences of "webhook". So does every
//    live swagger document, <https://test.saferpay.com/Api/swagger/{version}/swagger.json>
//    — checked 1.44 (the version this connector pins), 1.53 and 1.54 (the newest
//    that resolves; 1.55+ are 404). Their path sets are identical apart from
//    `Transaction/DccInquiry`, and none of them declares an inbound callback
//    schema, a signature header or an HMAC field (the single "signature" hit in
//    1.54 is the `SIGNATURE_REQUIRED` Klarna shipping attribute). The changelog
//    <https://github.com/saferpay/jsonapi/blob/master/texts/Changelog.md> covers
//    1.5 (2017) through 1.53 and never adds one; every notification entry in its
//    history is either an e-mail recipient list (v1.12 "replaced _MerchantEmail_
//    with _MerchantEmails_ ... to which the payment notification is sent",
//    v1.35 `TransactionNotification` = `PayerDccReceiptEmail` only) or a bare
//    callback URL (v1.23 "added container _RedirectNotifyUrls_", v1.24 "replaced
//    parameter `NotifyUrl` ... with the two separate parameters `SuccessNotifyUrl`
//    and `FailNotifyUrl`"). Nor is there a Backoffice setting to register one:
//    the docs sitemap <https://docs.saferpay.com/home/llms.txt> lists all 130
//    pages and has no webhook, event or notification-API page, and the one page
//    that would carry such a setting,
//    <https://docs.saferpay.com/home/interfaces/backoffice/settings>, scopes its
//    "Notifications" section to e-mail addresses for a human ("you can configure
//    if, where, and in what language Saferpay should contact you in case of
//    certain events and news. Each input accepts a comma-separated list").
//
// 2. What the callback URLs do deliver is nothing this trait can act on.
//    Transaction interface, `RedirectNotifyUrls.Success` / `.Fail`
//    <https://docs.saferpay.com/home/integration-guide/licences-and-interfaces/transaction-interface>,
//    §RedirectNotifyUrls: "The notification happens via http-GET and **does not
//    carry any data (like the token)**, except parameters, that have been added to
//    the URL by the merchant-system. ... Otherwise, the notification callback
//    would be an empty request", and again "The notification also does not return
//    any data to the merchants application, except your own parameters ... via
//    GET!". The Payment Page's `Notification.SuccessNotifyUrl` /
//    `.FailNotifyUrl` carries the identical sentence
//    <https://docs.saferpay.com/home/integration-guide/licences-and-interfaces/payment-page>.
//    So the request that would reach `ParseEvent` has no body, no Saferpay-set
//    query parameter, no event type, no resource id and no signature — there is
//    nothing to decode in `get_event_type`, nothing to return from
//    `get_webhook_event_reference`, and `verify_webhook_source` could never
//    honestly return `true`.
//
//    It is not even a payment-status event on the interface this connector uses.
//    Same section: "Note, that at this point, no transaction has been made. The
//    redirect ... only serves the purpose, to perform 3D Secure and DCC. The
//    transaction itself is made, with the execution of the transaction authorize
//    request." A `Success` ping means the redirect leg finished, not that money
//    moved, so mapping it to any concrete `AttemptStatus` or `PaymentIntent*`
//    event would be a fabrication.
//
// 3. No dispute events either, and nothing to poll for them. "chargeback" and
//    "dispute" appear zero times in the full JSON API specification and in every
//    swagger version above, and the docs sitemap has no chargeback page —
//    chargebacks are discussed only as liability-shift consequences under 3-D
//    Secure, never as something delivered to the merchant by API. There is no
//    disputed transaction state to observe either: the one reporting endpoint,
//    `GET /rest/customers/{customerId}/transactions`, documents "TransactionState
//    ... Possible values: SUCCESSFUL, FAILED, PENDING." Saferpay places the
//    chargeback relationship with the acquirer rather than itself — merchants are
//    told to keep documentation so they can "provide the acquirer with the
//    necessary documentation on request". That is why `Accept` / `DefendDispute`
//    / `SubmitEvidence` stay `not_implemented` below: there is no notification to
//    service them with, and emitting a `Dispute*` event type here would hand the
//    caller a dispute the rest of the stack has no way to act on.
//
// Correct reconciliation strategy, and note it is *not* polling. Saferpay
// forbids polling outright — <https://docs.saferpay.com/home/integration-guide/general-information>,
// §Polling: "Polling in general is strictly forbidden! You should always react to
// the redirect and/or notification, that is triggered by our gateway. Not
// following this rule, can lead to your account being blocked." The specification
// itself repeats it and names the remedy: "DO NOT implement a polling-process, to
// poll for the transaction-data. Respond with the necessary request, at the
// correct time (e.g. doing the assert only, if the SuccessUrl, or NotifyUrl are
// called). Saferpay reserves the right to otherwise deactivate, or block your
// account!", and on the result-fetch call, "Do not poll this function! Wait until
// the payer is redirected back to the shop or until the notification was called".
// The supported model is event-driven off the payer's return: the `ReturnUrl`
// redirect and, as its redundant twin, the notify-URL ping each trigger one
// `Transaction/Authorize` (or, for an already-authorized Payment Page session,
// one result fetch), with the two de-duplicated against each other — "It is
// important, that you do not handle both calls as separate transactions."
// On the UCS side that is the existing `PreAuthenticate` -> browser ->
// `Authorize` sequence, with PSync (`Transaction/Inquire`) used as a bounded
// one-shot repair for a session whose redirect was lost, not as a poll loop.
// Wiring `RedirectNotifyUrls` to the UCS webhook endpoint would be actively
// harmful while nothing consumes it: `ParseEvent` would reject the empty GET, the
// endpoint would answer non-200, and Saferpay retries a failed notification "up
// to five times more, for a total of six times", backing off "to a maximum of
// 1 day".
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Saferpay<T>
{
}

// ===== SOURCE VERIFICATION IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification for Saferpay<T>
{
}

// ===== BODY DECODING IMPLEMENTATION =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Saferpay<T>
{
}

// ===== PAYOUT TRAIT IMPLEMENTATIONS =====
macros::macro_connector_payout_implementation!(
    connector: Saferpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

// ===== FLOW STATUS IMPLEMENTATIONS =====
// Everything outside Authorize / PSync / Capture / Void / Refund / RSync /
// PreAuthenticate / SetupMandate / RepeatPayment is stubbed: alias revocation
// (`Alias/Delete`), disputes and payouts are out of scope for this card-only
// integration.
macros::macro_connector_flow_status_impls!(
    connector: Saferpay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        // Saferpay exposes no authentication-only call, so there is no second or
        // third leg for these to name.
        //
        // Its 3DS journey is `Transaction/Initialize` -> browser -> `Transaction/
        // Authorize`, and the docs are explicit that the second call is the payment:
        // "Up until now, no transaction has been made ... The transaction itself is
        // made, with the execution of the transaction authorize request", and "the
        // Transaction Authorize triggers the actual transaction, though it may only
        // happen once". Its response carries `Transaction.Id`, "obligatory for
        // capture/cancel". That is an authorization, and `PostAuthenticateResponse`
        // (`connector_types.rs:2071`) has no `resource_id` to report one with —
        // `pattern_postauthenticate.md:575` puts it plainly: "the subsequent Authorize
        // is the only flow that is allowed to transition to Authorized/Charged".
        //
        // The whole `Payment/v1/Transaction/*` inventory was checked for a call that
        // returns an authentication result without moving money: Initialize, Authorize,
        // AuthorizeDirect, AuthorizeReferenced, Capture, MultipartCapture,
        // AssertCapture, MultipartFinalize, Refund, AssertRefund, RefundDirect, Cancel,
        // Inquire, AlternativePayment, QueryAlternativePayment, DccInquiry. There is
        // none — no `AssertAuthorize` has ever existed. The one `Assert`-shaped
        // result-fetch Saferpay has, `PaymentPage/Assert`, belongs to the Payment Page
        // interface, where the authorization has *already* happened automatically
        // ("The Assert only calls for the result").
        //
        // So `PreAuthenticate` (Initialize) + `Authorize` is the honest mapping, and it
        // is one grace names for this exact shape: "Pre + Authorize only", alongside
        // Kount, Worldpayxml, NMI and Ilixium. Adding empty `Authenticate` /
        // `PostAuthenticate` legs would satisfy the flow-marker triplet and model
        // nothing.
        //
        // Externally-run 3DS does not need them either: the merchant's result arrives on
        // `PaymentsAuthorizeData::authentication_data` and goes out as
        // `Authentication.ExternalThreeDS` on `AuthorizeDirect` — the zero-leg external
        // 3DS shape, as Revolv3 does it.
        Authenticate,
        PostAuthenticate,
        IncrementalAuthorization,
        CreateOrder,
        PaymentMethodToken,
        VoidPC,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SubmitEvidence,
        GetConnectorCustomer,
        VoidPostRefund
    ],
);
