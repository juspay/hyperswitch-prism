pub mod crypto;
pub mod transformers;

use std::fmt::Debug;

use base64::Engine;
use common_enums::CurrencyUnit;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateOrder, PSync, RSync, RefreshPaymentMethod, Refund, Void,
    },
    connector_types::{
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefreshPaymentMethodData, RefreshPaymentMethodFlowData, RefreshPaymentMethodResponseData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData,
    },
    errors::{self, IntegrationError},
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
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as juspay, JuspayAuthorizeRequest, JuspayAuthorizeResponse, JuspayCaptureRequest,
    JuspayCaptureResponse, JuspayCardSyncRequest, JuspayCardSyncResponse, JuspayCreateOrderRequest,
    JuspayCreateOrderResponse, JuspayOrderStatusResponse, JuspayRefundRequest,
    JuspayRefundResponse, JuspayRefundSyncResponse, JuspayVoidRequest, JuspayVoidResponse,
};

use crate::{types::ResponseRouterData, with_error_response_body};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

pub(crate) mod headers {
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const X_MERCHANT_ID: &str = "x-merchantid";
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const VERSION: &str = "version";
}

const JUSPAY_API_VERSION: &str = "2023-06-30";

/// Shortest PAN Juspay's card-sync accepts; used by the transformers module.
pub(super) const JUSPAY_MIN_PAN_LENGTH: usize = 13;

use super::macros;

macros::create_amount_converter_wrapper!(connector_name: Juspay, amount_type: StringMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Juspay,
    generic_type: T,
    api: [
        (
            flow: CreateOrder,
            request_body: JuspayCreateOrderRequest,
            response_body: JuspayCreateOrderResponse,
            router_data: RouterDataV2<CreateOrder, PaymentFlowData, PaymentCreateOrderData, PaymentCreateOrderResponse>,
        ),
        (
            flow: Authorize,
            request_body: JuspayAuthorizeRequest,
            response_body: JuspayAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            response_body: JuspayOrderStatusResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: JuspayCaptureRequest,
            response_body: JuspayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: JuspayRefundRequest,
            response_body: JuspayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: JuspayRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: Void,
            request_body: JuspayVoidRequest,
            response_body: JuspayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: RefreshPaymentMethod,
            request_body: JuspayCardSyncRequest,
            response_body: JuspayCardSyncResponse,
            router_data: RouterDataV2<RefreshPaymentMethod, RefreshPaymentMethodFlowData, RefreshPaymentMethodData<T>, RefreshPaymentMethodResponseData>,
        )
    ],
    amount_converters: [],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let mut headers = vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::VERSION.to_string(),
                    JUSPAY_API_VERSION.to_string().into(),
                ),
            ];
            let mut auth_headers = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth_headers);
            Ok(headers)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.juspay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.juspay.base_url
        }

        pub fn build_card_sync_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            let auth = juspay::JuspayCardSyncAuthType::try_from(&req.connector_config)?;
            let encoded_api_key = BASE64_ENGINE.encode(format!("{}:", auth.api_key.peek()));

            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    "application/json".to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Basic {encoded_api_key}").into_masked(),
                ),
            ])
        }

        pub fn connector_base_url_refresh<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefreshPaymentMethodFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.juspay.base_url
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Juspay<T>
{
    fn id(&self) -> &'static str {
        "juspay"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.juspay.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = juspay::JuspayAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        let encoded_api_key = BASE64_ENGINE.encode(format!("{}:", auth.api_key.peek()));
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Basic {encoded_api_key}").into_masked(),
            ),
            (
                headers::X_MERCHANT_ID.to_string(),
                auth.merchant_id.peek().to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: juspay::JuspayErrorResponse = res
            .response
            .parse_struct("JuspayErrorResponse")
            .change_context(crate::utils::response_deserialization_fail(
                res.status_code,
                "juspay: response body did not match the expected error format; \
                 confirm API version and connector documentation.",
            ))?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error_code
            .clone()
            .or_else(|| {
                response
                    .error_info
                    .as_ref()
                    .and_then(|info| info.code.clone())
            })
            .or_else(|| response.status.clone())
            .unwrap_or_else(|| res.status_code.to_string());

        let message = response
            .error_message
            .clone()
            .or_else(|| response.user_message.clone())
            .or_else(|| {
                response
                    .error_info
                    .as_ref()
                    .and_then(|info| info.user_message.clone())
            })
            .or_else(|| response.status.clone())
            .unwrap_or_else(|| format!("juspay: HTTP {}", res.status_code));

        let reason = response
            .error_info
            .as_ref()
            .and_then(|info| {
                info.user_message
                    .clone()
                    .or_else(|| info.developer_message.clone())
            })
            .or_else(|| response.user_message.clone())
            .or_else(|| response.error_message.clone());

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason,
            attempt_status: None,
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

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayCreateOrderRequest),
    curl_response: JuspayCreateOrderResponse,
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
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}orders"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayAuthorizeRequest),
    curl_response: JuspayAuthorizeResponse,
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
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}txns"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_response: JuspayOrderStatusResponse,
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
            let order_id = req
                .resource_common_data
                .connector_order_id
                .clone()
                .unwrap_or_else(|| {
                    req.resource_common_data
                        .connector_request_reference_id
                        .clone()
                });
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayCaptureRequest),
    curl_response: JuspayCaptureResponse,
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
            let txn_uuid = req
                .request
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                })?;
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}v2/txns/{txn_uuid}/capture"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayRefundRequest),
    curl_response: JuspayRefundResponse,
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
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}orders/{order_id}/refunds"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_response: JuspayRefundSyncResponse,
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
            let base_url = self.connector_base_url_refunds(req);
            Ok(format!("{base_url}orders/{order_id}"))
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: FormUrlEncoded(JuspayVoidRequest),
    curl_response: JuspayVoidResponse,
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
            let txn_uuid = req.request.connector_transaction_id.as_str();
            if txn_uuid.is_empty() {
                return Err(error_stack::report!(
                    IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    }
                ));
            }
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}v2/txns/{txn_uuid}/void"))
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Juspay<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Juspay<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Juspay<T>
{
}

