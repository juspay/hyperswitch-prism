pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        VerifyWebhookSource, Void,
    },
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        SetupMandateRequestData, VerifyWebhookSourceFlowData,
    },
    errors::{ConnectorError, IntegrationError, WebhookError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::Response,
    router_response_types::VerifyWebhookSourceResponseData,
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
    SaferpayWebhookVerifyRequest, SaferpayWebhookVerifyResponse,
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
/// Registers a card alias in one synchronous server-to-server call (mandate setup).
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
            request_body: SaferpayRepeatPaymentRequest,
            response_body: SaferpayRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: VerifyWebhookSource,
            request_body: SaferpayWebhookVerifyRequest,
            response_body: SaferpayWebhookVerifyResponse,
            router_data: RouterDataV2<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData>,
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

// SetupMandate Flow — `Alias/InsertDirect`: a $0, synchronous, server-to-server card
// registration. The response's `Alias.Id` is the `connector_mandate_id` that
// RepeatPayment later charges, and it is returned on the same call — unlike the
// redirect-based `Alias/Insert` + `Alias/AssertInsert` pair, which only yields the
// alias after the payer returns from a hosted form.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpaySetupMandateRequest),
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

// RepeatPayment Flow — an unscheduled merchant-initiated `AuthorizeDirect` against the
// alias the SetupMandate flow registered. Same endpoint as the non-3DS Authorize, but
// the payment means is `PaymentMeans.Alias.Id` (the `connector_mandate_id`) and the
// request carries the MIT `Authentication{Initiator: MERCHANT, Exemption: RECURRING}`
// block instead of raw card data.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayRepeatPaymentRequest),
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

// VerifyWebhookSource Flow — Saferpay sends **unsigned** `*NotifyUrl` pings, so a
// signature can never authenticate them. Source verification is the authenticated
// pull the spec names ("receive notify URL → verify with an authenticated Assert
// call"), implemented as `Transaction/Inquire` on the transaction reference the
// ping carries (plan UD-12 option b): verified only when the response's
// `Transaction.Id` matches the claimed reference — an in-band 2xx with a missing
// or mismatched transaction is `SourceNotVerified`, never silently verified.
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Saferpay,
    curl_request: Json(SaferpayWebhookVerifyRequest),
    curl_response: SaferpayWebhookVerifyResponse,
    flow_name: VerifyWebhookSource,
    resource_common_data: VerifyWebhookSourceFlowData,
    flow_request: VerifyWebhookSourceRequestData,
    flow_response: VerifyWebhookSourceResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<VerifyWebhookSource, VerifyWebhookSourceFlowData, VerifyWebhookSourceRequestData, VerifyWebhookSourceResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.base_url(&req.resource_common_data.connectors),
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

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Saferpay<T>
{
}

// ===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Saferpay<T>
{
    /// Composite dispatch for the one-leg Saferpay 3DS journey.
    ///
    /// Saferpay-managed 3DS has exactly two calls: `Initialize` opens the journey
    /// and mints the session `Token` plus the ACS redirect URL (the
    /// `PreAuthenticate` leg), and the token-bearing `Authorize` settles it after
    /// the shopper returns (the charging call). There is no `Authenticate` or
    /// `PostAuthenticate` leg — Saferpay runs the authentication itself and never
    /// returns a CAVV/ECI — so the legs table carries `PreAuthenticate` only, and
    /// any redirect state means the shopper is back from the ACS and the settle
    /// `Authorize` is the next step, whatever `completed_step` the new
    /// `CompositeAuthorize` call happens to carry.
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                // Nothing run yet: open the journey with `Initialize`.
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
                // Frictionless exit: `Initialize` resolved inline (RedirectRequired:
                // false, so no redirect was emitted and the composite loop did not
                // break) — the token is still good, so settle it directly instead
                // of looping back into PreAuthenticate.
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authorize
                }
                // The shopper is back from the ACS (redirect_response is populated):
                // spend the session token with the settle Authorize, regardless of
                // whether the ACS returned query params.
                (RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams, _) => {
                    AuthenticationStep::Authorize
                }
                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
}

