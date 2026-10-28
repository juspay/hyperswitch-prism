pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    types::StringMajorUnit,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use hyperswitch_masking::Maskable;
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self},
    decode::BodyDecoding,
    verification::SourceVerification,
};
use serde::Serialize;
// Merchante returns the same flat urlencoded shape from every endpoint (single
// `/mes-api/tridentApi` URL, discriminated by `transaction_type`), so every
// flow decodes into the same struct — aliased at import.
use transformers::{
    MerchanteCaptureRequest, MerchantePaymentResponse as MerchanteAuthorizeResponse,
    MerchantePaymentResponse as MerchanteCaptureResponse,
    MerchantePaymentResponse as MerchanteVoidResponse,
    MerchantePaymentResponse as MerchanteRefundResponse,
    MerchantePaymentResponse as MerchanteRefundSyncResponse,
    MerchantePaymentResponse as MerchanteSyncResponse,
    MerchantePaymentResponse as MerchanteRepeatPaymentResponse,
    MerchantePaymentResponse as MerchanteSetupMandateResponse, MerchantePaymentResponse,
    MerchantePaymentsRequest, MerchanteRefundRequest, MerchanteRefundSyncRequest,
    MerchanteRepeatPaymentRequest, MerchanteSetupMandateRequest, MerchanteSyncRequest,
    MerchanteVoidRequest,
};

use super::macros;
use crate::{
    types::ResponseRouterData, utils::response_deserialization_fail, with_error_response_body,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
}

// ============================================================================
// CONNECTOR COMMON IMPLEMENTATION
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Merchante<T>
{
    fn id(&self) -> &'static str {
        "merchante"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Merchante puts profile_id + profile_key in the request body, not headers.
        Ok(vec![])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.merchante.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        // Merchante error bodies are raw URL-encoded — `preprocess_response_bytes`
        // only runs inside `handle_response_v2`, not here. Use `serde_qs` directly,
        // the same parser the preprocess hook uses.
        let body_str = String::from_utf8_lossy(res.response.as_ref());
        let trimmed = body_str.trim();
        let parsed = serde_qs::from_str::<MerchantePaymentResponse>(trimmed).map_err(|e| {
            error_stack::report!(response_deserialization_fail(
                res.status_code,
                "merchante: error body was not URL-encoded key=value pairs",
            ))
            .attach_printable(format!("serde_qs parse error: {e:?}"))
        });

        let (code, message, reason) = match parsed {
            Ok(ref r) => {
                with_error_response_body!(event_builder, r);
                // The gateway may return a body with no error_code / auth_response_text
                // populated. Emit NO_ERROR_CODE / NO_ERROR_MESSAGE rather than
                // fabricating one from the HTTP status.
                let code = if r.error_code.is_empty() {
                    NO_ERROR_CODE.to_string()
                } else {
                    r.error_code.clone()
                };
                let message = r
                    .auth_response_text
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());
                (code, message.clone(), Some(message))
            }
            Err(_) => {
                // Body didn't parse as the documented shape (e.g. WAF/proxy HTML page).
                // Keep NO_ERROR_CODE / NO_ERROR_MESSAGE for the machine-readable fields
                // and surface a short raw-body preview via `reason` for debugging.
                let body_preview = String::from_utf8_lossy(res.response.as_ref())
                    .chars()
                    .take(300)
                    .collect::<String>();
                with_error_response_body!(event_builder, body_preview);
                (
                    NO_ERROR_CODE.to_string(),
                    NO_ERROR_MESSAGE.to_string(),
                    Some(body_preview),
                )
            }
        };

        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: None,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

// ============================================================================
// FOUNDATION SETUP — create_all_prerequisites!
// ============================================================================

