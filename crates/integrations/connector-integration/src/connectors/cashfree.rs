pub mod test;
pub mod transformers;

use std::{fmt::Debug, sync::LazyLock};

use cashfree::{
    CashfreeCaptureRequest, CashfreeCaptureResponse, CashfreeOrderCreateRequest,
    CashfreeOrderCreateResponse, CashfreePaymentRequest, CashfreePaymentResponse,
    CashfreeRefundRequest, CashfreeRefundResponse, CashfreeRefundSyncResponse, CashfreeSyncRequest,
    CashfreeSyncResponse, CashfreeVoidRequest, CashfreeVoidResponse,
};
use common_enums::{AttemptStatus, CaptureMethod, PaymentMethod, PaymentMethodType};
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{Authorize, Capture, CreateOrder, PSync, RSync, Refund, Void},
    connector_types::{
        ConnectorSpecifications, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, SupportedPaymentMethodsExt,
    },
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::{
        ConnectorInfo, Connectors, FeatureStatus, PaymentMethodDetails, SupportedPaymentMethods,
    },
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers as cashfree;

use super::macros;
use crate::{types::ResponseRouterData, with_response_body};
use domain_types::errors::{ConnectorError, IntegrationError};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const X_CLIENT_ID: &str = "X-Client-Id";
    pub(crate) const X_CLIENT_SECRET: &str = "X-Client-Secret";
    pub(crate) const X_API_VERSION: &str = "x-api-version";
}

