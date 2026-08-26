//! Saferpay connector (SIX Payment Services).
//!
//! Saferpay's Transaction interface is a POST-only, RPC-style JSON API: every
//! operation is a `POST` to a fixed path under `/Payment/v1/Transaction/*`, there are
//! no GET endpoints, and no path or query parameters exist — the resource being acted
//! on is always named in the JSON body. Authentication is HTTP Basic over the API
//! user, while the `CustomerId` and `TerminalId` travel inside the body
//! (`RequestHeader.CustomerId` and the top-level `TerminalId`).
//!
//! Implemented scope: **Card, one-time payments only**.
//!
//! | UCS flow | Saferpay endpoint |
//! |---|---|
//! | Authorize (non-3DS) | `POST /Payment/v1/Transaction/AuthorizeDirect` |
//! | Authorize (3DS) | `POST /Payment/v1/Transaction/Initialize` |
//! | PSync (3DS second leg) | `POST /Payment/v1/Transaction/Authorize` |
//! | PSync / RSync | `POST /Payment/v1/Transaction/Inquire` |
//! | Capture | `POST /Payment/v1/Transaction/Capture` |
//! | Void | `POST /Payment/v1/Transaction/Cancel` |
//! | Refund | `POST /Payment/v1/Transaction/Refund` |
//!
//! **3DS second leg.** UCS has no `CompleteAuthorize` flow, so the token-based
//! `Authorize` that finalises a redirect transaction is issued from **PSync**, which
//! branches on whether the held handle is an `Initialize` session token (carried in
//! `PaymentsSyncData::encoded_data`) or a real `Transaction.Id`.
//!
//! **Capture method.** Saferpay has no auto-capture request field on
//! `AuthorizeDirect` / `Initialize` — settlement behaviour is a terminal-level
//! Backoffice setting and this interface always answers `AUTHORIZED`. `Manual` is
//! therefore the supported path; `Automatic` is accepted and reported honestly as
//! `Authorized`, leaving the caller to issue the explicit Capture.
//!
//! **Webhooks are not supported.** Saferpay offers no signed webhook for the
//! Transaction interface — only unauthenticated, bodyless `NotifyUrl` GET pings that
//! carry nothing verifiable — so state reconciliation goes through PSync / RSync.

pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData,
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
    SaferpayCaptureResponse, SaferpayPSyncRequest, SaferpayPSyncResponse, SaferpayRefundRequest,
    SaferpayRefundResponse, SaferpayRefundSyncRequest, SaferpayRefundSyncResponse,
    SaferpayVoidRequest, SaferpayVoidResponse,
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
            // `AuthorizeDirect` never performs 3-D Secure, so a 3DS attempt has to
            // go through the redirect-based `Initialize`.
            let path = if saferpay::is_three_ds_authorize(req) {
                PATH_INITIALIZE
            } else {
                PATH_AUTHORIZE_DIRECT
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
        }
    }
);

// PSync Flow — either the 3DS second leg (`Authorize` with the session token) or a
// plain `Inquire`, decided by whether a token was handed back to us.
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
            // While the attempt is `AuthenticationPending` no transaction exists
            // yet, so `Inquire` would answer TRANSACTION_NOT_FOUND; the token-based
            // `Authorize` both finalises and reports the transaction.
            let path = if saferpay::pending_three_ds_token(&req.request).is_some() {
                PATH_AUTHORIZE
            } else {
                PATH_INQUIRE
            };
            Ok(format!("{}{}", self.connector_base_url_payments(req), path))
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
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
    }
);

// RSync Flow — the same `Inquire` as PSync, keyed on the refund transaction id.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
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
            Ok(format!(
                "{}{}",
                self.connector_base_url_refunds(req),
                PATH_INQUIRE
            ))
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
    connector_types::PaymentAuthorizeV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Saferpay<T>
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

// Saferpay's Transaction interface exposes no signed webhook — only unauthenticated,
// bodyless `NotifyUrl` GET pings — so there is nothing to consume or verify here.
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
// Everything outside Authorize / PSync / Capture / Void / Refund / RSync is stubbed:
// mandates, tokenization (Alias / Secure Card Data), disputes and payouts are out of
// scope for this card-only integration.
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
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence,
        GetConnectorCustomer,
        VoidPostRefund
    ],
);