macros::create_all_prerequisites!(
    connector_name: Merchante,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: MerchantePaymentsRequest<T>,
            response_body: MerchanteAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: MerchanteSyncRequest,
            response_body: MerchanteSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: MerchanteCaptureRequest,
            response_body: MerchanteCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: MerchanteVoidRequest,
            response_body: MerchanteVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: MerchanteRefundRequest,
            response_body: MerchanteRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            request_body: MerchanteRefundSyncRequest,
            response_body: MerchanteRefundSyncResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
        (
            flow: RepeatPayment,
            request_body: MerchanteRepeatPaymentRequest,
            response_body: MerchanteRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>,
        ),
        (
            flow: SetupMandate,
            request_body: MerchanteSetupMandateRequest<T>,
            response_body: MerchanteSetupMandateResponse,
            router_data: RouterDataV2<SetupMandate, PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>,
        )
    ],
    amount_converters: [
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        // Merchante returns responses as urlencoded key=value pairs. The macro
        // framework wants JSON, so we parse the urlencoded body once and
        // re-emit it as JSON before the downstream `curl_response` type deserializes it.
        fn preprocess_response_bytes<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            bytes: bytes::Bytes,
            status_code: u16,
        ) -> CustomResult<bytes::Bytes, ConnectorError> {
            let response_str = String::from_utf8_lossy(&bytes);
            let trimmed = response_str.trim();

            let parsed: MerchantePaymentResponse = serde_qs::from_str(trimmed)
                .map_err(|e| {
                    error_stack::report!(response_deserialization_fail(
                        status_code,
                        "merchante: response body did not match the expected urlencoded shape"
                    ))
                    .attach_printable(format!("serde_qs parse error: {:?}", e))
                })?;

            let json_bytes = serde_json::to_vec(&parsed).map_err(|e| {
                error_stack::report!(response_deserialization_fail(
                    status_code,
                    "merchante: internal serialization error"
                ))
                .attach_printable(format!("serde_json to_vec error: {:?}", e))
            })?;

            Ok(bytes::Bytes::from(json_bytes))
        }

        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            Ok(vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )])
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.merchante.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.merchante.base_url
        }
    }
);

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Merchante<T>
{
}