macros::macro_connector_payout_implementation!(
    connector: Cashfree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Cashfree<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Cashfree<T>
{
}

// ── Capture ──────────────────────────────────────────────────────────────────
// Mirrors `map_capture_payment_status` in transformers.rs (called from the
// Capture TryFrom): "SUCCESS" → Charged, "FAILED" → CaptureFailed,
// "PENDING" → CaptureInitiated, "CAPTURE" (pre-auth authorization-status
// variant) → Charged, anything else → Pending (still in flight). The raw
// string is typed as `cashfree::CashfreeCaptureStatus`, which documents the
// same mapping — the `_ctx` form with `()` context is used only because the
// declarative macro's per-variant arms cannot express the `_` catch-all.
domain_types::impl_flow_status_mapping_ctx! {
    generics:        [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector:       Cashfree<T>,
    flow:            Capture,
    source:          cashfree::CashfreeCaptureStatus,
    context:         (),
    params:          [status, ctx],
    success_status:  Success,
    success_targets: [Charged],
    failure_status:  Failed,
    failure_target:  CaptureFailed,
    {
        let _ = ctx;
        use cashfree::CashfreeCaptureStatus;
        match status {
            CashfreeCaptureStatus::Success => AttemptStatus::Charged,
            CashfreeCaptureStatus::Failed => AttemptStatus::CaptureFailed,
            CashfreeCaptureStatus::Pending => AttemptStatus::CaptureInitiated,
            CashfreeCaptureStatus::Other => AttemptStatus::Pending,
        }
    }
}

// ── Void ─────────────────────────────────────────────────────────────────────
// Mirrors `map_void_payment_status` in transformers.rs (called from the Void
// TryFrom): "VOID" → Voided, "FAILED" → VoidFailed, "PENDING" → Pending,
// anything else → Pending. Cashfree never reports an explicit
// "VOID_INITIATED" string here, so Pending is the honest in-flight verdict.
// The raw string is typed as `cashfree::CashfreeVoidStatus`; same catch-all
// rationale for the `_ctx` form as Capture.
domain_types::impl_flow_status_mapping_ctx! {
    generics:        [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector:       Cashfree<T>,
    flow:            Void,
    source:          cashfree::CashfreeVoidStatus,
    context:         (),
    params:          [status, ctx],
    success_status:  Void,
    success_targets: [Voided],
    failure_status:  Failed,
    failure_target:  VoidFailed,
    {
        let _ = ctx;
        use cashfree::CashfreeVoidStatus;
        match status {
            CashfreeVoidStatus::Void => AttemptStatus::Voided,
            CashfreeVoidStatus::Failed => AttemptStatus::VoidFailed,
            CashfreeVoidStatus::Pending | CashfreeVoidStatus::Other => {
                AttemptStatus::Pending
            }
        }
    }
}

// NOTE: no impl_flow_status_mapping! for Authorize. Cashfree's authorize
// response carries no payment status at all — only a payment session/link.
// The TryFrom derives the attempt status from the response `channel` string:
// "link" (wallet/netbanking redirect or UPI intent/QR deep link) →
// AuthenticationPending, "collect" (UPI collect — customer approves in-app,
// no redirect) → Pending, any other channel → Failure. These are
// redirect-stage verdicts, not terminal outcomes (the payment resolves via
// PSync), and the macro demands success/failure terminals the flow produces
// from the connector status — AuthenticationPending is a non-terminal in
// Authorize::ALLOWED that cannot satisfy the TERMINAL_SUCCESS_SET const
// assertion, and there is no success terminal to declare.

// NOTE: no impl_flow_status_mapping! for CreateOrder. Order creation is an
// ack-only step — the TryFrom hardcodes `AttemptStatus::Pending` regardless
// of the response body, so there is no status mapping to mirror.

// ── PSync ────────────────────────────────────────────────────────────────────
// Mirrors the `From<CashfreePaymentStatus> for AttemptStatus` impl in
// transformers.rs that the PSync TryFrom routes through. The `_ctx` variant is
// used (with `()` context) only because `Unknown(String)` is a tuple variant
// that the plain macro's unit-variant arms cannot match.
domain_types::impl_flow_status_mapping_ctx! {
    generics:        [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector:       Cashfree<T>,
    flow:            PSync,
    source:          cashfree::CashfreePaymentStatus,
    context:         (),
    params:          [status, _ctx],
    success_status:  Success,
    success_targets: [Charged],
    failure_status:  Failed,
    failure_target:  Failure,
    {
        match status {
            cashfree::CashfreePaymentStatus::Success => AttemptStatus::Charged,
            cashfree::CashfreePaymentStatus::Pending
            | cashfree::CashfreePaymentStatus::NotAttempted => {
                AttemptStatus::Pending
            }
            cashfree::CashfreePaymentStatus::Failed
            | cashfree::CashfreePaymentStatus::Cancelled
            | cashfree::CashfreePaymentStatus::UserDropped => {
                AttemptStatus::Failure
            }
            cashfree::CashfreePaymentStatus::Unknown(_) => AttemptStatus::Pending,
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Cashfree<T>
{
}
// Mirrors `map_refund_status(&response.refund_status)` in transformers.rs —
// the raw `refund_status` string is first typed into `CashfreeRefundStatus`
// ("SUCCESS"/"OK" → Success, "PENDING" → Pending, "CANCELLED"/"FAILED" →
// Failure, anything else → `Other` → Pending, i.e. still in flight). The
// `_ctx` form with `()` context is used only because the declarative macro's
// match arms cannot cover the catch-all `Other` semantics in a readable way.
domain_types::impl_refund_flow_status_mapping_ctx! {
    generics:       [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector:      Cashfree<T>,
    flow:           RSync,
    source:         cashfree::CashfreeRefundStatus,
    context:        (),
    params:         [status, ctx],
    success_status: Success,
    failure_status: Failed,
    {
        let _ = ctx;
        use common_enums::RefundStatus;
        use cashfree::CashfreeRefundStatus;
        match status {
            CashfreeRefundStatus::Success => RefundStatus::Success,
            CashfreeRefundStatus::Pending => RefundStatus::Pending,
            CashfreeRefundStatus::Cancelled | CashfreeRefundStatus::Failed => {
                RefundStatus::Failure
            }
            CashfreeRefundStatus::Other => RefundStatus::Pending,
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Cashfree<T>
{
}
// Refund runs the identical `map_refund_status` mapping as RSync — the refund
// create response carries the same `refund_status` string.
domain_types::impl_refund_flow_status_mapping_ctx! {
    generics:       [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector:      Cashfree<T>,
    flow:           Refund,
    source:         cashfree::CashfreeRefundStatus,
    context:        (),
    params:         [status, ctx],
    success_status: Success,
    failure_status: Failed,
    {
        let _ = ctx;
        use common_enums::RefundStatus;
        use cashfree::CashfreeRefundStatus;
        match status {
            CashfreeRefundStatus::Success => RefundStatus::Success,
            CashfreeRefundStatus::Pending => RefundStatus::Pending,
            CashfreeRefundStatus::Cancelled | CashfreeRefundStatus::Failed => {
                RefundStatus::Failure
            }
            CashfreeRefundStatus::Other => RefundStatus::Pending,
        }
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Cashfree<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Cashfree<T>
{
    fn should_do_order_create(&self) -> bool {
        true // Cashfree V3 requires order creation
    }
}

// Define connector prerequisites
macros::create_all_prerequisites!(
    connector_name: Cashfree,
    generic_type: T,
    api: [
        (
            flow: CreateOrder,
            request_body: CashfreeOrderCreateRequest,
            response_body: CashfreeOrderCreateResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: CashfreePaymentRequest,
            response_body: CashfreePaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: CashfreeCaptureRequest,
            response_body: CashfreeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: CashfreeSyncRequest,
            response_body: CashfreeSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: CashfreeVoidRequest,
            response_body: CashfreeVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: CashfreeRefundRequest,
            response_body: CashfreeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: CashfreeRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        pub fn connector_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.cashfree.base_url.to_string()
        }

        pub fn refund_base_url<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.cashfree.base_url.to_string()
        }
    }
);

// CreateOrder flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_request: Json(CashfreeOrderCreateRequest),
    curl_response: CashfreeOrderCreateResponse,
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
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}pg/orders"))
        }
    }
);

// Authorize flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_request: Json(CashfreePaymentRequest),
    curl_response: CashfreePaymentResponse,
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
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}pg/orders/sessions"))
        }
    }
);

// Capture flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_request: Json(CashfreeCaptureRequest),
    curl_response: CashfreeCaptureResponse,
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
            let order_id = req
                .request
                .merchant_order_id
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "merchant_order_id",
                    context: Default::default(),
                })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}pg/orders/{order_id}/authorization"))
        }
    }
);