// CreateOrder is not an AttemptStatus flow (its response type is
// `PaymentCreateOrderResponse`, carrying an order id) — outside macro scope.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for Juspay<T>
{
}

// Authorize: mirrors `From<JuspayOrderStatus> for AttemptStatus`
// (transformers.rs:177). The shared From is order-lifecycle-wide, so the
// capture/void legs (`CaptureInitiated`, `CaptureFailed`, `VoidInitiated`,
// `VoidFailed`) — which the authorize endpoint never legitimately returns —
// are folded into Authorize-legal statuses: still-in-flight states stay
// `Pending`, and a state already past the authorization leg counts as an
// authorization failure. (Precedent: the Capture macro folds them to
// `CaptureFailed`; Void folds them to `VoidFailed`.)
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Juspay<T>,
    flow:      Authorize,
    source:    transformers::JuspayOrderStatus,
    success:   Charged => Charged,
    failure:   AuthorizationFailed => Failure,
    {
        New => Started,
        Created => Started,
        Started => Pending,
        Authorizing => Pending,
        VbvSuccessful => Pending,
        CodInitiated => Pending,
        PendingVbv => AuthenticationPending,
        Authorized => Authorized,
        Voided => Voided,
        VoidInitiated => AuthorizationFailed,
        CaptureInitiated => AuthorizationFailed,
        CaptureFailed => AuthorizationFailed,
        VoidFailed => AuthorizationFailed,
        AutoRefunded => AutoRefunded,
        AuthenticationFailed => Failure,
        JuspayDeclined => Failure,
        NotFound => Failure
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Juspay<T>
{
}

// PSync: mirrors the same `From<JuspayOrderStatus> for AttemptStatus`
// (transformers.rs:177) — every target is PSync-legal.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Juspay<T>,
    flow:      PSync,
    source:    transformers::JuspayOrderStatus,
    success:   Charged => Charged,
    failure:   AuthorizationFailed => Failure,
    {
        New => Started,
        Created => Started,
        Started => Pending,
        Authorizing => Pending,
        VbvSuccessful => Pending,
        CodInitiated => Pending,
        PendingVbv => AuthenticationPending,
        Authorized => Authorized,
        Voided => Voided,
        VoidInitiated => VoidInitiated,
        CaptureInitiated => CaptureInitiated,
        CaptureFailed => CaptureFailed,
        VoidFailed => VoidFailed,
        AutoRefunded => AutoRefunded,
        AuthenticationFailed => Failure,
        JuspayDeclined => Failure,
        NotFound => Failure
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Juspay<T>
{
}

// Capture: mirrors the Capture TryFrom via the shared `From<JuspayOrderStatus>`
// (transformers.rs:1077), with the payment-lifecycle arms it can never legitimately
// return folded into Capture-legal statuses (mirroring how cybersource maps
// Voided/Reversed/Cancelled to CaptureFailed): an un-captured order state is
// progress (`Pending`), a voided/refunded order has failed the capture.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Juspay<T>,
    flow:      Capture,
    source:    transformers::JuspayOrderStatus,
    success:   Charged => Charged,
    failure:   CaptureFailed => CaptureFailed,
    {
        CaptureInitiated => CaptureInitiated,
        New => Pending,
        Created => Pending,
        Started => Pending,
        Authorizing => Pending,
        VbvSuccessful => Pending,
        CodInitiated => Pending,
        Authorized => Pending,
        PendingVbv => CaptureFailed,
        Voided => CaptureFailed,
        VoidInitiated => CaptureFailed,
        VoidFailed => CaptureFailed,
        AutoRefunded => CaptureFailed,
        AuthenticationFailed => Failure,
        AuthorizationFailed => Failure,
        JuspayDeclined => Failure,
        NotFound => Failure
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Juspay<T>
{
}

// NOTE — Refund: the Refund TryFrom (transformers.rs:1184) never reads a wire
// status enum for the response-level branch. It searches `refunds[]` for the
// entry matching the request's `refund_id` (or falls back to the last entry)
// and stamps `RefundStatus::Pending` whenever the selected entry's `status`
// field is absent. The status therefore depends on list structure + two
// request-side values, not on a connector status alone — no typed source enum
// covers that domain. Lifting this needs the response to carry a single
// authoritative status field (or the TryFrom to error on a missing entry)
// instead of list-scan + Pending fallback.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Juspay<T>
{
}

// NOTE — RSync: the RSync TryFrom (transformers.rs:1238) has the same
// shape as Refund: find the `refunds[]` entry matching `connector_refund_id`,
// else stamp `RefundStatus::Pending`. The match-on-list + Pending fallback
// cannot be expressed against a connector status enum (a missing entry is not
// a status). Lifting needs an authoritative per-refund status field or a
// lookup error on a missing entry.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Juspay<T>
{
}

// Void: mirrors `From<JuspayOrderStatus>` (transformers.rs:1327), folding the
// non-void lifecycle arms into Void-legal statuses: pre-void order states are
// progress (`Pending`), a transaction already in the capture lifecycle cannot
// be voided (`VoidFailed`).
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Juspay<T>,
    flow:      Void,
    source:    transformers::JuspayOrderStatus,
    success:   Voided => Voided,
    failure:   VoidFailed => VoidFailed,
    {
        VoidInitiated => VoidInitiated,
        New => Pending,
        Created => Pending,
        Started => Pending,
        Authorizing => Pending,
        VbvSuccessful => Pending,
        CodInitiated => Pending,
        PendingVbv => Pending,
        Authorized => Pending,
        Charged => VoidFailed,
        CaptureInitiated => VoidFailed,
        CaptureFailed => VoidFailed,
        AutoRefunded => VoidFailed,
        AuthenticationFailed => Failure,
        AuthorizationFailed => Failure,
        JuspayDeclined => Failure,
        NotFound => Failure
    }
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Juspay<T>
{
}

crate::connectors::macros::macro_connector_payout_implementation!(
    connector: Juspay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefreshPaymentMethodV2<T> for Juspay<T>
{
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Juspay,
    curl_request: Json(JuspayCardSyncRequest),
    curl_response: JuspayCardSyncResponse,
    flow_name: RefreshPaymentMethod,
    resource_common_data: RefreshPaymentMethodFlowData,
    flow_request: RefreshPaymentMethodData<T>,
    flow_response: RefreshPaymentMethodResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RefreshPaymentMethod, RefreshPaymentMethodFlowData, RefreshPaymentMethodData<T>, RefreshPaymentMethodResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_card_sync_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RefreshPaymentMethod, RefreshPaymentMethodFlowData, RefreshPaymentMethodData<T>, RefreshPaymentMethodResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_refresh(req);
            Ok(format!("{base_url}cardAccountUpdater"))
        }
    }
);

crate::connectors::macros::macro_connector_flow_status_impls!(
    connector: Juspay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        IncrementalAuthorization,
        MandateRevoke,
        PaymentMethodToken,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate
    ],
    not_supported: [
        VoidPostRefund,
        Accept,
        DefendDispute,
        SubmitEvidence,
        Authenticate,
        PreAuthenticate,
        PostAuthenticate,
        VoidPC
    ],
);