// Authorize: mirrors `MerchantePaymentStatus::to_attempt_status` (transformers.rs:310) —
// Approved → Charged (auto-capture) / Authorized (manual), Declined → Failure; the
// failure target does not depend on capture intent, unlike bambora, but the success
// target does, hence the ctx.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: Authorize,
    source: transformers::MerchantePaymentStatus,
    context: transformers::MerchanteCaptureIntent,
    mapping: |status, ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        use transformers::MerchantePaymentStatus;
        match status {
            MerchantePaymentStatus::Approved => {
                if ctx.is_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            MerchantePaymentStatus::Declined => AttemptStatus::Failure,
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
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(transformers::MerchanteCaptureIntent {
            is_auto_capture: request.is_auto_capture(),
        }) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Merchante<T>
{
}

// PSync: mirrors the PSync TryFrom (transformers.rs:601), which reuses
// `MerchantePaymentStatus::to_attempt_status` with the request's capture intent.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: PSync,
    source: transformers::MerchantePaymentStatus,
    context: transformers::MerchanteCaptureIntent,
    mapping: |status, ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        use transformers::MerchantePaymentStatus;
        match status {
            MerchantePaymentStatus::Approved => {
                if ctx.is_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            MerchantePaymentStatus::Declined => AttemptStatus::Failure,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Authorized => success!(Authorized),
            common_enums::AttemptStatus::AutoRefunded => success!(AutoRefunded),
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::PartiallyAuthorized => success!(PartiallyAuthorized),
            common_enums::AttemptStatus::Voided => success!(Voided),
            common_enums::AttemptStatus::VoidedPostCapture => success!(VoidedPostCapture),
            common_enums::AttemptStatus::AuthenticationFailed => failure!(AuthenticationFailed),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::CaptureFailed => failure!(CaptureFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::VoidFailed => failure!(VoidFailed),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::AuthenticationSuccessful => non_terminal!(AuthenticationSuccessful),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::CaptureInitiated => non_terminal!(CaptureInitiated),
            common_enums::AttemptStatus::CodInitiated => non_terminal!(CodInitiated),
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
            common_enums::AttemptStatus::VoidInitiated => non_terminal!(VoidInitiated),
            common_enums::AttemptStatus::VoidPostCaptureInitiated => non_terminal!(VoidPostCaptureInitiated),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsSyncData,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(transformers::MerchanteCaptureIntent {
            is_auto_capture: request.is_auto_capture(),
        }) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for Merchante<T>
{
}

// Capture: mirrors the Capture TryFrom (transformers.rs:785-789) — an approved
// Settle is Charged, anything else Failure.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: Capture,
    source: transformers::MerchantePaymentStatus,
    mapping: |status| {
        match status {
            transformers::MerchantePaymentStatus::Approved => success!(Charged),
            transformers::MerchantePaymentStatus::Declined => failure!(Failure),
        }
    },
    runtime: {
        request: PaymentsCaptureData,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for Merchante<T>
{
}

// Void: mirrors the Void TryFrom (transformers.rs:877-881) — an approved Void is
// Voided, anything else VoidFailed.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: Void,
    source: transformers::MerchantePaymentStatus,
    mapping: |status| {
        match status {
            transformers::MerchantePaymentStatus::Approved => success!(Voided),
            transformers::MerchantePaymentStatus::Declined => failure!(VoidFailed),
        }
    },
    runtime: {
        request: PaymentVoidData,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for Merchante<T>
{
}

// Refund: mirrors the Refund TryFrom, i.e. `From<MerchantePaymentStatus> for
// RefundStatus` (transformers.rs:319) — approved → Success, declined → Failure.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: Refund,
    source: transformers::MerchantePaymentStatus,
    mapping: |status| {
        match status {
            transformers::MerchantePaymentStatus::Approved => success!(Success),
            transformers::MerchantePaymentStatus::Declined => failure!(Failure),
        }
    },
    runtime: {
        request: RefundsData,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Merchante<T>
{
}

// RSync: mirrors the RSync TryFrom (transformers.rs:676-692), the same
// `From<MerchantePaymentStatus> for RefundStatus` mapping.
domain_types::impl_refund_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: RSync,
    source: transformers::MerchantePaymentStatus,
    mapping: |status| {
        match status {
            transformers::MerchantePaymentStatus::Approved => success!(Success),
            transformers::MerchantePaymentStatus::Declined => failure!(Failure),
        }
    },
    runtime: {
        request: RefundSyncData,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for Merchante<T>
{
}

// RepeatPayment: mirrors the RepeatPayment TryFrom (transformers.rs:1197-1198),
// the same `to_attempt_status` helper as Authorize.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: RepeatPayment,
    source: transformers::MerchantePaymentStatus,
    context: transformers::MerchanteCaptureIntent,
    mapping: |status, ctx| {
        let mapped_status = (|| { use common_enums::AttemptStatus;
        use transformers::MerchantePaymentStatus;
        match status {
            MerchantePaymentStatus::Approved => {
                if ctx.is_auto_capture {
                    AttemptStatus::Charged
                } else {
                    AttemptStatus::Authorized
                }
            }
            MerchantePaymentStatus::Declined => AttemptStatus::Failure,
        } })();
        match mapped_status {
            common_enums::AttemptStatus::Charged => success!(Charged),
            common_enums::AttemptStatus::PartialCharged => success!(PartialCharged),
            common_enums::AttemptStatus::AuthorizationFailed => failure!(AuthorizationFailed),
            common_enums::AttemptStatus::Failure => failure!(Failure),
            common_enums::AttemptStatus::IntegrityFailure => failure!(IntegrityFailure),
            common_enums::AttemptStatus::AuthenticationPending => non_terminal!(AuthenticationPending),
            common_enums::AttemptStatus::Authorized => non_terminal!(Authorized),
            common_enums::AttemptStatus::Authorizing => non_terminal!(Authorizing),
            common_enums::AttemptStatus::PartialChargedAndChargeable => non_terminal!(PartialChargedAndChargeable),
            common_enums::AttemptStatus::PartiallyAuthorized => non_terminal!(PartiallyAuthorized),
            common_enums::AttemptStatus::Pending => non_terminal!(Pending),
            common_enums::AttemptStatus::Started => non_terminal!(Started),
            _ => failure!(Failure),
        }
    },
    runtime: {
        request: RepeatPaymentData<T>,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
        context: |_resource_common_data, request, _response, _http_status_code| { Ok(transformers::MerchanteCaptureIntent {
            is_auto_capture: request.is_auto_capture(),
        }) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for Merchante<T>
{
}

// SetupMandate: mirrors the SetupMandate TryFrom (transformers.rs:1350-1388) — an
// approved $0 Verify (store_card=Y) is stamped Charged (the flow's terminal success),
// a decline goes to the error path.
domain_types::impl_flow_status_mapping! {
    generics: [T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    connector: Merchante<T>,
    flow: SetupMandate,
    source: transformers::MerchantePaymentStatus,
    mapping: |status| {
        match status {
            transformers::MerchantePaymentStatus::Approved => success!(Charged),
            transformers::MerchantePaymentStatus::Declined => failure!(Failure),
        }
    },
    runtime: {
        request: SetupMandateRequestData<T>,
        response: MerchantePaymentResponse,
        source: |_resource_common_data, _request, response, _http_status_code| { Ok(transformers::MerchantePaymentStatus::from(response)) },
    },
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for Merchante<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Merchante<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Merchante<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Merchante<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Merchante<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Merchante<T>
{
}

// ============================================================================
// FLOW IMPLEMENTATIONS
// ============================================================================

// Authorize (transaction_type = D for auto-capture, P for manual)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchantePaymentsRequest),
    curl_response: MerchanteAuthorizeResponse,
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
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// PSync (Inquiry, transaction_type = I, keyed on retry_id = connector_request_reference_id)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteSyncRequest),
    curl_response: MerchanteSyncResponse,
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
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Capture (Settle, transaction_type = S)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteCaptureRequest),
    curl_response: MerchanteCaptureResponse,
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
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Void (transaction_type = V)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteVoidRequest),
    curl_response: MerchanteVoidResponse,
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
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// Refund (transaction_type = U — auto-becomes Void, Adjustment, or Credit)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteRefundRequest),
    curl_response: MerchanteRefundResponse,
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
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// RSync — same Inquiry endpoint (transaction_type = I, keyed on refund reference)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteRefundSyncRequest),
    curl_response: MerchanteRefundSyncResponse,
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
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(self.connector_base_url_refunds(req).to_string())
        }
    }
);

// RepeatPayment (MIT — D or P + stored card_id + cit_mit_indicator=M10x)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteRepeatPaymentRequest),
    curl_response: MerchanteRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

// SetupMandate (Verify $0 with store_card=Y, transaction_type = A)
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Merchante,
    curl_request: FormUrlEncoded(MerchanteSetupMandateRequest),
    curl_response: MerchanteSetupMandateResponse,
    flow_name: SetupMandate,
    resource_common_data: PaymentFlowData,
    flow_request: SetupMandateRequestData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_response: true,
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
            Ok(self.connector_base_url_payments(req).to_string())
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Merchante,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PaymentMethodToken,
        VoidPC,
        MandateRevoke,
    ],
    not_supported: [
        VoidPostRefund,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ServerSessionAuthenticationToken,
        ServerAuthenticationToken,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        ClientAuthenticationToken,
        IncrementalAuthorization,
        SubmitEvidence,
        DefendDispute,
        Accept,
    ],
);