// PSync flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_response: CashfreeSyncResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            // Cashfree PSync URL uses the merchant order_id, not cf_payment_id.
            // Try reference_id (connector_order_reference_id from gRPC) first,
            // then connector_request_reference_id (merchant_transaction_id).
            let order_id = req
                .resource_common_data
                .reference_id
                .as_ref()
                .unwrap_or(&req.resource_common_data.connector_request_reference_id);
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}pg/orders/{order_id}/payments"))
        }
    }
);

// Void flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_request: Json(CashfreeVoidRequest),
    curl_response: CashfreeVoidResponse,
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
            let order_id = req
                .request
                .merchant_order_id
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "merchant_order_id",
                    context: Default::default(),
                })?;
            let base_url = self.connector_base_url(req);
            Ok(format!("{base_url}pg/orders/{order_id}/authorization"))
        }
    }
);

// Refund flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_request: Json(CashfreeRefundRequest),
    curl_response: CashfreeRefundResponse,
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
            let order_id = &req.request.connector_transaction_id;
            let base_url = self.refund_base_url(req);
            Ok(format!("{base_url}pg/orders/{order_id}/refunds"))
        }
    }
);

// Type alias for non-generic trait implementations

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Cashfree<T>
{
    fn id(&self) -> &'static str {
        "cashfree"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Base // For major units
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.cashfree.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = cashfree::CashfreeAuthType::try_from(auth_type)?;
        Ok(vec![
            (headers::X_CLIENT_ID.to_string(), auth.app_id.into_masked()),
            (
                headers::X_CLIENT_SECRET.to_string(),
                auth.secret_key.into_masked(),
            ),
            (
                headers::X_API_VERSION.to_string(),
                "2022-09-01".to_string().into(),
            ),
            (
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: cashfree::CashfreeErrorResponse = res
            .response
            .parse_struct("CashfreeErrorResponse")
            .change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                "cashfree: response body did not match the expected format; confirm API version and connector documentation."),
            )?;

        with_response_body!(event_builder, response);

        let attempt_status = match response.code.as_str() {
            "AUTHENTICATION_ERROR" => AttemptStatus::AuthenticationFailed,
            "AUTHORIZATION_ERROR" => AttemptStatus::AuthorizationFailed,
            "INVALID_REQUEST_ERROR" => AttemptStatus::Failure,
            "GATEWAY_ERROR" => AttemptStatus::Failure,
            "SERVER_ERROR" => AttemptStatus::Pending,
            _ => AttemptStatus::Failure,
        };

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.code.clone(),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: Some(FlowStatus::Payment(attempt_status)),
            connector_transaction_id: None,
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

// RSync flow implementation using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Cashfree,
    curl_response: CashfreeRefundSyncResponse,
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
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let order_id = &req.request.connector_transaction_id;
            let refund_id = &req.request.connector_refund_id;
            let base_url = self.refund_base_url(req);
            Ok(format!("{base_url}pg/orders/{order_id}/refunds/{refund_id}"))
        }
    }
);