// Saferpay has no signed webhook: only unauthenticated, bodyless `*NotifyUrl`
// GET pings (spec:## Webhook Events). The ping itself never proves anything, so
// the stateless half only *classifies* the URL family (success/fail/ambiguous)
// and surfaces the notification reference, while the details half reports the
// outcome the URL asserts. Whether that assertion is true is decided by the
// authenticated `Transaction/Inquire` pull of the VerifyWebhookSource flow
// (UD-12), whose result the server folds into `source_verified`; an ambiguous
// ping fails closed here (G-webhook-01) and an unverifiable one stays
// `source_verified = false` there.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Saferpay<T>
{
    fn sample_webhook_body(&self) -> &'static [u8] {
        br#"{"id":"SaferpayTransactionIdplaceholder0"}"#
    }

    fn get_event_type(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<domain_types::connector_types::EventType, error_stack::Report<WebhookError>> {
        let body = if request.body.is_empty() {
            None
        } else {
            request
                .body
                .parse_struct::<saferpay::SaferpayWebhookBody>("SaferpayWebhookBody")
                .ok()
        };
        saferpay::classify_notify_event(request.uri.as_deref(), body.as_ref())
    }

    fn get_webhook_event_reference(
        &self,
        request: domain_types::connector_types::RequestDetails,
    ) -> Result<
        Option<domain_types::connector_types::WebhookResourceReference>,
        error_stack::Report<WebhookError>,
    > {
        let is_refund = saferpay::classify_notify_event(request.uri.as_deref(), None)
            .map(|event_type| event_type.is_refund_event())
            .unwrap_or(false);
        // The stateless phase only names the reference — the authenticated
        // pull resolves the transaction id behind the session token.
        let token_hint =
            saferpay::notify_query_token(request.query_params.as_deref()).or_else(|| {
                (!request.body.is_empty()).then(|| {
                    request
                        .body
                        .parse_struct::<saferpay::SaferpayWebhookBody>("SaferpayWebhookBody")
                        .ok()
                        .and_then(|body| body.id)
                })?
            });
        if is_refund {
            Ok(Some(
                domain_types::connector_types::WebhookResourceReference::Refund(
                    domain_types::connector_types::RefundWebhookReference {
                        connector_refund_id: None,
                        merchant_refund_id: None,
                        connector_transaction_id: token_hint,
                        merchant_transaction_id: None,
                    },
                ),
            ))
        } else {
            Ok(Some(
                domain_types::connector_types::WebhookResourceReference::Payment(
                    domain_types::connector_types::PaymentWebhookReference {
                        connector_transaction_id: token_hint,
                        merchant_transaction_id: None,
                    },
                ),
            ))
        }
    }

    fn process_payment_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<
        domain_types::connector_types::WebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let raw = String::from_utf8_lossy(&request.body).to_string();
        let status = saferpay::webhook_attempt_status(saferpay::classify_notify_url(
            request.uri.as_deref(),
        ))?;
        // A failure ping carries no reason of its own; the connector-side cause
        // only ever surfaces on the Inquire response, not on the notify URL.
        let (error_code, error_message) = if status == common_enums::AttemptStatus::Failure {
            (
                Some("NOTIFY_FAILED".to_string()),
                Some("Saferpay reported the transaction outcome on the FailNotifyUrl".to_string()),
            )
        } else {
            (None, None)
        };
        Ok(domain_types::connector_types::WebhookDetailsResponse {
            resource_id: saferpay::notify_query_token(request.query_params.as_deref())
                .or_else(|| {
                    (!request.body.is_empty()).then(|| {
                        request
                            .body
                            .parse_struct::<saferpay::SaferpayWebhookBody>("SaferpayWebhookBody")
                            .ok()
                            .and_then(|body| body.id)
                    })?
                })
                .map(domain_types::connector_types::ResponseId::ConnectorTransactionId),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id: None,
            mandate_reference: None,
            error_code,
            error_message,
            error_reason: None,
            raw_connector_response: Some(raw),
            status_code: 200,
            response_headers: None,
            amount_captured: None,
            minor_amount_captured: None,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        })
    }

    fn process_refund_webhook(
        &self,
        request: domain_types::connector_types::RequestDetails,
        _connector_webhook_secret: Option<domain_types::connector_types::ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        domain_types::connector_types::RefundWebhookDetailsResponse,
        error_stack::Report<WebhookError>,
    > {
        let raw = String::from_utf8_lossy(&request.body).to_string();
        let status = match saferpay::classify_notify_url(request.uri.as_deref()) {
            saferpay::SaferpayNotifyPath::Success => common_enums::RefundStatus::Success,
            saferpay::SaferpayNotifyPath::Fail => common_enums::RefundStatus::Failure,
            saferpay::SaferpayNotifyPath::Ambiguous => {
                return Err(WebhookError::WebhookEventTypeNotFound.into())
            }
        };
        let (error_code, error_message) = if status == common_enums::RefundStatus::Failure {
            (
                Some("NOTIFY_FAILED".to_string()),
                Some("Saferpay reported the refund outcome on the FailNotifyUrl".to_string()),
            )
        } else {
            (None, None)
        };
        Ok(
            domain_types::connector_types::RefundWebhookDetailsResponse {
                connector_refund_id: None,
                merchant_transaction_id: None,
                status,
                connector_response_reference_id: None,
                error_code,
                error_message,
                raw_connector_response: Some(raw),
                status_code: 200,
                response_headers: None,
            },
        )
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Saferpay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyWebhookSourceV2 for Saferpay<T>
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
// PreAuthenticate / SetupMandate / RepeatPayment is stubbed: mandate revocation,
// tokenization, disputes and payouts are out of scope for this card-only
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
        // Both exist to hand `AuthenticationData` to a following Authorize. Saferpay's
        // second call *is* the authorization, so it lives on Authorize instead.
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