// ServerSessionAuthenticationToken stub implementation

// ServerAuthenticationToken stub implementation

// CreateConnectorCustomer stub implementation

// ============================================================================
// Supported Payment Methods
// ============================================================================

static CASHFREE_SUPPORTED_PAYMENT_METHODS: LazyLock<SupportedPaymentMethods> =
    LazyLock::new(|| {
        let cashfree_supported_capture_methods =
            vec![CaptureMethod::Automatic, CaptureMethod::Manual];

        let mut cashfree_supported_payment_methods = SupportedPaymentMethods::new();

        // UPI - UpiIntent (UPI_PAY)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Upi,
            PaymentMethodType::UpiIntent,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // UPI - UpiCollect (UPI_COLLECT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Upi,
            PaymentMethodType::UpiCollect,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // UPI - UpiQr (UPI_QR)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Upi,
            PaymentMethodType::UpiQr,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - AmazonPay (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::AmazonPay,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - GooglePay (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::GooglePay,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - PhonePe (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::PhonePe,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - LazyPay (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::LazyPay,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - BillDesk (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::BillDesk,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - Cashfree (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::Cashfree,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - PayU (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::PayU,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Wallet - EaseBuzz (REDIRECT_WALLET_DEBIT)
        cashfree_supported_payment_methods.add(
            PaymentMethod::Wallet,
            PaymentMethodType::EaseBuzz,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        // Netbanking
        cashfree_supported_payment_methods.add(
            PaymentMethod::BankRedirect,
            PaymentMethodType::Netbanking,
            PaymentMethodDetails {
                mandates: FeatureStatus::NotSupported,
                refunds: FeatureStatus::Supported,
                supported_capture_methods: cashfree_supported_capture_methods.clone(),
                specific_features: None,
            },
        );

        cashfree_supported_payment_methods
    });

static CASHFREE_CONNECTOR_INFO: ConnectorInfo = ConnectorInfo {
    display_name: "Cashfree",
    description: "Cashfree Payments is an Indian payment gateway and banking technology company.",
    connector_type: domain_types::types::PaymentConnectorCategory::PaymentGateway,
};

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorSpecifications
    for Cashfree<T>
{
    fn get_connector_about(&self) -> Option<&'static ConnectorInfo> {
        Some(&CASHFREE_CONNECTOR_INFO)
    }

    fn get_supported_payment_methods(&self) -> Option<&'static SupportedPaymentMethods> {
        Some(&CASHFREE_SUPPORTED_PAYMENT_METHODS)
    }
}

macros::macro_connector_flow_status_impls!(
    connector: Cashfree,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        SetupMandate,
        Accept,
        SubmitEvidence,
        PaymentMethodToken,
        ServerSessionAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        DefendDispute,
        RepeatPayment,
        ClientAuthenticationToken,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        IncrementalAuthorization,
        ServerAuthenticationToken,
        VoidPC,
    ],
);
